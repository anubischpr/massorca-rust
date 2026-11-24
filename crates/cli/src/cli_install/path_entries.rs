//! Reads and edits a PATH value: whether it names a folder, and how it changes when a folder is added or removed.

use std::path::Path;

use super::layout::{Platform, same_path};

/// Whether `value` names `dir`, after Windows variables such as `%LOCALAPPDATA%` are expanded.
pub fn contains(
    platform: Platform,
    value: &str,
    dir: &Path,
    variables: &[(String, String)],
) -> bool {
    entries(platform, value).any(|entry| entry_is(platform, entry, dir, variables))
}

/// `value` with `dir` appended; every other entry keeps its text and its place.
pub fn with_entry(platform: Platform, value: &str, dir: &Path) -> String {
    let separator = separator(platform);
    let base = value.trim_end_matches(separator);
    if base.trim().is_empty() {
        return dir.display().to_string();
    }
    format!("{base}{separator}{}", dir.display())
}

/// `value` without the entries that name `dir`; the same text when none does, so callers can tell nothing changed.
pub fn without_entry(
    platform: Platform,
    value: &str,
    dir: &Path,
    variables: &[(String, String)],
) -> String {
    if !contains(platform, value, dir, variables) {
        return value.to_owned();
    }
    value
        .split(separator(platform))
        .map(str::trim)
        .filter(|entry| !entry.is_empty() && !entry_is(platform, entry, dir, variables))
        .collect::<Vec<_>>()
        .join(separator(platform))
}

/// `text` with each `%NAME%` replaced by its value; unknown names stay as they are.
pub fn expand_variables(text: &str, variables: &[(String, String)]) -> String {
    let mut expanded = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('%') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('%') else {
            break;
        };
        let name = &after[..end];
        expanded.push_str(&rest[..start]);
        match variables
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
        {
            Some((_, value)) if !name.is_empty() => expanded.push_str(value),
            _ => expanded.push_str(&rest[start..start + end + 2]),
        }
        rest = &after[end + 1..];
    }
    expanded.push_str(rest);
    expanded
}

fn separator(platform: Platform) -> &'static str {
    match platform {
        Platform::Unix => ":",
        Platform::Windows => ";",
    }
}

fn entries(platform: Platform, value: &str) -> impl Iterator<Item = &str> {
    value
        .split(separator(platform))
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
}

fn entry_is(platform: Platform, entry: &str, dir: &Path, variables: &[(String, String)]) -> bool {
    match platform {
        Platform::Unix => {
            entry.trim_end_matches('/') == dir.to_string_lossy().trim_end_matches('/')
        }
        Platform::Windows => same_path(
            platform,
            Path::new(&expand_variables(entry, variables)),
            dir,
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::cli_install::layout::Platform;

    fn variables() -> Vec<(String, String)> {
        vec![(
            "LOCALAPPDATA".to_owned(),
            r"C:\Users\ana\AppData\Local".to_owned(),
        )]
    }

    #[test]
    fn contains_finds_the_folder_after_expanding_windows_variables() {
        let dir = Path::new(r"C:\Users\ana\AppData\Local\Programs\MassOrca\bin");
        let value = r"C:\Windows;%LOCALAPPDATA%\Programs\MassOrca\bin";
        assert!(contains(Platform::Windows, value, dir, &variables()));
        assert!(!contains(
            Platform::Windows,
            r"C:\Windows",
            dir,
            &variables()
        ));
    }

    #[test]
    fn contains_ignores_a_trailing_separator_on_the_entry() {
        assert!(contains(
            Platform::Unix,
            "/usr/bin:/home/ana/.local/bin/",
            Path::new("/home/ana/.local/bin"),
            &[],
        ));
        assert!(contains(
            Platform::Windows,
            r"C:\Apps\bin\",
            Path::new(r"C:\Apps\bin"),
            &[],
        ));
    }

    #[test]
    fn with_entry_appends_the_folder_and_keeps_the_other_entries_in_order() {
        assert_eq!(
            with_entry(
                Platform::Windows,
                r"C:\Windows;C:\Tools",
                Path::new(r"C:\Apps\bin")
            ),
            r"C:\Windows;C:\Tools;C:\Apps\bin",
        );
        assert_eq!(
            with_entry(
                Platform::Unix,
                "/usr/bin:/bin",
                Path::new("/home/ana/.local/bin")
            ),
            "/usr/bin:/bin:/home/ana/.local/bin",
        );
    }

    #[test]
    fn with_entry_on_an_empty_value_is_only_the_folder() {
        assert_eq!(
            with_entry(Platform::Windows, "", Path::new(r"C:\Apps\bin")),
            r"C:\Apps\bin"
        );
    }

    #[test]
    fn without_entry_removes_only_the_folder_and_keeps_the_text_when_it_is_absent() {
        let dir = Path::new(r"C:\Apps\bin");
        assert_eq!(
            without_entry(
                Platform::Windows,
                r"C:\Windows;C:\Apps\BIN\;C:\Tools",
                dir,
                &[]
            ),
            r"C:\Windows;C:\Tools",
        );
        assert_eq!(
            without_entry(Platform::Windows, r"C:\Windows;C:\Tools", dir, &[]),
            r"C:\Windows;C:\Tools",
        );
    }

    #[test]
    fn expand_variables_replaces_known_names_ignoring_case_and_keeps_unknown_ones() {
        assert_eq!(
            expand_variables(r"%localappdata%\x;%NOPE%", &variables()),
            r"C:\Users\ana\AppData\Local\x;%NOPE%",
        );
        assert_eq!(expand_variables("100% sure", &variables()), "100% sure");
    }
}
