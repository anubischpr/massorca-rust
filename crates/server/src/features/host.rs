//! `host.memory` (capability `host/1`, decision 0081): the memory of the service and of each live terminal's processes,
//! read on this host when asked. Nothing is pushed; clients poll.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use massorca_core_agents::sessions::Agents;
use massorca_core_store::now_ms;
use massorca_core_terminal::{TerminalManager, TerminalSample};
use massorca_host_stats::{self as stats, Metric};
use massorca_protocol::methods::host::{
    self, HostMemory, MemoryMetric, ProcessMemory, TerminalMemory,
};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::NoParams;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

/// How long one sample is shared: concurrent and repeated calls within it get the same figures (decision 0081).
const SAMPLE_TTL: Duration = Duration::from_secs(1);

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let memory_samples = Arc::new(SampleCache::default());
    Ok(vec![
        registries
            .capabilities
            .register(capabilities::HOST, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            host::MEMORY,
            capabilities::HOST,
            Permission::Read,
            move |ctx: RequestCtx, _params: NoParams| {
                let samples = Arc::clone(&memory_samples);
                async move { memory(&samples, &ctx).await }
            },
        )?,
    ])
}

/// Samples on the blocking pool: listing processes and reading each one are system calls, never a spawned program.
/// A sample taken less than `SAMPLE_TTL` ago is answered again, with the time it was taken.
async fn memory(
    samples: &SampleCache<HostMemory>,
    ctx: &RequestCtx,
) -> Result<HostMemory, ErrorBody> {
    let terminals = ctx.state.terminals.clone();
    let agents = ctx.state.agents.clone();
    samples
        .get_or_sample(Instant::now(), move || async move {
            match tokio::task::spawn_blocking(move || sample(&terminals, &agents)).await {
                Ok(answer) => answer,
                Err(error) => {
                    tracing::error!(error = %error, "the host memory sample did not finish");
                    Err(ErrorBody::new(
                        codes::INTERNAL,
                        "the memory sample did not finish",
                    ))
                }
            }
        })
        .await
}

/// The last sample of one host figure, shared for `SAMPLE_TTL` so that concurrent and repeated calls get one sample
/// instead of rescanning every process each time. The lock is held while a sample is taken, so a call that arrives
/// meanwhile waits for it and shares it. A failed sample is not kept: the next call tries again.
#[derive(Debug)]
pub(crate) struct SampleCache<T> {
    last: tokio::sync::Mutex<Option<Sampled<T>>>,
}

#[derive(Debug)]
struct Sampled<T> {
    at: Instant,
    value: T,
}

impl<T> Default for SampleCache<T> {
    fn default() -> Self {
        Self {
            last: tokio::sync::Mutex::new(None),
        }
    }
}

impl<T: Clone> SampleCache<T> {
    /// The sample taken less than `SAMPLE_TTL` before `now` when there is one; otherwise a new one from `take`, kept.
    pub(crate) async fn get_or_sample<E, Fut>(
        &self,
        now: Instant,
        take: impl FnOnce() -> Fut,
    ) -> Result<T, E>
    where
        Fut: Future<Output = Result<T, E>>,
    {
        let mut last = self.last.lock().await;
        if let Some(sampled) = last.as_ref()
            && now.saturating_duration_since(sampled.at) < SAMPLE_TTL
        {
            return Ok(sampled.value.clone());
        }
        let value = take().await?;
        *last = Some(Sampled {
            at: now,
            value: value.clone(),
        });
        Ok(value)
    }
}

/// One terminal's wire entry, and how many of its processes could not be read. A live terminal always runs at least
/// its shell, so a terminal whose processes cannot be listed, or of which none was read, is `unverifiable` with no
/// bytes: never reported as using zero.
fn entry_of(terminal: TerminalSample, sessions: &HashMap<String, String>) -> (TerminalMemory, u32) {
    let session_id = sessions.get(&terminal.terminal_id).cloned();
    let (processes, bytes, unreadable, unverifiable) = match &terminal.sample {
        Some(sample) if !sample.processes.is_empty() => (
            u32::try_from(sample.processes.len()).unwrap_or(u32::MAX),
            sample.total_bytes(),
            sample.unreadable,
            false,
        ),
        Some(sample) => (0, 0, sample.unreadable, true),
        None => (0, 0, 0, true),
    };
    let entry = TerminalMemory {
        terminal_id: terminal.terminal_id,
        workspace_id: terminal.workspace_id,
        session_id,
        processes,
        bytes,
        unverifiable,
        partial: !unverifiable && unreadable > 0,
    };
    (entry, unreadable)
}

fn sample(terminals: &TerminalManager, agents: &Agents) -> Result<HostMemory, ErrorBody> {
    let own_pid = std::process::id();
    // The service always reads its own process; failing to is a platform without a reader, not an empty figure.
    let own_bytes = stats::memory_of(own_pid).ok_or_else(|| {
        ErrorBody::new(codes::INTERNAL, "this host's process memory cannot be read")
    })?;
    let sessions = sessions_by_terminal(agents);
    let mut unreadable = 0u32;
    let mut total_bytes = own_bytes;
    let mut entries = Vec::new();
    for terminal in terminals.memory() {
        let (entry, skipped) = entry_of(terminal, &sessions);
        unreadable = unreadable.saturating_add(skipped);
        total_bytes = total_bytes.saturating_add(entry.bytes);
        entries.push(entry);
    }
    Ok(HostMemory {
        metric: metric_of(stats::METRIC),
        total_bytes,
        service: ProcessMemory {
            pid: own_pid,
            bytes: own_bytes,
        },
        terminals: entries,
        unreadable,
        sampled_at_ms: now_ms(),
    })
}

/// The agent session whose current run uses each terminal. A session's row names only its current run's terminal,
/// and only live terminals are sampled, so a match is that session's live run. The labels are optional: when the
/// sessions cannot be read the figures are still answered, without them.
fn sessions_by_terminal(agents: &Agents) -> HashMap<String, String> {
    match agents.list(None) {
        Ok(sessions) => sessions
            .into_iter()
            .filter_map(|session| Some((session.terminal_id?, session.id)))
            .collect(),
        Err(error) => {
            tracing::warn!(
                code = error.code,
                "host memory could not read the agent sessions"
            );
            HashMap::new()
        }
    }
}

fn metric_of(metric: Metric) -> MemoryMetric {
    match metric {
        Metric::WorkingSet => MemoryMetric::WorkingSet,
        Metric::Resident => MemoryMetric::Resident,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use massorca_core_terminal::TerminalSample;
    use massorca_host_stats::{METRIC, MemorySample, ProcessMemory};

    use super::entry_of;

    fn terminal(sample: Option<MemorySample>) -> TerminalSample {
        TerminalSample {
            terminal_id: "t-1".to_owned(),
            workspace_id: "w-1".to_owned(),
            sample,
        }
    }

    fn sample(processes: Vec<ProcessMemory>, unreadable: u32) -> MemorySample {
        MemorySample {
            metric: METRIC,
            processes,
            unreadable,
        }
    }

    #[test]
    fn a_terminal_whose_processes_cannot_be_listed_is_unverifiable_not_zero() {
        let (entry, unreadable) = entry_of(terminal(None), &HashMap::new());
        assert!(entry.unverifiable, "{entry:?}");
        assert_eq!((entry.processes, entry.bytes), (0, 0));
        assert_eq!(unreadable, 0);
    }

    #[test]
    fn a_terminal_with_no_process_read_is_unverifiable_and_its_pids_are_unreadable() {
        let (entry, unreadable) = entry_of(terminal(Some(sample(Vec::new(), 2))), &HashMap::new());
        assert!(entry.unverifiable, "{entry:?}");
        assert_eq!(unreadable, 2);
        let (entry, _) = entry_of(terminal(Some(sample(Vec::new(), 0))), &HashMap::new());
        assert!(
            entry.unverifiable,
            "an empty listing of a live terminal is not zero memory"
        );
    }

    #[test]
    fn a_read_terminal_carries_its_bytes_and_session() {
        let sessions = HashMap::from([("t-1".to_owned(), "s-1".to_owned())]);
        let read = sample(
            vec![
                ProcessMemory { pid: 1, bytes: 10 },
                ProcessMemory { pid: 2, bytes: 5 },
            ],
            1,
        );
        let (entry, unreadable) = entry_of(terminal(Some(read)), &sessions);
        assert!(!entry.unverifiable);
        assert_eq!((entry.processes, entry.bytes), (2, 15));
        assert_eq!(entry.session_id.as_deref(), Some("s-1"));
        assert_eq!(unreadable, 1);
    }

    #[test]
    fn a_terminal_with_some_processes_unread_is_partial_and_counts_what_it_read() {
        let read = sample(vec![ProcessMemory { pid: 1, bytes: 10 }], 2);
        let (entry, unreadable) = entry_of(terminal(Some(read)), &HashMap::new());
        assert!(entry.partial, "{entry:?}");
        assert!(!entry.unverifiable, "{entry:?}");
        assert_eq!((entry.processes, entry.bytes), (1, 10));
        assert_eq!(unreadable, 2);
        let (complete, _) = entry_of(
            terminal(Some(sample(vec![ProcessMemory { pid: 1, bytes: 10 }], 0))),
            &HashMap::new(),
        );
        assert!(!complete.partial, "{complete:?}");
        let (none_read, _) = entry_of(terminal(Some(sample(Vec::new(), 2))), &HashMap::new());
        assert!(
            none_read.unverifiable && !none_read.partial,
            "{none_read:?}"
        );
    }
}

#[cfg(test)]
mod sample_cache_tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    use super::{SAMPLE_TTL, SampleCache};

    /// A take that counts itself: the value it returns is the number of samples taken so far.
    async fn take_counted(taken: &AtomicUsize) -> Result<usize, ()> {
        Ok(taken.fetch_add(1, Ordering::SeqCst) + 1)
    }

    #[tokio::test]
    async fn a_sample_is_shared_for_the_ttl_and_taken_again_after_it() {
        let cache = SampleCache::default();
        let taken = AtomicUsize::new(0);
        let start = Instant::now();

        let first = cache.get_or_sample(start, || take_counted(&taken)).await;
        let shared = cache
            .get_or_sample(start + SAMPLE_TTL / 2, || take_counted(&taken))
            .await;
        assert_eq!(first, Ok(1));
        assert_eq!(shared, Ok(1), "a call within the ttl takes no new sample");
        assert_eq!(taken.load(Ordering::SeqCst), 1);

        let later = cache
            .get_or_sample(start + SAMPLE_TTL, || take_counted(&taken))
            .await;
        assert_eq!(later, Ok(2), "a sample older than the ttl is taken again");
    }

    #[tokio::test]
    async fn concurrent_calls_share_one_sample_and_a_failed_one_is_not_kept() {
        let cache = Arc::new(SampleCache::default());
        let taken = Arc::new(AtomicUsize::new(0));
        let calls: Vec<_> = (0..8)
            .map(|_| {
                let cache = Arc::clone(&cache);
                let taken = Arc::clone(&taken);
                tokio::spawn(async move {
                    cache
                        .get_or_sample(Instant::now(), || async {
                            tokio::time::sleep(Duration::from_millis(20)).await;
                            take_counted(&taken).await
                        })
                        .await
                })
            })
            .collect();
        for call in calls {
            assert_eq!(call.await.expect("the call finishes"), Ok(1));
        }
        assert_eq!(
            taken.load(Ordering::SeqCst),
            1,
            "one sample for eight calls"
        );

        let failed: Result<usize, &str> = cache
            .get_or_sample(Instant::now() + SAMPLE_TTL * 5, || async {
                Err("no sample")
            })
            .await;
        assert_eq!(failed, Err("no sample"));
        let retried = cache
            .get_or_sample(Instant::now() + SAMPLE_TTL * 5, || async {
                Ok::<_, &str>(9)
            })
            .await;
        assert_eq!(retried, Ok(9), "a failed sample is not kept");
    }
}
