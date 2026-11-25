//! The launch plan (decisions 0051, 0053, 0054, 0056): what a run of a kind executes, its environment and the hook
//! files it needs. Pure: no file is read or written here, so every mechanism is tested on its own.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use massorca_core_terminal::EnvValue;
use massorca_protocol::methods::agents::MAX_PROMPT_BYTES;
use serde_json::{Value, json};

use crate::kind::{FastDecl, KindDecl, PromptMode, REF_PLACEHOLDER, StatusMechanism};
use crate::paths;
use crate::provided_env::{ProvidedEnv, dangerous_caller_name, reserved_name, same_name};

/// The session id, for `massorca agent hook` and other commands run by the agent.
pub const ENV_SESSION: &str = "MASSORCA_AGENT_SESSION";
/// The run's hook token (0053).
pub const ENV_TOKEN: &str = "MASSORCA_AGENT_TOKEN";
pub const ENV_KIND: &str = "MASSORCA_AGENT_KIND";
/// The absolute path of the `massorca` executable that runs the service.
pub const ENV_BIN: &str = "MASSORCA_BIN";

/// The settings file of a `claude-settings` run, in the session's run directory.
pub const CLAUDE_SETTINGS_FILE: &str = "claude-settings.json";
/// The flag a `claude-settings` run passes its settings file with.
const SETTINGS_FLAG: &str = "--settings";
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
    /// The arguments of the run's permission mode (decision 0083), right after the kind's own.
    pub mode_args: &'a [String],
    pub caller_args: &'a [String],
    /// Variables from the profile's vault (decision 0071): set after the run's own and the credential home, before the
    /// caller's.
    pub provided_env: &'a ProvidedEnv,
    /// The run is marked as holding secrets: it got vault variables now, earlier (the mark is sticky), or through its
    /// parent. Its terminal carries the mark, and the caller's env may not add a dangerous name.
    pub holds_secrets: bool,
    pub caller_env: &'a BTreeMap<String, String>,
    /// MassOrca's per-launch instructions (decision 0161), passed through the kind's `instructions` channel when it
    /// declares one. `None` when the session may not delegate further.
    pub instructions: Option<&'a str>,
    /// The `PATH` the run's terminal inherits from the service, which the run's own `PATH` starts from.
    pub inherited_path: Option<&'a str>,
    /// The kind's fast-mode settings, when the start asked for fast mode (decision 0165). They go on the kind's settings
    /// flag, or into the hook settings file when that file is passed on `--settings` already.
    pub fast: Option<&'a FastDecl>,
}

/// What to run. Values in `env` may be secrets: never log them.
#[derive(Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Set on top of the terminal environment, in this order of precedence: the caller's values win.
    pub env: BTreeMap<String, EnvValue>,
    /// Inherited variables the run must not see.
    pub remove_env: Vec<String>,
    /// Files to write before the run starts.
    pub files: Vec<(PathBuf, String)>,
    /// The agent's id assigned at this launch.
    pub agent_ref: Option<String>,
    /// A first prompt the kind cannot take at launch (`prompt.mode` `none`).
    pub pending_prompt: Option<String>,
    /// Copied to the run's terminal at its creation (decision 0071).
    pub holds_secrets: bool,
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
    #[error(
        "the vault provides {0}, which the run sets itself or which may not carry a secret; rename that secret"
    )]
    ReservedName(String),
    #[error("the caller's env sets {0}, which a run holding vault secrets may not receive")]
    DangerousCallerEnv(String),
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

/// Builds the plan: the kind's args, the mode's args, hook args, the session ref or resume args, the caller's args, then
/// the prompt.
pub fn plan(input: &LaunchInput<'_>) -> Result<LaunchPlan, LaunchError> {
    let decl = input.decl;
    if let Some(name) = reserved_name(input.provided_env, decl) {
        return Err(LaunchError::ReservedName(name.to_owned()));
    }
    if (input.holds_secrets || !input.provided_env.is_empty())
        && let Some(name) = dangerous_caller_name(input.caller_env)
    {
        return Err(LaunchError::DangerousCallerEnv(name.to_owned()));
    }
    let mut args = decl.args.clone();
    args.extend(input.mode_args.iter().cloned());
    let mut files = Vec::new();
    let carries_fast = install_hooks(input, &mut args, &mut files);
    if let Some(fast) = input.fast
        && !carries_fast
    {
        args.push(fast.settings_flag.clone());
        args.push(Value::Object(fast.settings.clone()).to_string());
    }
    if let (Some(channel), Some(text)) = (&decl.instructions, input.instructions) {
        args.extend(channel.args_with(text));
    }

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
                    PromptMode::StartupQuery => {
                        prompt_args.extend(
                            decl.prompt
                                .flag
                                .as_ref()
                                .map(|flag| format!("{flag}={text}")),
                        );
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
        holds_secrets: input.holds_secrets || !input.provided_env.is_empty(),
    })
}

fn with_ref(args: &[String], agent_ref: &str) -> Vec<String> {
    args.iter()
        .map(|arg| arg.replace(REF_PLACEHOLDER, agent_ref))
        .collect()
}

/// Hooks are installed per run, never globally (0053). Returns whether the settings file it wrote carries the run's fast
/// settings: a `claude-settings` run passes its file on `--settings`, so fast mode merges into that file when the kind's
/// settings flag is `--settings` too (decision 0165), instead of a second `--settings`.
fn install_hooks(
    input: &LaunchInput<'_>,
    args: &mut Vec<String>,
    files: &mut Vec<(PathBuf, String)>,
) -> bool {
    let bin = input.massorca_bin.to_string_lossy();
    match input.decl.status.mechanism {
        StatusMechanism::ClaudeSettings => {
            let path =
                paths::run_dir(input.profile_dir, input.session_id).join(CLAUDE_SETTINGS_FILE);
            let fast = input
                .fast
                .filter(|fast| fast.settings_flag == SETTINGS_FLAG);
            files.push((path.clone(), claude_settings(&bin, input.decl, fast)));
            args.push(SETTINGS_FLAG.to_owned());
            args.push(path.to_string_lossy().into_owned());
            fast.is_some()
        }
        StatusMechanism::CodexNotify => {
            // A JSON array of strings is also a TOML array: both escape `\` and `"` the same way.
            let command = Value::from(vec![&*bin, "agent", "hook", CODEX_TURN_EVENT]);
            args.push("-c".to_owned());
            args.push(format!("notify={command}"));
            false
        }
        StatusMechanism::EnvCommand | StatusMechanism::None => false,
    }
}

/// A Claude settings document whose hooks run `"<bin>" agent hook <Event>` for each declared event, with the keys of
/// the run's fast settings when it asked for fast mode (decision 0165).
fn claude_settings(bin: &str, decl: &KindDecl, fast: Option<&FastDecl>) -> String {
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
    let mut document = json!({ "hooks": hooks });
    // The kind's validation keeps the fast settings from setting `hooks`, so the hooks stay as they are.
    if let (Some(fast), Some(object)) = (fast, document.as_object_mut()) {
        for (key, value) in &fast.settings {
            object.insert(key.clone(), value.clone());
        }
    }
    let mut text = serde_json::to_string_pretty(&document).unwrap_or_else(|_| "{}".to_owned());
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

/// The run identity a launch sets (decision 0160): a value inherited by the service or given by the caller is never
/// passed through, so a run never carries another session's identity.
pub const RUN_IDENTITY_ENV: [&str; 3] = [ENV_SESSION, ENV_TOKEN, ENV_KIND];

/// The vendor session markers the built-in kinds declare in `strip_inherited_env` (decision 0160). The service drops
/// them from the environment every terminal starts from, so they are absent rather than empty; a kind installed later
/// still has its own list emptied at launch. A test keeps this list equal to the built-in declarations.
pub const INHERITED_VENDOR_MARKERS: &[&str] = &[
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_BRIDGE_SESSION_ID",
    "CLAUDECODE",
];

/// Whether `name` is one of [`INHERITED_VENDOR_MARKERS`], compared as the platform compares variable names.
#[must_use]
pub fn is_inherited_vendor_marker(name: &str) -> bool {
    INHERITED_VENDOR_MARKERS
        .iter()
        .any(|marker| same_name(marker, name))
}

/// Whether `name` is one of [`RUN_IDENTITY_ENV`], compared without case on Windows.
#[must_use]
pub fn is_run_identity(name: &str) -> bool {
    RUN_IDENTITY_ENV
        .iter()
        .any(|identity| same_name(identity, name))
}

/// The credential home when there is one, the provided variables, the caller's, then the run's own identity, which
/// nothing overrides; and what to strip: the credentials' `strip_env` with a home, then the kind's
/// `strip_inherited_env` (decision 0160). A name the launch sets is set, not stripped: a provided name in `strip_env` was
/// granted by the vault.
fn environment(input: &LaunchInput<'_>) -> (BTreeMap<String, EnvValue>, Vec<String>) {
    let mut env = BTreeMap::new();
    let value = |text: &str| EnvValue::new(text.to_owned());
    env.insert(
        ENV_BIN.to_owned(),
        value(&input.massorca_bin.to_string_lossy()),
    );
    let mut remove_env = Vec::new();
    if let (Some(home), Some(credentials)) = (input.credential_home, &input.decl.credentials) {
        env.insert(credentials.home_env.clone(), value(&home.to_string_lossy()));
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
        env.insert(name.to_owned(), EnvValue::new(value.to_owned()));
    }
    for (name, caller_value) in input.caller_env {
        env.retain(|existing, _| !same_name(existing, name));
        env.insert(name.clone(), value(caller_value));
    }
    env.retain(|existing, _| !is_run_identity(existing));
    env.insert(ENV_SESSION.to_owned(), value(input.session_id));
    env.insert(ENV_TOKEN.to_owned(), value(input.token));
    env.insert(ENV_KIND.to_owned(), value(&input.decl.id));
    for name in &input.decl.strip_inherited_env {
        let set = env.keys().any(|existing| same_name(existing, name));
        if !set && !remove_env.iter().any(|stripped| same_name(stripped, name)) {
            remove_env.push(name.clone());
        }
    }
    with_massorca_on_path(input, &mut env);
    (env, remove_env)
}

/// Puts the folder of the `massorca` executable first on the run's `PATH` (decision 0161), so the agent can run
/// `massorca`: in front of the `PATH` the caller or the vault set, else the inherited one. The rest is kept as it is, and
/// a folder that is already first is not added again.
fn with_massorca_on_path(input: &LaunchInput<'_>, env: &mut BTreeMap<String, EnvValue>) {
    let Some(folder) = input
        .massorca_bin
        .parent()
        .filter(|folder| !folder.as_os_str().is_empty())
    else {
        return;
    };
    let folder = folder.to_string_lossy();
    let separator = if cfg!(windows) { ';' } else { ':' };
    let current = env
        .iter()
        .find(|(name, _)| same_name(name, PATH_ENV))
        .map(|(_, value)| value.as_str().to_owned())
        .or_else(|| input.inherited_path.map(str::to_owned))
        .unwrap_or_default();
    let first = current.split(separator).next().unwrap_or_default();
    let already_first = if cfg!(windows) {
        first
            .trim_end_matches(['\\', '/'])
            .eq_ignore_ascii_case(folder.trim_end_matches(['\\', '/']))
    } else {
        first.trim_end_matches('/') == folder.trim_end_matches('/')
    };
    let path = if already_first {
        current
    } else if current.is_empty() {
        folder.into_owned()
    } else {
        format!("{folder}{separator}{current}")
    };
    env.retain(|name, _| !same_name(name, PATH_ENV));
    env.insert(PATH_ENV.to_owned(), EnvValue::new(path));
}

/// The search path variable a run's `PATH` is set under; Windows compares the name without case.
const PATH_ENV: &str = "PATH";

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
        mode_args: Vec<String>,
        caller_args: Vec<String>,
        provided: ProvidedEnv,
        holds_secrets: bool,
        caller_env: BTreeMap<String, String>,
        instructions: Option<String>,
        inherited_path: Option<String>,
        fast: Option<FastDecl>,
    }

    impl Fixture {
        fn new(decl: KindDecl) -> Self {
            Self {
                decl,
                program: PathBuf::from("/opt/agents/agent"),
                profile: PathBuf::from("/data/profiles/default"),
                bin: PathBuf::from("/opt/mass orca/massorca"),
                home: None,
                mode_args: Vec::new(),
                caller_args: Vec::new(),
                provided: ProvidedEnv::default(),
                holds_secrets: false,
                caller_env: BTreeMap::new(),
                instructions: None,
                inherited_path: None,
                fast: None,
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
                mode_args: &self.mode_args,
                caller_args: &self.caller_args,
                provided_env: &self.provided,
                holds_secrets: self.holds_secrets,
                caller_env: &self.caller_env,
                instructions: self.instructions.as_deref(),
                inherited_path: self.inherited_path.as_deref(),
                fast: self.fast.as_ref(),
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

    /// The fast settings `kind` declares in the claude form: `{"fastMode": true}` on `--settings`.
    fn fast_settings(flag: &str) -> FastDecl {
        serde_json::from_value(json!({ "settings": { "fastMode": true }, "settings_flag": flag }))
            .expect("fast settings parse")
    }

    #[test]
    fn claude_fast_mode_joins_its_hook_settings_file_and_passes_one_settings_flag() {
        let mut fixture = Fixture::new(built_in("claude"));
        fixture.fast = Some(fast_settings("--settings"));
        let plan = fixture.first(Some("go"));
        let settings = paths::run_dir(&fixture.profile, "s-1").join(CLAUDE_SETTINGS_FILE);
        let flags = plan.args.iter().filter(|arg| *arg == "--settings").count();
        assert_eq!(
            flags, 1,
            "one --settings, carrying hooks and fast mode: {:?}",
            plan.args
        );
        assert_eq!(plan.args[1], settings.to_string_lossy());
        assert_eq!(plan.files.len(), 1);
        let document: Value = serde_json::from_str(&plan.files[0].1).expect("valid JSON");
        assert_eq!(document["fastMode"], json!(true));
        assert!(
            document["hooks"]["Stop"].is_array(),
            "the hooks stay: {document}"
        );
    }

    #[test]
    fn a_kind_without_hook_settings_gets_its_fast_settings_as_one_json_argument() {
        let mut fixture = Fixture::new(fake(json!({ "mode": "argv" })));
        fixture.fast = Some(fast_settings("--settings"));
        let plan = fixture.first(Some("fix it"));
        assert_eq!(
            plan.args,
            ["--plain", "--settings", "{\"fastMode\":true}", "fix it"]
        );
        assert!(plan.files.is_empty());
    }

    #[test]
    fn fast_settings_on_another_flag_are_passed_on_it_beside_the_hook_settings() {
        let mut fixture = Fixture::new(built_in("claude"));
        fixture.fast = Some(fast_settings("--config"));
        let plan = fixture.first(None);
        let settings = paths::run_dir(&fixture.profile, "s-1").join(CLAUDE_SETTINGS_FILE);
        let document: Value = serde_json::from_str(&plan.files[0].1).expect("valid JSON");
        assert!(document.get("fastMode").is_none(), "{document}");
        let at = plan
            .args
            .iter()
            .position(|arg| arg == "--config")
            .expect("the fast settings flag");
        assert_eq!(plan.args[at + 1], "{\"fastMode\":true}");
        assert_eq!(plan.args[1], settings.to_string_lossy());
    }

    #[test]
    fn no_fast_settings_leave_the_launch_unchanged() {
        let fixture = Fixture::new(fake(json!({ "mode": "argv" })));
        let plan = fixture.first(Some("fix it"));
        assert_eq!(plan.args, ["--plain", "fix it"]);
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
    fn mode_args_follow_the_kind_args_on_a_first_run_and_a_resume() {
        let mut fixture = Fixture::new(built_in("codex"));
        fixture.mode_args = vec!["--sandbox".to_owned(), "read-only".to_owned()];
        let first = fixture.first(Some("go"));
        assert_eq!(&first.args[..2], ["--sandbox", "read-only"]);
        assert_eq!(first.args.last().map(String::as_str), Some("go"));
        let resumed = fixture.plan(Run::Resume { agent_ref: "r-9" }).unwrap();
        assert_eq!(&resumed.args[..2], ["--sandbox", "read-only"]);
        assert_eq!(&resumed.args[resumed.args.len() - 2..], ["resume", "r-9"]);
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
    fn startup_query_mode_hands_the_prompt_as_one_flag_token_after_the_caller_args() {
        let mut fixture = Fixture::new(fake(json!({ "mode": "startup-query", "flag": "--query" })));
        fixture.caller_args = vec!["--model".to_owned(), "x".to_owned()];
        let plan = fixture.first(Some("-x then go"));
        assert_eq!(plan.args, ["--plain", "--model", "x", "--query=-x then go"]);
        assert_eq!(
            plan.pending_prompt, None,
            "the agent submits the query itself, so nothing is typed later"
        );
        assert_eq!(fixture.first(None).args, ["--plain", "--model", "x"]);
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
        assert_eq!(plan.env[ENV_SESSION].as_str(), "s-1");
        assert_eq!(plan.env[ENV_TOKEN].as_str(), "t0k3n");
        assert_eq!(plan.env[ENV_KIND].as_str(), "fake");
        assert_eq!(plan.env[ENV_BIN].as_str(), "/opt/mass orca/massorca");
        assert!(plan.remove_env.is_empty());
    }

    /// The `PATH` separator and the folder of the fixture's `massorca`.
    const SEPARATOR: &str = if cfg!(windows) { ";" } else { ":" };
    const BIN_FOLDER: &str = "/opt/mass orca";

    #[test]
    fn the_run_path_starts_with_the_massorca_folder_and_keeps_the_rest() {
        let mut fixture = Fixture::new(fake(json!({ "mode": "argv" })));
        fixture.inherited_path = Some(format!("/usr/bin{SEPARATOR}/bin"));
        let plan = fixture.first(None);
        assert_eq!(
            plan.env[PATH_ENV].as_str(),
            format!("{BIN_FOLDER}{SEPARATOR}/usr/bin{SEPARATOR}/bin")
        );

        // Already first: not added again.
        fixture.inherited_path = Some(format!("{BIN_FOLDER}{SEPARATOR}/usr/bin"));
        assert_eq!(
            fixture.first(None).env[PATH_ENV].as_str(),
            format!("{BIN_FOLDER}{SEPARATOR}/usr/bin")
        );

        // Nothing inherited: the folder alone.
        fixture.inherited_path = None;
        assert_eq!(fixture.first(None).env[PATH_ENV].as_str(), BIN_FOLDER);

        // A caller's PATH is the one the folder goes in front of.
        fixture.inherited_path = Some("/usr/bin".to_owned());
        let caller_name = if cfg!(windows) { "Path" } else { "PATH" };
        fixture
            .caller_env
            .insert(caller_name.to_owned(), "/caller/bin".to_owned());
        let plan = fixture.first(None);
        let paths: Vec<_> = plan
            .env
            .iter()
            .filter(|(name, _)| same_name(name, PATH_ENV))
            .map(|(_, value)| value.as_str().to_owned())
            .collect();
        assert_eq!(paths, [format!("{BIN_FOLDER}{SEPARATOR}/caller/bin")]);
    }

    #[test]
    fn instructions_go_through_the_declared_channel_only_when_given() {
        let mut fixture = Fixture::new(built_in("claude"));
        fixture.instructions = Some("be brief".to_owned());
        let plan = fixture.first(Some("go"));
        let at = plan
            .args
            .iter()
            .position(|arg| arg == "--append-system-prompt")
            .expect("the channel's flag");
        assert_eq!(plan.args[at + 1], "be brief");
        assert_eq!(plan.args.last().map(String::as_str), Some("go"));
        let resumed = fixture.plan(Run::Resume { agent_ref: "abc" }).unwrap();
        assert!(resumed.args.iter().any(|arg| arg == "be brief"));

        fixture.instructions = None;
        let plan = fixture.first(Some("go"));
        assert!(!plan.args.iter().any(|arg| arg == "--append-system-prompt"));

        // A kind without a channel gets no text.
        let mut codex = Fixture::new(built_in("codex"));
        codex.instructions = Some("be brief".to_owned());
        assert!(!codex.first(None).args.iter().any(|arg| arg == "be brief"));
    }

    #[test]
    fn a_credential_home_sets_its_variable_and_strips_keys() {
        let mut fixture = Fixture::new(built_in("claude"));
        let without = fixture.first(None);
        assert!(!without.env.contains_key("CLAUDE_CONFIG_DIR"));
        assert_eq!(
            without.remove_env, CLAUDE_INHERITED,
            "no home, only the inherited markers are stripped"
        );

        fixture.home = Some(PathBuf::from("/data/profiles/default/agents/homes/claude"));
        let with = fixture.first(None);
        assert_eq!(
            with.env["CLAUDE_CONFIG_DIR"].as_str(),
            "/data/profiles/default/agents/homes/claude"
        );
        assert_eq!(
            with.remove_env[..3],
            [
                "ANTHROPIC_API_KEY",
                "ANTHROPIC_AUTH_TOKEN",
                "CLAUDE_CODE_OAUTH_TOKEN"
            ]
        );
        assert_eq!(with.remove_env[3..], CLAUDE_INHERITED);
    }

    /// The inherited markers the built-in claude kind strips (decision 0160).
    const CLAUDE_INHERITED: [&str; 4] = [
        "CLAUDE_CODE_CHILD_SESSION",
        "CLAUDE_CODE_SESSION_ID",
        "CLAUDE_CODE_BRIDGE_SESSION_ID",
        "CLAUDECODE",
    ];

    #[test]
    fn inherited_markers_are_stripped_from_every_run_unless_the_launch_sets_them() {
        let mut fixture = Fixture::new(kind(json!({
            "id": "fake", "name": "Fake", "program": ["fake"],
            "resume": ["--resume", "{ref}"], "session_ref": { "learn": "id" },
            "strip_inherited_env": ["PARENT_STAMP", "PARENT_ID", "SHARED"]
        })));
        fixture.caller_env = BTreeMap::from([("SHARED".to_owned(), "mine".to_owned())]);
        let first = fixture.first(None);
        assert_eq!(first.remove_env, ["PARENT_STAMP", "PARENT_ID"]);
        assert_eq!(
            first.env["SHARED"].as_str(),
            "mine",
            "a name the launch sets is set"
        );
        // A resume carries no caller env (docs/architecture/agents.md), so every listed name is stripped.
        fixture.caller_env.clear();
        let resumed = fixture.plan(Run::Resume { agent_ref: "r-9" }).unwrap();
        assert_eq!(resumed.remove_env, ["PARENT_STAMP", "PARENT_ID", "SHARED"]);
    }

    #[test]
    fn the_run_identity_is_the_launch_s_own_whatever_the_caller_sets() {
        let mut fixture = Fixture::new(fake(json!({ "mode": "argv" })));
        fixture.caller_env = BTreeMap::from([
            (ENV_SESSION.to_owned(), "other-session".to_owned()),
            (ENV_TOKEN.to_owned(), "other-token".to_owned()),
            (ENV_KIND.to_owned(), "other-kind".to_owned()),
        ]);
        let plan = fixture.first(None);
        assert_eq!(plan.env[ENV_SESSION].as_str(), "s-1");
        assert_eq!(plan.env[ENV_TOKEN].as_str(), "t0k3n");
        assert_eq!(plan.env[ENV_KIND].as_str(), "fake");
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
        assert_eq!(plan.env["ANTHROPIC_API_KEY"].as_str(), "explicit");
        assert_eq!(plan.env["CLAUDE_CONFIG_DIR"].as_str(), "/elsewhere");
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
        assert_eq!(plan.env["API_URL"].as_str(), "from-vault");
        assert_eq!(
            plan.env["SHARED"].as_str(),
            "from-caller",
            "the caller wins"
        );
        assert_eq!(plan.env["CLAUDE_CONFIG_DIR"].as_str(), "/home-dir");
    }

    #[test]
    fn a_provided_strip_env_name_is_set_and_not_stripped() {
        let mut fixture = Fixture::new(built_in("claude"));
        fixture.home = Some(PathBuf::from("/home-dir"));
        fixture.provided = provided(&[("ANTHROPIC_API_KEY", "granted")]);
        let plan = fixture.first(None);
        assert_eq!(plan.env["ANTHROPIC_API_KEY"].as_str(), "granted");
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
        for name in [
            "MASSORCA_X",
            "MASSORCA_AGENT_TOKEN",
            "CLAUDE_CONFIG_DIR",
            "NODE_OPTIONS",
            "ANTHROPIC_BASE_URL",
        ] {
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
    fn a_run_holding_secrets_refuses_a_dangerous_caller_variable() {
        let mut fixture = Fixture::new(built_in("claude"));
        fixture.caller_env = BTreeMap::from([("NODE_OPTIONS".to_owned(), "--x".to_owned())]);
        assert!(
            fixture
                .plan(Run::First {
                    prompt: None,
                    new_ref: "r-1"
                })
                .is_ok()
        );
        fixture.provided = provided(&[("API_URL", "from-vault")]);
        let refused = fixture
            .plan(Run::First {
                prompt: None,
                new_ref: "r-1",
            })
            .unwrap_err();
        assert_eq!(
            refused,
            LaunchError::DangerousCallerEnv("NODE_OPTIONS".to_owned())
        );
    }

    #[test]
    fn a_run_marked_only_by_inheritance_refuses_a_dangerous_caller_variable_and_marks_its_terminal()
    {
        let mut fixture = Fixture::new(built_in("claude"));
        fixture.holds_secrets = true;
        let plan = fixture.first(None);
        assert!(plan.holds_secrets, "the terminal is created marked");
        fixture.caller_env =
            BTreeMap::from([("https_proxy".to_owned(), "http://proxy.invalid".to_owned())]);
        let refused = fixture
            .plan(Run::First {
                prompt: None,
                new_ref: "r-1",
            })
            .unwrap_err();
        assert_eq!(
            refused,
            LaunchError::DangerousCallerEnv("https_proxy".to_owned())
        );
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
