use std::fmt;

/// A git release, such as 2.49.0 from `git version 2.49.0.windows.1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GitVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl GitVersion {
    /// Parses the line printed by `git --version`; a missing patch number counts as 0.
    pub(crate) fn parse_version_output(text: &str) -> Option<Self> {
        let rest = text.trim().strip_prefix("git version ")?;
        let mut numbers = rest.split('.').map(leading_number);
        let major = numbers.next()??;
        let minor = numbers.next()??;
        let patch = numbers.next().unwrap_or(Some(0))?;
        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

/// The digits at the start of one dotted part, so `0.windows` gives 0.
fn leading_number(part: &str) -> Option<u32> {
    let end = part
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(part.len());
    part[..end].parse().ok()
}

impl fmt::Display for GitVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[cfg(test)]
mod tests {
    use super::GitVersion;

    fn version(major: u32, minor: u32, patch: u32) -> GitVersion {
        GitVersion {
            major,
            minor,
            patch,
        }
    }

    #[test]
    fn parses_the_windows_build_suffix() {
        assert_eq!(
            GitVersion::parse_version_output("git version 2.49.0.windows.1\n"),
            Some(version(2, 49, 0))
        );
    }

    #[test]
    fn parses_the_apple_build_suffix() {
        assert_eq!(
            GitVersion::parse_version_output("git version 2.39.2 (Apple Git-143)"),
            Some(version(2, 39, 2))
        );
    }

    #[test]
    fn a_missing_patch_number_counts_as_zero() {
        assert_eq!(
            GitVersion::parse_version_output("git version 2.25"),
            Some(version(2, 25, 0))
        );
    }

    #[test]
    fn other_output_is_not_a_version() {
        for text in [
            "",
            "hello",
            "git version ",
            "git version x.1",
            "git version 2..1",
        ] {
            assert_eq!(GitVersion::parse_version_output(text), None, "{text:?}");
        }
    }

    #[test]
    fn versions_order_by_release() {
        assert!(version(2, 25, 0) < version(2, 49, 0));
        assert!(version(2, 49, 0) < version(3, 0, 0));
        assert!(version(2, 9, 9) < version(2, 10, 0));
    }

    #[test]
    fn displays_as_dotted_numbers() {
        assert_eq!(version(2, 25, 1).to_string(), "2.25.1");
    }
}
