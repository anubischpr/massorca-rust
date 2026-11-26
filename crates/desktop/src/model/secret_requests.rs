//! The pending secret requests the vault page answers (decision 0071): who asks, the name and scope asked for, and the
//! scopes that already own a secret of that name. A request is answered with a new value, with a link to one of those
//! scopes, or declined. Names, reasons and scopes only: no row carries a value.

use massorca_protocol::methods::projects::Project;
use massorca_protocol::methods::secrets::{SecretEntry, SecretRequest};

use super::secret_form::scope_choices;
use super::secret_vault::scope_title;
use crate::service::SecretsState;

/// One pending request as the vault page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestRow {
    pub id: String,
    pub name: String,
    pub scope: String,
    /// The scope as the page names it, such as "Project oppm" or "Global".
    pub scope_title: String,
    /// Who asks, in words: an agent session, or a command run on this computer.
    pub asker: String,
    pub reason: String,
    /// Whether a new value can be stored for it: only the scopes the Add dialog offers, not a team.
    pub can_store: bool,
    /// The scopes that own a secret of this name, as `(owner scope, title)`, to link the request to.
    pub links: Vec<(String, String)>,
}

/// The rows of the pending requests, in the order the service lists them. Empty while they are unread or unreadable.
pub fn request_rows(secrets: &SecretsState, projects: &[Project]) -> Vec<RequestRow> {
    let Some(Ok(requests)) = &secrets.requests else {
        return Vec::new();
    };
    let entries: &[SecretEntry] = match &secrets.list {
        Some(Ok(entries)) => entries,
        _ => &[],
    };
    let storable = scope_choices(projects);
    requests
        .iter()
        .filter(|request| request.state == "pending")
        .map(|request| row(request, entries, projects, &storable))
        .collect()
}

fn row(
    request: &SecretRequest,
    entries: &[SecretEntry],
    projects: &[Project],
    storable: &[String],
) -> RequestRow {
    let links = entries
        .iter()
        .filter(|entry| !entry.linked && entry.name == request.name && entry.scope != request.scope)
        .map(|entry| (entry.scope.clone(), scope_title(&entry.scope, projects)))
        .collect();
    RequestRow {
        id: request.id.clone(),
        name: request.name.clone(),
        scope: request.scope.clone(),
        scope_title: scope_title(&request.scope, projects),
        asker: if request.session.is_some() {
            "An agent".to_owned()
        } else {
            "A command on this computer".to_owned()
        },
        reason: request.reason.clone(),
        can_store: storable.contains(&request.scope),
        links,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::fixtures::project;

    fn request(id: &str, scope: &str, name: &str, session: Option<&str>) -> SecretRequest {
        SecretRequest {
            id: id.to_owned(),
            scope: scope.to_owned(),
            name: name.to_owned(),
            session: session.map(str::to_owned),
            ..crate::service::fixtures::base_secret_request()
        }
    }

    fn owned(scope: &str, name: &str) -> SecretEntry {
        SecretEntry {
            scope: scope.to_owned(),
            name: name.to_owned(),
            owner_scope: scope.to_owned(),
            ..crate::service::fixtures::base_secret_entry()
        }
    }

    fn linked(scope: &str, name: &str, owner: &str) -> SecretEntry {
        SecretEntry {
            linked: true,
            owner_scope: owner.to_owned(),
            ..owned(scope, name)
        }
    }

    #[test]
    fn a_pending_request_names_who_asks_the_name_and_the_scope() {
        let secrets = SecretsState {
            requests: Some(Ok(vec![request(
                "r1",
                "project:p1",
                "OPENAI_API_KEY",
                Some("s1"),
            )])),
            ..SecretsState::default()
        };
        let rows = request_rows(&secrets, &[project("p1", "oppm")]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "r1");
        assert_eq!(rows[0].name, "OPENAI_API_KEY");
        assert_eq!(rows[0].scope_title, "Project oppm");
        assert_eq!(rows[0].asker, "An agent");
        assert!(rows[0].can_store);
    }

    #[test]
    fn a_request_from_the_command_line_says_so() {
        let secrets = SecretsState {
            requests: Some(Ok(vec![request("r2", "global", "KEY", None)])),
            ..SecretsState::default()
        };
        let rows = request_rows(&secrets, &[]);
        assert_eq!(rows[0].asker, "A command on this computer");
    }

    #[test]
    fn a_secret_of_the_same_name_in_another_scope_is_offered_as_a_link() {
        let secrets = SecretsState {
            requests: Some(Ok(vec![request("r1", "project:p1", "KEY", None)])),
            list: Some(Ok(vec![
                owned("global", "KEY"),
                linked("project:p1", "KEY", "global"),
                owned("global", "OTHER"),
            ])),
            ..SecretsState::default()
        };
        let rows = request_rows(&secrets, &[project("p1", "oppm")]);
        let owners: Vec<&str> = rows[0]
            .links
            .iter()
            .map(|(scope, _)| scope.as_str())
            .collect();
        assert_eq!(
            owners,
            ["global"],
            "the request's own scope and other names are not offered"
        );
    }

    #[test]
    fn a_team_scope_is_listed_but_cannot_take_a_value_here() {
        let secrets = SecretsState {
            requests: Some(Ok(vec![request("r3", "team:t1", "KEY", None)])),
            ..SecretsState::default()
        };
        let rows = request_rows(&secrets, &[project("p1", "oppm")]);
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].can_store);
    }

    #[test]
    fn a_resolved_request_is_not_listed_as_pending() {
        let declined = SecretRequest {
            state: "declined".to_owned(),
            ..request("r5", "project:p1", "KEY", None)
        };
        let secrets = SecretsState {
            requests: Some(Ok(vec![declined])),
            ..SecretsState::default()
        };
        assert!(request_rows(&secrets, &[project("p1", "oppm")]).is_empty());
    }

    #[test]
    fn nothing_is_listed_while_the_requests_are_unread_or_unreadable() {
        let unread = SecretsState::default();
        assert!(request_rows(&unread, &[]).is_empty());
        let refused = SecretsState {
            requests: Some(Err(crate::service::Refusal::unavailable("not now"))),
            ..SecretsState::default()
        };
        assert!(request_rows(&refused, &[]).is_empty());
    }
}
