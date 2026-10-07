//! Optional household screen delivery. Financial commits never depend on it.
use crate::{
    domain::{Command, Error, Event, MemberId},
    journal::{Journal, Service},
};
use core::fmt::Write as _;
use heapless::String;
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    sync::mpsc,
    time::{Duration, Instant},
};

pub const LIFETIME: u64 = 86_400;
pub const CAPACITY: usize = 8;
pub const STORAGE_BYTES: usize = 4096;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Notification {
    pub id: String<64>,
    pub recipient: String<64>,
    pub text: String<256>,
    pub expires_at: u64,
    pub read: bool,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Outbox {
    pub pending: heapless::Vec<Notification, CAPACITY>,
}
impl Outbox {
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < 4 || bytes.len() > STORAGE_BYTES {
            return Err(Error::Storage);
        }
        let (crc, data) = bytes.split_at(4);
        if u32::from_le_bytes(crc.try_into().unwrap()) != crc32fast::hash(data) {
            return Err(Error::Storage);
        }
        let (outbox, remaining) = postcard::take_from_bytes(data).map_err(|_| Error::Storage)?;
        if !remaining.is_empty() {
            return Err(Error::Storage);
        }
        Ok(outbox)
    }
    pub fn encode<'a>(&self, bytes: &'a mut [u8; STORAGE_BYTES]) -> Result<&'a [u8], Error> {
        let data = postcard::to_slice(self, &mut bytes[4..]).map_err(|_| Error::Capacity)?;
        let len = data.len();
        let crc = crc32fast::hash(data);
        bytes[..4].copy_from_slice(&crc.to_le_bytes());
        Ok(&bytes[..4 + len])
    }
}

impl<J: Journal> Service<J> {
    fn screen_id(&self, sequence: u64, kind: &str) -> String<64> {
        let mut id = String::new();
        write!(
            id,
            "{}-{}-{sequence}-{kind}",
            crate::board::ID,
            self.state.archive.incarnation
        )
        .unwrap();
        id
    }
    pub fn queue_screen(&mut self, notification: Notification) -> Result<(), Error> {
        if self.now() < 1_700_000_000 || notification.expires_at <= self.now() {
            return Err(Error::InvalidInput);
        }
        if self
            .screen_outbox
            .pending
            .iter()
            .any(|n| n.id == notification.id && n.read == notification.read)
        {
            return Ok(());
        }
        let mut next = (*self.screen_outbox).clone();
        let now = self.now();
        next.pending.retain(|n| n.expires_at > now);
        // A read supersedes a copy that has not yet reached the display.
        if notification.read {
            next.pending.retain(|n| n.id != notification.id);
        }
        next.pending
            .push(notification)
            .map_err(|_| Error::Capacity)?;
        self.journal.set_screen_outbox(&next)?;
        *self.screen_outbox = next;
        Ok(())
    }
    pub fn screen_message(
        &mut self,
        actor: MemberId,
        sequence: u64,
        read: bool,
    ) -> Result<(), Error> {
        let tx = self
            .state
            .history
            .iter()
            .find(|tx| tx.id == sequence)
            .ok_or(Error::NotFound)?;
        #[cfg(feature = "cobol-core")]
        crate::cobol::policy(
            62,
            &[
                11,
                tx.amount,
                tx.usd.into(),
                tx.memo.trim().is_empty().into(),
                tx.reverses.is_some().into(),
                tx.loan.is_some().into(),
                tx.lotto.is_some().into(),
                read.into(),
                actor.0.into(),
                tx.from.0.into(),
                tx.to.0.into(),
            ],
        )?;
        #[cfg(not(feature = "cobol-core"))]
        if tx.amount != 0
            || tx.usd
            || tx.memo.trim().is_empty()
            || tx.reverses.is_some()
            || tx.loan.is_some()
            || tx.lotto.is_some()
        {
            return Err(Error::InvalidInput);
        }
        #[cfg(not(feature = "cobol-core"))]
        if actor != if read { tx.to } else { tx.from } {
            return Err(Error::Forbidden);
        }
        let mut text = String::new();
        write!(text, "{}: {}", self.state.member(tx.from)?.name, tx.memo)
            .map_err(|_| Error::Capacity)?;
        let notification = Notification {
            id: self.screen_id(sequence, "mail"),
            recipient: self
                .state
                .member(tx.to)?
                .name
                .as_str()
                .try_into()
                .map_err(|_| Error::Capacity)?,
            text,
            expires_at: if read {
                self.now() + LIFETIME
            } else {
                tx.created_at + LIFETIME
            },
            read,
        };
        self.queue_screen(notification)
    }
    pub(crate) fn screen_event(&mut self, event: &Event) {
        if !self.journal.supports_screen() {
            return;
        }
        let mut text = String::new();
        let kind = match &event.command {
            Command::CreateLotto { terms } => {
                let _ = write!(text, "New Lotto created: {}", terms.title);
                "lotto"
            }
            Command::RunLotto { lotto, .. } => {
                let Ok(lotto) = self.state.lotto(*lotto) else {
                    return;
                };
                if lotto.step != crate::lotto::DONE {
                    return;
                }
                let _ = write!(text, "Lotto completed: {}", lotto.terms.title);
                "draw"
            }
            Command::List { title, .. } | Command::ClassifiedList { title, .. } => {
                let _ = write!(text, "New listing: {title}");
                "listing"
            }
            Command::UpdateListing { listing, .. } => {
                let Ok(l) = self.state.listing(*listing) else {
                    return;
                };
                let _ = write!(text, "Listing updated: {}", l.title);
                "listing-update"
            }
            Command::Cancel { listing } => {
                let Ok(l) = self.state.listing(*listing) else {
                    return;
                };
                let _ = write!(text, "Listing withdrawn: {}", l.title);
                "listing-close"
            }
            Command::MakeOffer { listing, .. } => {
                let Ok(l) = self.state.listing(*listing) else {
                    return;
                };
                let _ = write!(
                    text,
                    "Response to {}: awaiting {}'s acceptance",
                    l.title,
                    self.state.member(l.owner).unwrap().name
                );
                "offer"
            }
            Command::AcceptOffer { offer } => {
                let Ok(o) = self.state.offer(*offer) else {
                    return;
                };
                let _ = write!(text, "Offer accepted: {}", o.listing_title);
                "accepted"
            }
            Command::UnacceptOffer { .. } | Command::Reverse { .. } => {
                text.push_str("Payment reversed; check NanaCoin for details")
                    .unwrap();
                "reversal"
            }
            Command::Issue { .. } => {
                text.push_str("Coins issued; money supply increased")
                    .unwrap();
                "issue"
            }
            Command::Retire { .. } => {
                text.push_str("Coins retired; money supply decreased")
                    .unwrap();
                "retire"
            }
            Command::Refund { .. } => {
                text.push_str("Refund recorded").unwrap();
                "refund"
            }
            Command::DeclineOffer { .. } => {
                text.push_str("Offer declined").unwrap();
                "declined"
            }
            Command::WithdrawOffer { .. } => {
                text.push_str("Offer withdrawn").unwrap();
                "withdrawn"
            }
            Command::Buy { listing } => {
                let Ok(l) = self.state.listing(*listing) else {
                    return;
                };
                let _ = write!(text, "Purchased: {}", l.title);
                "purchase"
            }
            Command::OfferLoan { .. } => {
                text.push_str("New loan offered; borrower can review and accept")
                    .unwrap();
                "loan-offer"
            }
            Command::RequestLoan { .. } => {
                text.push_str("New loan application in Loans Wanted")
                    .unwrap();
                "loan-request"
            }
            Command::RespondLoan { .. } => {
                text.push_str("Loan application received an offer; awaiting borrower acceptance")
                    .unwrap();
                "loan-response"
            }
            Command::AcceptLoan { .. } => {
                text.push_str("Loan accepted").unwrap();
                "loan-accepted"
            }
            Command::CloseLoan { .. } => {
                text.push_str("Loan offer or application closed").unwrap();
                "loan-closed"
            }
            Command::RepayLoan { .. } => {
                text.push_str("Loan repayment recorded").unwrap();
                "loan-payment"
            }
            Command::RunLoan { loan, .. } => {
                let Ok(l) = self.state.loan(*loan) else {
                    return;
                };
                if l.status == crate::loans::LoanStatus::Paid {
                    text.push_str("Loan fully repaid").unwrap();
                    "loan-paid"
                } else {
                    return;
                }
            }
            Command::SetFulfillment { action, .. } => {
                let _ = write!(text, "Work or delivery updated: {action:?}");
                "fulfillment"
            }
            Command::PostQuote { .. } => {
                text.push_str("New exchange quote available").unwrap();
                "quote"
            }
            Command::TakeQuote { .. } => {
                text.push_str("Currency exchange completed").unwrap();
                "exchange"
            }
            Command::CancelQuote { .. } => {
                text.push_str("Exchange quote withdrawn").unwrap();
                "quote-close"
            }
            Command::ReformCurrency { .. } => {
                text.push_str("Currency reformed; reload NanaCoin").unwrap();
                "reform"
            }
            Command::Commerce { action } => {
                use crate::commerce::Action;
                let message = match action {
                    Action::CreateRequest { .. } => "New gift request",
                    Action::CloseRequest { .. } => "Gift request closed",
                    Action::Contribute { .. } => "Gift contribution received",
                    Action::MintArt { .. } => "New art created",
                    Action::ListArt { price: Some(_), .. } => "Art offered for sale",
                    Action::ListArt { price: None, .. } => "Art withdrawn from sale",
                    Action::BuyArt { .. } => "Art purchased",
                    Action::GiftArt { .. } => "Art gifted",
                    Action::EquipArt { .. } => return,
                };
                text.push_str(message).unwrap();
                "commerce"
            }
            _ => return,
        };
        let notification = Notification {
            id: self.screen_id(event.sequence, kind),
            recipient: String::new(),
            text,
            expires_at: event.timestamp + LIFETIME,
            read: false,
        };
        if let Err(error) = self.queue_screen(notification) {
            eprintln!("Kitchen screen event not queued: {error:?}");
        }
    }
    pub fn screen_pending(&self) -> &[Notification] {
        &self.screen_outbox.pending
    }
}

pub struct Worker {
    send: mpsc::SyncSender<Notification>,
    results: mpsc::Receiver<(Notification, bool)>,
    busy: bool,
    next_attempt: Instant,
    next_stats: Instant,
}
impl Worker {
    pub fn spawn() -> std::io::Result<Self> {
        Self::spawn_with_resolver(resolve)
    }
    pub fn spawn_with_resolver(
        resolver: impl Fn(&str, u16) -> std::io::Result<SocketAddr> + Send + 'static,
    ) -> std::io::Result<Self> {
        let url = std::env::var("NANACOIN_MINICLOUD_URL").unwrap_or_else(|_| {
            option_env!("NANACOIN_MINICLOUD_URL")
                .unwrap_or("http://minicloud.local")
                .into()
        });
        let (send, receive) = mpsc::sync_channel::<Notification>(1);
        let (results_send, results) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("nanacoin-screen".into())
            .stack_size(16 * 1024)
            .spawn(move || {
                while let Ok(notification) = receive.recv() {
                    let ok = deliver_with_resolver(&url, &notification, &resolver).is_ok();
                    if !ok {
                        eprintln!("Kitchen screen unavailable; retrying later");
                    }
                    if results_send.send((notification, ok)).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            send,
            results,
            busy: false,
            next_attempt: Instant::now(),
            next_stats: Instant::now(),
        })
    }
    /// Called by the journal owner. Only the network worker blocks on sockets.
    pub fn pump<J: Journal>(&mut self, service: &mut Service<J>) {
        self.pump_at(service, Instant::now());
    }
    fn pump_at<J: Journal>(&mut self, service: &mut Service<J>, at: Instant) {
        if let Ok((notification, ok)) = self.results.try_recv() {
            self.busy = false;
            self.next_attempt = at + Duration::from_secs(if ok { 0 } else { 30 });
            if ok {
                let mut next = (*service.screen_outbox).clone();
                next.pending.retain(|n| n != &notification);
                if service.journal.set_screen_outbox(&next).is_ok() {
                    *service.screen_outbox = next;
                } else {
                    self.next_attempt = at + Duration::from_secs(30);
                }
            }
        }
        if at >= self.next_stats
            && service.now() >= 1_700_000_000
            && service.screen_outbox.pending.len() <= CAPACITY - 2
        {
            for (index, text) in stats(&service.state, service.now()).into_iter().enumerate() {
                let mut id = String::new();
                let _ = write!(
                    id,
                    "stats-{index}-{}-{}-{:x}",
                    crate::board::ID,
                    service.now() / 300,
                    crc32fast::hash(text.as_bytes())
                );
                let _ = service.queue_screen(Notification {
                    id,
                    recipient: String::try_from("Household economy").unwrap(),
                    text,
                    expires_at: service.now() + LIFETIME,
                    read: false,
                });
            }
            self.next_stats = at + Duration::from_secs(300);
        }
        if self.busy || at < self.next_attempt || service.now() < 1_700_000_000 {
            return;
        }
        let now = service.now();
        if service
            .screen_outbox
            .pending
            .iter()
            .any(|n| n.expires_at <= now)
        {
            let mut next = (*service.screen_outbox).clone();
            next.pending.retain(|n| n.expires_at > now);
            if service.journal.set_screen_outbox(&next).is_err() {
                return;
            }
            *service.screen_outbox = next;
        }
        if let Some(n) = service.screen_outbox.pending.first() {
            if self.send.try_send(n.clone()).is_ok() {
                self.busy = true;
            }
        }
    }
}

#[cfg(test)]
fn deliver(url: &str, n: &Notification) -> Result<(), Box<dyn std::error::Error>> {
    deliver_with_resolver(url, n, resolve)
}
fn deliver_with_resolver(
    url: &str,
    n: &Notification,
    resolver: impl Fn(&str, u16) -> std::io::Result<SocketAddr>,
) -> Result<(), Box<dyn std::error::Error>> {
    let authority = url
        .strip_prefix("http://")
        .ok_or("screen URL must use http")?
        .trim_end_matches('/');
    if authority.is_empty()
        || authority.len() > 192
        || authority
            .bytes()
            .any(|b| b <= b' ' || b == b'/' || b == b'@' || b == b'?' || b == b'#')
    {
        return Err("invalid screen URL".into());
    }
    let (host, port) = authority
        .rsplit_once(':')
        .map_or((authority, 80), |(h, p)| (h, p.parse().unwrap_or(0)));
    if port == 0 {
        return Err("invalid screen port".into());
    }
    let address = resolver(host, port)?;
    let timeout = Duration::from_secs(2);
    let mut stream = TcpStream::connect_timeout(&address, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    #[derive(Serialize)]
    struct Payload<'a> {
        event_id: String<80>,
        source: &'static str,
        id: &'a str,
        recipient: &'a str,
        text: &'a str,
        size: &'static str,
        expires_at: u64,
    }
    let mut event_id = String::new();
    write!(
        event_id,
        "{}-{}",
        n.id,
        if n.read { "read" } else { "notify" }
    )?;
    let payload = Payload {
        event_id,
        source: "nanacoin",
        id: &n.id,
        recipient: &n.recipient,
        text: &n.text,
        size: "medium",
        expires_at: n.expires_at,
    };
    let mut body = [0; 2048];
    let len =
        serde_json_core::to_slice(&payload, &mut body).map_err(|_| "screen payload too large")?;
    write!(stream, "POST /api/screen/{} HTTP/1.1\r\nHost: {authority}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n", if n.read {"read"} else {"notify"})?;
    stream.write_all(&body[..len])?;
    read_status(&mut stream)
}

fn read_status(stream: &mut impl Read) -> Result<(), Box<dyn std::error::Error>> {
    let mut response = [0; 256];
    let mut used = 0;
    while used < response.len() {
        let count = stream.read(&mut response[used..])?;
        if count == 0 {
            break;
        }
        used += count;
        if response[..used].contains(&b'\n') {
            break;
        }
    }
    let end = response[..used]
        .windows(2)
        .position(|b| b == b"\r\n")
        .ok_or("incomplete screen status")?;
    let line = &response[..end];
    if line.len() < 13
        || !matches!(&line[..9], b"HTTP/1.0 " | b"HTTP/1.1 ")
        || !matches!(&line[9..12], b"200" | b"202")
        || line[12] != b' '
        || line[13..]
            .iter()
            .any(|b| *b < b' ' && *b != b'\t' || *b == 127)
    {
        return Err("screen rejected message".into());
    }
    Ok(())
}
fn resolve(host: &str, port: u16) -> std::io::Result<SocketAddr> {
    (host, port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "screen host not found"))
}

/// Retained-window household measures. Missing observations remain missing.
pub fn stats(s: &crate::domain::State, now: u64) -> [String<256>; 2] {
    #[cfg(feature = "cobol-core")]
    {
        crate::cobol::household_stats(s, now)
    }
    #[cfg(not(feature = "cobol-core"))]
    {
        use crate::domain::{EconomicKind, Role};
        let start = now.saturating_sub(365 * 86_400);
        let usable = |t: &&crate::domain::Transaction| {
            !t.usd
                && t.created_at >= start
                && t.reverses.is_none()
                && s.ledger.reversed_by(t.id).is_none()
        };
        let eligible: std::vec::Vec<_> = s
            .members
            .iter()
            .filter(|m| !m.disabled && m.role != Role::Nana)
            .collect();
        let employed = eligible
            .iter()
            .filter(|m| {
                s.history
                    .iter()
                    .filter(usable)
                    .any(|t| t.to == m.id && t.amount > 0 && t.economic.kind == EconomicKind::Labor)
            })
            .count();
        let mut inflation = 0.0;
        let mut count = 0;
        for thing in &s.things {
            let mut sales = s
                .history
                .iter()
                .filter(usable)
                .filter(|t| {
                    t.economic.kind == EconomicKind::Good
                        && t.economic.thing == thing.id
                        && t.economic.quantity_milli > 0
                        && t.amount > 0
                })
                .rev();
            if let (Some(latest), Some(previous)) = (sales.next(), sales.next()) {
                if let (Ok(latest_amount), Ok(previous_amount)) = (
                    s.current_amount(latest, latest.amount),
                    s.current_amount(previous, previous.amount),
                ) {
                    if previous_amount > 0 {
                        let a = latest_amount as f64 / latest.economic.quantity_milli as f64;
                        let b = previous_amount as f64 / previous.economic.quantity_milli as f64;
                        inflation += (a / b - 1.0) * 100.0;
                        count += 1;
                    }
                }
            }
        }
        let mut first = String::new();
        let _ = write!(
            first,
            "Retained year
Inflation: "
        );
        if count > 0 {
            let _ = write!(first, "{:.1}%", inflation / count as f64);
        } else {
            let _ = first.push_str("No data");
        }
        if eligible.is_empty() {
            let _ = first.push_str(
                "
Employment: No data",
            );
        } else {
            let _ = write!(
                first,
                "
Employment: {:.0}% ({employed}/{})",
                employed as f64 * 100.0 / eligible.len() as f64,
                eligible.len()
            );
        }
        let mut principal = 0i128;
        let mut weighted = 0.0;
        for l in &s.loans {
            if l.status == crate::loans::LoanStatus::Active {
                principal += l.principal as i128;
                weighted += l.principal as f64 * l.terms.rate_bps as f64 / 100.0 * 365.0
                    / l.terms.rate_days as f64;
            }
        }
        let mut second = String::new();
        let _ = second.push_str("Interest: ");
        if principal > 0 {
            let _ = write!(second, "{:.2}%/yr", weighted / principal as f64);
        } else {
            let _ = second.push_str("No active loans");
        }
        let _ = second.push_str(
            "
Exchange: ",
        );
        let mut exchange = s
            .quotes
            .iter()
            .filter(|q| {
                q.status == crate::forex::QuoteStatus::Filled
                    && ![q.coin_tx, q.cash_tx]
                        .into_iter()
                        .flatten()
                        .any(|id| s.ledger.reversed_by(id).is_some())
            })
            .max_by_key(|q| q.updated_at)
            .map(|q| (q.updated_at, q.cents_per_coin as f64));
        // Completed quote rows are recyclable; paired retained ledger legs also
        // provide a rate, using the current currency epoch's NC unit.
        for coin in s.history.iter().rev().filter(|t| {
            !t.usd
                && t.quote.is_some()
                && t.reverses.is_none()
                && t.amount > 0
                && s.ledger.reversed_by(t.id).is_none()
        }) {
            if exchange.is_some_and(|(at, _)| at > coin.created_at) {
                break;
            }
            if let Some(cash) = s.history.iter().find(|t| {
                t.usd
                    && t.quote == coin.quote
                    && t.reverses.is_none()
                    && t.amount > 0
                    && s.ledger.reversed_by(t.id).is_none()
            }) {
                if let Ok(amount) = s.current_amount(coin, coin.amount) {
                    if amount > 0 {
                        exchange = Some((
                            coin.created_at,
                            cash.amount as f64 * 10u64.pow(s.decimals as u32) as f64
                                / amount as f64,
                        ));
                        break;
                    }
                }
            }
        }
        if let Some((_, cents)) = exchange {
            let _ = write!(second, "${:.2}/NC", cents / 100.0);
        } else {
            let _ = second.push_str("No trades");
        }
        [first, second]
    }
}

#[cfg(test)]
#[path = "tests/screen.rs"]
mod hostile_tests;
