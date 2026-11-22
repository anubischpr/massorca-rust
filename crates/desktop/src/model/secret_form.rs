//! The dialog that adds or rotates a secret (decision 0071): the name rule, the scopes, and what may be sent. The
//! value is held from the paste until a save sends it, and then it is gone. The form never shows or prints it.

use std::fmt;

use massorca_protocol::codes;
use massorca_protocol::methods::projects::Project;
use massorca_protocol::methods::secrets::{SecretRemove, SecretSet};

use crate::model::secret_vault::count_of;
use crate::service::Refusal;

/// The longest variable name a secret can have (A29).
pub const NAME_MAX_CHARS: usize = 64;

/// Why `name` cannot be stored, in words, or `None` when it can. The rule (A29): letters, digits and
/// underscores, not starting with a digit, and at most 64 characters.
pub fn name_problem(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        Some("Enter a variable name.")
    } else if name.chars().count() > NAME_MAX_CHARS {
        Some("Use up to 64 characters.")
    } else if name.starts_with(|c: char| c.is_ascii_digit())
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        Some("Use letters, digits and underscores. It cannot start with a digit.")
    } else {
        None
    }
}

/// The scopes an add can store to: each project by name, then Global.
pub fn scope_choices(projects: &[Project]) -> Vec<String> {
    let mut sorted: Vec<&Project> = projects.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    sorted
        .into_iter()
        .map(|project| format!("project:{}", project.id))
        .chain(std::iter::once("global".to_owned()))
        .collect()
}

/// Whether the form adds a new name or rotates an owned one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormMode {
    Add,
    Rotate,
}

/// What the form is doing with the service, and why it last stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormStatus {
    Idle,
    Saving,
    Refused(String),
}

/// One secret's form. Its value is held from the paste until a save takes it.
pub struct SecretForm {
    pub mode: FormMode,
    /// The scope the value goes to; fixed for a rotation.
    pub scope: String,
    /// The name a rotation replaces. An add takes its name from the field when it is saved.
    pub name: String,
    value: Option<String>,
    pub status: FormStatus,
}

impl fmt::Debug for SecretForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretForm")
            .field("mode", &self.mode)
            .field("scope", &self.scope)
            .field("name", &self.name)
            .field("value", &self.value.as_ref().map(|_| "[redacted]"))
            .field("status", &self.status)
            .finish()
    }
}

impl SecretForm {
    /// An add to `scope`, with no value pasted yet.
    pub fn add(scope: String) -> Self {
        Self::new(FormMode::Add, scope, String::new())
    }

    /// A rotation of the owned secret `name` in `scope`, with no value pasted yet.
    pub fn rotate(scope: String, name: String) -> Self {
        Self::new(FormMode::Rotate, scope, name)
    }

    fn new(mode: FormMode, scope: String, name: String) -> Self {
        Self {
            mode,
            scope,
            name,
            value: None,
            status: FormStatus::Idle,
        }
    }

    /// Whether a value is waiting to be sent. The form never says what it is.
    pub fn has_value(&self) -> bool {
        self.value.is_some()
    }

    /// Takes the clipboard text as the value. Empty text is refused and keeps whatever was there, and nothing
    /// is taken while a save is in flight.
    pub fn paste(&mut self, text: &str) {
        if self.status == FormStatus::Saving {
            return;
        }
        if text.is_empty() {
            self.status = FormStatus::Refused(
                "The clipboard holds no text. Copy the value, then paste it here.".to_owned(),
            );
        } else {
            self.value = Some(text.to_owned());
            self.status = FormStatus::Idle;
        }
    }

    /// Whether the save may go: a valid name, a pasted value, the service connected, and nothing in flight.
    pub fn can_save(&self, name: &str, connected: bool) -> bool {
        connected
            && name_problem(name).is_none()
            && self.value.is_some()
            && self.status != FormStatus::Saving
    }

    /// The request that stores `name` with the pasted value. The value leaves the form here: it is sent once and
    /// never kept. A name or a missing value is refused first, and then the value stays where it was.
    pub fn request(&mut self, name: &str) -> Result<SecretSet, &'static str> {
        if let Some(problem) = name_problem(name) {
            return Err(problem);
        }
        let Some(value) = self.value.take() else {
            return Err("Paste the value first.");
        };
        self.status = FormStatus::Saving;
        Ok(SecretSet {
            scope: self.scope.clone(),
            name: name.to_owned(),
            value,
            replace: self.mode == FormMode::Rotate,
        })
    }

    /// The service refused the save: the form stays open and says why. The value was already sent.
    pub fn refused(&mut self, sentence: String) {
        self.status = FormStatus::Refused(sentence);
    }
}

/// The confirm before an owned secret is deleted (D-VT-02, merged into X-DLG-01).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoveForm {
    pub scope: String,
    pub name: String,
    /// The scopes that link to the secret. Their links go too, but only once the confirm has named them.
    pub linked_scopes: Vec<String>,
    pub status: FormStatus,
}

impl RemoveForm {
    pub fn new(scope: String, name: String, linked_scopes: Vec<String>) -> Self {
        Self {
            scope,
            name,
            linked_scopes,
            status: FormStatus::Idle,
        }
    }

    /// The request that deletes the secret. The links go with it only when there are some, which the confirm showed.
    pub fn request(&mut self) -> SecretRemove {
        self.status = FormStatus::Saving;
        SecretRemove {
            scope: self.scope.clone(),
            name: self.name.clone(),
            include_links: !self.linked_scopes.is_empty(),
            force: false,
        }
    }

    /// The deletion was refused: the dialog stays and says why.
    pub fn refused(&mut self, sentence: String) {
        self.status = FormStatus::Refused(sentence);
    }
}

/// The action's words: "Delete secret", or "Delete secret (1 project, 1 team)" when links go with it.
pub fn delete_label(linked_scopes: &[String]) -> String {
    let projects = linked_scopes
        .iter()
        .filter(|scope| scope.starts_with("project:"))
        .count();
    let teams = linked_scopes
        .iter()
        .filter(|scope| scope.starts_with("team:"))
        .count();
    let mut parts = Vec::new();
    if projects > 0 {
        parts.push(count_of(projects as u64, "project", "projects"));
    }
    if teams > 0 {
        parts.push(count_of(teams as u64, "team", "teams"));
    }
    if parts.is_empty() {
        "Delete secret".to_owned()
    } else {
        format!("Delete secret ({})", parts.join(", "))
    }
}

/// What a refused deletion says: the reason in plain words, and that the secret is still there.
pub fn removal_refusal_sentence(refusal: &Refusal, name: &str) -> String {
    let reason = match refusal {
        Refusal::Failed { code, .. } if code.as_str() == codes::SECRET_NOT_FOUND => {
            format!("There is no secret {name} in this scope to delete")
        }
        other => other.message(),
    };
    format!(
        "Not deleted: {}. The secret is still there.",
        reason.trim_end_matches('.')
    )
}

/// What a refused save says, in plain words, always ending with "Nothing was stored.".
pub fn refusal_sentence(refusal: &Refusal, name: &str) -> String {
    let reason = match refusal {
        Refusal::Failed { code, .. } => match code.as_str() {
            codes::SECRET_NAME_IN_USE => {
                format!("{name} already exists in this scope. Pick another name")
            }
            codes::SECRET_NOT_FOUND => {
                format!("There is no secret {name} in this scope to replace. Add it instead")
            }
            codes::SECRET_INVALID_NAME => {
                "Use letters, digits and underscores. It cannot start with a digit".to_owned()
            }
            codes::SECRET_TOO_LARGE => "The value is too long to store".to_owned(),
            codes::SECRET_VAULT_LOCKED => "The vault is locked: its key will not open".to_owned(),
            codes::SECRET_INVALID_SCOPE => "That scope is not valid".to_owned(),
            codes::SECRET_LINKED => {
                format!("{name} is a link in this scope. Change it where it is owned")
            }
            _ => refusal.message(),
        },
        other => other.message(),
    };
    format!(
        "Not saved: {}. Nothing was stored.",
        reason.trim_end_matches('.')
    )
}

#[cfg(test)]
mod tests {
    use massorca_protocol::ErrorBody;
    use massorca_protocol::methods::projects::ProjectKind;

    use super::*;
    use crate::service::Refusal;

    const VALUE: &str = "form-test-value-5e6f";

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

    fn remote(code: &str) -> Refusal {
        Refusal::Failed {
            code: code.to_owned(),
            message: "the service says no".to_owned(),
        }
    }

    #[test]
    fn a_name_must_be_a_variable_name_of_at_most_64_characters() {
        assert_eq!(name_problem("OPENAI_API_KEY"), None);
        assert_eq!(name_problem("_1"), None, "a leading underscore is fine");
        assert!(name_problem("").is_some());
        assert!(name_problem("1ST").is_some(), "a digit cannot start a name");
        assert!(name_problem("A-B").is_some(), "a dash is not allowed");
        assert!(name_problem("ÑAME").is_some(), "only ASCII letters");
        assert_eq!(name_problem(&"A".repeat(NAME_MAX_CHARS)), None);
        assert!(name_problem(&"A".repeat(NAME_MAX_CHARS + 1)).is_some());
    }

    #[test]
    fn the_add_scopes_are_the_projects_by_name_then_global() {
        let projects = [project("nx", "NexusCare"), project("oppm", "oppm")];
        assert_eq!(
            scope_choices(&projects),
            ["project:nx", "project:oppm", "global"]
        );
        assert_eq!(scope_choices(&[]), ["global"]);
    }

    #[test]
    fn a_request_takes_the_value_out_of_the_form_and_sends_it_once() {
        let mut form = SecretForm::add("global".to_owned());
        form.paste(VALUE);
        assert!(form.can_save("OPENAI_API_KEY", true));
        let sent = form
            .request("OPENAI_API_KEY")
            .expect("the request is built");
        assert_eq!(sent.value, VALUE);
        assert_eq!(sent.name, "OPENAI_API_KEY");
        assert!(!sent.replace, "an add never replaces");
        assert!(!form.has_value(), "the value is gone after it is sent");
        assert_eq!(
            form.request("OPENAI_API_KEY").unwrap_err(),
            "Paste the value first."
        );
    }

    #[test]
    fn an_invalid_name_keeps_the_value_for_the_next_try() {
        let mut form = SecretForm::add("global".to_owned());
        form.paste(VALUE);
        assert!(form.request("1ST").is_err());
        assert!(form.has_value(), "a refused name keeps the pasted value");
        assert!(!form.can_save("1ST", true));
        assert!(form.can_save("FIRST", true));
    }

    #[test]
    fn a_rotation_replaces_the_named_secret_in_its_scope() {
        let mut form = SecretForm::rotate("project:oppm".to_owned(), "DB_PASSWORD".to_owned());
        form.paste(VALUE);
        let sent = form.request("DB_PASSWORD").expect("the rotation is built");
        assert!(sent.replace);
        assert_eq!(sent.scope, "project:oppm");
    }

    #[test]
    fn nothing_can_be_saved_while_offline_or_while_a_save_is_in_flight() {
        let mut form = SecretForm::add("global".to_owned());
        form.paste(VALUE);
        assert!(!form.can_save("OPENAI_API_KEY", false));
        form.request("OPENAI_API_KEY")
            .expect("the request is built");
        form.paste(VALUE);
        assert!(
            !form.has_value(),
            "no value is taken while a save is in flight"
        );
        assert!(
            !form.can_save("OPENAI_API_KEY", true),
            "a save is in flight"
        );
    }

    #[test]
    fn an_empty_clipboard_is_refused_and_no_value_is_kept() {
        let mut form = SecretForm::add("global".to_owned());
        form.paste("");
        assert!(!form.has_value());
        assert!(matches!(form.status, FormStatus::Refused(_)));
    }

    #[test]
    fn the_form_never_prints_its_value() {
        let mut form = SecretForm::add("global".to_owned());
        form.paste(VALUE);
        let printed = format!("{form:?}");
        assert!(!printed.contains(VALUE), "{printed}");
        assert!(printed.contains("[redacted]"), "{printed}");
    }

    #[test]
    fn a_refused_save_says_what_is_wrong_and_that_nothing_was_stored() {
        assert_eq!(
            refusal_sentence(&remote(codes::SECRET_NAME_IN_USE), "OPENAI_API_KEY"),
            "Not saved: OPENAI_API_KEY already exists in this scope. Pick another name. Nothing was stored."
        );
        assert_eq!(
            refusal_sentence(&remote(codes::SECRET_VAULT_LOCKED), "A"),
            "Not saved: The vault is locked: its key will not open. Nothing was stored."
        );
        assert_eq!(
            refusal_sentence(
                &Refusal::unavailable("The service cannot be reached right now."),
                "A"
            ),
            "Not saved: The service cannot be reached right now. Nothing was stored."
        );
        assert_eq!(
            refusal_sentence(&remote("secret.something_new"), "A"),
            "Not saved: the service says no. Nothing was stored."
        );
    }

    #[test]
    fn a_delete_takes_the_links_only_when_the_confirm_named_them() {
        let mut alone = RemoveForm::new("global".into(), "OLD".into(), Vec::new());
        assert!(!alone.request().include_links);
        let mut shared =
            RemoveForm::new("global".into(), "SHARED".into(), vec!["project:a".into()]);
        assert!(shared.request().include_links);
        assert_eq!(shared.status, FormStatus::Saving);
    }

    #[test]
    fn the_delete_label_counts_the_linked_projects_and_teams() {
        assert_eq!(delete_label(&[]), "Delete secret");
        assert_eq!(
            delete_label(&["project:a".into(), "team:b".into()]),
            "Delete secret (1 project, 1 team)"
        );
        assert_eq!(
            delete_label(&["project:a".into(), "project:b".into()]),
            "Delete secret (2 projects)"
        );
    }

    #[test]
    fn a_refused_deletion_says_the_secret_is_still_there() {
        assert_eq!(
            removal_refusal_sentence(&remote(codes::SECRET_NOT_FOUND), "OLD"),
            "Not deleted: There is no secret OLD in this scope to delete. The secret is still there."
        );
        assert_eq!(
            removal_refusal_sentence(
                &Refusal::unavailable("The service cannot be reached right now."),
                "OLD"
            ),
            "Not deleted: The service cannot be reached right now. The secret is still there."
        );
    }

    #[test]
    fn an_unknown_refusal_keeps_the_service_text() {
        let refusal = Refusal::from(&massorca_client::ClientError::Remote(ErrorBody::new(
            "secret.other",
            "the service says no",
        )));
        assert!(matches!(refusal, Refusal::Failed { .. }));
    }
}
