//! Linux: `/proc/<pid>/status` for the resident set, `/proc/<pid>/stat` for the table. No `unsafe`.

use std::fs;

use crate::ProcessInfo;

pub(crate) fn resident_bytes(pid: u32) -> Option<u64> {
    let status = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    parse_vm_rss(&status)
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
        .filter_map(|pid| {
            let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
            parse_stat(pid, &stat)
        })
        .collect()
}

/// The name between the first `(` and the last `)`, so a name with spaces or parentheses cannot shift the fields
/// after it (`proc(5)`): the state, then the parent. Ended processes (`Z`, `X`) are left out.
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
    Some(ProcessInfo {
        pid,
        parent: (parent != 0).then_some(parent),
        name,
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_stat, parse_vm_rss};
    use crate::ProcessInfo;

    #[test]
    fn vm_rss_is_read_in_kibibytes() {
        let status = "Name:\tsh\nVmPeak:\t  2000 kB\nVmRSS:\t  1536 kB\nThreads:\t1\n";
        assert_eq!(parse_vm_rss(status), Some(1536 * 1024));
        assert_eq!(parse_vm_rss("Name:\tkthreadd\nThreads:\t1\n"), None);
    }

    #[test]
    fn stat_names_and_parents_survive_odd_names_and_zombies_are_left_out() {
        assert_eq!(
            parse_stat(43, "43 (my (odd) name) R 42 43 99 0 -1"),
            Some(ProcessInfo {
                pid: 43,
                parent: Some(42),
                name: "my (odd) name".to_owned(),
            })
        );
        assert_eq!(
            parse_stat(1, "1 (init) S 0 1 1 0 -1"),
            Some(ProcessInfo {
                pid: 1,
                parent: None,
                name: "init".to_owned(),
            })
        );
        assert_eq!(parse_stat(44, "44 (gone) Z 42 44 99 0 -1"), None);
        assert_eq!(parse_stat(45, "45 (truncated"), None);
    }
}
