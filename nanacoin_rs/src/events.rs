//! A bounded scheduling primitive; this module does not run or persist events.
//! The future service runner derives one candidate per agreement and commits it
//! before updating its durable next_due_at. No timer queue or per-loan task.
use crate::domain::{Error, MAX_SEQUENCE};
use crate::offers::MIN_CLOCK;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DueOccurrence {
    /// Together with the agreement ID, identifies the occurrence across reboot.
    pub due_at: u64,
    /// Advance to this value only in the same durable event as settlement.
    pub next_due_at: u64,
}

/// Returns at most one occurrence, anchored to the original schedule. Missed
/// periods stay due, allowing a service-wide work budget to bound catch-up.
/// `now` must come from Service::now(), which also checks the journal watermark.
pub fn due_occurrence(
    next_due_at: u64,
    interval_seconds: u64,
    now: u64,
) -> Result<Option<DueOccurrence>, Error> {
    if !(MIN_CLOCK..=MAX_SEQUENCE).contains(&now) {
        return Err(Error::Unavailable);
    }
    if !(MIN_CLOCK..=MAX_SEQUENCE).contains(&next_due_at)
        || !(1..=MAX_SEQUENCE).contains(&interval_seconds)
    {
        return Err(Error::InvalidInput);
    }
    if now < next_due_at {
        return Ok(None);
    }
    let following = next_due_at
        .checked_add(interval_seconds)
        .filter(|v| *v <= MAX_SEQUENCE)
        .ok_or(Error::Overflow)?;
    Ok(Some(DueOccurrence {
        due_at: next_due_at,
        next_due_at: following,
    }))
}
