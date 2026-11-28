//! `host.update_*` (capability `self-update/1`, decision 0132): a `host-admin` client uploads a binary in chunks, the
//! service checks its length, sha256 and `--version`, swaps it in keeping `.previous`, and asks its supervisor to restart.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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

/// Lets the answer reach the client before the supervisor stops this process.
const RESTART_DELAY: Duration = Duration::from_millis(300);

/// How long an upload may go without a chunk before it is dropped with its staging file.
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// What an update replaces and what restarts the service afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfUpdateOptions {
    /// The installed binary. `None` refuses every update.
    pub target: Option<PathBuf>,
    /// `None` refuses every update before anything is uploaded.
    pub supervisor: Option<Supervisor>,
    /// An upload with no chunk for this long is dropped with its staging file.
    pub idle_timeout: Duration,
}

impl Default for SelfUpdateOptions {
    fn default() -> Self {
        Self {
            target: None,
            supervisor: None,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
        }
    }
}

impl SelfUpdateOptions {
    /// This process's own binary and the supervisor it runs under. No variable changes either: tests inject theirs
    /// through `Service::start_with_self_update`.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            target: std::env::current_exe().ok(),
            supervisor: Supervisor::detect(),
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
        }
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
    /// Deletes the staging files a previous run left: no upload survives a restart.
    pub(crate) fn new(options: SelfUpdateOptions) -> Self {
        if let Some(target) = &options.target {
            UpdatePaths::of(target).remove_stale_staging();
        }
        Self {
            options,
            ..Self::default()
        }
    }

    /// A guard that drops the upload `connection_id` began when the guard drops, on every way a connection ends.
    pub(crate) fn release_on_drop(self: &Arc<Self>, connection_id: &str) -> UploadRelease {
        UploadRelease {
            updates: Arc::clone(self),
            connection_id: connection_id.to_owned(),
        }
    }

    /// Drops the upload the closed connection began, with its staging file.
    pub(crate) fn connection_closed(&self, connection_id: &str) {
        let Ok(mut slot) = self.upload.lock() else {
            return;
        };
        if slot
            .as_ref()
            .is_some_and(|upload| upload.connection_id == connection_id)
            && let Some(upload) = slot.take()
        {
            drop(upload.file);
            let _ = std::fs::remove_file(&upload.staged);
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

/// Releases a connection's upload when dropped; see [`SelfUpdate::release_on_drop`].
pub(crate) struct UploadRelease {
    updates: Arc<SelfUpdate>,
    connection_id: String,
}

impl Drop for UploadRelease {
    fn drop(&mut self) {
        self.updates.connection_closed(&self.connection_id);
    }
}

#[derive(Debug)]
struct Upload {
    id: String,
    /// The connection that began it: only it may add chunks or finish, and closing it drops the upload.
    connection_id: String,
    /// Who began it, for the refusal another caller gets.
    owner: String,
    /// The last begin or chunk; an upload idle for the timeout is dropped.
    last_active: Instant,
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

/// Starts an upload. Refused before any byte when nothing would restart the service, or while another connection's upload
/// is in progress; the same connection's unfinished upload is replaced.
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
    let id = new_id();
    let staged = paths.staged(&id);
    let mut slot = updates.upload.lock().map_err(|_| poisoned())?;
    if let Some(other) = slot
        .as_ref()
        .filter(|upload| upload.connection_id != ctx.connection_id)
    {
        return Err(ErrorBody::new(
            codes::IN_PROGRESS,
            format!(
                "an update from {} is already in progress on this host; try again when it ends",
                other.owner
            ),
        ));
    }
    let file = host_update::create_new(&staged)
        .map_err(|error| failed("creating the staging file", &error))?;
    let owner = ctx.principal.device_id().map_or_else(
        || "the host's owner".to_owned(),
        |id| format!("device {id}"),
    );
    let replaced = slot.replace(Upload {
        id: id.clone(),
        connection_id: ctx.connection_id.clone(),
        owner,
        last_active: Instant::now(),
        size: params.size,
        sha256: params.sha256,
        received: 0,
        hasher: Sha256::new(),
        file,
        staged,
    });
    drop(slot);
    if let Some(replaced) = replaced {
        drop(replaced.file);
        let _ = std::fs::remove_file(&replaced.staged);
    }
    drop_when_idle(
        Arc::clone(&updates.upload),
        id.clone(),
        updates.options.idle_timeout,
    );
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
    let connection_id = ctx.connection_id.clone();
    blocking(move || {
        let mut guard = upload.lock().map_err(|_| poisoned())?;
        let current = current(&mut guard, &params.upload_id, &connection_id)?;
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
        current.last_active = Instant::now();
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
    let _swap = updates.swap.lock().await;
    let upload = {
        let mut guard = updates.upload.lock().map_err(|_| poisoned())?;
        current(&mut guard, &params.upload_id, &ctx.connection_id)?;
        guard.take().ok_or_else(unknown_upload)?
    };
    // Like every refusal of a finish, this one ends the upload.
    if params.expected_version.is_none() {
        drop(upload.file);
        let _ = std::fs::remove_file(&upload.staged);
        return Err(invalid(
            "expected_version is required: the version the person confirmed",
        ));
    }
    let checked = paths.clone();
    let running = ctx.state.server.version.clone();
    let installed = blocking(move || {
        let staged = upload.staged.clone();
        let outcome = verify(upload).and_then(|version| {
            version_policy(&running, &version, &params)?;
            host_update::swap_in(&checked, &staged)
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
    if !params.dry_run && params.expected_version.is_none() {
        return Err(invalid(
            "expected_version is required: the version a dry run named",
        ));
    }
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

/// Length and sha256 of the received stream, then of the file on disk, which is what gets installed; then `--version`.
fn verify(mut upload: Upload) -> Result<Checked, ErrorBody> {
    let streamed = hex(&upload.hasher.finalize_reset());
    let (announced_size, announced_sha) = (upload.size, upload.sha256.clone());
    let mismatch = |what: &str, size: u64, sha: &str| {
        ErrorBody::new(
            codes::CHECKSUM_MISMATCH,
            format!(
                "the {what} has {size} of {announced_size} bytes and sha256 {sha}, not {announced_sha}; the installed \
                 binary is unchanged"
            ),
        )
    };
    if upload.received != upload.size || streamed != upload.sha256 {
        return Err(mismatch("upload", upload.received, &streamed));
    }
    upload
        .file
        .sync_all()
        .map_err(|error| failed("writing the staging file", &error))?;
    drop(upload.file);
    let (size, on_disk) =
        file_digest(&upload.staged).map_err(|error| failed("reading the staging file", &error))?;
    if size != upload.size || on_disk != upload.sha256 {
        return Err(mismatch("staged file", size, &on_disk));
    }
    checked_version(&upload.staged)
}

/// Length and sha256 of the file at `path`.
fn file_digest(path: &Path) -> std::io::Result<(u64, String)> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 64 * 1024];
    let mut size = 0;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok((size, hex(&hasher.finalize())));
        }
        hasher.update(&buffer[..read]);
        size += read as u64;
    }
}

/// Runs `binary --version` and requires a protocol range that overlaps this service's.
fn checked_version(binary: &Path) -> Result<Checked, ErrorBody> {
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

/// Drops upload `id` with its staging file once it has gone `idle` without a chunk; stops when the upload ends.
fn drop_when_idle(slot: Arc<Mutex<Option<Upload>>>, id: String, idle: Duration) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(idle).await;
            let Ok(mut guard) = slot.lock() else {
                return;
            };
            match guard.as_ref() {
                Some(upload) if upload.id == id && upload.last_active.elapsed() >= idle => {
                    if let Some(upload) = guard.take() {
                        drop(upload.file);
                        let _ = std::fs::remove_file(&upload.staged);
                        tracing::info!("an idle self-update upload was dropped");
                    }
                    return;
                }
                Some(upload) if upload.id == id => {}
                _ => return,
            }
        }
    });
}

/// The upload `id`, only for the connection that began it.
fn current<'a>(
    slot: &'a mut Option<Upload>,
    id: &str,
    connection_id: &str,
) -> Result<&'a mut Upload, ErrorBody> {
    slot.as_mut()
        .filter(|upload| upload.id == id && upload.connection_id == connection_id)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropping_the_release_guard_drops_the_connection_upload_and_its_file() {
        let dir = tempfile::tempdir().expect("a folder");
        let installed = dir.path().join("massorca");
        std::fs::write(&installed, b"old").expect("installed");
        let updates = Arc::new(SelfUpdate::new(SelfUpdateOptions {
            target: Some(installed.clone()),
            ..SelfUpdateOptions::default()
        }));
        let staged = UpdatePaths::of(&installed).staged("01");
        let file = host_update::create_new(&staged).expect("staged");
        *updates.upload.lock().expect("slot") = Some(Upload {
            id: "01".to_owned(),
            connection_id: "c1".to_owned(),
            owner: "the host's owner".to_owned(),
            last_active: Instant::now(),
            size: 1,
            sha256: String::new(),
            received: 0,
            hasher: Sha256::new(),
            file,
            staged: staged.clone(),
        });
        let other = updates.release_on_drop("c2");
        drop(other);
        assert!(
            updates.upload.lock().expect("slot").is_some(),
            "another connection's guard keeps it"
        );
        let guard = updates.release_on_drop("c1");
        drop(guard);
        assert!(updates.upload.lock().expect("slot").is_none());
        assert!(!staged.exists());
    }
}
