//! Owner-only directories on Windows: an explicit, protected DACL for directories that hold secrets, such as agent
//! credential homes (0056). The profile directory itself keeps its inherited access (0004).
//! Every unsafe call states, in its `SAFETY` comment, the invariant it relies on.
#![allow(unsafe_code)]

use std::io;
use std::path::Path;
use std::ptr;

use interprocess::os::windows::security_descriptor::{AsSecurityDescriptor, SecurityDescriptor};
use widestring::U16CString;
use windows_sys::Win32::Foundation::{ERROR_SUCCESS, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW, GetNamedSecurityInfoW, SDDL_REVISION_1,
    SE_FILE_OBJECT, SetNamedSecurityInfoW,
};
use windows_sys::Win32::Security::{
    ACL, DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl, PROTECTED_DACL_SECURITY_INFORMATION,
    PSECURITY_DESCRIPTOR,
};

use crate::windows_endpoint_access::{current_user_sid, wide_to_string};

/// The SDDL of an owner-only directory: full access for the current user and for SYSTEM, inherited by every file and
/// directory created in it (`OICI`), and nothing from the parent (`P`).
pub fn owner_only_dir_sddl() -> io::Result<String> {
    Ok(format!(
        "D:P(A;OICI;FA;;;{})(A;OICI;FA;;;SY)",
        current_user_sid()?
    ))
}

/// Replaces `dir`'s DACL with [`owner_only_dir_sddl`] and cuts inheritance from its parent. Call it on a new, empty
/// directory before anything is written in it, so every child inherits the owner-only entries.
pub fn restrict_dir_to_owner(dir: &Path) -> io::Result<()> {
    let sddl = U16CString::from_str(owner_only_dir_sddl()?)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let descriptor = SecurityDescriptor::deserialize(&sddl)?;
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
    if read == 0 {
        return Err(io::Error::last_os_error());
    }
    if present == 0 || acl.is_null() {
        return Err(io::Error::other("the owner-only descriptor has no DACL"));
    }
    let name = wide_path(dir)?;
    // SAFETY: `name` is a NUL-terminated path that lives through the call, and `acl` points into `descriptor`, which
    // also outlives it; owner, group and SACL are not set, so their null pointers are not read.
    let status = unsafe {
        SetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            acl,
            ptr::null(),
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    Ok(())
}

/// The DACL of `path` as SDDL text, for checks and tests.
pub fn dacl_sddl(path: &Path) -> io::Result<String> {
    let name = wide_path(path)?;
    let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
    // SAFETY: `name` is NUL-terminated and lives through the call; only the descriptor out-pointer is requested, and
    // the call allocates it with LocalAlloc.
    let status = unsafe {
        GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let mut text: *mut u16 = ptr::null_mut();
    // SAFETY: `descriptor` came from GetNamedSecurityInfoW above, and `text` is a valid out-pointer; the length is not
    // asked for.
    let converted = unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor,
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION,
            &mut text,
            ptr::null_mut(),
        )
    };
    let result = if converted == 0 {
        Err(io::Error::last_os_error())
    } else {
        let sddl = wide_to_string(text);
        // SAFETY: the string was allocated by the conversion with LocalAlloc, so LocalFree releases it exactly once.
        unsafe { LocalFree(text.cast()) };
        Ok(sddl)
    };
    // SAFETY: the descriptor was allocated by GetNamedSecurityInfoW with LocalAlloc and is not used after this.
    unsafe { LocalFree(descriptor) };
    result
}

fn wide_path(path: &Path) -> io::Result<U16CString> {
    U16CString::from_os_str(path.as_os_str())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))
}

#[cfg(test)]
mod tests {
    use super::{dacl_sddl, owner_only_dir_sddl, restrict_dir_to_owner};
    use crate::windows_endpoint_access::current_user_sid;

    #[test]
    fn a_restricted_directory_has_a_protected_dacl_for_the_owner_and_system_only() {
        let parent = tempfile::tempdir().unwrap();
        let dir = parent.path().join("secret");
        std::fs::create_dir(&dir).unwrap();
        restrict_dir_to_owner(&dir).unwrap();
        std::fs::write(dir.join("file"), b"fake").unwrap();

        let sid = current_user_sid().unwrap();
        let sddl = dacl_sddl(&dir).unwrap();
        assert!(sddl.starts_with("D:P"), "{sddl}");
        assert!(sddl.contains(&sid), "{sddl}");
        assert!(sddl.contains(";;;SY)"), "{sddl}");
        let file = dacl_sddl(&dir.join("file")).unwrap();
        for granted in [&sddl, &file] {
            for other in [";AU)", ";BU)", ";WD)", ";BA)"] {
                assert!(!granted.contains(other), "{granted}");
            }
        }
        assert!(
            file.contains(&sid),
            "the file inherits the owner entry: {file}"
        );
        assert!(owner_only_dir_sddl().unwrap().starts_with("D:P("));
    }
}
