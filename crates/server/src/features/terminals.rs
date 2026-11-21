//! The terminal methods except the attach stream (capability `terminals/1`, decision 0035). Blocking manager calls run on the
//! blocking pool; the attach stream lives in `terminal_attach`.

use std::path::{Path, PathBuf};

use massorca_core_terminal::pty::PtySize;
use massorca_core_terminal::{
    CreateSpec, ExitReason as CoreExitReason, ExitRecord, ShellChoice, TerminalError, TerminalInfo,
    TerminalState as CoreTerminalState,
};
use massorca_protocol::methods::terminals::{
    self, DEFAULT_COLS, DEFAULT_ROWS, ExitReason, Terminal, TerminalCreateParams, TerminalExit,
    TerminalList, TerminalListParams, TerminalRef, TerminalState,
};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::workspace_domain::{blocking, shared};
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![
        registries
            .capabilities
            .register(capabilities::TERMINALS, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            terminals::CREATE,
            capabilities::TERMINALS,
            Permission::Workspace,
            create,
        )?,
        registries.methods.register(
            Owner::Core,
            terminals::LIST,
            capabilities::TERMINALS,
            Permission::Read,
            list,
        )?,
        registries.methods.register(
            Owner::Core,
            terminals::SHOW,
            capabilities::TERMINALS,
            Permission::Read,
            show,
        )?,
    ])
}

async fn create(ctx: RequestCtx, params: TerminalCreateParams) -> Result<Terminal, ErrorBody> {
    let domain = shared(&ctx);
    let selector = params.workspace;
    let workspace = blocking(move || domain.show_workspace(&selector)).await?;
    if workspace.missing {
        return Err(ErrorBody::new(
            codes::PATH_NOT_FOUND,
            format!("the workspace directory is missing: {}", workspace.path),
        ));
    }
    let spec = CreateSpec {
        cwd: resolve_cwd(&workspace.path, params.cwd.as_deref()),
        workspace_id: workspace.id,
        shell: params.shell.map(|shell| ShellChoice {
            program: shell.program,
            args: shell.args,
        }),
        env: params.env,
        size: PtySize {
            cols: params.cols.unwrap_or(DEFAULT_COLS),
            rows: params.rows.unwrap_or(DEFAULT_ROWS),
        },
        title: params.title,
        request_id: params.request_id,
    };
    let manager = ctx.state.terminals.clone();
    let info = on_manager(move || manager.create(spec)).await?;
    Ok(terminal_of(info))
}

async fn list(ctx: RequestCtx, params: TerminalListParams) -> Result<TerminalList, ErrorBody> {
    let workspace_id = match params.workspace {
        Some(selector) => {
            let domain = shared(&ctx);
            Some(blocking(move || domain.show_workspace(&selector)).await?.id)
        }
        None => None,
    };
    let terminals = ctx
        .state
        .terminals
        .list(workspace_id.as_deref())
        .into_iter()
        .map(terminal_of)
        .collect();
    Ok(TerminalList { terminals })
}

async fn show(ctx: RequestCtx, params: TerminalRef) -> Result<Terminal, ErrorBody> {
    ctx.state
        .terminals
        .get(&params.terminal)
        .map(terminal_of)
        .map_err(error_of)
}

/// `cwd` relative to the workspace root, or absolute as given (`Path::join` keeps an absolute path as is).
fn resolve_cwd(workspace_path: &str, cwd: Option<&str>) -> PathBuf {
    let root = Path::new(workspace_path);
    match cwd {
        Some(cwd) => root.join(cwd),
        None => root.to_path_buf(),
    }
}

/// Runs one manager call on the blocking pool and maps its refusal to the protocol error.
pub(crate) async fn on_manager<T, F>(call: F) -> Result<T, ErrorBody>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, TerminalError> + Send + 'static,
{
    match tokio::task::spawn_blocking(call).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(error_of(error)),
        Err(error) => Err(ErrorBody::new(
            codes::INTERNAL,
            format!("the terminal task failed: {error}"),
        )),
    }
}

pub(crate) fn error_of(error: TerminalError) -> ErrorBody {
    let code = match &error {
        TerminalError::NotFound => codes::TERMINAL_NOT_FOUND,
        TerminalError::Exited => codes::TERMINAL_EXITED,
        TerminalError::InvalidSize => codes::TERMINAL_INVALID_SIZE,
        TerminalError::CwdNotFound(_) => codes::PATH_NOT_FOUND,
        TerminalError::CwdNotDirectory(_) => codes::PATH_NOT_DIRECTORY,
        TerminalError::ShellNotFound(_) | TerminalError::Spawn(_) => codes::TERMINAL_SPAWN_FAILED,
        TerminalError::LimitReached => codes::TERMINAL_LIMIT_REACHED,
        TerminalError::InputTooLarge => codes::TERMINAL_INPUT_TOO_LARGE,
        TerminalError::Io(_) => codes::INTERNAL,
    };
    ErrorBody::new(code, error.to_string())
}

pub(crate) fn terminal_of(info: TerminalInfo) -> Terminal {
    Terminal {
        id: info.id,
        workspace_id: info.workspace_id,
        state: state_of(info.state),
        title: info.title,
        cols: info.cols,
        rows: info.rows,
        pid: info.pid,
        shell: info.shell,
        cwd: info.cwd,
        created_at_ms: info.created_at_ms,
        exit: info.exit.map(exit_of),
    }
}

fn state_of(state: CoreTerminalState) -> TerminalState {
    match state {
        CoreTerminalState::Live => TerminalState::Live,
        CoreTerminalState::Unverifiable => TerminalState::Unverifiable,
        CoreTerminalState::Exited => TerminalState::Exited,
    }
}

fn exit_of(record: ExitRecord) -> TerminalExit {
    TerminalExit {
        code: record.code,
        signal: record.signal,
        reason: match record.reason {
            CoreExitReason::Exited => ExitReason::Exited,
            CoreExitReason::Killed => ExitReason::Killed,
            CoreExitReason::ServiceStopped => ExitReason::ServiceStopped,
        },
        at_ms: record.at_ms,
    }
}
