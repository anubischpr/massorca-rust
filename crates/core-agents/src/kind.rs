//! Agent kind declarations (decision 0050): the JSON a kind is, and the rules that make one valid.
//!
//! Behaviour is a closed set of mechanisms. An unknown field or mechanism fails to parse; it is never ignored.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub use crate::event::{DoneOutcome, EventTarget};

/// The placeholder replaced by the agent's own conversation id.
pub const REF_PLACEHOLDER: &str = "{ref}";

/// Longest kind id.
pub const MAX_ID_LEN: usize = 32;

/// One agent kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KindDecl {
    /// Lowercase `[a-z0-9-]`, 1 to 32 characters.
    pub id: String,
    pub name: String,
    #[serde(default = "default_icon")]
    pub icon: String,
    /// Program candidates, tried in order: an absolute path, or a bare name searched in `PATH`.
    pub program: Vec<String>,
    /// Launch arguments, before the caller's.
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub prompt: PromptDecl,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_ref: Option<SessionRefDecl>,
    /// Arguments of a new run of an exited session, with `{ref}`; absent means not resumable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume: Option<Vec<String>>,
    #[serde(default)]
    pub status: StatusDecl,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials: Option<CredentialsDecl>,
}

fn default_icon() -> String {
    "bot".to_owned()
}

/// How a first prompt reaches the agent at launch (0054).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PromptMode {
    /// The text is the last argument.
    #[default]
    Argv,
    /// The declared flag, then the text.
    Flag,
    /// The kind cannot take a prompt at launch.
    None,
}

/// How later prompts are typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PasteMode {
    #[default]
    Plain,
    /// Wrapped in bracketed-paste markers, so multi-line text is not submitted line by line.
    Bracketed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptDecl {
    #[serde(default)]
    pub mode: PromptMode,
    /// The flag of `flag` mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flag: Option<String>,
    /// Put `--` before the text in `argv` mode.
    #[serde(default)]
    pub separator: bool,
    #[serde(default)]
    pub paste: PasteMode,
    /// What submits a typed prompt.
    #[serde(default = "default_submit")]
    pub submit: String,
}

impl Default for PromptDecl {
    fn default() -> Self {
        Self {
            mode: PromptMode::Argv,
            flag: None,
            separator: false,
            paste: PasteMode::Plain,
            submit: default_submit(),
        }
    }
}

fn default_submit() -> String {
    "\r".to_owned()
}

/// How MassOrca knows the agent's own conversation id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRefDecl {
    /// Arguments added at the first launch with a generated UUID as `{ref}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assign: Option<Vec<String>>,
    /// The hook payload field that carries the agent's id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learn: Option<String>,
}

/// How a kind's status hooks are installed (0053).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StatusMechanism {
    ClaudeSettings,
    CodexNotify,
    EnvCommand,
    #[default]
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusDecl {
    #[serde(default)]
    pub mechanism: StatusMechanism,
    /// Hook event name to activity. Events not listed are ignored.
    #[serde(default)]
    pub events: BTreeMap<String, EventTarget>,
    /// The payload field of a `done` event that carries the turn's result text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_field: Option<String>,
}

/// Where a kind keeps its login, and what to copy and strip (0056).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialsDecl {
    /// The variable that points the agent at a home directory.
    pub home_env: String,
    /// The files `credentials import` copies; empty means import is unsupported.
    #[serde(default)]
    pub files: Vec<String>,
    /// Variables removed from the run's environment when the profile home is used.
    #[serde(default)]
    pub strip_env: Vec<String>,
}

impl KindDecl {
    /// Parses one declaration. The error text names the field serde rejected.
    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|error| format!("not a valid kind declaration: {error}"))
    }

    /// Whether a new run of an exited session can be started: a resume form, and a way to know the ref.
    pub fn resumable(&self) -> bool {
        self.resume.is_some()
            && self.session_ref.as_ref().is_some_and(|session_ref| {
                session_ref.assign.is_some() || session_ref.learn.is_some()
            })
    }

    /// Whether `agent.credentials.import` works for the kind.
    pub fn credentials_supported(&self) -> bool {
        self.credentials
            .as_ref()
            .is_some_and(|credentials| !credentials.files.is_empty())
    }

    /// Every rule the declaration breaks; empty when it is valid.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if !valid_id(&self.id) {
            problems.push(format!(
                "id {:?} must be 1 to {MAX_ID_LEN} characters of a-z, 0-9 and -",
                self.id
            ));
        }
        if self.name.trim().is_empty() {
            problems.push("name must not be empty".to_owned());
        }
        if self.icon.trim().is_empty() {
            problems.push("icon must not be empty".to_owned());
        }
        if self.program.is_empty() {
            problems.push("program must list at least one candidate".to_owned());
        }
        for program in &self.program {
            if !valid_program(program) {
                problems.push(format!(
                    "program {program:?} must be an absolute path or a bare name"
                ));
            }
        }
        no_placeholder("args", &self.args, &mut problems);
        self.prompt_problems(&mut problems);
        if let Some(session_ref) = &self.session_ref {
            if let Some(assign) = &session_ref.assign {
                needs_placeholder("session_ref.assign", assign, &mut problems);
            }
            if session_ref.learn.as_deref().is_some_and(str::is_empty) {
                problems.push("session_ref.learn must not be empty".to_owned());
            }
        }
        if let Some(resume) = &self.resume {
            needs_placeholder("resume", resume, &mut problems);
        }
        self.status_problems(&mut problems);
        if let Some(credentials) = &self.credentials {
            credentials_problems(credentials, &mut problems);
        }
        problems
    }

    fn prompt_problems(&self, problems: &mut Vec<String>) {
        let flag = self.prompt.flag.as_deref();
        match self.prompt.mode {
            PromptMode::Flag if flag.is_none_or(|flag| flag.trim().is_empty()) => {
                problems.push("prompt.flag is required in flag mode".to_owned());
            }
            PromptMode::Argv | PromptMode::None if flag.is_some() => {
                problems.push("prompt.flag is only used in flag mode".to_owned());
            }
            _ => {}
        }
        if self.prompt.separator && self.prompt.mode != PromptMode::Argv {
            problems.push("prompt.separator is only used in argv mode".to_owned());
        }
        if self.prompt.submit.is_empty() {
            problems.push("prompt.submit must not be empty".to_owned());
        }
    }

    fn status_problems(&self, problems: &mut Vec<String>) {
        if self.status.mechanism == StatusMechanism::None
            && (!self.status.events.is_empty() || self.status.result_field.is_some())
        {
            problems.push("status.events and status.result_field need a mechanism".to_owned());
        }
        if !self.status.events.keys().all(|name| valid_event_name(name)) {
            problems.push(
                "status.events names must be 1 to 64 characters of letters, digits, _, ., : and -"
                    .to_owned(),
            );
        }
        if self
            .status
            .result_field
            .as_deref()
            .is_some_and(str::is_empty)
        {
            problems.push("status.result_field must not be empty".to_owned());
        }
    }
}

fn credentials_problems(credentials: &CredentialsDecl, problems: &mut Vec<String>) {
    if !valid_env_name(&credentials.home_env) {
        problems.push(format!(
            "credentials.home_env {:?} is not a variable name",
            credentials.home_env
        ));
    }
    for file in &credentials.files {
        if !valid_file_name(file) {
            problems.push(format!(
                "credentials.files entry {file:?} must be a plain file name"
            ));
        }
    }
    for name in &credentials.strip_env {
        if !valid_env_name(name) {
            problems.push(format!(
                "credentials.strip_env entry {name:?} is not a variable name"
            ));
        }
    }
}

/// `[a-z0-9-]{1,32}`.
pub fn valid_id(id: &str) -> bool {
    (1..=MAX_ID_LEN).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn valid_program(program: &str) -> bool {
    !program.trim().is_empty()
        && (Path::new(program).is_absolute() || !program.contains(['/', '\\']))
}

/// Event names go into hook commands, so they are limited to characters no shell treats specially.
fn valid_event_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-'))
}

/// `[A-Za-z_][A-Za-z0-9_]*`.
fn valid_env_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn valid_file_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', '\0', ':'])
        && !name.ends_with(['.', ' '])
        && !windows_device_name(name)
}

/// `CON`, `PRN`, `AUX`, `NUL`, `COM1`-`COM9` and `LPT1`-`LPT9`, in any case, with or without an extension: Windows
/// opens the device instead of a file of that name.
fn windows_device_name(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
}

/// Braces in an argument that are not the `{ref}` placeholder.
fn stray_braces(arg: &str) -> bool {
    arg.replace(REF_PLACEHOLDER, "").contains(['{', '}'])
}

fn needs_placeholder(field: &str, args: &[String], problems: &mut Vec<String>) {
    if args.is_empty() {
        problems.push(format!("{field} must not be empty"));
    } else if !args.iter().any(|arg| arg.contains(REF_PLACEHOLDER)) {
        problems.push(format!("{field} must contain {REF_PLACEHOLDER}"));
    }
    if args.iter().any(|arg| stray_braces(arg)) {
        problems.push(format!(
            "{field} may use no placeholder other than {REF_PLACEHOLDER}"
        ));
    }
}

fn no_placeholder(field: &str, args: &[String], problems: &mut Vec<String>) {
    if args.iter().any(|arg| arg.contains(['{', '}'])) {
        problems.push(format!("{field} may not use placeholders"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal() -> serde_json::Value {
        serde_json::json!({ "id": "fake", "name": "Fake agent", "program": ["fake-agent"] })
    }

    fn parse(value: serde_json::Value) -> KindDecl {
        KindDecl::from_json(&value.to_string()).expect("parses")
    }

    fn problems_of(mut patch: impl FnMut(&mut serde_json::Value)) -> Vec<String> {
        let mut value = minimal();
        patch(&mut value);
        parse(value).problems()
    }

    #[test]
    fn a_minimal_declaration_is_valid_with_defaults() {
        let kind = parse(minimal());
        assert!(kind.problems().is_empty(), "{:?}", kind.problems());
        assert_eq!(kind.icon, "bot");
        assert_eq!(kind.prompt.mode, PromptMode::Argv);
        assert_eq!(kind.prompt.submit, "\r");
        assert_eq!(kind.status.mechanism, StatusMechanism::None);
        assert!(!kind.resumable());
        assert!(!kind.credentials_supported());
    }

    #[test]
    fn unknown_fields_and_mechanisms_do_not_parse() {
        let mut extra = minimal();
        extra["yolo"] = serde_json::json!(true);
        assert!(KindDecl::from_json(&extra.to_string()).is_err());

        let mut mechanism = minimal();
        mechanism["status"] = serde_json::json!({ "mechanism": "screen-scrape" });
        let error = KindDecl::from_json(&mechanism.to_string()).expect_err("refused");
        assert!(error.contains("screen-scrape"), "{error}");

        let mut target = minimal();
        target["status"] =
            serde_json::json!({ "mechanism": "env-command", "events": { "x": "done:maybe" } });
        let error = KindDecl::from_json(&target.to_string()).expect_err("refused");
        assert!(error.contains("done:maybe"), "{error}");
    }

    #[test]
    fn ids_follow_the_pattern() {
        assert!(valid_id("gentle-shell"));
        assert!(valid_id("a"));
        assert!(!valid_id(""));
        assert!(!valid_id("Claude"));
        assert!(!valid_id("my_agent"));
        assert!(!valid_id(&"a".repeat(33)));
        let problems = problems_of(|value| value["id"] = serde_json::json!("Bad Id"));
        assert_eq!(problems.len(), 1, "{problems:?}");
    }

    #[test]
    fn program_needs_a_candidate_that_is_absolute_or_bare() {
        assert!(!problems_of(|value| value["program"] = serde_json::json!([])).is_empty());
        assert!(
            !problems_of(|value| value["program"] = serde_json::json!(["bin/agent"])).is_empty()
        );
        assert!(!problems_of(|value| value["program"] = serde_json::json!([""])).is_empty());
    }

    #[test]
    fn flag_mode_needs_a_flag_and_other_modes_refuse_one() {
        let missing = problems_of(|value| value["prompt"] = serde_json::json!({ "mode": "flag" }));
        assert!(
            missing
                .iter()
                .any(|problem| problem.contains("prompt.flag"))
        );
        let stray = problems_of(|value| {
            value["prompt"] = serde_json::json!({ "mode": "argv", "flag": "-p" });
        });
        assert!(stray.iter().any(|problem| problem.contains("prompt.flag")));
        let fine = problems_of(|value| {
            value["prompt"] = serde_json::json!({ "mode": "flag", "flag": "--prompt" });
        });
        assert!(fine.is_empty(), "{fine:?}");
    }

    #[test]
    fn ref_placeholder_rules() {
        let no_ref = problems_of(|value| value["resume"] = serde_json::json!(["--resume"]));
        assert!(no_ref.iter().any(|problem| problem.contains("{ref}")));
        let other =
            problems_of(|value| value["resume"] = serde_json::json!(["--resume", "{ref}", "{id}"]));
        assert!(other.iter().any(|problem| problem.contains("placeholder")));
        let in_args = problems_of(|value| value["args"] = serde_json::json!(["{ref}"]));
        assert!(in_args.iter().any(|problem| problem.contains("args")));
        let assign = problems_of(|value| {
            value["session_ref"] = serde_json::json!({ "assign": ["--session-id"] });
        });
        assert!(
            assign
                .iter()
                .any(|problem| problem.contains("session_ref.assign"))
        );
    }

    #[test]
    fn resumable_needs_a_resume_form_and_a_ref_source() {
        let mut value = minimal();
        value["resume"] = serde_json::json!(["--resume", "{ref}"]);
        assert!(!parse(value.clone()).resumable());
        value["session_ref"] = serde_json::json!({ "learn": "session_id" });
        assert!(parse(value).resumable());
    }

    #[test]
    fn events_without_a_mechanism_are_a_problem() {
        let problems = problems_of(|value| {
            value["status"] = serde_json::json!({ "events": { "stop": "done" } });
        });
        assert!(problems.iter().any(|problem| problem.contains("mechanism")));
    }

    #[test]
    fn event_names_are_safe_in_a_hook_command() {
        let problems = problems_of(|value| {
            value["status"] =
                serde_json::json!({ "mechanism": "env-command", "events": { "a b;rm": "done" } });
        });
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("status.events"))
        );
        let fine = problems_of(|value| {
            value["status"] = serde_json::json!({
                "mechanism": "env-command", "events": { "turn-complete": "done", "Stop": "done" }
            });
        });
        assert!(fine.is_empty(), "{fine:?}");
    }

    #[test]
    fn credential_names_are_checked() {
        let problems = problems_of(|value| {
            value["credentials"] = serde_json::json!({
                "home_env": "1BAD", "files": ["../token.json"], "strip_env": ["A-B"]
            });
        });
        assert_eq!(problems.len(), 3, "{problems:?}");
        for name in [
            "",
            ".",
            "..",
            "a/b",
            "a\\b",
            "/etc/passwd",
            "C:\\token",
            "C:token",
            "\\\\host\\share",
            "nul\0",
            "CON",
            "nul.txt",
            "Com1.json",
            "LPT9",
            "aux",
            "prn.json",
            "token.",
            "token ",
        ] {
            let problems = problems_of(|value| {
                value["credentials"] =
                    serde_json::json!({ "home_env": "FAKE_AGENT_HOME", "files": [name] });
            });
            assert_eq!(problems.len(), 1, "{name:?}: {problems:?}");
        }
        for name in [
            "console.json",
            "com10",
            "lpt",
            ".credentials.json",
            "con-fig",
        ] {
            let problems = problems_of(|value| {
                value["credentials"] =
                    serde_json::json!({ "home_env": "FAKE_AGENT_HOME", "files": [name] });
            });
            assert!(problems.is_empty(), "{name:?}: {problems:?}");
        }
        let fine = problems_of(|value| {
            value["credentials"] =
                serde_json::json!({ "home_env": "FAKE_AGENT_HOME", "files": ["token.json"] });
        });
        assert!(fine.is_empty(), "{fine:?}");
    }
}
