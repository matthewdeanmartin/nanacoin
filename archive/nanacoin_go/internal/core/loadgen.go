package core

import (
	"fmt"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

// Load generation: fill a service with a plausible household history.
//
// Why this is in the package rather than a test helper
//
// Two callers need it. Tests use it to measure what a year of use costs in
// RAM, and `nanacoin -seed` uses it to fill a desktop server so the client
// and the API can be exercised at realistic scale.
//
// That second use is the point: exercising the app against hundreds of
// transactions should not mean reflashing the board. Flash has a finite
// erase-cycle budget and firmware images occupy hundreds of sectors of it, so
// burning a flash cycle to answer "does the market page work with 200
// listings" is the wrong trade when the same code runs on a laptop.
//
// The generated history is deliberately shaped like a household's rather than
// uniform: most transactions are small transfers, some are purchases, a few
// are corrections, and the amounts and memos repeat the way real ones do -
// which matters because repetition is exactly what string interning exploits.

// SeedOptions describes the history to generate.
type SeedOptions struct {
	// Members is how many household members besides Nana.
	Members int
	// Weeks of history to generate.
	Weeks int
	// PerWeek is the economic events per week. The spec's estimate is ten.
	PerWeek int
	// Listings posted over the whole period.
	Listings int
	// Password given to every generated member, so a seeded server can be
	// logged into.
	Password string
}

// DefaultSeed is a year of household use at the spec's estimated volume.
func DefaultSeed() SeedOptions {
	return SeedOptions{
		Members:  3,
		Weeks:    52,
		PerWeek:  10,
		Listings: 40,
		Password: "seed-pin",
	}
}

// SeedResult reports what was generated.
type SeedResult struct {
	Members      int
	Transactions int
	Listings     int
	Sold         int
	Reversals    int
}

// Seed fills the service with a generated history.
//
// Provisions Nana if the service is empty, so it works on a fresh journal.
// Every operation goes through the ordinary public methods - Transfer, Issue,
// Purchase, Reverse - so what is generated is reachable state rather than
// something constructed behind the model's back. A seeded ledger balances for
// the same reason a real one does.
func (s *Service) Seed(opts SeedOptions) (SeedResult, error) {
	var res SeedResult

	if opts.Password == "" {
		opts.Password = "seed-pin"
	}

	nana, err := s.seedNana(opts.Password)
	if err != nil {
		return res, err
	}

	// Members, each with the configured starting grant.
	members := make([]*users.User, 0, opts.Members)
	for i := 0; i < opts.Members; i++ {
		name := seedNames[i%len(seedNames)]
		if i >= len(seedNames) {
			name = fmt.Sprintf("%s%d", name, i/len(seedNames)+1)
		}
		u, err := s.CreateUser(nana, name, title(name), opts.Password, users.RoleUser, true)
		if err != nil {
			return res, fmt.Errorf("creating %s: %w", name, err)
		}
		members = append(members, u)
		res.Members++
	}
	if len(members) < 2 {
		return res, fmt.Errorf("seeding needs at least 2 members, got %d", len(members))
	}

	// Issue a float so the transfers below cannot run out of funds. Without
	// this the generator quietly stops moving money partway through and
	// reports a history it did not actually create.
	total := opts.Weeks * opts.PerWeek
	for _, m := range members {
		if _, err := s.Issue(nana, m.Account, ledger.Amount(total*2), "Seed float"); err != nil {
			return res, fmt.Errorf("issuing float: %w", err)
		}
	}

	// Listings, spread over the period.
	listings := make([]ledger.ListingID, 0, opts.Listings)
	for i := 0; i < opts.Listings; i++ {
		seller := members[i%len(members)]
		l, err := s.CreateListing(seller, ListingInput{
			Title:       seedListings[i%len(seedListings)],
			Description: seedDescriptions[i%len(seedDescriptions)],
			Price:       ledger.Amount(5 + (i*7)%40),
		})
		if err != nil {
			return res, fmt.Errorf("creating listing %d: %w", i, err)
		}
		listings = append(listings, l.ID)
		res.Listings++
	}

	// The economic events themselves.
	var recent []ledger.TransactionID
	for i := 0; i < total; i++ {
		from := members[i%len(members)]
		to := members[(i+1)%len(members)]

		switch {
		// Roughly one in eight is a purchase, while listings remain.
		case i%8 == 3 && len(listings) > 0:
			id := listings[0]
			listings = listings[1:]
			if l, _, err := s.Purchase(from, id); err == nil {
				res.Sold++
				_ = l
				continue
			}
			// A purchase can legitimately fail - own listing, already sold -
			// so fall through to a transfer rather than abandoning the seed.
			fallthrough

		default:
			txn, err := s.Transfer(from, to.Account, ledger.Amount(1+i%9), seedMemos[i%len(seedMemos)])
			if err != nil {
				return res, fmt.Errorf("transfer %d: %w", i, err)
			}
			res.Transactions++
			recent = append(recent, txn.ID)
			if len(recent) > 20 {
				recent = recent[1:]
			}
		}

		// Roughly one in forty gets corrected, which is what makes reversals
		// appear in a seeded history at a believable rate.
		if i%40 == 39 && len(recent) > 0 {
			if _, err := s.Reverse(nana, recent[0], "Seeded correction"); err == nil {
				res.Reversals++
			}
			recent = recent[1:]
		}
	}

	res.Transactions = s.Status().Transactions
	return res, nil
}

// seedNana returns the existing Nana, provisioning one if the service is
// empty.
func (s *Service) seedNana(password string) (*users.User, error) {
	for _, u := range s.Users() {
		if u.IsNana() {
			return u, nil
		}
	}
	return s.Provision("nana", "Nana", password, "Seeded House")
}

func title(s string) string {
	if s == "" {
		return s
	}
	return string(s[0]-32) + s[1:]
}

// Names, titles and memos that repeat the way a household's do - which is
// what makes a seeded history a fair test of string interning rather than a
// pathological one.
var (
	seedNames = []string{"alice", "bob", "carol", "dave", "erin", "frank"}

	seedListings = []string{
		"Do your dishes", "Old LEGO set", "One hour of Switch time",
		"$5 USD cash", "Cookies", "Walk the dog", "Mow the lawn",
		"Take out the trash", "Vacuum the stairs", "Wash the car",
	}

	seedDescriptions = []string{
		"", "Peanut butter", "Uninterrupted", "Includes the bin",
		"Front and back", "Needs doing before Friday",
	}

	seedMemos = []string{
		"chores", "Taking out the trash", "dishes", "thanks",
		"allowance", "for the LEGO", "lawn", "babysitting", "",
	}
)
