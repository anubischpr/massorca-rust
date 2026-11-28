//! `massorca usage`: each agent account's usage and limits, read by the host that runs the agents (decision 0154). With
//! `--host` it reads that paired host; `--refresh` reads the accounts again first.

use clap::Args;
use massorca_protocol::methods::usage::{
    AccountUsage, Collection, SETTING_ENABLED, UsageList, UsageParams, UsageState, UsageWindow,
    WindowKind,
};

use crate::connect::connect;
use crate::error::CliError;
use crate::output::Output;
use crate::target::Target;
use crate::time_text::utc_minute;
use crate::workspace_text;

/// Options of `massorca usage`.
#[derive(Debug, Args)]
pub struct UsageArgs {
    /// Only accounts of this agent kind, such as claude or codex.
    #[arg(long, value_name = "KIND")]
    pub kind: Option<String>,
    /// Only this account, by id.
    #[arg(long, value_name = "ID")]
    pub account: Option<String>,
    /// Reads the accounts' logs again instead of answering from the service's cache.
    #[arg(long)]
    pub refresh: bool,
}

/// Runs `usage` against the service `target` names.
pub async fn execute(
    target: &Target,
    autostart: bool,
    args: UsageArgs,
    output: Output,
) -> Result<(), CliError> {
    let client = connect(target, autostart).await?;
    let params = UsageParams {
        kind: args.kind,
        account: args.account,
    };
    let list = if args.refresh {
        client.usage_refresh(&params).await?
    } else {
        client.usage_list(&params).await?
    };
    output.success(&list, &lines(&list));
    Ok(())
}

/// The text answer: a line for collection off, otherwise one row per account.
pub fn lines(list: &UsageList) -> Vec<String> {
    match list.collection {
        Collection::On => {}
        Collection::Off => {
            return vec![format!(
                "usage collection is off on this host; turn it on with: massorca settings set {SETTING_ENABLED} true"
            )];
        }
        Collection::Unknown => return vec!["usage collection: unknown state".to_owned()],
    }
    if list.accounts.is_empty() {
        return vec!["no agent accounts with usage on this host".to_owned()];
    }
    let rows = list.accounts.iter().map(row).collect();
    workspace_text::table(
        &["KIND", "ACCOUNT", "STATE", "SESSION", "WEEKLY", "READ"],
        rows,
    )
}

fn row(usage: &AccountUsage) -> Vec<String> {
    let state = match usage.state {
        UsageState::Pending => "pending".to_owned(),
        UsageState::Ok => "ok".to_owned(),
        UsageState::NoData => "no data".to_owned(),
        UsageState::Error => format!("error: {}", usage.error.as_deref().unwrap_or("unknown")),
        UsageState::Unknown => "unknown".to_owned(),
    };
    let cell = |kind: WindowKind| {
        usage
            .windows
            .iter()
            .find(|window| window.window == kind)
            .map_or_else(|| "-".to_owned(), window_text)
    };
    vec![
        usage.kind.clone(),
        usage.label.clone(),
        state,
        cell(WindowKind::Session),
        cell(WindowKind::Weekly),
        usage
            .fetched_at_ms
            .map_or_else(|| "-".to_owned(), utc_minute),
    ]
}

/// `42%`, `150 tokens` or `limit reached`, then the reset when known.
fn window_text(window: &UsageWindow) -> String {
    let amount = if window.limited {
        "limit reached".to_owned()
    } else if let Some(percent) = window.used_percent {
        format!("{percent:.0}%")
    } else if let Some(tokens) = window.tokens {
        format!("{tokens} tokens")
    } else {
        "unknown".to_owned()
    };
    match window.resets_at_ms {
        Some(reset) => format!("{amount}, resets {}", utc_minute(reset)),
        None => amount,
    }
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::usage::{
        AccountUsage, UsageList, UsageState, UsageWindow, WindowKind,
    };

    use super::lines;

    #[test]
    fn off_says_how_to_turn_it_on() {
        let text = lines(&UsageList::off());
        assert_eq!(text.len(), 1);
        assert!(text[0].contains("massorca settings set usage.enabled true"));
    }

    #[test]
    fn rows_show_percent_tokens_and_reset() {
        let mut usage = AccountUsage::pending("codex", "a-1", "work");
        usage.state = UsageState::Ok;
        usage.windows = vec![
            UsageWindow {
                used_percent: Some(42.4),
                resets_at_ms: Some(0),
                ..UsageWindow::new(WindowKind::Session)
            },
            UsageWindow {
                tokens: Some(150),
                ..UsageWindow::new(WindowKind::Weekly)
            },
        ];
        let list = UsageList::on(vec![usage]);
        let text = lines(&list).join("\n");
        assert!(text.contains("42%, resets 1970-01-01 00:00 UTC"), "{text}");
        assert!(text.contains("150 tokens"), "{text}");
        assert!(text.contains("work"), "{text}");
    }
}
