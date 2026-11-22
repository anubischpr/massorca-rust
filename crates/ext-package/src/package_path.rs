//! Package-relative paths: the only form a manifest or an archive entry may use to name a file.

/// Checks a path that names one file in the package. Glob characters are refused unless `allow_glob` is set.
pub fn check(path: &str, allow_glob: bool) -> Result<(), &'static str> {
    if path.is_empty() {
        return Err("empty path");
    }
    if path.chars().any(char::is_control) {
        return Err("control character");
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
        check_windows_alias(segment)?;
    }
    Ok(())
}

/// Refuses, on every OS, a name Windows would resolve to another file or a device: a trailing dot or space (dropped),
/// an 8.3 short name (`EXTENS~1.JSO`), or a reserved device name with any extension.
fn check_windows_alias(segment: &str) -> Result<(), &'static str> {
    if segment.ends_with(['.', ' ']) {
        return Err("trailing dot or space");
    }
    let bytes = segment.as_bytes();
    if bytes
        .windows(2)
        .any(|pair| pair[0] == b'~' && pair[1].is_ascii_digit())
    {
        return Err("short-name alias");
    }
    let stem = segment
        .split('.')
        .next()
        .unwrap_or(segment)
        .trim_end_matches(' ')
        .to_ascii_lowercase();
    let numbered = |prefix: &str| {
        stem.strip_prefix(prefix)
            .is_some_and(|digit| matches!(digit.as_bytes(), [b'1'..=b'9']))
    };
    if matches!(
        stem.as_str(),
        "con" | "prn" | "aux" | "nul" | "conin$" | "conout$"
    ) || numbered("com")
        || numbered("lpt")
    {
        return Err("reserved device name");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::check;

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
    fn globs_are_allowed_only_when_asked() {
        assert_eq!(check("src/**/*.md", true), Ok(()));
        assert_eq!(check("../*", true), Err("parent-directory segment"));
    }
}
