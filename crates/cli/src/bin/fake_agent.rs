//! `massorca-fake-agent`: a stand-in for an agent CLI, for MassOrca's own tests (ADR 0057).
//!
//! It prints `fake-agent: ...` lines, takes prompts from its arguments and then from
//! stdin, and reports its activity through `massorca agent hook` when `MASSORCA_BIN`
//! names that program. It prints whether a credential file exists, never its contents, and whether the
//! variables `FAKE_AGENT_PROBE_ENV` names are set, never their values.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

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
/// The content of each file that `write:<name>` creates before committing it.
const WRITTEN_CONTENT: &str = "written by fake agent\n";
/// The exit code of the `crash` command.
const CRASH_EXIT_CODE: u8 = 3;
/// Set to `1`, `login` exits 1 without writing the token (decision 0059).
const LOGIN_FAIL_ENV: &str = "FAKE_AGENT_LOGIN_FAIL";
/// What `login` writes into the token file.
const FAKE_LOGIN_TOKEN: &str = r#"{"fake":true}"#;
/// A token containing this is expired: `whoami` then reports `expired`.
const EXPIRED_MARK: &str = r#""expired":true"#;
/// The identity `whoami` reports for a token that names none.
const DEFAULT_IDENTITY: &str = "fake@example.com";

/// The lines of standard input, read as they arrive.
type Input = io::Lines<io::StdinLock<'static>>;

fn main() -> ExitCode {
    // The sign-in and check of an account (decision 0059) are subcommands: `login` and `whoami`, run in the account home.
    match env::args().nth(1).as_deref() {
        Some("login") => return login(),
        Some("whoami") => return whoami(),
        _ => {}
    }
    let arguments = Arguments::parse(env::args().skip(1));
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
            text => {
                if let Flow::Finish(code) = agent.handle_prompt(text, &mut input) {
                    return code;
                }
            }
        }
    }
    ExitCode::SUCCESS
}

/// The command line: `[--session-id <ID>] [--resume <ID>] [PROMPT WORDS...]`. The flags
/// count only before the first prompt word; every later word belongs to the prompt.
#[derive(Debug, Default)]
struct Arguments {
    session_id: Option<String>,
    resume: Option<String>,
    prompt: Option<String>,
}

impl Arguments {
    fn parse(mut args: impl Iterator<Item = String>) -> Self {
        let mut parsed = Self::default();
        let mut words = Vec::new();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--session-id" if words.is_empty() => parsed.session_id = args.next(),
                "--resume" if words.is_empty() => parsed.resume = args.next(),
                _ => words.push(arg),
            }
        }
        if !words.is_empty() {
            parsed.prompt = Some(words.join(" "));
        }
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
        say(if credentials_found() {
            "credentials found"
        } else {
            "credentials missing"
        });
        say(&format!("home {}", home_name()));
        for name in probed_env() {
            say(&format!("env {name} {}", env_state(&name)));
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
        Command::new("git")
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
        let _ = Command::new(program)
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
    let Some(home) = credential_home() else {
        say("no credential home");
        return ExitCode::from(1);
    };
    match fs::write(home.join(CREDENTIAL_FILE), FAKE_LOGIN_TOKEN) {
        Ok(()) => {
            say("signed in");
            ExitCode::SUCCESS
        }
        Err(_) => {
            say("login failed");
            ExitCode::from(1)
        }
    }
}

/// The check: prints `signed in as <identity>` and exits 0 when the token exists and is not expired; otherwise prints
/// `expired` and exits 1. A token's `email` names its identity; the default identity is used otherwise.
fn whoami() -> ExitCode {
    let token =
        credential_home().and_then(|home| fs::read_to_string(home.join(CREDENTIAL_FILE)).ok());
    match token {
        Some(text) if !text.contains(EXPIRED_MARK) => {
            let identity = email_of(&text).unwrap_or_else(|| DEFAULT_IDENTITY.to_owned());
            println!("signed in as {identity}");
            ExitCode::SUCCESS
        }
        _ => {
            println!("expired");
            ExitCode::from(1)
        }
    }
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
