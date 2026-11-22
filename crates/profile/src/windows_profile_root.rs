//! Windows access of the profile root (ADR 0073): created with a protected owner-only DACL, checked and repaired on start.
//! Descriptors are compared as SDDL text, so SIDs and access masks are spelled the same way on both sides.
//! Every unsafe call states, in its `SAFETY` comment, the invariant it relies on.
#![allow(unsafe_code)]

use std::fs;
use std::io;
use std::path::Path;
use std::ptr;

use interprocess::os::windows::security_descriptor::{
    AsSecurityDescriptor, AsSecurityDescriptorExt, SecurityDescriptor,
};
use widestring::{U16CStr, U16CString};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_SUCCESS, INVALID_HANDLE_VALUE, LocalFree,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW, GetSecurityInfo, SDDL_REVISION_1,
    SE_FILE_OBJECT, SetNamedSecurityInfoW,
};
use windows_sys::Win32::Security::{
    ACL, DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
    PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
    UNPROTECTED_DACL_SECURITY_INFORMATION,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateDirectoryW, CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, READ_CONTROL,
};

use crate::root_access::{Findings, RootAccess, RootAccessError, profiles_dir};
use crate::windows_sddl::{Descriptor, verbatim};
use crate::windows_sid::{Sid, Trusted};

/// Whether a descriptor read opens a link itself or what it points to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Links {
    Follow,
    Open,
}

/// The descriptor reads and DACL writes `secure_root` makes, so tests can report owners and users no unprivileged
/// test can set, and record a repair instead of applying it.
trait Disk {
    /// The owner and DACL of `path` as SDDL.
    fn read(&self, path: &Path, links: Links) -> io::Result<String>;
    /// Replaces the DACL of `path` with the protected `dacl`.
    fn protect(&self, path: &Path, dacl: &str) -> io::Result<()>;
}

#[derive(Debug)]
struct RealDisk;

impl Disk for RealDisk {
    fn read(&self, path: &Path, links: Links) -> io::Result<String> {
        read_sddl(path, links)
    }

    fn protect(&self, path: &Path, dacl: &str) -> io::Result<()> {
        set_dacl(path, dacl, true)
    }
}

/// The DACL of a private folder: full access for `user`, inherited by every file and folder below, nothing from above.
/// The user is written as an `S-1-…` string, never an alias.
fn owner_only_dir_dacl_sddl(user: &Sid) -> io::Result<String> {
    Ok(format!("D:P(A;OICI;FA;;;{})", user.to_sid_string()?))
}

pub(crate) fn secure_root(home: &Path, dir: &Path) -> Result<RootAccess, RootAccessError> {
    secure_root_with(home, dir, Sid::current_user()?, &RealDisk)
}

fn secure_root_with(
    home: &Path,
    dir: &Path,
    user: Sid,
    disk: &dyn Disk,
) -> Result<RootAccess, RootAccessError> {
    let trusted = Trusted::for_user(user)?;
    if let Some(parent) = home.parent() {
        fs::create_dir_all(parent)?;
    }
    // The home may itself be a link the user chose; `profiles` and the root may not.
    for (ancestor, links) in [
        (home, Links::Follow),
        (profiles_dir(home, dir), Links::Open),
    ] {
        if !create_private_dir(ancestor)? {
            check_ancestor(ancestor, links, &trusted, disk)?;
        }
    }
    if create_private_dir(dir)? {
        return Ok(RootAccess::Created);
    }
    if fs::symlink_metadata(dir)?.file_type().is_symlink() {
        return Err(RootAccessError::UnsafeLocation {
            path: dir.to_path_buf(),
        });
    }
    let root = Descriptor::parse(&disk.read(dir, Links::Open)?);
    if root.owner.as_ref() != Some(&trusted.user) {
        return Err(RootAccessError::ForeignEntries {
            paths: vec![dir.to_path_buf()],
        });
    }
    // Others could have rewritten files the user owns; locking them in would make the content trusted.
    if root.grants_write_to_others(&trusted) {
        return Err(RootAccessError::WritableByOthers {
            paths: vec![dir.to_path_buf()],
        });
    }
    let repaired = !root.is_owner_only(&trusted.user);
    if repaired {
        disk.protect(dir, &owner_only_dir_dacl_sddl(&trusted.user)?)?;
    }
    // After the repair, so nothing can be planted between the listing and the new access.
    scan_entries(dir, &trusted, disk)?.into_result()?;
    Ok(if repaired {
        RootAccess::Repaired
    } else {
        RootAccess::Private
    })
}

/// A folder between the home and the root must be a real folder (unless `links` follows), owned by a trusted SID,
/// and changeable by nobody else.
fn check_ancestor(
    path: &Path,
    links: Links,
    trusted: &Trusted,
    disk: &dyn Disk,
) -> Result<(), RootAccessError> {
    let unsafe_location = || RootAccessError::UnsafeLocation {
        path: path.to_path_buf(),
    };
    if links == Links::Open && fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(unsafe_location());
    }
    let descriptor = Descriptor::parse(&disk.read(path, links)?);
    let owned = descriptor
        .owner
        .as_ref()
        .is_some_and(|owner| trusted.contains(owner));
    if !owned || descriptor.grants_write_to_others(trusted) {
        return Err(unsafe_location());
    }
    Ok(())
}

/// Every entry below `root`, links read themselves and never followed: owners must be trusted and readable, and no
/// entry may let others change it.
fn scan_entries(root: &Path, trusted: &Trusted, disk: &dyn Disk) -> io::Result<Findings> {
    let mut findings = Findings::default();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if dir == root => return Err(error),
            // A folder that cannot be listed may hide planted content.
            Err(_) => {
                findings.add_foreign(dir);
                continue;
            }
        };
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            match disk
                .read(&path, Links::Open)
                .map(|sddl| Descriptor::parse(&sddl))
            {
                Ok(descriptor) => {
                    let owned = descriptor
                        .owner
                        .as_ref()
                        .is_some_and(|owner| trusted.contains(owner));
                    if !owned {
                        findings.add_foreign(path);
                    } else if descriptor.grants_write_to_others(trusted) {
                        findings.add_writable(path);
                    } else if entry.file_type()?.is_dir() {
                        pending.push(path);
                    }
                }
                Err(_) => findings.add_foreign(path),
            }
            if findings.is_full() {
                return Ok(findings);
            }
        }
    }
    Ok(findings)
}

/// Creates `dir` with the owner-only descriptor in one call, so it is never reachable by others; false when it existed.
pub(crate) fn create_private_dir(dir: &Path) -> io::Result<bool> {
    let user = Sid::current_user()?;
    let descriptor = descriptor(&format!(
        "O:{}{}",
        user.to_sid_string()?,
        owner_only_dir_dacl_sddl(&user)?
    ))?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.as_sd().cast_mut(),
        bInheritHandle: 0,
    };
    let path = wide_path(dir)?;
    // SAFETY: `path` is NUL-terminated, and `attributes` points at `descriptor`, which both outlive the call.
    if unsafe { CreateDirectoryW(path.as_ptr(), &attributes) } != 0 {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(ERROR_ALREADY_EXISTS as i32) {
        Ok(false)
    } else {
        Err(error)
    }
}

fn descriptor(sddl: &str) -> io::Result<SecurityDescriptor> {
    let text = U16CString::from_str(sddl)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    SecurityDescriptor::deserialize(&text)
}

/// Replaces the DACL of `path` with `dacl`. Windows passes the change on to the inherited entries of everything below.
fn set_dacl(path: &Path, dacl: &str, protected: bool) -> io::Result<()> {
    let descriptor = descriptor(dacl)?;
    let (acl, _) = descriptor
        .dacl()?
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the descriptor has no DACL"))?;
    let path = wide_path(path)?;
    let protection = if protected {
        PROTECTED_DACL_SECURITY_INFORMATION
    } else {
        UNPROTECTED_DACL_SECURITY_INFORMATION
    };
    // SAFETY: `path` is NUL-terminated and `acl` points into `descriptor`; both outlive the call, which only reads them.
    let status = unsafe {
        SetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | protection,
            ptr::null_mut(),
            ptr::null_mut(),
            acl.cast::<ACL>(),
            ptr::null(),
        )
    };
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status as i32))
    }
}

/// The owner and DACL of `path` as SDDL; with `Links::Open` a link is read itself, never its target.
fn read_sddl(path: &Path, links: Links) -> io::Result<String> {
    let wide = wide_path(path)?;
    let flags = match links {
        Links::Follow => FILE_FLAG_BACKUP_SEMANTICS,
        Links::Open => FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
    };
    // SAFETY: `wide` is NUL-terminated and outlives the call; the result is checked against INVALID_HANDLE_VALUE.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            READ_CONTROL,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            ptr::null(),
            OPEN_EXISTING,
            flags,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
    // SAFETY: `handle` is open with READ_CONTROL; the owner, group and ACL out-pointers may be null, and `descriptor`
    // receives a LocalAlloc'd copy that is freed below.
    let status = unsafe {
        GetSecurityInfo(
            handle,
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    // SAFETY: the handle came from CreateFileW above and is closed only here, once.
    unsafe { CloseHandle(handle) };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let text = descriptor_to_sddl(descriptor);
    // SAFETY: GetSecurityInfo allocated `descriptor` with LocalAlloc, so LocalFree releases it exactly once.
    unsafe { LocalFree(descriptor) };
    text
}

fn descriptor_to_sddl(descriptor: PSECURITY_DESCRIPTOR) -> io::Result<String> {
    let mut text: *mut u16 = ptr::null_mut();
    // SAFETY: `descriptor` is a valid descriptor for the call, and `text` receives a LocalAlloc'd string; the length is optional.
    let converted = unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor,
            SDDL_REVISION_1,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut text,
            ptr::null_mut(),
        )
    };
    if converted == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: on success `text` is a NUL-terminated string that stays valid until it is freed below.
    let value = unsafe { U16CStr::from_ptr_str(text) }.to_string_lossy();
    // SAFETY: the string was allocated with LocalAlloc by the conversion, so LocalFree releases it exactly once.
    unsafe { LocalFree(text.cast()) };
    Ok(value)
}

/// The NUL-terminated `\\?\` form of `path`, which Win32 calls accept past 248 characters.
fn wide_path(path: &Path) -> io::Result<U16CString> {
    U16CString::from_os_str(verbatim(path))
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))
}

#[cfg(test)]
#[path = "windows_profile_root_tests.rs"]
mod tests;
