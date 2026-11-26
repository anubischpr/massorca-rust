//! The secret requests an agent session asked for, as the cards its chat transcript draws (X-CH-07). A card names the
//! variable and its state, never a value: it is built from the request rows the service sent, and the owner's links.

use gpui::SharedString;
use massorca_protocol::methods::projects::Project;
use massorca_protocol::methods::secrets::{SecretEntry, SecretRequest};
use massorca_ui::components::secret_request_card::SecretState;
use massorca_ui::components::transcript::{SecretRow, TranscriptRow};

use super::secret_form::scope_choices;
use super::secret_vault::scope_title;
use crate::service::SecretsState;

/// One secret card of a transcript, with the time its request was made: it sits before the first item made after it.
#[derive(Clone, Debug)]
pub struct SecretCard {
    pub at_ms: i64,
    pub row: TranscriptRow,
}

/// The cards of the requests `session` asked, oldest first. `agent` names the asker in the title. `notice` is the last
/// refusal as `(request id, sentence)`, and `entering` is the request whose Add dialog is open.
pub fn session_secret_rows(
    secrets: &SecretsState,
    projects: &[Project],
    session: &str,
    agent: &str,
    notice: Option<&(String, String)>,
    entering: Option<&str>,
) -> Vec<SecretCard> {
    let Some(Ok(requests)) = &secrets.requests else {
        return Vec::new();
    };
    let entries: &[SecretEntry] = match &secrets.list {
        Some(Ok(entries)) => entries,
        _ => &[],
    };
    let storable = scope_choices(projects);
    let mut cards: Vec<SecretCard> = requests
        .iter()
        .filter(|request| request.session.as_deref() == Some(session))
        .filter_map(|request| {
            let state = state_of(request, notice, entering)?;
            let secret = SecretRow {
                request_id: request.id.as_str().into(),
                name: request.name.as_str().into(),
                title: format!("{agent} needs a secret").into(),
                reason: non_empty(&request.reason),
                scope: scope_text(request, entries, projects).into(),
                state,
                storable: storable.contains(&request.scope),
            };
            Some(SecretCard {
                at_ms: request.created_at_ms,
                row: TranscriptRow::Secret {
                    id: format!("secret-{}", request.id).into(),
                    secret,
                },
            })
        })
        .collect();
    cards.sort_by_key(|card| card.at_ms);
    cards
}

/// The state a request's card shows. A pending request whose Add dialog is open is entering, and one with a refusal
/// shows it; an unknown state has no card at all, so it is never drawn as done or as asking.
fn state_of(
    request: &SecretRequest,
    notice: Option<&(String, String)>,
    entering: Option<&str>,
) -> Option<SecretState> {
    Some(match request.state.as_str() {
        "pending" => {
            let refused = notice
                .filter(|(id, _)| *id == request.id)
                .map(|(_, sentence)| sentence.as_str());
            if entering == Some(request.id.as_str()) {
                SecretState::Entering
            } else if let Some(sentence) = refused {
                SecretState::Failed {
                    reason: sentence.into(),
                }
            } else {
                SecretState::Pending
            }
        }
        "stored" => SecretState::Stored,
        "linked" => SecretState::Linked,
        "declined" => SecretState::Declined,
        "failed" => SecretState::Failed {
            reason: "the request failed".into(),
        },
        _ => return None,
    })
}

/// Where the secret belongs: "for project oppm", or "from global" once the request is linked to the scope that owns it.
fn scope_text(request: &SecretRequest, entries: &[SecretEntry], projects: &[Project]) -> String {
    let owner = entries
        .iter()
        .find(|entry| entry.linked && entry.scope == request.scope && entry.name == request.name);
    match owner {
        Some(entry) if request.state == "linked" => {
            format!(
                "from {}",
                lower_first(&scope_title(&entry.owner_scope, projects))
            )
        }
        _ => format!(
            "for {}",
            lower_first(&scope_title(&request.scope, projects))
        ),
    }
}

/// The scope title with its first letter lower-cased, as the scope line reads: "project oppm".
fn lower_first(title: &str) -> String {
    let mut chars = title.chars();
    let mut lowered = chars
        .next()
        .map(|first| first.to_lowercase().collect::<String>())
        .unwrap_or_default();
    lowered.push_str(chars.as_str());
    lowered
}

/// The reason as the card shows it, or none when the agent gave none.
fn non_empty(reason: &str) -> Option<SharedString> {
    let reason = reason.trim();
    (!reason.is_empty()).then(|| reason.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::fixtures::{base_secret_entry, base_secret_request, project};

    fn request(id: &str, session: Option<&str>, state: &str, created: i64) -> SecretRequest {
        SecretRequest {
            id: id.to_owned(),
            scope: "project:p1".to_owned(),
            name: "OPENAI_API_KEY".to_owned(),
            reason: "Calls the OpenAI API for the project.".to_owned(),
            session: session.map(str::to_owned),
            state: state.to_owned(),
            created_at_ms: created,
            ..base_secret_request()
        }
    }

    fn secrets(requests: Vec<SecretRequest>, entries: Vec<SecretEntry>) -> SecretsState {
        SecretsState {
            requests: Some(Ok(requests)),
            list: Some(Ok(entries)),
            ..SecretsState::default()
        }
    }

    fn cards_of(cards: &[SecretCard]) -> Vec<(&str, SecretState)> {
        cards
            .iter()
            .map(|card| match &card.row {
                TranscriptRow::Secret { id, secret } => (id.as_ref(), secret.state.clone()),
                other => panic!("a secret card, not {other:?}"),
            })
            .collect()
    }

    fn secret_of(card: &SecretCard) -> &SecretRow {
        match &card.row {
            TranscriptRow::Secret { secret, .. } => secret,
            other => panic!("a secret card, not {other:?}"),
        }
    }

    #[test]
    fn a_pending_request_of_the_session_is_a_card_titled_for_its_agent() {
        let secrets = secrets(vec![request("r1", Some("s1"), "pending", 10)], Vec::new());

        let cards = session_secret_rows(
            &secrets,
            &[project("p1", "alpha")],
            "s1",
            "Claude Code",
            None,
            None,
        );

        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].at_ms, 10);
        let secret = secret_of(&cards[0]);
        assert_eq!(secret.request_id.as_ref(), "r1");
        assert_eq!(secret.name.as_ref(), "OPENAI_API_KEY");
        assert_eq!(secret.title.as_ref(), "Claude Code needs a secret");
        assert_eq!(
            secret.reason.as_deref(),
            Some("Calls the OpenAI API for the project.")
        );
        assert_eq!(secret.scope.as_ref(), "for project alpha");
        assert_eq!(secret.state, SecretState::Pending);
        assert!(secret.storable, "a project's secret can be stored here");
    }

    #[test]
    fn requests_of_other_sessions_and_of_unknown_states_have_no_card_here() {
        let secrets = secrets(
            vec![
                request("r1", Some("s2"), "pending", 10),
                request("r2", Some("s1"), "unknown", 11),
                request("r3", None, "pending", 12),
            ],
            Vec::new(),
        );

        let cards = session_secret_rows(&secrets, &[], "s1", "Claude Code", None, None);

        assert!(cards.is_empty());
    }

    #[test]
    fn a_settled_request_says_how_it_settled_and_a_link_names_its_owner() {
        let linked = SecretEntry {
            scope: "project:p1".to_owned(),
            name: "OPENAI_API_KEY".to_owned(),
            owner_scope: "global".to_owned(),
            linked: true,
            ..base_secret_entry()
        };
        let secrets = secrets(
            vec![
                request("r1", Some("s1"), "linked", 10),
                request("r2", Some("s1"), "stored", 11),
                request("r3", Some("s1"), "declined", 12),
            ],
            vec![linked],
        );

        let cards = session_secret_rows(
            &secrets,
            &[project("p1", "alpha")],
            "s1",
            "Claude Code",
            None,
            None,
        );

        assert_eq!(
            cards_of(&cards),
            [
                ("secret-r1", SecretState::Linked),
                ("secret-r2", SecretState::Stored),
                ("secret-r3", SecretState::Declined),
            ]
        );
        assert_eq!(secret_of(&cards[0]).scope.as_ref(), "from global");
        assert_eq!(secret_of(&cards[1]).scope.as_ref(), "for project alpha");
    }

    #[test]
    fn a_refused_answer_shows_its_sentence_and_the_request_whose_dialog_is_open_is_entering() {
        let secrets = secrets(
            vec![
                request("r1", Some("s1"), "pending", 10),
                request("r2", Some("s1"), "pending", 11),
            ],
            Vec::new(),
        );
        let notice = ("r1".to_owned(), "The vault is locked.".to_owned());

        let cards = session_secret_rows(
            &secrets,
            &[project("p1", "alpha")],
            "s1",
            "Claude Code",
            Some(&notice),
            Some("r2"),
        );

        assert_eq!(
            cards_of(&cards),
            [
                (
                    "secret-r1",
                    SecretState::Failed {
                        reason: "The vault is locked.".into()
                    }
                ),
                ("secret-r2", SecretState::Entering),
            ]
        );
    }

    #[test]
    fn cards_come_oldest_first_and_a_team_scope_cannot_be_stored_here() {
        let team = SecretRequest {
            scope: "team:t1".to_owned(),
            ..request("r2", Some("s1"), "pending", 5)
        };
        let secrets = secrets(
            vec![request("r1", Some("s1"), "pending", 20), team],
            Vec::new(),
        );

        let cards = session_secret_rows(
            &secrets,
            &[project("p1", "alpha")],
            "s1",
            "Claude Code",
            None,
            None,
        );

        assert_eq!(
            cards.iter().map(|card| card.at_ms).collect::<Vec<_>>(),
            [5, 20]
        );
        assert!(!secret_of(&cards[0]).storable);
        assert!(secret_of(&cards[1]).storable);
    }
}
