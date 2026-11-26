//! The git credential guard of an agent run's terminal (TERM-24). Git inside an unattended agent fails at a credential
//! prompt instead of waiting for one. The settings are pv96's `gitCredentialPromptGuardEnv`, applied to the finished
//! environment; a user terminal never gets them.

use std::collections::BTreeMap;

use crate::EnvValue;
use crate::shell::{EnvMap, key, names_match, set_var};

/// The variable a run's launch plan sets to ask for the guard. The terminal applies the guard and never passes it on.
pub const CREDENTIAL_GUARD_ENV: &str = "MASSORCA_INTERNAL_GIT_CREDENTIAL_GUARD";

/// The value of [`CREDENTIAL_GUARD_ENV`] that asks for the guard.
pub const CREDENTIAL_GUARD_ON: &str = "guard";

/// Whether the variables a terminal is created with ask for the guard.
pub fn requested(extra: &BTreeMap<String, EnvValue>) -> bool {
    extra.iter().any(|(name, value)| {
        names_match(name, CREDENTIAL_GUARD_ENV) && value.as_str() == CREDENTIAL_GUARD_ON
    })
}

/// pv96's `gitCredentialPromptGuardEnv` over the finished environment. A configured askpass helper stays; the git
/// config protocol is extended only when it is unambiguous (TERM-24).
pub(crate) fn apply(vars: &mut EnvMap) {
    set_var(vars, "GIT_TERMINAL_PROMPT", "0");
    set_var(vars, "GCM_INTERACTIVE", "never");
    for name in ["GIT_ASKPASS", "SSH_ASKPASS"] {
        if !vars.contains_key(&key(name)) {
            set_var(vars, name, "");
        }
    }
    append_git_config(
        vars,
        &[
            ("credential.interactive", "false"),
            ("credential.guiPrompt", "false"),
        ],
    );
}

fn append_git_config(vars: &mut EnvMap, entries: &[(&str, &str)]) {
    let Some(base) = protocol_count(vars) else {
        return;
    };
    for (offset, &(name, value)) in entries.iter().enumerate() {
        let index = base + offset;
        set_var(vars, &format!("GIT_CONFIG_KEY_{index}"), name);
        set_var(vars, &format!("GIT_CONFIG_VALUE_{index}"), value);
    }
    set_var(
        vars,
        "GIT_CONFIG_COUNT",
        &(base + entries.len()).to_string(),
    );
}

/// The entry count of git's indexed config protocol when it is unambiguous; `None` when it is malformed, so that
/// nothing may be appended (pv96 `readValidGitConfigEnvCount`).
fn protocol_count(vars: &EnvMap) -> Option<usize> {
    let indexed = vars
        .keys()
        .filter(|name| name.starts_with("GIT_CONFIG_KEY_") || name.starts_with("GIT_CONFIG_VALUE_"))
        .count();
    let Some((_, raw)) = vars.get(&key("GIT_CONFIG_COUNT")) else {
        return (indexed == 0).then_some(0);
    };
    let raw = raw.as_str();
    let canonical = raw == "0"
        || (!raw.is_empty()
            && !raw.starts_with('0')
            && raw.bytes().all(|byte| byte.is_ascii_digit()));
    if !canonical {
        return None;
    }
    let count: usize = raw.parse().ok()?;
    if indexed != count.checked_mul(2)? {
        return None;
    }
    (0..count)
        .all(|index| {
            vars.contains_key(&key(&format!("GIT_CONFIG_KEY_{index}")))
                && vars.contains_key(&key(&format!("GIT_CONFIG_VALUE_{index}")))
        })
        .then_some(count)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{CREDENTIAL_GUARD_ENV, CREDENTIAL_GUARD_ON, requested};
    use crate::EnvValue;
    use crate::shell::{EnvSnapshot, TerminalEnv, build_environment};

    /// The environment a terminal starts with: `base` inherited, `extra` added last.
    fn start(base: &[(&str, &str)], extra: &[(&str, &str)]) -> Vec<(String, String)> {
        let base = EnvSnapshot::from_pairs(base.iter().copied());
        let extra: BTreeMap<String, EnvValue> = extra
            .iter()
            .map(|(name, value)| ((*name).to_owned(), EnvValue::new((*value).to_owned())))
            .collect();
        let terminal = TerminalEnv {
            terminal_id: "terminal-1",
            workspace_id: "workspace-1",
            service_version: "0.1.0",
            massorca_home: None,
            massorca_profile: None,
            extra: &extra,
        };
        build_environment(&base, &terminal)
            .into_iter()
            .map(|(name, value)| (name, value.as_str().to_owned()))
            .collect()
    }

    /// The environment of an agent run's terminal: `extra` plus the guard request the launch plan adds.
    fn guarded(base: &[(&str, &str)], extra: &[(&str, &str)]) -> Vec<(String, String)> {
        let mut requested_extra = extra.to_vec();
        requested_extra.push((CREDENTIAL_GUARD_ENV, CREDENTIAL_GUARD_ON));
        start(base, &requested_extra)
    }

    fn var(env: &[(String, String)], name: &str) -> Option<String> {
        env.iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    }

    #[test]
    fn an_agent_run_terminal_switches_git_prompts_off_over_inherited_values() {
        let env = guarded(
            &[("GIT_TERMINAL_PROMPT", "1"), ("GCM_INTERACTIVE", "always")],
            &[],
        );
        assert_eq!(var(&env, "GIT_TERMINAL_PROMPT").as_deref(), Some("0"));
        assert_eq!(var(&env, "GCM_INTERACTIVE").as_deref(), Some("never"));
        assert_eq!(
            var(&env, CREDENTIAL_GUARD_ENV),
            None,
            "the request never reaches the program"
        );
    }

    #[test]
    fn the_guard_wins_over_a_caller_value_for_git_prompts() {
        let env = guarded(&[], &[("GIT_TERMINAL_PROMPT", "1")]);
        assert_eq!(var(&env, "GIT_TERMINAL_PROMPT").as_deref(), Some("0"));
    }

    #[test]
    fn askpass_helpers_are_blanked_unless_one_is_configured() {
        let blank = guarded(&[], &[]);
        assert_eq!(var(&blank, "GIT_ASKPASS").as_deref(), Some(""));
        assert_eq!(var(&blank, "SSH_ASKPASS").as_deref(), Some(""));

        let configured = guarded(
            &[("GIT_ASKPASS", "/opt/helper")],
            &[("SSH_ASKPASS", "/opt/ssh-helper")],
        );
        assert_eq!(
            var(&configured, "GIT_ASKPASS").as_deref(),
            Some("/opt/helper")
        );
        assert_eq!(
            var(&configured, "SSH_ASKPASS").as_deref(),
            Some("/opt/ssh-helper")
        );
    }

    #[test]
    fn credential_helpers_are_told_not_to_open_an_interactive_ui() {
        let env = guarded(&[], &[]);
        assert_eq!(var(&env, "GIT_CONFIG_COUNT").as_deref(), Some("2"));
        assert_eq!(
            var(&env, "GIT_CONFIG_KEY_0").as_deref(),
            Some("credential.interactive")
        );
        assert_eq!(var(&env, "GIT_CONFIG_VALUE_0").as_deref(), Some("false"));
        assert_eq!(
            var(&env, "GIT_CONFIG_KEY_1").as_deref(),
            Some("credential.guiPrompt")
        );
        assert_eq!(var(&env, "GIT_CONFIG_VALUE_1").as_deref(), Some("false"));
    }

    #[test]
    fn an_inherited_git_config_protocol_is_extended_not_replaced() {
        let env = guarded(
            &[
                ("GIT_CONFIG_COUNT", "1"),
                ("GIT_CONFIG_KEY_0", "user.name"),
                ("GIT_CONFIG_VALUE_0", "Agent"),
            ],
            &[],
        );
        assert_eq!(var(&env, "GIT_CONFIG_COUNT").as_deref(), Some("3"));
        assert_eq!(var(&env, "GIT_CONFIG_KEY_0").as_deref(), Some("user.name"));
        assert_eq!(
            var(&env, "GIT_CONFIG_KEY_1").as_deref(),
            Some("credential.interactive")
        );
        assert_eq!(
            var(&env, "GIT_CONFIG_KEY_2").as_deref(),
            Some("credential.guiPrompt")
        );
    }

    #[test]
    fn an_ambiguous_git_config_protocol_keeps_only_the_scalar_guards() {
        let env = guarded(
            &[
                ("GIT_CONFIG_COUNT", "2"),
                ("GIT_CONFIG_KEY_0", "user.name"),
                ("GIT_CONFIG_VALUE_0", "Agent"),
            ],
            &[],
        );
        assert_eq!(var(&env, "GIT_CONFIG_COUNT").as_deref(), Some("2"));
        assert_eq!(var(&env, "GIT_CONFIG_KEY_2"), None);
        assert_eq!(var(&env, "GIT_TERMINAL_PROMPT").as_deref(), Some("0"));
    }

    #[test]
    fn a_user_terminal_keeps_its_inherited_git_settings() {
        let env = start(&[("GIT_TERMINAL_PROMPT", "1")], &[]);
        assert_eq!(var(&env, "GIT_TERMINAL_PROMPT").as_deref(), Some("1"));
        assert_eq!(var(&env, "GCM_INTERACTIVE"), None);
        assert_eq!(var(&env, "GIT_ASKPASS"), None);
        assert_eq!(var(&env, "GIT_CONFIG_COUNT"), None);
    }

    #[test]
    fn only_the_guard_value_of_the_request_turns_the_guard_on() {
        let mut extra = BTreeMap::new();
        extra.insert(
            CREDENTIAL_GUARD_ENV.to_owned(),
            EnvValue::new(CREDENTIAL_GUARD_ON.to_owned()),
        );
        assert!(requested(&extra));
        extra.insert(
            CREDENTIAL_GUARD_ENV.to_owned(),
            EnvValue::new("off".to_owned()),
        );
        assert!(!requested(&extra));
    }
}
