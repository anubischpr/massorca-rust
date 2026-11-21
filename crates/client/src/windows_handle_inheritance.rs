//! Clears the inheritance of this process's standard handles, so the detached service does not keep the caller's pipes open.
//!
//! `CreateProcess` is called with `bInheritHandles = TRUE`, so every inheritable handle leaks into the child. Rust's
//! `Stdio::inherit` duplicates the handles it passes on, so later children are unaffected by this change.
#![allow(unsafe_code)]

use std::io;

use windows_sys::Win32::Foundation::{
    HANDLE, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE, SetHandleInformation,
};
use windows_sys::Win32::System::Console::{
    GetStdHandle, STD_ERROR_HANDLE, STD_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};

const STANDARD_HANDLES: [STD_HANDLE; 3] = [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE];

/// Clears `HANDLE_FLAG_INHERIT` on each valid standard handle of this process; missing ones are skipped.
pub(crate) fn clear_standard_handle_inheritance() -> io::Result<()> {
    for which in STANDARD_HANDLES {
        let Some(handle) = standard_handle(which) else {
            continue;
        };
        // SAFETY: `handle` is a valid handle GetStdHandle returned for this process; the mask only selects HANDLE_FLAG_INHERIT and the flags clear it.
        if unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0) } == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

/// The standard handle `which`, or `None` when the process has none or it is invalid.
fn standard_handle(which: STD_HANDLE) -> Option<HANDLE> {
    // SAFETY: GetStdHandle only reads this process's standard-handle table for the selector it is given.
    let handle = unsafe { GetStdHandle(which) };
    (!handle.is_null() && handle != INVALID_HANDLE_VALUE).then_some(handle)
}

#[cfg(test)]
mod tests {
    use windows_sys::Win32::Foundation::GetHandleInformation;

    use super::*;

    #[test]
    fn standard_handles_are_not_inherited_once_cleared() {
        clear_standard_handle_inheritance().expect("the standard handles are updated");
        for which in STANDARD_HANDLES {
            let Some(handle) = standard_handle(which) else {
                continue;
            };
            let mut flags = 0_u32;
            // SAFETY: `flags` is a valid, writable u32 for the whole call, and `handle` came from GetStdHandle.
            let read = unsafe { GetHandleInformation(handle, &mut flags) };
            assert_ne!(read, 0, "{}", io::Error::last_os_error());
            assert_eq!(
                flags & HANDLE_FLAG_INHERIT,
                0,
                "the handle is still inheritable"
            );
        }
    }
}
