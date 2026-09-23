//! Versioned, bounded rows. Private credentials are deliberately stored here,
//! never by changing the public State/Member serializers used by HTTP.
use super::*;
use crate::domain::*;
use serde::de::DeserializeOwned;

pub const ROW_BYTES: usize = 4096;
pub const MAX_ROWS: usize = 1
    + crate::fulfillment::CAPACITY
    + MEMBERS
    + LISTINGS
    + HISTORY
    + crate::offers::OFFERS
    + crate::forex::QUOTES
    + crate::loans::LOANS
    + crate::lotto::LOTTOS
    + THINGS
    + MAX_RECORDS;

pub(super) fn save_empty<J: Journal>(j: &mut J) -> Result<(), Error> {
    let header = Header {
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
    j.commit_checkpoint(1)
}

#[derive(Serialize, Deserialize)]
struct Header {
    fulfillments: usize,
    decimals: u8,
    money_epoch: u64,
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

fn write<J: Journal>(
    j: &mut J,
    index: &mut usize,
    kind: u8,
    value: &impl Serialize,
) -> Result<(), Error> {
    let mut row = [0u8; ROW_BYTES];
    row[..4].copy_from_slice(b"NCS1");
    row[4] = kind;
    let len = serde_json_core::to_slice(value, &mut row[16..]).map_err(|_| Error::Capacity)?;
    row[8..12].copy_from_slice(&(len as u32).to_le_bytes());
    let crc = crc32fast::hash(&row[16..16 + len]);
    row[12..16].copy_from_slice(&crc.to_le_bytes());
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
    if used < 16 || &row[..4] != b"NCS1" || row[4] != kind || row[5..8] != [0; 3] {
        return Err(Error::CorruptJournal);
    }
    let len = u32::from_le_bytes(row[8..12].try_into().unwrap()) as usize;
    if len != used - 16
        || crc32fast::hash(&row[16..used]) != u32::from_le_bytes(row[12..16].try_into().unwrap())
    {
        return Err(Error::CorruptJournal);
    }
    let mut scratch = [0; ROW_BYTES];
    let (value, consumed) = serde_json_core::from_slice_escaped(&row[16..used], &mut scratch)
        .map_err(|_| Error::CorruptJournal)?;
    if consumed != len {
        return Err(Error::CorruptJournal);
    }
    Ok(value)
}

pub(super) fn save<J: Journal>(
    j: &mut J,
    s: &State,
    keys: &VecDeque<KeyReceipt>,
) -> Result<(), Error> {
    let h = Header {
        decimals: s.decimals,
        money_epoch: s.money_epoch,
        credit_blocked: s.credit_blocked,
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
        history: s.history.len(),
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
    for x in &s.history {
        write(j, &mut index, 3, x)?;
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
    j.commit_checkpoint(index)
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
    if h.fulfillments > crate::fulfillment::CAPACITY
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
        || 1 + h.fulfillments
            + h.members
            + h.listings
            + h.history
            + h.offers
            + h.quotes
            + h.things
            + h.loans
            + h.lottos
            + h.keys
            != j.checkpoint_rows()
        || h.sequence > MAX_SEQUENCE
    {
        return Err(Error::CorruptJournal);
    }
    s.household_name = h.household_name;
    s.decimals = h.decimals;
    s.money_epoch = h.money_epoch;
    s.credit_blocked = h.credit_blocked;
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
