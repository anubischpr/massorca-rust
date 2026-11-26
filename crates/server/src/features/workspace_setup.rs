//! Runs a new worktree's setup script in a service terminal (decision 0086): every client can list and attach to it, so
//! the script never runs unseen. Only `workspace.create`, which needs the `workspace` permission, calls this, and only a
//! script whose exact content the owner trusted runs (decision 0095).

use std::path::{Path, PathBuf};

use massorca_core_terminal::pty::PtySize;
use massorca_core_terminal::shell::{EnvSnapshot, resolve_program};
use massorca_core_terminal::{CreateSpec, EnvValue, ShellChoice};
use massorca_core_workspace::setup::read_setup_script;
use massorca_core_workspace::setup_runner::{
    Interpreters, RUNNER_DIR, RunnerHost, check_cmd_values, plan_runner, setup_env, write_runner,
};
use massorca_protocol::methods::projects::ScriptKind;
use massorca_protocol::methods::terminals::{DEFAULT_COLS, DEFAULT_ROWS};
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceKind, WorkspaceSetup};

use crate::context::RequestCtx;
use crate::features::script_trust;

/// The setup terminal's title.
const TITLE: &str = "Setup";

/// Starts the setup of the worktree `workspace` just created; `None` when it is a folder or has no script. A setup that
/// cannot start is reported, never fails the create: the worktree already exists.
pub(crate) async fn start(
    ctx: &RequestCtx,
    workspace: &Workspace,
    no_setup: bool,
) -> Option<WorkspaceSetup> {
    if workspace.kind != WorkspaceKind::Worktree {
        return None;
    }
    let domain = std::sync::Arc::clone(&ctx.state.workspaces);
    let terminals = ctx.state.terminals.clone();
    let store = ctx.state.store.clone();
    let workspace = workspace.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        let script = match read_setup_script(Path::new(&workspace.path)) {
            Ok(Some(script)) => script,
            Ok(None) => return None,
            Err(error) => return Some(failed(&workspace, error.to_string())),
        };
        if no_setup {
            return Some(WorkspaceSetup::Skipped);
        }
        let project = match domain.show_project(&workspace.project_id) {
            Ok(project) => project,
            Err(error) => return Some(failed(&workspace, error.to_string())),
        };
        match script_trust::untrusted(
            &store,
            &project,
            ScriptKind::Setup,
            &script,
            Some(&workspace.name),
        ) {
            Ok(None) => {}
            Ok(Some((reason, hash))) => return Some(WorkspaceSetup::Untrusted { reason, hash }),
            Err(error) => return Some(failed(&workspace, error.message)),
        }
        let root = project.path;
        let runner_dir = match domain.git().repository_dirs(Path::new(&workspace.path)) {
            Ok((git_dir, _)) => git_dir.join(RUNNER_DIR),
            Err(error) => return Some(failed(&workspace, error.to_string())),
        };
        let host = RunnerHost::current();
        let env = EnvSnapshot::current();
        let runner = match plan_runner(&script, host, &runner_dir, &interpreters(&env)) {
            Ok(runner) => runner,
            Err(refusal) => return Some(failed(&workspace, refusal.to_string())),
        };
        let vars = setup_env(
            &root,
            &workspace.path,
            &workspace.name,
            workspace.branch.as_deref(),
            &runner,
            host,
        );
        if let Err(refusal) = check_cmd_values(&runner, &vars) {
            return Some(failed(&workspace, refusal.to_string()));
        }
        if let Err(error) = write_runner(&runner) {
            return Some(failed(
                &workspace,
                format!("the setup runner could not be written: {error}"),
            ));
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
            // A repeated create of the same workspace finds this terminal instead of running setup again.
            request_id: Some(format!("workspace.setup:{}", workspace.id)),
            holds_secrets: false,
        };
        Some(match terminals.create(spec) {
            Ok(info) => WorkspaceSetup::Started { terminal: info.id },
            Err(error) => failed(
                &workspace,
                format!("the setup terminal did not start: {error}"),
            ),
        })
    })
    .await;
    outcome.unwrap_or_else(|error| {
        Some(WorkspaceSetup::Failed {
            reason: format!("the setup task failed: {error}"),
        })
    })
}

fn failed(workspace: &Workspace, reason: String) -> WorkspaceSetup {
    tracing::warn!(workspace = %workspace.id, reason = %reason, "a workspace setup did not start");
    WorkspaceSetup::Failed { reason }
}

/// The absolute interpreters a runner may use. Never a bare name: Windows would look in the workspace folder first.
pub(crate) fn interpreters(env: &EnvSnapshot) -> Interpreters {
    if cfg!(windows) {
        Interpreters {
            cmd: windows_cmd(env),
            bash: git_bash(env),
        }
    } else {
        Interpreters {
            cmd: None,
            bash: resolve_program("bash", env)
                .filter(|path| path.is_absolute())
                .or_else(|| Some(PathBuf::from("/bin/bash")).filter(|path| path.is_file())),
        }
    }
}

/// `%ComSpec%` when it is an absolute file, else `%SystemRoot%\System32\cmd.exe`.
fn windows_cmd(env: &EnvSnapshot) -> Option<PathBuf> {
    env.get("ComSpec")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && path.is_file())
        .or_else(|| {
            env.get("SystemRoot")
                .map(|root| Path::new(root).join("System32").join("cmd.exe"))
                .filter(|path| path.is_file())
        })
}

/// Git for Windows' `bin\bash.exe`, found beside the git on the path or in Program Files. The `bash.exe` in System32
/// starts WSL, so it is never taken.
fn git_bash(env: &EnvSnapshot) -> Option<PathBuf> {
    let system_root = env.get("SystemRoot").map(|root| root.to_ascii_lowercase());
    let not_system = |path: &PathBuf| {
        system_root.as_deref().is_none_or(|root| {
            !path
                .to_string_lossy()
                .to_ascii_lowercase()
                .starts_with(root)
        })
    };
    let git = env
        .get("MASSORCA_GIT")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| resolve_program("git", env));
    let beside_git = git.into_iter().flat_map(|git| {
        git.ancestors()
            .skip(1)
            .take(3)
            .map(|dir| dir.join("bin").join("bash.exe"))
            .collect::<Vec<_>>()
    });
    let installed = ["ProgramW6432", "ProgramFiles"]
        .into_iter()
        .filter_map(|name| env.get(name))
        .map(|dir| Path::new(dir).join("Git").join("bin").join("bash.exe"));
    beside_git
        .chain(installed)
        .find(|path| is_git_for_windows_bash(path) && not_system(path))
}

/// `<root>/bin/bash.exe` of a Git for Windows install: `<root>` also has `cmd/git.exe` or `git-bash.exe`.
fn is_git_for_windows_bash(bash: &Path) -> bool {
    let Some(root) = bash.parent().and_then(Path::parent) else {
        return false;
    };
    bash.is_file()
        && (root.join("cmd").join("git.exe").is_file() || root.join("git-bash.exe").is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_bash_inside_a_git_for_windows_install_counts_as_git_bash() {
        let root = tempfile::tempdir().expect("temp dir");
        let bash = root.path().join("bin").join("bash.exe");
        std::fs::create_dir_all(bash.parent().expect("bin")).expect("bin");
        std::fs::write(&bash, "").expect("bash");
        assert!(!is_git_for_windows_bash(&bash));

        std::fs::create_dir_all(root.path().join("cmd")).expect("cmd");
        std::fs::write(root.path().join("cmd").join("git.exe"), "").expect("git");
        assert!(is_git_for_windows_bash(&bash));

        let other = tempfile::tempdir().expect("temp dir");
        let other_bash = other.path().join("bin").join("bash.exe");
        std::fs::create_dir_all(other_bash.parent().expect("bin")).expect("bin");
        std::fs::write(&other_bash, "").expect("bash");
        std::fs::write(other.path().join("git-bash.exe"), "").expect("git-bash");
        assert!(is_git_for_windows_bash(&other_bash));
    }
}
