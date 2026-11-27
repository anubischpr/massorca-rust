//! Runs a worktree's archive script before the worktree is removed (decision 0095), as the reference does: the
//! project's `orca.yaml` `scripts.archive`, in the worktree, before anything in it is stopped or removed. It runs in a
//! service terminal titled `Archive`, so every client can watch it, and the removal waits for its exit. A failure or a
//! timeout refuses the removal unless the caller waives it; an untrusted script does not run and the removal goes on.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use massorca_core_terminal::pty::PtySize;
use massorca_core_terminal::shell::EnvSnapshot;
use massorca_core_terminal::{CreateSpec, EnvValue, ExitWait, ShellChoice, TerminalManager};
use massorca_core_workspace::Workspaces;
use massorca_core_workspace::setup::{ARCHIVE_KEY, read_script};
use massorca_core_workspace::setup_runner::{
    ARCHIVE_RUNNER, RUNNER_DIR, RunnerHost, check_cmd_values, plan_named_runner, setup_env,
    write_runner,
};
use massorca_protocol::methods::projects::ScriptKind;
use massorca_protocol::methods::terminals::{DEFAULT_COLS, DEFAULT_ROWS};
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceArchive, WorkspaceKind};
use massorca_protocol::{ErrorBody, codes};

use crate::context::RequestCtx;
use crate::features::script_trust;
use crate::features::workspace_setup::interpreters;

/// The archive terminal's title.
const TITLE: &str = "Archive";
/// How long an archive script may run, the reference's hook timeout. A longer run is killed and counts as failed.
const ARCHIVE_TIMEOUT: Duration = Duration::from_secs(120);

/// Runs the archive script of the worktree `workspace`, which `begin_remove` has just marked `removing`. `None` when
/// it is a folder, its directory is gone, or the project has no archive script. A failure is an error unless
/// `allow_failed` waives it, and then it is reported in the reply.
pub(crate) async fn run(
    ctx: &RequestCtx,
    workspace: &Workspace,
    allow_failed: bool,
) -> Result<Option<WorkspaceArchive>, ErrorBody> {
    // The project's own checkout is the person's and is only forgotten, so nothing runs in it (decision 0112).
    if workspace.kind != WorkspaceKind::Worktree
        || workspace.in_place
        || !Path::new(&workspace.path).is_dir()
    {
        return Ok(None);
    }
    let domain = Arc::clone(&ctx.state.workspaces);
    let terminals = ctx.state.terminals.clone();
    let store = ctx.state.store.clone();
    let workspace = workspace.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        archive(&domain, &terminals, &store, &workspace).map(|outcome| (workspace, outcome))
    })
    .await
    .unwrap_or_else(|error| {
        Err(ErrorBody::new(
            codes::INTERNAL,
            format!("the archive task failed: {error}"),
        ))
    })?;
    match outcome {
        (_, None) => Ok(None),
        (_, Some(Outcome::Done(archive))) => Ok(Some(archive)),
        (_, Some(Outcome::Failed { reason, terminal })) if allow_failed => {
            Ok(Some(WorkspaceArchive::Failed { reason, terminal }))
        }
        (workspace, Some(Outcome::Failed { reason, .. })) => Err(ErrorBody::new(
            codes::WORKSPACE_ARCHIVE_FAILED,
            format!(
                "the archive script of workspace {} failed: {reason}. Nothing was stopped or removed; fix the script \
                 and remove it again, or remove it anyway with allow_failed_archive (--allow-failed-archive)",
                workspace.name
            ),
        )),
    }
}

enum Outcome {
    Done(WorkspaceArchive),
    Failed {
        reason: String,
        terminal: Option<String>,
    },
}

fn failed(reason: impl Into<String>) -> Option<Outcome> {
    Some(Outcome::Failed {
        reason: reason.into(),
        terminal: None,
    })
}

fn archive(
    domain: &Workspaces,
    terminals: &TerminalManager,
    store: &massorca_core_store::Store,
    workspace: &Workspace,
) -> Result<Option<Outcome>, ErrorBody> {
    let project = domain
        .show_project(&workspace.project_id)
        .map_err(ErrorBody::from)?;
    // As the reference does, an orca.yaml that cannot be read counts as no archive script, so it never blocks removal.
    let script = match read_script(Path::new(&project.path), ARCHIVE_KEY) {
        Ok(Some(script)) => script,
        Ok(None) => return Ok(None),
        Err(error) => {
            tracing::warn!(project = %project.id, error = %error, "the archive script could not be read");
            return Ok(None);
        }
    };
    if let Some((reason, hash)) =
        script_trust::untrusted(store, &project, ScriptKind::Archive, &script, None)?
    {
        return Ok(Some(Outcome::Done(WorkspaceArchive::Untrusted {
            reason,
            hash,
        })));
    }
    let runner_dir = match domain.git().repository_dirs(Path::new(&workspace.path)) {
        Ok((git_dir, _)) => git_dir.join(RUNNER_DIR),
        Err(error) => return Ok(failed(error.to_string())),
    };
    let host = RunnerHost::current();
    let env = EnvSnapshot::current();
    let runner = match plan_named_runner(
        &script,
        ARCHIVE_RUNNER,
        host,
        &runner_dir,
        &interpreters(&env),
    ) {
        Ok(runner) => runner,
        Err(refusal) => return Ok(failed(refusal.to_string())),
    };
    let vars = setup_env(
        &project.path,
        &workspace.path,
        &workspace.name,
        workspace.branch.as_deref(),
        &runner,
        host,
    );
    if let Err(refusal) = check_cmd_values(&runner, &vars) {
        return Ok(failed(refusal.to_string()));
    }
    if let Err(error) = write_runner(&runner) {
        return Ok(failed(format!(
            "the archive runner could not be written: {error}"
        )));
    }
    let spec = CreateSpec {
        workspace_id: workspace.id.clone(),
        cwd: PathBuf::from(&workspace.path),
        shell: Some(ShellChoice {
            program: runner.program.to_string_lossy().into_owned(),
            args: runner.args,
        }),
        env: vars
            .into_iter()
            .map(|(name, value)| (name, EnvValue::new(value)))
            .collect(),
        size: PtySize {
            cols: DEFAULT_COLS,
            rows: DEFAULT_ROWS,
        },
        title: Some(TITLE.to_owned()),
        // Each removal runs the script afresh, so a retry after a failure never finds the failed run.
        request_id: None,
        holds_secrets: false,
    };
    let terminal = match terminals.create(spec) {
        Ok(info) => info.id,
        Err(error) => {
            return Ok(failed(format!(
                "the archive terminal did not start: {error}"
            )));
        }
    };
    let reason = match terminals.wait_for_exit(&terminal, ARCHIVE_TIMEOUT) {
        Ok(ExitWait::Exited(info)) => match info.exit.as_ref().and_then(|exit| exit.code) {
            Some(0) => {
                return Ok(Some(Outcome::Done(WorkspaceArchive::Succeeded {
                    terminal,
                })));
            }
            Some(code) => format!("it exited {code}"),
            None => "it ended without an exit code".to_owned(),
        },
        Ok(ExitWait::TimedOut(_)) => {
            let _ = terminals.kill(&terminal, true);
            format!(
                "it did not finish within {} seconds and was stopped",
                ARCHIVE_TIMEOUT.as_secs()
            )
        }
        Ok(ExitWait::Gone | ExitWait::TooManyWaits) | Err(_) => {
            "its outcome could not be verified".to_owned()
        }
    };
    Ok(Some(Outcome::Failed {
        reason,
        terminal: Some(terminal),
    }))
}
