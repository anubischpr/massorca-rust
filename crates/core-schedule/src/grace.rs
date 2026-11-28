//! The missed-run rule (decision 0125): a claim runs its occurrence only when it is no later than the grace window,
//! with two checks of slack so a check that lands just after the window does not miss an occurrence on time.

/// Checks of slack past the grace window, as pv96 allows.
pub const GRACE_SLACK_CHECKS: i64 = 2;

/// Whether the occurrence at `scheduled_for` may still run at `now`, given `grace_minutes` and the timer's
/// `check_every_ms`. An occurrence later than that is recorded as missed and never replayed.
pub fn runs_within_grace(
    scheduled_for: i64,
    now: i64,
    grace_minutes: i64,
    check_every_ms: i64,
) -> bool {
    let allowed = grace_minutes
        .saturating_mul(60_000)
        .saturating_add(check_every_ms.saturating_mul(GRACE_SLACK_CHECKS));
    now.saturating_sub(scheduled_for) <= allowed
}

#[cfg(test)]
mod tests {
    use super::runs_within_grace;

    const MINUTE: i64 = 60_000;
    const CHECK: i64 = MINUTE;
    const DUE: i64 = 1_795_564_800_000;

    #[test]
    fn an_occurrence_claimed_on_time_runs() {
        assert!(runs_within_grace(DUE, DUE, 0, CHECK));
    }

    #[test]
    fn a_late_occurrence_inside_the_grace_runs() {
        assert!(runs_within_grace(DUE, DUE + 700 * MINUTE, 720, CHECK));
    }

    #[test]
    fn an_occurrence_past_the_grace_and_its_slack_is_missed() {
        assert!(!runs_within_grace(DUE, DUE + 722 * MINUTE + 1, 720, CHECK));
    }

    #[test]
    fn a_zero_grace_still_runs_within_two_checks() {
        assert!(runs_within_grace(DUE, DUE + 2 * CHECK, 0, CHECK));
        assert!(!runs_within_grace(DUE, DUE + 2 * CHECK + 1, 0, CHECK));
    }
}
