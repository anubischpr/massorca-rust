//! Windows endpoint checks: the owner-only descriptor of the service pipe, and the user of the process at its far end.
//! Every unsafe call states, in its `SAFETY` comment, the invariant it relies on.
#![allow(unsafe_code)]

use std::io;
use std::ptr;

use interprocess::os::windows::security_descriptor::SecurityDescriptor;
use widestring::U16CString;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows_sys::Win32::Security::{GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};

/// The SDDL of a descriptor that grants full access to the current user and nobody else. `P` stops inheritance from the parent directory.
pub fn owner_only_sddl() -> io::Result<String> {
    Ok(format!("D:P(A;;GA;;;{})", current_user_sid()?))
}

/// The descriptor the service pipe is created with: full access for the current user, and nothing for anyone else.
pub fn owner_only_security_descriptor() -> io::Result<SecurityDescriptor> {
    let sddl = U16CString::from_str(owner_only_sddl()?)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    SecurityDescriptor::deserialize(&sddl)
}

/// Whether the process `pid` runs as the current user. A process whose token cannot be read is an error, never a match.
pub fn process_belongs_to_current_user(pid: u32) -> io::Result<bool> {
    // SAFETY: OpenProcess takes plain values and returns a handle or null; null is checked before use.
    let process = Handle(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) });
    if process.0.is_null() {
        return Err(io::Error::last_os_error());
    }
    Ok(process_token_user(process.0)? == current_user_sid()?)
}

fn current_user_sid() -> io::Result<String> {
    // SAFETY: GetCurrentProcess returns a pseudo-handle that is valid for this process and needs no closing.
    process_token_user(unsafe { GetCurrentProcess() })
}

/// The SID string of the user a process runs as.
fn process_token_user(process: HANDLE) -> io::Result<String> {
    let mut token: HANDLE = ptr::null_mut();
    // SAFETY: `process` is a process handle or pseudo-handle for this call, and `token` is a valid out-pointer.
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = Handle(token);
    token_user_sid(token.0)
}

fn token_user_sid(token: HANDLE) -> io::Result<String> {
    let mut needed = 0_u32;
    // SAFETY: a null buffer of length 0 only asks for the size, which the call writes to `needed`.
    unsafe { GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut needed) };
    let words = (needed as usize).div_ceil(size_of::<usize>()).max(1);
    // A usize buffer keeps the TOKEN_USER aligned for its pointer fields.
    let mut buffer = vec![0_usize; words];
    // SAFETY: the buffer holds `needed` bytes and is aligned for TOKEN_USER; the call fills it with the token user.
    if unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let user = buffer.as_ptr().cast::<TOKEN_USER>();
    // SAFETY: on success the buffer starts with a TOKEN_USER, and its SID points into the same buffer, which lives through this call.
    let sid = unsafe { (*user).User.Sid };
    let mut text: *mut u16 = ptr::null_mut();
    // SAFETY: `sid` is a valid SID from the buffer, and `text` is a valid out-pointer for the allocated string.
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let sid_text = wide_to_string(text);
    // SAFETY: ConvertSidToStringSidW allocates `text` with LocalAlloc, so LocalFree releases it exactly once.
    unsafe { LocalFree(text.cast()) };
    Ok(sid_text)
}

/// Reads a NUL-terminated wide string that ConvertSidToStringSidW returned.
fn wide_to_string(text: *const u16) -> String {
    let mut len = 0;
    // SAFETY: the string is NUL-terminated, so every index up to the terminator is readable.
    while unsafe { *text.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: `len` code units from `text` were just read and are valid for the slice's lifetime.
    let units = unsafe { std::slice::from_raw_parts(text, len) };
    String::from_utf16_lossy(units)
}

/// Closes a process or token handle on drop; a null handle is left alone.
struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the handle came from OpenProcess or OpenProcessToken and is closed only here, once.
            unsafe { CloseHandle(self.0) };
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::ptr;

    use interprocess::os::windows::security_descriptor::AsSecurityDescriptor;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows_sys::Win32::Security::{
        ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, ACL_SIZE_INFORMATION, AclSizeInformation, GetAce,
        GetAclInformation, GetSecurityDescriptorDacl,
    };
    use windows_sys::Win32::System::SystemServices::ACCESS_ALLOWED_ACE_TYPE;

    use super::{
        SecurityDescriptor, current_user_sid, owner_only_sddl, owner_only_security_descriptor,
        process_belongs_to_current_user, wide_to_string,
    };

    #[test]
    fn the_pipe_descriptor_grants_exactly_one_allowed_ace_to_the_current_user() {
        let descriptor = owner_only_security_descriptor().expect("the descriptor");
        let sid = current_user_sid().expect("the current user's SID");
        assert_eq!(allowed_sids(&descriptor), vec![sid]);
    }

    /// The SID strings of the allowed ACEs in the descriptor's DACL.
    fn allowed_sids(descriptor: &SecurityDescriptor) -> Vec<String> {
        let mut present = 0;
        let mut acl: *mut ACL = ptr::null_mut();
        let mut defaulted = 0;
        // SAFETY: the descriptor outlives the call, and the three out-pointers are valid for writes.
        let read = unsafe {
            GetSecurityDescriptorDacl(
                descriptor.borrow().as_sd().cast_mut(),
                &mut present,
                &mut acl,
                &mut defaulted,
            )
        };
        assert_ne!(read, 0, "{}", io::Error::last_os_error());
        assert_ne!(present, 0, "the descriptor has a DACL");

        let mut info = ACL_SIZE_INFORMATION::default();
        // SAFETY: `acl` came from the descriptor above, and `info` is the size the call is told it has.
        let sized = unsafe {
            GetAclInformation(
                acl,
                ptr::from_mut(&mut info).cast(),
                size_of::<ACL_SIZE_INFORMATION>() as u32,
                AclSizeInformation,
            )
        };
        assert_ne!(sized, 0, "{}", io::Error::last_os_error());

        (0..info.AceCount)
            .filter_map(|index| {
                let mut ace = ptr::null_mut();
                // SAFETY: `index` is below AceCount, so the ACE exists in the DACL and `ace` is a valid out-pointer.
                assert_ne!(unsafe { GetAce(acl, index, &mut ace) }, 0);
                // SAFETY: the ACE header is the first field of every ACE, so reading it is valid.
                let header = unsafe { &*ace.cast::<ACE_HEADER>() };
                (u32::from(header.AceType) == ACCESS_ALLOWED_ACE_TYPE).then(|| {
                    // SAFETY: an ACCESS_ALLOWED_ACE stores its SID at SidStart, which stays inside the ACE.
                    let allowed = unsafe { &*ace.cast::<ACCESS_ALLOWED_ACE>() };
                    sid_text(ptr::from_ref(&allowed.SidStart).cast_mut().cast())
                })
            })
            .collect()
    }

    fn sid_text(sid: *mut std::ffi::c_void) -> String {
        let mut text: *mut u16 = ptr::null_mut();
        // SAFETY: `sid` points at a valid SID inside an ACE of the descriptor, and `text` receives a LocalAlloc'd string.
        assert_ne!(unsafe { ConvertSidToStringSidW(sid, &mut text) }, 0);
        let value = wide_to_string(text);
        // SAFETY: the string was allocated by ConvertSidToStringSidW, so LocalFree releases it exactly once.
        unsafe { LocalFree(text.cast()) };
        value
    }

    #[test]
    fn owner_only_sddl_grants_full_access_to_the_current_user_alone() {
        let sid = current_user_sid().expect("the current user's SID");
        assert!(sid.starts_with("S-1-"), "{sid}");
        assert_eq!(
            owner_only_sddl().expect("the descriptor text"),
            format!("D:P(A;;GA;;;{sid})")
        );
    }

    #[test]
    fn this_process_belongs_to_the_current_user() {
        assert!(process_belongs_to_current_user(std::process::id()).expect("own token"));
    }

    #[test]
    fn a_system_process_is_not_read_as_the_current_user() {
        assert!(!matches!(process_belongs_to_current_user(4), Ok(true)));
    }
}
