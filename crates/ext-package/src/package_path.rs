//! Package-relative paths: the only form a manifest or an archive entry may use to name a file.

use crate::display_chars;

/// Checks a path that names one file in the package. Glob characters are refused unless `allow_glob` is set.
pub fn check(path: &str, allow_glob: bool) -> Result<(), &'static str> {
    if path.is_empty() {
        return Err("empty path");
    }
    if path.chars().any(char::is_control) {
        return Err("control character");
    }
    if path.chars().any(display_chars::is_format) {
        return Err("format character");
    }
    if path.contains('\\') {
        return Err("backslash separator");
    }
    if path.contains(':') {
        return Err("drive letter or stream name");
    }
    if path.starts_with('/') {
        return Err("absolute path");
    }
    if !allow_glob && path.contains(['*', '?', '[', ']', '{', '}']) {
        return Err("glob character");
    }
    for segment in path.split('/') {
        match segment {
            "" => return Err("empty segment"),
            "." => return Err("current-directory segment"),
            ".." => return Err("parent-directory segment"),
            _ => {}
        }
        windows_name_problem(segment).map_or(Ok(()), Err)?;
    }
    Ok(())
}

/// Refuses, on every OS, a name Windows would resolve to another file or a device: a trailing dot or space (dropped),
/// an 8.3 short name (`EXTENS~1.JSO`), or a reserved device name with any extension, including the superscript digits
/// (`COM¹`, `LPT²`). Package paths and ids both use this one rule.
pub fn windows_name_problem(segment: &str) -> Option<&'static str> {
    if segment.ends_with(['.', ' ']) {
        return Some("trailing dot or space");
    }
    if segment
        .as_bytes()
        .windows(2)
        .any(|pair| pair[0] == b'~' && pair[1].is_ascii_digit())
    {
        return Some("short-name alias");
    }
    let stem = segment
        .split('.')
        .next()
        .unwrap_or(segment)
        .trim_end_matches(' ')
        .to_ascii_lowercase();
    // Windows maps the superscript digits ¹ ² ³ to COM1..3 and LPT1..3, so they name devices too.
    let numbered = |prefix: &str| {
        stem.strip_prefix(prefix).is_some_and(|digit| {
            matches!(
                digit,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    };
    if matches!(
        stem.as_str(),
        "con" | "prn" | "aux" | "nul" | "conin$" | "conout$"
    ) || numbered("com")
        || numbered("lpt")
    {
        return Some("reserved device name");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{check, windows_name_problem};

    #[test]
    fn accepts_plain_relative_paths() {
        assert_eq!(check("views/wall.json", false), Ok(()));
        assert_eq!(check("main.js", false), Ok(()));
        assert_eq!(check("a.b/c-d_e.json", false), Ok(()));
    }

    #[test]
    fn refuses_unsafe_forms() {
        assert_eq!(check("", false), Err("empty path"));
        assert_eq!(check("/etc/passwd", false), Err("absolute path"));
        assert_eq!(check("../x.json", false), Err("parent-directory segment"));
        assert_eq!(check("a/../b", false), Err("parent-directory segment"));
        assert_eq!(check("a\\b.json", false), Err("backslash separator"));
        assert_eq!(
            check("C:/x.json", false),
            Err("drive letter or stream name")
        );
        assert_eq!(check("a//b", false), Err("empty segment"));
        assert_eq!(check("a/", false), Err("empty segment"));
        assert_eq!(check("./a", false), Err("current-directory segment"));
        assert_eq!(check("a\u{0}b", false), Err("control character"));
        assert_eq!(check("a*.json", false), Err("glob character"));
    }

    #[test]
    fn refuses_names_windows_would_alias_on_every_os() {
        assert_eq!(
            check("extension.json.", false),
            Err("trailing dot or space")
        );
        assert_eq!(
            check("extension.json ", false),
            Err("trailing dot or space")
        );
        assert_eq!(
            check("agents./kit.json", false),
            Err("trailing dot or space")
        );
        assert_eq!(check("EXTENS~1.JSO", false), Err("short-name alias"));
        assert_eq!(check("a/b~9/c", true), Err("short-name alias"));
        for reserved in [
            "con", "CON.json", "nul.txt", "Aux", "prn.a.b", "com1", "LPT9.log", "conin$",
        ] {
            assert_eq!(
                check(&format!("media/{reserved}"), false),
                Err("reserved device name"),
                "{reserved}"
            );
        }
        assert_eq!(check("console.json", false), Ok(()));
        assert_eq!(check("com10.json", false), Ok(()));
        assert_eq!(check("a~b.json", false), Ok(()));
    }

    #[test]
    fn refuses_superscript_device_names() {
        for reserved in ["COM¹", "com²", "LPT³.txt", "lpt¹.log"] {
            assert_eq!(
                check(&format!("media/{reserved}"), false),
                Err("reserved device name"),
                "{reserved}"
            );
            assert!(windows_name_problem(reserved).is_some(), "{reserved}");
        }
        assert_eq!(check("COM⁴.json", false), Ok(()));
    }

    #[test]
    fn refuses_format_characters_in_paths() {
        for hidden in ["a\u{202E}b.json", "a\u{200B}.json", "a\u{FEFF}.json"] {
            assert_eq!(check(hidden, false), Err("format character"), "{hidden:?}");
        }
    }

    #[test]
    fn globs_are_allowed_only_when_asked() {
        assert_eq!(check("src/**/*.md", true), Ok(()));
        assert_eq!(check("../*", true), Err("parent-directory segment"));
    }
}
