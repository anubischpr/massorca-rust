//! Variable names a secret may never take (decision 0071): names that load code, redirect lookups, or swap a
//! provider's endpoint. The vault refuses them when a secret is set, linked or requested, and the agents domain again at
//! launch. Names compare without case, as Windows does.

/// Exact names.
const NAMES: &[&str] = &[
    "PATH",
    "PATHEXT",
    "COMSPEC",
    "HOME",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "TMP",
    "TEMP",
    "TMPDIR",
    "SHELL",
    "ENV",
    "BASH_ENV",
    "NODE_OPTIONS",
    "NODE_PATH",
    "RUBYOPT",
    "PERL5OPT",
    "JAVA_TOOL_OPTIONS",
    // Provider endpoints and routing of the built-in kinds.
    "OPENAI_API_BASE",
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_USE_FOUNDRY",
];

/// Prefixes: the dynamic loader, Python, git and ssh families.
const PREFIXES: &[&str] = &["LD_", "DYLD_", "PYTHON", "GIT_", "SSH_"];

/// Suffixes: any provider's base URL, such as `ANTHROPIC_BASE_URL` or `OPENAI_BASE_URL`.
const SUFFIXES: &[&str] = &["_BASE_URL"];

/// Whether a secret may not be named `name`.
#[must_use]
pub fn is_dangerous_env_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    NAMES.contains(&upper.as_str())
        || PREFIXES.iter().any(|prefix| upper.starts_with(prefix))
        || SUFFIXES.iter().any(|suffix| upper.ends_with(suffix))
}

#[cfg(test)]
mod tests {
    use super::is_dangerous_env_name;

    #[test]
    fn the_dangerous_names_are_refused_in_any_case() {
        for (name, dangerous) in [
            ("PATH", true),
            ("path", true),
            ("PathExt", true),
            ("COMSPEC", true),
            ("HOME", true),
            ("USERPROFILE", true),
            ("APPDATA", true),
            ("LOCALAPPDATA", true),
            ("TMP", true),
            ("TEMP", true),
            ("TMPDIR", true),
            ("SHELL", true),
            ("ENV", true),
            ("BASH_ENV", true),
            ("LD_PRELOAD", true),
            ("ld_library_path", true),
            ("DYLD_INSERT_LIBRARIES", true),
            ("NODE_OPTIONS", true),
            ("NODE_PATH", true),
            ("PYTHONPATH", true),
            ("PYTHONSTARTUP", true),
            ("RUBYOPT", true),
            ("PERL5OPT", true),
            ("JAVA_TOOL_OPTIONS", true),
            ("GIT_SSH_COMMAND", true),
            ("GIT_CONFIG_GLOBAL", true),
            ("GIT_EXEC_PATH", true),
            ("GIT_ASKPASS", true),
            ("SSH_AUTH_SOCK", true),
            ("ANTHROPIC_BASE_URL", true),
            ("OPENAI_BASE_URL", true),
            ("anthropic_bedrock_base_url", true),
            ("OPENAI_API_BASE", true),
            ("CLAUDE_CODE_USE_BEDROCK", true),
            ("OPENAI_API_KEY", false),
            ("ANTHROPIC_API_KEY", false),
            ("GITHUB_TOKEN", false),
            ("DATABASE_URL", false),
            ("HOMEBREW_PREFIX", false),
            ("MY_PATH", false),
            ("TEMPLATE_DIR", false),
        ] {
            assert_eq!(is_dangerous_env_name(name), dangerous, "{name}");
        }
    }
}
