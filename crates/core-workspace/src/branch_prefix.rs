//! The branch prefix of a new workspace: none, the git username, or custom text (ADR 0008).

use std::path::Path;

use massorca_core_git::Git;
use massorca_protocol::codes;

use crate::error::DomainError;

/// Where the prefix of a new workspace's branch comes from (`workspaces.branch_prefix`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BranchPrefix {
    /// No prefix: the branch is the workspace name.
    #[default]
    None,
    /// The repository's git username, when git accepts it as a branch segment.
    GitUsername,
    /// The text of `workspaces.branch_prefix_custom`.
    Custom(String),
}

impl BranchPrefix {
    /// The prefix a setting names: `none`, `git-username`, or `custom` with `custom_text`.
    pub fn from_setting(source: &str, custom_text: &str) -> Result<Self, DomainError> {
        match source {
            "none" => Ok(Self::None),
            "git-username" => Ok(Self::GitUsername),
            "custom" => Ok(Self::Custom(custom_text.to_owned())),
            other => Err(DomainError::new(
                codes::CONFIG_INVALID_VALUE,
                format!("branch prefix source {other:?} is not none, git-username or custom"),
            )),
        }
    }

    /// The segment before the `/` of a new branch created in `repo`; `None` means no prefix.
    pub(crate) fn segment(&self, git: &Git, repo: &Path) -> Result<Option<String>, DomainError> {
        match self {
            Self::None => Ok(None),
            Self::GitUsername => git_username(git, repo),
            Self::Custom(text) => {
                let segment = normalised_prefix(text);
                if segment.is_empty() {
                    Ok(None)
                } else if prefix_is_valid(&segment) {
                    Ok(Some(segment))
                } else {
                    Err(DomainError::new(
                        codes::CONFIG_INVALID_VALUE,
                        format!(
                            "workspaces.branch_prefix_custom {segment:?} is not a valid branch prefix"
                        ),
                    ))
                }
            }
        }
    }
}

/// The branch of a workspace named `name`, under `segment` when there is one.
pub(crate) fn branch_name(segment: Option<&str>, name: &str) -> String {
    match segment {
        Some(segment) => format!("{segment}/{name}"),
        None => name.to_owned(),
    }
}

/// The text without surrounding whitespace and slashes, with runs of slashes collapsed.
fn normalised_prefix(text: &str) -> String {
    text.trim()
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

/// Git's `check-ref-format` rules for a prefix, which may hold several `/` segments.
fn prefix_is_valid(prefix: &str) -> bool {
    !prefix
        .chars()
        .any(|ch| ch.is_ascii_control() || ch == ' ' || "~^:?*[\\".contains(ch))
        && !prefix.contains("..")
        && !prefix.contains("@{")
        && !prefix.starts_with('-')
        && !prefix.ends_with('.')
        && prefix
            .split('/')
            .all(|segment| !segment.starts_with('.') && !segment.ends_with(".lock"))
}

/// The username git config names for the repository: an explicit login key first, then a
/// `user.name` that is a plain token.
fn git_username(git: &Git, repo: &Path) -> Result<Option<String>, DomainError> {
    for key in ["github.user", "user.username"] {
        if let Some(login) = git
            .config_get(repo, key)?
            .and_then(|raw| configured_login(&raw))
        {
            return Ok(Some(login));
        }
    }
    Ok(git
        .config_get(repo, "user.name")?
        .map(|name| name.trim().to_owned())
        .filter(|name| is_branch_safe(name)))
}

/// What a configured login names, if a branch can carry it.
fn configured_login(raw: &str) -> Option<String> {
    let local = raw.trim().split('@').next().unwrap_or_default();
    let login = match local.split_once('+') {
        Some((digits, rest))
            if !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            rest
        }
        _ => local,
    };
    is_branch_safe(login).then(|| login.to_owned())
}

/// Whether git accepts `value` as one branch segment: the login rule of the reference app, plus
/// the refusal of a `.lock` ending that git makes.
fn is_branch_safe(value: &str) -> bool {
    value.len() <= 255
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        && !value.contains("..")
        && !value.ends_with('.')
        && !value.ends_with(".lock")
}

#[cfg(test)]
mod tests {
    use massorca_protocol::codes;

    use super::{BranchPrefix, configured_login, normalised_prefix, prefix_is_valid};

    #[test]
    fn a_custom_prefix_is_normalised_to_one_separator_between_its_segments() {
        assert_eq!(normalised_prefix("  team//frontend/ "), "team/frontend");
        assert_eq!(normalised_prefix(" / "), "");
    }

    #[test]
    fn a_prefix_git_refuses_is_not_valid_and_a_plain_one_is() {
        for prefix in [
            "bad..x", "a b", "-x", "x.", "x/.y", "x/y.lock", "a@{b", "a:b", "a\\b", "a?b", "a~b",
        ] {
            assert!(!prefix_is_valid(prefix), "{prefix:?}");
        }
        for prefix in ["team", "team/frontend", "a.b-c_d", "x.y"] {
            assert!(prefix_is_valid(prefix), "{prefix:?}");
        }
    }

    #[test]
    fn a_configured_login_is_its_local_part_when_a_branch_can_carry_it() {
        assert_eq!(
            configured_login("octo-user\n").as_deref(),
            Some("octo-user")
        );
        assert_eq!(
            configured_login("12345+octo-user@users.noreply.example.invalid").as_deref(),
            Some("octo-user")
        );
        assert_eq!(configured_login("Test Person"), None);
        assert_eq!(configured_login("   "), None);
        assert_eq!(configured_login("octo-user.lock"), None);
    }

    #[test]
    fn a_setting_names_its_source_and_an_unknown_source_is_refused() {
        assert_eq!(
            BranchPrefix::from_setting("none", "").expect("none"),
            BranchPrefix::None
        );
        assert_eq!(
            BranchPrefix::from_setting("git-username", "").expect("git-username"),
            BranchPrefix::GitUsername
        );
        assert_eq!(
            BranchPrefix::from_setting("custom", "team").expect("custom"),
            BranchPrefix::Custom("team".to_owned())
        );
        let error = BranchPrefix::from_setting("bogus", "").expect_err("bogus");
        assert_eq!(error.code(), codes::CONFIG_INVALID_VALUE);
    }
}
