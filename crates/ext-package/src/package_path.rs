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
    fn globs_are_allowed_only_when_asked() {
        assert_eq!(check("src/**/*.md", true), Ok(()));
        assert_eq!(check("../*", true), Err("parent-directory segment"));
    }
}
