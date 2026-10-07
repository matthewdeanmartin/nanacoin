use super::*;
use crate::journal::archive::{self, ArchiveRecord};

fn parameter<'a>(query: &'a str, name: &str) -> Option<&'a str> {
    query.split('&').find_map(|v| {
        v.split_once('=')
            .filter(|(k, _)| *k == name)
            .map(|(_, v)| v)
    })
}

#[derive(Clone, Copy)]
struct Cursor {
    incarnation: u64,
    upper: u64,
    before: u64,
    page: u64,
}
impl Cursor {
    fn parse(value: &str) -> Result<Self, Error> {
        let mut p = value.split(':');
        let mut n = || {
            p.next()
                .ok_or(Error::InvalidInput)?
                .parse::<u64>()
                .map_err(|_| Error::InvalidInput)
        };
        let c = Self {
            incarnation: n()?,
            upper: n()?,
            before: n()?,
            page: n()?,
        };
        if p.next().is_some() {
            return Err(Error::InvalidInput);
        }
        Ok(c)
    }
    fn text(self) -> heapless::String<96> {
        use core::fmt::Write;
        let mut s = heapless::String::new();
        write!(
            s,
            "{}:{}:{}:{}",
            self.incarnation, self.upper, self.before, self.page
        )
        .unwrap();
        s
    }
}

/// Bounded page scan, with a continuation even when account filtering finds no rows.
pub(crate) fn page<J: Journal>(
    s: &mut Service<J>,
    query: &str,
    account: Option<(MemberId, bool)>,
    actor: Option<MemberId>,
    output: &mut [u8],
) -> Result<usize, Error> {
    let limit = parameter(query, "limit")
        .map(|v| v.parse::<usize>().map_err(|_| Error::InvalidInput))
        .transpose()?
        .unwrap_or(100);
    let head = s.state.archive;
    let mut cursor = if let Some(c) = parameter(query, "cursor") {
        Cursor::parse(c)?
    } else {
        Cursor {
            incarnation: head.incarnation,
            upper: s.state.transactions,
            before: parameter(query, "before")
                .map(|v| v.parse::<u64>().map_err(|_| Error::InvalidInput))
                .transpose()?
                .unwrap_or(s.state.transactions),
            page: head.next_page,
        }
    };
    #[cfg(feature = "cobol-core")]
    let limit = {
        let n = crate::cobol::ledger_number;
        crate::cobol::ledger(&[
            1,
            (cursor.incarnation == head.incarnation).into(),
            n(cursor.upper),
            n(cursor.before),
            n(cursor.page),
            n(s.state.transactions),
            n(head.next_page),
            n(head.first_page),
            n(limit as u64),
        ])?[16] as usize
    };
    #[cfg(not(feature = "cobol-core"))]
    let limit = limit.clamp(1, 100);
    #[cfg(not(feature = "cobol-core"))]
    if cursor.incarnation != head.incarnation
        || cursor.upper > s.state.transactions
        || cursor.before > cursor.upper
        || cursor.page > head.next_page
        || cursor.page < head.first_page
    {
        return Err(Error::StaleRequest);
    }
    s.page_rows.clear();
    let matches = |t: &Transaction| {
        #[cfg(feature = "cobol-core")]
        {
            let n = crate::cobol::ledger_number;
            crate::cobol::ledger(&[
                2,
                n(t.meta.ordinal),
                n(cursor.before),
                n(cursor.upper),
                t.amount,
                actor.is_some().into(),
                actor.map_or(0, |a| a.0.into()),
                t.from.0.into(),
                t.to.0.into(),
                account.is_some().into(),
                account.map_or(0, |(a, _)| a.0.into()),
                t.usd.into(),
                account.is_some_and(|(_, u)| u).into(),
            ])
            .expect("valid ledger filter ABI")[16]
                != 0
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            t.meta.ordinal < cursor.before
                && t.meta.ordinal < cursor.upper
                && (t.amount != 0 || actor.is_some_and(|a| t.from == a || t.to == a))
                && account.is_none_or(|(a, u)| t.usd == u && (t.from == a || t.to == a))
        }
    };
    for t in s
        .state
        .history
        .iter()
        .rev()
        .filter(|t| matches(t))
        .take(limit)
    {
        s.page_rows.push(t.clone());
    }
    let mut scanned = 0;
    while s.page_rows.len() < limit && cursor.page > head.first_page && scanned < 8 {
        cursor.page -= 1;
        scanned += 1;
        let rows = &mut s.page_rows;
        archive::scan_page(&mut s.journal, &head, cursor.page, |record| {
            if let ArchiveRecord::Transaction(t) = record {
                #[cfg(feature = "cobol-core")]
                if matches(&t) {
                    let min_index = crate::cobol::ledger_minimum(rows);
                    let unique = !rows.iter().any(|r| r.meta.ordinal == t.meta.ordinal);
                    let decision = crate::cobol::ledger(&[
                        5,
                        unique.into(),
                        rows.len() as i64,
                        limit as i64,
                        crate::cobol::ledger_number(t.meta.ordinal),
                        min_index.map_or(0, |i| crate::cobol::ledger_number(rows[i].meta.ordinal)),
                    ])?[16];
                    match decision {
                        1 => rows.push(t),
                        2 => rows[min_index.expect("full page has a minimum")] = t,
                        _ => {}
                    }
                }
                #[cfg(not(feature = "cobol-core"))]
                if matches(&t) && !rows.iter().any(|r| r.meta.ordinal == t.meta.ordinal) {
                    if rows.len() < limit {
                        rows.push(t);
                    } else if let Some((i, min)) =
                        rows.iter().enumerate().min_by_key(|(_, r)| r.meta.ordinal)
                    {
                        if t.meta.ordinal > min.meta.ordinal {
                            rows[i] = t;
                        }
                    }
                }
            }
            Ok(())
        })?;
        if s.page_rows.len() == limit {
            cursor.page += 1;
            break;
        }
    }
    #[cfg(feature = "cobol-core")]
    s.page_rows.sort_by_cached_key(|t| {
        crate::cobol::ledger(&[6, crate::cobol::ledger_number(t.meta.ordinal)])
            .expect("valid ledger order ABI")[16]
    });
    #[cfg(not(feature = "cobol-core"))]
    s.page_rows
        .sort_unstable_by_key(|t| core::cmp::Reverse(t.meta.ordinal));
    let next_before = s
        .page_rows
        .last()
        .map(|t| t.meta.ordinal)
        .unwrap_or(cursor.before);
    #[cfg(feature = "cobol-core")]
    let more = crate::cobol::ledger(&[
        3,
        crate::cobol::ledger_number(next_before),
        s.page_rows.len() as i64,
        limit as i64,
        crate::cobol::ledger_number(cursor.page),
        crate::cobol::ledger_number(head.first_page),
    ])?[16]
        != 0;
    #[cfg(not(feature = "cobol-core"))]
    let more = next_before > 0 && (s.page_rows.len() == limit || cursor.page > head.first_page);
    #[cfg(feature = "cobol-core")]
    let truncated = crate::cobol::ledger(&[
        4,
        crate::cobol::ledger_number(head.first_page),
        s.journal.supports_archive().into(),
        crate::cobol::ledger_number(s.state.transactions),
        s.state.history.len() as i64,
    ])?[16]
        != 0;
    #[cfg(not(feature = "cobol-core"))]
    let truncated = head.first_page != 0
        || (!s.journal.supports_archive() && s.state.transactions > s.state.history.len() as u64);
    cursor.before = next_before;
    #[derive(Serialize)]
    struct Page<T> {
        #[serde(skip_serializing_if = "Option::is_none")]
        account: Option<Id>,
        #[serde(skip_serializing_if = "Option::is_none")]
        balance: Option<i64>,
        transactions: T,
        circulation: i64,
        state_sequence: u64,
        snapshot_upper: u64,
        next_before: Option<u64>,
        next_cursor: Option<heapless::String<96>>,
        history_truncated: bool,
        archive_first_page: u64,
        oldest_available_ordinal: u64,
    }
    serialize(
        &Page {
            account: account.map(|(a, u)| currency_account(a, u)),
            balance: account
                .map(|(a, u)| account_balance(&s.state, a, u))
                .transpose()?,
            transactions: Rows(s.page_rows.iter().map(|t| transaction(&s.state, t))),
            circulation: super::circulation(&s.state),
            state_sequence: s.state.sequence,
            snapshot_upper: cursor.upper,
            next_before: more.then_some(next_before),
            next_cursor: more.then(|| cursor.text()),
            history_truncated: truncated,
            archive_first_page: head.first_page,
            oldest_available_ordinal: head.first_transaction,
        },
        output,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn route<J: Journal>(
    s: &mut Service<J>,
    actor: MemberId,
    method: &str,
    uri: &str,
    key: &str,
    body: &[u8],
    output: &mut [u8],
) -> Option<Result<usize, Error>> {
    let (path, query) = uri.split_once('?').unwrap_or((uri, ""));
    if method == "GET" && path == "/api/v1/totals" {
        return Some({
            #[derive(Serialize)]
            struct EpochView {
                epoch: usize,
                decimals: u8,
                exponent: i16,
                values: [heapless::String<40>; crate::ledger::FLOW_COUNT],
            }
            #[derive(Serialize)]
            struct Totals<T> {
                categories: [&'static str; crate::ledger::FLOW_COUNT],
                epochs: T,
                treasury: MemberId,
                sequence: u64,
            }
            serialize(
                &Totals {
                    categories: crate::ledger::FLOW_NAMES,
                    treasury: MemberId(1),
                    sequence: s.state.sequence,
                    epochs: Rows(s.state.ledger.epochs.iter().enumerate().map(|(epoch, e)| {
                        EpochView {
                            epoch,
                            decimals: e.decimals,
                            exponent: e.exponent,
                            values: std::array::from_fn(|i| {
                                use core::fmt::Write;
                                let mut v = heapless::String::new();
                                write!(v, "{}", e.flows[i]).unwrap();
                                v
                            }),
                        }
                    })),
                },
                output,
            )
        });
    }
    if method == "GET" && path == "/api/v1/audit" {
        return Some((|| {
            s.state.admin(actor)?;
            let head = s.state.archive;
            #[cfg(feature = "cobol-core")]
            let incarnation_matches = parameter(query, "incarnation")
                .is_none_or(|v| v.parse::<u64>().ok() == Some(head.incarnation));
            #[cfg(feature = "cobol-core")]
            crate::cobol::policy(
                53,
                &[
                    9,
                    incarnation_matches.into(),
                    crate::cobol::ledger_number(head.next_page),
                    crate::cobol::ledger_number(head.next_page),
                    crate::cobol::ledger_number(head.first_page),
                ],
            )?;
            #[cfg(not(feature = "cobol-core"))]
            if parameter(query, "incarnation")
                .is_some_and(|v| v.parse::<u64>().ok() != Some(head.incarnation))
            {
                return Err(Error::StaleRequest);
            }
            #[cfg(feature = "cobol-core")]
            let default_before =
                crate::cobol::ledger(&[13, crate::cobol::ledger_number(s.state.sequence)])?[16]
                    as u64;
            #[cfg(not(feature = "cobol-core"))]
            let default_before = s.state.sequence + 1;
            let before = parameter(query, "before")
                .map(|v| v.parse::<u64>().map_err(|_| Error::InvalidInput))
                .transpose()?
                .unwrap_or(default_before);
            let mut page = parameter(query, "page")
                .map(|v| v.parse::<u64>().map_err(|_| Error::InvalidInput))
                .transpose()?
                .unwrap_or(head.next_page);
            #[cfg(feature = "cobol-core")]
            crate::cobol::policy(
                53,
                &[
                    9,
                    incarnation_matches.into(),
                    crate::cobol::ledger_number(page),
                    crate::cobol::ledger_number(head.next_page),
                    crate::cobol::ledger_number(head.first_page),
                ],
            )?;
            #[cfg(not(feature = "cobol-core"))]
            if page > head.next_page || page < head.first_page {
                return Err(Error::StaleRequest);
            }
            s.audit_rows.clear();
            for a in s
                .state
                .ledger
                .audit
                .iter()
                .rev()
                .filter(|a| {
                    #[cfg(feature = "cobol-core")]
                    {
                        crate::cobol::ledger(&[
                            10,
                            crate::cobol::ledger_number(a.sequence),
                            crate::cobol::ledger_number(before),
                        ])
                        .expect("valid audit filter ABI")[16]
                            != 0
                    }
                    #[cfg(not(feature = "cobol-core"))]
                    {
                        a.sequence < before
                    }
                })
                .take(16)
            {
                s.audit_rows.push(a.clone());
            }
            for _ in 0..8 {
                if s.audit_rows.len() == 16 || page == head.first_page {
                    break;
                }
                page -= 1;
                let rows = &mut s.audit_rows;
                archive::scan_page(&mut s.journal, &head, page, |r| {
                    if let ArchiveRecord::Audit(a) = r {
                        #[cfg(feature = "cobol-core")]
                        {
                            let n = crate::cobol::ledger_number;
                            let visible =
                                crate::cobol::ledger(&[10, n(a.sequence), n(before)])?[16] != 0;
                            if visible {
                                let mut f = [0; 64];
                                f[0] = 8;
                                f[1] = rows.len() as i64;
                                for (i, row) in rows.iter().enumerate() {
                                    f[i + 2] = n(row.sequence);
                                }
                                let minimum = crate::cobol::ledger(&f)?;
                                let unique = !rows.iter().any(|v| v.sequence == a.sequence);
                                match crate::cobol::ledger(&[
                                    5,
                                    unique.into(),
                                    rows.len() as i64,
                                    16,
                                    n(a.sequence),
                                    minimum[63],
                                ])?[16]
                                {
                                    1 => rows.push(a),
                                    2 => rows[minimum[62] as usize] = a,
                                    _ => {}
                                }
                            }
                        }
                        #[cfg(not(feature = "cobol-core"))]
                        if a.sequence < before && !rows.iter().any(|v| v.sequence == a.sequence) {
                            if rows.len() < 16 {
                                rows.push(a);
                            } else if let Some((i, min)) =
                                rows.iter().enumerate().min_by_key(|(_, v)| v.sequence)
                            {
                                if a.sequence > min.sequence {
                                    rows[i] = a;
                                }
                            }
                        }
                    }
                    Ok(())
                })?;
                if s.audit_rows.len() == 16 {
                    page += 1;
                    break;
                }
            }
            #[cfg(feature = "cobol-core")]
            s.audit_rows.sort_by_cached_key(|a| {
                crate::cobol::ledger(&[6, crate::cobol::ledger_number(a.sequence)])
                    .expect("valid audit order ABI")[16]
            });
            #[cfg(not(feature = "cobol-core"))]
            s.audit_rows
                .sort_unstable_by_key(|a| core::cmp::Reverse(a.sequence));
            let next = s.audit_rows.last().map(|a| a.sequence).unwrap_or(before);
            #[cfg(feature = "cobol-core")]
            let more = crate::cobol::ledger(&[
                11,
                crate::cobol::ledger_number(next),
                s.audit_rows.len() as i64,
                crate::cobol::ledger_number(page),
                crate::cobol::ledger_number(head.first_page),
            ])?[16]
                != 0;
            #[cfg(not(feature = "cobol-core"))]
            let more = next > 1 && (s.audit_rows.len() == 16 || page > head.first_page);
            #[cfg(feature = "cobol-core")]
            let truncated =
                crate::cobol::ledger(&[12, crate::cobol::ledger_number(head.first_page)])?[16] != 0;
            #[cfg(not(feature = "cobol-core"))]
            let truncated = head.first_page > 0;
            #[derive(Serialize)]
            struct Response<'a> {
                audit: &'a [crate::ledger::Audit],
                next_before: Option<u64>,
                next_page: u64,
                truncated: bool,
                incarnation: u64,
            }
            serialize(
                &Response {
                    audit: &s.audit_rows,
                    next_before: more.then_some(next),
                    next_page: page,
                    truncated,
                    incarnation: head.incarnation,
                },
                output,
            )
        })());
    }
    if method == "GET" {
        if let Some(account) = path
            .strip_prefix("/api/v1/accounts/")
            .and_then(|p| p.strip_suffix("/transactions"))
        {
            return Some((|| {
                let a = parse_account(account)?;
                // Money movements are the public ledger, so any member may read
                // another member's page; `page` still hides others' messages.
                #[cfg(feature = "cobol-core")]
                crate::cobol::policy(
                    54,
                    &[
                        3,
                        (actor == a.0).into(),
                        s.state.member(actor)?.disabled.into(),
                    ],
                )?;
                #[cfg(not(feature = "cobol-core"))]
                if actor != a.0 && s.state.member(actor)?.disabled {
                    return Err(Error::Forbidden);
                }
                page(s, query, Some(a), Some(actor), output)
            })());
        }
    }
    if method == "POST" {
        if let Some(id) = path
            .strip_prefix("/api/v1/transactions/")
            .and_then(|p| p.strip_suffix("/refund"))
        {
            return Some((|| {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Refund {
                    amount: i64,
                    #[serde(default)]
                    reason: Memo,
                }
                let r: Refund = parse(body)?;
                let receipt = s.execute_keyed(
                    actor,
                    key,
                    Command::Refund {
                        transaction: number(id, "tx-")?,
                        amount: r.amount,
                        memo: r.reason,
                    },
                )?;
                transaction_response(&s.state, receipt.sequence, output)
            })());
        }
    }
    None
}
