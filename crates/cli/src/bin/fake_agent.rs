//! `massorca-fake-agent`: a stand-in for an agent CLI, for MassOrca's own tests (ADR 0057).
//!
//! It prints `fake-agent: ...` lines, takes prompts from its arguments and then from
//! stdin, and reports its activity through `massorca agent hook` when `MASSORCA_BIN`
//! names that program. It prints whether a credential file exists, never its contents, and whether the
//! variables `FAKE_AGENT_PROBE_ENV` names are set, never their values. It prints the permission mode it was started in
//! (`--mode <id>`) and every live switch it reads (`/mode <id>`, decision 0083), so tests can see which reached it. It
//! can report whether an inherited value reached it, and play a CLI whose saved conversation is gone (decision 0160). It
//! can say whether it got MassOrca's instructions and whether `massorca`'s folder is first on its `PATH` (decision 0161).
//! Started as `chat`, it speaks the tiny structured dialect of core-agents' `TestAdapter` instead (decision 0101).

use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{ExitCode, Stdio};
use std::thread;
use std::time::Duration;

use serde_json::{Map, Value};

/// Every printed line starts with this, so a reader can tell the agent's lines apart.
const PREFIX: &str = "fake-agent: ";
/// The program whose `agent hook` receives the reports. Unset or empty means no reports.
const HOOK_PROGRAM_ENV: &str = "MASSORCA_BIN";
/// The credential home. `<home>/token.json` being a file means credentials were found.
const CREDENTIAL_HOME_ENV: &str = "FAKE_AGENT_HOME";
const CREDENTIAL_FILE: &str = "token.json";
/// Comma-separated variable names whose state (`set`, `empty` or `unset`) is printed at start, never their values.
const PROBE_ENV: &str = "FAKE_AGENT_PROBE_ENV";
/// Comma-separated `NAME=VALUE` pairs: start prints `env NAME foreign` when the variable holds VALUE, else `env NAME own`
/// (decision 0160), so a test can see that an inherited value did not reach the run. Never a value.
const PROBE_FOREIGN_ENV: &str = "FAKE_AGENT_PROBE_FOREIGN";
/// Set to `1`, a run started with `--resume <ID>` prints the vendor's "no conversation" line and exits 1, as a CLI whose
/// saved conversation is gone (decision 0160).
const RESUME_MISSING_ENV: &str = "FAKE_AGENT_RESUME_MISSING";
/// The content of each file that `write:<name>` creates before committing it.
const WRITTEN_CONTENT: &str = "written by fake agent\n";
/// Set to `1`, start prints `path first massorca` when the first `PATH` entry is the folder of `MASSORCA_BIN`, else
/// `path first other` (decision 0161). Never a path.
const PRINT_PATH_ENV: &str = "FAKE_AGENT_PRINT_PATH";
/// What start looks for in `--instructions`: it prints `instructions delegate` when the text names it, else
/// `instructions other`. Never the text.
const DELEGATE_COMMAND: &str = "massorca agent delegate";
/// Set to `1`, `--help` leaves `--instructions` out, as a CLI too old for the instruction channel.
const HELP_WITHOUT_INSTRUCTIONS_ENV: &str = "FAKE_AGENT_HELP_WITHOUT_INSTRUCTIONS";
/// Names a file `--help` appends a line to on each run.
const HELP_MARKER_ENV: &str = "FAKE_AGENT_HELP_MARKER";
/// Comma-separated values `--help` lists as the choices of `--permission-mode`, the way the vendor's help does. Unset, the
/// option is not listed (decision 0164).
const HELP_PERMISSION_CHOICES_ENV: &str = "FAKE_AGENT_HELP_PERMISSION_CHOICES";
/// Set to `1`, `--help` exits 1 and prints nothing, as a CLI whose help cannot be read (decision 0164).
const HELP_FAILS_ENV: &str = "FAKE_AGENT_HELP_FAILS";
/// The exit code of the `crash` command.
const CRASH_EXIT_CODE: u8 = 3;
/// Set to `1`, start prints `home <directory name>` (decision 0059): the account tests use it, other runs print nothing
/// extra.
const PRINT_HOME_ENV: &str = "FAKE_AGENT_PRINT_HOME";
/// Set to `<file>:<key>`, start prints `state <key> present` when `<home>/<file>` is a JSON object with that top-level
/// key, else `state <key> absent` (decision 0099). Never the value.
const PRINT_STATE_ENV: &str = "FAKE_AGENT_PRINT_STATE";
/// Set to `1`, `login` exits 1 without writing the token (decision 0059).
const LOGIN_FAIL_ENV: &str = "FAKE_AGENT_LOGIN_FAIL";
/// Set to `1`, `login` never finishes (decision 0059): the sign-in terminal stays open.
const LOGIN_HANG_ENV: &str = "FAKE_AGENT_LOGIN_HANG";
/// Set to `1`, `login` writes an expired token, so the check that follows reports expired.
const LOGIN_EXPIRED_ENV: &str = "FAKE_AGENT_LOGIN_EXPIRED";
/// Set to `1`, `login` writes its token and then never finishes: a sign-in whose login is done but whose terminal is left open.
const LOGIN_HANG_AFTER_TOKEN_ENV: &str = "FAKE_AGENT_LOGIN_HANG_AFTER_TOKEN";
/// Names a file that `logout` writes outside the home, so a test can see that the sign-out ran.
const LOGOUT_MARKER_ENV: &str = "FAKE_AGENT_LOGOUT_MARKER";
/// Set to `1`, `logout` never finishes once it has written its marker, so a sign-out is still running when a test looks.
const LOGOUT_HANG_ENV: &str = "FAKE_AGENT_LOGOUT_HANG";
/// The token `login` writes when it is told to expire it.
const EXPIRED_LOGIN_TOKEN: &str = r#"{"fake":true,"expired":true}"#;
/// What `login` writes into the token file.
const FAKE_LOGIN_TOKEN: &str = r#"{"fake":true}"#;
/// A token containing this is expired: `whoami` then reports `expired`.
const EXPIRED_MARK: &str = r#""expired":true"#;
/// A token containing this is unreachable: `whoami` exits 2 without the word `expired`, as a check that cannot reach its vendor.
const OFFLINE_MARK: &str = r#""offline":true"#;
/// The identity `whoami` reports for a token that names none.
const DEFAULT_IDENTITY: &str = "fake@example.com";

/// A stdin line starting with this switches the permission mode; the rest of the line is the mode.
const MODE_SWITCH: &str = "/mode ";
/// Names a file the `chat` mode waits for after the first part of each reply, so a test can subscribe in between.
const CHAT_GO_FILE_ENV: &str = "FAKE_CHAT_GO_FILE";
/// Set to `1`, the `chat` mode never reads its input, as a CLI that is stuck.
const CHAT_DEAF_ENV: &str = "FAKE_CHAT_DEAF";
/// How long the `chat` mode waits for that file before it replies anyway.
const CHAT_GO_WAIT: Duration = Duration::from_secs(60);

/// The lines of standard input, read as they arrive.
type Input = io::Lines<io::StdinLock<'static>>;

fn main() -> ExitCode {
    // The sign-in and check of an account (decision 0059) are subcommands: `login` and `whoami`, run in the account home.
    match env::args().nth(1).as_deref() {
        Some("login") => return login(),
        Some("whoami") => return whoami(),
        Some("whoami-json") => return whoami_json(),
        Some("logout") => return logout(),
        Some("hang") => return hang(),
        Some("hang-child") => return hang_child(),
        Some("chat") => return chat(),
        Some("--help") => return help(),
        _ => {}
    }
    let arguments = Arguments::parse(env::args().skip(1));
    if let Some(id) = &arguments.resume
        && env::var_os(RESUME_MISSING_ENV).is_some_and(|value| value == "1")
    {
        say(&format!("No conversation found with session ID: {id}"));
        return ExitCode::from(1);
    }
    let agent = Agent::new(&arguments);
    let mut input: Input = io::stdin().lock().lines();
    agent.start(&arguments);
    if let Some(prompt) = &arguments.prompt
        && let Flow::Finish(code) = agent.handle_prompt(prompt, &mut input)
    {
        return code;
    }
    while let Some(line) = next_line(&mut input) {
        match line.as_str() {
            "" => {}
            "exit" => {
                say("bye");
                return ExitCode::SUCCESS;
            }
            "crash" => return ExitCode::from(CRASH_EXIT_CODE),
            text if text.starts_with(MODE_SWITCH) => {
                say(&format!(
                    "mode switched {}",
                    text[MODE_SWITCH.len()..].trim()
                ));
            }
            text => {
                if let Flow::Finish(code) = agent.handle_prompt(text, &mut input) {
                    return code;
                }
            }
        }
    }
    ExitCode::SUCCESS
}

/// The command line: `[--session-id <ID>] [--resume <ID>] [--mode <ID>] [--model <ID>] [--effort <LEVEL>] [--query=<TEXT>]
/// [PROMPT WORDS...]`. The flags
/// count only before the first prompt word; every later word belongs to the prompt. `--query=<TEXT>` is the startup
/// query of a kind with the `startup-query` prompt mode (0054): the agent takes it as its prompt, as if it were typed.
#[derive(Debug, Default)]
struct Arguments {
    session_id: Option<String>,
    resume: Option<String>,
    mode: Option<String>,
    /// `--model <ID>` and `--effort <LEVEL>`, the flags a kind declares for a session's model and effort (decision 0094).
    model: Option<String>,
    effort: Option<String>,
    /// `--instructions <TEXT>`, the channel a test kind declares for MassOrca's per-launch instructions (decision 0161).
    instructions: Option<String>,
    prompt: Option<String>,
}

/// The prefix of the startup query argument.
const QUERY_PREFIX: &str = "--query=";

/// The `-c` setting codex reads the reasoning effort from (decision 0094).
const REASONING_EFFORT_CONFIG: &str = "model_reasoning_effort=";

impl Arguments {
    fn parse(mut args: impl Iterator<Item = String>) -> Self {
        let mut parsed = Self::default();
        let mut words = Vec::new();
        let mut query = None;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--session-id" if words.is_empty() => parsed.session_id = args.next(),
                "--resume" if words.is_empty() => parsed.resume = args.next(),
                "--mode" | "--permission-mode" if words.is_empty() => parsed.mode = args.next(),
                "--model" if words.is_empty() => parsed.model = args.next(),
                "--effort" if words.is_empty() => parsed.effort = args.next(),
                "--instructions" if words.is_empty() => parsed.instructions = args.next(),
                // Codex's spellings of the model and the effort (decision 0094); other `-c` settings pass unread.
                "-m" if words.is_empty() => parsed.model = args.next(),
                "-c" if words.is_empty() => {
                    if let Some(level) = args.next().and_then(|setting| {
                        setting
                            .strip_prefix(REASONING_EFFORT_CONFIG)
                            .map(str::to_owned)
                    }) {
                        parsed.effort = Some(level);
                    }
                }
                text if words.is_empty() && text.starts_with(QUERY_PREFIX) => {
                    query = Some(text[QUERY_PREFIX.len()..].to_owned());
                }
                _ => words.push(arg),
            }
        }
        parsed.prompt = if words.is_empty() {
            query
        } else {
            Some(words.join(" "))
        };
        parsed
    }
}

/// What the agent does after a prompt: keep reading input, or finish with an exit code.
enum Flow {
    Next,
    Finish(ExitCode),
}

struct Agent {
    /// The id hook payloads carry: the resumed one, else the one given at start.
    session: Option<String>,
    /// The program that receives hook reports; `None` when `MASSORCA_BIN` is unset.
    hook_program: Option<OsString>,
    /// The working directory, where `write:<name>` creates and commits files.
    cwd: PathBuf,
}

impl Agent {
    fn new(arguments: &Arguments) -> Self {
        Self {
            session: arguments
                .resume
                .clone()
                .or_else(|| arguments.session_id.clone()),
            hook_program: env::var_os(HOOK_PROGRAM_ENV).filter(|program| !program.is_empty()),
            cwd: env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    /// Prints the banner and the session lines, then reports the start.
    fn start(&self, arguments: &Arguments) {
        say("ready");
        if let Some(id) = &arguments.session_id {
            say(&format!("session {id}"));
        }
        if let Some(id) = &arguments.resume {
            say(&format!("resumed {id}"));
        }
        if let Some(mode) = &arguments.mode {
            say(&format!("mode {mode}"));
        }
        if let Some(model) = &arguments.model {
            say(&format!("model {model}"));
        }
        if let Some(effort) = &arguments.effort {
            say(&format!("effort {effort}"));
        }
        if let Some(text) = &arguments.instructions {
            say(if text.contains(DELEGATE_COMMAND) {
                "instructions delegate"
            } else {
                "instructions other"
            });
        }
        if env::var_os(PRINT_PATH_ENV).is_some_and(|value| value == "1") {
            say(if massorca_first_on_path() {
                "path first massorca"
            } else {
                "path first other"
            });
        }
        say(if credentials_found() {
            "credentials found"
        } else {
            "credentials missing"
        });
        if env::var_os(PRINT_HOME_ENV).is_some_and(|value| value == "1") {
            say(&format!("home {}", home_name()));
        }
        if let Some((file, key)) = env::var(PRINT_STATE_ENV)
            .ok()
            .as_deref()
            .and_then(|value| value.split_once(':'))
        {
            let present = credential_home()
                .and_then(|home| fs::read(home.join(file)).ok())
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                .is_some_and(|state| state.get(key).is_some());
            say(&format!(
                "state {key} {}",
                if present { "present" } else { "absent" }
            ));
        }
        for name in probed_env() {
            say(&format!("env {name} {}", env_state(&name)));
        }
        for (name, foreign) in probed_foreign() {
            let held = env::var_os(&name).is_some_and(|value| value == foreign.as_str());
            say(&format!(
                "env {name} {}",
                if held { "foreign" } else { "own" }
            ));
        }
        self.hook("start", None);
    }

    /// Handles one prompt, reading the further lines it needs from `input`.
    fn handle_prompt(&self, text: &str, input: &mut Input) -> Flow {
        self.hook("prompt", None);
        say(&format!("working on {text}"));
        if text.contains("hold") {
            say("holding");
            loop {
                match next_line(input) {
                    None => return Flow::Finish(ExitCode::SUCCESS),
                    Some(line) if line == "release" => break,
                    Some(_) => {}
                }
            }
            say("released");
        }
        if text.contains("ask:") {
            self.hook("ask", None);
            say("question: may I continue? (yes/no)");
            let answer = loop {
                match next_line(input) {
                    None => return Flow::Finish(ExitCode::SUCCESS),
                    Some(line) if !line.is_empty() => break line,
                    Some(_) => {}
                }
            };
            if answer != "yes" {
                self.hook("fail", Some("declined".to_owned()));
                say("declined");
                return Flow::Next;
            }
            self.hook("prompt", None);
        }
        for word in text.split_whitespace() {
            if let Some(name) = word
                .strip_prefix("write:")
                .filter(|name| is_file_name(name))
            {
                self.write_and_commit(name);
            }
        }
        if text.contains("fail") {
            self.hook("fail", Some(format!("failed: {text}")));
            say(&format!("failed {text}"));
        } else {
            self.hook("stop", Some(format!("done: {text}")));
            say(&format!("done {text}"));
        }
        Flow::Next
    }

    /// Writes `<cwd>/<name>` and commits that one file. A failure at either step prints `commit failed`.
    fn write_and_commit(&self, name: &str) {
        let written = fs::write(self.cwd.join(name), WRITTEN_CONTENT).is_ok();
        let message = format!("fake agent: {name}");
        let committed = written
            && self.git(&["add", "--", name])
            && self.git(&[
                "-c",
                "user.name=Fake",
                "-c",
                "user.email=fake@example.invalid",
                "commit",
                "-q",
                "-m",
                &message,
            ]);
        if committed {
            say(&format!("committed {name}"));
        } else {
            say(&format!("commit failed {name}"));
        }
    }

    /// Runs git in the working directory and reports whether it succeeded. Its output is discarded.
    fn git(&self, args: &[&str]) -> bool {
        massorca_process::command("git")
            .args(args)
            .current_dir(&self.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    /// Sends one report to `massorca agent hook <event> <payload>` and waits for it to finish.
    /// Its exit status, its output and a failure to start are all ignored: a status problem never stops the agent.
    fn hook(&self, event: &str, result: Option<String>) {
        let Some(program) = &self.hook_program else {
            return;
        };
        let mut payload = Map::new();
        if let Some(session) = &self.session {
            payload.insert("session_id".to_owned(), Value::String(session.clone()));
        }
        if let Some(result) = result {
            payload.insert("result".to_owned(), Value::String(result));
        }
        let _ = massorca_process::command(program)
            .args(["agent", "hook", event])
            .arg(Value::Object(payload).to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

/// Prints one line with the agent prefix and flushes it.
fn say(text: &str) {
    let mut out = io::stdout().lock();
    let _ = writeln!(out, "{PREFIX}{text}");
    let _ = out.flush();
}

/// Whether `<CREDENTIAL_HOME_ENV>/token.json` is a file. Only the presence is reported.
fn credentials_found() -> bool {
    env::var_os(CREDENTIAL_HOME_ENV)
        .filter(|home| !home.is_empty())
        .is_some_and(|home| Path::new(&home).join(CREDENTIAL_FILE).is_file())
}

/// The sign-in: writes the fake token into the credential home and exits 0; with `FAKE_AGENT_LOGIN_FAIL=1` it exits 1
/// without writing.
fn login() -> ExitCode {
    if env::var_os(LOGIN_FAIL_ENV).is_some_and(|value| value == "1") {
        say("login failed");
        return ExitCode::from(1);
    }
    if env::var_os(LOGIN_HANG_ENV).is_some_and(|value| value == "1") {
        loop {
            thread::sleep(Duration::from_secs(3600));
        }
    }
    let Some(home) = credential_home() else {
        say("no credential home");
        return ExitCode::from(1);
    };
    let token = if env::var_os(LOGIN_EXPIRED_ENV).is_some_and(|value| value == "1") {
        EXPIRED_LOGIN_TOKEN
    } else {
        FAKE_LOGIN_TOKEN
    };
    match fs::write(home.join(CREDENTIAL_FILE), token) {
        Ok(()) => {
            say("signed in");
            if env::var_os(LOGIN_HANG_AFTER_TOKEN_ENV).is_some_and(|value| value == "1") {
                loop {
                    thread::sleep(Duration::from_secs(3600));
                }
            }
            ExitCode::SUCCESS
        }
        Err(_) => {
            say("login failed");
            ExitCode::from(1)
        }
    }
}

/// A credential the vendor would read from the environment (an API key or token). When `FAKE_AGENT_AMBIENT_TOKEN` is
/// set and not empty, the check reports signed in whatever the home holds.
const AMBIENT_TOKEN_ENV: &str = "FAKE_AGENT_AMBIENT_TOKEN";
const AMBIENT_IDENTITY: &str = "ambient@example.com";

/// The check: prints `signed in as <identity>` and exits 0 when the token exists and is not expired; otherwise prints
/// `expired` and exits 1. A token's `email` names its identity; the default identity is used otherwise.
fn whoami() -> ExitCode {
    if env::var_os(AMBIENT_TOKEN_ENV).is_some_and(|value| !value.is_empty()) {
        let _ = writeln!(io::stdout().lock(), "signed in as {AMBIENT_IDENTITY}");
        return ExitCode::SUCCESS;
    }
    let token =
        credential_home().and_then(|home| fs::read_to_string(home.join(CREDENTIAL_FILE)).ok());
    match token {
        Some(text) if text.contains(OFFLINE_MARK) => {
            let _ = writeln!(io::stdout().lock(), "unreachable");
            ExitCode::from(2)
        }
        Some(text) if !text.contains(EXPIRED_MARK) => {
            let identity = email_of(&text).unwrap_or_else(|| DEFAULT_IDENTITY.to_owned());
            let _ = writeln!(io::stdout().lock(), "signed in as {identity}");
            ExitCode::SUCCESS
        }
        Some(_) => {
            let _ = writeln!(io::stdout().lock(), "expired");
            ExitCode::from(1)
        }
        // No token at all: a home with nothing in it is not signed in, and is not an expired login.
        None => {
            let _ = writeln!(io::stdout().lock(), "not signed in");
            ExitCode::from(1)
        }
    }
}

/// A JSON check, as `claude auth status --json` prints one (decision 0098): `{"loggedIn", "email", "orgName"}`. The email
/// is the token's (or the default identity), the organisation its `"org"` field (or `null`); a token containing
/// `"loggedOut":true` prints `{"loggedIn": false}` and still exits 0. No token prints `{"loggedIn": false}` and exits 1.
fn whoami_json() -> ExitCode {
    let token =
        credential_home().and_then(|home| fs::read_to_string(home.join(CREDENTIAL_FILE)).ok());
    let (status, code) = match token {
        Some(text) if text.contains(r#""loggedOut":true"#) => {
            (serde_json::json!({ "loggedIn": false }), ExitCode::SUCCESS)
        }
        Some(text) => {
            let email = email_of(&text).unwrap_or_else(|| DEFAULT_IDENTITY.to_owned());
            let org = text
                .split("\"org\":\"")
                .nth(1)
                .and_then(|rest| rest.split('"').next())
                .map(str::to_owned);
            (
                serde_json::json!({ "loggedIn": true, "email": email, "orgName": org }),
                ExitCode::SUCCESS,
            )
        }
        None => (serde_json::json!({ "loggedIn": false }), ExitCode::from(1)),
    };
    let _ = writeln!(io::stdout().lock(), "{status:#}");
    code
}

/// The credential home, when the variable names one.
fn credential_home() -> Option<PathBuf> {
    env::var_os(CREDENTIAL_HOME_ENV)
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

/// The directory name of the credential home: an account id, never a path. `none` when there is no home.
fn home_name() -> String {
    credential_home()
        .and_then(|home| {
            home.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "none".to_owned())
}

/// The value of the `email` field of a token, when it has one.
fn email_of(text: &str) -> Option<String> {
    text.split("\"email\":\"")
        .nth(1)?
        .split('"')
        .next()
        .map(str::to_owned)
}

/// `--help`: prints the options, `--instructions` among them unless [`HELP_WITHOUT_INSTRUCTIONS_ENV`] is `1`, and
/// appends a line to the file [`HELP_MARKER_ENV`] names, so a test can count the probes (decision 0161).
fn help() -> ExitCode {
    if let Some(marker) = env::var_os(HELP_MARKER_ENV).filter(|path| !path.is_empty())
        && let Ok(mut file) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(marker)
    {
        let _ = writeln!(file, "help");
    }
    if env::var_os(HELP_FAILS_ENV).is_some_and(|value| value == "1") {
        return ExitCode::from(1);
    }
    let mut text = String::from(
        "Usage: massorca-fake-agent [OPTIONS] [PROMPT]
  --session-id <ID>
  --resume <ID>
  --mode <ID>
",
    );
    if env::var_os(HELP_WITHOUT_INSTRUCTIONS_ENV).is_none_or(|value| value != "1") {
        text.push_str(
            "  --instructions <TEXT>
",
        );
    }
    if let Some(choices) = env::var_os(HELP_PERMISSION_CHOICES_ENV) {
        let listed: Vec<String> = choices
            .to_string_lossy()
            .split(',')
            .map(|choice| format!("\"{}\"", choice.trim()))
            .collect();
        text.push_str(&format!(
            "  --permission-mode <MODE>  Permission mode (choices: {})\n",
            listed.join(", ")
        ));
    }
    let mut stdout = io::stdout().lock();
    let _ = stdout.write_all(text.as_bytes());
    let _ = stdout.flush();
    ExitCode::SUCCESS
}

/// Whether the first `PATH` entry is the folder of the program `MASSORCA_BIN` names (decision 0161).
fn massorca_first_on_path() -> bool {
    let Some(folder) = env::var_os(HOOK_PROGRAM_ENV)
        .map(PathBuf::from)
        .and_then(|bin| bin.parent().map(Path::to_path_buf))
    else {
        return false;
    };
    env::var_os("PATH")
        .and_then(|path| env::split_paths(&path).next())
        .is_some_and(|first| first == folder)
}

/// The names listed in `FAKE_AGENT_PROBE_ENV`.
fn probed_env() -> Vec<String> {
    env::var(PROBE_ENV)
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The `NAME=VALUE` pairs of [`PROBE_FOREIGN_ENV`].
fn probed_foreign() -> Vec<(String, String)> {
    env::var(PROBE_FOREIGN_ENV)
        .unwrap_or_default()
        .split(',')
        .filter_map(|pair| pair.split_once('='))
        .map(|(name, value)| (name.trim().to_owned(), value.to_owned()))
        .filter(|(name, _)| !name.is_empty())
        .collect()
}

/// Whether a variable is set, set to the empty string, or not set. Its value is never printed.
fn env_state(name: &str) -> &'static str {
    match env::var_os(name) {
        None => "unset",
        Some(value) if value.is_empty() => "empty",
        Some(_) => "set",
    }
}

/// A name made of letters, digits, `.`, `_` and `-`, as `write:<name>` requires.
fn is_file_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// The next input line, cleaned. `None` at the end of input, or when a line cannot be read.
fn next_line(input: &mut Input) -> Option<String> {
    input.next()?.ok().map(|line| clean(&line))
}

/// Removes a trailing carriage return, the bracketed-paste markers and surrounding whitespace.
fn clean(line: &str) -> String {
    line.strip_suffix('\r')
        .unwrap_or(line)
        .replace("\x1b[200~", "")
        .replace("\x1b[201~", "")
        .trim()
        .to_owned()
}

/// The sign-out (decision 0059): writes the marker `FAKE_AGENT_LOGOUT_MARKER` names, outside the home, and leaves the
/// token alone. Without a marker it does nothing.
fn logout() -> ExitCode {
    if let Some(marker) = env::var_os(LOGOUT_MARKER_ENV).filter(|path| !path.is_empty())
        && fs::write(&marker, "logged out\n").is_err()
    {
        return ExitCode::from(1);
    }
    if env::var_os(LOGOUT_HANG_ENV).is_some_and(|value| value == "1") {
        loop {
            thread::sleep(Duration::from_secs(3600));
        }
    }
    ExitCode::SUCCESS
}

/// A check that never finishes: it starts a child that inherits the same output pipes, and both sleep. A stopped check
/// must take its child with it, or the pipes stay open and its output is never read to the end.
fn hang() -> ExitCode {
    spawn_hang_child();
    hang_child()
}

/// Starts [`hang_child`] with this program's output, and leaves it running.
fn spawn_hang_child() {
    if let Ok(me) = env::current_exe() {
        let _ = massorca_process::command(me)
            .arg("hang-child")
            .stdin(Stdio::null())
            .spawn();
    }
}

/// The structured mode (decision 0101): one JSON object per line on both streams, the dialect core-agents'
/// `TestAdapter` reads. For each `{"type":"user","text":T}` it writes `{"type":"text","id":"a<turn>","text":"echo: "}`,
/// waits for [`CHAT_GO_FILE_ENV`]'s file when it is set, then writes the same id with `T` (an append) and
/// `{"type":"done","outcome":"success"}`. The message `crash` exits with [`CRASH_EXIT_CODE`]; the message `hang` starts a
/// child that keeps the output open and stops reading, so only a kill of the whole tree ends the run; `leave` starts such
/// a child, ends the turn and exits 0, leaving the child behind. An interrupt is noted. Standard error gets one line at
/// start. It exits when its input ends; with [`CHAT_DEAF_ENV`] it never reads it.
fn chat() -> ExitCode {
    let _ = writeln!(io::stderr(), "fake-chat: started");
    if env::var_os(CHAT_DEAF_ENV).is_some_and(|value| value == "1") {
        return hang_child();
    }
    let go_file = env::var_os(CHAT_GO_FILE_ENV).map(PathBuf::from);
    let mut turn = 0_u32;
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let frame: Value = match serde_json::from_str(&line) {
            Ok(frame) => frame,
            Err(_) => {
                emit(&serde_json::json!({ "type": "notice", "text": "unreadable input" }));
                continue;
            }
        };
        match frame["type"].as_str() {
            Some("user") => {
                let text = frame["text"].as_str().unwrap_or_default();
                match text {
                    "crash" => return ExitCode::from(CRASH_EXIT_CODE),
                    "hang" => return hang(),
                    "leave" => {
                        spawn_hang_child();
                        emit(&serde_json::json!({ "type": "done", "outcome": "success" }));
                        return ExitCode::SUCCESS;
                    }
                    _ => {}
                }
                turn += 1;
                let id = format!("a{turn}");
                emit(&serde_json::json!({ "type": "text", "id": id, "text": "echo: " }));
                if let Some(go) = &go_file {
                    wait_for_file(go);
                }
                emit(&serde_json::json!({ "type": "text", "id": id, "text": text }));
                emit(&serde_json::json!({ "type": "done", "outcome": "success" }));
            }
            Some("interrupt") => {
                emit(&serde_json::json!({ "type": "notice", "text": "interrupted" }));
            }
            _ => emit(&serde_json::json!({ "type": "notice", "text": "unknown input" })),
        }
    }
    ExitCode::SUCCESS
}

/// Writes one JSON line to standard output at once.
fn emit(frame: &Value) {
    let mut stdout = io::stdout().lock();
    let _ = writeln!(stdout, "{frame}");
    let _ = stdout.flush();
}

/// Waits until `path` exists, at most [`CHAT_GO_WAIT`].
fn wait_for_file(path: &Path) {
    let deadline = std::time::Instant::now() + CHAT_GO_WAIT;
    while !path.exists() && std::time::Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
}

/// The sleeping half of [`hang`].
fn hang_child() -> ExitCode {
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}
