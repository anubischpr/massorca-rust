//! What a due-run claim writes. The compare-and-swap itself belongs to the caller's store.

use crate::occurrence::CompiledSchedule;

/// Skipped occurrences counted at most; a longer outage reports this number.
pub const MAX_COUNTED_SKIPS: u32 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimDecision {
    NotDue,
    Due {
        /// The occurrence this run stands for: the latest one at or before `now`.
        scheduled_for: i64,
        /// What to write to `next_run_at`: the first occurrence after `now`, `None` when finished.
        next_run_at: Option<i64>,
        /// Earlier missed occurrences dropped in favour of `scheduled_for`, capped at [`MAX_COUNTED_SKIPS`].
        skipped: u32,
    },
}

/// Decides a claim from the stored `next_run_at` (`None` is paused or finished) at `now`.
///
/// When the schedule has no occurrence between the stored time and `now` (it was edited without recomputing
/// `next_run_at`), the stored time itself runs once.
pub fn decide_claim(
    schedule: &CompiledSchedule,
    stored_next_run_at: Option<i64>,
    now: i64,
) -> ClaimDecision {
    let Some(stored) = stored_next_run_at else {
        return ClaimDecision::NotDue;
    };
    if stored > now {
        return ClaimDecision::NotDue;
    }
    let scheduled_for = schedule
        .latest_at_or_before(now)
        .filter(|latest| *latest >= stored)
        .unwrap_or(stored);
    ClaimDecision::Due {
        scheduled_for,
        next_run_at: schedule.next_after(now),
        skipped: count_skipped(schedule, stored, scheduled_for),
    }
}

/// The `next_run_at` to write when a schedule is created or resumed: nothing missed while paused is made up.
pub fn next_run_from(schedule: &CompiledSchedule, now: i64) -> Option<i64> {
    schedule.next_after(now)
}

/// Occurrences in `[from, until)`.
fn count_skipped(schedule: &CompiledSchedule, from: i64, until: i64) -> u32 {
    let mut count = 0;
    let mut cursor = schedule.next_after(from.saturating_sub(1));
    while let Some(at) = cursor
        && at < until
        && count < MAX_COUNTED_SKIPS
    {
        count += 1;
        cursor = schedule.next_after(at);
    }
    count
}
