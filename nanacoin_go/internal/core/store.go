package core

import (
	"sort"
	"strings"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

// The packed domain store: users, accounts and listings as flat arrays.
//
// # Why this exists
//
// The ledger was packed first and it worked - PackedTransaction is 40 bytes,
// pointer-free, held in one flat slice that the collector scans as a single
// opaque block. Forty-five transactions cost nothing measurable.
//
// Everything else stayed as maps of pointers:
//
//	users    map[ledger.UserID]*users.User
//	accounts map[ledger.AccountID]*users.Account
//	listings map[ledger.ListingID]*marketplace.Listing
//
// Each value there is an individual heap allocation holding four to six Go
// strings, each of which is *another* allocation. One listing with a 500-byte
// description is a dozen scattered objects. Create and abandon a few dozen
// between other allocations and the heap is interleaved with permanent
// objects that can never be collected and transient ones that can - which is
// precisely the arrangement that fragments a non-moving collector.
//
// Measured on the board: headroom halved every round under the torture suite
// (28 kB -> 17 kB -> gone) while the ledger itself grew by almost nothing.
// The domain objects were the leak, not the money.
//
// # What this does
//
// The same treatment the ledger already got. Every record is a fixed-size,
// pointer-free struct in a preallocated array:
//
//   - repeated strings (IDs, usernames, account names) become ledger.Ref,
//     an index into the shared intern table
//   - unique strings (listing titles and descriptions) become ledger.Slot,
//     an offset into a fixed byte arena
//   - cross-references become array indices, not pointers or string IDs
//
// The result is three arrays the collector treats as three objects, whatever
// they contain. The memory cost is fixed at startup and printed at boot; it
// does not change as the household uses the system, and it cannot fragment
// because nothing inside is ever individually allocated or freed.
//
// # The limits are real and deliberate
//
// A full store refuses the operation rather than growing. That is the whole
// point: a bounded system that says no is usable, and an unbounded one that
// falls over is not. The caps below are generous for a household and small
// enough to fit.

const (
	// MaxUsers is the household size ceiling. Ten people is a large
	// household; sixteen leaves room and keeps the scan trivial.
	MaxUsers = 16

	// MaxAccounts is one per user today, with room for a user to gain a
	// second account later without a format change.
	MaxAccounts = 32

	// MaxListings is how many listings exist at once, in any status.
	//
	// Sold and cancelled listings stay in the table because the ledger
	// references them and a purchase should still be able to name what was
	// bought. When the table fills, the oldest closed listing is recycled -
	// see recycleListing. Active listings are never recycled.
	MaxListings = 48
)

// packedUser is a household member. 32 bytes, no pointers.
type packedUser struct {
	CreatedAt int64

	ID       ledger.Ref // interned user ID
	Username ledger.Ref // interned, lowercased for lookup
	Display  ledger.Ref
	Account  ledger.Ref
	Verifier ledger.Slot // the PBKDF2 hash; unique per user, so arena not intern

	Role   uint8
	Status uint8

	// InUse marks an occupied slot. A zero-value record is free, which is
	// what makes the array usable without a separate free list.
	InUse bool
}

// packedAccount is the money-holding half of a user. 24 bytes.
type packedAccount struct {
	CreatedAt int64

	ID     ledger.Ref
	UserID ledger.Ref
	Name   ledger.Ref

	Status uint8
	InUse  bool
}

// packedListing is a marketplace offer. 56 bytes.
//
// Title and Description go to the arena rather than the intern table: they
// are genuinely unique per listing, so interning them would fill the table
// with single-use entries and defeat its purpose.
type packedListing struct {
	CreatedAt  int64
	UpdatedAt  int64
	Price      ledger.Amount
	MinorUnits int64

	// Seller and Buyer are interned: they are account IDs, of which a
	// household has a handful. ID is not - see writeListing.
	Seller ledger.Ref
	Buyer  ledger.Ref

	ID          ledger.Slot
	Title       ledger.Slot
	Description ledger.Slot

	// SoldTx is the ledger sequence number of the purchase, not a string ID.
	// Zero means unsold.
	SoldTx uint32

	Currency ledger.Ref
	Kind     uint8
	Status   uint8

	// Side is SELL or BUY. It sits here beside the other single-byte fields
	// because that padding was already being paid for: a want-ad costs the
	// board nothing over a listing.
	Side uint8

	InUse bool
}

// store holds every domain object, in fixed arrays.
//
// Not safe for concurrent use on its own: the Service holds one lock over all
// of it, which at household scale is the right amount of machinery.
type store struct {
	listingGeneration [MaxListings]uint64
	strs              *ledger.Strings
	arena             *ledger.Arena

	usersArr    [MaxUsers]packedUser
	accountsArr [MaxAccounts]packedAccount
	listingsArr [MaxListings]packedListing
	offersArr   [MaxOffers]packedOffer
	quotesArr   [MaxQuotes]packedQuote

	// Counts of occupied slots, so callers do not scan to answer "how many".
	nUsers                  int
	nAccounts               int
	nListings               int
	nOffers                 int
	nQuotes                 int
	listingCapacityReported bool
	listingText             ledger.TextReplacement
}

func newStore(strs *ledger.Strings, arena *ledger.Arena) *store {
	return &store{strs: strs, arena: arena}
}

// --- users ------------------------------------------------------------------

// findUser returns the index of a user by ID, or -1.
//
// A linear scan, deliberately. At MaxUsers=16 a scan of pointer-free memory
// beats a map lookup - no hashing, no pointer chasing, and the whole array
// fits in a couple of cache lines. It also removes the map's per-entry
// overhead, which was larger than the records it pointed at.
func (s *store) findUser(id ledger.UserID) int {
	ref := s.strs.Find(string(id))
	if ref == 0 {
		return -1
	}
	for i := range s.usersArr {
		if s.usersArr[i].InUse && s.usersArr[i].ID == ref {
			return i
		}
	}
	return -1
}

// findUserByName resolves a username case-insensitively.
func (s *store) findUserByName(username string) int {
	ref := s.strs.Find(strings.ToLower(strings.TrimSpace(username)))
	if ref == 0 {
		return -1
	}
	for i := range s.usersArr {
		if s.usersArr[i].InUse && s.usersArr[i].Username == ref {
			return i
		}
	}
	return -1
}

// putUser stores a user, returning its index. Reports false when full.
func (s *store) putUser(u *users.User) (int, bool) {
	if i := s.findUser(u.ID); i >= 0 {
		s.writeUser(i, u)
		return i, true
	}
	for i := range s.usersArr {
		if !s.usersArr[i].InUse {
			s.writeUser(i, u)
			s.nUsers++
			return i, true
		}
	}
	return -1, false
}

// writeUser packs a user into slot i.
//
// Keep unchanged verifier text in place. Changed fields release their old
// arena slots before storing replacements.
func (s *store) writeUser(i int, u *users.User) {
	prev := s.usersArr[i]

	verifier := prev.Verifier
	if !prev.InUse || s.arena.Get(prev.Verifier) != u.Verifier {
		s.arena.Release(prev.Verifier)
		verifier = s.arena.Put(u.Verifier)
	}

	s.usersArr[i] = packedUser{
		CreatedAt: u.CreatedAt,
		ID:        s.strs.Intern(string(u.ID)),
		Username:  s.strs.Intern(strings.ToLower(u.Username)),
		Display:   s.strs.Intern(u.DisplayName),
		Account:   s.strs.Intern(string(u.Account)),
		Verifier:  verifier,
		Role:      packRole(u.Role),
		Status:    packStatus(u.Status),
		InUse:     true,
	}
}

// unpackUser rebuilds the public form.
//
// Allocates a User per call, like ledger.Unpack, and for the same reason:
// reads are bounded by a page size and happen a handful of times per view,
// while the packed record is held for the life of the household. A scratch
// buffer was considered and rejected - the API layer holds these across
// calls (a *users.User travels from authenticate() into every handler), so
// handing out reused storage would be a use-after-free waiting to happen.
func (s *store) unpackUser(i int) *users.User {
	p := &s.usersArr[i]
	if !p.InUse {
		return nil
	}
	return &users.User{
		ID:          ledger.UserID(s.strs.Lookup(p.ID)),
		Username:    s.strs.Lookup(p.Username),
		DisplayName: s.strs.Lookup(p.Display),
		Role:        unpackRole(p.Role),
		Status:      unpackStatus(p.Status),
		Account:     ledger.AccountID(s.strs.Lookup(p.Account)),
		CreatedAt:   p.CreatedAt,
		Verifier:    s.arena.Get(p.Verifier),
	}
}

// --- accounts ---------------------------------------------------------------

func (s *store) findAccount(id ledger.AccountID) int {
	ref := s.strs.Find(string(id))
	if ref == 0 {
		return -1
	}
	for i := range s.accountsArr {
		if s.accountsArr[i].InUse && s.accountsArr[i].ID == ref {
			return i
		}
	}
	return -1
}

func (s *store) putAccount(a *users.Account) (int, bool) {
	if i := s.findAccount(a.ID); i >= 0 {
		s.writeAccount(i, a)
		return i, true
	}
	for i := range s.accountsArr {
		if !s.accountsArr[i].InUse {
			s.writeAccount(i, a)
			s.nAccounts++
			return i, true
		}
	}
	return -1, false
}

func (s *store) writeAccount(i int, a *users.Account) {
	s.accountsArr[i] = packedAccount{
		CreatedAt: a.CreatedAt,
		ID:        s.strs.Intern(string(a.ID)),
		UserID:    s.strs.Intern(string(a.UserID)),
		Name:      s.strs.Intern(a.Name),
		Status:    packStatus(a.Status),
		InUse:     true,
	}
}

func (s *store) unpackAccount(i int) *users.Account {
	p := &s.accountsArr[i]
	if !p.InUse {
		return nil
	}
	return &users.Account{
		ID:        ledger.AccountID(s.strs.Lookup(p.ID)),
		UserID:    ledger.UserID(s.strs.Lookup(p.UserID)),
		Name:      s.strs.Lookup(p.Name),
		Status:    unpackStatus(p.Status),
		CreatedAt: p.CreatedAt,
	}
}

// accountName resolves an account to its display name without allocating a
// whole Account. Used by the view layer on every posting of every rendered
// transaction, which is the hottest read path there is.
func (s *store) accountName(id ledger.AccountID) string {
	if id == ledger.SystemIssuance {
		return "Issuance"
	}
	if i := s.findAccount(id); i >= 0 {
		return s.strs.Lookup(s.accountsArr[i].Name)
	}
	return string(id)
}

// --- listings ---------------------------------------------------------------

func (s *store) findListing(id ledger.ListingID) int {
	if id == "" {
		return -1
	}
	// Compares arena text rather than an interned Ref. A scan of at most
	// MaxListings short strings costs less than the permanent table slot
	// interning each unique ID would have consumed.
	want := string(id)
	for i := range s.listingsArr {
		p := &s.listingsArr[i]
		if p.InUse && s.arena.Equal(p.ID, want) {
			return i
		}
	}
	return -1
}

// putListing stores a listing, recycling the oldest closed one when full.
func (s *store) listingSlot(l *marketplace.Listing) int {
	if i := s.findListing(l.ID); i >= 0 {
		return i
	}
	for i := range s.listingsArr {
		if !s.listingsArr[i].InUse {
			if s.listingFits(nil, l) {
				return i
			}
			break
		}
	}
	// Text pressure can fill before the row table. Closed offers remain
	// recyclable history; active offers are never sacrificed.
	oldest, oldestAt := -1, int64(1<<62)
	for i := range s.listingsArr {
		p := &s.listingsArr[i]
		if !p.InUse || p.Status == statusActive || p.UpdatedAt >= oldestAt {
			continue
		}
		if s.listingFits(p, l) {
			oldest, oldestAt = i, p.UpdatedAt
		}
	}
	return oldest
}
func (s *store) canWriteListing(l *marketplace.Listing) bool {
	i := s.listingSlot(l)
	if i < 0 {
		s.reportListingCapacity(l, i)
		return false
	}
	p := &s.listingsArr[i]
	if p.InUse && s.arena.Get(p.ID) == string(l.ID) && s.arena.Get(p.Title) == l.Title && s.arena.Get(p.Description) == l.Description {
		return true
	}
	ok := s.listingFits(p, l)
	if !ok {
		s.reportListingCapacity(l, i)
	}
	return ok
}
func (s *store) putListing(l *marketplace.Listing) (int, bool) {
	i := s.listingSlot(l)
	if i < 0 {
		return -1, false
	}
	existed := s.listingsArr[i].InUse
	if !s.writeListing(i, l) {
		return -1, false
	}
	if !existed {
		s.nListings++
	}
	return i, true
}

// recycleListing frees the oldest sold-or-cancelled slot, or -1 if every
// listing is still active.
//
// Active listings are never recycled: a household member's offer must not
// vanish because someone else posted something. A table full of active
// listings therefore refuses new ones, which is the correct answer - the
// alternative is deleting something a person is waiting on.
func (s *store) recycleListing() int {
	oldest, oldestAt := -1, int64(1<<62)
	for i := range s.listingsArr {
		p := &s.listingsArr[i]
		if !p.InUse || p.Status == statusActive {
			continue
		}
		if p.UpdatedAt < oldestAt {
			oldest, oldestAt = i, p.UpdatedAt
		}
	}
	return oldest
}

func (s *store) writeListing(i int, l *marketplace.Listing) bool {
	var soldTx uint32
	if l.SoldTx != "" {
		if seq, ok := ledger.SeqForTransactionID(l.SoldTx); ok {
			soldTx = seq
		}
	}
	prev := s.listingsArr[i]
	title, desc, id := prev.Title, prev.Description, prev.ID
	idChanged := !prev.InUse || s.arena.Get(prev.ID) != string(l.ID)
	if idChanged || s.arena.Get(prev.Title) != l.Title || s.arena.Get(prev.Description) != l.Description {
		s.prepareListingText(&prev, l)
		ok := s.arena.Replace(&s.listingText)
		s.listingText.Values = [3]string{}
		slots := &s.listingText.Slots
		if !ok {
			return false
		}
		title, desc, id = slots[0], slots[1], slots[2]
	}
	if idChanged {
		s.listingGeneration[i]++
	}

	s.listingsArr[i] = packedListing{
		CreatedAt:  l.CreatedAt,
		UpdatedAt:  l.UpdatedAt,
		Price:      l.Price,
		MinorUnits: l.MinorUnits,
		// The ID goes to the arena, not the intern table. Listing IDs are
		// unique per listing and a recycled slot creates another, so
		// interning them burned a permanent table entry per listing ever
		// created - the same unbounded growth memos had.
		ID:          id,
		Seller:      s.strs.Intern(string(l.Seller)),
		Buyer:       s.strs.Intern(string(l.Buyer)),
		Title:       title,
		Description: desc,
		SoldTx:      soldTx,
		Currency:    s.strs.Intern(l.Currency),
		Kind:        packListingKind(l.Kind),
		Status:      packListingStatus(l.Status),
		Side:        uint8(l.Side),
		InUse:       true,
	}
	return true
}

func (s *store) unpackListing(i int) *marketplace.Listing {
	p := &s.listingsArr[i]
	if !p.InUse {
		return nil
	}
	l := &marketplace.Listing{
		ID:          ledger.ListingID(s.arena.Get(p.ID)),
		Seller:      ledger.AccountID(s.strs.Lookup(p.Seller)),
		Title:       s.arena.Get(p.Title),
		Description: s.arena.Get(p.Description),
		Price:       p.Price,
		Quantity:    1,
		Status:      unpackListingStatus(p.Status),
		CreatedAt:   p.CreatedAt,
		UpdatedAt:   p.UpdatedAt,
		Buyer:       ledger.AccountID(s.strs.Lookup(p.Buyer)),
		Kind:        unpackListingKind(p.Kind),
		Currency:    s.strs.Lookup(p.Currency),
		MinorUnits:  p.MinorUnits,
		Side:        marketplace.Side(p.Side),
	}
	if p.SoldTx != 0 {
		l.SoldTx = ledger.TransactionIDFor(p.SoldTx)
	}
	return l
}

// --- enum packing -----------------------------------------------------------
//
// Small integers rather than strings, for the same reason as everywhere else:
// a `Role` is a Go string, and a string field is a pointer.

const (
	roleUser uint8 = iota
	roleNana
)

func packRole(r users.Role) uint8 {
	if r == users.RoleNana {
		return roleNana
	}
	return roleUser
}

func unpackRole(r uint8) users.Role {
	if r == roleNana {
		return users.RoleNana
	}
	return users.RoleUser
}

const (
	statusActive uint8 = iota
	statusDisabled
	statusSold
	statusCancelled
)

func packStatus(st users.Status) uint8 {
	if st == users.StatusDisabled {
		return statusDisabled
	}
	return statusActive
}

func unpackStatus(st uint8) users.Status {
	if st == statusDisabled {
		return users.StatusDisabled
	}
	return users.StatusActive
}

func packListingStatus(st marketplace.Status) uint8 {
	switch st {
	case marketplace.StatusSold:
		return statusSold
	case marketplace.StatusCancelled:
		return statusCancelled
	}
	return statusActive
}

func unpackListingStatus(st uint8) marketplace.Status {
	switch st {
	case statusSold:
		return marketplace.StatusSold
	case statusCancelled:
		return marketplace.StatusCancelled
	}
	return marketplace.StatusActive
}

const (
	listingKindNanaCoin uint8 = iota
	listingKindCurrency
)

func packListingKind(k string) uint8 {
	if k == "currency" {
		return listingKindCurrency
	}
	return listingKindNanaCoin
}

func unpackListingKind(k uint8) string {
	if k == listingKindCurrency {
		return "currency"
	}
	return ""
}

// --- budget -----------------------------------------------------------------

// StoreBytes is what the packed domain store costs, fixed at startup.
//
// Present so it can be asserted by a test and printed at boot. The whole
// point of this design is that this number is knowable and never changes;
// a figure nobody can state is a figure nobody is controlling.
const StoreBytes = MaxUsers*32 + MaxAccounts*24 + MaxListings*56 + ledger.ArenaSize

// --- Service accessors ------------------------------------------------------
//
// These keep the call sites in market.go, users.go, money.go and queries.go
// reading the way they did when the state was maps. The packing is a storage
// decision and the business logic should not be rewritten around it - the
// same argument the ledger made when it packed transactions and kept Unpack.
//
// All of them require the service lock to be held.

// userByID returns a user, or ErrUserNotFound.
func (s *Service) userByID(id ledger.UserID) (*users.User, error) {
	i := s.store.findUser(id)
	if i < 0 {
		return nil, ErrUserNotFound
	}
	return s.store.unpackUser(i), nil
}

// userByName resolves a username, or nil when absent. Nil rather than an
// error because the caller distinguishes "no such user" from "wrong password"
// only in how long it takes, never in what it says.
func (s *Service) userByName(username string) *users.User {
	i := s.store.findUserByName(username)
	if i < 0 {
		return nil
	}
	return s.store.unpackUser(i)
}

// listingByID returns a listing, or ErrListingUnknown.
func (s *Service) listingByID(id ledger.ListingID) (*marketplace.Listing, error) {
	i := s.store.findListing(id)
	if i < 0 {
		return nil, ErrListingUnknown
	}
	return s.store.unpackListing(i), nil
}

// accountByID returns an account, or nil.
func (s *Service) accountByID(id ledger.AccountID) *users.Account {
	i := s.store.findAccount(id)
	if i < 0 {
		return nil
	}
	return s.store.unpackAccount(i)
}

// eachUserLocked walks users in creation order.
//
// Sorted by an index slice of at most MaxUsers entries rather than by
// building a slice of unpacked users, so a listing of the household costs one
// small slice of ints rather than sixteen heap objects.
func (s *Service) eachUserLocked(fn func(*users.User) bool) {
	idx := make([]int, 0, MaxUsers)
	for i := range s.store.usersArr {
		if s.store.usersArr[i].InUse {
			idx = append(idx, i)
		}
	}
	sort.Slice(idx, func(a, b int) bool {
		ua, ub := &s.store.usersArr[idx[a]], &s.store.usersArr[idx[b]]
		if ua.CreatedAt != ub.CreatedAt {
			return ua.CreatedAt < ub.CreatedAt
		}
		return ua.ID < ub.ID
	})
	for _, i := range idx {
		if !fn(s.store.unpackUser(i)) {
			return
		}
	}
}

// eachListingLocked walks listings newest first, optionally filtered.
func (s *Service) eachListingLocked(status marketplace.Status, fn func(*marketplace.Listing) bool) {
	want := packListingStatus(status)
	var indices [MaxListings]int
	idx := indices[:0]
	generations := s.store.listingGeneration
	for i := range s.store.listingsArr {
		p := &s.store.listingsArr[i]
		if !p.InUse {
			continue
		}
		if status != "" && p.Status != want {
			continue
		}
		idx = append(idx, i)
	}
	sort.Slice(idx, func(a, b int) bool {
		la, lb := &s.store.listingsArr[idx[a]], &s.store.listingsArr[idx[b]]
		if la.CreatedAt != lb.CreatedAt {
			return la.CreatedAt > lb.CreatedAt
		}
		return s.store.arena.Get(la.ID) < s.store.arena.Get(lb.ID)
	})
	for _, i := range idx {
		if generations[i] != s.store.listingGeneration[i] {
			continue
		}
		if status != "" && s.store.listingsArr[i].Status != want {
			continue
		}
		if !fn(s.store.unpackListing(i)) {
			return
		}
	}
}

// countActiveNanaLocked is how many enabled Nanas exist.
func (s *Service) countNanaLocked() int {
	n := 0
	for i := range s.store.usersArr {
		p := &s.store.usersArr[i]
		if p.InUse && p.Role == roleNana && p.Status == statusActive {
			n++
		}
	}
	return n
}

// snapshotListings unpacks every listing, newest first.
//
// For the slice-returning Listings query, which the desktop uses. The board
// uses EachListing instead and never builds this.
func (s *Service) snapshotListings() []*marketplace.Listing {
	out := make([]*marketplace.Listing, 0, MaxListings)
	s.eachListingLocked("", func(l *marketplace.Listing) bool {
		out = append(out, l)
		return true
	})
	return out
}

// actorStatusLocked re-reads an actor's live role and status from the store.
//
// # Why this exists
//
// Callers hand the service a *users.User they obtained earlier. When state
// lived in maps that pointer aliased the stored object, so a user disabled
// mid-session was seen as disabled by every later call - correct, but by
// accident: it depended on the caller and the store sharing memory.
//
// The packed store returns copies, so that accident is gone and a stale
// actor would be trusted. Rather than restore the aliasing (which would mean
// handing out pointers into the table and losing the whole point), every
// entry point that cares re-reads the authoritative record here.
//
// This is the safer arrangement regardless of packing: authorization should
// be decided from the service's own state, not from what a caller passes in.
func (s *Service) actorStatusLocked(actor *users.User) (isActive, isNana bool, ok bool) {
	if actor == nil {
		return false, false, false
	}
	i := s.store.findUser(actor.ID)
	if i < 0 {
		return false, false, false
	}
	p := &s.store.usersArr[i]
	return p.Status == statusActive, p.Role == roleNana, true
}

// requireActiveLocked is the common guard: the actor must still exist and
// still be enabled.
func (s *Service) requireActiveLocked(actor *users.User) error {
	active, _, ok := s.actorStatusLocked(actor)
	if !ok {
		return ErrUserNotFound
	}
	if !active {
		return ErrDisabled
	}
	return nil
}

// requireNanaLocked is the guard for administrative operations.
func (s *Service) requireNanaLocked(actor *users.User) error {
	active, nana, ok := s.actorStatusLocked(actor)
	if !ok {
		return ErrUserNotFound
	}
	if !active {
		return ErrDisabled
	}
	if !nana {
		return ErrForbidden
	}
	return nil
}

// One bounded serial snapshot per boot. No formatting allocation and no user
// text/passwords: enough structure to distinguish row exhaustion and text fit.
func (s *store) reportListingCapacity(l *marketplace.Listing, selected int) {
	if s.listingCapacityReported {
		return
	}
	s.listingCapacityReported = true
	used, capacity, truncated := s.arena.Stats()
	println("listing capacity: selected", selected, "rows", s.nListings, "text", used, "of", capacity, "truncated", truncated, "requested title/description/id", len(l.Title), len(l.Description), len(l.ID))
	for i := range s.listingsArr {
		p := &s.listingsArr[i]
		if !p.InUse {
			continue
		}
		fits := s.listingFits(p, l)
		println("listing slot", i, "status", p.Status, "updated", p.UpdatedAt, "title", p.Title.Off, p.Title.Len, "description", p.Description.Off, p.Description.Len, "id", p.ID.Off, p.ID.Len, "fits", fits)
	}
}

func (s *store) prepareListingText(p *packedListing, l *marketplace.Listing) {
	s.listingText.Old = [3]ledger.Slot{}
	if p != nil {
		s.listingText.Old = [3]ledger.Slot{p.Title, p.Description, p.ID}
	}
	s.listingText.Values = [3]string{l.Title, l.Description, string(l.ID)}
}
func (s *store) listingFits(p *packedListing, l *marketplace.Listing) bool {
	s.prepareListingText(p, l)
	ok := s.arena.PlanReplacement(&s.listingText)
	s.listingText.Values = [3]string{}
	return ok
}
