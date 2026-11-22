//! The launch plan (decisions 0051, 0053, 0054, 0056): what a run of a kind executes, its environment and the hook
//! files it needs. Pure: no file is read or written here, so every mechanism is tested on its own.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use massorca_protocol::methods::agents::MAX_PROMPT_BYTES;
use serde_json::{Value, json};

use crate::kind::{KindDecl, PromptMode, REF_PLACEHOLDER, StatusMechanism};
use crate::paths;
use crate::provided_env::{ProvidedEnv, reserved_name, same_name};

/// The session id, for `massorca agent hook` and other commands run by the agent.
pub const ENV_SESSION: &str = "MASSORCA_AGENT_SESSION";
/// The run's hook token (0053).
pub const ENV_TOKEN: &str = "MASSORCA_AGENT_TOKEN";
pub const ENV_KIND: &str = "MASSORCA_AGENT_KIND";
/// The absolute path of the `massorca` executable that runs the service.
pub const ENV_BIN: &str = "MASSORCA_BIN";

/// The settings file of a `claude-settings` run, in the session's run directory.
pub const CLAUDE_SETTINGS_FILE: &str = "claude-settings.json";
/// The event name `codex-notify` reports at each turn end.
pub const CODEX_TURN_EVENT: &str = "turn-complete";

/// Which run is being planned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Run<'a> {
    /// The session's first run. `new_ref` is used when the kind assigns the agent's id at launch.
    First {
        prompt: Option<&'a str>,
        new_ref: &'a str,
    },
    /// A new run of an exited session, with the agent's known id.
    Resume { agent_ref: &'a str },
}

/// Everything a plan is built from.
#[derive(Debug, Clone, Copy)]
pub struct LaunchInput<'a> {
    pub decl: &'a KindDecl,
    /// The resolved program.
    pub program: &'a Path,
    pub session_id: &'a str,
    pub token: &'a str,
    pub run: Run<'a>,
    pub profile_dir: &'a Path,
    pub massorca_bin: &'a Path,
    /// The kind's credential home, only when the profile has one (0056).
    pub credential_home: Option<&'a Path>,
    pub caller_args: &'a [String],
    /// Variables from the profile's vault (decision 0071): set after the run's own and the credential home, before the
    /// caller's.
    pub provided_env: &'a ProvidedEnv,
    pub caller_env: &'a BTreeMap<String, String>,
}

/// What to run. Values in `env` may be secrets: never log them.
#[derive(Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Set on top of the terminal environment, in this order of precedence: the caller's values win.
    pub env: BTreeMap<String, String>,
    /// Inherited variables the run must not see.
    pub remove_env: Vec<String>,
    /// Files to write before the run starts.
    pub files: Vec<(PathBuf, String)>,
    /// The agent's id assigned at this launch.
    pub agent_ref: Option<String>,
    /// A first prompt the kind cannot take at launch (`prompt.mode` `none`).
    pub pending_prompt: Option<String>,
}

impl std::fmt::Debug for LaunchPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LaunchPlan")
            .field("program", &self.program)
            .field("arg_count", &self.args.len())
            .field("env_names", &self.env.keys().collect::<Vec<_>>())
            .field("remove_env", &self.remove_env)
            .field(
                "files",
                &self.files.iter().map(|(path, _)| path).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

/// Why a run cannot be planned.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LaunchError {
    #[error("the prompt is longer than {MAX_PROMPT_BYTES} bytes")]
    PromptTooLong,
    #[error("the prompt contains a NUL character")]
    PromptHasNul,
    #[error(
        "the prompt contains a control character other than tab, line feed and carriage return"
    )]
    PromptHasControl,
    #[error("the kind declares no resume form")]
    NotResumable,
    #[error("the vault provides {0}, which the run sets itself; rename that secret")]
    ReservedName(String),
}

/// Refuses prompts over the size limit, with a NUL, or with a control character other than tab, line feed and carriage
/// return (0054, decision 0058): an escape sequence, DEL or a C1 control could make the terminal or the agent act on the
/// text. The text itself never appears in the error.
pub fn check_prompt(text: &str) -> Result<(), LaunchError> {
    if text.len() > MAX_PROMPT_BYTES {
        return Err(LaunchError::PromptTooLong);
    }
    if text.contains('\0') {
        return Err(LaunchError::PromptHasNul);
    }
    if text
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\t' | '\n' | '\r'))
    {
        return Err(LaunchError::PromptHasControl);
    }
    Ok(())
}

/// Builds the plan: the kind's args, hook args, the session ref or resume args, the caller's args, then the prompt.
pub fn plan(input: &LaunchInput<'_>) -> Result<LaunchPlan, LaunchError> {
    let decl = input.decl;
    if let Some(name) = reserved_name(input.provided_env, decl) {
        return Err(LaunchError::ReservedName(name.to_owned()));
    }
    let mut args = decl.args.clone();
    let mut files = Vec::new();
    install_hooks(input, &mut args, &mut files);

    let mut agent_ref = None;
    let mut pending_prompt = None;
    let mut prompt_args = Vec::new();
    match input.run {
        Run::First { prompt, new_ref } => {
            if let Some(assign) = decl.session_ref.as_ref().and_then(|r| r.assign.as_ref()) {
                args.extend(with_ref(assign, new_ref));
                agent_ref = Some(new_ref.to_owned());
            }
            if let Some(text) = prompt {
                check_prompt(text)?;
                match decl.prompt.mode {
                    PromptMode::Argv => {
                        if decl.prompt.separator {
                            prompt_args.push("--".to_owned());
                        }
                        prompt_args.push(text.to_owned());
                    }
                    PromptMode::Flag => {
                        prompt_args.extend(decl.prompt.flag.clone());
                        prompt_args.push(text.to_owned());
                    }
                    PromptMode::None => pending_prompt = Some(text.to_owned()),
                }
            }
        }
        Run::Resume { agent_ref: known } => {
            let resume = decl.resume.as_ref().ok_or(LaunchError::NotResumable)?;
            args.extend(with_ref(resume, known));
        }
    }
    args.extend(input.caller_args.iter().cloned());
    args.extend(prompt_args);

    let (env, remove_env) = environment(input);
    Ok(LaunchPlan {
        program: input.program.to_path_buf(),
        args,
        env,
        remove_env,
        files,
        agent_ref,
        pending_prompt,
    })
}

fn with_ref(args: &[String], agent_ref: &str) -> Vec<String> {
    args.iter()
        .map(|arg| arg.replace(REF_PLACEHOLDER, agent_ref))
        .collect()
}

/// Hooks are installed per run, never globally (0053).
fn install_hooks(
    input: &LaunchInput<'_>,
    args: &mut Vec<String>,
    files: &mut Vec<(PathBuf, String)>,
) {
    let bin = input.massorca_bin.to_string_lossy();
    match input.decl.status.mechanism {
        StatusMechanism::ClaudeSettings => {
            let path =
                paths::run_dir(input.profile_dir, input.session_id).join(CLAUDE_SETTINGS_FILE);
            files.push((path.clone(), claude_settings(&bin, input.decl)));
            args.push("--settings".to_owned());
            args.push(path.to_string_lossy().into_owned());
        }
        StatusMechanism::CodexNotify => {
            // A JSON array of strings is also a TOML array: both escape `\` and `"` the same way.
            let command = Value::from(vec![&*bin, "agent", "hook", CODEX_TURN_EVENT]);
            args.push("-c".to_owned());
            args.push(format!("notify={command}"));
        }
        StatusMechanism::EnvCommand | StatusMechanism::None => {}
    }
}

/// A Claude settings document whose hooks run `"<bin>" agent hook <Event>` for each declared event.
fn claude_settings(bin: &str, decl: &KindDecl) -> String {
    let quoted = quote_for_shell(bin);
    let hooks: serde_json::Map<String, Value> = decl
        .status
        .events
        .keys()
        .map(|event| {
            let command = format!("{quoted} agent hook {event}");
            (
                event.clone(),
                json!([{ "hooks": [{ "type": "command", "command": command }] }]),
            )
        })
        .collect();
    let mut text = serde_json::to_string_pretty(&json!({ "hooks": hooks }))
        .unwrap_or_else(|_| "{}".to_owned());
    text.push('\n');
    text
}

/// `path` in double quotes. On Unix the characters a POSIX shell expands inside double quotes are escaped; Windows
/// paths cannot contain `"`, and their backslashes are separators.
fn quote_for_shell(path: &str) -> String {
    if cfg!(windows) {
        return format!("\"{path}\"");
    }
    let mut quoted = String::from("\"");
    for character in path.chars() {
        if matches!(character, '\\' | '"' | '$' | '`') {
            quoted.push('\\');
        }
        quoted.push(character);
    }
    quoted.push('"');
    quoted
}

/// The run's own variables, the credential home when there is one, the provided ones, then the caller's; and what to
/// strip. A provided name in `strip_env` is set, not stripped: the vault granted it.
fn environment(input: &LaunchInput<'_>) -> (BTreeMap<String, String>, Vec<String>) {
    let mut env = BTreeMap::new();
    env.insert(ENV_SESSION.to_owned(), input.session_id.to_owned());
    env.insert(ENV_TOKEN.to_owned(), input.token.to_owned());
    env.insert(ENV_KIND.to_owned(), input.decl.id.clone());
    env.insert(
        ENV_BIN.to_owned(),
        input.massorca_bin.to_string_lossy().into_owned(),
    );
    let mut remove_env = Vec::new();
    if let (Some(home), Some(credentials)) = (input.credential_home, &input.decl.credentials) {
        env.insert(
            credentials.home_env.clone(),
            home.to_string_lossy().into_owned(),
        );
        remove_env = credentials
            .strip_env
            .iter()
            .filter(|name| !input.caller_env.keys().any(|set| same_name(set, name)))
            .cloned()
            .collect();
    }
    for (name, value) in input.provided_env.iter() {
        env.retain(|existing, _| !same_name(existing, name));
        remove_env.retain(|stripped| !same_name(stripped, name));
        env.insert(name.to_owned(), value.to_owned());
    }
    for (name, value) in input.caller_env {
        env.retain(|existing, _| !same_name(existing, name));
        env.insert(name.clone(), value.clone());
    }
    (env, remove_env)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::catalog::built_ins;

    struct Fixture {
        decl: KindDecl,
        program: PathBuf,
        profile: PathBuf,
        bin: PathBuf,
        home: Option<PathBuf>,
        caller_args: Vec<String>,
        provided: ProvidedEnv,
        caller_env: BTreeMap<String, String>,
    }

    impl Fixture {
        fn new(decl: KindDecl) -> Self {
            Self {
                decl,
                program: PathBuf::from("/opt/agents/agent"),
                profile: PathBuf::from("/data/profiles/default"),
                bin: PathBuf::from("/opt/mass orca/massorca"),
                home: None,
                caller_args: Vec::new(),
                provided: ProvidedEnv::default(),
                caller_env: BTreeMap::new(),
            }
        }

        fn plan(&self, run: Run<'_>) -> Result<LaunchPlan, LaunchError> {
            plan(&LaunchInput {
                decl: &self.decl,
                program: &self.program,
                session_id: "s-1",
                token: "t0k3n",
                run,
                profile_dir: &self.profile,
                massorca_bin: &self.bin,
                credential_home: self.home.as_deref(),
                caller_args: &self.caller_args,
                provided_env: &self.provided,
                caller_env: &self.caller_env,
            })
        }

        fn first(&self, prompt: Option<&str>) -> LaunchPlan {
            self.plan(Run::First {
                prompt,
                new_ref: "r-1",
            })
            .expect("plans")
        }
    }

    fn built_in(id: &str) -> KindDecl {
        built_ins().into_iter().find(|kind| kind.id == id).unwrap()
    }

    fn kind(value: serde_json::Value) -> KindDecl {
        KindDecl::from_json(&value.to_string()).unwrap()
    }

    fn fake(prompt: serde_json::Value) -> KindDecl {
        kind(json!({
            "id": "fake", "name": "Fake", "program": ["fake"], "args": ["--plain"],
            "prompt": prompt, "status": { "mechanism": "env-command", "events": { "stop": "done" } }
        }))
    }

    #[test]
    fn argv_mode_puts_the_prompt_last_after_the_caller_args() {
        let mut fixture = Fixture::new(fake(json!({ "mode": "argv" })));
        fixture.caller_args = vec!["--model".to_owned(), "x".to_owned()];
        let plan = fixture.first(Some("fix it"));
        assert_eq!(plan.args, ["--plain", "--model", "x", "fix it"]);
        assert_eq!(plan.program, PathBuf::from("/opt/agents/agent"));
        assert!(plan.files.is_empty(), "env-command installs nothing");
        assert_eq!(plan.agent_ref, None);
    }

    #[test]
    fn argv_mode_with_a_separator_puts_dashes_before_the_prompt() {
        let fixture = Fixture::new(fake(json!({ "mode": "argv", "separator": true })));
        assert_eq!(fixture.first(Some("-x")).args, ["--plain", "--", "-x"]);
        assert_eq!(
            fixture.first(None).args,
            ["--plain"],
            "no prompt, no separator"
        );
    }

    #[test]
    fn flag_mode_puts_the_flag_before_the_prompt() {
        let fixture = Fixture::new(built_in("antigravity"));
        assert_eq!(
            fixture.first(Some("hello")).args,
            ["--prompt-interactive", "hello"]
        );
    }

    #[test]
    fn none_mode_keeps_the_prompt_for_later() {
        let plan = Fixture::new(fake(json!({ "mode": "none" }))).first(Some("later"));
        assert_eq!(plan.args, ["--plain"]);
        assert_eq!(plan.pending_prompt.as_deref(), Some("later"));
    }

    #[test]
    fn prompts_over_the_limit_or_with_nul_are_refused() {
        let fixture = Fixture::new(fake(json!({ "mode": "argv" })));
        let long = "a".repeat(MAX_PROMPT_BYTES + 1);
        let refused = fixture.plan(Run::First {
            prompt: Some(&long),
            new_ref: "r",
        });
        assert_eq!(refused, Err(LaunchError::PromptTooLong));
        let at_limit = "a".repeat(MAX_PROMPT_BYTES);
        assert!(check_prompt(&at_limit).is_ok());
        assert_eq!(check_prompt("a\0b"), Err(LaunchError::PromptHasNul));
    }

    /// Escape sequences, DEL and the C1 controls never reach an agent in a prompt: a spliced paste marker, a plain ESC and
    /// a CSI (U+009B) are refused. Tabs and line breaks are allowed (decision 0058).
    #[test]
    fn controls_are_refused_except_tab_and_line_breaks() {
        for refused in [
            "\x1b[20\x1b[201~1~",
            "\x1b",
            "\u{9b}",
            "\u{7f}",
            "\u{85}",
            "\u{9f}",
            "\x07",
        ] {
            assert_eq!(
                check_prompt(refused),
                Err(LaunchError::PromptHasControl),
                "{refused:?} must be refused"
            );
        }
        for accepted in [
            "a\tb",
            "line one\nline two\r\n",
            "héllo — 日本語 🙂",
            "\u{a0}",
        ] {
            assert!(
                check_prompt(accepted).is_ok(),
                "{accepted:?} must be accepted"
            );
        }
    }

    #[test]
    fn claude_assigns_its_ref_and_writes_a_settings_file_for_its_hooks() {
        let fixture = Fixture::new(built_in("claude"));
        let plan = fixture.first(Some("go"));
        let settings = paths::run_dir(&fixture.profile, "s-1").join(CLAUDE_SETTINGS_FILE);
        assert_eq!(
            plan.args,
            [
                "--settings".to_owned(),
                settings.to_string_lossy().into_owned(),
                "--session-id".to_owned(),
                "r-1".to_owned(),
                "go".to_owned()
            ]
        );
        assert_eq!(plan.agent_ref.as_deref(), Some("r-1"));
        assert_eq!(plan.files.len(), 1);
        assert_eq!(plan.files[0].0, settings);
        let document: Value = serde_json::from_str(&plan.files[0].1).expect("valid JSON");
        let hooks = document["hooks"].as_object().unwrap();
        let mut events: Vec<&str> = hooks.keys().map(String::as_str).collect();
        events.sort_unstable();
        assert_eq!(
            events,
            [
                "Notification",
                "PreToolUse",
                "SessionStart",
                "Stop",
                "UserPromptSubmit"
            ]
        );
        let command = hooks["Stop"][0]["hooks"][0]["command"].as_str().unwrap();
        assert_eq!(hooks["Stop"][0]["hooks"][0]["type"], "command");
        assert_eq!(command, "\"/opt/mass orca/massorca\" agent hook Stop");
    }

    #[cfg(unix)]
    #[test]
    fn shell_quoting_escapes_what_double_quotes_expand() {
        assert_eq!(quote_for_shell("/a b/$x\"`\\"), "\"/a b/\\$x\\\"\\`\\\\\"");
    }

    #[test]
    fn codex_gets_its_notify_command_as_an_escaped_array() {
        let mut fixture = Fixture::new(built_in("codex"));
        fixture.bin = PathBuf::from("C:\\Program Files\\Mass\"Orca\\massorca.exe");
        let plan = fixture.first(Some("go"));
        assert_eq!(plan.args[0], "-c");
        assert_eq!(
            plan.args[1],
            "notify=[\"C:\\\\Program Files\\\\Mass\\\"Orca\\\\massorca.exe\",\"agent\",\"hook\",\"turn-complete\"]"
        );
        assert_eq!(&plan.args[2..], ["go"]);
        assert_eq!(plan.agent_ref, None, "Codex's ref is learned, not assigned");
        assert!(plan.files.is_empty());
    }

    #[test]
    fn resume_uses_the_known_ref_and_no_prompt() {
        let fixture = Fixture::new(built_in("gentle-shell"));
        let plan = fixture.plan(Run::Resume { agent_ref: "abc" }).unwrap();
        assert_eq!(plan.args, ["--", "--session", "abc"]);
        let claude = Fixture::new(built_in("claude"));
        let plan = claude.plan(Run::Resume { agent_ref: "abc" }).unwrap();
        assert_eq!(&plan.args[2..], ["--resume", "abc"]);
        let no_resume = Fixture::new(fake(json!({ "mode": "argv" })));
        assert_eq!(
            no_resume.plan(Run::Resume { agent_ref: "abc" }),
            Err(LaunchError::NotResumable)
        );
    }

    #[test]
    fn the_run_environment_names_the_session_and_the_binary() {
        let plan = Fixture::new(fake(json!({ "mode": "argv" }))).first(None);
        assert_eq!(plan.env[ENV_SESSION], "s-1");
        assert_eq!(plan.env[ENV_TOKEN], "t0k3n");
        assert_eq!(plan.env[ENV_KIND], "fake");
        assert_eq!(plan.env[ENV_BIN], "/opt/mass orca/massorca");
        assert!(plan.remove_env.is_empty());
    }

    #[test]
    fn a_credential_home_sets_its_variable_and_strips_keys() {
        let mut fixture = Fixture::new(built_in("claude"));
        let without = fixture.first(None);
        assert!(!without.env.contains_key("CLAUDE_CONFIG_DIR"));
        assert!(without.remove_env.is_empty(), "no home, nothing stripped");

        fixture.home = Some(PathBuf::from("/data/profiles/default/agents/homes/claude"));
        let with = fixture.first(None);
        assert_eq!(
            with.env["CLAUDE_CONFIG_DIR"],
            "/data/profiles/default/agents/homes/claude"
        );
        assert_eq!(
            with.remove_env,
            [
                "ANTHROPIC_API_KEY",
                "ANTHROPIC_AUTH_TOKEN",
                "CLAUDE_CODE_OAUTH_TOKEN"
            ]
        );
    }

    #[test]
    fn the_caller_env_comes_last_and_is_never_stripped() {
        let mut fixture = Fixture::new(built_in("claude"));
        fixture.home = Some(PathBuf::from("/home-dir"));
        fixture.caller_env = BTreeMap::from([
            ("ANTHROPIC_API_KEY".to_owned(), "explicit".to_owned()),
            ("CLAUDE_CONFIG_DIR".to_owned(), "/elsewhere".to_owned()),
        ]);
        let plan = fixture.first(None);
        assert_eq!(plan.env["ANTHROPIC_API_KEY"], "explicit");
        assert_eq!(plan.env["CLAUDE_CONFIG_DIR"], "/elsewhere");
        assert!(
            !plan
                .remove_env
                .iter()
                .any(|name| name == "ANTHROPIC_API_KEY")
        );
    }

    fn provided(pairs: &[(&str, &str)]) -> ProvidedEnv {
        ProvidedEnv::new(
            pairs
                .iter()
                .map(|(name, value)| {
                    (
                        (*name).to_owned(),
                        zeroize::Zeroizing::new((*value).to_owned()),
                    )
                })
                .collect(),
        )
    }

    #[test]
    fn provided_variables_come_after_the_home_and_before_the_caller() {
        let mut fixture = Fixture::new(built_in("claude"));
        fixture.home = Some(PathBuf::from("/home-dir"));
        fixture.provided = provided(&[("API_URL", "from-vault"), ("SHARED", "from-vault")]);
        fixture.caller_env = BTreeMap::from([("SHARED".to_owned(), "from-caller".to_owned())]);
        let plan = fixture.first(None);
        assert_eq!(plan.env["API_URL"], "from-vault");
        assert_eq!(plan.env["SHARED"], "from-caller", "the caller wins");
        assert_eq!(plan.env["CLAUDE_CONFIG_DIR"], "/home-dir");
    }

    #[test]
    fn a_provided_strip_env_name_is_set_and_not_stripped() {
        let mut fixture = Fixture::new(built_in("claude"));
        fixture.home = Some(PathBuf::from("/home-dir"));
        fixture.provided = provided(&[("ANTHROPIC_API_KEY", "granted")]);
        let plan = fixture.first(None);
        assert_eq!(plan.env["ANTHROPIC_API_KEY"], "granted");
        assert!(
            !plan
                .remove_env
                .iter()
                .any(|name| name == "ANTHROPIC_API_KEY")
        );
        assert!(
            plan.remove_env
                .iter()
                .any(|name| name == "ANTHROPIC_AUTH_TOKEN")
        );
    }

    #[test]
    fn a_provided_massorca_or_home_variable_is_refused() {
        for name in ["MASSORCA_X", "MASSORCA_AGENT_TOKEN", "CLAUDE_CONFIG_DIR"] {
            let mut fixture = Fixture::new(built_in("claude"));
            fixture.provided = provided(&[(name, "placeholder-value")]);
            let refused = fixture
                .plan(Run::First {
                    prompt: None,
                    new_ref: "r-1",
                })
                .unwrap_err();
            assert_eq!(refused, LaunchError::ReservedName(name.to_owned()));
            assert!(!refused.to_string().contains("placeholder-value"));
        }
    }

    #[test]
    fn debug_output_shows_no_values() {
        let mut fixture = Fixture::new(fake(json!({ "mode": "argv" })));
        fixture.provided = provided(&[("API_KEY", "provided-placeholder")]);
        let plan = fixture.first(Some("secret prompt"));
        let debug = format!("{plan:?}");
        assert!(
            !debug.contains("t0k3n")
                && !debug.contains("secret prompt")
                && !debug.contains("provided-placeholder"),
            "{debug}"
        );
    }
}
