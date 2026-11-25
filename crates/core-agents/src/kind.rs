//! Agent kind declarations (decision 0050): the JSON a kind is, and the rules that make one valid.
//!
//! Behaviour is a closed set of mechanisms. An unknown field or mechanism fails to parse; it is never ignored.

use std::collections::BTreeMap;
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
    /// Launch arguments that select the mode, after the kind's own. Empty means the mode has no launch flag.
    #[serde(default)]
    pub args: Vec<String>,
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
        problems
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
            no_placeholder(&format!("{field}.args"), &mode.args, problems);
            if let Some(switch) = &mode.live_switch
                && (switch.text.trim().is_empty() || switch.text.chars().any(char::is_control))
            {
                problems.push(format!(
                    "{field}: live_switch.text must be one line of text without control characters"
                ));
            }
            let selectable = !mode.args.is_empty() || mode.live_switch.is_some();
            if mode.bypass && !selectable {
                problems.push(format!(
                    "{field}: a bypass mode needs its own args or live_switch"
                ));
            }
            if !selectable && self.default_permission_mode.as_deref() != Some(mode.id.as_str()) {
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
}
