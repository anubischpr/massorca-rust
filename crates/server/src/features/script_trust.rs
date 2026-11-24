//! Per-script trust of a project's `orca.yaml` scripts (decision 0092): `project.scripts` and `project.trust_script`
//! (capability `script-trust/1`), and the check a setup or archive script passes before it runs. A script runs only
//! when the owner trusted its exact content: the store keeps one SHA-256 per project and kind, so changed content needs
//! trust again.

use std::path::{Path, PathBuf};

use massorca_core_extensions::source::sha256_hex;
use massorca_core_store::{Store, StoreError};
use massorca_core_workspace::Workspaces;
use massorca_core_workspace::setup::{SETUP_FILE, read_script};
use massorca_protocol::methods::projects::{
    self, Project, ProjectScript, ProjectScripts, ProjectScriptsParams, ProjectTrustScriptParams,
    ScriptKind,
};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::workspace_domain::shared;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

/// The kinds `project.scripts` lists, in this order.
const KINDS: [ScriptKind; 2] = [ScriptKind::Setup, ScriptKind::Archive];

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![
        registries
            .capabilities
            .register(capabilities::SCRIPT_TRUST, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            projects::SCRIPTS,
            capabilities::SCRIPT_TRUST,
            Permission::Read,
            scripts,
        )?,
        // Trusting lets the host run the script for every later create or removal, so it takes host admin.
        registries.methods.register(
            Owner::Core,
            projects::TRUST_SCRIPT,
            capabilities::SCRIPT_TRUST,
            Permission::HostAdmin,
            trust_script,
        )?,
    ])
}

/// Lowercase hex SHA-256 of the script, trimmed as the reference trims it before hashing.
pub(crate) fn script_hash(script: &str) -> String {
    sha256_hex(script.trim().as_bytes())
}

/// Why `script` may not run: `None` when the owner trusted this exact content, else the reason a client can show and the
/// script's hash. `workspace` names the workspace whose copy was read, for the command the reason suggests.
pub(crate) fn untrusted(
    store: &Store,
    project: &Project,
    kind: ScriptKind,
    script: &str,
    workspace: Option<&str>,
) -> Result<Option<(String, String)>, ErrorBody> {
    let hash = script_hash(script);
    let trusted = store
        .trusted_script_hash(&project.id, kind.as_str())
        .map_err(store_error)?;
    if trusted.as_deref() == Some(hash.as_str()) {
        return Ok(None);
    }
    let state = if trusted.is_some() {
        "changed since it was trusted"
    } else {
        "is not trusted"
    };
    let mut command = format!("massorca project trust-scripts {}", quoted(&project.name));
    if let Some(workspace) = workspace {
        command.push_str(&format!(" --workspace {}", quoted(workspace)));
    }
    Ok(Some((
        format!(
            "the {} script in {SETUP_FILE} {state}, so it did not run; read it and trust it with `{command}`",
            kind.as_str()
        ),
        hash,
    )))
}

fn quoted(text: &str) -> String {
    if text.contains(char::is_whitespace) {
        format!("\"{text}\"")
    } else {
        text.to_owned()
    }
}

async fn scripts(
    ctx: RequestCtx,
    params: ProjectScriptsParams,
) -> Result<ProjectScripts, ErrorBody> {
    let domain = shared(&ctx);
    let store = ctx.state.store.clone();
    on_pool(move || {
        let (project, dir) = source(&domain, &params.project, params.workspace.as_deref())?;
        let mut scripts = Vec::new();
        // Removal reads the archive script from the project's copy only, so a workspace's copy shows setup alone.
        let kinds: &[ScriptKind] = if params.workspace.is_some() {
            &[ScriptKind::Setup]
        } else {
            &KINDS
        };
        for &kind in kinds {
            if let Some(script) = read(&dir, kind)? {
                scripts.push(shown(&store, &project, kind, script)?);
            }
        }
        Ok(ProjectScripts {
            project: project.id,
            source: dir.join(SETUP_FILE).to_string_lossy().into_owned(),
            scripts,
        })
    })
    .await
}

async fn trust_script(
    ctx: RequestCtx,
    params: ProjectTrustScriptParams,
) -> Result<ProjectScript, ErrorBody> {
    let domain = shared(&ctx);
    let store = ctx.state.store.clone();
    on_pool(move || {
        if !KINDS.contains(&params.kind) {
            return Err(ErrorBody::new(
                codes::REQUEST_INVALID_PARAMS,
                "kind must be setup or archive",
            ));
        }
        if params.kind == ScriptKind::Archive && params.workspace.is_some() {
            return Err(ErrorBody::new(
                codes::REQUEST_INVALID_PARAMS,
                "the archive script is read from the project's orca.yaml; trust it without a workspace",
            ));
        }
        let (project, dir) = source(&domain, &params.project, params.workspace.as_deref())?;
        let kind = params.kind.as_str();
        let script = read(&dir, params.kind)?.ok_or_else(|| {
            ErrorBody::new(
                codes::SCRIPT_NOT_FOUND,
                format!("{} defines no {kind} script", dir.join(SETUP_FILE).display()),
            )
        })?;
        if script_hash(&script) != params.hash.trim().to_ascii_lowercase() {
            return Err(ErrorBody::new(
                codes::SCRIPT_CHANGED,
                format!(
                    "the {kind} script is not the content with hash {}; show it again and trust the hash shown",
                    params.hash
                ),
            ));
        }
        store
            .trust_script(&project.id, kind, &script_hash(&script))
            .map_err(store_error)?;
        shown(&store, &project, params.kind, script)
    })
    .await
}

/// The project, and the folder whose `orca.yaml` is read: the project's, or that of `workspace` when given.
fn source(
    domain: &Workspaces,
    project: &str,
    workspace: Option<&str>,
) -> Result<(Project, PathBuf), ErrorBody> {
    let project = domain.show_project(project).map_err(ErrorBody::from)?;
    let dir = match workspace {
        None => PathBuf::from(&project.path),
        Some(selector) => {
            let workspace = domain.show_workspace(selector).map_err(ErrorBody::from)?;
            if workspace.project_id != project.id {
                return Err(ErrorBody::new(
                    codes::WORKSPACE_NOT_FOUND,
                    format!(
                        "workspace {selector:?} is not a workspace of project {}",
                        project.name
                    ),
                ));
            }
            PathBuf::from(workspace.path)
        }
    };
    Ok((project, dir))
}

fn read(dir: &Path, kind: ScriptKind) -> Result<Option<String>, ErrorBody> {
    read_script(dir, kind.as_str())
        .map_err(|error| ErrorBody::new(codes::REQUEST_INVALID_PARAMS, error.to_string()))
}

fn shown(
    store: &Store,
    project: &Project,
    kind: ScriptKind,
    script: String,
) -> Result<ProjectScript, ErrorBody> {
    let hash = script_hash(&script);
    let trusted = store
        .trusted_script_hash(&project.id, kind.as_str())
        .map_err(store_error)?;
    Ok(ProjectScript {
        kind,
        trusted: trusted.as_deref() == Some(hash.as_str()),
        changed: trusted.is_some_and(|trusted| trusted != hash),
        script,
        hash,
    })
}

pub(crate) fn store_error(error: StoreError) -> ErrorBody {
    ErrorBody::new(codes::INTERNAL, error.to_string())
}

/// Runs `call` on the blocking pool.
async fn on_pool<T, F>(call: F) -> Result<T, ErrorBody>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, ErrorBody> + Send + 'static,
{
    tokio::task::spawn_blocking(call)
        .await
        .unwrap_or_else(|error| {
            Err(ErrorBody::new(
                codes::INTERNAL,
                format!("the script task failed: {error}"),
            ))
        })
}
