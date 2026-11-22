//! Owner-only directories on Windows: an explicit, protected DACL for directories that hold secrets, such as agent
//! credential homes (0056). The profile directory itself keeps its inherited access (0004).
//! Every unsafe call states, in its `SAFETY` comment, the invariant it relies on.
#![allow(unsafe_code)]

use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::os::windows::io::AsRawHandle;
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
    PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateDirectoryW, DELETE, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
    FILE_RENAME_INFO, FILE_SHARE_READ, FILE_SHARE_WRITE, FileRenameInfo,
    SetFileInformationByHandle,
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

/// The descriptor of [`owner_only_dir_sddl`].
fn owner_only_dir_descriptor() -> io::Result<SecurityDescriptor> {
    let sddl = U16CString::from_str(owner_only_dir_sddl()?)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    SecurityDescriptor::deserialize(&sddl)
}

/// Creates `dir` with the owner-only DACL of [`owner_only_dir_sddl`] from its first moment, so no other user ever has
/// access to it. Fails with `AlreadyExists` when anything is at the path, a link or junction included, which is never
/// followed.
pub fn create_dir_owner_only(dir: &Path) -> io::Result<()> {
    let descriptor = owner_only_dir_descriptor()?;
    let name = wide_path(dir)?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.borrow().as_sd().cast_mut(),
        bInheritHandle: 0,
    };
    // SAFETY: `name` is a NUL-terminated path, and `attributes` points at `descriptor`; both outlive the call, which
    // only reads them.
    if unsafe { CreateDirectoryW(name.as_ptr(), &attributes) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Whether `path`'s DACL is protected and grants only the current user and SYSTEM, with no inherited entry: what
/// [`create_dir_owner_only`] and [`restrict_dir_to_owner`] set.
pub fn has_owner_only_dacl(path: &Path) -> io::Result<bool> {
    Ok(is_owner_only_sddl(&dacl_sddl(path)?, &current_user_sid()?))
}

fn is_owner_only_sddl(sddl: &str, user_sid: &str) -> bool {
    // `D:`, then the DACL's control flags (`P` protected, `AI`/`AR` auto-inherit), then the ACEs.
    let Some(rest) = sddl.strip_prefix("D:") else {
        return false;
    };
    let Some(open) = rest.find('(') else {
        return false;
    };
    let (flags, aces) = rest.split_at(open);
    if !flags.contains('P') {
        return false;
    }
    let Some(aces) = aces
        .strip_prefix('(')
        .and_then(|aces| aces.strip_suffix(')'))
    else {
        return false;
    };
    aces.split(")(").all(|ace| {
        let fields: Vec<&str> = ace.split(';').collect();
        fields.len() == 6
            && fields[0] == "A"
            && !fields[1].contains("ID")
            && (fields[5] == user_sid || fields[5] == "SY")
    })
}

/// Replaces `dir`'s DACL with [`owner_only_dir_sddl`] and cuts inheritance from its parent; children that inherit
/// get the owner-only entries. A directory made with [`create_dir_owner_only`] needs no call to this.
pub fn restrict_dir_to_owner(dir: &Path) -> io::Result<()> {
    let descriptor = owner_only_dir_descriptor()?;
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

/// A directory held open so that nobody, this process included, can rename, delete or replace it while the guard
/// lives: it is opened without `FILE_SHARE_DELETE`. It is opened as itself (`FILE_FLAG_OPEN_REPARSE_POINT`), and a
/// reparse point (a junction or a link) or anything but a directory is refused, so the pinned directory is the real
/// one. Only [`PinnedDir::rename_to`], through this handle, can still move it.
#[derive(Debug)]
pub struct PinnedDir {
    file: File,
}

impl PinnedDir {
    /// Opens and pins `dir`. Fails with `InvalidInput` when it is a reparse point or not a directory, and with a
    /// sharing violation when another handle already holds it without sharing delete.
    pub fn open(dir: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .access_mode(FILE_READ_ATTRIBUTES | DELETE)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(dir)?;
        let attributes = file.metadata()?.file_attributes();
        if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} is a reparse point", dir.display()),
            ));
        }
        if attributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} is not a directory", dir.display()),
            ));
        }
        Ok(Self { file })
    }

    /// Renames the pinned directory to `target` (a full path) through its own handle, so the pin holds until the
    /// move is done. Fails when anything is at `target`.
    pub fn rename_to(&self, target: &Path) -> io::Result<()> {
        let name: Vec<u16> = target.as_os_str().encode_wide().collect();
        let name_bytes = name.len() * size_of::<u16>();
        let header = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
        let size = header + name_bytes + size_of::<u16>();
        // A u64 buffer keeps FILE_RENAME_INFO aligned for its handle field.
        let mut buffer = vec![0_u64; size.div_ceil(size_of::<u64>())];
        let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        // SAFETY: the buffer is zeroed, aligned for FILE_RENAME_INFO and holds `size` bytes: the header, the name and a
        // NUL. The fields are written through the raw pointer, and the name is copied into the space after the header.
        unsafe {
            (*info).Anonymous.ReplaceIfExists = false;
            (*info).RootDirectory = ptr::null_mut();
            (*info).FileNameLength = u32::try_from(name_bytes).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "the target is too long")
            })?;
            ptr::copy_nonoverlapping(
                name.as_ptr(),
                ptr::addr_of_mut!((*info).FileName).cast::<u16>(),
                name.len(),
            );
        }
        let size = u32::try_from(size)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "the target is too long"))?;
        // SAFETY: the handle is open with DELETE access for the life of `self`, and `info` points at `size` valid bytes
        // laid out as FILE_RENAME_INFO, which the call only reads.
        if unsafe {
            SetFileInformationByHandle(self.file.as_raw_handle(), FileRenameInfo, info.cast(), size)
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

fn wide_path(path: &Path) -> io::Result<U16CString> {
    U16CString::from_os_str(path.as_os_str())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))
}

#[cfg(test)]
mod tests {
    use super::{
        PinnedDir, create_dir_owner_only, dacl_sddl, has_owner_only_dacl, is_owner_only_sddl,
        owner_only_dir_sddl, restrict_dir_to_owner,
    };

    #[test]
    fn a_pinned_directory_cannot_be_renamed_or_deleted_except_through_its_pin() {
        let parent = tempfile::tempdir().unwrap();
        let dir = parent.path().join("pinned");
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("file"), b"fake").unwrap();
        let pin = PinnedDir::open(&dir).unwrap();
        let refused = std::fs::rename(&dir, parent.path().join("moved")).unwrap_err();
        assert_eq!(refused.raw_os_error(), Some(32), "{refused}");
        assert!(
            std::fs::remove_dir(&dir).is_err(),
            "the pinned directory itself stays"
        );
        assert!(dir.is_dir());
        pin.rename_to(&parent.path().join("moved")).unwrap();
        assert!(parent.path().join("moved").join("file").exists());
    }

    #[test]
    fn a_junction_or_a_file_is_not_pinned() {
        let parent = tempfile::tempdir().unwrap();
        let target = parent.path().join("target");
        std::fs::create_dir(&target).unwrap();
        let junction = parent.path().join("junction");
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&target)
            .output()
            .unwrap();
        assert!(made.status.success());
        let error = PinnedDir::open(&junction).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        std::fs::write(parent.path().join("file"), b"fake").unwrap();
        let error = PinnedDir::open(&parent.path().join("file")).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }
    use crate::windows_endpoint_access::current_user_sid;

    #[test]
    fn a_directory_created_owner_only_is_protected_from_its_first_moment() {
        let parent = tempfile::tempdir().unwrap();
        let dir = parent.path().join("secret");
        create_dir_owner_only(&dir).unwrap();
        assert!(
            has_owner_only_dacl(&dir).unwrap(),
            "{}",
            dacl_sddl(&dir).unwrap()
        );
        std::fs::write(dir.join("file"), b"fake").unwrap();
        let file = dacl_sddl(&dir.join("file")).unwrap();
        for other in [";AU)", ";BU)", ";WD)", ";BA)"] {
            assert!(!file.contains(other), "{file}");
        }
        let again = create_dir_owner_only(&dir).unwrap_err();
        assert_eq!(again.kind(), std::io::ErrorKind::AlreadyExists);
        assert!(!has_owner_only_dacl(parent.path()).unwrap());
    }

    #[test]
    fn only_a_protected_dacl_for_the_user_and_system_counts_as_owner_only() {
        let sid = "S-1-5-21-1-2-3-1001";
        assert!(is_owner_only_sddl(
            &format!("D:P(A;OICI;FA;;;{sid})(A;OICI;FA;;;SY)"),
            sid
        ));
        assert!(is_owner_only_sddl(
            &format!("D:PAI(A;OICI;FA;;;{sid})(A;OICI;FA;;;SY)"),
            sid
        ));
        for sddl in [
            format!("D:(A;OICI;FA;;;{sid})"),
            format!("D:P(A;OICI;FA;;;{sid})(A;OICI;FA;;;AU)"),
            format!("D:P(A;OICIID;FA;;;{sid})"),
            "D:P(A;OICI;FA;;;S-1-5-21-9-9-9-1005)".to_owned(),
            format!("D:AI(A;OICI;FA;;;{sid})"),
        ] {
            assert!(!is_owner_only_sddl(&sddl, sid), "{sddl}");
        }
    }

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
