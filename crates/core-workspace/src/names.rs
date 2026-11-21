//! Name rules for projects and workspaces (ADR 0008).

use massorca_protocol::codes;

use crate::error::DomainError;

/// The longest workspace name; it is also a branch name and a directory name.
const MAX_WORKSPACE_NAME: usize = 64;

/// A workspace name: `[A-Za-z0-9._-]`, 1 to 64 characters, no leading `.` or `-`, and no
/// trailing `.`, which Windows silently drops from directory names.
pub(crate) fn workspace_name(name: &str) -> Result<(), DomainError> {
    let allowed = |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-');
    let well_formed = !name.is_empty()
        && name.len() <= MAX_WORKSPACE_NAME
        && name.bytes().all(allowed)
        && !name.starts_with(['.', '-'])
        && !name.ends_with('.');
    if well_formed {
        Ok(())
    } else {
        Err(DomainError::new(
            codes::WORKSPACE_NAME_INVALID,
            format!(
                "workspace name {name:?} must be 1 to {MAX_WORKSPACE_NAME} of [A-Za-z0-9._-], \
                 not start with '.' or '-', and not end with '.'"
            ),
        ))
    }
}

/// A project name: not blank, no path separator, and not `.` or `..`, because a default
/// workspaces root is built from it.
pub(crate) fn project_name(name: &str) -> Result<(), DomainError> {
    let well_formed =
        !name.trim().is_empty() && !name.contains(['/', '\\']) && name != "." && name != "..";
    if well_formed {
        Ok(())
    } else {
        Err(DomainError::new(
            codes::REQUEST_INVALID_PARAMS,
            format!(
                "project name {name:?} must not be blank, contain a path separator, or be . or .."
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use massorca_protocol::codes;

    use super::{project_name, workspace_name};

    #[test]
    fn workspace_names_allow_letters_digits_dots_dashes_and_underscores_up_to_64() {
        assert!(workspace_name("feature-1.x_2").is_ok());
        assert!(workspace_name(&"a".repeat(64)).is_ok());
        assert!(workspace_name(&"a".repeat(65)).is_err());
    }

    #[test]
    fn workspace_names_refuse_other_characters_and_leading_or_trailing_dots() {
        for name in ["", "-x", ".x", "x.", "a b", "a/b"] {
            let error = workspace_name(name).expect_err(name);
            assert_eq!(error.code(), codes::WORKSPACE_NAME_INVALID, "{name:?}");
        }
    }

    #[test]
    fn project_names_refuse_blank_separators_and_dot_names() {
        for name in ["", "  ", "a/b", "a\\b", ".", ".."] {
            let error = project_name(name).expect_err(name);
            assert_eq!(error.code(), codes::REQUEST_INVALID_PARAMS, "{name:?}");
        }
        assert!(project_name("repo").is_ok());
    }
}
