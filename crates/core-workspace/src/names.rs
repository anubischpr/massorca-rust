//! Name rules for projects and workspaces (ADR 0008).

use icu_normalizer::DecomposingNormalizer;
use icu_normalizer::properties::CanonicalCombiningClassMapBorrowed;
use massorca_protocol::codes;

use crate::error::DomainError;

/// The longest workspace name; it is also a branch name and a directory name.
const MAX_WORKSPACE_NAME: usize = 64;

/// The names Windows reserves as devices, with or without an extension, in any case.
const WINDOWS_DEVICE_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Whether `name` is a Windows device name, which no file or directory may take there.
fn is_windows_device_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or_default();
    WINDOWS_DEVICE_NAMES
        .iter()
        .any(|device| stem.eq_ignore_ascii_case(device))
}

/// A workspace name: `[A-Za-z0-9._-]`, 1 to 64 characters, no leading `.` or `-`, no trailing
/// `.` (Windows drops it from directory names), and no Windows device name, because the name is
/// also a directory.
pub(crate) fn workspace_name(name: &str) -> Result<(), DomainError> {
    let allowed = |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-');
    let well_formed = !name.is_empty()
        && name.len() <= MAX_WORKSPACE_NAME
        && name.bytes().all(allowed)
        && !name.starts_with(['.', '-'])
        && !name.ends_with('.')
        && !is_windows_device_name(name);
    if well_formed {
        Ok(())
    } else {
        Err(DomainError::new(
            codes::WORKSPACE_NAME_INVALID,
            format!(
                "workspace name {name:?} must be 1 to {MAX_WORKSPACE_NAME} of [A-Za-z0-9._-],                  not start with '.' or '-', not end with '.', and not be a Windows device name"
            ),
        ))
    }
}

/// A project name: not blank, no leading `.` (so it is never read as a relative path selector,
/// and `.` and `..` are refused with it), no path separator, colon, or shell wildcard or pipe, no
/// trailing dot or space, and no Windows device name, because a default workspaces root is built
/// from it.
pub(crate) fn project_name(name: &str) -> Result<(), DomainError> {
    let well_formed = !name.trim().is_empty()
        && !name.starts_with('.')
        && !name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
        && !name.ends_with(['.', ' '])
        && !is_windows_device_name(name);
    if well_formed {
        Ok(())
    } else {
        Err(DomainError::new(
            codes::REQUEST_INVALID_PARAMS,
            format!(
                "project name {name:?} must not be blank, start with '.', contain a path separator, colon, or any of * ? \" < > |, end with a dot or space, or be a Windows device name"
            ),
        ))
    }
}

/// The stored form of a typed workspace name: accents folded, other characters cleaned to
/// `[A-Za-z0-9._-]`, then validated. Refuses only what cleaning cannot make valid.
pub(crate) fn clean_workspace_name(typed: &str) -> Result<String, DomainError> {
    let mut cleaned = String::with_capacity(typed.len());
    for ch in fold_accents(typed).chars() {
        let ch = if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            ch
        } else {
            '-'
        };
        // Why: git refuses `..` in a branch name, and a run of separators is only noise.
        if matches!(ch, '-' | '.') && cleaned.ends_with(ch) {
            continue;
        }
        cleaned.push(ch);
    }
    let cleaned = cleaned.trim_matches(['.', '-']).to_owned();
    workspace_name(&cleaned)?;
    Ok(cleaned)
}

/// Decomposes the text and drops its combining marks, so `é` keeps its `e`.
fn fold_accents(typed: &str) -> String {
    let combining = CanonicalCombiningClassMapBorrowed::new();
    let mut folded = String::with_capacity(typed.len());
    for ch in DecomposingNormalizer::new_nfd().normalize(typed).chars() {
        // Why: a non-zero canonical combining class is what marks a combining accent.
        if combining.get_u8(ch) != 0 {
            continue;
        }
        match fold_letter(ch) {
            Some(spelling) => folded.push_str(spelling),
            None => folded.push(ch),
        }
    }
    folded
}

/// Letters with no decomposition, spelt the way Latin text spells them.
fn fold_letter(ch: char) -> Option<&'static str> {
    Some(match ch {
        'ß' => "ss",
        'Æ' => "AE",
        'æ' => "ae",
        'Œ' => "OE",
        'œ' => "oe",
        'Ø' => "O",
        'ø' => "o",
        'Ł' => "L",
        'ł' => "l",
        'Đ' => "D",
        'đ' => "d",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use massorca_protocol::codes;

    use super::{clean_workspace_name, project_name, workspace_name};

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
    fn workspace_names_refuse_windows_device_names_with_or_without_an_extension() {
        for name in ["CON", "nul", "Com1", "lpt9", "AUX.txt", "prn.log"] {
            let error = workspace_name(name).expect_err(name);
            assert_eq!(error.code(), codes::WORKSPACE_NAME_INVALID, "{name:?}");
        }
        assert!(workspace_name("console").is_ok());
        assert!(workspace_name("com10").is_ok());
    }

    #[test]
    fn project_names_refuse_drive_colons_device_names_and_trailing_dots_or_spaces() {
        for name in ["C:", "a:b", "CON", "NUL.txt", "repo.", "repo "] {
            let error = project_name(name).expect_err(name);
            assert_eq!(error.code(), codes::REQUEST_INVALID_PARAMS, "{name:?}");
        }
        assert!(project_name("my-repo").is_ok());
    }

    #[test]
    fn project_names_refuse_leading_dots_and_shell_wildcards_or_pipes() {
        for name in [".hidden", "..x", "a*b", "a?b", "a\"b", "a<b", "a>b", "a|b"] {
            let error = project_name(name).expect_err(name);
            assert_eq!(error.code(), codes::REQUEST_INVALID_PARAMS, "{name:?}");
        }
        assert!(project_name("my.repo").is_ok());
    }

    #[test]
    fn a_leading_dot_is_refused_on_project_and_workspace_names_alike() {
        for name in [".x", "..x"] {
            let project = project_name(name).expect_err(name);
            assert_eq!(project.code(), codes::REQUEST_INVALID_PARAMS, "{name:?}");
            let workspace = workspace_name(name).expect_err(name);
            assert_eq!(workspace.code(), codes::WORKSPACE_NAME_INVALID, "{name:?}");
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

    #[test]
    fn typed_names_are_cleaned_to_the_allowed_characters_with_accents_folded() {
        for (typed, stored) in [
            ("Caf\u{e9} L\u{f6}we", "Cafe-Lowe"),
            ("  feature / login  ", "feature-login"),
            ("Cr\u{e8}me Br\u{fb}l\u{e9}e", "Creme-Brulee"),
            ("\u{c5}ngstr\u{f6}m", "Angstrom"),
            ("Stra\u{df}e", "Strasse"),
            ("-leading", "leading"),
            ("trailing.", "trailing"),
            ("a..b", "a.b"),
            ("a---b", "a-b"),
            ("back\\slash", "back-slash"),
        ] {
            assert_eq!(
                clean_workspace_name(typed).expect(typed),
                stored,
                "{typed:?}"
            );
        }
    }

    #[test]
    fn a_valid_name_is_stored_unchanged() {
        for name in ["feature-1.x_2", "A9", "a", "x_y.z"] {
            assert_eq!(clean_workspace_name(name).expect(name), name);
        }
        let longest = "x".repeat(64);
        assert_eq!(
            clean_workspace_name(&longest).expect("64 characters"),
            longest
        );
    }

    #[test]
    fn a_name_with_nothing_to_keep_or_that_stays_too_long_is_still_refused() {
        let too_long = "x".repeat(65);
        for name in [
            "",
            "   ",
            "***",
            "..",
            "\u{5de5}\u{4f5c}",
            "\u{1f680}",
            "CON",
            "con.txt",
            too_long.as_str(),
        ] {
            let error = clean_workspace_name(name).expect_err(name);
            assert_eq!(error.code(), codes::WORKSPACE_NAME_INVALID, "{name:?}");
        }
    }
}
