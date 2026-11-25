//! Chat titles by a one-shot run (decision 0167): a kind's non-interactive form runs once with an account's home and names
//! a chat from its first prompt and reply. The run carries no MassOrca run identity, installs no status hooks, works in a
//! scratch folder outside every workspace that is removed afterwards, and keeps no session row, terminal or transcript.
//! The naming rules live in [`crate::title`]; the output is matched there and never logged or returned, except as the slug.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use massorca_core_store::new_id;
use massorca_protocol::codes;

use crate::account_check::{account_command, account_strip_names, collect, read_limited};
use crate::credentials;
use crate::instructions::probe_run;
use crate::kind::{CredentialsDecl, KindDecl, OneshotDecl, OptionalArgDecl, PROMPT_PLACEHOLDER};
use crate::process_tree;
use crate::sessions::{AgentError, Agents};
use crate::state_file;
use crate::title::{plain_reply, title_prompt, title_slug, validated_prompt};

/// How often a running title run is checked for its end.
const POLL: Duration = Duration::from_millis(25);
/// The age after which a scratch folder of an earlier run is swept by the next run; no run lasts that long (decision 0167).
const STALE_SCRATCH_AGE: Duration = Duration::from_secs(60 * 60);
/// The argument that asks a program for its help text, whose listing decides the optional arguments (decision 0167).
const HELP_ARG: &str = "--help";

/// How long a title run may take (decision 0167). `MASSORCA_TITLE_TIMEOUT_MS` may shorten it, for tests only.
pub const TITLE_TIMEOUT: Duration = Duration::from_secs(30);
/// The variable that sets the title time limit in milliseconds, clamped to [`TITLE_TIMEOUT_FLOOR`] and [`TITLE_TIMEOUT`].
pub const TITLE_TIMEOUT_ENV: &str = "MASSORCA_TITLE_TIMEOUT_MS";
/// The shortest time limit the test hook can set.
const TITLE_TIMEOUT_FLOOR: Duration = Duration::from_millis(100);

/// What a caller asks for: the kind, the account to run it with (the kind's active account when absent), the model (used
/// only by a kind that reuses its model flag) and the chat's first prompt and, once the agent has answered, its reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitleRequest {
    pub kind: String,
    pub account: Option<String>,
    pub model: Option<String>,
    pub prompt: String,
    pub reply: Option<String>,
}

/// The time limit of a title run: [`TITLE_TIMEOUT`], unless [`TITLE_TIMEOUT_ENV`] shortens it.
pub fn title_timeout() -> Duration {
    title_timeout_from(std::env::var(TITLE_TIMEOUT_ENV).ok().as_deref())
}

/// The time limit for a value of [`TITLE_TIMEOUT_ENV`]: whole milliseconds, clamped to the hook's range; any other value,
/// or none, is the default.
pub fn title_timeout_from(value: Option<&str>) -> Duration {
    value
        .and_then(|text| text.trim().parse::<u64>().ok())
        .map_or(TITLE_TIMEOUT, |millis| {
            Duration::from_millis(millis).clamp(TITLE_TIMEOUT_FLOOR, TITLE_TIMEOUT)
        })
}

/// The arguments of one run: the kind's one-shot arguments with the prompt text in place of [`PROMPT_PLACEHOLDER`], then the
/// optional arguments its `--help` lists, then the model arguments the kind reuses for the run. The text is one argument,
/// never split or scanned again.
pub(crate) fn oneshot_args(
    oneshot: &OneshotDecl,
    text: &str,
    optional_args: &[String],
    model_args: &[String],
) -> Vec<String> {
    oneshot
        .args
        .iter()
        .map(|arg| arg.replace(PROMPT_PLACEHOLDER, text))
        .chain(optional_args.iter().cloned())
        .chain(model_args.iter().cloned())
        .collect()
}

/// The arguments of the optional flags that `help` lists, each flag then its values, in declared order (decision 0167). A
/// flag the listing does not name is never passed.
pub(crate) fn optional_args_listed(optional: &[OptionalArgDecl], help: &str) -> Vec<String> {
    optional
        .iter()
        .filter(|arg| flag_listed(help, &arg.flag))
        .flat_map(|arg| std::iter::once(arg.flag.clone()).chain(arg.values.iter().cloned()))
        .collect()
}

/// Whether `help` lists `flag` as a whole word: a longer flag that contains it, such as `--strict-mcp-config` for
/// `--mcp-config`, does not count.
fn flag_listed(help: &str, flag: &str) -> bool {
    let word = |ch: char| ch.is_ascii_alphanumeric() || ch == '-';
    help.match_indices(flag).any(|(index, _)| {
        let before = help[..index].chars().next_back();
        let after = help[index + flag.len()..].chars().next();
        !before.is_some_and(word) && !after.is_some_and(word)
    })
}

/// The optional arguments of a run: those the program's `--help` lists, when the kind declares any and `--help` finishes with
/// exit 0. Without such a listing none is passed. The listing is the instruction channel's cached probe output (decision
/// 0167), so the program's help runs once per installed program.
fn listed_optional_args(decl: &KindDecl, program: &Path, oneshot: &OneshotDecl) -> Vec<String> {
    if oneshot.optional_args.is_empty() {
        return Vec::new();
    }
    let help = probe_run(decl, program, &[HELP_ARG.to_owned()]);
    if help.exit != Some(0) {
        return Vec::new();
    }
    optional_args_listed(&oneshot.optional_args, &help.output)
}

/// The command of one title run: `program` with `args`, the account's home variable set, `strip` removed from its
/// environment, and `scratch` as its working directory.
pub(crate) fn oneshot_command(
    program: &Path,
    args: &[String],
    home: &Path,
    home_env: &str,
    strip: &[String],
    scratch: &Path,
) -> Command {
    let mut command = account_command(program, args, home, home_env, strip);
    command.current_dir(scratch);
    command
}

/// A fresh scratch folder under the profile's `tmp` folder, the working directory of one run. It is removed when dropped. A
/// new one first sweeps the folders that earlier runs left behind (decision 0167).
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn create(profile_dir: &Path) -> io::Result<Self> {
        sweep_stale_scratch(profile_dir, SystemTime::now(), STALE_SCRATCH_AGE);
        let dir = profile_dir
            .join("tmp")
            .join(format!("oneshot-{}", new_id()));
        fs::create_dir_all(&dir)?;
        Ok(Self(dir))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        match fs::remove_dir_all(&self.0) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                tracing::warn!(%error, "a title run's scratch folder could not be removed");
            }
        }
    }
}

/// Removes the scratch folders under `profile_dir/tmp` that are older than `max_age` at `now`. A run is always younger than
/// any time limit a run has, so a folder in use is never removed; folders that are not scratch folders are kept.
pub(crate) fn sweep_stale_scratch(profile_dir: &Path, now: SystemTime, max_age: Duration) {
    let Ok(entries) = fs::read_dir(profile_dir.join("tmp")) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with("oneshot-") {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|metadata| metadata.modified()) else {
            continue;
        };
        if now.duration_since(modified).is_ok_and(|age| age > max_age) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

/// How a title run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OneshotRun {
    /// The program exited, with `code` (`None` when a signal ended it) and the output it printed.
    Finished { code: Option<i32>, output: String },
    /// The program was still running at the deadline; its process tree was stopped.
    TimedOut,
    /// The program could not be started.
    NotStarted,
}

/// Runs `command` for at most `timeout`, then stops its whole process tree and collects what it printed.
pub(crate) fn run_title(command: &mut Command, timeout: Duration) -> OneshotRun {
    let Ok((mut child, mut tree)) = process_tree::spawn(command) else {
        return OneshotRun::NotStarted;
    };
    let stdout = read_limited(child.stdout.take());
    let stderr = read_limited(child.stderr.take());
    let status = wait_until(&mut child, timeout);
    tree.stop();
    // Only stdout is the reply: a warning on stderr must never become the title. stderr is read so its pipe closes.
    let output = collect(stdout);
    let _ = collect(stderr);
    match status {
        Some(status) => OneshotRun::Finished {
            code: status.code(),
            output,
        },
        None => OneshotRun::TimedOut,
    }
}

/// Waits for the program to end, for at most `timeout`. Past the deadline the program is killed and `None` is returned.
fn wait_until(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) if started.elapsed() < timeout => thread::sleep(POLL),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// The title a run names, or the refusal it ends with. A run that exits with a failure, prints no usable title or does
/// not start is `agent.oneshot_failed`; one past its limit is `agent.oneshot_timeout`. Output is never in the error.
pub(crate) fn title_of(run: OneshotRun, kind: &str) -> Result<String, AgentError> {
    match run {
        OneshotRun::Finished {
            code: Some(0),
            output,
        } => title_slug(&output).ok_or_else(|| failed(kind, "it printed no usable title")),
        OneshotRun::Finished {
            code: Some(code), ..
        } => Err(failed(kind, &format!("it exited with status {code}"))),
        OneshotRun::Finished { code: None, .. } => Err(failed(kind, "a signal ended it")),
        OneshotRun::TimedOut => Err(AgentError::new(
            codes::AGENT_ONESHOT_TIMEOUT,
            format!("agent kind {kind} did not name the chat within its time limit"),
        )),
        OneshotRun::NotStarted => Err(failed(kind, "its program did not start")),
    }
}

/// A refused title run: the reason names the kind and the cause, never the output.
fn failed(kind: &str, why: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_ONESHOT_FAILED,
        format!("agent kind {kind} could not name the chat: {why}"),
    )
}

/// The refusal of a title run that has no account to run with.
fn no_usable_account(kind: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_CREDENTIALS_INVALID,
        format!(
            "agent kind {kind} has no usable account to name a chat with; sign an account in first"
        ),
    )
}

impl Agents {
    /// Names a chat (decision 0167): runs the kind's one-shot form once with the account's home and returns the slug of the
    /// title it prints. Refuses with `agent.oneshot_unsupported`, `agent.credentials_invalid`, `agent.oneshot_timeout`,
    /// `agent.oneshot_failed` or `agent.prompt_invalid`.
    pub fn generate_title(
        &self,
        request: TitleRequest,
        timeout: Duration,
    ) -> Result<String, AgentError> {
        let (decl, program) = self.startable_kind(&request.kind)?;
        let oneshot = decl.oneshot.as_ref().ok_or_else(|| {
            AgentError::new(
                codes::AGENT_ONESHOT_UNSUPPORTED,
                format!(
                    "agent kind {} declares no one-shot form, so it cannot name a chat",
                    decl.id
                ),
            )
        })?;
        let prompt = validated_prompt(&request.prompt)?;
        let reply = request.reply.as_deref().map(plain_reply);
        let text = title_prompt(&prompt, reply.as_deref());
        let credentials = decl
            .credentials
            .as_ref()
            .ok_or_else(|| no_usable_account(&decl.id))?;
        let account = self.title_account(&decl.id, request.account.as_deref())?;
        let home = self.title_home(&decl.id, &account, credentials)?;
        let model_args = match (request.model.as_deref(), oneshot.model_flag_reuse) {
            (Some(model), true) => decl.option_args(Some(model), None)?,
            _ => Vec::new(),
        };
        let optional_args = listed_optional_args(&decl, &program, oneshot);
        let args = oneshot_args(oneshot, &text, &optional_args, &model_args);
        let scratch = Scratch::create(self.profile_dir()).map_err(|error| {
            AgentError::new(
                codes::AGENT_ONESHOT_FAILED,
                format!(
                    "agent kind {} could not make the folder its run works in: {error}",
                    decl.id
                ),
            )
        })?;
        let mut command = oneshot_command(
            &program,
            &args,
            &home,
            &credentials.home_env,
            &account_strip_names(&decl),
            scratch.path(),
        );
        let run = run_title(&mut command, timeout);
        drop(scratch);
        title_of(run, &decl.id)
    }

    /// The account a title run uses: the one `account` names, else the kind's active one. Any account that is not there is
    /// no usable account.
    fn title_account(&self, kind: &str, account: Option<&str>) -> Result<String, AgentError> {
        match account {
            Some(id) => self
                .account_of(kind, id)
                .map(|row| row.id)
                .map_err(|_| no_usable_account(kind)),
            None => self
                .store()
                .get_active_agent_account(kind)?
                .ok_or_else(|| no_usable_account(kind)),
        }
    }

    /// The account's credential home, read under the homes gate as a check reads it, with its state file repaired first
    /// (decision 0099). The gate is released before the run starts.
    fn title_home(
        &self,
        kind: &str,
        account: &str,
        credentials: &CredentialsDecl,
    ) -> Result<PathBuf, AgentError> {
        let _gate = credentials::lock_homes();
        let home = credentials::usable_account_home(self.profile_dir(), kind, account)?
            .ok_or_else(|| no_usable_account(kind))?;
        if let Some(rule) = &credentials.state
            && let Err(reason) = state_file::ensure(&home, rule)
        {
            tracing::warn!(kind, file = %rule.file, %reason, "the account's state file cannot be repaired");
            return Err(no_usable_account(kind));
        }
        Ok(home)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use massorca_protocol::codes;
    use serde_json::{Value, json};

    use super::*;
    use crate::kind::KindDecl;
    use crate::packages::{KindPackages, PackageKind, PackageState};

    fn claude_like(model_flag_reuse: bool) -> KindDecl {
        let mut oneshot = json!({ "args": ["-p", "{prompt}", "--output-format", "text"] });
        if model_flag_reuse {
            oneshot["model_flag_reuse"] = json!(true);
        }
        KindDecl::from_json(
            &json!({
                "id": "fake", "name": "Fake", "program": ["fake-agent"],
                "credentials": { "home_env": "FAKE_HOME", "files": ["token.json"] },
                "model": { "flag": "--model", "choices": [{ "id": "sonnet", "label": "Sonnet" }] },
                "oneshot": oneshot
            })
            .to_string(),
        )
        .expect("parses")
    }

    #[test]
    fn the_title_time_limit_is_thirty_seconds_and_the_test_hook_clamps_it() {
        assert_eq!(TITLE_TIMEOUT, Duration::from_secs(30));
        assert_eq!(title_timeout_from(None), Duration::from_secs(30));
        assert_eq!(title_timeout_from(Some("bogus")), Duration::from_secs(30));
        assert_eq!(title_timeout_from(Some("5")), Duration::from_millis(100));
        assert_eq!(title_timeout_from(Some("250")), Duration::from_millis(250));
        assert_eq!(title_timeout_from(Some("999999")), Duration::from_secs(30));
    }

    #[test]
    fn the_prompt_is_one_argument_and_is_never_split_or_scanned_again() {
        let decl = claude_like(false);
        let oneshot = decl.oneshot.as_ref().expect("declared");
        let text = "Name it {prompt} with 'quotes' and \"doubles\"";
        assert_eq!(
            oneshot_args(oneshot, text, &[], &[]),
            vec![
                "-p".to_owned(),
                text.to_owned(),
                "--output-format".to_owned(),
                "text".to_owned()
            ]
        );
    }

    #[test]
    fn the_built_argument_holds_no_carriage_return_or_line_feed() {
        let decl = claude_like(false);
        let oneshot = decl.oneshot.as_ref().expect("declared");
        let text = title_prompt(
            "Fix\r\nthe login\nbug",
            Some("I will\r\nread the\nauth module"),
        );
        for arg in oneshot_args(oneshot, &text, &[], &[]) {
            assert!(!arg.contains('\r') && !arg.contains('\n'), "{arg:?}");
        }
    }

    #[test]
    fn the_model_flag_is_passed_only_by_a_kind_that_reuses_it() {
        let reuse = claude_like(true);
        let model_args = reuse
            .option_args(Some("sonnet"), None)
            .expect("the model is declared");
        assert_eq!(model_args, ["--model", "sonnet"]);
        let args = oneshot_args(
            reuse.oneshot.as_ref().expect("declared"),
            "text",
            &[],
            &model_args,
        );
        assert_eq!(&args[args.len() - 2..], ["--model", "sonnet"]);

        let plain = claude_like(false);
        let plain_args = oneshot_args(plain.oneshot.as_ref().expect("declared"), "text", &[], &[]);
        assert_eq!(plain_args.len(), 4, "no model argument without the reuse");
    }

    #[test]
    fn an_optional_argument_is_passed_only_when_the_help_lists_its_flag() {
        let optional: Vec<OptionalArgDecl> = serde_json::from_value(json!([
            { "flag": "--max-turns", "values": ["1"] },
            { "flag": "--strict-mcp-config" },
            { "flag": "--no-session-persistence" },
            { "flag": "--tools", "values": [""] }
        ]))
        .expect("parses");
        let help = "Usage: agent [OPTIONS]\n  --max-turns <N>\n  --strict-mcp-config\n  --disallowedTools <tools...>\n";
        assert_eq!(
            optional_args_listed(&optional, help),
            ["--max-turns", "1", "--strict-mcp-config"]
        );
        assert!(optional_args_listed(&optional, "no flags here").is_empty());
    }

    #[test]
    fn a_flag_counts_as_listed_only_as_a_whole_word() {
        let help = "  --strict-mcp-config\n  --disallowedTools <tools...>\n  --tools <list>\n";
        assert!(!flag_listed(help, "--mcp-config"), "part of a longer flag");
        assert!(!flag_listed(help, "--Tools"), "the case is kept");
        assert!(flag_listed(help, "--tools"));
        assert!(flag_listed("  --max-turns=<n>", "--max-turns"));
        assert!(!flag_listed("  --max-turns-extra", "--max-turns"));
    }

    #[test]
    fn the_optional_arguments_follow_the_template_and_precede_the_model_argument() {
        let decl = claude_like(true);
        let oneshot = decl.oneshot.as_ref().expect("declared");
        let model_args = decl
            .option_args(Some("sonnet"), None)
            .expect("the model is declared");
        let optional = ["--max-turns".to_owned(), "1".to_owned()];
        let args = oneshot_args(oneshot, "text", &optional, &model_args);
        assert_eq!(
            args,
            [
                "-p",
                "text",
                "--output-format",
                "text",
                "--max-turns",
                "1",
                "--model",
                "sonnet"
            ]
        );
    }

    #[test]
    fn the_run_drops_the_run_identity_the_stripped_names_and_works_in_the_scratch_folder() {
        let decl = KindDecl::from_json(
            r#"{
                "id": "fake", "name": "Fake", "program": ["fake-agent"],
                "strip_inherited_env": ["PARENT_STAMP"],
                "credentials": { "home_env": "FAKE_HOME", "files": ["token.json"], "strip_env": ["API_KEY"] }
            }"#,
        )
        .expect("parses");
        let strip = crate::account_check::account_strip_names(&decl);
        let profile = tempfile::tempdir().expect("a temporary profile");
        let scratch = Scratch::create(profile.path()).expect("the scratch folder");
        let command = oneshot_command(
            Path::new("fake-agent"),
            &["-p".to_owned()],
            Path::new("home"),
            "FAKE_HOME",
            &strip,
            scratch.path(),
        );
        let removed: Vec<String> = command
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(key, _)| key.to_string_lossy().into_owned())
            .collect();
        for name in [
            "MASSORCA_AGENT_SESSION",
            "MASSORCA_AGENT_TOKEN",
            "MASSORCA_AGENT_KIND",
            "PARENT_STAMP",
            "API_KEY",
        ] {
            assert!(
                removed.iter().any(|removed| removed == name),
                "{name} is removed"
            );
        }
        assert_eq!(command.get_current_dir(), Some(scratch.path()));
    }

    #[test]
    fn the_scratch_folder_is_under_the_profile_and_is_removed_when_the_run_ends() {
        let profile = tempfile::tempdir().expect("a temporary profile");
        let path = {
            let scratch = Scratch::create(profile.path()).expect("the scratch folder");
            assert!(scratch.path().is_dir(), "the folder exists during the run");
            assert!(scratch.path().starts_with(profile.path().join("tmp")));
            scratch.path().to_path_buf()
        };
        assert!(!path.exists(), "the folder is gone afterwards");
    }

    #[test]
    fn a_stale_scratch_folder_is_swept_and_a_fresh_one_or_another_folder_is_kept() {
        let profile = tempfile::tempdir().expect("a temporary profile");
        let tmp = profile.path().join("tmp");
        fs::create_dir_all(tmp.join("oneshot-left-behind")).expect("a left-behind folder");
        fs::create_dir_all(tmp.join("other")).expect("another folder");
        let now = SystemTime::now();
        sweep_stale_scratch(profile.path(), now, Duration::from_secs(3600));
        assert!(
            tmp.join("oneshot-left-behind").exists(),
            "a fresh folder is kept"
        );
        sweep_stale_scratch(
            profile.path(),
            now + Duration::from_secs(7200),
            Duration::from_secs(3600),
        );
        assert!(
            !tmp.join("oneshot-left-behind").exists(),
            "a folder older than the limit is swept"
        );
        assert!(
            tmp.join("other").exists(),
            "a folder that is not scratch is kept"
        );
    }

    #[test]
    fn a_finished_run_with_a_title_names_the_chat_by_its_slug() {
        let run = OneshotRun::Finished {
            code: Some(0),
            output: "Fix login\n".to_owned(),
        };
        assert_eq!(title_of(run, "fake").expect("a title"), "fix-login");
    }

    #[test]
    fn a_failed_exit_an_empty_output_or_a_program_that_does_not_start_is_oneshot_failed() {
        for run in [
            OneshotRun::Finished {
                code: Some(1),
                output: "Fix login".to_owned(),
            },
            OneshotRun::Finished {
                code: None,
                output: String::new(),
            },
            OneshotRun::Finished {
                code: Some(0),
                output: "  \n\t".to_owned(),
            },
            OneshotRun::NotStarted,
        ] {
            let error = title_of(run.clone(), "fake").expect_err("refused");
            assert_eq!(error.code, codes::AGENT_ONESHOT_FAILED, "{run:?}");
            assert!(
                !error.message.contains("Fix login"),
                "output is never in the error"
            );
        }
    }

    #[test]
    fn a_run_past_its_limit_is_oneshot_timeout() {
        let error = title_of(OneshotRun::TimedOut, "fake").expect_err("refused");
        assert_eq!(error.code, codes::AGENT_ONESHOT_TIMEOUT);
    }

    /// Hands one declared kind to a service, installed at this test's own executable.
    struct OneKind(String);

    impl KindPackages for OneKind {
        fn kinds(&self) -> Vec<PackageKind> {
            vec![PackageKind {
                kind_id: "fake".to_owned(),
                package_id: "massorca.fake".to_owned(),
                version: "0.1.0".to_owned(),
                state: PackageState::Enabled,
                decl: Ok(self.0.clone()),
            }]
        }
    }

    /// A kind declared at this test's executable, with the given one-shot form if any.
    fn fake_kind(oneshot: Option<Value>) -> String {
        let program = std::env::current_exe().expect("the test executable");
        let mut kind = json!({
            "id": "fake", "name": "Fake", "program": [program.to_string_lossy()],
            "credentials": { "home_env": "FAKE_HOME", "files": ["token.json"] }
        });
        if let Some(oneshot) = oneshot {
            kind["oneshot"] = oneshot;
        }
        kind.to_string()
    }

    fn request(kind: &str, account: Option<&str>) -> TitleRequest {
        TitleRequest {
            kind: kind.to_owned(),
            account: account.map(str::to_owned),
            model: None,
            prompt: "Fix the login bug".to_owned(),
            reply: None,
        }
    }

    #[test]
    fn a_kind_without_a_one_shot_form_is_refused_before_any_run() {
        let (agents, _, _profile) =
            crate::sessions::tests::service_with(Arc::new(OneKind(fake_kind(None))));
        let error = agents
            .generate_title(request("fake", None), Duration::from_secs(1))
            .expect_err("refused");
        assert_eq!(error.code, codes::AGENT_ONESHOT_UNSUPPORTED);
    }

    #[test]
    fn a_kind_with_no_account_to_run_with_is_refused_with_credentials_invalid() {
        let oneshot = json!({ "args": ["-p", "{prompt}"] });
        let (agents, _, _profile) =
            crate::sessions::tests::service_with(Arc::new(OneKind(fake_kind(Some(oneshot)))));
        let none = agents
            .generate_title(request("fake", None), Duration::from_secs(1))
            .expect_err("refused");
        assert_eq!(none.code, codes::AGENT_CREDENTIALS_INVALID);
        let unknown = agents
            .generate_title(
                request("fake", Some("no-such-account")),
                Duration::from_secs(1),
            )
            .expect_err("refused");
        assert_eq!(unknown.code, codes::AGENT_CREDENTIALS_INVALID);
    }
}
