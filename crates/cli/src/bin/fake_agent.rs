//! `massorca-fake-agent`: a stand-in for an agent CLI, for MassOrca's own tests (ADR 0057).
//!
//! It prints `fake-agent: ...` lines, takes prompts from its arguments and then from
//! stdin, and reports its activity through `massorca agent hook` when `MASSORCA_BIN`
//! names that program. It prints whether a credential file exists, never its contents.

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
/// The content of each file that `write:<name>` creates before committing it.
const WRITTEN_CONTENT: &str = "written by fake agent\n";
/// The exit code of the `crash` command.
const CRASH_EXIT_CODE: u8 = 3;

/// The lines of standard input, read as they arrive.
type Input = io::Lines<io::StdinLock<'static>>;

fn main() -> ExitCode {
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
