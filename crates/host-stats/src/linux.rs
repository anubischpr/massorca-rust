//! Linux: `/proc/<pid>/status` for the resident set, `/proc/<pid>/stat` for the table and for each process's identity.
//! No `unsafe`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

use crate::proc_mounts;
use crate::proc_net::{self, Family};
use crate::{Listening, ListeningSocket, ProcessInfo, ProcessTable};

/// The LISTEN sockets of the processes `pids`: the tables of `/proc/net/tcp` and `/proc/net/tcp6` name each socket's
/// inode, and each process's descriptors name the inodes it holds. Only the processes asked about have their
/// descriptors read. `None` when `/proc/net/tcp` cannot be read.
pub(crate) fn listening_sockets(table: &[ProcessInfo], pids: &BTreeSet<u32>) -> Option<Listening> {
    let mut listeners = proc_net::listeners(&read_table("/proc/net/tcp", false)?, Family::V4);
    listeners.extend(proc_net::listeners(
        &read_table("/proc/net/tcp6", true)?,
        Family::V6,
    ));
    let (owners, unreadable) = socket_owners(table, pids);
    let sockets = listeners
        .into_iter()
        .filter_map(|listener| {
            let pid = *owners.get(&listener.inode)?;
            Some(ListeningSocket {
                pid,
                address: listener.address,
                port: listener.port,
            })
        })
        .collect();
    Some(Listening {
        sockets,
        unreadable,
    })
}

/// A `/proc/net/tcp*` table's text. A missing IPv6 table is no sockets, since a kernel built without IPv6 has none;
/// any other failure leaves the table unreadable.
fn read_table(path: &str, absent_is_empty: bool) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(error) if absent_is_empty && error.kind() == io::ErrorKind::NotFound => {
            Some(String::new())
        }
        Err(_) => None,
    }
}

/// The pid that holds each socket inode among `pids` (the lowest pid when several hold it: the one that created it
/// after a fork is the lowest), and the asked pids whose descriptors could not be read.
fn socket_owners(
    table: &[ProcessInfo],
    pids: &BTreeSet<u32>,
) -> (BTreeMap<u64, u32>, BTreeSet<u32>) {
    let listed: BTreeMap<u32, &ProcessInfo> = table.iter().map(|info| (info.pid, info)).collect();
    let mut owners = BTreeMap::new();
    let mut unreadable = BTreeSet::new();
    for &pid in pids {
        match listed.get(&pid).and_then(|info| socket_inodes(info)) {
            Some(inodes) => {
                for inode in inodes {
                    owners.entry(inode).or_insert(pid);
                }
            }
            None => {
                unreadable.insert(pid);
            }
        }
    }
    (owners, unreadable)
}

/// The socket inodes `listed`'s process holds. `None` when the process is no longer the one listed, or its descriptors
/// cannot be read; the identity is checked before and after the read, so a reused pid is never read (decision 0081).
fn socket_inodes(listed: &ProcessInfo) -> Option<Vec<u64>> {
    if !still_listed(listed) {
        return None;
    }
    let entries = fs::read_dir(format!("/proc/{}/fd", listed.pid)).ok()?;
    let inodes = entries
        .filter_map(Result::ok)
        // A descriptor that closed since the listing is no longer a socket to read.
        .filter_map(|entry| fs::read_link(entry.path()).ok())
        .filter_map(|target| socket_inode(&target))
        .collect();
    still_listed(listed).then_some(inodes)
}

/// The inode of a descriptor's link target, `socket:[inode]`.
fn socket_inode(target: &Path) -> Option<u64> {
    target
        .to_str()?
        .strip_prefix("socket:[")?
        .strip_suffix(']')?
        .parse()
        .ok()
}

/// The resident set of `pid` in bytes, with no identity check. Only for a process whose identity is the caller's own.
pub(crate) fn resident_bytes(pid: u32) -> Option<u64> {
    let status = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    parse_vm_rss(&status)
}

/// The resident set of a process as `listed` recorded it. Its start time and session must match the listing before
/// and after the read, so a pid reused since the listing is never read (decision 0081).
pub(crate) fn resident_checked(listed: &ProcessInfo) -> Option<u64> {
    if !still_listed(listed) {
        return None;
    }
    let status = fs::read_to_string(format!("/proc/{}/status", listed.pid)).ok()?;
    let bytes = parse_vm_rss(&status)?;
    still_listed(listed).then_some(bytes)
}

/// Whether the process `listed` names is live and is the same process: same start time and same session.
fn still_listed(listed: &ProcessInfo) -> bool {
    current(listed.pid)
        .is_some_and(|now| now.start == listed.start && now.session == listed.session)
}

/// `/proc/<pid>/stat` now. `None` when the process has ended or cannot be read.
fn current(pid: u32) -> Option<ProcessInfo> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    parse_stat(pid, &stat)
}

/// `VmRSS` in bytes. Zombies and kernel threads have no such line.
fn parse_vm_rss(status: &str) -> Option<u64> {
    let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
    let mut fields = line["VmRSS:".len()..].split_ascii_whitespace();
    let value: u64 = fields.next()?.parse().ok()?;
    match fields.next()? {
        "kB" => value.checked_mul(1024),
        _ => None,
    }
}

pub(crate) fn process_table() -> ProcessTable {
    let Ok(entries) = fs::read_dir("/proc") else {
        return ProcessTable {
            processes: Vec::new(),
            complete: false,
        };
    };
    let processes = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        .filter_map(current)
        .collect();
    // `hidepid` keeps other users' processes out of `/proc`, so the listing is then not every process (decision 0081).
    // A mountinfo that cannot be read is taken as no hiding.
    let hidden = fs::read_to_string("/proc/self/mountinfo")
        .is_ok_and(|mounts| proc_mounts::hides_processes(&mounts));
    ProcessTable {
        processes,
        complete: !hidden,
    }
}

/// The name between the first `(` and the last `)`, so a name with spaces or parentheses cannot shift the fields
/// after it (`proc(5)`): the state, the parent, the process group, the session, then the start time. Ended processes
/// (`Z`, `X`) are left out.
fn parse_stat(pid: u32, stat: &str) -> Option<ProcessInfo> {
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    let name = stat.get(open + 1..close)?.to_owned();
    let mut fields = stat.get(close + 1..)?.split_ascii_whitespace();
    let state = fields.next()?;
    if matches!(state, "Z" | "X") {
        return None;
    }
    let parent: u32 = fields.next()?.parse().ok()?;
    let _group = fields.next()?;
    let session: u32 = fields.next()?.parse().ok()?;
    // Fields 7 to 21 come between the session and the start time, which is field 22.
    let start: u64 = fields.nth(15)?.parse().ok()?;
    Some(ProcessInfo {
        pid,
        parent: (parent != 0).then_some(parent),
        name,
        session: Some(session),
        start: Some(start),
    })
}

#[cfg(test)]
mod tests {
    use super::{current, parse_stat, parse_vm_rss, resident_checked};
    use crate::ProcessInfo;

    #[test]
    fn vm_rss_is_read_in_kibibytes() {
        let status = "Name:\tsh\nVmPeak:\t  2000 kB\nVmRSS:\t  1536 kB\nThreads:\t1\n";
        assert_eq!(parse_vm_rss(status), Some(1536 * 1024));
        assert_eq!(parse_vm_rss("Name:\tkthreadd\nThreads:\t1\n"), None);
    }

    #[test]
    fn stat_names_parents_sessions_and_start_times_survive_odd_names_and_zombies_are_left_out() {
        assert_eq!(
            parse_stat(
                43,
                "43 (my (odd) name) R 42 43 99 0 -1 4194560 1 0 0 0 1 1 0 0 20 0 1 0 555"
            ),
            Some(ProcessInfo {
                pid: 43,
                parent: Some(42),
                name: "my (odd) name".to_owned(),
                session: Some(99),
                start: Some(555),
            })
        );
        assert_eq!(
            parse_stat(
                1,
                "1 (init) S 0 1 1 0 -1 4194560 1 0 0 0 1 1 0 0 20 0 1 0 1"
            ),
            Some(ProcessInfo {
                pid: 1,
                parent: None,
                name: "init".to_owned(),
                session: Some(1),
                start: Some(1),
            })
        );
        assert_eq!(
            parse_stat(
                44,
                "44 (gone) Z 42 44 99 0 -1 4194560 1 0 0 0 1 1 0 0 20 0 1 0 9"
            ),
            None
        );
        assert_eq!(parse_stat(45, "45 (truncated"), None);
    }

    #[test]
    fn a_listing_with_another_start_time_is_not_read() {
        let own = std::process::id();
        let listed = current(own).expect("this process is listed");
        assert!(resident_checked(&listed).is_some_and(|bytes| bytes > 0));
        let reused = ProcessInfo {
            start: listed.start.map(|start| start + 1),
            ..listed
        };
        assert_eq!(resident_checked(&reused), None);
    }
}
