//! The runner file of a setup or archive script and the program a service terminal starts to run it (decisions 0086,
//! 0095).

use std::fmt;
use std::path::{Path, PathBuf};

/// The folder under the worktree's own git directory that holds the runner.
pub const RUNNER_DIR: &str = "massorca";

/// Interpreters a `#!` line may name for the POSIX runner on Windows.
const POSIX_SHELLS: [&str; 6] = ["sh", "bash", "zsh", "dash", "ksh", "ash"];
/// Characters a `%NAME%` value must not hold for a `.cmd` runner.
const CMD_UNSAFE_VALUE_CHARS: [char; 11] = ['&', '|', '<', '>', '^', '%', '!', '(', ')', '"', '\n'];
/// Characters cmd re-reads in a `/c` command line however the runner path is quoted.
/// `,`, `;` and `=` separate arguments for cmd, so an unquoted path holding one names another program.
const CMD_UNSAFE_PATH_CHARS: [char; 13] = [
    '&', '<', '>', '(', ')', '@', '^', '|', '%', '!', ',', ';', '=',
];

/// The host the runner is written for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerHost {
    Windows,
    Posix,
}

impl RunnerHost {
    #[must_use]
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Posix
        }
    }
}

/// Absolute interpreters found on the host; `bash` on Windows is Git Bash only, never the WSL launcher.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Interpreters {
    pub cmd: Option<PathBuf>,
    pub bash: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerKind {
    /// A batch file run by `cmd.exe /d /c`.
    Cmd,
    /// A bash script run by `bash`.
    Posix,
}

/// The runner file and the program a terminal starts to run it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupRunner {
    pub kind: RunnerKind,
    pub path: PathBuf,
    pub contents: String,
    pub program: PathBuf,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerRefusal {
    NoCmd,
    NoBash,
    UnsafeCmdPath(PathBuf),
    /// A variable whose value cmd would read as syntax once `%NAME%` expands it.
    UnsafeValue(String),
}

impl fmt::Display for RunnerRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCmd => f.write_str("cmd.exe was not found"),
            Self::NoBash => f.write_str("bash was not found"),
            Self::UnsafeValue(name) => write!(
                f,
                "the script was not run: {name} holds a character cmd would run as a command ({})",
                CMD_UNSAFE_VALUE_CHARS
                    .iter()
                    .collect::<String>()
                    .escape_debug()
            ),
            Self::UnsafeCmdPath(path) => write!(
                f,
                "cmd.exe cannot run a runner whose path has one of {}: {}",
                CMD_UNSAFE_PATH_CHARS.iter().collect::<String>(),
                path.display()
            ),
        }
    }
}

/// The runner file stem of a setup script.
pub const SETUP_RUNNER: &str = "setup-runner";
/// The runner file stem of an archive script (decision 0095).
pub const ARCHIVE_RUNNER: &str = "archive-runner";

/// Chooses and writes out the setup runner for `script` in `runner_dir`. On Windows it is a `.cmd` unless the script
/// starts with a `#!` line naming a POSIX shell and Git Bash exists; the user's terminal shell never decides it.
pub fn plan_runner(
    script: &str,
    host: RunnerHost,
    runner_dir: &Path,
    interpreters: &Interpreters,
) -> Result<SetupRunner, RunnerRefusal> {
    plan_named_runner(script, SETUP_RUNNER, host, runner_dir, interpreters)
}

/// [`plan_runner`] for a runner file named `stem` plus the runner's extension.
pub fn plan_named_runner(
    script: &str,
    stem: &str,
    host: RunnerHost,
    runner_dir: &Path,
    interpreters: &Interpreters,
) -> Result<SetupRunner, RunnerRefusal> {
    let shebang = shebang_interpreter(script);
    if host == RunnerHost::Posix {
        let bash = interpreters.bash.clone().ok_or(RunnerRefusal::NoBash)?;
        let path = runner_dir.join(format!("{stem}.sh"));
        return Ok(SetupRunner {
            kind: RunnerKind::Posix,
            args: vec![path.to_string_lossy().into_owned()],
            path,
            contents: posix_runner(script),
            program: bash,
        });
    }
    let posix_shell = shebang
        .as_deref()
        .is_some_and(|name| POSIX_SHELLS.contains(&name));
    if let (true, Some(bash)) = (posix_shell, &interpreters.bash) {
        let path = runner_dir.join(format!("{stem}.sh"));
        return Ok(SetupRunner {
            kind: RunnerKind::Posix,
            args: vec![posix_shell_path(&path.to_string_lossy())],
            path,
            contents: posix_runner(script),
            program: bash.clone(),
        });
    }
    let cmd = interpreters.cmd.clone().ok_or(RunnerRefusal::NoCmd)?;
    let path = runner_dir.join(format!("{stem}.cmd"));
    let text = path.to_string_lossy().into_owned();
    // A path with spaces stays one argument: cmd keeps exactly two quotes around an existing file's name.
    if text.contains(CMD_UNSAFE_PATH_CHARS) || text.contains('"') {
        return Err(RunnerRefusal::UnsafeCmdPath(path));
    }
    let contents = match (&shebang, posix_shell) {
        (None, _) => windows_runner(script),
        (Some(_), true) => windows_refusal(&[
            "this script starts with a \"#!\" line naming a POSIX shell, and Git Bash was not found.",
            "install Git for Windows, or rewrite the script in cmd syntax without the \"#!\" line.",
        ]),
        (Some(_), false) => windows_refusal(&[
            "this script starts with a \"#!\" line, and only a POSIX shell (sh, bash, ...) can run one.",
            "name a POSIX shell in the \"#!\" line, or rewrite the script in cmd syntax without it.",
        ]),
    };
    Ok(SetupRunner {
        kind: RunnerKind::Cmd,
        args: vec!["/d".to_owned(), "/c".to_owned(), text],
        path,
        contents,
        program: cmd,
    })
}

/// Refuses a `.cmd` runner when a value it exposes would turn into cmd syntax: cmd expands `%NAME%` before it parses
/// `&`, `|` and the rest, so a branch `x&calc` would run `calc`. A POSIX runner quotes its own expansions.
pub fn check_cmd_values(
    runner: &SetupRunner,
    vars: &[(String, String)],
) -> Result<(), RunnerRefusal> {
    if runner.kind != RunnerKind::Cmd {
        return Ok(());
    }
    match vars
        .iter()
        .find(|(_, value)| value.contains(CMD_UNSAFE_VALUE_CHARS))
    {
        Some((name, _)) => Err(RunnerRefusal::UnsafeValue(name.clone())),
        None => Ok(()),
    }
}

/// Writes the runner, replacing one an earlier create of the same worktree left.
pub fn write_runner(runner: &SetupRunner) -> std::io::Result<()> {
    if let Some(parent) = runner.path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&runner.path, &runner.contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&runner.path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

/// The variables the setup script sees: MassOrca's names, the reference's `ORCA_*` and compatibility names, and git's
/// prompt guard, since nobody may be watching. Path values are `/c/...` for a POSIX runner on Windows.
pub fn setup_env(
    root: &str,
    workspace_path: &str,
    workspace_name: &str,
    branch: Option<&str>,
    runner: &SetupRunner,
    host: RunnerHost,
) -> Vec<(String, String)> {
    let convert = host == RunnerHost::Windows && runner.kind == RunnerKind::Posix;
    let path = |value: &str| {
        if convert {
            posix_shell_path(value)
        } else {
            value.to_owned()
        }
    };
    let folder_name = Path::new(workspace_path).file_name().map_or_else(
        || workspace_name.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let mut vars = vec![
        ("MASSORCA_ROOT_PATH".to_owned(), path(root)),
        ("MASSORCA_WORKSPACE_PATH".to_owned(), path(workspace_path)),
        (
            "MASSORCA_WORKSPACE_NAME".to_owned(),
            workspace_name.to_owned(),
        ),
        ("ORCA_ROOT_PATH".to_owned(), path(root)),
        ("ORCA_WORKTREE_PATH".to_owned(), path(workspace_path)),
        ("ORCA_WORKSPACE_NAME".to_owned(), folder_name),
        ("CONDUCTOR_ROOT_PATH".to_owned(), path(root)),
        ("GHOSTX_ROOT_PATH".to_owned(), path(root)),
        ("GIT_TERMINAL_PROMPT".to_owned(), "0".to_owned()),
        ("GCM_INTERACTIVE".to_owned(), "never".to_owned()),
    ];
    if let Some(branch) = branch {
        vars.push(("MASSORCA_WORKSPACE_BRANCH".to_owned(), branch.to_owned()));
    }
    vars
}

/// The lowercase interpreter a leading `#!` line names (after `env` and its flags), without `.exe`.
fn shebang_interpreter(script: &str) -> Option<String> {
    let first = script.split('\n').next()?.trim();
    let rest = first.strip_prefix("#!")?;
    let name = rest
        .split_whitespace()
        .map(executable_name)
        .find(|name| !name.is_empty() && name != "env" && !name.starts_with('-'))
        .unwrap_or_default();
    Some(name)
}

fn executable_name(token: &str) -> String {
    let base = token.rsplit(['/', '\\']).next().unwrap_or_default();
    let lower = base.to_ascii_lowercase();
    lower.strip_suffix(".exe").unwrap_or(&lower).to_owned()
}

/// `C:\a\b` as Git Bash spells it: `/c/a/b`.
fn posix_shell_path(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
        && let Some(rest) = value.get(3..)
    {
        return format!(
            "/{}/{}",
            (bytes[0] as char).to_ascii_lowercase(),
            rest.replace('\\', "/")
        );
    }
    value.replace('\\', "/")
}

/// One `call` per line, stopping at the first failure; delayed expansion off so `!` survives.
fn windows_runner(script: &str) -> String {
    let mut out =
        String::from("@echo off\r\nsetlocal EnableExtensions DisableDelayedExpansion\r\n");
    for line in script.split('\n') {
        let command = line.trim();
        if command.is_empty() {
            out.push_str("\r\n");
            continue;
        }
        out.push_str("call ");
        out.push_str(command);
        out.push_str("\r\nif errorlevel 1 exit /b %errorlevel%\r\n");
    }
    out
}

/// A runner that says, in the terminal, why the script did not run, and fails.
fn windows_refusal(lines: &[&str]) -> String {
    let mut out =
        String::from("@echo off\r\nsetlocal EnableExtensions DisableDelayedExpansion\r\n");
    for line in lines {
        out.push_str("echo MassOrca setup: ");
        out.push_str(line);
        out.push_str(" 1>&2\r\n");
    }
    out.push_str("exit /b 1\r\n");
    out
}

/// `set -e`, the `#!` line's own `set` flags replayed (bash runs the file, so the line itself is ignored), then the body.
fn posix_runner(script: &str) -> String {
    let (options, body) = match script.split_once('\n') {
        Some((first, rest)) if first.trim_start().starts_with("#!") => (shell_options(first), rest),
        None if script.trim_start().starts_with("#!") => (shell_options(script), ""),
        _ => (Vec::new(), script),
    };
    let declared = if options.is_empty() {
        String::new()
    } else {
        format!("set {}\n", options.join(" "))
    };
    format!(
        "#!/usr/bin/env bash\nset -e\n{declared}{}\n",
        body.replace("\r\n", "\n")
    )
}

/// The flags of a `#!` line that `set` accepts; an invocation-only flag would make `set` fail under `set -e`.
fn shell_options(line: &str) -> Vec<String> {
    let tokens: Vec<&str> = line
        .trim()
        .trim_start_matches("#!")
        .split_whitespace()
        .collect();
    let Some(start) = tokens.iter().position(|token| {
        let name = executable_name(token);
        !name.is_empty() && name != "env" && !name.starts_with('-')
    }) else {
        return Vec::new();
    };
    let is_set_flag = |token: &str, long: bool| {
        let mut chars = token.chars();
        matches!(chars.next(), Some('-' | '+')) && {
            let letters: String = chars.collect();
            let letters = if long {
                match letters.strip_suffix('o') {
                    Some(rest) => rest.to_owned(),
                    None => return false,
                }
            } else {
                letters
            };
            (long || !letters.is_empty())
                && letters.chars().all(|c| "abefhkmnptuvxBCHP".contains(c))
        }
    };
    let mut options = Vec::new();
    let mut rest = tokens[start + 1..].iter();
    while let Some(token) = rest.next() {
        if is_set_flag(token, true) {
            if let Some(name) = rest.next().filter(|name| {
                !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '_')
            }) {
                options.push((*token).to_owned());
                options.push((*name).to_owned());
            }
        } else if is_set_flag(token, false) {
            options.push((*token).to_owned());
        }
    }
    options
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools() -> Interpreters {
        Interpreters {
            cmd: Some(PathBuf::from(r"C:\Windows\System32\cmd.exe")),
            bash: Some(PathBuf::from(r"C:\Program Files\Git\bin\bash.exe")),
        }
    }

    #[test]
    fn a_plain_windows_script_gets_a_cmd_runner_started_by_cmd_with_no_autorun() {
        let dir = Path::new(r"C:\repo\.git\worktrees\feat\massorca");
        let runner = plan_runner("pnpm install\necho ok", RunnerHost::Windows, dir, &tools())
            .expect("plans");
        assert_eq!(runner.kind, RunnerKind::Cmd);
        assert_eq!(runner.path, dir.join("setup-runner.cmd"));
        assert_eq!(
            runner.program,
            PathBuf::from(r"C:\Windows\System32\cmd.exe")
        );
        assert_eq!(
            runner.args,
            vec!["/d", "/c", &*dir.join("setup-runner.cmd").to_string_lossy()]
        );
        assert_eq!(
            runner.contents,
            "@echo off\r\nsetlocal EnableExtensions DisableDelayedExpansion\r\n\
             call pnpm install\r\nif errorlevel 1 exit /b %errorlevel%\r\n\
             call echo ok\r\nif errorlevel 1 exit /b %errorlevel%\r\n"
        );
    }

    #[test]
    fn a_windows_script_with_a_posix_shebang_runs_through_git_bash() {
        let dir = Path::new(r"C:\repo\.git\worktrees\feat\massorca");
        let runner = plan_runner(
            "#!/usr/bin/env -S bash -euo pipefail\npnpm install\r\n",
            RunnerHost::Windows,
            dir,
            &tools(),
        )
        .expect("plans");
        assert_eq!(runner.kind, RunnerKind::Posix);
        assert_eq!(runner.path, dir.join("setup-runner.sh"));
        assert_eq!(
            runner.program,
            PathBuf::from(r"C:\Program Files\Git\bin\bash.exe")
        );
        assert_eq!(
            runner.args,
            vec!["/c/repo/.git/worktrees/feat/massorca/setup-runner.sh"]
        );
        assert_eq!(
            runner.contents,
            "#!/usr/bin/env bash\nset -e\nset -euo pipefail\npnpm install\n\n"
        );
    }

    #[test]
    fn a_windows_shebang_script_without_git_bash_or_a_posix_shell_gets_a_refusing_cmd_runner() {
        let dir = Path::new(r"C:\r\massorca");
        let no_bash = Interpreters {
            bash: None,
            ..tools()
        };
        let runner =
            plan_runner("#!/bin/bash\necho hi", RunnerHost::Windows, dir, &no_bash).expect("plans");
        assert_eq!(runner.kind, RunnerKind::Cmd);
        assert!(runner.contents.contains("Git Bash was not found"));
        assert!(runner.contents.ends_with("exit /b 1\r\n"));
        assert!(!runner.contents.contains("call "));

        let python = plan_runner(
            "#!/usr/bin/env python3\nprint(1)",
            RunnerHost::Windows,
            dir,
            &tools(),
        )
        .expect("plans");
        assert_eq!(python.kind, RunnerKind::Cmd);
        assert!(python.contents.contains("only a POSIX shell"));
    }

    #[test]
    fn a_cmd_runner_path_cmd_would_reread_is_refused() {
        let dir = Path::new(r"C:\Program Files (x86)\r\massorca");
        assert_eq!(
            plan_runner("echo", RunnerHost::Windows, dir, &tools()),
            Err(RunnerRefusal::UnsafeCmdPath(dir.join("setup-runner.cmd")))
        );
        let none = Interpreters::default();
        assert_eq!(
            plan_runner("echo", RunnerHost::Windows, Path::new(r"C:\r"), &none),
            Err(RunnerRefusal::NoCmd)
        );
    }

    #[test]
    fn a_posix_host_always_runs_a_bash_runner() {
        let dir = Path::new("/repo/.git/worktrees/feat/massorca");
        let bash = Interpreters {
            cmd: None,
            bash: Some(PathBuf::from("/bin/bash")),
        };
        let runner = plan_runner("make setup", RunnerHost::Posix, dir, &bash).expect("plans");
        assert_eq!(runner.kind, RunnerKind::Posix);
        assert_eq!(runner.program, PathBuf::from("/bin/bash"));
        assert_eq!(
            runner.args,
            vec![dir.join("setup-runner.sh").to_string_lossy().into_owned()]
        );
        assert_eq!(runner.contents, "#!/usr/bin/env bash\nset -e\nmake setup\n");
        assert_eq!(
            plan_runner("x", RunnerHost::Posix, dir, &Interpreters::default()),
            Err(RunnerRefusal::NoBash)
        );
    }

    #[test]
    fn the_environment_names_the_root_workspace_and_branch_in_the_runner_spelling() {
        let dir = Path::new(r"C:\r\massorca");
        let bash_runner =
            plan_runner("#!/bin/sh\nx", RunnerHost::Windows, dir, &tools()).expect("plans");
        let vars = setup_env(
            r"C:\repo",
            r"C:\ws\feat",
            "feat",
            Some("feat/x"),
            &bash_runner,
            RunnerHost::Windows,
        );
        let get = |name: &str| {
            vars.iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.as_str())
        };
        assert_eq!(get("MASSORCA_ROOT_PATH"), Some("/c/repo"));
        assert_eq!(get("ORCA_WORKTREE_PATH"), Some("/c/ws/feat"));
        assert_eq!(get("MASSORCA_WORKSPACE_NAME"), Some("feat"));
        assert_eq!(get("MASSORCA_WORKSPACE_BRANCH"), Some("feat/x"));
        assert_eq!(get("GIT_TERMINAL_PROMPT"), Some("0"));

        let cmd_runner = plan_runner("x", RunnerHost::Windows, dir, &tools()).expect("plans");
        let vars = setup_env(
            r"C:\repo",
            r"C:\ws\feat",
            "feat",
            None,
            &cmd_runner,
            RunnerHost::Windows,
        );
        assert!(vars.contains(&(
            "MASSORCA_WORKSPACE_PATH".to_owned(),
            r"C:\ws\feat".to_owned()
        )));
        assert!(
            !vars
                .iter()
                .any(|(key, _)| key == "MASSORCA_WORKSPACE_BRANCH")
        );
    }

    #[test]
    fn a_cmd_runner_path_with_a_separator_cmd_splits_on_is_refused() {
        for dir in [r"C:\a,b\massorca", r"C:\a;b\massorca", r"C:\a=b\massorca"] {
            let dir = Path::new(dir);
            assert_eq!(
                plan_runner("echo", RunnerHost::Windows, dir, &tools()),
                Err(RunnerRefusal::UnsafeCmdPath(dir.join("setup-runner.cmd")))
            );
        }
    }

    #[test]
    fn a_cmd_runner_refuses_a_variable_value_cmd_would_reread() {
        let dir = Path::new(r"C:\r\massorca");
        let env = |runner: &SetupRunner, branch: &str| {
            setup_env(
                r"C:\repo",
                r"C:\ws\f",
                "f",
                Some(branch),
                runner,
                RunnerHost::Windows,
            )
        };
        let runner = plan_runner("x", RunnerHost::Windows, dir, &tools()).expect("plans");
        assert_eq!(
            check_cmd_values(&runner, &env(&runner, "x&calc")),
            Err(RunnerRefusal::UnsafeValue(
                "MASSORCA_WORKSPACE_BRANCH".to_owned()
            ))
        );
        for value in [
            "a|b", "a<b", "a>b", "a^b", "a%b", "a!b", "a(b", "a)b", "a\"b", "a\nb",
        ] {
            let vars = vec![("MASSORCA_ROOT_PATH".to_owned(), value.to_owned())];
            assert!(check_cmd_values(&runner, &vars).is_err(), "{value:?}");
        }
        assert_eq!(check_cmd_values(&runner, &env(&runner, "feat/x")), Ok(()));
        let posix = plan_runner("#!/bin/sh\nx", RunnerHost::Windows, dir, &tools()).expect("plans");
        assert_eq!(check_cmd_values(&posix, &env(&posix, "x&calc")), Ok(()));
    }
}
