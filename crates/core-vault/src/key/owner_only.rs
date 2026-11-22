//! Windows: the vault folder and its files are created with an owner-only, protected DACL, so they never inherit the
//! permissions of the folder above. Every unsafe call states, in its `SAFETY` comment, the invariant it relies on.
#![expect(
    unsafe_code,
    reason = "CreateDirectoryW and CreateFileW are C APIs; decision 0007 allows unsafe only in named Windows modules"
)]

use std::fs::File;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::FromRawHandle;
use std::path::Path;
use std::ptr;

use interprocess::os::windows::security_descriptor::AsSecurityDescriptor;
use massorca_profile::owner_only_security_descriptor;
use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GENERIC_WRITE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::{
    CREATE_NEW, CreateDirectoryW, CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_OPEN_REPARSE_POINT,
};

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

/// Creates `dir` with an owner-only DACL; an existing directory is kept as it is.
pub(super) fn create_dir(dir: &Path) -> io::Result<()> {
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let descriptor = owner_only_security_descriptor()?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(u32::MAX),
        lpSecurityDescriptor: descriptor.borrow().as_sd().cast_mut(),
        bInheritHandle: 0,
    };
    let path = wide(dir);
    // SAFETY: `path` is NUL-terminated, and `attributes` points at a descriptor that lives until the call returns.
    if unsafe { CreateDirectoryW(path.as_ptr(), &attributes) } != 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == i32::try_from(ERROR_ALREADY_EXISTS).ok() {
        return Ok(());
    }
    Err(error)
}

/// Creates `path`, which must not exist, with an owner-only DACL, open for writing. A link is never followed.
pub(super) fn create_new_file(path: &Path) -> io::Result<File> {
    let descriptor = owner_only_security_descriptor()?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(u32::MAX),
        lpSecurityDescriptor: descriptor.borrow().as_sd().cast_mut(),
        bInheritHandle: 0,
    };
    let path = wide(path);
    // SAFETY: `path` is NUL-terminated, `attributes` outlives the call, and the template handle may be null.
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            GENERIC_WRITE,
            0,
            &attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `handle` is a valid file handle that this call just opened and nothing else owns.
    Ok(unsafe { File::from_raw_handle(handle) })
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::ptr;

    use massorca_profile::owner_only_sddl;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, GetNamedSecurityInfoW, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::{
        ACCESS_ALLOWED_ACE, ACL, ACL_SIZE_INFORMATION, AclSizeInformation,
        DACL_SECURITY_INFORMATION, GetAce, GetAclInformation, GetSecurityDescriptorControl,
        PSECURITY_DESCRIPTOR, SE_DACL_PROTECTED,
    };

    use super::{create_dir, create_new_file};

    /// Whether the DACL is protected, and the SID of each of its ACEs.
    fn dacl_of(path: &Path) -> (bool, Vec<String>) {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut acl: *mut ACL = ptr::null_mut();
        let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
        // SAFETY: `wide` is NUL-terminated; the out-pointers are valid, and the descriptor is freed below.
        let status = unsafe {
            GetNamedSecurityInfoW(
                wide.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut acl,
                ptr::null_mut(),
                &mut descriptor,
            )
        };
        assert_eq!(status, 0, "{}", io::Error::from_raw_os_error(status as i32));
        let mut control = 0u16;
        let mut revision = 0u32;
        // SAFETY: `descriptor` came from GetNamedSecurityInfoW and is still alive.
        assert_ne!(
            unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) },
            0
        );
        let mut info = ACL_SIZE_INFORMATION::default();
        // SAFETY: `acl` points into `descriptor`, and `info` has the size the call is told.
        assert_ne!(
            unsafe {
                GetAclInformation(
                    acl,
                    ptr::from_mut(&mut info).cast(),
                    u32::try_from(size_of::<ACL_SIZE_INFORMATION>()).unwrap_or(u32::MAX),
                    AclSizeInformation,
                )
            },
            0
        );
        let sids = (0..info.AceCount)
            .map(|index| {
                let mut ace = ptr::null_mut();
                // SAFETY: `index` is below AceCount, and `ace` is a valid out-pointer.
                assert_ne!(unsafe { GetAce(acl, index, &mut ace) }, 0);
                // SAFETY: the SID of an allowed ACE starts at SidStart and stays inside the ACE.
                let allowed = unsafe { &*ace.cast::<ACCESS_ALLOWED_ACE>() };
                let mut text: *mut u16 = ptr::null_mut();
                let sid = ptr::from_ref(&allowed.SidStart).cast_mut().cast();
                // SAFETY: `sid` is a valid SID, and `text` receives a LocalAlloc'd string freed below.
                assert_ne!(unsafe { ConvertSidToStringSidW(sid, &mut text) }, 0);
                // SAFETY: `text` is NUL-terminated by ConvertSidToStringSidW.
                let length = (0..).take_while(|&i| unsafe { *text.add(i) } != 0).count();
                // SAFETY: `text` holds `length` valid UTF-16 units.
                let value =
                    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, length) });
                // SAFETY: allocated by ConvertSidToStringSidW, freed exactly once.
                unsafe { LocalFree(text.cast()) };
                value
            })
            .collect();
        // SAFETY: allocated by GetNamedSecurityInfoW, freed exactly once; `acl` is not used after this.
        unsafe { LocalFree(descriptor.cast()) };
        (control & SE_DACL_PROTECTED != 0, sids)
    }

    fn current_sid() -> String {
        let sddl = owner_only_sddl().expect("sddl");
        sddl.trim_start_matches("D:P(A;;GA;;;")
            .trim_end_matches(')')
            .to_owned()
    }

    #[test]
    fn the_folder_and_a_new_file_are_protected_with_one_ace_for_the_user() {
        let root = tempfile::tempdir().expect("tempdir");
        let dir = root.path().join("profile").join("vault");
        create_dir(&dir).expect("the folder");
        create_dir(&dir).expect("an existing folder is kept");
        let key = dir.join("key");
        drop(create_new_file(&key).expect("the file"));
        assert!(
            create_new_file(&key).is_err(),
            "an existing file is never replaced"
        );
        for path in [&dir, &key] {
            assert_eq!(
                dacl_of(path),
                (true, vec![current_sid()]),
                "{}",
                path.display()
            );
        }
    }
}
