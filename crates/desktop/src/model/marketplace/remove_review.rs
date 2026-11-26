//! The remove confirm's words (owner ruling 2026-11-24): one sentence on what removing does in MassOrca, and the
//! exact effects under Show details. A vendor program that stays is never said to be removed from this computer.

use super::item_page::offers_cli_removal;
use super::types::{ItemKind, MarketplaceItem};

/// What the remove confirm says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveOutcome {
    pub title: String,
    pub sentence: String,
    pub button: &'static str,
}

/// Whether `item` offers an agent, whose accounts MassOrca forgets when it is removed.
fn is_agent(item: &MarketplaceItem) -> bool {
    item.kind == ItemKind::Agent
        || item.agent_kind.is_some()
        || item
            .extension
            .as_deref()
            .is_some_and(|extension| !extension.agents.is_empty())
}

/// The confirm for `item`; `remove_cli` is the person's tick for also uninstalling the CLI the package installed.
#[must_use]
pub fn remove_outcome(item: &MarketplaceItem, remove_cli: bool) -> RemoveOutcome {
    let name = &item.name;
    let ours = offers_cli_removal(item);
    let sentence = if !is_agent(item) {
        format!("MassOrca will remove {name} and the data it kept here.")
    } else if ours {
        format!("MassOrca will stop offering {name} and forget the {name} accounts you added here.")
    } else {
        format!(
            "MassOrca will stop offering {name} and forget the {name} accounts you added here. {name} itself stays installed on this computer."
        )
    };
    RemoveOutcome {
        title: format!("Remove {name} from MassOrca?"),
        sentence,
        button: if ours && remove_cli {
            "Remove and uninstall"
        } else {
            "Remove"
        },
    }
}

/// The exact effects of the removal, one per line, for Show details.
#[must_use]
pub fn remove_details(item: &MarketplaceItem, remove_cli: bool) -> Vec<String> {
    let Some(extension) = item.extension.as_deref() else {
        return Vec::new();
    };
    let effects = &extension.uninstall;
    let mut lines = Vec::new();
    if effects.package_files {
        lines.push("Its package files are deleted.".to_owned());
    }
    if effects.storage {
        lines.push("Its data is deleted.".to_owned());
    }
    if effects.secrets_unscoped {
        lines.push("Every secret it stored is deleted.".to_owned());
    } else if !effects.secret_names.is_empty() {
        lines.push(format!(
            "Its secrets are deleted: {}.",
            effects.secret_names.join(", ")
        ));
    }
    if !effects.connections.is_empty() {
        lines.push(format!(
            "Its connections are removed: {}.",
            effects.connections.join(", ")
        ));
    }
    let records = extension
        .installed
        .iter()
        .flat_map(|installed| &installed.cli);
    let mut binaries: Vec<(String, bool)> = records
        .map(|record| (record.binary.clone(), record.installed_by_package))
        .collect();
    for binary in &effects.cli_binaries {
        if !binaries.iter().any(|(known, _)| known == binary) {
            binaries.push((binary.clone(), true));
        }
    }
    for (binary, ours) in binaries {
        lines.push(if ours && remove_cli {
            format!("The {binary} CLI MassOrca installed is uninstalled.")
        } else {
            format!("The {binary} CLI stays on this computer.")
        });
    }
    lines
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::extensions::{
        CliRecord, ExtensionState, InstalledInfo, SourceKind, SourceRef,
    };

    use super::*;
    use crate::model::marketplace::extension_items::tests::{agent_package, items, package};

    const NEVER: &str = "removed from this computer";

    /// The installed Claude package; `ours` says whether the package installed its CLI.
    fn claude(ours: bool) -> MarketplaceItem {
        let mut entry = agent_package("claude", ExtensionState::Enabled);
        entry.name = "Claude".to_owned();
        entry.uninstall.storage = true;
        entry.uninstall.connections = vec!["claude".to_owned()];
        entry.uninstall.cli_binaries = vec!["claude".to_owned()];
        entry.installed = Some(InstalledInfo {
            source: SourceRef {
                kind: SourceKind::Catalogue,
                reference: "massorca.claude".to_owned(),
            },
            cli: vec![CliRecord {
                agent: "claude".to_owned(),
                binary: "claude".to_owned(),
                detected_before_install: !ours,
                installer: None,
                installed_by_package: ours,
            }],
            ..crate::service::fixtures::base_installed_info()
        });
        items(&[entry.clone()], &[entry], &[]).remove(0)
    }

    fn says(outcome: &RemoveOutcome, details: &[String]) -> String {
        format!(
            "{}\n{}\n{}",
            outcome.title,
            outcome.sentence,
            details.join("\n")
        )
    }

    const FIRST: &str =
        "MassOrca will stop offering Claude and forget the Claude accounts you added here.";

    #[test]
    fn a_cli_not_installed_by_massorca_stays_installed() {
        let item = claude(false);
        let outcome = remove_outcome(&item, false);
        assert_eq!(
            outcome,
            RemoveOutcome {
                title: "Remove Claude from MassOrca?".to_owned(),
                sentence: format!("{FIRST} Claude itself stays installed on this computer."),
                button: "Remove",
            }
        );
        let details = remove_details(&item, false);
        assert!(
            details.contains(&"The claude CLI stays on this computer.".to_owned()),
            "{details:?}"
        );
        assert!(!says(&outcome, &details).contains(NEVER));
    }

    #[test]
    fn a_cli_massorca_installed_and_left_unticked_stays() {
        let item = claude(true);
        let outcome = remove_outcome(&item, false);
        assert_eq!(
            (
                outcome.title.as_str(),
                outcome.sentence.as_str(),
                outcome.button
            ),
            ("Remove Claude from MassOrca?", FIRST, "Remove")
        );
        let details = remove_details(&item, false);
        assert!(
            details.contains(&"The claude CLI stays on this computer.".to_owned()),
            "{details:?}"
        );
        assert!(!says(&outcome, &details).contains(NEVER));
    }

    #[test]
    fn a_cli_massorca_installed_and_ticked_is_uninstalled_too() {
        let item = claude(true);
        let outcome = remove_outcome(&item, true);
        assert_eq!(
            (outcome.sentence.as_str(), outcome.button),
            (FIRST, "Remove and uninstall")
        );
        let details = remove_details(&item, true);
        assert!(
            details.contains(&"The claude CLI MassOrca installed is uninstalled.".to_owned()),
            "{details:?}"
        );
        for line in [
            "Its package files are deleted.",
            "Its data is deleted.",
            "Its connections are removed: claude.",
        ] {
            assert!(details.contains(&line.to_owned()), "{line}: {details:?}");
        }
    }

    #[test]
    fn a_package_without_an_agent_or_cli_is_removed_with_its_data() {
        let mut entry = package("acme.notes", "extensions", ExtensionState::Enabled);
        entry.name = "Notes".to_owned();
        let item = items(&[entry.clone()], &[entry], &[]).remove(0);
        let outcome = remove_outcome(&item, false);
        assert_eq!(
            outcome,
            RemoveOutcome {
                title: "Remove Notes from MassOrca?".to_owned(),
                sentence: "MassOrca will remove Notes and the data it kept here.".to_owned(),
                button: "Remove",
            }
        );
        assert_eq!(
            remove_details(&item, false),
            vec!["Its package files are deleted.".to_owned()]
        );
    }
}
