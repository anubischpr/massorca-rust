//! The effective user of this process, which the client compares with the peer of the service socket.
#![allow(unsafe_code)]

/// The effective user id of this process.
pub fn current_uid() -> u32 {
    // SAFETY: geteuid takes no arguments, cannot fail, and only reads this process's credentials.
    unsafe { libc::geteuid() }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt;

    use super::current_uid;

    #[test]
    fn current_uid_is_the_owner_of_a_file_this_process_created() {
        let file = tempfile::NamedTempFile::new().expect("a temporary file");
        let owner = file.as_file().metadata().expect("its metadata").uid();
        assert_eq!(owner, current_uid());
    }
}
