//! Windows access of the profile root (ADR 0073): created with a protected owner-only DACL, checked and repaired on start.
//! Descriptors are read as SDDL and parsed: SIDs are compared as SIDs (`EqualSid`), access masks as numbers.
//! Every unsafe call states, in its `SAFETY` comment, the invariant it relies on.
#![allow(unsafe_code)]

use std::fs;
use std::io;
use std::path::Path;
use std::ptr;
use std::time::SystemTime;

use interprocess::os::windows::security_descriptor::{
    AsSecurityDescriptor, AsSecurityDescriptorExt, SecurityDescriptor,
};
use widestring::{U16CStr, U16CString};
use windows_sys::Win32::Foundation::GENERIC_WRITE;
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_SUCCESS, INVALID_HANDLE_VALUE, LocalFree,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW, GetSecurityInfo, ProgressInvokeNever,
    SDDL_REVISION_1, SE_FILE_OBJECT, SetNamedSecurityInfoW, SetSecurityInfo,
    TREE_SEC_INFO_RESET_KEEP_EXPLICIT, TreeSetNamedSecurityInfoW,
};
use windows_sys::Win32::Security::{
    ACL, DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
    PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
    UNPROTECTED_DACL_SECURITY_INFORMATION,
};
use windows_sys::Win32::Storage::FileSystem::{
    CREATE_NEW, CreateDirectoryW, CreateFileW, DELETE, FILE_ATTRIBUTE_DIRECTORY,
    FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, GetDriveTypeW, GetVolumeInformationW, GetVolumePathNameW, OPEN_EXISTING,
    READ_CONTROL, WRITE_DAC,
};

use crate::root_access::{Findings, RootAccess, RootAccessError, profiles_dir};
use crate::scan_cache::{ScanCache, stamp};
use crate::windows_dir_access::Sid;
use crate::windows_sddl::{Descriptor, verbatim};
use crate::windows_trust::{Trusted, current_user};

/// `GetDriveTypeW` for a network drive (`DRIVE_REMOTE` in WinBase.h).
const DRIVE_REMOTE: u32 = 4;

/// Whether a descriptor read opens a link itself or what it points to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Links {
    Follow,
    Open,
}

/// The descriptor reads and DACL writes `secure_root` makes, so tests can report owners and users no unprivileged
/// test can set, and record a repair instead of applying it.
pub(crate) trait Disk {
    /// The owner and DACL of `path` as SDDL.
    fn read(&self, path: &Path, links: Links) -> io::Result<String>;
    /// Replaces the DACL of `path` with the protected `dacl`.
    fn protect(&self, path: &Path, dacl: &str) -> io::Result<()>;
}

#[derive(Debug)]
pub(crate) struct RealDisk;

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
pub(crate) fn owner_only_dir_dacl_sddl(user: &Sid) -> io::Result<String> {
    Ok(format!("D:P(A;OICI;FA;;;{user})"))
}

pub(crate) fn secure_root(home: &Path, dir: &Path) -> Result<RootAccess, RootAccessError> {
    secure_root_with(home, dir, Trusted::current()?, &RealDisk)
}

fn secure_root_with(
    home: &Path,
    dir: &Path,
    trusted: Trusted,
    disk: &dyn Disk,
) -> Result<RootAccess, RootAccessError> {
    let user = trusted.user().clone();
    if let Some(parent) = home.parent() {
        fs::create_dir_all(parent)?;
    }
    check_link_home(home, &trusted, disk)?;
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
    if !root.owner.as_ref().is_some_and(|owner| trusted.owns(owner)) {
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
    let repaired = !root.is_owner_only(&user);
    if repaired {
        // Before the scan, so nothing can be planted between the listing and the new access. A refusal by the scan
        // leaves the root owner-only: that only takes access away and trusts nothing.
        disk.protect(dir, &owner_only_dir_dacl_sddl(&user)?)?;
    }
    let (findings, scanned) = scan_entries(dir, &trusted, disk, start_cache(dir, repaired))?;
    if let Err(refused) = findings.into_result() {
        ScanCache::clear(dir);
        return Err(refused);
    }
    // The cache only saves time; a start never fails on it.
    let _ = scanned.save(dir);
    Ok(if repaired {
        RootAccess::Repaired
    } else {
        RootAccess::Private
    })
}

/// The scan cache a start may use: none after a repair, off a local NTFS volume, or when a full scan is due.
pub(crate) fn start_cache(dir: &Path, repaired: bool) -> Option<ScanCache> {
    if repaired || !folder_times_are_reliable(dir) {
        ScanCache::clear(dir);
        return None;
    }
    ScanCache::load(dir, SystemTime::now())
}

/// Whether folder modification times on `dir`'s volume can be trusted to change when an entry is added, removed or
/// renamed: only on a local NTFS volume. A remote drive, FAT, exFAT, ReFS or a volume that cannot be read count as
/// unreliable, so every start reads every descriptor there.
fn folder_times_are_reliable(dir: &Path) -> bool {
    let Ok(path) = wide_path(dir) else {
        return false;
    };
    let mut volume = [0_u16; 1024];
    // SAFETY: `path` is NUL-terminated, and `volume` is a writable buffer of the length passed.
    if unsafe { GetVolumePathNameW(path.as_ptr(), volume.as_mut_ptr(), volume.len() as u32) } == 0 {
        return false;
    }
    // SAFETY: on success `volume` holds a NUL-terminated volume root path.
    if unsafe { GetDriveTypeW(volume.as_ptr()) } == DRIVE_REMOTE {
        return false;
    }
    let mut name = [0_u16; 64];
    // SAFETY: `volume` is a NUL-terminated root path; only the file system name buffer is requested, with its length,
    // and the other out-pointers may be null.
    let read = unsafe {
        GetVolumeInformationW(
            volume.as_ptr(),
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            name.as_mut_ptr(),
            name.len() as u32,
        )
    };
    let len = name
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(name.len());
    read != 0 && String::from_utf16_lossy(&name[..len]) == "NTFS"
}

/// The read-only part of a start's location check: nothing is created or repaired, and the content is not scanned.
pub(crate) fn check_location(home: &Path, dir: &Path) -> Result<(), RootAccessError> {
    let trusted = Trusted::current()?;
    check_link_home(home, &trusted, &RealDisk)?;
    for (ancestor, links) in [
        (home, Links::Follow),
        (profiles_dir(home, dir), Links::Open),
    ] {
        if fs::symlink_metadata(ancestor).is_ok() {
            check_ancestor(ancestor, links, &trusted, &RealDisk)?;
        }
    }
    if fs::symlink_metadata(dir).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(RootAccessError::UnsafeLocation {
            path: dir.to_path_buf(),
        });
    }
    Ok(())
}

/// Who can reach a folder that will hold secrets, judged from its own DACL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FolderAccess {
    /// Protected, with one entry: full access for the user.
    Private,
    /// Only the accepted owners and Administrators, but not yet the protected owner-only DACL.
    OwnersOnly,
    WritableByOthers,
    ReadableByOthers,
}

pub(crate) fn folder_access(path: &Path) -> Result<FolderAccess, RootAccessError> {
    let descriptor = Descriptor::parse(&read_sddl(path, Links::Open)?);
    let trusted = Trusted::current()?;
    Ok(if descriptor.grants_write_to_others(&trusted) {
        FolderAccess::WritableByOthers
    } else if descriptor.grants_any_to_others(&trusted) {
        FolderAccess::ReadableByOthers
    } else if descriptor.is_owner_only(trusted.user()) {
        FolderAccess::Private
    } else {
        FolderAccess::OwnersOnly
    })
}

/// Gives the folder `path` the protected owner-only DACL, inherited by what is created in it later.
pub(crate) fn make_owner_only(path: &Path) -> io::Result<()> {
    set_dacl(path, &owner_only_dir_dacl_sddl(&current_user()?)?, true)
}

/// Creates a new file at `path` whose descriptor allows only the current user, in the same call that creates it, so it
/// is never reachable by anyone else, not even briefly. Fails when anything is at `path`, a link included.
pub(crate) fn create_new_owner_only(path: &Path) -> io::Result<fs::File> {
    use std::os::windows::io::FromRawHandle;

    let user = current_user()?;
    let descriptor = descriptor(&format!("O:{user}D:P(A;;FA;;;{user})"))?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.as_sd().cast_mut(),
        bInheritHandle: 0,
    };
    let wide = wide_path(path)?;
    // SAFETY: `wide` is NUL-terminated and `attributes` points at `descriptor`, which both outlive the call; the result
    // is checked against INVALID_HANDLE_VALUE before a File owns it.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
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
    // SAFETY: the handle was just opened and nothing else owns it.
    Ok(unsafe { fs::File::from_raw_handle(handle) })
}

/// A home that is itself a link is checked through its target, but the link lives in its parent: whoever can change
/// that parent can point the home elsewhere later. So the parent must be as safe as an ancestor.
pub(crate) fn check_link_home(
    home: &Path,
    trusted: &Trusted,
    disk: &dyn Disk,
) -> Result<(), RootAccessError> {
    let is_link =
        fs::symlink_metadata(home).is_ok_and(|metadata| metadata.file_type().is_symlink());
    let Some(parent) = home.parent().filter(|_| is_link) else {
        return Ok(());
    };
    match check_ancestor(parent, Links::Follow, trusted, disk) {
        Err(RootAccessError::UnsafeLocation { .. }) => Err(RootAccessError::UnsafeLocation {
            path: home.to_path_buf(),
        }),
        other => other,
    }
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
        .is_some_and(|owner| trusted.owns(owner));
    if !owned || descriptor.grants_write_to_others(trusted) {
        return Err(unsafe_location());
    }
    Ok(())
}

/// Every entry below `root`, links read themselves and never followed: owners must be trusted and readable, and no
/// entry may let others change it. Folders `cache` records unchanged are listed but their entries' descriptors are not
/// read. Returns the findings and the folders this scan found clean.
fn scan_entries(
    root: &Path,
    trusted: &Trusted,
    disk: &dyn Disk,
    cache: Option<ScanCache>,
) -> io::Result<(Findings, ScanCache)> {
    let mut findings = Findings::default();
    let mut scanned = cache
        .as_ref()
        .map_or_else(|| ScanCache::full_scan(SystemTime::now()), ScanCache::next);
    let cache = cache.unwrap_or_default();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        // Read from the folder itself before it is listed: the copy in its parent's listing is updated lazily.
        let dir_stamp = stamp(fs::metadata(&dir).and_then(|meta| meta.modified()));
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if dir == root => return Err(error),
            // Removed since its parent was listed, for example by a service starting at the same time: nothing is hidden.
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            // A folder that cannot be listed may hide planted content.
            Err(_) => {
                findings.add_foreign(dir);
                continue;
            }
        };
        let unchanged = cache.is_unchanged(root, &dir, dir_stamp);
        let mut clean = true;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let is_dir = entry.file_type()?.is_dir();
            if unchanged {
                if is_dir {
                    pending.push(path);
                }
                continue;
            }
            match disk
                .read(&path, Links::Open)
                .map(|sddl| Descriptor::parse(&sddl))
            {
                Ok(descriptor) => {
                    let owned = descriptor
                        .owner
                        .as_ref()
                        .is_some_and(|owner| trusted.owns(owner));
                    if !owned {
                        clean = false;
                        findings.add_foreign(path);
                    } else if descriptor.grants_write_to_others(trusted) {
                        clean = false;
                        findings.add_writable(path);
                    } else if is_dir {
                        pending.push(path);
                    }
                }
                // Removed since it was listed, as a concurrent start does with the store's journal.
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(_) => {
                    clean = false;
                    findings.add_foreign(path);
                }
            }
            if findings.is_full() {
                return Ok((findings, scanned));
            }
        }
        if clean {
            scanned.record(root, &dir, dir_stamp);
        }
    }
    Ok((findings, scanned))
}

/// Creates `dir` with the owner-only descriptor in one call, so it is never reachable by others; false when it existed.
pub(crate) fn create_private_dir(dir: &Path) -> io::Result<bool> {
    let user = current_user()?;
    let descriptor = descriptor(&format!("O:{}{}", user, owner_only_dir_dacl_sddl(&user)?))?;
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
pub(crate) fn set_dacl(path: &Path, dacl: &str, protected: bool) -> io::Result<()> {
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

/// Opens a folder to change its access, without sharing delete, so it cannot be renamed, deleted or replaced while
/// the handle lives. With `Links::Open` a link is opened itself and refused; anything but a folder is refused too.
pub(crate) fn pin_for_access_change(
    path: &Path,
    links: Links,
) -> Result<fs::File, RootAccessError> {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};

    let flags = match links {
        Links::Follow => FILE_FLAG_BACKUP_SEMANTICS,
        Links::Open => FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
    };
    let file = fs::OpenOptions::new()
        // DELETE makes the share mode bind: Windows checks sharing only against handles with data or delete access.
        .access_mode(READ_CONTROL | WRITE_DAC | FILE_READ_ATTRIBUTES | DELETE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(flags)
        .open(path)?;
    let attributes = file.metadata()?.file_attributes();
    if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 || attributes & FILE_ATTRIBUTE_DIRECTORY == 0
    {
        return Err(RootAccessError::UnsafeLocation {
            path: path.to_path_buf(),
        });
    }
    Ok(file)
}

/// Replaces the DACL of the folder `file` holds with the protected `dacl`, on exactly that object. It does not pass the
/// change on to what is below; `set_dacl` by path does that.
pub(crate) fn set_dacl_through(file: &fs::File, dacl: &str) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;

    let descriptor = descriptor(dacl)?;
    let (acl, _) = descriptor
        .dacl()?
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the descriptor has no DACL"))?;
    // SAFETY: the handle is open with WRITE_DAC for the call, and `acl` points into `descriptor`, which outlives it;
    // owner, group and SACL are not set, so their null pointers are not read.
    let status = unsafe {
        SetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
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

/// Sets the protected `dacl` on `path` and resets the inherited entries of everything below it, keeping their explicit
/// ones. Unlike a plain set, this walks the tree even when the folder's own DACL is already `dacl`.
pub(crate) fn reset_inherited_below(path: &Path, dacl: &str) -> io::Result<()> {
    let descriptor = descriptor(dacl)?;
    let (acl, _) = descriptor
        .dacl()?
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the descriptor has no DACL"))?;
    let path = wide_path(path)?;
    // SAFETY: `path` is NUL-terminated and `acl` points into `descriptor`; both outlive the call, which only reads them.
    // No progress callback is passed, so the callback argument is never read.
    let status = unsafe {
        TreeSetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            acl.cast::<ACL>(),
            ptr::null(),
            TREE_SEC_INFO_RESET_KEEP_EXPLICIT,
            None,
            ProgressInvokeNever,
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
pub(crate) fn read_sddl(path: &Path, links: Links) -> io::Result<String> {
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
