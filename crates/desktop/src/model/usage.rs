//! This computer's agent accounts' usage, as the rail rings and the usage popover show it (ACC-09, ACC-10, ACC-15): a
//! ring per account with a reading, the 5-hour and weekly windows, the usage switch, and the newer-MassOrca
//! state for a service that cannot report usage. Free of GPUI.

use massorca_protocol::methods::usage::{
    AccountUsage, Collection, UsageList, UsageState, UsageWindow, WindowKind,
};

use super::agent_sessions::agent_of_kind;
use super::shell::{Agent, UsageAccount, UsageControl, UsageModel, UsageStatus, UsageWindowLine};

/// The usage colour slots the rings cycle through, as the core assigns them (decision 0154).
const USAGE_SLOTS: usize = 6;

/// The rail's usage rings: one per account when usage is on, and one placeholder ring while it is off, while the service
/// is not known yet, or when it cannot report usage. Nothing is shown before the store is loaded.
pub fn rings(
    loaded: bool,
    offered: bool,
    list: Option<&UsageList>,
    now_ms: i64,
) -> Vec<UsageModel> {
    if !loaded {
        return Vec::new();
    }
    if !offered {
        return vec![placeholder(
            UsageStatus::NeedsNewerService,
            UsageControl::None,
        )];
    }
    let Some(list) = list else {
        return vec![placeholder(UsageStatus::Loading, UsageControl::None)];
    };
    match list.collection {
        Collection::Off => vec![placeholder(
            UsageStatus::Off,
            UsageControl::Switch { on: false },
        )],
        Collection::Unknown => vec![placeholder(UsageStatus::Unavailable, UsageControl::None)],
        Collection::On if list.accounts.is_empty() => {
            vec![placeholder(
                UsageStatus::NotReported,
                UsageControl::Switch { on: true },
            )]
        }
        Collection::On => list
            .accounts
            .iter()
            .enumerate()
            .map(|(index, account)| ring(account, index, now_ms))
            .collect(),
    }
}

/// The ring's reading for an account: the 5-hour window's percent when the vendor recorded it, else the weekly one's.
/// A missing percent is unknown and never zero; an error holds no numbers, so no stale percent shows (ACC-15).
pub fn status_of(account: &AccountUsage) -> UsageStatus {
    match account.state {
        UsageState::Pending => UsageStatus::Loading,
        UsageState::Ok => {
            percent_of(account).map_or(UsageStatus::NotReported, UsageStatus::Percent)
        }
        UsageState::NoData => UsageStatus::NotReported,
        UsageState::Error => UsageStatus::Failed { last: None },
        UsageState::Unknown => UsageStatus::Unavailable,
    }
}

/// One window's line in the popover: its name, what it holds, and when it resets.
pub fn window_line(window: &UsageWindow, now_ms: i64) -> UsageWindowLine {
    let title = match window.window {
        WindowKind::Session if window.window_minutes == Some(300) => "5-hour".to_owned(),
        WindowKind::Session => "Session".to_owned(),
        WindowKind::Weekly => "Weekly".to_owned(),
        WindowKind::Unknown => "Limit".to_owned(),
    };
    let value = match (window.used_percent, window.tokens) {
        (Some(percent), _) if percent.is_finite() => {
            format!("{} % used", percent.clamp(0.0, 100.0).round() as u64)
        }
        (_, Some(tokens)) => tokens_text(tokens),
        _ => "Not reported".to_owned(),
    };
    let resets = match window.resets_at_ms {
        Some(at) if at > now_ms => format!("resets in {}", span_text(at - now_ms)),
        _ => String::new(),
    };
    UsageWindowLine {
        title,
        value,
        resets,
        limited: window.limited,
    }
}

/// How long a span of milliseconds reads, in its largest whole unit: "45 min", "2 h", "1 h 30 min", "3 days".
pub fn span_text(ms: i64) -> String {
    let minutes = ms.max(0) / 60_000;
    if minutes < 60 {
        return format!("{minutes} min");
    }
    let hours = minutes / 60;
    if hours < 24 {
        let rest = minutes % 60;
        return if rest == 0 {
            format!("{hours} h")
        } else {
            format!("{hours} h {rest} min")
        };
    }
    match hours / 24 {
        1 => "1 day".to_owned(),
        days => format!("{days} days"),
    }
}

/// How long ago a time was, such as "just now", "3 min ago" or "2 days ago".
pub fn ago_text(ms: i64) -> String {
    let minutes = ms.max(0) / 60_000;
    if minutes < 1 {
        "just now".to_owned()
    } else if minutes < 60 {
        format!("{minutes} min ago")
    } else if minutes < 24 * 60 {
        format!("{} h ago", minutes / 60)
    } else {
        match minutes / (24 * 60) {
            1 => "1 day ago".to_owned(),
            days => format!("{days} days ago"),
        }
    }
}

/// The current time as the epoch milliseconds the usage readings carry.
pub fn now_ms() -> i64 {
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
}

/// The percent a ring draws: the 5-hour window's when the vendor recorded it, else the weekly one's. A missing percent is
/// unknown, never zero.
fn percent_of(account: &AccountUsage) -> Option<u8> {
    let used = |kind: WindowKind| {
        account
            .windows
            .iter()
            .find(|window| window.window == kind)
            .and_then(|window| window.used_percent)
    };
    let percent = used(WindowKind::Session).or_else(|| used(WindowKind::Weekly))?;
    percent
        .is_finite()
        .then(|| percent.clamp(0.0, 100.0).round() as u8)
}

/// One account's ring, with its windows and times, in the order the service lists the accounts.
fn ring(account: &AccountUsage, index: usize, now_ms: i64) -> UsageModel {
    let drawn = account
        .windows
        .iter()
        .find(|window| window.window == WindowKind::Session)
        .or_else(|| {
            account
                .windows
                .iter()
                .find(|window| window.window == WindowKind::Weekly)
        });
    let resets_in = drawn
        .and_then(|window| window.resets_at_ms)
        .filter(|at| *at > now_ms)
        .map(|at| span_text(at - now_ms))
        .unwrap_or_default();
    let mut ring = UsageModel::new(
        agent_of_kind(&account.kind),
        slot_for(index),
        status_of(account),
        resets_in,
    );
    ring.label = account.label.clone();
    ring.account = Some(UsageAccount {
        kind: account.kind.clone(),
        account: account.account.clone(),
    });
    ring.windows = account
        .windows
        .iter()
        .map(|window| window_line(window, now_ms))
        .collect();
    ring.read_text = account
        .fetched_at_ms
        .map(|at| ago_text(now_ms - at))
        .unwrap_or_default();
    ring.error = account
        .error
        .clone()
        .filter(|_| account.state == UsageState::Error);
    ring.control = UsageControl::Switch { on: true };
    ring
}

/// A ring that is not an account: the state of usage on this computer, with the switch when it has one.
fn placeholder(status: UsageStatus, control: UsageControl) -> UsageModel {
    UsageModel {
        control,
        ..UsageModel::new(Agent::Unknown, 1, status, String::new())
    }
}

/// The colour slot of the `index`th ring, cycling through the six the core assigns.
fn slot_for(index: usize) -> u8 {
    u8::try_from(index % USAGE_SLOTS + 1).unwrap_or(1)
}

/// A token count, in millions, thousands or whole tokens.
fn tokens_text(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1} M tokens", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{} k tokens", tokens / 1_000)
    } else {
        format!("{tokens} tokens")
    }
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::usage::{
        AccountUsage, UsageList, UsageState, UsageWindow, WindowKind,
    };

    use super::*;

    const NOW: i64 = 10_000_000;
    const HOUR: i64 = 3_600_000;

    fn account(kind: &str, id: &str, state: UsageState) -> AccountUsage {
        AccountUsage {
            state,
            ..AccountUsage::pending(kind, id, "work")
        }
    }

    fn session(percent: Option<f64>, tokens: Option<u64>) -> UsageWindow {
        UsageWindow {
            window_minutes: Some(300),
            used_percent: percent,
            tokens,
            resets_at_ms: Some(NOW + 2 * HOUR),
            ..UsageWindow::new(WindowKind::Session)
        }
    }

    #[test]
    fn a_missing_percent_is_unknown_and_a_reported_zero_is_zero() {
        let mut claude = account("claude", "a-1", UsageState::Ok);
        claude.windows = vec![session(None, Some(120_000))];
        assert_eq!(status_of(&claude), UsageStatus::NotReported);
        let mut codex = account("codex", "a-2", UsageState::Ok);
        codex.windows = vec![session(Some(0.0), None)];
        assert_eq!(status_of(&codex), UsageStatus::Percent(0));
    }

    #[test]
    fn the_five_hour_window_draws_the_ring_and_the_weekly_one_is_the_fallback() {
        let mut codex = account("codex", "a-2", UsageState::Ok);
        codex.windows = vec![
            UsageWindow {
                used_percent: Some(35.0),
                ..UsageWindow::new(WindowKind::Weekly)
            },
            session(Some(62.4), None),
        ];
        assert_eq!(status_of(&codex), UsageStatus::Percent(62));
        codex.windows.remove(1);
        assert_eq!(status_of(&codex), UsageStatus::Percent(35));
        codex.windows.clear();
        assert_eq!(status_of(&codex), UsageStatus::NotReported);
    }

    #[test]
    fn an_error_holds_no_stale_percent_and_an_unknown_state_is_unavailable() {
        let mut codex = account("codex", "a-2", UsageState::Error);
        codex.error = Some("the log folder is locked".into());
        codex.windows = vec![session(Some(90.0), None)];
        assert_eq!(status_of(&codex), UsageStatus::Failed { last: None });
        let unknown = account("codex", "a-3", UsageState::Unknown);
        assert_eq!(status_of(&unknown), UsageStatus::Unavailable);
    }

    #[test]
    fn off_shows_one_ring_with_the_switch_and_an_older_service_says_it_needs_a_newer_build() {
        let off = rings(true, true, Some(&UsageList::off()), NOW);
        assert_eq!(off.len(), 1);
        assert_eq!(off[0].status, UsageStatus::Off);
        assert_eq!(off[0].control, UsageControl::Switch { on: false });
        assert_eq!(off[0].account, None);
        let older = rings(true, false, None, NOW);
        assert_eq!(older.len(), 1);
        assert_eq!(older[0].status, UsageStatus::NeedsNewerService);
        assert_eq!(older[0].control, UsageControl::None);
    }

    #[test]
    fn nothing_shows_before_the_store_is_loaded_and_a_pending_read_is_loading() {
        assert!(rings(false, true, None, NOW).is_empty());
        let unread = rings(true, true, None, NOW);
        assert_eq!(unread[0].status, UsageStatus::Loading);
        let pending = UsageList::on(vec![account("claude", "a-1", UsageState::Pending)]);
        assert_eq!(
            rings(true, true, Some(&pending), NOW)[0].status,
            UsageStatus::Loading
        );
    }

    #[test]
    fn each_account_gets_a_ring_in_order_with_its_windows_and_times() {
        let mut claude = account("claude", "a-1", UsageState::Ok);
        claude.label = "work".into();
        claude.fetched_at_ms = Some(NOW - 180_000);
        claude.windows = vec![
            session(None, Some(1_200_000)),
            UsageWindow {
                tokens: Some(9_000_000),
                resets_at_ms: Some(NOW + 3 * 24 * HOUR),
                ..UsageWindow::new(WindowKind::Weekly)
            },
        ];
        let codex = account("codex", "a-2", UsageState::NoData);
        let lines = rings(true, true, Some(&UsageList::on(vec![claude, codex])), NOW);
        assert_eq!(lines.len(), 2);
        assert_eq!((lines[0].agent, lines[0].slot), (Agent::Claude, 1));
        assert_eq!(lines[0].label, "work");
        assert_eq!(
            lines[0].account,
            Some(UsageAccount {
                kind: "claude".into(),
                account: "a-1".into()
            })
        );
        assert_eq!(lines[0].status, UsageStatus::NotReported);
        assert_eq!(lines[0].read_text, "3 min ago");
        assert_eq!(lines[0].windows[0].title, "5-hour");
        assert_eq!(lines[0].windows[0].value, "1.2 M tokens");
        assert_eq!(lines[0].windows[0].resets, "resets in 2 h");
        assert_eq!(lines[0].windows[1].title, "Weekly");
        assert_eq!(lines[0].windows[1].value, "9.0 M tokens");
        assert_eq!(lines[0].windows[1].resets, "resets in 3 days");
        assert_eq!((lines[1].agent, lines[1].slot), (Agent::Codex, 2));
        assert_eq!(lines[1].status, UsageStatus::NotReported);
        assert_eq!(lines[1].control, UsageControl::Switch { on: true });
    }

    #[test]
    fn a_window_names_its_percent_and_its_limit_and_an_unreported_one_says_so() {
        let percent = window_line(&session(Some(62.4), None), NOW);
        assert_eq!(percent.value, "62 % used");
        let limited = window_line(
            &UsageWindow {
                limited: true,
                ..session(Some(100.0), None)
            },
            NOW,
        );
        assert!(limited.limited);
        assert_eq!(window_line(&session(None, None), NOW).value, "Not reported");
    }

    #[test]
    fn spans_and_ages_read_in_the_largest_whole_unit() {
        assert_eq!(span_text(45 * 60_000), "45 min");
        assert_eq!(span_text(2 * HOUR), "2 h");
        assert_eq!(span_text(90 * 60_000), "1 h 30 min");
        assert_eq!(span_text(24 * HOUR), "1 day");
        assert_eq!(span_text(3 * 24 * HOUR), "3 days");
        assert_eq!(ago_text(20_000), "just now");
        assert_eq!(ago_text(3 * 60_000), "3 min ago");
        assert_eq!(ago_text(5 * HOUR), "5 h ago");
    }
}
