//! `skills.discover` (capability `skills/1`, SKL-01): the skills agent tools and the registered projects keep on this
//! host. The scan is `core-extensions`' discovery and runs on the blocking pool; a scan is shared for ten seconds.
//! Read only. The paths of this host reach a host administrator only.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use massorca_core_extensions::skills::{self as core, DiscoveryRequest};
use massorca_protocol::methods::skills::{self as wire, SkillsDiscoverParams};
use massorca_protocol::{ErrorBody, capabilities, codes};

use tokio::sync::watch;

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::workspace_domain::{blocking, shared};
use crate::owner::Owner;
use crate::permission::{Permission, authorize};
use crate::registry::RegistrationHandle;
use crate::service::DiscoveryOptions;

/// How long a scan is shared: a caller within it gets the last scan made for the same request (pv96 shares ten seconds).
const SCAN_TTL: Duration = Duration::from_secs(10);

/// Runs one scan of a request. Production runs `core::discover`; a test can hold or count the scans it runs.
type Scanner = Arc<dyn Fn(&DiscoveryRequest) -> core::SkillDiscovery + Send + Sync>;

/// What skill discovery reads on this host, the scan running now and the last scan it finished.
pub(crate) struct SkillScans {
    options: DiscoveryOptions,
    scanner: Scanner,
    state: Arc<Mutex<ScanState>>,
}

impl fmt::Debug for SkillScans {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkillScans")
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
struct ScanState {
    last: Option<Finished>,
    running: Option<Running>,
}

struct Finished {
    at: Instant,
    request: DiscoveryRequest,
    scan: core::SkillDiscovery,
}

/// A scan in progress: the request it answers, and the channel its result is sent on for every caller that joined it.
struct Running {
    request: DiscoveryRequest,
    done: watch::Receiver<Option<core::SkillDiscovery>>,
}

impl SkillScans {
    pub(crate) fn new(options: DiscoveryOptions) -> Self {
        Self::with_scanner(options, Arc::new(core::discover))
    }

    pub(crate) fn with_scanner(options: DiscoveryOptions, scanner: Scanner) -> Self {
        Self {
            options,
            scanner,
            state: Arc::new(Mutex::new(ScanState::default())),
        }
    }

    /// The scan of `request`. A caller joins a scan of the same request that is already running, `refresh` or not, so
    /// concurrent callers share one run. Without `refresh`, a scan that finished within `SCAN_TTL` answers from memory.
    pub(crate) async fn scan(
        &self,
        request: DiscoveryRequest,
        refresh: bool,
    ) -> Result<core::SkillDiscovery, ErrorBody> {
        let mut done = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(running) = state
                .running
                .as_ref()
                .filter(|running| running.request == request)
            {
                running.done.clone()
            } else {
                let cached = state.last.as_ref().filter(|last| {
                    !refresh && last.request == request && last.at.elapsed() < SCAN_TTL
                });
                if let Some(last) = cached {
                    return Ok(last.scan.clone());
                }
                let (sender, receiver) = watch::channel(None);
                state.running = Some(Running {
                    request: request.clone(),
                    done: receiver.clone(),
                });
                tokio::spawn(run_scan(
                    Arc::clone(&self.scanner),
                    Arc::clone(&self.state),
                    request,
                    sender,
                ));
                receiver
            }
        };
        loop {
            let finished = done.borrow_and_update().clone();
            if let Some(scan) = finished {
                return Ok(scan);
            }
            done.changed().await.map_err(|_| {
                ErrorBody::new(codes::INTERNAL, "the skill scan stopped before it finished")
            })?;
        }
    }
}

/// Runs one scan on the blocking pool and records it: the running slot is freed, the result becomes the last scan and
/// goes to every caller that joined. The task is detached, so a caller that goes away does not stop the scan.
async fn run_scan(
    scanner: Scanner,
    state: Arc<Mutex<ScanState>>,
    request: DiscoveryRequest,
    done: watch::Sender<Option<core::SkillDiscovery>>,
) {
    let scanning = request.clone();
    let scan = tokio::task::spawn_blocking(move || scanner(&scanning)).await;
    let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
    if state
        .running
        .as_ref()
        .is_some_and(|running| running.request == request)
    {
        state.running = None;
    }
    if let Ok(scan) = scan {
        state.last = Some(Finished {
            at: Instant::now(),
            request,
            scan: scan.clone(),
        });
        let _ = done.send_replace(Some(scan));
    }
}

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let cap = capabilities::SKILLS;
    Ok(vec![
        registries.capabilities.register(cap, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            wire::DISCOVER,
            cap,
            Permission::Read,
            discover,
        )?,
    ])
}

async fn discover(
    ctx: RequestCtx,
    params: SkillsDiscoverParams,
) -> Result<wire::SkillDiscovery, ErrorBody> {
    let source_kinds = params
        .source_kinds
        .iter()
        .map(core_kind)
        .collect::<Result<Vec<_>, _>>()?;
    let domain = shared(&ctx);
    let projects = blocking(move || domain.list_projects()).await?;
    let scans = &ctx.state.skills;
    let home = scans.options.home.clone().ok_or_else(|| {
        ErrorBody::new(codes::INTERNAL, "there is no home folder to scan skills in")
    })?;
    let request = DiscoveryRequest {
        home,
        folders: projects
            .iter()
            .map(|project| PathBuf::from(&project.path))
            .collect(),
        env: scans.options.env.clone(),
        source_kinds,
    };
    let scan = scans.scan(request, params.refresh).await?;
    let show_paths = authorize(&ctx.principal, Permission::HostAdmin).is_ok();
    Ok(to_wire(&scan, show_paths))
}

fn core_kind(kind: &wire::SkillSourceKind) -> Result<core::SkillSourceKind, ErrorBody> {
    Ok(match kind {
        wire::SkillSourceKind::Home => core::SkillSourceKind::Home,
        wire::SkillSourceKind::Repo => core::SkillSourceKind::Repo,
        wire::SkillSourceKind::Bundled => core::SkillSourceKind::Bundled,
        wire::SkillSourceKind::Plugin => core::SkillSourceKind::Plugin,
        wire::SkillSourceKind::Unknown => {
            return Err(ErrorBody::new(
                codes::REQUEST_INVALID_PARAMS,
                "`source_kinds` names a kind this build does not know",
            ));
        }
    })
}

fn wire_kind(kind: core::SkillSourceKind) -> wire::SkillSourceKind {
    match kind {
        core::SkillSourceKind::Home => wire::SkillSourceKind::Home,
        core::SkillSourceKind::Repo => wire::SkillSourceKind::Repo,
        core::SkillSourceKind::Bundled => wire::SkillSourceKind::Bundled,
        core::SkillSourceKind::Plugin => wire::SkillSourceKind::Plugin,
    }
}

fn wire_provider(provider: &core::SkillProvider) -> wire::SkillProvider {
    match provider {
        core::SkillProvider::Codex => wire::SkillProvider::Codex,
        core::SkillProvider::Claude => wire::SkillProvider::Claude,
        core::SkillProvider::AgentSkills => wire::SkillProvider::AgentSkills,
    }
}

fn wire_reason(reason: core::SkipReason) -> wire::SkipReason {
    match reason {
        core::SkipReason::Missing => wire::SkipReason::Missing,
        core::SkipReason::Unreadable => wire::SkipReason::Unreadable,
        core::SkipReason::Truncated => wire::SkipReason::Truncated,
    }
}

/// The scan as the protocol carries it. Host paths are left out unless `show_paths` is set.
fn to_wire(scan: &core::SkillDiscovery, show_paths: bool) -> wire::SkillDiscovery {
    let path = |path: &Path| show_paths.then(|| path.to_string_lossy().into_owned());
    wire::SkillDiscovery {
        skills: scan
            .skills
            .iter()
            .map(|skill| wire::DiscoveredSkill {
                id: skill.id.clone(),
                name: skill.name.clone(),
                description: skill.description.clone(),
                providers: skill.providers.iter().map(wire_provider).collect(),
                source_kind: wire_kind(skill.source_kind),
                source_label: skill.source_label.clone(),
                root_path: path(&skill.root_path),
                root_paths: match show_paths {
                    true => skill
                        .root_paths
                        .iter()
                        .map(|root| root.to_string_lossy().into_owned())
                        .collect(),
                    false => Vec::new(),
                },
                directory_path: path(&skill.directory_path),
                skill_file_path: path(&skill.skill_file_path),
                installed: skill.installed,
                updated_at: skill.updated_at_ms,
                disable_slash_command: skill.disable_slash_command,
            })
            .collect(),
        sources: scan
            .sources
            .iter()
            .map(|source| wire::SkillSource {
                id: source.id.clone(),
                label: source.label.clone(),
                path: path(&source.path),
                source_kind: wire_kind(source.source_kind),
                providers: source.providers.iter().map(wire_provider).collect(),
                owner: source.owner.clone(),
                exists: source.exists,
                skipped_reason: source.skipped_reason.map(wire_reason),
            })
            .collect(),
        scanned_at: scan.scanned_at_ms,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;

    use super::*;

    #[tokio::test]
    async fn a_refresh_that_arrives_during_a_scan_joins_that_scan() {
        let (entered_tx, entered_rx) = mpsc::channel::<()>();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let (entered_tx, release_rx) = (Mutex::new(entered_tx), Mutex::new(release_rx));
        let runs = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&runs);
        let scanner: Scanner = Arc::new(move |_request: &DiscoveryRequest| {
            counted.fetch_add(1, Ordering::SeqCst);
            let _ = entered_tx
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .send(());
            let _ = release_rx
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .recv();
            core::SkillDiscovery {
                skills: Vec::new(),
                sources: Vec::new(),
                scanned_at_ms: 1,
            }
        });
        let scans = Arc::new(SkillScans::with_scanner(
            DiscoveryOptions::default(),
            scanner,
        ));
        let first = tokio::spawn({
            let scans = Arc::clone(&scans);
            async move { scans.scan(DiscoveryRequest::default(), false).await }
        });
        tokio::task::spawn_blocking(move || entered_rx.recv())
            .await
            .expect("the first scan runs")
            .expect("it entered the scanner");
        let second = tokio::spawn({
            let scans = Arc::clone(&scans);
            async move { scans.scan(DiscoveryRequest::default(), true).await }
        });
        tokio::time::sleep(Duration::from_millis(200)).await;
        let runs_seen = runs.load(Ordering::SeqCst);
        // Release every scan that may be waiting, so none stays blocked whatever the outcome.
        let _ = release_tx.send(());
        let _ = release_tx.send(());
        let first = first.await.expect("the task").expect("the scan");
        let second = second.await.expect("the task").expect("the scan");

        assert_eq!(
            runs_seen, 1,
            "a refresh during a scan must join it, not start another"
        );
        assert_eq!(first, second);
    }
}
