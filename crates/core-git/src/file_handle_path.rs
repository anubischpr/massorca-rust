//! The path an open Windows file handle names now, as the operating system resolves it (decision 0103).
#![allow(unsafe_code)]

use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::io::AsRawHandle;
use std::path::PathBuf;

use windows_sys::Win32::Storage::FileSystem::GetFinalPathNameByHandleW;

/// The location of the file `file` holds, with the verbatim prefix (`\\?\`) Windows gives it. The name comes from the
/// handle, so the links on the way to the file are already followed when it is read.
pub(crate) fn final_path(file: &File) -> io::Result<PathBuf> {
    let handle = file.as_raw_handle();
    let mut buffer: Vec<u16> = vec![0; 512];
    loop {
        let capacity = u32::try_from(buffer.len()).unwrap_or(u32::MAX);
        // SAFETY: `handle` belongs to `file`, which is open for the whole call, and `buffer` has `capacity` units.
        let written =
            unsafe { GetFinalPathNameByHandleW(handle, buffer.as_mut_ptr(), capacity, 0) };
        if written == 0 {
            return Err(io::Error::last_os_error());
        }
        let needed = usize::try_from(written).unwrap_or(usize::MAX);
        if needed < buffer.len() {
            buffer.truncate(needed);
            return Ok(PathBuf::from(OsString::from_wide(&buffer)));
        }
        buffer.resize(needed + 1, 0);
    }
}
