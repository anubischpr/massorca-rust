//! Linux: `/proc/<pid>/status` for the resident set, `/proc/<pid>/stat` for the table and for each process's identity.
//! No `unsafe`.

use std::fs;

use crate::ProcessInfo;

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

pub(crate) fn process_table() -> Vec<ProcessInfo> {
    let Ok(entries) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        .filter_map(current)
        .collect()
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
