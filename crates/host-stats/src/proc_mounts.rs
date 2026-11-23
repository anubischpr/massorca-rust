//! `/proc/self/mountinfo`: whether the `proc` filesystem mounted on `/proc` hides some processes with `hidepid`, so that its
//! listing is not every process (decision 0081). Parsing is pure, so it is tested on every platform.

/// Whether the `proc` mount on `/proc` hides some processes: a `hidepid` option other than zero, among its superblock
/// options.
pub(crate) fn hides_processes(mountinfo: &str) -> bool {
    mountinfo.lines().any(proc_mount_hides)
}

/// Whether one mountinfo line mounts `proc` on `/proc` with `hidepid` other than zero. A line is `id parent major:minor
/// root mountpoint options [optional fields] - fstype source superoptions`: the mount point is the fifth field, and the
/// superblock options, where `hidepid` is, come after the separator.
fn proc_mount_hides(line: &str) -> bool {
    let Some((before, after)) = line.split_once(" - ") else {
        return false;
    };
    if before.split_ascii_whitespace().nth(4) != Some("/proc") {
        return false;
    }
    let mut fields = after.split_ascii_whitespace();
    if fields.next() != Some("proc") {
        return false;
    }
    let _source = fields.next();
    fields.next().is_some_and(|options| {
        options
            .split(',')
            .filter_map(|option| option.strip_prefix("hidepid="))
            .any(|value| value != "0")
    })
}

#[cfg(test)]
mod tests {
    use super::hides_processes;

    /// A usual host's mounts, with `/proc` mounted without `hidepid`.
    const USUAL: &str = "\
20 1 8:1 / / rw,relatime shared:1 - ext4 /dev/sda1 rw
21 20 0:20 / /proc rw,nosuid,nodev,noexec,relatime shared:12 - proc proc rw
22 20 0:21 / /sys rw,nosuid,nodev,noexec,relatime shared:13 - sysfs sysfs rw
";

    #[test]
    fn a_proc_mount_without_hidepid_hides_nothing() {
        assert!(!hides_processes(USUAL));
    }

    #[test]
    fn hidepid_on_another_mount_is_not_the_proc_mounts() {
        let mounts = format!("{USUAL}30 20 0:30 / /run/x rw - tmpfs tmpfs rw,hidepid=2\n");
        assert!(!hides_processes(&mounts));
    }

    #[test]
    fn hidepid_other_than_zero_on_the_proc_mount_hides_processes() {
        for option in [
            "hidepid=1",
            "hidepid=2",
            "hidepid=invisible",
            "hidepid=noaccess",
        ] {
            let mounts = format!(
                "21 20 0:20 / /proc rw,nosuid,nodev,noexec,relatime shared:12 - proc proc rw,{option}\n"
            );
            assert!(hides_processes(&mounts), "{option}");
        }
        let zero = "21 20 0:20 / /proc rw,relatime shared:12 - proc proc rw,hidepid=0\n";
        assert!(
            !hides_processes(zero),
            "hidepid=0 is the default: nothing is hidden"
        );
    }

    #[test]
    fn hidepid_is_read_from_the_superblock_options_after_the_separator() {
        // In the per-mount options before the separator, `hidepid` is not a superblock option and is not read.
        let per_mount = "21 20 0:20 / /proc rw,hidepid=2 shared:12 - proc proc rw\n";
        assert!(!hides_processes(per_mount));
    }
}
