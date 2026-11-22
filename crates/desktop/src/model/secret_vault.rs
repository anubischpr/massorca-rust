//! The secret vault's page in Settings (decision 0071): the key's state and every name, grouped by scope. Only
//! names and times reach this view, as the service reports them; a value never does.

use std::collections::{BTreeMap, BTreeSet};

use massorca_protocol::methods::projects::Project;
use massorca_protocol::methods::secrets::{SecretEntry, SecretStatus};

use crate::service::SecretsState;

/// What the vault section shows, drawn from the store's vault state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VaultPage {
    /// The service has not answered yet.
    Reading,
    /// The service could not give the vault; the reason in words.
    Unavailable(String),
    /// The service answered with the key's state and every name.
    Listed(VaultListing),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VaultListing {
    /// The key in one line, for example "The key is kept in a file in this profile."
    pub key: String,
    /// The counts in one line: "2 secrets, 3 scopes".
    pub counts: String,
    /// True while the key will not open; the names still show.
    pub locked: bool,
    /// The names by scope: projects by name, then teams, then Global.
    pub groups: Vec<ScopeGroup>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopeGroup {
    pub scope: String,
    pub title: String,
    pub rows: Vec<SecretRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretRow {
    pub scope: String,
    pub name: String,
    /// The title of the scope that owns the value, when this name is a link.
    pub linked_from: Option<String>,
    /// The dates in one line: "Rotated 12 Nov 2026 · Last used 14 Nov 2026".
    pub detail: String,
}

impl VaultPage {
    /// The page for the store's vault state, naming scopes after the projects the store holds.
    pub fn from_state(secrets: &SecretsState, projects: &[Project]) -> Self {
        match (&secrets.status, &secrets.list) {
            (Some(Err(refusal)), _) | (_, Some(Err(refusal))) => {
                Self::Unavailable(refusal.message())
            }
            (Some(Ok(status)), Some(Ok(entries))) => {
                Self::Listed(listing(status, entries, projects))
            }
            _ => Self::Reading,
        }
    }
}

fn listing(status: &SecretStatus, entries: &[SecretEntry], projects: &[Project]) -> VaultListing {
    let scopes: BTreeSet<&str> = entries.iter().map(|entry| entry.scope.as_str()).collect();
    VaultListing {
        key: key_line(status),
        counts: format!(
            "{}, {}",
            count_of(status.count, "secret", "secrets"),
            count_of(scopes.len() as u64, "scope", "scopes")
        ),
        locked: status.state == "locked",
        groups: groups(entries, projects),
    }
}

fn key_line(status: &SecretStatus) -> String {
    if status.state == "locked" {
        return "The vault is locked: its key will not open.".to_owned();
    }
    match status.protection.as_str() {
        "dpapi" => "The key is protected by your Windows account.",
        "secret_service" => "The key is kept by the system keyring.",
        "file" => "The key is kept in a file in this profile.",
        _ => "No key yet. The first secret creates one.",
    }
    .to_owned()
}

fn count_of(count: u64, one: &str, many: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{count} {many}")
    }
}

fn groups(entries: &[SecretEntry], projects: &[Project]) -> Vec<ScopeGroup> {
    let mut by_scope: BTreeMap<&str, Vec<SecretRow>> = BTreeMap::new();
    for entry in entries {
        by_scope
            .entry(entry.scope.as_str())
            .or_default()
            .push(row(entry, projects));
    }
    let mut groups: Vec<ScopeGroup> = by_scope
        .into_iter()
        .map(|(scope, mut rows)| {
            rows.sort_by(|a, b| a.name.cmp(&b.name));
            ScopeGroup {
                scope: scope.to_owned(),
                title: scope_title(scope, projects),
                rows,
            }
        })
        .collect();
    groups.sort_by_key(|group| (scope_rank(&group.scope), group.title.clone()));
    groups
}

fn row(entry: &SecretEntry, projects: &[Project]) -> SecretRow {
    let rotated = format!("Rotated {}", day(entry.rotated_at_ms));
    let (linked_from, detail) = if entry.linked {
        (
            Some(scope_title(&entry.owner_scope, projects)),
            format!("{rotated} · Read-only here"),
        )
    } else {
        let used = match entry.used_at_ms {
            Some(ms) => format!("Last used {}", day(ms)),
            None => "Not used yet".to_owned(),
        };
        (None, format!("{rotated} · {used}"))
    };
    SecretRow {
        scope: entry.scope.clone(),
        name: entry.name.clone(),
        linked_from,
        detail,
    }
}

/// The name a scope shows: its project's name, the team's id, or Global.
pub fn scope_title(scope: &str, projects: &[Project]) -> String {
    if scope == "global" {
        return "Global".to_owned();
    }
    if let Some(id) = scope.strip_prefix("project:") {
        return match projects.iter().find(|project| project.id == id) {
            Some(project) => format!("Project {}", project.name),
            None => format!("Project {id}"),
        };
    }
    match scope.strip_prefix("team:") {
        Some(id) => format!("Team {id}"),
        None => scope.to_owned(),
    }
}

/// Projects first, then teams, then Global, as the design groups them.
fn scope_rank(scope: &str) -> u8 {
    if scope.starts_with("project:") {
        0
    } else if scope.starts_with("team:") {
        1
    } else if scope == "global" {
        2
    } else {
        3
    }
}

/// The UTC calendar day of `ms` since the Unix epoch, as "12 Nov 2026".
pub fn day(ms: i64) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let (year, month, day) = civil_from_days(ms.div_euclid(86_400_000));
    format!("{day} {} {year}", MONTHS[(month - 1) as usize])
}

/// The proleptic Gregorian date of a day count since 1970-01-01 (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::projects::ProjectKind;

    use super::*;
    use crate::service::Refusal;

    const DAY_MS: i64 = 86_400_000;

    fn project(id: &str, name: &str) -> Project {
        Project {
            id: id.into(),
            kind: ProjectKind::Git,
            path: format!("/repos/{name}"),
            name: name.into(),
            workspaces_root: format!("/repos/{name}-worktrees"),
            added_at_ms: 1,
        }
    }

    fn owned(scope: &str, name: &str, rotated_day: i64) -> SecretEntry {
        SecretEntry {
            scope: scope.into(),
            name: name.into(),
            owner_scope: scope.into(),
            linked: false,
            linked_scopes: Vec::new(),
            created_at_ms: 0,
            rotated_at_ms: rotated_day * DAY_MS,
            used_at_ms: None,
        }
    }

    fn linked(scope: &str, owner: &str, name: &str) -> SecretEntry {
        SecretEntry {
            linked: true,
            owner_scope: owner.into(),
            ..owned(scope, name, 0)
        }
    }

    fn status(state: &str, protection: &str, count: u64) -> SecretStatus {
        SecretStatus {
            protection: protection.into(),
            state: state.into(),
            count,
            name_clashes: Vec::new(),
        }
    }

    fn state(status: SecretStatus, entries: Vec<SecretEntry>) -> SecretsState {
        SecretsState {
            status: Some(Ok(status)),
            list: Some(Ok(entries)),
        }
    }

    fn listed(page: VaultPage) -> VaultListing {
        match page {
            VaultPage::Listed(listing) => listing,
            other => panic!("expected a listing, got {other:?}"),
        }
    }

    #[test]
    fn nothing_is_shown_until_the_status_and_the_names_have_both_answered() {
        let page = VaultPage::from_state(&SecretsState::default(), &[]);
        assert_eq!(page, VaultPage::Reading);
        let half = SecretsState {
            status: Some(Ok(status("ready", "none", 0))),
            list: None,
        };
        assert_eq!(VaultPage::from_state(&half, &[]), VaultPage::Reading);
    }

    #[test]
    fn an_empty_vault_has_no_groups_and_says_there_is_no_key_yet() {
        let page = VaultPage::from_state(&state(status("ready", "none", 0), Vec::new()), &[]);
        let listing = listed(page);
        assert!(listing.groups.is_empty());
        assert_eq!(listing.key, "No key yet. The first secret creates one.");
        assert_eq!(listing.counts, "0 secrets, 0 scopes");
        assert!(!listing.locked);
    }

    #[test]
    fn names_group_by_scope_projects_first_then_teams_then_global() {
        let projects = [project("oppm", "oppm"), project("nx", "NexusCare")];
        let page = VaultPage::from_state(
            &state(
                status("ready", "file", 3),
                vec![
                    owned("global", "GITHUB_TOKEN", 10),
                    owned("team:desarrollo", "TEAM_KEY", 4),
                    owned("project:oppm", "STAGING_SMTP_PASSWORD", 2),
                    owned("project:oppm", "OPENAI_API_KEY", 12),
                    owned("project:nx", "DATABASE_URL", 30),
                ],
            ),
            &projects,
        );
        let listing = listed(page);
        let titles: Vec<&str> = listing.groups.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Project NexusCare",
                "Project oppm",
                "Team desarrollo",
                "Global"
            ]
        );
        let oppm: Vec<&str> = listing.groups[1]
            .rows
            .iter()
            .map(|row| row.name.as_str())
            .collect();
        assert_eq!(oppm, ["OPENAI_API_KEY", "STAGING_SMTP_PASSWORD"]);
        assert_eq!(listing.counts, "3 secrets, 4 scopes");
    }

    #[test]
    fn a_linked_name_says_which_scope_owns_it_and_is_read_only_there() {
        let projects = [project("oppm", "oppm"), project("nx", "NexusCare")];
        let listing = listed(VaultPage::from_state(
            &state(
                status("ready", "file", 1),
                vec![linked("project:nx", "project:oppm", "OPENAI_API_KEY")],
            ),
            &projects,
        ));
        let row = &listing.groups[0].rows[0];
        assert_eq!(row.linked_from.as_deref(), Some("Project oppm"));
        assert_eq!(row.detail, "Rotated 1 Jan 1970 · Read-only here");
    }

    #[test]
    fn an_owned_name_shows_its_rotation_and_when_it_was_last_used() {
        // Day 20_769 after the epoch is 12 Nov 2026.
        let mut used = owned("global", "GITHUB_TOKEN", 20_769);
        used.used_at_ms = Some(20_770 * DAY_MS);
        let listing = listed(VaultPage::from_state(
            &state(
                status("ready", "file", 2),
                vec![used, owned("global", "ZED", 1)],
            ),
            &[],
        ));
        let rows = &listing.groups[0].rows;
        assert_eq!(rows[0].name, "GITHUB_TOKEN");
        assert_eq!(rows[0].linked_from, None);
        assert_eq!(
            rows[0].detail,
            "Rotated 12 Nov 2026 · Last used 13 Nov 2026"
        );
        assert_eq!(rows[1].detail, "Rotated 2 Jan 1970 · Not used yet");
    }

    #[test]
    fn a_locked_vault_says_so_and_still_lists_the_names() {
        let listing = listed(VaultPage::from_state(
            &state(status("locked", "file", 1), vec![owned("global", "A", 1)]),
            &[],
        ));
        assert!(listing.locked);
        assert_eq!(listing.key, "The vault is locked: its key will not open.");
        assert_eq!(listing.groups.len(), 1);
    }

    #[test]
    fn a_refused_read_shows_the_refusal_in_words() {
        let refused = SecretsState {
            status: Some(Ok(status("ready", "file", 0))),
            list: Some(Err(Refusal::unavailable(
                "The service cannot be reached right now.",
            ))),
        };
        assert_eq!(
            VaultPage::from_state(&refused, &[]),
            VaultPage::Unavailable("The service cannot be reached right now.".to_owned())
        );
    }

    #[test]
    fn days_are_utc_calendar_days_from_the_epoch() {
        assert_eq!(day(0), "1 Jan 1970");
        assert_eq!(day(11_017 * DAY_MS), "1 Mar 2000", "2000 is a leap year");
        assert_eq!(day(20_769 * DAY_MS + DAY_MS - 1), "12 Nov 2026");
        assert_eq!(day(-1), "31 Dec 1969", "a time before the epoch");
    }
}
