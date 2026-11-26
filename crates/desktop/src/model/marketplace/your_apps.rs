//! Your apps: the installed skills and the connected hosted services the Installed list shows beside its packages
//! (SKL-27). Each row says what the picked host reports and carries only the one action that works for it.

use std::collections::BTreeSet;

use super::hosted_services::ServiceRow;
use super::skill_placement::ManagedRow;
use super::skills::SkillRow;
use super::types::ItemKind;

/// The one action a row's More menu offers. A row without one has no menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppAction {
    /// Removes the copies MassOrca placed for a skill, after the owner confirms.
    RemovePlacement(String),
    /// Disconnects a hosted service by its id.
    Disconnect(String),
}

/// A skill or a hosted service as a row of Your apps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppRow {
    pub kind: ItemKind,
    /// Unique among the rows and stable across reads: `skill:<name>` or `service:<id>`.
    pub key: String,
    pub name: String,
    /// The line under the name: the agent tools a skill is in, or the account a service is connected with.
    pub detail: String,
    /// Placed, Installed or Connected.
    pub status: &'static str,
    pub action: Option<AppAction>,
}

/// The skills the picked host has installed in an agent tool's folder, and the skills MassOrca placed. `placed` is the
/// placed list of This computer; a paired host passes none, because placements are made here only. Removing a placement
/// is offered only on This computer.
pub fn skill_apps(found: &[SkillRow], placed: &[ManagedRow], local: bool) -> Vec<AppRow> {
    let names: BTreeSet<&str> = found
        .iter()
        .filter(|skill| skill.installed)
        .map(|skill| skill.name.as_str())
        .chain(placed.iter().map(|row| row.name.as_str()))
        .collect();
    let mut rows: Vec<AppRow> = names
        .into_iter()
        .map(|name| {
            let placement = placed.iter().find(|row| row.name == name);
            let detail = match (
                placement,
                found
                    .iter()
                    .find(|skill| skill.name == name && skill.installed),
            ) {
                (Some(row), _) => row.agents.clone(),
                (None, Some(skill)) => skill.agents.join(", "),
                (None, None) => String::new(),
            };
            skill_row(
                name,
                detail,
                placement.is_some(),
                local && placement.is_some(),
            )
        })
        .collect();
    rows.sort_by_cached_key(|row| (row.name.to_lowercase(), row.key.clone()));
    rows
}

fn skill_row(name: &str, detail: String, placed: bool, removable: bool) -> AppRow {
    AppRow {
        kind: ItemKind::Skill,
        key: format!("skill:{name}"),
        name: name.to_owned(),
        detail,
        status: if placed { "Placed" } else { "Installed" },
        action: removable.then(|| AppAction::RemovePlacement(name.to_owned())),
    }
}

/// The connected services of a host, in the order the host lists them. A service that is not connected is not an
/// installed app; its Connect and Disconnect stay on the Services section.
pub fn service_apps(services: &[ServiceRow]) -> Vec<AppRow> {
    services
        .iter()
        .filter(|service| service.connected)
        .map(|service| AppRow {
            kind: ItemKind::Service,
            key: format!("service:{}", service.id),
            name: service.name.clone(),
            detail: service.address.clone().unwrap_or_default(),
            status: "Connected",
            action: service
                .can_disconnect
                .then(|| AppAction::Disconnect(service.id.clone())),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::marketplace::hosted_services::ServiceAction;

    fn found(name: &str, installed: bool) -> SkillRow {
        SkillRow {
            id: format!("home/{name}"),
            name: name.to_owned(),
            description: None,
            agents: vec!["Claude"],
            source: "Claude home".to_owned(),
            placeable: true,
            installed,
        }
    }

    fn placed(name: &str) -> ManagedRow {
        ManagedRow {
            name: name.to_owned(),
            agents: "Claude".to_owned(),
        }
    }

    fn service(id: &str, connected: bool, can_disconnect: bool) -> ServiceRow {
        ServiceRow {
            id: id.to_owned(),
            name: format!("Name of {id}"),
            state: if connected {
                "Connected"
            } else {
                "Not connected"
            },
            connected,
            address: connected.then(|| "user@example.com".to_owned()),
            detail: None,
            action: ServiceAction::Replace,
            can_disconnect,
        }
    }

    #[test]
    fn an_installed_home_skill_is_a_row_without_a_menu() {
        let rows = skill_apps(&[found("notes", true)], &[], true);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, "skill:notes");
        assert_eq!(rows[0].status, "Installed");
        assert_eq!(rows[0].action, None);
    }

    #[test]
    fn a_skill_only_in_the_home_folder_is_not_an_installed_app() {
        assert!(skill_apps(&[found("draft", false)], &[], true).is_empty());
    }

    #[test]
    fn a_placed_skill_offers_remove_placement_on_this_computer_only() {
        let here = skill_apps(&[found("notes", true)], &[placed("notes")], true);
        assert_eq!(here[0].status, "Placed");
        assert_eq!(
            here[0].action,
            Some(AppAction::RemovePlacement("notes".to_owned()))
        );
        let paired = skill_apps(&[found("notes", true)], &[], false);
        assert_eq!(paired[0].action, None);
    }

    #[test]
    fn a_placed_skill_the_scan_does_not_list_is_still_a_row() {
        let rows = skill_apps(&[], &[placed("archived")], true);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "archived");
        assert_eq!(rows[0].detail, "Claude");
        assert_eq!(
            rows[0].action,
            Some(AppAction::RemovePlacement("archived".to_owned()))
        );
    }

    #[test]
    fn only_connected_services_are_apps_and_only_removable_ones_disconnect() {
        let rows = service_apps(&[
            service("gitea", true, true),
            service("linear", true, false),
            service("github", false, false),
        ]);
        let keys: Vec<&str> = rows.iter().map(|row| row.key.as_str()).collect();
        assert_eq!(keys, ["service:gitea", "service:linear"]);
        assert_eq!(
            rows[0].action,
            Some(AppAction::Disconnect("gitea".to_owned()))
        );
        assert_eq!(rows[1].action, None);
        assert_eq!(rows[0].status, "Connected");
    }

    #[test]
    fn nothing_found_and_nothing_placed_lists_no_skill_apps() {
        assert!(skill_apps(&[], &[], true).is_empty());
    }
}
