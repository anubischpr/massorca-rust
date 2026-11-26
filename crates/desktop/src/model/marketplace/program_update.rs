//! Updating a host program MassOrca installed (decision 0151): when the Update action shows, and the dialog's words.
//! The update dialog reuses the install review: one sentence, the command and consent under Show details.

use massorca_protocol::methods::extensions::ProgramUpdateReview;

use super::host_target::HostTarget;
use super::install_review::ReviewOutcome;
use super::types::{ItemKind, MarketplaceItem};

/// The host program of a tool item that Update acts on: offered only when the service offers `extensions-programs/1`,
/// the item is installed on This computer, MassOrca installed the program, and the package names an update command for
/// `os`. Anything else gets no Update action, so the menu never offers one that cannot work.
pub fn updatable_program(item: &MarketplaceItem, offered: bool, os: &str) -> Option<String> {
    if !offered
        || item.kind != ItemKind::Tool
        || !item.state.is_installed()
        || item.host != HostTarget::Local.id()
    {
        return None;
    }
    item.extension
        .as_deref()?
        .programs
        .iter()
        .find(|program| {
            program.cli_installed_by_package
                && program
                    .cli
                    .as_ref()
                    .is_some_and(|cli| cli.update.iter().any(|step| step.os == os))
        })
        .map(|program| program.id.clone())
}

/// The dialog's words: one sentence, and the primary button Update.
pub fn program_update_outcome(name: &str) -> ReviewOutcome {
    ReviewOutcome::Update {
        title: format!("Update {name}"),
        sentence: format!("MassOrca will install the latest {name} on this computer."),
    }
}

/// The lines under Show details: the command, its check, and the consent the update sends.
pub fn program_update_details(review: &ProgramUpdateReview) -> Vec<String> {
    let mut lines = vec![format!(
        "{}: this vendor command runs: {}. It installs the vendor's current release; the version is not pinned.",
        review.program,
        review.command.join(" ")
    )];
    if let Some(verify) = &review.verify {
        lines.push(format!("Verify after the update: {}", verify.join(" ")));
    }
    lines.push(format!("Consent fingerprint: {}", review.consent));
    lines
}

/// The rows of an item's More menu: Open always, Update when [`updatable_program`] allows it, Uninstall when installed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemMenuId {
    Open,
    Update,
    Uninstall,
}

pub fn item_menu(item: &MarketplaceItem, updatable: bool) -> Vec<ItemMenuId> {
    let mut rows = vec![ItemMenuId::Open];
    if updatable {
        rows.push(ItemMenuId::Update);
    }
    if item.state.is_installed() {
        rows.push(ItemMenuId::Uninstall);
    }
    rows
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::extensions::{AgentItem, CliSpec, CliStep, ExtensionState};

    use super::*;
    use crate::model::marketplace::extension_items::tests::{items, package};

    fn git(state: ExtensionState, by_package: bool) -> MarketplaceItem {
        let mut entry = package("massorca.git", "tools", state);
        entry.programs = vec![AgentItem {
            id: "git".to_owned(),
            detected: true,
            program_path: None,
            path_source: None,
            cli_installed_by_package: by_package,
            cli: Some(CliSpec {
                binary: "git".to_owned(),
                verify: None,
                source: "https://git-scm.com/downloads".to_owned(),
                source_checked: None,
                install: Vec::new(),
                update: vec![CliStep {
                    os: "windows".to_owned(),
                    command: vec!["winget".to_owned(), "upgrade".to_owned()],
                }],
                uninstall: Vec::new(),
            }),
        }];
        items(&[entry], &[], &[]).remove(0)
    }

    #[test]
    fn update_shows_only_for_a_program_massorca_installed_when_offered() {
        let ours = git(ExtensionState::Enabled, true);
        assert_eq!(
            updatable_program(&ours, true, "windows").as_deref(),
            Some("git")
        );
        assert_eq!(
            updatable_program(&ours, false, "windows"),
            None,
            "not offered"
        );
        assert_eq!(updatable_program(&ours, true, "linux"), None, "no command");
        let theirs = git(ExtensionState::Enabled, false);
        assert_eq!(updatable_program(&theirs, true, "windows"), None);
        let available = git(ExtensionState::Available, true);
        assert_eq!(updatable_program(&available, true, "windows"), None);
    }

    #[test]
    fn the_more_menu_lists_only_actions_that_work() {
        let ours = git(ExtensionState::Enabled, true);
        assert_eq!(
            item_menu(&ours, true),
            [ItemMenuId::Open, ItemMenuId::Update, ItemMenuId::Uninstall]
        );
        assert_eq!(
            item_menu(&ours, false),
            [ItemMenuId::Open, ItemMenuId::Uninstall]
        );
    }

    #[test]
    fn the_update_dialog_says_one_sentence_and_keeps_the_command_in_details() {
        let outcome = program_update_outcome("Git");
        assert_eq!(outcome.button(), "Update");
        assert_eq!(outcome.title(), "Update Git");
        let review = ProgramUpdateReview {
            id: "massorca.git".to_owned(),
            program: "git".to_owned(),
            binary: "git".to_owned(),
            command: vec!["winget".to_owned(), "upgrade".to_owned()],
            verify: Some(vec!["git".to_owned(), "--version".to_owned()]),
            consent: "fp".to_owned(),
        };
        let details = program_update_details(&review);
        assert!(details[0].contains("winget upgrade"), "{details:?}");
        assert!(details.contains(&"Consent fingerprint: fp".to_owned()));
        assert!(details.contains(&"Verify after the update: git --version".to_owned()));
    }
}
