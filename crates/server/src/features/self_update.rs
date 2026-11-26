//! `host.update_*` (capability `self-update/1`, decision 0132): a `host-admin` client uploads a binary in chunks, the
//! service checks its length, sha256 and `--version`, swaps it in keeping `.previous`, and asks its supervisor to restart.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use massorca_core_remote::self_update::{self as host_update, Supervisor, UpdatePaths};
use massorca_core_store::new_id;
use massorca_protocol::methods::self_update::{
    self as methods, MAX_BINARY_BYTES, MAX_CHUNK_BYTES, UpdateBeginParams, UpdateBegun,
    UpdateChunkAccepted, UpdateChunkParams, UpdateFinishParams, UpdateInstalled,
    UpdateRollbackParams, codes, compare_versions, overlaps, parse_version_line,
};
use massorca_protocol::{
    ErrorBody, MIN_PROTOCOL_VERSION, PROTOCOL_VERSION, ProtocolRange, capabilities,
};
use sha2::{Digest, Sha256};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

/// Tests only: the binary an update replaces instead of this process's own.
pub const TEST_TARGET_ENV: &str = "MASSORCA_TEST_SELF_UPDATE_TARGET";
/// Tests only: a program run as `<program> restart <binary>` instead of systemd.
pub const TEST_RESTART_ENV: &str = "MASSORCA_TEST_SELF_UPDATE_RESTART";
/// Lets the answer reach the client before the supervisor stops this process.
const RESTART_DELAY: Duration = Duration::from_millis(300);

/// What an update replaces and what restarts the service afterwards.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelfUpdateOptions {
    /// The installed binary. `None` refuses every update.
    pub target: Option<PathBuf>,
    /// `None` refuses every update before anything is uploaded.
    pub supervisor: Option<Supervisor>,
}

impl SelfUpdateOptions {
    /// This process's own binary and the supervisor it runs under, unless the test variables name others.
    #[must_use]
    pub fn from_env() -> Self {
        let target = std::env::var_os(TEST_TARGET_ENV)
            .map(PathBuf::from)
            .or_else(|| std::env::current_exe().ok());
        let supervisor = std::env::var_os(TEST_RESTART_ENV)
            .map(|program| Supervisor::Program(PathBuf::from(program)))
            .or_else(Supervisor::detect);
        Self { target, supervisor }
    }
}

/// The one upload in progress, and a lock that keeps a swap and a rollback apart.
#[derive(Debug, Default)]
pub(crate) struct SelfUpdate {
    options: SelfUpdateOptions,
    upload: Arc<Mutex<Option<Upload>>>,
    swap: tokio::sync::Mutex<()>,
}

impl SelfUpdate {
    pub(crate) fn new(options: SelfUpdateOptions) -> Self {
        Self {
            options,
            ..Self::default()
        }
    }

    /// The installed binary and the supervisor, or why this host cannot update itself.
    fn ready(&self) -> Result<(UpdatePaths, Supervisor), ErrorBody> {
        match (&self.options.target, &self.options.supervisor) {
            (Some(target), Some(supervisor)) => Ok((UpdatePaths::of(target), supervisor.clone())),
            _ => Err(ErrorBody::new(
                codes::UNSUPPORTED,
                "this service cannot update itself: it does not run as the systemd user unit massorca.service, so \
                 nothing would restart it; upgrade it with host add-ssh or by hand",
            )),
        }
    }
}

#[derive(Debug)]
struct Upload {
    id: String,
    size: u64,
    sha256: String,
    received: u64,
    hasher: Sha256,
    file: File,
    staged: PathBuf,
}

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    Ok(vec![
        registries.capabilities.register(capabilities::SELF_UPDATE, Owner::Core)?,
        methods.register(
            Owner::Core,
            methods::UPDATE_BEGIN,
            capabilities::SELF_UPDATE,
            Permission::HostAdmin,
            |ctx: RequestCtx, params: UpdateBeginParams| async move { begin(&ctx, params).await },
        )?,
        methods.register(
            Owner::Core,
            methods::UPDATE_CHUNK,
            capabilities::SELF_UPDATE,
            Permission::HostAdmin,
            |ctx: RequestCtx, params: UpdateChunkParams| async move { chunk(&ctx, params).await },
        )?,
        methods.register(
            Owner::Core,
            methods::UPDATE_FINISH,
            capabilities::SELF_UPDATE,
            Permission::HostAdmin,
            |ctx: RequestCtx, params: UpdateFinishParams| async move { finish(&ctx, params).await },
        )?,
        methods.register(
            Owner::Core,
            methods::UPDATE_ROLLBACK,
            capabilities::SELF_UPDATE,
            Permission::HostAdmin,
            |ctx: RequestCtx, params: UpdateRollbackParams| async move { rollback(&ctx, params).await },
        )?,
    ])
}

/// Starts an upload, replacing any unfinished one. Refused before any byte when nothing would restart the service.
async fn begin(ctx: &RequestCtx, params: UpdateBeginParams) -> Result<UpdateBegun, ErrorBody> {
    let updates = &ctx.state.self_update;
    let (paths, supervisor) = updates.ready()?;
    if params.size == 0 || params.size > MAX_BINARY_BYTES {
        return Err(ErrorBody::new(
            codes::TOO_LARGE,
            format!(
                "a binary is 1 byte to {MAX_BINARY_BYTES} bytes; this one is {} bytes",
                params.size
            ),
        ));
    }
    let sha_ok = params.sha256.len() == 64
        && params
            .sha256
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    if !sha_ok {
        return Err(invalid("sha256 is 64 lowercase hex digits"));
    }
    let _swap = updates.swap.lock().await;
    let staged = paths.staged.clone();
    let file = blocking(move || create_staged(&staged))
        .await?
        .map_err(|error| failed("creating the staging file", &error))?;
    let id = new_id();
    // A replaced upload's file is closed here; its staging path is reused by this one.
    let _replaced = updates
        .upload
        .lock()
        .map_err(|_| poisoned())?
        .replace(Upload {
            id: id.clone(),
            size: params.size,
            sha256: params.sha256,
            received: 0,
            hasher: Sha256::new(),
            file,
            staged: paths.staged,
        });
    Ok(UpdateBegun {
        upload_id: id,
        max_chunk_bytes: MAX_CHUNK_BYTES as u32,
        supervisor: supervisor.name().to_owned(),
    })
}

async fn chunk(
    ctx: &RequestCtx,
    params: UpdateChunkParams,
) -> Result<UpdateChunkAccepted, ErrorBody> {
    let bytes = params
        .decode_data()
        .filter(|bytes| bytes.len() <= MAX_CHUNK_BYTES)
        .ok_or_else(|| invalid("data is padded standard base64 of at most 512 KiB"))?;
    let upload = Arc::clone(&ctx.state.self_update.upload);
    blocking(move || {
        let mut guard = upload.lock().map_err(|_| poisoned())?;
        let current = current(&mut guard, &params.upload_id)?;
        if params.offset != current.received {
            return Err(bad_chunk(format!(
                "the chunk starts at {}, the upload has {} bytes",
                params.offset, current.received
            )));
        }
        let end = current.received + bytes.len() as u64;
        if end > current.size {
            return Err(ErrorBody::new(
                codes::TOO_LARGE,
                format!(
                    "the chunk ends at {end} bytes, past the announced {} bytes",
                    current.size
                ),
            ));
        }
        current
            .file
            .write_all(&bytes)
            .map_err(|error| failed("writing the staging file", &error))?;
        current.hasher.update(&bytes);
        current.received = end;
        Ok(UpdateChunkAccepted { received: end })
    })
    .await?
}

/// Checks the upload and swaps it in, then restarts the service once the answer is sent. Any refusal deletes the staged
/// file and leaves the installed binary as it was.
async fn finish(
    ctx: &RequestCtx,
    params: UpdateFinishParams,
) -> Result<UpdateInstalled, ErrorBody> {
    let updates = &ctx.state.self_update;
    let (paths, supervisor) = updates.ready()?;
    // Taken before the upload, so no new upload can rewrite the staging file while it is checked.
    let _swap = updates.swap.lock().await;
    let upload = {
        let mut guard = updates.upload.lock().map_err(|_| poisoned())?;
        current(&mut guard, &params.upload_id)?;
        guard.take().ok_or_else(unknown_upload)?
    };
    let checked = paths.clone();
    let running = ctx.state.server.version.clone();
    let installed = blocking(move || {
        let staged = upload.staged.clone();
        let outcome = verify(upload, &checked).and_then(|version| {
            version_policy(&running, &version, &params)?;
            host_update::swap_in(&checked)
                .map_err(|error| failed("replacing the binary", &error))?;
            Ok(version)
        });
        if outcome.is_err() {
            let _ = std::fs::remove_file(&staged);
        }
        outcome
    })
    .await??;
    tracing::info!(version = %installed.version, "a new service binary is installed; restarting");
    Ok(restart_after_answer(ctx, paths, supervisor, installed))
}

/// Puts `.previous` back and restarts; a dry run only reports the kept binary's version. A rollback may lower the
/// version: that is its purpose, and the person confirmed the version a dry run reported.
async fn rollback(
    ctx: &RequestCtx,
    params: UpdateRollbackParams,
) -> Result<UpdateInstalled, ErrorBody> {
    let updates = &ctx.state.self_update;
    let (paths, supervisor) = updates.ready()?;
    let _swap = updates.swap.lock().await;
    let checked = paths.clone();
    let dry_run = params.dry_run;
    let installed = blocking(move || {
        if !checked.previous.is_file() {
            return Err(ErrorBody::new(
                codes::NO_PREVIOUS,
                "no previous binary is kept on this host",
            ));
        }
        let version = checked_version(&checked.previous)?;
        expect_version(&version, params.expected_version.as_deref())?;
        if !dry_run {
            host_update::roll_back(&checked)
                .map_err(|error| failed("restoring the previous binary", &error))?;
        }
        Ok(version)
    })
    .await??;
    if dry_run {
        return Ok(answer(ctx, &supervisor, installed));
    }
    tracing::info!(version = %installed.version, "the previous service binary is restored; restarting");
    Ok(restart_after_answer(ctx, paths, supervisor, installed))
}

/// The installed version, for the answer; the restart runs after a short delay so the answer is sent first.
fn restart_after_answer(
    ctx: &RequestCtx,
    paths: UpdatePaths,
    supervisor: Supervisor,
    installed: Checked,
) -> UpdateInstalled {
    let answer = answer(ctx, &supervisor, installed);
    tokio::spawn(async move {
        tokio::time::sleep(RESTART_DELAY).await;
        let restarted =
            tokio::task::spawn_blocking(move || supervisor.restart(&paths.installed)).await;
        match restarted {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                tracing::error!(%error, "the service did not restart after its update")
            }
            Err(error) => tracing::error!(%error, "the restart step did not finish"),
        }
    });
    answer
}

fn answer(ctx: &RequestCtx, supervisor: &Supervisor, installed: Checked) -> UpdateInstalled {
    UpdateInstalled {
        previous_version: ctx.state.server.version.clone(),
        version: installed.version,
        protocol: installed.protocol,
        supervisor: supervisor.name().to_owned(),
    }
}

/// The host's own downgrade rule, whatever the client checked: older needs `allow_downgrade`, the same needs `force`.
fn version_policy(
    running: &str,
    checked: &Checked,
    params: &UpdateFinishParams,
) -> Result<(), ErrorBody> {
    expect_version(checked, params.expected_version.as_deref())?;
    match compare_versions(&checked.version, running) {
        Some(std::cmp::Ordering::Greater) => Ok(()),
        Some(std::cmp::Ordering::Equal) if params.force => Ok(()),
        Some(std::cmp::Ordering::Equal) => Err(ErrorBody::new(
            codes::SAME_VERSION,
            format!("the service already runs {running}: nothing to do (force reinstalls it)"),
        )),
        _ if params.allow_downgrade => Ok(()),
        order => Err(ErrorBody::new(
            codes::DOWNGRADE,
            format!(
                "the binary is {}, {} the running {running}; allow_downgrade installs it",
                checked.version,
                if order.is_some() {
                    "older than"
                } else {
                    "not comparable with"
                }
            ),
        )),
    }
}

fn expect_version(checked: &Checked, expected: Option<&str>) -> Result<(), ErrorBody> {
    match expected {
        Some(expected) if expected != checked.version => Err(ErrorBody::new(
            codes::VERSION_MISMATCH,
            format!(
                "the binary reports {}, not the confirmed {expected}",
                checked.version
            ),
        )),
        _ => Ok(()),
    }
}

/// What a checked binary reports.
struct Checked {
    version: String,
    protocol: ProtocolRange,
}

/// Length, sha256, then `--version` of the staged file.
fn verify(mut upload: Upload, paths: &UpdatePaths) -> Result<Checked, ErrorBody> {
    let sha = hex(&upload.hasher.finalize_reset());
    if upload.received != upload.size || sha != upload.sha256 {
        return Err(ErrorBody::new(
            codes::CHECKSUM_MISMATCH,
            format!(
                "the upload has {} of {} bytes and sha256 {sha}, not {}; the installed binary is unchanged",
                upload.received, upload.size, upload.sha256
            ),
        ));
    }
    upload
        .file
        .sync_all()
        .map_err(|error| failed("writing the staging file", &error))?;
    drop(upload.file);
    checked_version(&paths.staged)
}

/// Runs `binary --version` and requires a protocol range that overlaps this service's.
fn checked_version(binary: &std::path::Path) -> Result<Checked, ErrorBody> {
    let line = host_update::probe_version(binary)
        .map_err(|why| incompatible(format!("the binary was refused: {why}")))?;
    let parsed = parse_version_line(&line).ok_or_else(|| {
        incompatible("the binary was refused: its --version does not read `massorca <version> (protocol <min>..<max>)`".to_owned())
    })?;
    let ours = ProtocolRange {
        min: MIN_PROTOCOL_VERSION,
        max: PROTOCOL_VERSION,
    };
    if !overlaps(parsed.protocol, ours) {
        return Err(incompatible(format!(
            "the binary speaks protocol {}..{}, this service speaks {}..{}; clients could not reach it",
            parsed.protocol.min, parsed.protocol.max, ours.min, ours.max
        )));
    }
    Ok(Checked {
        version: parsed.version,
        protocol: parsed.protocol,
    })
}

/// A new staging file, owner-only and executable on Unix.
fn create_staged(path: &std::path::Path) -> std::io::Result<File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o700);
    }
    let file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(file)
}

fn current<'a>(slot: &'a mut Option<Upload>, id: &str) -> Result<&'a mut Upload, ErrorBody> {
    slot.as_mut()
        .filter(|upload| upload.id == id)
        .ok_or_else(unknown_upload)
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ErrorBody> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| ErrorBody::new(codes::FAILED, "the update step did not finish"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn invalid(message: &str) -> ErrorBody {
    ErrorBody::new(massorca_protocol::codes::REQUEST_INVALID_PARAMS, message)
}

fn incompatible(message: String) -> ErrorBody {
    ErrorBody::new(codes::INCOMPATIBLE, message)
}

fn bad_chunk(message: String) -> ErrorBody {
    ErrorBody::new(codes::BAD_CHUNK, message)
}

fn unknown_upload() -> ErrorBody {
    ErrorBody::new(
        codes::UNKNOWN_UPLOAD,
        "no upload with this id is in progress; begin again",
    )
}

fn failed(step: &str, error: &std::io::Error) -> ErrorBody {
    ErrorBody::new(codes::FAILED, format!("{step} failed: {error}"))
}

fn poisoned() -> ErrorBody {
    ErrorBody::new(
        codes::FAILED,
        "the upload state is unusable after a failed step",
    )
}
