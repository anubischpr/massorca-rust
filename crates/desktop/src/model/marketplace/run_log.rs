//! Which version a rollback of an installed package restores, read from its run log (decision 0100). The lockfile keeps
//! the replaced version, but the protocol does not list it, so the run log is the record the Marketplace reads.

use massorca_protocol::methods::extensions::{
    ExtensionRun, ExtensionRunAction, ExtensionRunOutcome,
};

/// The version a rollback restores: the version the newest run that worked replaced, when that run was an update or a
/// rollback. An install or a removal since then leaves nothing kept, and a run that failed changed nothing.
#[must_use]
pub fn kept_version(runs: &[ExtensionRun]) -> Option<String> {
    let newest = runs.iter().rev().find(|run| {
        run.outcome == ExtensionRunOutcome::Ok && run.action != ExtensionRunAction::Unknown
    })?;
    match newest.action {
        ExtensionRunAction::Update | ExtensionRunAction::Rollback => newest.from_version.clone(),
        ExtensionRunAction::Install | ExtensionRunAction::Remove | ExtensionRunAction::Unknown => {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(
        action: ExtensionRunAction,
        outcome: ExtensionRunOutcome,
        from: Option<&str>,
        to: Option<&str>,
    ) -> ExtensionRun {
        ExtensionRun {
            at_ms: 1_700_000_000_000,
            action,
            from_version: from.map(str::to_owned),
            to_version: to.map(str::to_owned),
            outcome,
            output: Vec::new(),
        }
    }

    fn installed() -> ExtensionRun {
        run(
            ExtensionRunAction::Install,
            ExtensionRunOutcome::Ok,
            None,
            Some("1.0.0"),
        )
    }

    fn updated() -> ExtensionRun {
        run(
            ExtensionRunAction::Update,
            ExtensionRunOutcome::Ok,
            Some("1.0.0"),
            Some("2.0.0"),
        )
    }

    #[test]
    fn an_empty_log_or_a_fresh_install_keeps_nothing() {
        assert_eq!(kept_version(&[]), None);
        assert_eq!(kept_version(&[installed()]), None);
    }

    #[test]
    fn an_update_keeps_the_version_it_replaced() {
        assert_eq!(
            kept_version(&[installed(), updated()]),
            Some("1.0.0".to_owned())
        );
    }

    #[test]
    fn a_rollback_keeps_the_version_it_rolled_back_from_so_a_second_one_goes_forward() {
        let rolled_back = run(
            ExtensionRunAction::Rollback,
            ExtensionRunOutcome::Ok,
            Some("2.0.0"),
            Some("1.0.0"),
        );
        assert_eq!(
            kept_version(&[installed(), updated(), rolled_back]),
            Some("2.0.0".to_owned())
        );
    }

    #[test]
    fn a_failed_run_changes_nothing_about_what_is_kept() {
        let failed = run(
            ExtensionRunAction::Update,
            ExtensionRunOutcome::Failed,
            Some("2.0.0"),
            Some("3.0.0"),
        );
        assert_eq!(
            kept_version(&[installed(), updated(), failed]),
            Some("1.0.0".to_owned())
        );
    }

    #[test]
    fn a_removal_since_the_last_update_leaves_nothing_kept() {
        let removed = run(
            ExtensionRunAction::Remove,
            ExtensionRunOutcome::Ok,
            Some("2.0.0"),
            None,
        );
        assert_eq!(kept_version(&[installed(), updated(), removed]), None);
    }
}
