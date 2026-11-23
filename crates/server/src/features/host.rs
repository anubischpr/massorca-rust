//! `host.memory` and `host.ports` (capability `host/1`, decision 0081): the memory of the service and of each live
//! terminal's processes, and the listening sockets those processes hold, read on this host when asked. Nothing is
//! pushed; clients poll.

use std::collections::{BTreeSet, HashMap};
use std::future::Future;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use massorca_core_agents::sessions::Agents;
use massorca_core_store::now_ms;
use massorca_core_terminal::{TerminalManager, TerminalSample};
use massorca_host_stats::{self as stats, Listening, ListeningSocket, Metric, ProcessTable};
use massorca_protocol::methods::host::{
    self, HostMemory, HostPort, HostPorts, MemoryMetric, PortTerminal, ProcessMemory,
    TerminalMemory,
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
    let port_samples = Arc::new(SampleCache::default());
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
        registries.methods.register(
            Owner::Core,
            host::PORTS,
            capabilities::HOST,
            Permission::Read,
            move |ctx: RequestCtx, _params: NoParams| {
                let samples = Arc::clone(&port_samples);
                async move { ports(&samples, &ctx).await }
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

/// Samples the listening sockets on the blocking pool, as memory is sampled (decision 0081). A sample taken less than
/// `SAMPLE_TTL` ago is answered again, with the time it was taken.
async fn ports(samples: &SampleCache<HostPorts>, ctx: &RequestCtx) -> Result<HostPorts, ErrorBody> {
    let terminals = ctx.state.terminals.clone();
    let agents = ctx.state.agents.clone();
    samples
        .get_or_sample(Instant::now(), move || async move {
            match tokio::task::spawn_blocking(move || sample_ports(&terminals, &agents)).await {
                Ok(answer) => answer,
                Err(error) => {
                    tracing::error!(error = %error, "the host ports sample did not finish");
                    Err(ErrorBody::new(
                        codes::INTERNAL,
                        "the ports sample did not finish",
                    ))
                }
            }
        })
        .await
}

/// Reads the listening sockets of the live terminals' members: the membership first, then the sockets, then each
/// terminal's members again, so a pid reused meanwhile is not taken for a member (decision 0081).
fn sample_ports(terminals: &TerminalManager, agents: &Agents) -> Result<HostPorts, ErrorBody> {
    let table = stats::processes();
    let listed = terminals.member_pids(&table);
    let asked: Vec<u32> = listed
        .iter()
        .flat_map(|terminal| terminal.pids.iter().flatten().copied())
        .collect();
    let listening = stats::listening_tcp(&table.processes, &asked);
    let members: Vec<TerminalPorts> = listed
        .into_iter()
        .map(|terminal| {
            let confirmed = terminal
                .pids
                .as_ref()
                .map(|pids| terminals.still_members(&terminal.terminal_id, pids))
                .unwrap_or_default();
            TerminalPorts {
                terminal_id: terminal.terminal_id,
                workspace_id: terminal.workspace_id,
                listed: terminal.pids,
                confirmed,
            }
        })
        .collect();
    let names: HashMap<u32, String> = table
        .processes
        .iter()
        .map(|process| (process.pid, process.name.clone()))
        .collect();
    let sessions = sessions_by_terminal(agents);
    let (ports, statuses, unverifiable) = ports_answer(
        &members,
        listening.as_ref(),
        &names,
        &sessions,
        listing_complete(&table),
    );
    Ok(HostPorts {
        ports,
        terminals: statuses,
        unverifiable,
        sampled_at_ms: now_ms(),
    })
}

/// One live terminal's members for ports: `listed` as the membership read them (`None` when they cannot be listed), and
/// `confirmed`, the ones still in the terminal once the sockets were read.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TerminalPorts {
    terminal_id: String,
    workspace_id: String,
    listed: Option<Vec<u32>>,
    confirmed: Vec<u32>,
}

/// Where a client connects to a socket: `localhost` for a wildcard bind, otherwise the address itself.
fn connect_host(address: IpAddr) -> String {
    if address.is_unspecified() {
        "localhost".to_owned()
    } else {
        address.to_string()
    }
}

/// The wire answer for the ports: the sockets that each terminal's confirmed members hold and that could be read, with
/// each terminal's status, and whether the socket table itself is unreadable. Without a table nothing is known, so every
/// terminal is unverifiable and no port is listed (decision 0081). `names` and `sessions` label the entries.
fn ports_answer(
    terminals: &[TerminalPorts],
    listening: Option<&Listening>,
    names: &HashMap<u32, String>,
    sessions: &HashMap<String, String>,
    table_complete: bool,
) -> (Vec<HostPort>, Vec<PortTerminal>, bool) {
    let Some(listening) = listening else {
        let statuses = terminals
            .iter()
            .map(|terminal| PortTerminal {
                terminal_id: terminal.terminal_id.clone(),
                workspace_id: terminal.workspace_id.clone(),
                unverifiable: true,
                partial: false,
            })
            .collect();
        return (Vec::new(), statuses, true);
    };
    let mut ports = Vec::new();
    let mut statuses = Vec::new();
    for terminal in terminals {
        let readable: BTreeSet<u32> = terminal
            .confirmed
            .iter()
            .copied()
            .filter(|pid| !listening.unreadable.contains(pid))
            .collect();
        let listed = terminal.listed.as_ref().map_or(0, Vec::len);
        let unverifiable = readable.is_empty();
        let partial = !unverifiable && (!table_complete || readable.len() < listed);
        statuses.push(PortTerminal {
            terminal_id: terminal.terminal_id.clone(),
            workspace_id: terminal.workspace_id.clone(),
            unverifiable,
            partial,
        });
        if unverifiable {
            continue;
        }
        let mut own: Vec<&ListeningSocket> = listening
            .sockets
            .iter()
            .filter(|socket| readable.contains(&socket.pid))
            .collect();
        own.sort_by_key(|socket| (socket.port, socket.address, socket.pid));
        ports.extend(own.into_iter().map(|socket| HostPort {
            port: socket.port,
            address: socket.address,
            connect_host: connect_host(socket.address),
            pid: socket.pid,
            process_name: names.get(&socket.pid).cloned().unwrap_or_default(),
            terminal_id: terminal.terminal_id.clone(),
            workspace_id: terminal.workspace_id.clone(),
            session_id: sessions.get(&terminal.terminal_id).cloned(),
        }));
    }
    (ports, statuses, false)
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

/// A terminal of a process table that could not list every process may be missing some of its processes, so it is
/// `partial` unless it is `unverifiable` (decision 0081).
fn with_listing(mut entry: TerminalMemory, complete: bool) -> TerminalMemory {
    if !complete && !entry.unverifiable {
        entry.partial = true;
    }
    entry
}

/// Whether every process of the terminals was listed. On Unix a terminal's processes come from the process table, so a
/// table that ended early can miss some, and its terminals are partial. On Windows they come from each terminal's job,
/// so the snapshot does not decide (decision 0081).
#[cfg(unix)]
fn listing_complete(table: &ProcessTable) -> bool {
    table.complete
}

#[cfg(not(unix))]
fn listing_complete(table: &ProcessTable) -> bool {
    let _ = table;
    true
}

fn sample(terminals: &TerminalManager, agents: &Agents) -> Result<HostMemory, ErrorBody> {
    let own_pid = std::process::id();
    // The service always reads its own process; failing to is a platform without a reader, not an empty figure.
    let own_bytes = stats::memory_of(own_pid).ok_or_else(|| {
        ErrorBody::new(codes::INTERNAL, "this host's process memory cannot be read")
    })?;
    let table = stats::processes();
    let sessions = sessions_by_terminal(agents);
    let mut unreadable = 0u32;
    let mut total_bytes = own_bytes;
    let mut entries = Vec::new();
    for terminal in terminals.memory(&table) {
        let (entry, skipped) = entry_of(terminal, &sessions);
        let entry = with_listing(entry, listing_complete(&table));
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
    use massorca_host_stats::{METRIC, MemorySample, ProcessMemory, ProcessTable};

    use massorca_protocol::methods::host::TerminalMemory;

    use super::{entry_of, listing_complete, with_listing};

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
    fn a_terminal_of_an_incomplete_listing_is_partial_unless_unverifiable() {
        let read = TerminalMemory {
            terminal_id: "t-1".to_owned(),
            workspace_id: "w-1".to_owned(),
            session_id: None,
            processes: 1,
            bytes: 5,
            unverifiable: false,
            partial: false,
        };
        assert!(with_listing(read.clone(), false).partial);
        assert!(!with_listing(read, true).partial);
        let gone = TerminalMemory {
            terminal_id: "t-2".to_owned(),
            workspace_id: "w-1".to_owned(),
            session_id: None,
            processes: 0,
            bytes: 0,
            unverifiable: true,
            partial: false,
        };
        assert!(
            !with_listing(gone, false).partial,
            "an unverifiable terminal is never partial"
        );
    }

    #[test]
    fn only_a_unix_table_that_ended_early_makes_terminals_partial() {
        let ended_early = ProcessTable {
            processes: Vec::new(),
            complete: false,
        };
        // On Windows a terminal's processes come from its job, so the snapshot's completeness does not decide.
        #[cfg(windows)]
        {
            assert!(
                listing_complete(&ended_early),
                "a Windows snapshot does not decide a terminal's members"
            );
        }
        #[cfg(unix)]
        {
            assert!(
                !listing_complete(&ended_early),
                "a Unix table that ended early is incomplete"
            );
        }
        let whole = ProcessTable {
            processes: Vec::new(),
            complete: true,
        };
        assert!(listing_complete(&whole));
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
mod ports_tests {
    use std::collections::{BTreeSet, HashMap};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use massorca_host_stats::{Listening, ListeningSocket};
    use massorca_protocol::methods::host::{HostPort, PortTerminal};

    use super::{TerminalPorts, ports_answer};

    fn member(id: &str, listed: Option<Vec<u32>>, confirmed: Vec<u32>) -> TerminalPorts {
        TerminalPorts {
            terminal_id: id.to_owned(),
            workspace_id: "w-1".to_owned(),
            listed,
            confirmed,
        }
    }

    fn socket(pid: u32, address: IpAddr, port: u16) -> ListeningSocket {
        ListeningSocket { pid, address, port }
    }

    fn reading(sockets: Vec<ListeningSocket>, unreadable: &[u32]) -> Listening {
        Listening {
            sockets,
            unreadable: unreadable.iter().copied().collect::<BTreeSet<u32>>(),
        }
    }

    fn names() -> HashMap<u32, String> {
        HashMap::from([
            (10, "node".to_owned()),
            (30, "server".to_owned()),
            (40, "worker".to_owned()),
        ])
    }

    fn sessions() -> HashMap<String, String> {
        HashMap::from([("t-1".to_owned(), "s-1".to_owned())])
    }

    fn status(answer: &[PortTerminal], id: &str) -> PortTerminal {
        answer
            .iter()
            .find(|terminal| terminal.terminal_id == id)
            .cloned()
            .unwrap_or_else(|| panic!("{id} in {answer:?}"))
    }

    #[test]
    fn a_terminals_sockets_are_listed_with_their_address_port_process_and_session() {
        let terminals = [member("t-1", Some(vec![10, 11]), vec![10, 11])];
        let listening = reading(
            vec![
                socket(10, IpAddr::V6(Ipv6Addr::LOCALHOST), 3000),
                socket(10, IpAddr::V4(Ipv4Addr::UNSPECIFIED), 3000),
                socket(99, IpAddr::V4(Ipv4Addr::LOCALHOST), 5000),
            ],
            &[],
        );
        let (ports, statuses, unverifiable) =
            ports_answer(&terminals, Some(&listening), &names(), &sessions(), true);
        assert!(!unverifiable);
        assert_eq!(ports.len(), 2, "{ports:?}");
        assert_eq!(
            ports[0],
            HostPort {
                port: 3000,
                address: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                connect_host: "localhost".to_owned(),
                pid: 10,
                process_name: "node".to_owned(),
                terminal_id: "t-1".to_owned(),
                workspace_id: "w-1".to_owned(),
                session_id: Some("s-1".to_owned()),
            }
        );
        assert_eq!(ports[1].connect_host, "::1");
        assert!(
            ports.iter().all(|port| port.pid != 99),
            "a process outside every terminal is not listed: {ports:?}"
        );
        let t1 = status(&statuses, "t-1");
        assert!(!t1.unverifiable && !t1.partial, "{t1:?}");
    }

    #[test]
    fn an_unreadable_socket_table_leaves_every_terminal_unverifiable() {
        let terminals = [member("t-1", Some(vec![10]), vec![10])];
        let (ports, statuses, unverifiable) =
            ports_answer(&terminals, None, &names(), &sessions(), true);
        assert!(unverifiable);
        assert!(ports.is_empty(), "{ports:?}");
        assert!(status(&statuses, "t-1").unverifiable);
    }

    #[test]
    fn a_terminal_whose_members_cannot_be_listed_is_unverifiable_not_without_ports() {
        let terminals = [member("t-2", None, Vec::new())];
        let listening = reading(vec![socket(10, IpAddr::V4(Ipv4Addr::LOCALHOST), 3000)], &[]);
        let (ports, statuses, unverifiable) =
            ports_answer(&terminals, Some(&listening), &names(), &sessions(), true);
        assert!(!unverifiable);
        assert!(ports.is_empty(), "{ports:?}");
        let t2 = status(&statuses, "t-2");
        assert!(t2.unverifiable && !t2.partial, "{t2:?}");
    }

    #[test]
    fn a_terminal_none_of_whose_sockets_can_be_read_is_unverifiable() {
        let terminals = [member("t-3", Some(vec![20]), vec![20])];
        let listening = reading(
            vec![socket(20, IpAddr::V4(Ipv4Addr::LOCALHOST), 4000)],
            &[20],
        );
        let (ports, statuses, _) =
            ports_answer(&terminals, Some(&listening), &names(), &sessions(), true);
        assert!(ports.is_empty(), "{ports:?}");
        let t3 = status(&statuses, "t-3");
        assert!(t3.unverifiable && !t3.partial, "{t3:?}");
    }

    #[test]
    fn some_unreadable_members_make_a_terminal_partial_and_its_readable_ports_are_listed() {
        let terminals = [member("t-4", Some(vec![30, 31]), vec![30, 31])];
        let listening = reading(
            vec![
                socket(30, IpAddr::V4(Ipv4Addr::LOCALHOST), 8080),
                socket(31, IpAddr::V4(Ipv4Addr::LOCALHOST), 9090),
            ],
            &[31],
        );
        let (ports, statuses, _) =
            ports_answer(&terminals, Some(&listening), &names(), &sessions(), true);
        assert_eq!(
            ports.iter().map(|port| port.port).collect::<Vec<_>>(),
            [8080]
        );
        let t4 = status(&statuses, "t-4");
        assert!(t4.partial && !t4.unverifiable, "{t4:?}");
    }

    #[test]
    fn a_member_that_left_the_terminal_makes_it_partial() {
        let terminals = [member("t-5", Some(vec![40, 41]), vec![40])];
        let listening = reading(
            vec![
                socket(40, IpAddr::V4(Ipv4Addr::LOCALHOST), 9000),
                socket(41, IpAddr::V4(Ipv4Addr::LOCALHOST), 9001),
            ],
            &[],
        );
        let (ports, statuses, _) =
            ports_answer(&terminals, Some(&listening), &names(), &sessions(), true);
        assert_eq!(
            ports.iter().map(|port| port.port).collect::<Vec<_>>(),
            [9000]
        );
        assert!(status(&statuses, "t-5").partial);
    }

    #[test]
    fn an_incomplete_process_table_makes_a_readable_terminal_partial() {
        let terminals = [member("t-1", Some(vec![10]), vec![10])];
        let listening = reading(vec![socket(10, IpAddr::V4(Ipv4Addr::LOCALHOST), 3000)], &[]);
        let (ports, statuses, _) =
            ports_answer(&terminals, Some(&listening), &names(), &sessions(), false);
        assert_eq!(ports.len(), 1, "{ports:?}");
        assert!(status(&statuses, "t-1").partial);
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
