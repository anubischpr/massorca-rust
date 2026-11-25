//! Agent kind declarations (decision 0050): the JSON a kind is, and the rules that make one valid.
//!
//! Behaviour is a closed set of mechanisms. An unknown field or mechanism fails to parse; it is never ignored.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use massorca_protocol::codes;

pub use crate::event::{DoneOutcome, EventTarget};
use crate::sessions::AgentError;

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
    /// Sign-in, check and sign-out of the kind's accounts (decision 0059); absent means the kind has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accounts: Option<AccountsDecl>,
    /// The permission modes the vendor documents (decision 0083); empty means the kind declares none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permission_modes: Vec<PermissionModeDecl>,
    /// The mode a run gets when its caller names none. Required with modes, never a `bypass` mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_permission_mode: Option<String>,
    /// The `model` a start may name (decision 0094): how it reaches the launch line, and the choices the picker shows
    /// (decision 0096). Absent means the kind takes no model, and a start that names one is refused with
    /// `agent.option_unsupported`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelDecl>,
    /// The `effort` a start may name, declared as `model` is (decision 0094).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<EffortDecl>,
    /// Inherited variables that no run of the kind and none of its account commands may see (decision 0160): markers
    /// that a process above the service left, such as another agent's session stamp. Names compare without case on
    /// Windows. A name the launch itself sets is set, not stripped.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub strip_inherited_env: Vec<String>,
    /// Where the kind's CLI saves a conversation per agent ref (decision 0160). When the file for a session's ref is not
    /// there, the session cannot resume. Absent means no check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_transcript: Option<ResumeTranscriptDecl>,
    /// A regular expression over a resumed run's output that means the CLI found no conversation for the ref (decision
    /// 0160); such a run that exits with a failure leaves the session unable to resume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_not_found_pattern: Option<String>,
    /// How a run of the kind receives MassOrca's per-launch instructions (decision 0161). Absent means the kind gets no
    /// text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<InstructionsDecl>,
}

/// The placeholder of [`InstructionsDecl::args`] that the instruction text replaces.
pub const TEXT_PLACEHOLDER: &str = "{text}";

/// A kind's channel for MassOrca's per-launch instructions (decision 0161): launch arguments with [`TEXT_PLACEHOLDER`]
/// exactly once, as `["--append-system-prompt", "{text}"]`. The text is passed on a run's own command line only, never
/// written into the vendor's configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstructionsDecl {
    pub args: Vec<String>,
    /// How to tell whether the kind's CLI takes the channel. Absent means it always does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supported_when: Option<SupportProbe>,
}

/// A support check of an instruction channel (decision 0161): the kind's program run with `args`, which supports the
/// channel when it exits 0 and its output contains `contains`. It runs without an account home and with the run's
/// stripped names removed, once per installed program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupportProbe {
    pub args: Vec<String>,
    pub contains: String,
}

impl InstructionsDecl {
    /// The launch arguments that carry `text`.
    #[must_use]
    pub fn args_with(&self, text: &str) -> Vec<String> {
        self.args
            .iter()
            .map(|arg| arg.replace(TEXT_PLACEHOLDER, text))
            .collect()
    }
}

/// Where a kind's CLI keeps one conversation per agent ref (decision 0160). The folder is the run's account home when it
/// used one; else `env`'s value when it is set to an absolute path; else `dir`, where `{home}` is the user's home folder.
/// `glob` is a relative path under that folder: `/`-separated parts, where a part may be `*` (any one folder) and the
/// last part holds `{ref}` once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResumeTranscriptDecl {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
    pub dir: String,
    pub glob: String,
}

/// The `model` option of a kind (decision 0094): how a start's model is passed, and the choices the picker shows
/// (decision 0096). The value follows `flag`, or is substituted for `{value}` in `args`: exactly one of the two.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelDecl {
    /// The flag, starting with `-` and holding no spaces, `=` or control characters: `--model`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flag: Option<String>,
    /// Launch arguments with `{value}` exactly once, as `["-m", "{value}"]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    /// The models the picker shows, in order. Empty when the list is account-scoped and the vendor CLI reports it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<ModelChoiceDecl>,
}

/// One model a kind's picker offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelChoiceDecl {
    /// The value a start names for this model, passed as declared.
    pub id: String,
    pub label: String,
    /// The model the picker preselects. At most one choice has it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub default: bool,
    /// The effort levels this model accepts, by id. Absent: every level the kind declares. Empty: the model takes no
    /// effort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub efforts: Option<Vec<String>>,
}

/// The `effort` option of a kind (decision 0094): how a start's effort is passed, and the levels the picker shows
/// (decision 0096). Declared as `ModelDecl` is, with levels and a default in place of choices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffortDecl {
    /// The flag, as `ModelDecl::flag`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flag: Option<String>,
    /// The launch arguments, as `ModelDecl::args`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    /// The levels a start may name, in order. Empty: any value that passes the format check is passed on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub levels: Vec<EffortLevelDecl>,
    /// The level the picker preselects. It is one of `levels`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

/// One effort level of a kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffortLevelDecl {
    pub id: String,
    pub label: String,
}

/// Longest value of a `model` or an `effort` a start may name.
pub const MAX_OPTION_VALUE_LEN: usize = 128;

/// One permission mode of a kind (decision 0083): how it is passed at launch, and how a running session switches to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PermissionModeDecl {
    /// The vendor's own spelling of the mode (`acceptEdits`, `accept-edits`): 1 to 32 letters, digits and `-`.
    pub id: String,
    /// The vendor's name for the mode.
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Launch arguments that select the mode, after the kind's own. Empty means the mode has no launch flag. With
    /// `launch_values`, `args` holds [`VALUE_PLACEHOLDER`] once, right after its flag (decision 0164).
    #[serde(default)]
    pub args: Vec<String>,
    /// The vendor's spellings of the mode's launch value, in preference order (decision 0164). The launch passes the first
    /// one the CLI's `--help` lists for the flag before the placeholder, else the first one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub launch_values: Vec<String>,
    /// How a running session switches to the mode, when the vendor documents a deterministic way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_switch: Option<LiveSwitchDecl>,
    /// The mode lifts the vendor's permission prompts or sandbox: never the default, and never kept without being named.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bypass: bool,
    /// The vendor documents that a run asked for this mode may start in another one without failing, so launch
    /// arguments never confirm it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub may_fall_back: bool,
    /// A lucide icon name for the mode (`file-pen`): lowercase letters and digits in `-`-separated words (decision 0163).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// How a picker presents the mode (decision 0163): [`MODE_TONE_NORMAL`] or [`MODE_TONE_WARNING`]; every bypass mode
    /// warns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<String>,
    /// Whether pickers offer the mode (decision 0163). An unoffered mode stays declared, so a session reporting it still
    /// shows its name, and a caller naming it is still served.
    #[serde(default = "offered", skip_serializing_if = "is_offered")]
    pub selectable: bool,
}

impl PermissionModeDecl {
    /// The launch arguments with the mode's placeholder replaced by `value` (decision 0164). A mode without launch values
    /// passes `args` as declared.
    pub(crate) fn args_for(&self, value: &str) -> Vec<String> {
        launch_args(None, Some(self.args.as_slice()), value)
    }

    /// The flag the launch value follows in `args`, when the mode declares a placeholder for it (decision 0164).
    pub(crate) fn value_flag(&self) -> Option<&str> {
        let index = self
            .args
            .iter()
            .position(|arg| arg.as_str() == VALUE_PLACEHOLDER)?;
        self.args.get(index.checked_sub(1)?).map(String::as_str)
    }
}

/// The tone of an ordinary mode.
pub const MODE_TONE_NORMAL: &str = "normal";
/// The tone of a mode that lifts the vendor's prompts or sandbox.
pub const MODE_TONE_WARNING: &str = "warning";
/// Longest icon name a mode may declare.
const MAX_ICON_LEN: usize = 64;

fn offered() -> bool {
    true
}

fn is_offered(selectable: &bool) -> bool {
    *selectable
}

/// Whether `icon` reads as a lucide icon name: lowercase letters and digits in `-`-separated words.
fn valid_icon_name(icon: &str) -> bool {
    icon.len() <= MAX_ICON_LEN
        && !icon.is_empty()
        && icon.split('-').all(|word| {
            !word.is_empty()
                && word
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

/// How a live switch reaches the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LiveSwitchType {
    /// The text is typed into the run's terminal, then the kind's submit sequence.
    Input,
}

/// A live switch: `{ "type": "input", "text": "/command mode" }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveSwitchDecl {
    #[serde(rename = "type")]
    pub kind: LiveSwitchType,
    pub text: String,
}

/// A flag that can take a value: it starts with `-` and holds no spaces, `=` or control characters.
fn valid_option_flag(text: &str) -> bool {
    text.len() > 1
        && text.starts_with('-')
        && !text.chars().any(|character| {
            character.is_whitespace() || character.is_control() || character == '='
        })
}

/// A value a start may name for `model` or `effort`: 1 to [`MAX_OPTION_VALUE_LEN`] letters, digits, `-`, `_`, `.`, `:` or
/// `/`, never starting with `-`, so it cannot be read as a flag.
fn valid_option_value(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_OPTION_VALUE_LEN
        && !text.starts_with('-')
        && text
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_.:/".contains(character))
}

/// The placeholder an `args` template uses for the value a start names (decision 0094).
const VALUE_PLACEHOLDER: &str = "{value}";

/// Every rule a `model` or `effort` launch declaration breaks, named by `name`: exactly one of `flag` and `args`, a valid
/// flag, and an `args` template that holds [`VALUE_PLACEHOLDER`] exactly once and no other placeholder.
fn launch_problems(
    name: &str,
    flag: Option<&str>,
    args: Option<&[String]>,
    problems: &mut Vec<String>,
) {
    match (flag, args) {
        (Some(flag), None) => {
            if !valid_option_flag(flag) {
                problems.push(format!(
                    "{name}.flag {flag:?} must start with - and hold no spaces, = or control characters"
                ));
            }
        }
        (None, Some(args)) => {
            let placeholders: usize = args
                .iter()
                .map(|arg| arg.matches(VALUE_PLACEHOLDER).count())
                .sum();
            if placeholders != 1 {
                problems.push(format!(
                    "{name}.args must hold {VALUE_PLACEHOLDER} exactly once, not {placeholders} times"
                ));
            }
            if args
                .iter()
                .any(|arg| arg.replace(VALUE_PLACEHOLDER, "").contains(['{', '}']))
            {
                problems.push(format!(
                    "{name}.args may use {VALUE_PLACEHOLDER} as its only placeholder"
                ));
            }
        }
        _ => problems.push(format!("{name} must declare exactly one of flag and args")),
    }
}

/// The launch arguments that pass `value` on through a declared `flag`, or through `args` with the value substituted.
fn launch_args(flag: Option<&str>, args: Option<&[String]>, value: &str) -> Vec<String> {
    match (flag, args) {
        (Some(flag), _) => vec![flag.to_owned(), value.to_owned()],
        (None, Some(args)) => args
            .iter()
            .map(|arg| arg.replace(VALUE_PLACEHOLDER, value))
            .collect(),
        (None, None) => Vec::new(),
    }
}

/// Refuses a `model` or an `effort` value the format rule does not accept (decision 0094).
fn check_option_value(name: &str, value: &str) -> Result<(), AgentError> {
    if valid_option_value(value) {
        return Ok(());
    }
    Err(AgentError::new(
        codes::REQUEST_INVALID_PARAMS,
        format!(
            "the {name} must be 1 to {MAX_OPTION_VALUE_LEN} letters, digits, - _ . : or /, not starting with -"
        ),
    ))
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
    /// The declared flag and the text as one argument, `<flag>=<text>`. The agent takes the text as its startup query and
    /// submits it itself once its composer is ready, so MassOrca types nothing later (0054; pv96 `hermes-query`).
    StartupQuery,
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
    /// The flag of `flag` and `startup-query` modes.
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
    /// Declared files an import may skip when they are missing (decision 0097). Every other declared file is required;
    /// absent means every declared file is required.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional: Vec<String>,
    /// Where the kind's CLI keeps each declared file by default, for an import without a folder (decision 0097).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<CredentialSource>,
    /// The declared file that holds the CLI's own state, built on import and repaired before each check and launch
    /// instead of being copied raw (decision 0099).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<StateDecl>,
}

/// Largest `max_source_bytes` a state rule may declare.
pub const MAX_STATE_SOURCE_BYTES: u64 = 64 * 1024 * 1024;

/// How an account home's state file is made (decision 0099): a JSON object holding only the `keep` keys of the source
/// file and the `set` keys with their values. The source may be up to `max_source_bytes` long; only the kept keys are
/// ever written into the home.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateDecl {
    /// A declared credential file.
    pub file: String,
    /// Top-level keys copied from the source file when present.
    #[serde(default)]
    pub keep: Vec<String>,
    /// Top-level keys forced to these values, on import and before each check and launch.
    #[serde(default)]
    pub set: BTreeMap<String, serde_json::Value>,
    /// The largest source file the import reads; defaults to 1 MiB.
    #[serde(default = "default_state_source_bytes")]
    pub max_source_bytes: u64,
}

fn default_state_source_bytes() -> u64 {
    crate::credentials::MAX_FILE_BYTES
}

impl CredentialsDecl {
    /// Whether an import needs `file`: every declared file not listed in `optional`.
    pub fn required(&self, file: &str) -> bool {
        !self.optional.iter().any(|name| name == file)
    }

    /// The largest source of `file` an import reads: the state rule's `max_source_bytes` for its file, else 1 MiB.
    pub fn source_cap(&self, file: &str) -> u64 {
        self.state
            .as_ref()
            .filter(|state| state.file == file)
            .map_or(crate::credentials::MAX_FILE_BYTES, |state| {
                state.max_source_bytes
            })
    }
}

/// Where the kind's CLI keeps one declared credential file (decision 0097). `dir` is the default folder; a platform key
/// replaces it on that platform. The only placeholder is `{home}`, the user's home folder. When `env` names a variable
/// that is set to an absolute path, that folder is used instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialSource {
    /// A declared file name.
    pub file: String,
    /// The vendor's own override variable, for example `CLAUDE_CONFIG_DIR`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
    /// The default folder template, for example `{home}/.claude`.
    pub dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub windows: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macos: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linux: Option<String>,
}

/// How a kind signs an account in, checks it and signs it out (decision 0059). Every list is an argument list, passed
/// to the kind's resolved program with the account's home variable set. Nothing here is run at declaration time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountsDecl {
    /// The sign-in arguments. Empty means the kind has no sign-in, and `agent.accounts.add` refuses it.
    #[serde(default)]
    pub login: Vec<String>,
    /// How to tell whether an account is signed in. Absent means its state is `unknown`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<CheckDecl>,
    /// The sign-out arguments, run before an account's home is deleted. Absent means none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logout: Option<Vec<String>>,
    /// Variables removed from the environment of the check, the sign-in and the sign-out, in addition to the
    /// credentials' `strip_env`. Only those commands are affected; agent runs keep their environment (decision 0059).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub strip_env: Vec<String>,
}

/// The check of an account: its arguments and how its exit code and output decide the state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckDecl {
    pub args: Vec<String>,
    /// The exit code that means signed in.
    #[serde(default)]
    pub signed_in_exit: i32,
    /// A regular expression over the check's output; a match means `expired`, whatever the exit code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expired_pattern: Option<String>,
    /// A regular expression over the output when signed in; its first group (or the whole match) is the identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_pattern: Option<String>,
    /// For a check that prints a JSON object (decision 0098): the top-level string fields that make up the identity,
    /// joined with ` / ` in this order, those present only. Declared instead of `identity_pattern`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_json: Option<Vec<String>>,
    /// For a check that prints a JSON object (decision 0098): a top-level field that is `false` when the login is not
    /// signed in, which then needs a sign-in whatever the exit code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed_in_json: Option<String>,
}

impl KindDecl {
    /// Whether the kind can sign an account in: it declares a login, and the home variable that points the login at the
    /// account's home.
    pub fn accounts_supported(&self) -> bool {
        self.credentials.is_some()
            && self
                .accounts
                .as_ref()
                .is_some_and(|accounts| !accounts.login.is_empty())
    }

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

    /// Whether the kind can report `idle`, the state a first prompt waits for (0054): it has a status mechanism and maps
    /// an event to `idle`.
    pub fn reports_idle(&self) -> bool {
        self.status.mechanism != StatusMechanism::None
            && self
                .status
                .events
                .values()
                .any(|target| *target == EventTarget::Idle)
    }

    /// Whether `agent.credentials.import` works for the kind.
    pub fn credentials_supported(&self) -> bool {
        self.credentials
            .as_ref()
            .is_some_and(|credentials| !credentials.files.is_empty())
    }

    /// The declared mode `id`.
    pub fn permission_mode(&self, id: &str) -> Option<&PermissionModeDecl> {
        self.permission_modes.iter().find(|mode| mode.id == id)
    }

    /// The mode a run gets when its caller names none.
    pub fn default_mode(&self) -> Option<&PermissionModeDecl> {
        self.default_permission_mode
            .as_deref()
            .and_then(|id| self.permission_mode(id))
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
        if let Some(accounts) = &self.accounts {
            accounts_problems(self.credentials.as_ref(), accounts, &mut problems);
        }
        self.mode_problems(&mut problems);
        self.model_problems(&mut problems);
        self.effort_problems(&mut problems);
        self.inherited_env_problems(&mut problems);
        self.resume_check_problems(&mut problems);
        self.instructions_problems(&mut problems);
        problems
    }

    /// `instructions.args` holds [`TEXT_PLACEHOLDER`] exactly once, as a whole argument or inside one, and no other
    /// placeholder (decision 0161).
    fn instructions_problems(&self, problems: &mut Vec<String>) {
        let Some(instructions) = &self.instructions else {
            return;
        };
        let placeholders: usize = instructions
            .args
            .iter()
            .map(|arg| arg.matches(TEXT_PLACEHOLDER).count())
            .sum();
        if placeholders != 1 {
            problems.push(format!(
                "instructions.args must hold {TEXT_PLACEHOLDER} exactly once, not {placeholders} times"
            ));
        }
        if instructions
            .args
            .iter()
            .any(|arg| arg.replace(TEXT_PLACEHOLDER, "").contains(['{', '}']))
        {
            problems.push(format!(
                "instructions.args may use {TEXT_PLACEHOLDER} as its only placeholder"
            ));
        }
        if let Some(probe) = &instructions.supported_when {
            if probe.args.is_empty() {
                problems.push("instructions.supported_when.args must not be empty".to_owned());
            }
            no_placeholder("instructions.supported_when.args", &probe.args, problems);
            if probe.contains.trim().is_empty() {
                problems.push("instructions.supported_when.contains must not be empty".to_owned());
            }
        }
    }

    /// `strip_inherited_env` lists variable names, each once (without case, as Windows compares them).
    fn inherited_env_problems(&self, problems: &mut Vec<String>) {
        let mut seen = BTreeSet::new();
        for name in &self.strip_inherited_env {
            if !valid_env_name(name) {
                problems.push(format!(
                    "strip_inherited_env entry {name:?} is not a variable name"
                ));
            } else if !seen.insert(name.to_ascii_uppercase()) {
                problems.push(format!("strip_inherited_env lists {name:?} twice"));
            }
        }
    }

    /// The transcript check and the not-found pattern need a resume form; the folder, the glob and the pattern must be
    /// usable (decision 0160).
    fn resume_check_problems(&self, problems: &mut Vec<String>) {
        if let Some(transcript) = &self.resume_transcript {
            if self.resume.is_none() {
                problems.push("resume_transcript needs resume".to_owned());
            }
            if let Some(env) = &transcript.env
                && !valid_env_name(env)
            {
                problems.push(format!(
                    "resume_transcript.env {env:?} is not a variable name"
                ));
            }
            if !valid_transcript_dir(&transcript.dir) {
                problems.push(format!(
                    "resume_transcript.dir {:?} must be {{home}} or an absolute local folder, without ..",
                    transcript.dir
                ));
            }
            if !valid_transcript_glob(&transcript.glob) {
                problems.push(format!(
                    "resume_transcript.glob {:?} must be a relative path of plain parts or *, whose last part holds {REF_PLACEHOLDER} once",
                    transcript.glob
                ));
            }
        }
        if let Some(pattern) = &self.resume_not_found_pattern {
            if self.resume.is_none() {
                problems.push("resume_not_found_pattern needs resume".to_owned());
            }
            if let Err(error) = regex::Regex::new(pattern) {
                problems.push(format!(
                    "resume_not_found_pattern does not compile: {error}"
                ));
            }
        }
    }

    /// The launch arguments of the `model` and `effort` a start asked for, in the kind's declared flags or args (decisions
    /// 0094, 0096). An option the kind declares nothing for is `agent.option_unsupported`, naming the kind. A value that
    /// could be read as a flag, or is empty, too long or has a space, is `request.invalid_params`, and so is an effort
    /// that the declared levels, or the named model's own efforts, do not list.
    pub(crate) fn option_args(
        &self,
        model: Option<&str>,
        effort: Option<&str>,
    ) -> Result<Vec<String>, AgentError> {
        let mut args = Vec::new();
        if let Some(value) = model {
            let decl = self
                .model
                .as_ref()
                .ok_or_else(|| self.unsupported("model"))?;
            check_option_value("model", value)?;
            args.extend(launch_args(
                decl.flag.as_deref(),
                decl.args.as_deref(),
                value,
            ));
        }
        if let Some(value) = effort {
            let decl = self
                .effort
                .as_ref()
                .ok_or_else(|| self.unsupported("effort"))?;
            check_option_value("effort", value)?;
            self.check_effort(decl, model, value)?;
            args.extend(launch_args(
                decl.flag.as_deref(),
                decl.args.as_deref(),
                value,
            ));
        }
        Ok(args)
    }

    fn unsupported(&self, name: &str) -> AgentError {
        AgentError::new(
            codes::AGENT_OPTION_UNSUPPORTED,
            format!("kind {} declares no flag for a {name}", self.id),
        )
    }

    /// The declared choice `id` of the `model` option.
    fn model_choice(&self, id: &str) -> Option<&ModelChoiceDecl> {
        self.model
            .as_ref()?
            .choices
            .iter()
            .find(|choice| choice.id == id)
    }

    /// Whether the kind declares `id` as one of its effort levels.
    fn effort_declares(&self, id: &str) -> bool {
        self.effort
            .as_ref()
            .is_some_and(|effort| effort.levels.iter().any(|level| level.id == id))
    }

    /// Refuses an effort the declared levels do not list, and one the named model does not accept (decision 0096).
    fn check_effort(
        &self,
        decl: &EffortDecl,
        model: Option<&str>,
        value: &str,
    ) -> Result<(), AgentError> {
        if !decl.levels.is_empty() && !decl.levels.iter().any(|level| level.id == value) {
            let allowed: Vec<&str> = decl.levels.iter().map(|level| level.id.as_str()).collect();
            return Err(AgentError::new(
                codes::REQUEST_INVALID_PARAMS,
                format!("the effort must be one of {}", allowed.join(", ")),
            ));
        }
        let listed = model
            .and_then(|id| self.model_choice(id))
            .and_then(|choice| choice.efforts.as_deref());
        if let (Some(model), Some(listed)) = (model, listed)
            && !listed.iter().any(|id| id == value)
        {
            let message = if listed.is_empty() {
                format!("model {model} takes no effort")
            } else {
                format!(
                    "the effort for model {model} must be one of {}",
                    listed.join(", ")
                )
            };
            return Err(AgentError::new(codes::REQUEST_INVALID_PARAMS, message));
        }
        Ok(())
    }

    /// Every rule the `model` declaration breaks (decision 0094, 0096): its launch, its choices' ids, labels and efforts,
    /// and at most one default.
    fn model_problems(&self, problems: &mut Vec<String>) {
        let Some(model) = &self.model else { return };
        launch_problems(
            "model",
            model.flag.as_deref(),
            model.args.as_deref(),
            problems,
        );
        let mut seen = std::collections::HashSet::new();
        let mut defaults = 0;
        for choice in &model.choices {
            if !valid_option_value(&choice.id) {
                problems.push(format!(
                    "model.choices id {:?} must be 1 to {MAX_OPTION_VALUE_LEN} letters, digits, - _ . : or /, not starting with -",
                    choice.id
                ));
            } else if !seen.insert(choice.id.as_str()) {
                problems.push(format!("model.choices repeats id {:?}", choice.id));
            }
            if choice.label.trim().is_empty() {
                problems.push(format!("model.choices {:?} needs a label", choice.id));
            }
            if choice.default {
                defaults += 1;
            }
            for effort in choice.efforts.iter().flatten() {
                if !self.effort_declares(effort) {
                    problems.push(format!(
                        "model.choices {:?} names effort {effort:?}, which the effort does not declare",
                        choice.id
                    ));
                }
            }
        }
        if defaults > 1 {
            problems.push(format!(
                "model.choices may name one default, not {defaults}"
            ));
        }
    }

    /// Every rule the `effort` declaration breaks (decision 0094, 0096): its launch, its levels' ids and labels, and a
    /// default that is one of the levels.
    fn effort_problems(&self, problems: &mut Vec<String>) {
        let Some(effort) = &self.effort else { return };
        launch_problems(
            "effort",
            effort.flag.as_deref(),
            effort.args.as_deref(),
            problems,
        );
        let mut seen = std::collections::HashSet::new();
        for level in &effort.levels {
            if !valid_option_value(&level.id) {
                problems.push(format!(
                    "effort.levels id {:?} must be 1 to {MAX_OPTION_VALUE_LEN} letters, digits, - _ . : or /, not starting with -",
                    level.id
                ));
            } else if !seen.insert(level.id.as_str()) {
                problems.push(format!("effort.levels repeats id {:?}", level.id));
            }
            if level.label.trim().is_empty() {
                problems.push(format!("effort.levels {:?} needs a label", level.id));
            }
        }
        if let Some(default) = &effort.default
            && !self.effort_declares(default)
        {
            problems.push(format!(
                "effort.default {default:?} is not one of effort.levels"
            ));
        }
    }

    fn mode_problems(&self, problems: &mut Vec<String>) {
        let mut seen = std::collections::HashSet::new();
        for mode in &self.permission_modes {
            let field = format!("permission_modes {:?}", mode.id);
            if !valid_mode_id(&mode.id) {
                problems.push(format!(
                    "{field}: id must be 1 to {MAX_ID_LEN} characters of letters, digits and -"
                ));
            }
            if !seen.insert(mode.id.as_str()) {
                problems.push(format!("{field}: the id is declared twice"));
            }
            if mode.label.trim().is_empty() {
                problems.push(format!("{field}: label must not be empty"));
            }
            launch_values_problems(&field, mode, problems);
            if let Some(switch) = &mode.live_switch
                && (switch.text.trim().is_empty() || switch.text.chars().any(char::is_control))
            {
                problems.push(format!(
                    "{field}: live_switch.text must be one line of text without control characters"
                ));
            }
            if let Some(icon) = &mode.icon
                && !valid_icon_name(icon)
            {
                problems.push(format!(
                    "{field}: icon {icon:?} must be a lucide icon name: lowercase letters and digits in words joined by -"
                ));
            }
            match mode.tone.as_deref() {
                None | Some(MODE_TONE_NORMAL | MODE_TONE_WARNING) => {}
                Some(tone) => problems.push(format!(
                    "{field}: tone {tone:?} must be {MODE_TONE_NORMAL} or {MODE_TONE_WARNING}"
                )),
            }
            if mode.bypass && mode.tone.as_deref() == Some(MODE_TONE_NORMAL) {
                problems.push(format!(
                    "{field}: a bypass mode warns, so its tone must be {MODE_TONE_WARNING}"
                ));
            }
            let appliable = !mode.args.is_empty() || mode.live_switch.is_some();
            if mode.bypass && !appliable {
                problems.push(format!(
                    "{field}: a bypass mode needs its own args or live_switch"
                ));
            }
            if !appliable && self.default_permission_mode.as_deref() != Some(mode.id.as_str()) {
                problems.push(format!(
                    "{field}: a mode with no args and no live_switch can only be the default"
                ));
            }
        }
        match (
            &self.default_permission_mode,
            self.permission_modes.is_empty(),
        ) {
            (None, false) => {
                problems
                    .push("default_permission_mode is required with permission_modes".to_owned());
            }
            (Some(_), true) => {
                problems.push("default_permission_mode needs permission_modes".to_owned());
            }
            (Some(id), false) => match self.permission_mode(id) {
                None => problems.push(format!(
                    "default_permission_mode {id:?} is not one of the permission_modes"
                )),
                Some(mode) if mode.bypass => problems.push(format!(
                    "default_permission_mode {id:?} is a bypass mode, which is never the default"
                )),
                Some(_) => {}
            },
            (None, true) => {}
        }
    }

    fn prompt_problems(&self, problems: &mut Vec<String>) {
        let flag = self.prompt.flag.as_deref();
        match self.prompt.mode {
            PromptMode::Flag if flag.is_none_or(|flag| flag.trim().is_empty()) => {
                problems.push("prompt.flag is required in flag mode".to_owned());
            }
            PromptMode::StartupQuery if flag.is_none_or(|flag| flag.trim().is_empty()) => {
                problems.push("prompt.flag is required in startup-query mode".to_owned());
            }
            // Why: the flag and the text form one argument, so a flag with `=` would split it at the wrong place.
            PromptMode::StartupQuery if flag.is_some_and(|flag| flag.contains('=')) => {
                problems.push("prompt.flag must not contain '=' in startup-query mode".to_owned());
            }
            PromptMode::Argv | PromptMode::None if flag.is_some() => {
                problems.push("prompt.flag is only used in flag and startup-query mode".to_owned());
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

fn accounts_problems(
    credentials: Option<&CredentialsDecl>,
    accounts: &AccountsDecl,
    problems: &mut Vec<String>,
) {
    if credentials.is_none() {
        problems.push("accounts needs credentials (the home variable the login uses)".to_owned());
    }
    no_placeholder("accounts.login", &accounts.login, problems);
    if let Some(logout) = &accounts.logout {
        no_placeholder("accounts.logout", logout, problems);
    }
    for name in &accounts.strip_env {
        if !valid_env_name(name) {
            problems.push(format!(
                "accounts.strip_env entry {name:?} is not a variable name"
            ));
        }
    }
    if let Some(check) = &accounts.check {
        no_placeholder("accounts.check.args", &check.args, problems);
        for (field, pattern) in [
            ("expired_pattern", &check.expired_pattern),
            ("identity_pattern", &check.identity_pattern),
        ] {
            if let Some(pattern) = pattern
                && let Err(error) = regex::Regex::new(pattern)
            {
                problems.push(format!("accounts.check.{field} does not compile: {error}"));
            }
        }
        check_json_problems(check, problems);
    }
}

/// The problems of a check's JSON fields (decision 0098): `identity_json` lists at least one field, each a non-empty
/// name once, and is not declared with `identity_pattern`; `signed_in_json` is a non-empty name.
fn check_json_problems(check: &CheckDecl, problems: &mut Vec<String>) {
    if let Some(fields) = &check.identity_json {
        if fields.is_empty() {
            problems.push("accounts.check.identity_json lists no field".to_owned());
        }
        for (index, field) in fields.iter().enumerate() {
            if field.trim().is_empty() {
                problems.push("accounts.check.identity_json has an empty field name".to_owned());
            } else if fields[..index].contains(field) {
                problems.push(format!(
                    "accounts.check.identity_json names {field:?} twice"
                ));
            }
        }
        if check.identity_pattern.is_some() {
            problems.push(
                "accounts.check declares both identity_pattern and identity_json; declare one"
                    .to_owned(),
            );
        }
    }
    if check
        .signed_in_json
        .as_deref()
        .is_some_and(|field| field.trim().is_empty())
    {
        problems.push("accounts.check.signed_in_json is an empty field name".to_owned());
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
    sources_problems(credentials, problems);
    if let Some(state) = &credentials.state {
        state_problems(credentials, state, problems);
    }
}

/// The state rule (decision 0099): `file` is a declared file, `keep` and `set` name plain top-level keys, each once and
/// in only one of them, and the source cap is between 1 byte and [`MAX_STATE_SOURCE_BYTES`].
fn state_problems(credentials: &CredentialsDecl, state: &StateDecl, problems: &mut Vec<String>) {
    if !credentials.files.iter().any(|file| file == &state.file) {
        problems.push(format!(
            "credentials.state.file {:?} is not a declared file",
            state.file
        ));
    }
    let mut seen = BTreeSet::new();
    for key in &state.keep {
        if !valid_state_key(key) {
            problems.push(format!(
                "credentials.state.keep entry {key:?} is not a plain key name"
            ));
        }
        if !seen.insert(key.as_str()) {
            problems.push(format!("credentials.state.keep lists {key:?} twice"));
        }
    }
    for key in state.set.keys() {
        if !valid_state_key(key) {
            problems.push(format!(
                "credentials.state.set key {key:?} is not a plain key name"
            ));
        }
        if seen.contains(key.as_str()) {
            problems.push(format!(
                "credentials.state lists {key:?} in both keep and set"
            ));
        }
    }
    if state.max_source_bytes == 0 || state.max_source_bytes > MAX_STATE_SOURCE_BYTES {
        problems.push(format!(
            "credentials.state.max_source_bytes {} must be between 1 and {MAX_STATE_SOURCE_BYTES}",
            state.max_source_bytes
        ));
    }
}

/// A top-level state key: 1 to 64 ASCII letters, digits, `_`, `-` or `.`, starting with a letter or `_`.
fn valid_state_key(key: &str) -> bool {
    let mut bytes = key.bytes();
    key.len() <= 64
        && bytes
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

/// The import source declarations (decision 0097): `optional` and every source name declared files, each file has at
/// most one source, its variable is a variable name and its templates start with `{home}` or are absolute.
fn sources_problems(credentials: &CredentialsDecl, problems: &mut Vec<String>) {
    let declared = |name: &str| credentials.files.iter().any(|file| file == name);
    let mut seen = BTreeSet::new();
    for name in &credentials.optional {
        if !declared(name) {
            problems.push(format!(
                "credentials.optional entry {name:?} is not a declared file"
            ));
        }
        if !seen.insert(name.as_str()) {
            problems.push(format!("credentials.optional lists {name:?} twice"));
        }
    }
    let mut seen = BTreeSet::new();
    for source in &credentials.sources {
        let file = &source.file;
        if !declared(file) {
            problems.push(format!(
                "credentials.sources entry {file:?} is not a declared file"
            ));
        }
        if !seen.insert(file.as_str()) {
            problems.push(format!("credentials.sources lists {file:?} twice"));
        }
        if let Some(env) = &source.env
            && !valid_env_name(env)
        {
            problems.push(format!(
                "credentials.sources env {env:?} of {file:?} is not a variable name"
            ));
        }
        let templates = [
            ("dir", Some(&source.dir)),
            ("windows", source.windows.as_ref()),
            ("macos", source.macos.as_ref()),
            ("linux", source.linux.as_ref()),
        ];
        for (key, template) in templates {
            if let Some(template) = template
                && !valid_source_template(template)
            {
                problems.push(format!(
                    "credentials.sources {key} {template:?} of {file:?} must start with {{home}} or be a local absolute path"
                ));
            }
        }
    }
}

/// A source folder template: `{home}` alone or followed by a separator, or a local absolute path (`/...`, `C:\...` or
/// `C:/...`), with no other brace. A network or device path (`\\server\...`, `\\?\...`, `\\.\...`, `//server/...`) is
/// refused, so detection never reaches out to another machine.
pub(crate) fn valid_source_template(template: &str) -> bool {
    let rest = if let Some(rest) = template.strip_prefix("{home}") {
        if !(rest.is_empty() || rest.starts_with(['/', '\\'])) {
            return false;
        }
        rest
    } else {
        let bytes = template.as_bytes();
        let drive = bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\');
        if !((template.starts_with('/') && !remote_path(template)) || drive) {
            return false;
        }
        template
    };
    !rest.contains(['{', '}', '\0'])
}

/// A path that starts with two separators: a UNC share (`\\server\...`, `//server/...`) or a Win32 device or verbatim
/// prefix (`\\?\...`, `\\.\...`). Opening one may reach another machine.
pub(crate) fn remote_path(path: &str) -> bool {
    let mut chars = path.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some('/' | '\\'), Some('/' | '\\'))
    )
}

/// `[a-z0-9-]{1,32}`.
pub fn valid_id(id: &str) -> bool {
    (1..=MAX_ID_LEN).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// A mode id: the vendor's own spelling (`acceptEdits`, `accept-edits`), `[A-Za-z0-9-]{1,32}`.
pub fn valid_mode_id(id: &str) -> bool {
    (1..=MAX_ID_LEN).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
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

/// A transcript folder (decision 0160): a source template whose parts after its root are plain names, never `.`, `..`,
/// empty (a second root) or a drive.
pub(crate) fn valid_transcript_dir(dir: &str) -> bool {
    if !valid_source_template(dir) {
        return false;
    }
    let rest = match dir.strip_prefix("{home}") {
        Some(rest) => rest,
        None if dir.starts_with('/') => dir,
        None => &dir[2..],
    };
    let mut parts = rest.split(['/', '\\']);
    // The root itself: `{home}` is followed by a separator, an absolute path starts with one.
    if parts.next().is_some_and(|first| !first.is_empty()) {
        return false;
    }
    let parts: Vec<&str> = parts.collect();
    let trailing = usize::from(parts.last().is_some_and(|last| last.is_empty()));
    parts[..parts.len() - trailing]
        .iter()
        .all(|part| valid_file_name(part))
}

/// A transcript glob (decision 0160): `/`-separated parts, none empty, `.` or `..`; a part is `*` or a plain name without
/// `*`; the last part is a plain name with `{ref}` exactly once, and no other part has it.
pub(crate) fn valid_transcript_glob(glob: &str) -> bool {
    let parts: Vec<&str> = glob.split('/').collect();
    let Some((last, folders)) = parts.split_last() else {
        return false;
    };
    let folders_ok = folders
        .iter()
        .all(|part| *part == "*" || (valid_file_name(part) && !part.contains(['*', '{', '}'])));
    let filled = last.replacen(REF_PLACEHOLDER, "r", 1);
    folders_ok
        && last.matches(REF_PLACEHOLDER).count() == 1
        && valid_file_name(&filled)
        && !filled.contains(['*', '{', '}'])
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

/// The rules of a permission mode's `launch_values` (decision 0164). Without values, `args` holds no placeholder. With
/// values, `args` holds [`VALUE_PLACEHOLDER`] exactly once, right after a flag, and the values are valid and distinct.
fn launch_values_problems(field: &str, mode: &PermissionModeDecl, problems: &mut Vec<String>) {
    if mode.launch_values.is_empty() {
        no_placeholder(&format!("{field}.args"), &mode.args, problems);
        return;
    }
    let placeholders: usize = mode
        .args
        .iter()
        .map(|arg| arg.matches(VALUE_PLACEHOLDER).count())
        .sum();
    let after_flag = mode.args.iter().enumerate().any(|(index, arg)| {
        arg.as_str() == VALUE_PLACEHOLDER && index > 0 && valid_option_flag(&mode.args[index - 1])
    });
    if placeholders != 1 || !after_flag {
        problems.push(format!(
            "{field}.args must hold {VALUE_PLACEHOLDER} exactly once, right after its flag"
        ));
    }
    if mode
        .args
        .iter()
        .any(|arg| arg.replace(VALUE_PLACEHOLDER, "").contains(['{', '}']))
    {
        problems.push(format!(
            "{field}.args may use {VALUE_PLACEHOLDER} as its only placeholder"
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for value in &mode.launch_values {
        if !valid_option_value(value) {
            problems.push(format!(
                "{field}.launch_values {value:?} must be 1 to {MAX_OPTION_VALUE_LEN} letters, digits, - _ . : or /, not starting with -"
            ));
        }
        if !seen.insert(value.as_str()) {
            problems.push(format!("{field}.launch_values repeats {value:?}"));
        }
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
    fn startup_query_mode_needs_a_flag_that_cannot_take_an_equals_sign() {
        let mut value = minimal();
        value["prompt"] = serde_json::json!({ "mode": "startup-query", "flag": "--query" });
        let kind = parse(value);
        assert_eq!(kind.prompt.mode, PromptMode::StartupQuery);
        assert!(kind.problems().is_empty(), "{:?}", kind.problems());

        let missing =
            problems_of(|value| value["prompt"] = serde_json::json!({ "mode": "startup-query" }));
        assert!(
            missing
                .iter()
                .any(|problem| problem.contains("prompt.flag")),
            "{missing:?}"
        );
        let joined = problems_of(|value| {
            value["prompt"] = serde_json::json!({ "mode": "startup-query", "flag": "--query=" });
        });
        assert!(
            joined.iter().any(|problem| problem.contains("'='")),
            "{joined:?}"
        );
        let separated = problems_of(|value| {
            value["prompt"] = serde_json::json!({ "mode": "startup-query", "flag": "--query", "separator": true });
        });
        assert!(
            separated
                .iter()
                .any(|problem| problem.contains("prompt.separator")),
            "{separated:?}"
        );
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
    fn inherited_strip_names_must_be_variable_names_listed_once() {
        let problems = problems_of(|value| {
            value["strip_inherited_env"] = serde_json::json!(["GOOD_ONE", "BAD-NAME", "good_one"]);
        });
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("strip_inherited_env entry \"BAD-NAME\"")),
            "{problems:?}"
        );
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("strip_inherited_env lists \"good_one\" twice")),
            "{problems:?}"
        );
        let clean = problems_of(|value| {
            value["strip_inherited_env"] = serde_json::json!(["MARKER_ONE", "MARKER_TWO"]);
        });
        assert!(clean.is_empty(), "{clean:?}");
    }

    #[test]
    fn the_resume_checks_need_a_resume_form_a_usable_folder_glob_and_pattern() {
        let problems = problems_of(|value| {
            value["resume_transcript"] =
                serde_json::json!({ "env": "1BAD", "dir": "relative", "glob": "../{ref}.log" });
            value["resume_not_found_pattern"] = serde_json::json!("(");
        });
        for expected in [
            "resume_transcript needs resume",
            "resume_transcript.env",
            "resume_transcript.dir",
            "resume_transcript.glob",
            "resume_not_found_pattern needs resume",
            "resume_not_found_pattern does not compile",
        ] {
            assert!(
                problems.iter().any(|problem| problem.contains(expected)),
                "{expected}: {problems:?}"
            );
        }
        let clean = problems_of(|value| {
            value["resume"] = serde_json::json!(["--resume", "{ref}"]);
            value["session_ref"] = serde_json::json!({ "learn": "session_id" });
            value["resume_transcript"] = serde_json::json!({
                "env": "FAKE_VENDOR_DIR", "dir": "{home}/.fake", "glob": "projects/*/{ref}.jsonl"
            });
            value["resume_not_found_pattern"] = serde_json::json!("No conversation found");
        });
        assert!(clean.is_empty(), "{clean:?}");
    }

    #[test]
    fn the_instructions_channel_holds_the_text_placeholder_once_and_no_other() {
        for (args, expected) in [
            (serde_json::json!(["--append"]), "exactly once, not 0 times"),
            (
                serde_json::json!(["--a", "{text}", "--b", "{text}"]),
                "exactly once, not 2 times",
            ),
            (
                serde_json::json!(["--a", "{text}", "{ref}"]),
                "as its only placeholder",
            ),
        ] {
            let problems = problems_of(|value| {
                value["instructions"] = serde_json::json!({ "args": args.clone() });
            });
            assert!(
                problems.iter().any(|problem| problem.contains(expected)),
                "{expected}: {problems:?}"
            );
        }
        let probe = problems_of(|value| {
            value["instructions"] = serde_json::json!({
                "args": ["--a", "{text}"],
                "supported_when": { "args": [], "contains": " " }
            });
        });
        for expected in [
            "supported_when.args must not be empty",
            "supported_when.contains must not be empty",
        ] {
            assert!(
                probe.iter().any(|problem| problem.contains(expected)),
                "{expected}: {probe:?}"
            );
        }
        let clean = problems_of(|value| {
            value["instructions"] = serde_json::json!({
                "args": ["--append-system-prompt", "{text}"],
                "supported_when": { "args": ["--help"], "contains": "--append-system-prompt" }
            });
        });
        assert!(clean.is_empty(), "{clean:?}");
        let decl = parse({
            let mut value = minimal();
            value["instructions"] = serde_json::json!({ "args": ["--note={text}"] });
            value
        });
        assert_eq!(
            decl.instructions.expect("declared").args_with("hello"),
            ["--note=hello"]
        );
    }

    #[test]
    fn a_transcript_glob_is_relative_with_the_ref_once_in_its_last_part() {
        for good in [
            "{home}",
            "{home}/.claude",
            "/opt/vendor",
            "C:\\vendor\\data",
            "{home}/.fake/",
        ] {
            assert!(valid_transcript_dir(good), "{good}");
        }
        for bad in [
            "relative",
            "{home}/../elsewhere",
            "{home}/./x",
            "{home}//abs",
            "/opt/../etc",
            "{home}/C:/x",
            "C:\\a\\..\\b",
        ] {
            assert!(!valid_transcript_dir(bad), "{bad}");
        }
        for good in ["{ref}.jsonl", "projects/*/{ref}.jsonl", "a/b/{ref}"] {
            assert!(valid_transcript_glob(good), "{good}");
        }
        for bad in [
            "",
            "/abs/{ref}",
            "C:/abs/{ref}",
            "projects/*",
            "{ref}/x.jsonl",
            "a/../{ref}",
            "a//{ref}",
            "a\\b/{ref}",
            "a/*{ref}",
            "{ref}{ref}",
            "p*/{ref}",
        ] {
            assert!(!valid_transcript_glob(bad), "{bad}");
        }
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
    fn a_kind_reports_idle_only_through_a_mechanism_that_maps_idle() {
        assert!(!parse(minimal()).reports_idle(), "no mechanism, no idle");
        let mut mapped = minimal();
        mapped["status"] =
            serde_json::json!({ "mechanism": "env-command", "events": { "start": "idle" } });
        assert!(parse(mapped.clone()).reports_idle());
        mapped["status"] =
            serde_json::json!({ "mechanism": "env-command", "events": { "stop": "done" } });
        assert!(
            !parse(mapped).reports_idle(),
            "a mechanism that never maps idle"
        );
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

    fn with_accounts(accounts: serde_json::Value) -> Vec<String> {
        problems_of(|value| {
            value["credentials"] =
                serde_json::json!({ "home_env": "FAKE_AGENT_HOME", "files": ["token.json"] });
            value["accounts"] = accounts.clone();
        })
    }

    #[test]
    fn the_json_fields_of_a_check_are_validated() {
        let check = |fields: serde_json::Value| {
            with_accounts(serde_json::json!({ "login": ["login"], "check": fields }))
        };
        let fine = check(serde_json::json!({
            "args": ["status"], "identity_json": ["email", "orgName"], "signed_in_json": "loggedIn"
        }));
        assert!(fine.is_empty(), "{fine:?}");
        let empty = check(serde_json::json!({ "args": ["status"], "identity_json": [] }));
        assert_eq!(empty, ["accounts.check.identity_json lists no field"]);
        let twice = check(
            serde_json::json!({ "args": ["status"], "identity_json": ["email", "", "email"] }),
        );
        assert_eq!(
            twice,
            [
                "accounts.check.identity_json has an empty field name",
                "accounts.check.identity_json names \"email\" twice"
            ]
        );
        let both = check(serde_json::json!({
            "args": ["status"], "identity_json": ["email"], "identity_pattern": "x"
        }));
        assert_eq!(
            both,
            ["accounts.check declares both identity_pattern and identity_json; declare one"]
        );
        let blank = check(serde_json::json!({ "args": ["status"], "signed_in_json": " " }));
        assert_eq!(
            blank,
            ["accounts.check.signed_in_json is an empty field name"]
        );
    }

    #[test]
    fn an_accounts_block_with_login_check_and_logout_is_valid() {
        let problems = with_accounts(serde_json::json!({
            "login": ["login"],
            "check": {
                "args": ["whoami"],
                "expired_pattern": "expired",
                "identity_pattern": "signed in as (\\S+)"
            },
            "logout": ["logout"]
        }));
        assert!(problems.is_empty(), "{problems:?}");
        let kind = parse(serde_json::json!({
            "id": "fake", "name": "Fake", "program": ["fake-agent"],
            "credentials": { "home_env": "FAKE_AGENT_HOME", "files": ["token.json"] },
            "accounts": { "login": ["login"], "check": { "args": ["whoami"] } }
        }));
        let check = kind.accounts.as_ref().unwrap().check.as_ref().unwrap();
        assert_eq!(check.signed_in_exit, 0, "the default exit is 0");
        assert!(kind.accounts_supported());
    }

    #[test]
    fn accounts_need_a_credential_home_to_point_the_login_at() {
        let problems = problems_of(|value| {
            value["accounts"] = serde_json::json!({ "login": ["login"] });
        });
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("accounts needs credentials")),
            "{problems:?}"
        );
    }

    #[test]
    fn a_pattern_that_does_not_compile_is_refused_by_name() {
        let problems = with_accounts(serde_json::json!({
            "login": ["login"],
            "check": { "args": ["whoami"], "expired_pattern": "(" }
        }));
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("accounts.check.expired_pattern")),
            "{problems:?}"
        );
    }

    #[test]
    fn an_accounts_strip_entry_that_is_not_a_variable_name_is_refused() {
        let problems = with_accounts(serde_json::json!({
            "login": ["login"],
            "strip_env": ["ANTHROPIC_PROFILE", "BAD-NAME"]
        }));
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("accounts.strip_env entry \"BAD-NAME\"")),
            "{problems:?}"
        );
        let fine = with_accounts(serde_json::json!({
            "login": ["login"],
            "strip_env": ["ANTHROPIC_PROFILE"]
        }));
        assert!(fine.is_empty(), "{fine:?}");
    }

    #[test]
    fn a_kind_without_a_login_has_no_sign_in() {
        let kind = parse(serde_json::json!({
            "id": "fake", "name": "Fake", "program": ["fake-agent"],
            "credentials": { "home_env": "FAKE_AGENT_HOME", "files": ["token.json"] },
            "accounts": { "check": { "args": ["whoami"] } }
        }));
        assert!(kind.problems().is_empty(), "{:?}", kind.problems());
        assert!(!kind.accounts_supported());
    }
    fn with_modes(modes: serde_json::Value, default: Option<&str>) -> Vec<String> {
        problems_of(|value| {
            value["permission_modes"] = modes.clone();
            if let Some(default) = default {
                value["default_permission_mode"] = serde_json::json!(default);
            }
        })
    }

    fn two_modes() -> serde_json::Value {
        serde_json::json!([
            { "id": "careful", "label": "Careful", "args": ["--mode", "careful"] },
            {
                "id": "wild", "label": "Wild", "args": ["--mode", "wild"], "bypass": true,
                "live_switch": { "type": "input", "text": "/mode wild" }
            }
        ])
    }

    #[test]
    fn declared_modes_with_a_default_that_is_not_bypass_are_valid() {
        assert!(with_modes(two_modes(), Some("careful")).is_empty());
        let kind = parse(serde_json::json!({
            "id": "fake", "name": "Fake", "program": ["fake-agent"],
            "permission_modes": two_modes(), "default_permission_mode": "careful"
        }));
        assert_eq!(
            kind.default_mode().map(|mode| mode.id.as_str()),
            Some("careful")
        );
        let wild = kind.permission_mode("wild").unwrap();
        assert!(wild.bypass);
        assert_eq!(
            wild.live_switch,
            Some(LiveSwitchDecl {
                kind: LiveSwitchType::Input,
                text: "/mode wild".to_owned()
            })
        );
        assert!(
            parse(minimal()).permission_modes.is_empty(),
            "a kind may declare none"
        );
    }

    #[test]
    fn mode_ids_are_unique_and_the_default_is_one_of_them() {
        let twice = with_modes(
            serde_json::json!([
                { "id": "a", "label": "A", "args": ["-a"] },
                { "id": "a", "label": "A again", "args": ["-b"] }
            ]),
            Some("a"),
        );
        assert!(
            twice.iter().any(|p| p.contains("declared twice")),
            "{twice:?}"
        );
        let missing = with_modes(two_modes(), Some("other"));
        assert!(
            missing.iter().any(|p| p.contains("not one of")),
            "{missing:?}"
        );
        let none = with_modes(two_modes(), None);
        assert!(none.iter().any(|p| p.contains("is required")), "{none:?}");
        let orphan = problems_of(|value| value["default_permission_mode"] = serde_json::json!("a"));
        assert!(
            orphan.iter().any(|p| p.contains("needs permission_modes")),
            "{orphan:?}"
        );
        let bad_id = with_modes(
            serde_json::json!([{ "id": "a b", "label": "A", "args": ["-a"] }]),
            Some("a b"),
        );
        assert!(
            bad_id.iter().any(|p| p.contains("id must be")),
            "{bad_id:?}"
        );
    }

    #[test]
    fn the_default_is_never_a_bypass_mode() {
        let problems = with_modes(two_modes(), Some("wild"));
        assert!(
            problems.iter().any(|p| p.contains("bypass mode")),
            "{problems:?}"
        );
    }

    #[test]
    fn a_mode_must_be_selectable_and_its_switch_one_clean_line() {
        let bypass = with_modes(
            serde_json::json!([
                { "id": "safe", "label": "Safe" },
                { "id": "open", "label": "Open", "bypass": true }
            ]),
            Some("safe"),
        );
        assert!(
            bypass.iter().any(|p| p.contains("needs its own args")),
            "{bypass:?}"
        );
        let unselectable = with_modes(
            serde_json::json!([
                { "id": "safe", "label": "Safe", "args": ["-s"] },
                { "id": "quiet", "label": "Quiet" }
            ]),
            Some("safe"),
        );
        assert!(
            unselectable
                .iter()
                .any(|p| p.contains("can only be the default")),
            "{unselectable:?}"
        );
        let control = with_modes(
            serde_json::json!([{
                "id": "safe", "label": "Safe",
                "live_switch": { "type": "input", "text": "/mode safe[A" }
            }]),
            Some("safe"),
        );
        assert!(
            control.iter().any(|p| p.contains("live_switch.text")),
            "{control:?}"
        );
        let label = with_modes(
            serde_json::json!([{ "id": "safe", "label": " ", "args": ["-s"] }]),
            Some("safe"),
        );
        assert!(label.iter().any(|p| p.contains("label")), "{label:?}");
        let placeholder = with_modes(
            serde_json::json!([{ "id": "safe", "label": "S", "args": ["{ref}"] }]),
            Some("safe"),
        );
        assert!(
            placeholder.iter().any(|p| p.contains("args")),
            "{placeholder:?}"
        );
    }

    #[test]
    fn a_mode_presentation_names_a_known_tone_and_a_lucide_icon() {
        let good = with_modes(
            serde_json::json!([
                { "id": "safe", "label": "Safe", "args": ["-s"], "icon": "file-pen", "tone": "normal" },
                { "id": "open", "label": "Open", "args": ["-o"], "bypass": true, "icon": "shield-alert", "tone": "warning" },
                { "id": "hush", "label": "Hush", "args": ["-h"], "selectable": false }
            ]),
            Some("safe"),
        );
        assert!(good.is_empty(), "{good:?}");
        let tone = with_modes(
            serde_json::json!([{ "id": "safe", "label": "Safe", "args": ["-s"], "tone": "loud" }]),
            Some("safe"),
        );
        assert!(tone.iter().any(|p| p.contains("tone")), "{tone:?}");
        let calm_bypass = with_modes(
            serde_json::json!([
                { "id": "safe", "label": "Safe", "args": ["-s"] },
                { "id": "open", "label": "Open", "args": ["-o"], "bypass": true, "tone": "normal" }
            ]),
            Some("safe"),
        );
        assert!(
            calm_bypass.iter().any(|p| p.contains("bypass mode warns")),
            "{calm_bypass:?}"
        );
        for icon in [
            "Shield",
            "shield alert",
            "-shield",
            "shield-",
            "shield--alert",
            "",
        ] {
            let found = with_modes(
                serde_json::json!([{ "id": "safe", "label": "Safe", "args": ["-s"], "icon": icon }]),
                Some("safe"),
            );
            assert!(
                found.iter().any(|p| p.contains("icon")),
                "{icon:?}: {found:?}"
            );
        }
        let kind = parse({
            let mut value = minimal();
            value["permission_modes"] = serde_json::json!([
                { "id": "safe", "label": "Safe", "args": ["-s"] },
                { "id": "hush", "label": "Hush", "args": ["-h"], "selectable": false }
            ]);
            value["default_permission_mode"] = serde_json::json!("safe");
            value
        });
        assert!(kind.permission_mode("safe").unwrap().selectable);
        assert!(!kind.permission_mode("hush").unwrap().selectable);
    }

    #[test]
    fn a_mode_may_say_that_the_vendor_falls_back_from_it() {
        let mut value = minimal();
        value["permission_modes"] = serde_json::json!([
            { "id": "auto", "label": "Auto", "args": ["--auto"], "may_fall_back": true },
            { "id": "plan", "label": "Plan", "args": ["--plan"] }
        ]);
        value["default_permission_mode"] = serde_json::json!("auto");
        let kind = parse(value);
        assert!(kind.problems().is_empty(), "{:?}", kind.problems());
        assert!(kind.permission_mode("auto").unwrap().may_fall_back);
        assert!(!kind.permission_mode("plan").unwrap().may_fall_back);
    }

    #[test]
    fn an_unknown_live_switch_type_or_mode_field_does_not_parse() {
        let mut value = minimal();
        value["permission_modes"] = serde_json::json!([{
            "id": "a", "label": "A", "live_switch": { "type": "keys", "text": "x" }
        }]);
        assert!(KindDecl::from_json(&value.to_string()).is_err());
        value["permission_modes"] = serde_json::json!([{ "id": "a", "label": "A", "flag": "-a" }]);
        assert!(KindDecl::from_json(&value.to_string()).is_err());
    }

    /// A kind that takes a model and an effort, declared as given, on the minimal declaration.
    fn options_kind(model: serde_json::Value, effort: serde_json::Value) -> KindDecl {
        let mut value = minimal();
        value["model"] = model;
        value["effort"] = effort;
        parse(value)
    }

    /// The code and the message of a refused `option_args`.
    fn refusal(result: Result<Vec<String>, AgentError>) -> (&'static str, String) {
        let error = result.expect_err("refused");
        (error.code, error.message)
    }

    #[test]
    fn a_model_or_effort_declares_a_flag_or_args_with_the_value_once() {
        let flag = options_kind(
            serde_json::json!({ "flag": "--model" }),
            serde_json::json!({ "flag": "--effort" }),
        );
        assert!(flag.problems().is_empty(), "{:?}", flag.problems());
        assert_eq!(
            flag.option_args(Some("opus"), Some("high"))
                .expect("launch arguments"),
            ["--model", "opus", "--effort", "high"]
        );

        let args = options_kind(
            serde_json::json!({ "args": ["-m", "{value}"] }),
            serde_json::json!({ "args": ["-c", "model_reasoning_effort={value}"] }),
        );
        assert!(args.problems().is_empty(), "{:?}", args.problems());
        assert_eq!(
            args.option_args(Some("gpt-5.5"), Some("high"))
                .expect("launch arguments"),
            ["-m", "gpt-5.5", "-c", "model_reasoning_effort=high"]
        );

        for model in [
            serde_json::json!({ "args": ["-m"] }),
            serde_json::json!({ "args": ["{value}", "--x", "{value}"] }),
            serde_json::json!({ "args": ["-m", "{model}"] }),
            serde_json::json!({ "args": [] }),
            serde_json::json!({ "flag": "--model", "args": ["-m", "{value}"] }),
            serde_json::json!({}),
        ] {
            let problems =
                options_kind(model.clone(), serde_json::json!({ "flag": "--effort" })).problems();
            assert!(
                problems.iter().any(|problem| problem.starts_with("model")),
                "{model}: {problems:?}"
            );
        }
        let effort = options_kind(
            serde_json::json!({ "flag": "--model" }),
            serde_json::json!({ "flag": "--effort", "args": ["-c", "{value}"] }),
        );
        assert!(
            effort
                .problems()
                .iter()
                .any(|problem| problem.starts_with("effort")),
            "{:?}",
            effort.problems()
        );
    }

    #[test]
    fn model_choices_need_valid_unique_ids_one_default_and_declared_efforts() {
        let effort = serde_json::json!({
            "flag": "--effort",
            "levels": [{ "id": "low", "label": "Low" }, { "id": "high", "label": "High" }]
        });
        let valid = options_kind(
            serde_json::json!({ "flag": "--model", "choices": [
                { "id": "big", "label": "Big", "default": true, "efforts": ["low", "high"] },
                { "id": "small", "label": "Small", "efforts": [] },
                { "id": "plain", "label": "Plain" }
            ]}),
            effort.clone(),
        );
        assert!(valid.problems().is_empty(), "{:?}", valid.problems());

        let cases = [
            (
                "a repeated id",
                serde_json::json!({ "flag": "--model", "choices": [
                    { "id": "a", "label": "A" }, { "id": "a", "label": "B" }
                ]}),
            ),
            (
                "two defaults",
                serde_json::json!({ "flag": "--model", "choices": [
                    { "id": "a", "label": "A", "default": true },
                    { "id": "b", "label": "B", "default": true }
                ]}),
            ),
            (
                "an id with a space",
                serde_json::json!({ "flag": "--model", "choices": [
                    { "id": "gpt 5", "label": "GPT 5" }
                ]}),
            ),
            (
                "an effort the kind does not declare",
                serde_json::json!({ "flag": "--model", "choices": [
                    { "id": "a", "label": "A", "efforts": ["max"] }
                ]}),
            ),
            (
                "an empty label",
                serde_json::json!({ "flag": "--model", "choices": [
                    { "id": "a", "label": " " }
                ]}),
            ),
        ];
        for (case, model) in cases {
            let problems = options_kind(model, effort.clone()).problems();
            assert!(
                problems
                    .iter()
                    .any(|problem| problem.starts_with("model.choices")),
                "{case}: {problems:?}"
            );
        }
    }

    #[test]
    fn effort_levels_need_valid_unique_ids_and_a_default_that_is_one_of_them() {
        let valid = options_kind(
            serde_json::json!({ "flag": "--model" }),
            serde_json::json!({
                "flag": "--effort",
                "levels": [{ "id": "low", "label": "Low" }, { "id": "high", "label": "High" }],
                "default": "high"
            }),
        );
        assert!(valid.problems().is_empty(), "{:?}", valid.problems());

        let cases = [
            (
                "a default that is no level",
                serde_json::json!({
                    "flag": "--effort", "levels": [{ "id": "low", "label": "Low" }], "default": "max"
                }),
            ),
            (
                "a default without levels",
                serde_json::json!({
                    "flag": "--effort", "default": "low"
                }),
            ),
            (
                "a repeated level",
                serde_json::json!({
                    "flag": "--effort",
                    "levels": [{ "id": "low", "label": "Low" }, { "id": "low", "label": "Again" }]
                }),
            ),
            (
                "a level id with a space",
                serde_json::json!({
                    "flag": "--effort", "levels": [{ "id": "very high", "label": "Very high" }]
                }),
            ),
            (
                "an empty label",
                serde_json::json!({
                    "flag": "--effort", "levels": [{ "id": "low", "label": "" }]
                }),
            ),
        ];
        for (case, effort) in cases {
            let problems =
                options_kind(serde_json::json!({ "flag": "--model" }), effort).problems();
            assert!(
                problems.iter().any(|problem| problem.starts_with("effort")),
                "{case}: {problems:?}"
            );
        }
    }

    #[test]
    fn an_effort_outside_the_declared_levels_is_refused_and_names_the_allowed_ids() {
        let kind = options_kind(
            serde_json::json!({ "flag": "--model" }),
            serde_json::json!({
                "flag": "--effort",
                "levels": [{ "id": "low", "label": "Low" }, { "id": "high", "label": "High" }]
            }),
        );
        assert_eq!(
            kind.option_args(None, Some("low")).expect("a level passes"),
            ["--effort", "low"]
        );
        let (code, message) = refusal(kind.option_args(None, Some("ultra")));
        assert_eq!(code, codes::REQUEST_INVALID_PARAMS);
        assert!(message.contains("low, high"), "{message}");

        let free = options_kind(
            serde_json::json!({ "flag": "--model" }),
            serde_json::json!({ "flag": "--effort" }),
        );
        assert_eq!(
            free.option_args(None, Some("anything-1"))
                .expect("no levels declared, so any value passes"),
            ["--effort", "anything-1"]
        );
    }

    #[test]
    fn a_model_with_its_own_efforts_refuses_the_others_for_that_model() {
        let kind = options_kind(
            serde_json::json!({ "flag": "--model", "choices": [
                { "id": "big", "label": "Big", "efforts": ["low", "high"] },
                { "id": "small", "label": "Small", "efforts": [] },
                { "id": "any", "label": "Any" }
            ]}),
            serde_json::json!({ "flag": "--effort", "levels": [
                { "id": "low", "label": "Low" },
                { "id": "medium", "label": "Medium" },
                { "id": "high", "label": "High" }
            ]}),
        );
        assert!(kind.problems().is_empty(), "{:?}", kind.problems());

        let (code, message) = refusal(kind.option_args(Some("big"), Some("medium")));
        assert_eq!(code, codes::REQUEST_INVALID_PARAMS);
        assert!(message.contains("low, high"), "{message}");

        let (code, message) = refusal(kind.option_args(Some("small"), Some("low")));
        assert_eq!(code, codes::REQUEST_INVALID_PARAMS);
        assert!(message.contains("takes no effort"), "{message}");

        assert_eq!(
            kind.option_args(Some("big"), Some("high")).expect("listed"),
            ["--model", "big", "--effort", "high"]
        );
        assert_eq!(
            kind.option_args(Some("small"), None).expect("no effort"),
            ["--model", "small"]
        );
        assert_eq!(
            kind.option_args(Some("any"), Some("medium"))
                .expect("every level"),
            ["--model", "any", "--effort", "medium"]
        );
        assert_eq!(
            kind.option_args(Some("unlisted"), Some("low"))
                .expect("a model the declaration does not list"),
            ["--model", "unlisted", "--effort", "low"]
        );
    }

    #[test]
    fn a_flag_only_declaration_launches_and_refuses_as_it_did_before_the_choices() {
        let kind = options_kind(
            serde_json::json!({ "flag": "--model" }),
            serde_json::json!({ "flag": "--effort" }),
        );
        assert_eq!(
            kind.option_args(Some("opus"), Some("max"))
                .expect("a flag-only effort passes any value the format rule accepts"),
            ["--model", "opus", "--effort", "max"]
        );
        let (code, message) = refusal(kind.option_args(None, Some("bad value!")));
        assert_eq!(code, codes::REQUEST_INVALID_PARAMS);
        assert_eq!(
            message,
            "the effort must be 1 to 128 letters, digits, - _ . : or /, not starting with -"
        );

        let plain = parse(minimal());
        let (code, message) = refusal(plain.option_args(Some("opus"), None));
        assert_eq!(code, codes::AGENT_OPTION_UNSUPPORTED);
        assert_eq!(message, "kind fake declares no flag for a model");
    }

    #[test]
    fn the_built_in_kinds_declare_the_reference_model_and_effort_choices() {
        let claude =
            KindDecl::from_json(include_str!("../kinds/claude.json")).expect("claude parses");
        assert!(claude.problems().is_empty(), "{:?}", claude.problems());
        let model = claude.model.as_ref().expect("claude takes a model");
        assert_eq!(model.flag.as_deref(), Some("--model"));
        let choices: Vec<(&str, bool)> = model
            .choices
            .iter()
            .map(|choice| (choice.id.as_str(), choice.default))
            .collect();
        assert_eq!(
            choices,
            [
                ("fable", false),
                ("opus", false),
                ("sonnet", true),
                ("haiku", false)
            ]
        );
        assert_eq!(model.choices[3].efforts, Some(Vec::new()));
        assert_eq!(model.choices[1].efforts, None);
        let effort = claude.effort.as_ref().expect("claude takes an effort");
        let levels: Vec<&str> = effort
            .levels
            .iter()
            .map(|level| level.id.as_str())
            .collect();
        assert_eq!(levels, ["low", "medium", "high", "xhigh", "max"]);
        assert_eq!(effort.default.as_deref(), Some("high"));

        let codex = KindDecl::from_json(include_str!("../kinds/codex.json")).expect("codex parses");
        assert!(codex.problems().is_empty(), "{:?}", codex.problems());
        let model = codex.model.as_ref().expect("codex takes a model");
        assert_eq!(
            model.args.as_deref(),
            Some(&["-m".to_owned(), "{value}".to_owned()][..])
        );
        assert!(model.choices.iter().all(|choice| !choice.default));
        let ceiling = |id: &str| {
            model
                .choices
                .iter()
                .find(|choice| choice.id == id)
                .and_then(|choice| choice.efforts.as_ref())
                .and_then(|efforts| efforts.last())
                .cloned()
        };
        assert_eq!(ceiling("gpt-5.6-sol").as_deref(), Some("ultra"));
        assert_eq!(ceiling("gpt-5.6-terra").as_deref(), Some("ultra"));
        assert_eq!(ceiling("gpt-5.6-luna").as_deref(), Some("max"));
        assert_eq!(ceiling("gpt-5.5").as_deref(), Some("xhigh"));
        assert_eq!(ceiling("gpt-5.2-codex").as_deref(), Some("xhigh"));
        assert_eq!(
            codex
                .effort
                .as_ref()
                .and_then(|effort| effort.default.as_deref()),
            Some("medium")
        );
        let (code, _) = refusal(codex.option_args(Some("gpt-5.5"), Some("ultra")));
        assert_eq!(code, codes::REQUEST_INVALID_PARAMS);
        assert_eq!(
            codex
                .option_args(Some("gpt-5.6-sol"), Some("ultra"))
                .expect("sol goes up to ultra"),
            ["-m", "gpt-5.6-sol", "-c", "model_reasoning_effort=ultra"]
        );

        let antigravity = KindDecl::from_json(include_str!("../kinds/antigravity.json"))
            .expect("antigravity parses");
        assert!(
            antigravity.problems().is_empty(),
            "{:?}",
            antigravity.problems()
        );
        assert!(
            antigravity
                .model
                .as_ref()
                .is_some_and(|model| model.choices.is_empty())
        );
        let levels: Vec<&str> = antigravity
            .effort
            .as_ref()
            .expect("antigravity takes an effort")
            .levels
            .iter()
            .map(|level| level.id.as_str())
            .collect();
        assert_eq!(levels, ["low", "medium", "high", "xhigh", "max"]);
        assert_eq!(
            antigravity
                .effort
                .as_ref()
                .and_then(|effort| effort.default.as_deref()),
            Some("high")
        );

        let gentle = KindDecl::from_json(include_str!("../kinds/gentle-shell.json"))
            .expect("gentle-shell parses");
        assert!(gentle.model.is_none() && gentle.effort.is_none());
    }

    fn with_sources(credentials: serde_json::Value) -> Vec<String> {
        problems_of(|value| value["credentials"] = credentials.clone())
    }

    #[test]
    fn import_sources_name_declared_files_once_with_home_or_absolute_templates() {
        let good = serde_json::json!({
            "home_env": "FAKE_AGENT_HOME",
            "files": ["token.fake", "extra.fake"],
            "optional": ["extra.fake"],
            "sources": [
                { "file": "token.fake", "env": "FAKE_VENDOR_DIR", "dir": "{home}/.fake",
                  "windows": "C:/Fake", "macos": "/opt/fake", "linux": "{home}" },
                { "file": "extra.fake", "dir": "{home}" }
            ]
        });
        assert!(
            with_sources(good.clone()).is_empty(),
            "{:?}",
            with_sources(good.clone())
        );

        let cases = [
            (
                "/optional",
                serde_json::json!(["other.fake"]),
                "credentials.optional entry \"other.fake\" is not a declared file",
            ),
            (
                "/optional",
                serde_json::json!(["extra.fake", "extra.fake"]),
                "credentials.optional lists \"extra.fake\" twice",
            ),
            (
                "/sources/1/file",
                serde_json::json!("other.fake"),
                "credentials.sources entry \"other.fake\" is not a declared file",
            ),
            (
                "/sources/1/file",
                serde_json::json!("token.fake"),
                "credentials.sources lists \"token.fake\" twice",
            ),
            (
                "/sources/0/env",
                serde_json::json!("NOT A NAME"),
                "credentials.sources env \"NOT A NAME\" of \"token.fake\" is not a variable name",
            ),
            (
                "/sources/0/dir",
                serde_json::json!("relative/.fake"),
                "credentials.sources dir \"relative/.fake\" of \"token.fake\" must start with {home} or be a local absolute path",
            ),
            (
                "/sources/0/windows",
                serde_json::json!("{home}/{user}"),
                "credentials.sources windows \"{home}/{user}\" of \"token.fake\" must start with {home} or be a local absolute path",
            ),
            (
                "/sources/0/linux",
                serde_json::json!("{homes}"),
                "credentials.sources linux \"{homes}\" of \"token.fake\" must start with {home} or be a local absolute path",
            ),
        ];
        let mut cases = cases.to_vec();
        // Network and device paths: detection would reach another machine through them.
        let remote = [
            r"\\server\share",
            r"\\?\C:\Fake",
            r"\\.\pipe\fake",
            "//server/share",
            r"/\server\share",
        ];
        let messages: Vec<String> = remote
            .iter()
            .map(|template| {
                format!(
                    "credentials.sources dir {template:?} of \"token.fake\" must start with {{home}} or be a local absolute path"
                )
            })
            .collect();
        for (template, message) in remote.iter().zip(&messages) {
            cases.push((
                "/sources/0/dir",
                serde_json::json!(template),
                message.as_str(),
            ));
        }
        for (pointer, bad, expected) in cases {
            let mut credentials = good.clone();
            *credentials.pointer_mut(pointer).expect("a field to break") = bad;
            let problems = with_sources(credentials);
            assert!(
                problems.iter().any(|problem| problem == expected),
                "{pointer}: {problems:?}"
            );
        }
        assert!(
            KindDecl::from_json(
                &serde_json::json!({
                    "id": "fake", "name": "Fake", "program": ["fake-agent"],
                    "credentials": { "home_env": "FAKE_AGENT_HOME", "files": ["token.fake"],
                        "sources": [{ "file": "token.fake", "dir": "{home}", "path": "x" }] }
                })
                .to_string()
            )
            .is_err(),
            "an unknown source field does not parse"
        );
    }

    #[test]
    fn a_state_rule_names_a_declared_file_and_plain_keys_once_with_a_sane_cap() {
        let good = serde_json::json!({
            "home_env": "FAKE_AGENT_HOME",
            "files": ["token.fake", "state.fake"],
            "optional": ["state.fake"],
            "state": { "file": "state.fake", "keep": ["account", "theme"],
                       "set": { "onboarded": true }, "max_source_bytes": 2048 }
        });
        assert!(
            with_sources(good.clone()).is_empty(),
            "{:?}",
            with_sources(good.clone())
        );
        let cases = [
            (
                "/state/file",
                serde_json::json!("other.fake"),
                "credentials.state.file \"other.fake\" is not a declared file",
            ),
            (
                "/state/keep",
                serde_json::json!(["theme", "theme"]),
                "credentials.state.keep lists \"theme\" twice",
            ),
            (
                "/state/keep",
                serde_json::json!(["a/b"]),
                "credentials.state.keep entry \"a/b\" is not a plain key name",
            ),
            (
                "/state/keep",
                serde_json::json!(["onboarded"]),
                "credentials.state lists \"onboarded\" in both keep and set",
            ),
            (
                "/state/set",
                serde_json::json!({ "": 1 }),
                "credentials.state.set key \"\" is not a plain key name",
            ),
            (
                "/state/max_source_bytes",
                serde_json::json!(0),
                "credentials.state.max_source_bytes 0 must be between 1 and 67108864",
            ),
            (
                "/state/max_source_bytes",
                serde_json::json!(MAX_STATE_SOURCE_BYTES + 1),
                "credentials.state.max_source_bytes 67108865 must be between 1 and 67108864",
            ),
        ];
        for (pointer, bad, expected) in cases {
            let mut credentials = good.clone();
            *credentials.pointer_mut(pointer).expect("a field to break") = bad;
            let problems = with_sources(credentials);
            assert!(
                problems.iter().any(|problem| problem == expected),
                "{pointer}: {problems:?}"
            );
        }
        let mut unknown = good.clone();
        unknown["state"]["copy"] = serde_json::json!(true);
        assert!(
            KindDecl::from_json(
                &serde_json::json!({
                    "id": "fake", "name": "Fake", "program": ["fake-agent"], "credentials": unknown
                })
                .to_string()
            )
            .is_err(),
            "an unknown state field does not parse"
        );
    }

    #[test]
    fn claude_builds_its_state_file_with_onboarding_done_and_only_three_kept_keys() {
        for text in [
            include_str!("../kinds/claude.json"),
            include_str!("../../../packages/agents/massorca.claude/agents/claude.json"),
        ] {
            let claude = KindDecl::from_json(text).expect("claude parses");
            let credentials = claude.credentials.as_ref().expect("claude has credentials");
            let state = credentials
                .state
                .as_ref()
                .expect("claude declares a state rule");
            assert_eq!(state.file, ".claude.json");
            assert_eq!(state.keep, ["oauthAccount", "theme", "mcpServers"]);
            assert_eq!(
                serde_json::to_value(&state.set).unwrap(),
                serde_json::json!({ "hasCompletedOnboarding": true })
            );
            assert_eq!(state.max_source_bytes, 16 * 1024 * 1024);
            assert!(!credentials.required(".claude.json"));
        }
        for kind in [
            include_str!("../kinds/codex.json"),
            include_str!("../kinds/antigravity.json"),
            include_str!("../kinds/gentle-shell.json"),
        ] {
            let decl = KindDecl::from_json(kind).expect("the kind parses");
            assert!(
                decl.credentials
                    .is_none_or(|credentials| credentials.state.is_none())
            );
        }
    }

    #[test]
    fn claude_and_codex_declare_where_their_clis_keep_each_file() {
        let claude =
            KindDecl::from_json(include_str!("../kinds/claude.json")).expect("claude parses");
        let credentials = claude.credentials.as_ref().expect("claude has credentials");
        assert!(credentials.required(".credentials.json"));
        assert!(!credentials.required(".claude.json"));
        let dirs: Vec<(&str, Option<&str>, &str)> = credentials
            .sources
            .iter()
            .map(|source| {
                (
                    source.file.as_str(),
                    source.env.as_deref(),
                    source.dir.as_str(),
                )
            })
            .collect();
        assert_eq!(
            dirs,
            [
                (
                    ".credentials.json",
                    Some("CLAUDE_CONFIG_DIR"),
                    "{home}/.claude"
                ),
                (".claude.json", Some("CLAUDE_CONFIG_DIR"), "{home}"),
            ]
        );
        let codex = KindDecl::from_json(include_str!("../kinds/codex.json")).expect("codex parses");
        let credentials = codex.credentials.as_ref().expect("codex has credentials");
        assert!(credentials.required("auth.json") && !credentials.required("config.toml"));
        assert!(
            credentials
                .sources
                .iter()
                .all(|source| source.env.as_deref() == Some("CODEX_HOME")
                    && source.dir == "{home}/.codex")
        );
        assert_eq!(credentials.sources.len(), 2);
    }

    #[test]
    fn the_service_drops_exactly_the_markers_the_built_in_kinds_declare() {
        let mut declared: Vec<String> = [
            include_str!("../kinds/claude.json"),
            include_str!("../kinds/codex.json"),
            include_str!("../kinds/antigravity.json"),
            include_str!("../kinds/gentle-shell.json"),
        ]
        .into_iter()
        .flat_map(|json| {
            KindDecl::from_json(json)
                .expect("parses")
                .strip_inherited_env
        })
        .collect();
        declared.sort();
        declared.dedup();
        let mut constant: Vec<String> = crate::launch::INHERITED_VENDOR_MARKERS
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        constant.sort();
        assert_eq!(declared, constant);
    }
}
