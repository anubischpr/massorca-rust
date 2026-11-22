//! Windows endpoint checks: the owner-only descriptor of the service pipe, and the user of the process at its far end.
//! Every unsafe call states, in its `SAFETY` comment, the invariant it relies on.
#![allow(unsafe_code)]

use std::io;
use std::path::Path;
use std::ptr;

use interprocess::os::windows::security_descriptor::SecurityDescriptor;
use widestring::U16CString;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    DACL_SECURITY_INFORMATION, GetTokenInformation, PROTECTED_DACL_SECURITY_INFORMATION,
    PSECURITY_DESCRIPTOR, PSID, SetFileSecurityW, TOKEN_INFORMATION_CLASS, TOKEN_QUERY, TOKEN_USER,
    TokenUser, UNPROTECTED_DACL_SECURITY_INFORMATION,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};

use crate::windows_dir_access::wide_path;

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
/// Replaces the DACL of an existing file or directory with the owner-only one, protected from inheritance, so wider
/// ACLs on the parent directory never reach it.
pub fn restrict_to_owner(path: &Path) -> io::Result<()> {
    set_dacl_sddl(path, &owner_only_sddl()?)
}

/// Replaces the DACL of an existing file or directory with the one in `sddl`. A `D:P` DACL is stored protected; any
/// other is stored unprotected, so its inheritable entries apply to new children.
pub fn set_dacl_sddl(path: &Path, sddl: &str) -> io::Result<()> {
    let wide_sddl = U16CString::from_str(sddl)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let wide_path = wide_path(path)?;
    let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
    // SAFETY: `wide_sddl` is NUL-terminated and outlives the call; `descriptor` is a valid out-pointer and the size
    // out-pointer may be null.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide_sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let protection = if sddl.starts_with("D:P") {
        PROTECTED_DACL_SECURITY_INFORMATION
    } else {
        UNPROTECTED_DACL_SECURITY_INFORMATION
    };
    // SAFETY: `wide_path` is NUL-terminated and `descriptor` is the valid descriptor converted above.
    let set = unsafe {
        SetFileSecurityW(
            wide_path.as_ptr(),
            DACL_SECURITY_INFORMATION | protection,
            descriptor,
        )
    };
    let result = if set == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    };
    // SAFETY: the conversion allocated `descriptor` with LocalAlloc, so LocalFree releases it exactly once.
    unsafe { LocalFree(descriptor) };
    result
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

/// The SID string of the user this process runs as.
pub(crate) fn current_user_sid() -> io::Result<String> {
    // SAFETY: GetCurrentProcess returns a pseudo-handle that is valid for this process and needs no closing.
    process_token_user(unsafe { GetCurrentProcess() })
}

/// The SID string of the user a process runs as.
fn process_token_user(process: HANDLE) -> io::Result<String> {
    let token = process_token(process)?;
    token_user_sid(&token)
}

/// This process's access token, opened for queries.
pub(crate) fn current_process_token() -> io::Result<Handle> {
    // SAFETY: GetCurrentProcess returns a pseudo-handle that is valid for this process and needs no closing.
    process_token(unsafe { GetCurrentProcess() })
}

fn process_token(process: HANDLE) -> io::Result<Handle> {
    let mut token: HANDLE = ptr::null_mut();
    // SAFETY: `process` is a process handle or pseudo-handle for this call, and `token` is a valid out-pointer.
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(Handle(token))
}

/// One class of information about `token`, in a buffer aligned for the structure the class returns; the structure's
/// pointers point into the same buffer.
pub(crate) fn token_information(
    token: &Handle,
    class: TOKEN_INFORMATION_CLASS,
) -> io::Result<Vec<usize>> {
    let mut needed = 0_u32;
    // SAFETY: a null buffer of length 0 only asks for the size, which the call writes to `needed`.
    unsafe { GetTokenInformation(token.0, class, ptr::null_mut(), 0, &mut needed) };
    let words = (needed as usize).div_ceil(size_of::<usize>()).max(1);
    // A usize buffer keeps the structure aligned for its pointer fields.
    let mut buffer = vec![0_usize; words];
    // SAFETY: the buffer holds `needed` bytes and is aligned for the class's structure; the call fills it.
    if unsafe {
        GetTokenInformation(
            token.0,
            class,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(buffer)
}

fn token_user_sid(token: &Handle) -> io::Result<String> {
    let buffer = token_information(token, TokenUser)?;
    let user = buffer.as_ptr().cast::<TOKEN_USER>();
    // SAFETY: on success the buffer starts with a TOKEN_USER, and its SID points into the same buffer, which lives through this call.
    let sid = unsafe { (*user).User.Sid };
    sid_to_string(sid)
}

/// The string form (`S-1-...`) of a SID. `sid` must point at a valid SID for the duration of the call.
pub(crate) fn sid_to_string(sid: PSID) -> io::Result<String> {
    let mut text: *mut u16 = ptr::null_mut();
    // SAFETY: the caller passes a valid SID, and `text` is a valid out-pointer for the allocated string.
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let sid_text = wide_to_string(text);
    // SAFETY: ConvertSidToStringSidW allocates `text` with LocalAlloc, so LocalFree releases it exactly once.
    unsafe { LocalFree(text.cast()) };
    Ok(sid_text)
}

/// Reads a NUL-terminated wide string that ConvertSidToStringSidW returned.
pub(crate) fn wide_to_string(text: *const u16) -> String {
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
pub(crate) struct Handle(HANDLE);

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
        process_belongs_to_current_user, restrict_to_owner, set_dacl_sddl, wide_to_string,
    };
    use crate::windows_dir_access::{dacl_sddl, has_owner_only_dacl};

    #[test]
    fn restrict_to_owner_replaces_an_inherited_wide_dacl_with_a_protected_owner_only_one() {
        let dir = tempfile::tempdir().expect("tempdir");
        set_dacl_sddl(dir.path(), "D:(A;OICI;GA;;;WD)").expect("widen the directory");
        let file = dir.path().join("secret");
        std::fs::write(&file, b"x").expect("write");
        assert!(
            dacl_sddl(&file).expect("the file DACL").contains(";WD)"),
            "the file inherits the wide entry before the fix"
        );

        assert!(!has_owner_only_dacl(&file).expect("read the DACL"));

        restrict_to_owner(&file).expect("restrict");

        assert!(has_owner_only_dacl(&file).expect("read the DACL"));
    }

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
