//! Public aggregate inventory and bounded read-only benchmarks. Never calls
//! execute/tick, cleans expired records, or serializes credentials or contents.
use crate::{
    api::serialize,
    domain::*,
    journal::{Journal, Service, FRAME_SIZE, MAX_RECORDS},
};
use serde::Serialize;
use std::{mem::size_of, time::Instant};

#[derive(Serialize)]
pub struct Collection {
    pub name: &'static str,
    pub persistence: &'static str,
    pub used: usize,
    pub capacity: usize,
    pub free: usize,
    pub active: Option<usize>,
    pub payload_reserved_bytes: usize,
    pub retention: &'static str,
}
fn row<T>(
    name: &'static str,
    used: usize,
    capacity: usize,
    reserved: usize,
    retention: &'static str,
) -> Collection {
    Collection {
        name,
        persistence: "journal + checkpoint",
        used,
        capacity,
        free: capacity.saturating_sub(used),
        active: None,
        payload_reserved_bytes: reserved * size_of::<T>(),
        retention,
    }
}

#[derive(Serialize)]
pub struct Inventory {
    pub storage_codec: &'static str,
    pub archive: crate::journal::archive::ArchiveHead,
    pub archive_page_slots: usize,
    pub archive_rotation_pending_limit: usize,
    pub sample_transaction_json_bytes: Option<usize>,
    pub sample_transaction_binary_bytes: Option<usize>,
    pub engine: &'static str,
    pub generation: u64,
    pub sequence: u64,
    pub storage_failed: bool,
    pub invariants_ok: bool,
    pub model_reserved_bytes: usize,
    pub memory_note: &'static str,
    pub checkpoint_supported: bool,
    pub checkpoint_rows: usize,
    pub checkpoint_row_capacity: usize,
    pub checkpoint_row_max_bytes: usize,
    pub journal_records: usize,
    pub journal_record_capacity: usize,
    pub journal_free_records: usize,
    pub checkpoint_after: usize,
    pub journal_frame_bytes: usize,
    pub journal_logical_bytes: usize,
    pub lifetime_transactions: u64,
    pub oldest_retained_sequence: Option<u64>,
    pub newest_retained_sequence: Option<u64>,
    pub oldest_retained_at: Option<u64>,
    pub newest_retained_at: Option<u64>,
    pub collections: Vec<Collection>,
    pub indexes: &'static str,
}

pub fn inventory<J: Journal>(service: &Service<J>) -> Inventory {
    let s = service.state();
    let (checkpoint_rows, keys, key_capacity, key_bytes) = service.diagnostic_storage();
    let mut members = row::<Member>(
        "Members, balances and retry watermarks",
        s.members.len(),
        MEMBERS,
        MEMBERS,
        "Retained; disabled members still occupy a slot",
    );
    members.active = Some(s.members.iter().filter(|m| !m.disabled).count());
    let mut listings = row::<Listing>(
        "Listings",
        s.listings.len(),
        LISTINGS,
        LISTINGS,
        "Closed listings may be recycled once dependent offers permit it",
    );
    listings.active = Some(
        s.listings
            .iter()
            .filter(|l| l.status == ListingStatus::Active)
            .count(),
    );
    let mut offers = row::<crate::offers::Offer>(
        "Offers",
        s.offers.len(),
        crate::offers::OFFERS,
        crate::offers::OFFERS,
        "Terminal/settled offers may be recycled; open offers retained",
    );
    offers.active = Some(
        s.offers
            .iter()
            .filter(|o| matches!(o.phase, crate::offers::OfferPhase::Open))
            .count(),
    );
    let mut loans = row::<crate::loans::Loan>(
        "Loans and credit agreements",
        s.loans.len(),
        crate::loans::LOANS,
        crate::loans::LOANS,
        "Bounded agreement book; terms and outstanding debt are durable",
    );
    loans.active = Some(
        s.loans
            .iter()
            .filter(|l| {
                matches!(
                    l.status,
                    crate::loans::LoanStatus::Offered
                        | crate::loans::LoanStatus::Armed
                        | crate::loans::LoanStatus::Active
                )
            })
            .count(),
    );
    let mut lottos = row::<crate::lotto::Lotto>(
        "Lottos (including ticket counts per member)",
        s.lottos.len(),
        crate::lotto::LOTTOS,
        crate::lotto::LOTTOS,
        "Tickets and settlement progress embedded; completed pools may be recycled",
    );
    lottos.active = Some(s.lottos.iter().filter(|l| l.step < 19).count());
    let mut fulfillments = row::<crate::fulfillment::Fulfillment>("Physical fulfillment and recent updates", s.fulfillments.len(), crate::fulfillment::CAPACITY, s.fulfillments.capacity(), "Four updates per obligation; completed/reversed obligations may be recycled after payment leaves recent history");
    fulfillments.active = Some(
        s.fulfillments
            .iter()
            .filter(|f| {
                matches!(
                    f.status,
                    crate::fulfillment::Status::Todo | crate::fulfillment::Status::Disputed
                )
            })
            .count(),
    );
    let mut quotes = row::<crate::forex::Quote>(
        "Foreign-exchange quotes",
        s.quotes.len(),
        crate::forex::QUOTES,
        crate::forex::QUOTES,
        "Bounded quote book; both currency postings commit atomically",
    );
    quotes.active = Some(
        s.quotes
            .iter()
            .filter(|q| q.status == crate::forex::QuoteStatus::Open)
            .count(),
    );
    let mut collections = vec![members, listings,
        row::<Thing>("Goods and services catalog", s.things.len(), THINGS, THINGS, "Durable bounded catalog"),
        row::<Transaction>("Recent transactions / messages", s.history.len(), HISTORY, s.history.capacity(), "Oldest first; bounded recent view, not a permanent full audit archive"),
        offers, quotes, loans, lottos, fulfillments,
        Collection { name: "Idempotency receipts", persistence: "journal + checkpoint", used: keys, capacity: MAX_RECORDS, free: MAX_RECORDS.saturating_sub(keys), active: None,
            payload_reserved_bytes: key_capacity * key_bytes, retention: "Bounded replay window; prevents duplicate writes within retention" },
        Collection { name: "Household configuration and accounting metadata", persistence: "journal + checkpoint header", used: 1, capacity: 1, free: 0, active: None,
            payload_reserved_bytes: 0, retention: "Household/currency, decimals/epoch, grants, settlement window, issuance/escrow balances, sequence/counters and scheduler bitmap; included in model total" },
    ];
    for ((used, active, capacity, bytes), (name, retention)) in
        service.auth.inventory().into_iter().zip([
            (
                "Login sessions",
                "Expires after eight hours; cleared at reboot",
            ),
            (
                "OAuth authorization codes",
                "One-use codes expire after 60 seconds; cleared at reboot",
            ),
            (
                "Login failure / lockout cache",
                "Five-minute window; reclaimed during authentication; cleared at reboot",
            ),
        ])
    {
        collections.push(Collection {
            name,
            persistence: "RAM only",
            used,
            capacity,
            free: capacity - used,
            active: Some(active),
            payload_reserved_bytes: bytes,
            retention,
        });
    }
    let incidents = crate::incidents::LOG.snapshot();
    collections.push(Collection {
        name: "Incident history",
        persistence: "RAM only",
        used: incidents.events.len(),
        capacity: 48,
        free: 48 - incidents.events.len(),
        active: None,
        payload_reserved_bytes: 48 * size_of::<crate::incidents::Event>(),
        retention: "Coalesced error/state events; overwritten ring, cleared at reboot",
    });
    collections.push(Collection {
        name: "Health sample history",
        persistence: "RAM only",
        used: incidents.samples.len(),
        capacity: 32,
        free: 32 - incidents.samples.len(),
        active: None,
        payload_reserved_bytes: 32 * size_of::<crate::incidents::Sample>(),
        retention: "Five-second sampling on board; about 160 seconds, cleared at reboot",
    });
    let mut sample = [0; 2048];
    let json_bytes = s
        .history
        .back()
        .and_then(|t| serde_json_core::to_slice(t, &mut sample).ok());
    let binary_bytes = s
        .history
        .back()
        .and_then(|t| postcard::to_slice(t, &mut sample).ok().map(|v| v.len()));
    Inventory { storage_codec: "postcard-1/NCR2/NCS2/NCA2", archive:s.archive, archive_page_slots:crate::journal::archive::SLOTS, archive_rotation_pending_limit:1024, sample_transaction_json_bytes:json_bytes, sample_transaction_binary_bytes:binary_bytes, engine: "Bounded in-memory model with durable event journal and checkpoints",
        generation: service.generation(), sequence: s.sequence, storage_failed: service.storage_failed(),
        invariants_ok: s.check_invariants().is_ok(), model_reserved_bytes: service.diagnostic_model_bytes(),
        memory_note: "RAM estimates use this build's Rust layouts, not flash sizes. Collection payloads omit container/allocator overhead; metadata is included in model total. Incident recorder, transport buffers, task stacks and native storage allocations are outside the model total. Embedded ticket/update rows are not separate tables. Browser-local caches are outside the board.",
        checkpoint_supported: service.checkpoint_supported(), checkpoint_rows,
        checkpoint_row_capacity: crate::journal::checkpoint::MAX_ROWS,
        checkpoint_row_max_bytes: crate::journal::checkpoint::ROW_BYTES,
        journal_records: service.journal_records(), journal_record_capacity: MAX_RECORDS,
        journal_free_records: MAX_RECORDS.saturating_sub(service.journal_records()), checkpoint_after: 2048,
        journal_frame_bytes: FRAME_SIZE, journal_logical_bytes: service.journal_records() * FRAME_SIZE,
        lifetime_transactions: s.transactions,
        oldest_retained_sequence: s.history.front().map(|t| t.id), newest_retained_sequence: s.history.back().map(|t| t.id),
        oldest_retained_at: s.history.front().map(|t| t.created_at), newest_retained_at: s.history.back().map(|t| t.created_at),
        collections, indexes: "Primary-key lookups and filters scan bounded arrays/deques. No SQL engine, B-tree, query planner or per-table flash allocation. Checkpoints publish the bounded archive; logical frame bytes exclude NVS/flash overhead and inactive checkpoint banks." }
}

#[derive(Serialize)]
pub struct Configuration<'a> {
    pub household_name: &'a str,
    pub currency: &'a str,
    pub decimals: u8,
    pub minor_units_per_coin: u64,
    pub smallest_unit: &'static str,
    pub money_epoch: u64,
    pub initial_grant: i64,
    pub offer_settles_after: u64,
    pub usd_decimals: u8,
    pub maximum_amount_minor: i64,
    pub lending_enabled: bool,
    pub lending_policy: &'static str,
    pub rate_basis_points_per_percent: u32,
    pub savings_lotto_holding_seconds: u64,
    pub terms_policy: &'static str,
}
pub fn configuration<J: Journal>(s: &Service<J>) -> Configuration<'_> {
    let s = s.state();
    Configuration { household_name: &s.household_name, currency: &s.currency, decimals: s.decimals,
        minor_units_per_coin: 10u64.pow(s.decimals as u32), smallest_unit: "Amounts are stored as integer minor units; decimal precision is the household display scale",
        money_epoch: s.money_epoch, initial_grant: s.initial_grant, offer_settles_after: s.offer_settles_after,
        usd_decimals: 2, maximum_amount_minor: MAX_AMOUNT, lending_enabled: true,
        lending_policy: "Cash funded; automatic credit cannot borrow from issuance or recursively fund debt payments",
        rate_basis_points_per_percent: 100, savings_lotto_holding_seconds: crate::lotto::MONTH,
        terms_policy: "Loan rates, payment periods and lotto prices/rates are stored per agreement, not as household defaults" }
}

#[derive(Serialize)]
struct QueryResult {
    name: &'static str,
    runs: usize,
    min_us: u64,
    mean_us: u64,
    max_us: u64,
    response_bytes: usize,
    error: Option<Error>,
}
#[derive(Serialize)]
struct Benchmark {
    generation: u64,
    sequence: u64,
    read_only: bool,
    budget_ms: u64,
    elapsed_us: u64,
    queries: Vec<QueryResult>,
    note: &'static str,
}

/// Immutable service access is deliberate: no scheduler, authentication cleanup,
/// commands, compaction or synthetic data. Actual existing query serializers.
pub fn benchmark<J: Journal>(service: &Service<J>, output: &mut [u8]) -> Result<usize, Error> {
    let start = Instant::now();
    let mut results = Vec::with_capacity(4);
    for name in [
        "Household status + invariants",
        "Recent public ledger (up to 100)",
        "Active listings (sort, filter, projection)",
        "Retained transaction lookup",
    ] {
        let mut row = QueryResult {
            name,
            runs: 0,
            min_us: u64::MAX,
            mean_us: 0,
            max_us: 0,
            response_bytes: 0,
            error: None,
        };
        for _ in 0..3 {
            if start.elapsed().as_millis() >= 250 {
                break;
            }
            let began = Instant::now();
            let result = match name {
                "Household status + invariants" => crate::client::status(
                    service.state(),
                    service.journal_records(),
                    service.generation(),
                    service.checkpoint_supported(),
                    output,
                ),
                "Recent public ledger (up to 100)" => {
                    crate::client::public_ledger(service.state(), 100, output)
                }
                "Active listings (sort, filter, projection)" => {
                    crate::client::listings_query(service.state(), Some("ACTIVE"), output)
                }
                _ => match service.state().history.iter().find(|t| t.amount != 0) {
                    Some(t) => crate::client::public_transaction(
                        service.state(),
                        &format!("tx-{}", t.id),
                        output,
                    ),
                    None => Err(Error::NotFound),
                },
            };
            let us = began.elapsed().as_micros() as u64;
            row.runs += 1;
            row.min_us = row.min_us.min(us);
            row.max_us = row.max_us.max(us);
            row.mean_us += us;
            match result {
                Ok(bytes) => {
                    std::hint::black_box(&output[..bytes]);
                    row.response_bytes = bytes;
                }
                Err(error) => {
                    row.error = Some(error);
                    break;
                }
            }
        }
        if row.runs > 0 {
            row.mean_us /= row.runs as u64;
        } else {
            row.min_us = 0;
        }
        results.push(row);
    }
    serialize(&Benchmark { generation: service.generation(), sequence: service.state().sequence, read_only: true,
        budget_ms: 250, elapsed_us: start.elapsed().as_micros() as u64, queries: results,
        note: "Server-side query + serialization only, excluding network/TLS and waiting for the service lock. Up to three runs/query; no further query starts after 250 ms (an in-flight bounded query may exceed it). Missing transaction is expected on an empty ledger. No writes, tick, checkpoint or seeded data." }, output)
}
