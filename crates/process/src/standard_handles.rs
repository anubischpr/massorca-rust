//! Stops this process's standard handles being inherited, so a detached child keeps none of the caller's pipes open.
//!
//! `CreateProcess` runs with `bInheritHandles = TRUE`, so every inheritable handle of this process reaches the child, and
//! a caller reading the pipes to their end would wait for the child too. Only the three standard handles are covered.
#![allow(unsafe_code)]

use std::ffi::c_void;

// Declared here, not through windows-sys: this crate depends on it only for tests, and std already links kernel32.
unsafe extern "system" {
    fn GetStdHandle(std_handle: u32) -> *mut c_void;
    fn SetHandleInformation(object: *mut c_void, mask: u32, flags: u32) -> i32;
}

const HANDLE_FLAG_INHERIT: u32 = 0x0000_0001;
const INVALID_HANDLE_VALUE: isize = -1;
/// The selectors of the input, output and error handles: `STD_INPUT_HANDLE` to `STD_ERROR_HANDLE` are -10 to -12.
const STANDARD_HANDLES: [u32; 3] = [
    (-10_i32).cast_unsigned(),
    (-11_i32).cast_unsigned(),
    (-12_i32).cast_unsigned(),
];

/// Clears inheritance of each standard handle this process has; one that cannot be changed stays inheritable.
pub(crate) fn stop_inheriting() {
    for selector in STANDARD_HANDLES {
        // SAFETY: GetStdHandle only reads this process's standard-handle table for the selector it is given.
        let handle = unsafe { GetStdHandle(selector) };
        if handle.is_null() || handle as isize == INVALID_HANDLE_VALUE {
            continue;
        }
        // SAFETY: `handle` came from GetStdHandle, and the mask and flags only clear HANDLE_FLAG_INHERIT.
        unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0) };
    }
}

#[cfg(test)]
mod tests {
    use windows_sys::Win32::Foundation::{
        GetHandleInformation, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE, SetHandleInformation,
    };
    use windows_sys::Win32::System::Console::{GetStdHandle, STD_OUTPUT_HANDLE};

    use super::stop_inheriting;

    #[test]
    fn the_standard_output_handle_is_not_inherited_once_stopped() {
        // SAFETY: GetStdHandle only reads this process's standard-handle table for the selector given.
        let handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return;
        }
        // SAFETY: `handle` is this process's standard output handle; the mask and flags only touch HANDLE_FLAG_INHERIT.
        let set = unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) };
        assert_ne!(set, 0, "{}", std::io::Error::last_os_error());
        stop_inheriting();
        let mut flags = 0_u32;
        // SAFETY: `flags` is a valid, writable u32 for the whole call, and `handle` is this process's.
        let read = unsafe { GetHandleInformation(handle, &mut flags) };
        assert_ne!(read, 0, "{}", std::io::Error::last_os_error());
        assert_eq!(
            flags & HANDLE_FLAG_INHERIT,
            0,
            "the handle is still inheritable"
        );
    }
}
