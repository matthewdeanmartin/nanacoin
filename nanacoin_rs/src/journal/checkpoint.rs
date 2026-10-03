//! Versioned, bounded rows. Private credentials are deliberately stored here,
//! never by changing the public State/Member serializers used by HTTP.
use super::*;
use crate::domain::*;
use serde::de::DeserializeOwned;

pub const ROW_BYTES: usize = 4096;
pub const MAX_ROWS: usize = 1
    + crate::fulfillment::CAPACITY
    + MEMBERS
    // Trailing rows: the extension header, read keys, bot members and the
    // tickets of members 17-32, then full API keys.
    + 1
    + 3 * MEMBERS
    + crate::lotto::LOTTOS
    + LISTINGS
    + HISTORY
    + crate::offers::OFFERS
    + crate::forex::QUOTES
    + crate::loans::LOANS
    + crate::lotto::LOTTOS
    + THINGS
    + MAX_RECORDS
    + crate::ledger::CORRECTIONS
    + crate::ledger::EPOCHS
    + crate::ledger::AUDIT_CACHE
    + crate::commerce::REQUESTS
    + crate::commerce::ARTWORKS;

pub(super) fn save_empty<J: Journal>(
    j: &mut J,
    _old: archive::ArchiveHead,
) -> Result<archive::ArchiveHead, Error> {
    let archive = archive::ArchiveHead {
        first_page: 0,
        next_page: 0,
        incarnation: j.generation() + 1,
        ..Default::default()
    };
    let header = Header {
        archive,
        corrections: 0,
        epochs: 0,
        audits: 0,
        requests: 0,
        artworks: 0,
        decimals: 4,
        money_epoch: 0,
        credit_blocked: 0,
        fulfillments: 0,
        loans: 0,
        lottos: 0,
        lotto_escrow: 0,
        household_name: Name::new(),
        initial_grant: 1_000_000,
        currency: Name::try_from("NanaCoin").unwrap(),
        offer_settles_after: crate::offers::DEFAULT_SETTLEMENT,
        last_timestamp: 0,
        sequence: 0,
        transactions: 0,
        issuance_balance: 0,
        usd_issuance_balance: 0,
        members: 0,
        listings: 0,
        history: 0,
        offers: 0,
        quotes: 0,
        things: 0,
        keys: 0,
    };
    j.begin_checkpoint()?;
    write(j, &mut 0, 0, &header)?;
    j.commit_checkpoint(1)?;
    Ok(archive)
}

#[derive(Serialize, Deserialize)]
struct Header {
    archive: archive::ArchiveHead,
    corrections: usize,
    epochs: usize,
    audits: usize,
    requests: usize,
    artworks: usize,
    fulfillments: usize,
    decimals: u8,
    money_epoch: u64,
    /// Members 1-16; members 17-32 are in [`Extension`].
    credit_blocked: u16,
    loans: usize,
    lottos: usize,
    lotto_escrow: i64,
    household_name: Name,
    initial_grant: i64,
    currency: Name,
    offer_settles_after: u64,
    last_timestamp: u64,
    sequence: u64,
    transactions: u64,
    issuance_balance: i64,
    usd_issuance_balance: i64,
    members: usize,
    listings: usize,
    history: usize,
    offers: usize,
    quotes: usize,
    #[serde(default)]
    things: usize,
    keys: usize,
}
#[derive(Serialize)]
struct PrivateMember<'a> {
    member: &'a Member,
    password: &'a Option<crate::auth::PasswordVerifier>,
    token_hash: &'a TokenHash,
    last_command: &'a TokenHash,
    last_sequence: u64,
}
#[derive(Deserialize)]
struct StoredMember {
    member: Member,
    password: Option<crate::auth::PasswordVerifier>,
    token_hash: TokenHash,
    last_command: TokenHash,
    last_sequence: u64,
}

/// A member's API key digest. Written after every counted section, one row
/// per member who has a key, so a checkpoint made before API keys existed
/// (no such rows) restores unchanged and the header layout never moved.
#[derive(Serialize, Deserialize)]
struct ApiKeyRow {
    member: MemberId,
    key: TokenHash,
    created: u64,
}
const API_KEY_ROW: u8 = 16;

/// Everything stored after the release with API keys, without touching any
/// older row (spec/FORWARD_COMPATIBLE_DATA_CHANGES.md, pattern 2): one
/// extension header right after the counted sections, then the rows it
/// counts, then the API key rows. Older checkpoints have no extension.
///
/// `values` grows by appending slots; a shorter list (from older firmware)
/// reads missing slots as zero. Never reorder or reuse a slot.
#[derive(Serialize, Deserialize, Default)]
struct Extension {
    values: heapless::Vec<u64, 16>,
}
const EXTENSION_ROW: u8 = 17;
/// Slots of [`Extension::values`].
const EXT_READ_KEYS: usize = 0;
const EXT_BOTS: usize = 1;
const EXT_LOTTO_TICKETS: usize = 2;
/// `credit_blocked` bits for members 17-32 (a value, not a count).
const EXT_CREDIT_HIGH: usize = 3;

impl Extension {
    fn get(&self, slot: usize) -> u64 {
        self.values.get(slot).copied().unwrap_or(0)
    }
}

/// A member's read-only API key digest.
#[derive(Serialize, Deserialize)]
struct ReadKeyRow {
    member: MemberId,
    key: TokenHash,
    created: u64,
}
const READ_KEY_ROW: u8 = 18;

/// A bot member (`MemberKind::Bot`); members without a row are people.
#[derive(Serialize, Deserialize)]
struct BotRow {
    member: MemberId,
}
const BOT_ROW: u8 = 19;

/// Lotto tickets of members 17-32: the lotto row keeps the first 16, as
/// before members 17-32 existed.
#[derive(Serialize, Deserialize)]
struct LottoTicketsRow {
    lotto: u64,
    tickets: [u32; 16],
}
const LOTTO_TICKETS_ROW: u8 = 20;

/// The kind byte of row `index`, without consuming it.
fn peek_kind<J: Journal>(j: &mut J, index: usize) -> Result<u8, Error> {
    let mut row = [0; ROW_BYTES];
    let used = j.read_checkpoint(index, &mut row)?;
    if used < 16 || &row[..4] != b"NCS2" {
        return Err(Error::CorruptJournal);
    }
    Ok(row[4])
}

fn write<J: Journal>(
    j: &mut J,
    index: &mut usize,
    kind: u8,
    value: &impl Serialize,
) -> Result<(), Error> {
    let mut row = [0u8; ROW_BYTES];
    row[..4].copy_from_slice(b"NCS2");
    row[4] = kind;
    let len = postcard::to_slice(value, &mut row[16..])
        .map_err(|_| Error::Capacity)?
        .len();
    row[8..12].copy_from_slice(&(len as u32).to_le_bytes());
    let mut crc = crc32fast::Hasher::new();
    crc.update(&row[..12]);
    crc.update(&row[16..16 + len]);
    row[12..16].copy_from_slice(&crc.finalize().to_le_bytes());
    j.write_checkpoint(*index, &row[..16 + len])?;
    *index += 1;
    Ok(())
}

fn read<J: Journal, T: DeserializeOwned>(
    j: &mut J,
    index: &mut usize,
    kind: u8,
) -> Result<T, Error> {
    let mut row = [0; ROW_BYTES];
    let used = j.read_checkpoint(*index, &mut row)?;
    *index += 1;
    if !(16..=ROW_BYTES).contains(&used)
        || &row[..4] != b"NCS2"
        || row[4] != kind
        || row[5..8] != [0; 3]
    {
        return Err(Error::CorruptJournal);
    }
    let len = u32::from_le_bytes(row[8..12].try_into().unwrap()) as usize;
    let mut crc = crc32fast::Hasher::new();
    crc.update(&row[..12]);
    crc.update(&row[16..used]);
    if len != used - 16 || crc.finalize() != u32::from_le_bytes(row[12..16].try_into().unwrap()) {
        return Err(Error::CorruptJournal);
    }
    let (value, remaining) =
        postcard::take_from_bytes(&row[16..used]).map_err(|_| Error::CorruptJournal)?;
    if !remaining.is_empty() {
        return Err(Error::CorruptJournal);
    }
    Ok(value)
}

pub(super) fn save<J: Journal>(
    j: &mut J,
    s: &State,
    keys: &VecDeque<KeyReceipt>,
) -> Result<archive::ArchiveHead, Error> {
    let archive = if j.supports_archive() {
        archive::prepare(j, s, &s.archive)?
    } else {
        s.archive
    };
    let h = Header {
        archive,
        corrections: s.ledger.corrections.len(),
        epochs: s.ledger.epochs.len(),
        audits: if j.supports_archive() {
            0
        } else {
            s.ledger.audit.len()
        },
        requests: s.commerce.requests.len(),
        artworks: s.commerce.artworks.len(),
        decimals: s.decimals,
        money_epoch: s.money_epoch,
        credit_blocked: s.credit_blocked as u16,
        fulfillments: s.fulfillments.len(),
        loans: s.loans.len(),
        lottos: s.lottos.len(),
        lotto_escrow: s.lotto_escrow,
        household_name: s.household_name.clone(),
        initial_grant: s.initial_grant,
        currency: s.currency.clone(),
        offer_settles_after: s.offer_settles_after,
        last_timestamp: s.last_timestamp,
        sequence: s.sequence,
        transactions: s.transactions,
        issuance_balance: s.issuance_balance,
        usd_issuance_balance: s.usd_issuance_balance,
        members: s.members.len(),
        listings: s.listings.len(),
        history: if j.supports_archive() {
            0
        } else {
            s.history.len()
        },
        offers: s.offers.len(),
        quotes: s.quotes.len(),
        things: s.things.len(),
        keys: keys.len(),
    };
    j.begin_checkpoint()?;
    let mut index = 0;
    write(j, &mut index, 0, &h)?;
    for m in &s.members {
        write(
            j,
            &mut index,
            1,
            &PrivateMember {
                member: m,
                password: &m.password,
                token_hash: &m.token_hash,
                last_command: &m.last_command,
                last_sequence: m.last_sequence,
            },
        )?;
    }
    for x in &s.listings {
        write(j, &mut index, 2, x)?;
    }
    if !j.supports_archive() {
        for x in &s.history {
            write(j, &mut index, 3, x)?;
        }
    }
    for x in &s.offers {
        write(j, &mut index, 4, x)?;
    }
    for x in &s.quotes {
        write(j, &mut index, 5, x)?;
    }
    for x in &s.things {
        write(j, &mut index, 7, x)?;
    }
    for x in &s.loans {
        write(j, &mut index, 8, x)?;
    }
    for x in &s.lottos {
        write(j, &mut index, 9, x)?;
    }
    for x in &s.fulfillments {
        write(j, &mut index, 10, x)?;
    }
    for x in keys {
        write(j, &mut index, 6, x)?;
    }
    for x in &s.ledger.corrections {
        write(j, &mut index, 11, x)?;
    }
    for x in &s.ledger.epochs {
        write(j, &mut index, 12, x)?;
    }
    if !j.supports_archive() {
        for x in &s.ledger.audit {
            write(j, &mut index, 13, x)?;
        }
    }
    for x in &s.commerce.requests {
        write(j, &mut index, 14, x)?;
    }
    for x in &s.commerce.artworks {
        write(j, &mut index, 15, x)?;
    }
    let read_keys = s.members.iter().filter(|m| m.read_key != [0; 32]);
    let bots = s.members.iter().filter(|m| m.kind == MemberKind::Bot);
    let high_tickets = s
        .lottos
        .iter()
        .filter(|l| l.tickets[16..].iter().any(|n| *n != 0));
    let mut ext = Extension::default();
    for value in [
        read_keys.clone().count() as u64,
        bots.clone().count() as u64,
        high_tickets.clone().count() as u64,
        u64::from(s.credit_blocked >> 16),
    ] {
        ext.values.push(value).map_err(|_| Error::Capacity)?;
    }
    write(j, &mut index, EXTENSION_ROW, &ext)?;
    for m in read_keys {
        let row = ReadKeyRow {
            member: m.id,
            key: m.read_key,
            created: m.read_key_created,
        };
        write(j, &mut index, READ_KEY_ROW, &row)?;
    }
    for m in bots {
        write(j, &mut index, BOT_ROW, &BotRow { member: m.id })?;
    }
    for l in high_tickets {
        let row = LottoTicketsRow {
            lotto: l.id,
            tickets: l.tickets[16..].try_into().unwrap(),
        };
        write(j, &mut index, LOTTO_TICKETS_ROW, &row)?;
    }
    for m in s.members.iter().filter(|m| m.api_key != [0; 32]) {
        let row = ApiKeyRow {
            member: m.id,
            key: m.api_key,
            created: m.api_key_created,
        };
        write(j, &mut index, API_KEY_ROW, &row)?;
    }
    j.commit_checkpoint(index)?;
    Ok(archive)
}

pub(super) fn restore<J: Journal>(
    j: &mut J,
    s: &mut State,
    keys: &mut VecDeque<KeyReceipt>,
) -> Result<(), Error> {
    if j.checkpoint_rows() == 0 {
        return Ok(());
    }
    let mut index = 0;
    let h: Header = read(j, &mut index, 0)?;
    let counted = 1
        + h.corrections
        + h.epochs
        + h.audits
        + h.requests
        + h.artworks
        + h.fulfillments
        + h.members
        + h.listings
        + h.history
        + h.offers
        + h.quotes
        + h.things
        + h.loans
        + h.lottos
        + h.keys;
    // After the counted sections: an extension (newer firmware), then API key rows.
    let api_keys = j.checkpoint_rows().checked_sub(counted);
    if h.corrections > crate::ledger::CORRECTIONS
        || h.epochs > crate::ledger::EPOCHS
        || h.audits > crate::ledger::AUDIT_CACHE
        || h.requests > crate::commerce::REQUESTS
        || h.artworks > crate::commerce::ARTWORKS
        || h.fulfillments > crate::fulfillment::CAPACITY
        || h.decimals > 8
        || h.money_epoch > MAX_SEQUENCE
        || h.loans > crate::loans::LOANS
        || h.lottos > crate::lotto::LOTTOS
        || h.members > MEMBERS
        || h.listings > LISTINGS
        || h.history > HISTORY
        || h.offers > crate::offers::OFFERS
        || h.quotes > crate::forex::QUOTES
        || h.things > THINGS
        || h.keys > MAX_RECORDS
        || api_keys.is_none()
        || h.sequence > MAX_SEQUENCE
    {
        return Err(Error::CorruptJournal);
    }
    s.archive = h.archive;
    s.household_name = h.household_name;
    s.decimals = h.decimals;
    s.money_epoch = h.money_epoch;
    s.credit_blocked = u32::from(h.credit_blocked);
    s.lotto_escrow = h.lotto_escrow;
    s.currency = h.currency;
    s.initial_grant = h.initial_grant;
    s.offer_settles_after = h.offer_settles_after;
    s.last_timestamp = h.last_timestamp;
    s.sequence = h.sequence;
    s.transactions = h.transactions;
    s.issuance_balance = h.issuance_balance;
    s.usd_issuance_balance = h.usd_issuance_balance;
    for _ in 0..h.members {
        let mut m: StoredMember = read(j, &mut index, 1)?;
        m.member.password = m.password;
        m.member.token_hash = m.token_hash;
        m.member.last_command = m.last_command;
        m.member.last_sequence = m.last_sequence;
        if m.member.id.0 as usize != s.members.len() + 1 || m.member.last_sequence > s.sequence {
            return Err(Error::CorruptJournal);
        }
        s.members
            .push(m.member)
            .map_err(|_| Error::CorruptJournal)?;
    }
    for _ in 0..h.listings {
        s.listings
            .push(read(j, &mut index, 2)?)
            .map_err(|_| Error::CorruptJournal)?;
    }
    for _ in 0..h.history {
        s.history.push_back(read(j, &mut index, 3)?);
    }
    for _ in 0..h.offers {
        s.offers
            .push(read(j, &mut index, 4)?)
            .map_err(|_| Error::CorruptJournal)?;
    }
    for _ in 0..h.quotes {
        s.quotes
            .push(read(j, &mut index, 5)?)
            .map_err(|_| Error::CorruptJournal)?;
    }
    for _ in 0..h.things {
        s.things
            .push(read(j, &mut index, 7)?)
            .map_err(|_| Error::CorruptJournal)?;
    }
    for _ in 0..h.loans {
        s.loans
            .push(read(j, &mut index, 8)?)
            .map_err(|_| Error::CorruptJournal)?;
    }
    for _ in 0..h.lottos {
        s.lottos
            .push(read(j, &mut index, 9)?)
            .map_err(|_| Error::CorruptJournal)?;
    }
    for _ in 0..h.fulfillments {
        s.fulfillments.push(read(j, &mut index, 10)?);
    }
    for _ in 0..h.keys {
        let key: KeyReceipt = read(j, &mut index, 6)?;
        if key.sequence > s.sequence
            || keys
                .iter()
                .any(|k| k.key == key.key && k.actor == key.actor)
        {
            return Err(Error::CorruptJournal);
        }
        keys.push_back(key);
    }
    for _ in 0..h.corrections {
        s.ledger.corrections.push(read(j, &mut index, 11)?);
    }
    if h.epochs > 0 {
        s.ledger.epochs.clear();
    }
    for _ in 0..h.epochs {
        s.ledger.epochs.push(read(j, &mut index, 12)?);
    }
    for _ in 0..h.audits {
        s.ledger.audit.push_back(read(j, &mut index, 13)?);
    }
    for _ in 0..h.requests {
        s.commerce.requests.push(read(j, &mut index, 14)?);
    }
    for _ in 0..h.artworks {
        s.commerce.artworks.push(read(j, &mut index, 15)?);
    }
    if index < j.checkpoint_rows() && peek_kind(j, index)? == EXTENSION_ROW {
        let ext: Extension = read(j, &mut index, EXTENSION_ROW)?;
        let count = |slot: usize, most: usize| -> Result<usize, Error> {
            let n = ext.get(slot);
            if n > most as u64 {
                return Err(Error::CorruptJournal);
            }
            Ok(n as usize)
        };
        let high = ext.get(EXT_CREDIT_HIGH);
        if high > u64::from(u16::MAX) {
            return Err(Error::CorruptJournal);
        }
        s.credit_blocked |= (high as u32) << 16;
        for _ in 0..count(EXT_READ_KEYS, s.members.len())? {
            let row: ReadKeyRow = read(j, &mut index, READ_KEY_ROW)?;
            let m = s
                .members
                .iter_mut()
                .find(|m| m.id == row.member)
                .ok_or(Error::CorruptJournal)?;
            if row.key == [0; 32] || m.read_key != [0; 32] || row.created > s.last_timestamp {
                return Err(Error::CorruptJournal);
            }
            m.read_key = row.key;
            m.read_key_created = row.created;
        }
        for _ in 0..count(EXT_BOTS, s.members.len())? {
            let row: BotRow = read(j, &mut index, BOT_ROW)?;
            let m = s
                .members
                .iter_mut()
                .find(|m| m.id == row.member)
                .ok_or(Error::CorruptJournal)?;
            if m.kind == MemberKind::Bot || m.role == Role::Nana {
                return Err(Error::CorruptJournal);
            }
            m.kind = MemberKind::Bot;
        }
        for _ in 0..count(EXT_LOTTO_TICKETS, s.lottos.len())? {
            let row: LottoTicketsRow = read(j, &mut index, LOTTO_TICKETS_ROW)?;
            let l = s
                .lottos
                .iter_mut()
                .find(|l| l.id == row.lotto)
                .ok_or(Error::CorruptJournal)?;
            if l.tickets[16..].iter().any(|n| *n != 0) {
                return Err(Error::CorruptJournal);
            }
            l.tickets[16..].copy_from_slice(&row.tickets);
        }
    }
    // The rest are API key rows, at most one per member.
    let api_keys = j.checkpoint_rows() - index;
    if api_keys > s.members.len() {
        return Err(Error::CorruptJournal);
    }
    for _ in 0..api_keys {
        let row: ApiKeyRow = read(j, &mut index, API_KEY_ROW)?;
        let m = s
            .members
            .iter_mut()
            .find(|m| m.id == row.member)
            .ok_or(Error::CorruptJournal)?;
        if row.key == [0; 32] || m.api_key != [0; 32] || row.created > s.last_timestamp {
            return Err(Error::CorruptJournal);
        }
        m.api_key = row.key;
        m.api_key_created = row.created;
    }
    if j.supports_archive() {
        if h.archive.transactions != s.transactions || h.archive.audit_sequence != s.sequence {
            return Err(Error::CorruptJournal);
        }
        archive::restore_history(j, &h.archive, s)?;
    }
    s.check_invariants()
}

/// Small atomic publication record shared by file and NVS adapters.
pub fn head(generation: u64, rows: usize) -> [u8; 32] {
    let mut out = [0; 32];
    out[..4].copy_from_slice(b"NCH1");
    out[4..12].copy_from_slice(&generation.to_le_bytes());
    out[12..16].copy_from_slice(&(rows as u32).to_le_bytes());
    let crc = crc32fast::hash(&out[..28]);
    out[28..].copy_from_slice(&crc.to_le_bytes());
    out
}
pub fn parse_head(data: &[u8]) -> Result<(u64, usize), Error> {
    if data.len() != 32
        || &data[..4] != b"NCH1"
        || crc32fast::hash(&data[..28]) != u32::from_le_bytes(data[28..].try_into().unwrap())
    {
        return Err(Error::CorruptJournal);
    }
    let generation = u64::from_le_bytes(data[4..12].try_into().unwrap());
    let rows = u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;
    if generation == 0 || generation > MAX_SEQUENCE || rows == 0 || rows > MAX_ROWS {
        return Err(Error::CorruptJournal);
    }
    Ok((generation, rows))
}
