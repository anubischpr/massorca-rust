//! Owner-only directories on Windows: an explicit, protected DACL for directories that hold secrets, such as agent
//! credential homes (0056). The profile directory itself keeps its inherited access (0004).
//!
//! Owners and DACL entries are compared as binary SIDs (`EqualSid`), never as SDDL text or names: SDDL writes some
//! accounts as aliases (`LA` for the built-in Administrator), so text does not identify a principal. An owner is
//! acceptable when it is the token's user, the token's default owner (the Administrators group for an elevated
//! administrator, whose new objects that group owns), or SYSTEM; any other principal is refused, since an owner can
//! always rewrite an object's access list.
//! Every unsafe call states, in its `SAFETY` comment, the invariant it relies on.
#![allow(unsafe_code)]

use std::fmt;
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
    ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertStringSidToSidW,
    GetNamedSecurityInfoW, GetSecurityInfo, SDDL_REVISION_1, SE_FILE_OBJECT, SetNamedSecurityInfoW,
};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, ACL_SIZE_INFORMATION, AclSizeInformation, CopySid,
    DACL_SECURITY_INFORMATION, EqualSid, GetAce, GetAclInformation, GetLengthSid,
    GetSecurityDescriptorControl, GetSecurityDescriptorDacl, IsValidSid,
    OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
    SE_DACL_PROTECTED, SECURITY_ATTRIBUTES, TOKEN_OWNER, TOKEN_USER, TokenOwner, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateDirectoryW, DELETE, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
    FILE_RENAME_INFO, FILE_SHARE_READ, FILE_SHARE_WRITE, FileRenameInfo, READ_CONTROL,
    SetFileInformationByHandle,
};

use crate::windows_endpoint_access::{
    current_process_token, current_user_sid, sid_to_string, token_information, wide_to_string,
};

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
    // The owner is set to the current user explicitly: an elevated process's token would otherwise make the
    // Administrators group the owner, and [`PinnedDir::open`] accepts only the current user.
    let sddl = format!("O:{}{}", current_user_sid()?, owner_only_dir_sddl()?);
    let sddl = U16CString::from_str(sddl)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let descriptor = SecurityDescriptor::deserialize(&sddl)?;
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

/// Whether `path`'s DACL is protected and allows only acceptable owners ([`owner_is_acceptable`]: the token's user,
/// its default owner, SYSTEM), with no inherited and no deny entry: what [`create_dir_owner_only`] and
/// [`restrict_dir_to_owner`] set. Entries are compared as binary SIDs.
pub fn has_owner_only_dacl(path: &Path) -> io::Result<bool> {
    let (protected, entries) = dacl_entries(path)?;
    Ok(dacl_is_owner_only(protected, &entries, &accepted_owners()?))
}

/// One entry of a DACL, as far as the owner-only rule needs it.
struct DaclEntry {
    allowed: bool,
    inherited: bool,
    sid: Sid,
}

/// The rule behind [`has_owner_only_dacl`], on a DACL already read: protected, not empty, and every entry an
/// explicit allow for one of `accepted`.
fn dacl_is_owner_only(protected: bool, entries: &[DaclEntry], accepted: &[Sid]) -> bool {
    protected
        && !entries.is_empty()
        && entries
            .iter()
            .all(|entry| entry.allowed && !entry.inherited && owner_matches(&entry.sid, accepted))
}

/// `ACCESS_ALLOWED_ACE_TYPE` and the `INHERITED_ACE` flag of an ACE header.
const ACCESS_ALLOWED_ACE_TYPE: u8 = 0;
const INHERITED_ACE: u8 = 0x10;

/// Whether `path`'s DACL is protected, and its entries with their SIDs copied out.
fn dacl_entries(path: &Path) -> io::Result<(bool, Vec<DaclEntry>)> {
    let name = wide_path(path)?;
    let mut acl: *mut ACL = ptr::null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
    // SAFETY: `name` is NUL-terminated and lives through the call; the DACL and descriptor out-pointers are valid, and
    // the call allocates the descriptor with LocalAlloc, the DACL pointing into it.
    let status = unsafe {
        GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut acl,
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let result = read_dacl(descriptor, acl);
    // SAFETY: the descriptor was allocated by GetNamedSecurityInfoW; the entries were copied out of it.
    unsafe { LocalFree(descriptor) };
    result
}

/// Reads the protection flag and the entries of a DACL inside a live descriptor.
fn read_dacl(
    descriptor: PSECURITY_DESCRIPTOR,
    acl: *mut ACL,
) -> io::Result<(bool, Vec<DaclEntry>)> {
    let mut control = 0_u16;
    let mut revision = 0_u32;
    // SAFETY: `descriptor` is a live descriptor, and both out-pointers are valid for writes.
    if unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let protected = control & SE_DACL_PROTECTED != 0;
    if acl.is_null() {
        // A null DACL grants everyone everything.
        return Ok((protected, Vec::new()));
    }
    let mut info = ACL_SIZE_INFORMATION::default();
    // SAFETY: `acl` points into the live descriptor, and `info` is the size the call is told it has.
    if unsafe {
        GetAclInformation(
            acl,
            ptr::from_mut(&mut info).cast(),
            size_of::<ACL_SIZE_INFORMATION>() as u32,
            AclSizeInformation,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut entries = Vec::with_capacity(info.AceCount as usize);
    for index in 0..info.AceCount {
        let mut ace = ptr::null_mut();
        // SAFETY: `index` is below AceCount, so the ACE exists, and `ace` is a valid out-pointer.
        if unsafe { GetAce(acl, index, &mut ace) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: every ACE starts with its header.
        let header = unsafe { &*ace.cast::<ACE_HEADER>() };
        let allowed = header.AceType == ACCESS_ALLOWED_ACE_TYPE;
        let sid = if allowed {
            // SAFETY: an ACCESS_ALLOWED_ACE stores its SID at SidStart, inside the ACE, which is alive here.
            let allowed_ace = unsafe { &*ace.cast::<ACCESS_ALLOWED_ACE>() };
            unsafe { Sid::from_psid(ptr::from_ref(&allowed_ace.SidStart).cast_mut().cast())? }
        } else {
            // Any other entry type fails the rule; its SID is not needed.
            Sid::parse("S-1-0-0")?
        };
        entries.push(DaclEntry {
            allowed,
            inherited: header.AceFlags & INHERITED_ACE != 0,
            sid,
        });
    }
    Ok((protected, entries))
}

/// SYSTEM, which owns the objects services create.
const LOCAL_SYSTEM: &str = "S-1-5-18";

/// A SID held as its own bytes, compared with `EqualSid`.
#[derive(Clone)]
pub struct Sid(Vec<u8>);

impl Sid {
    /// Copies the SID `sid` points at.
    ///
    /// # Safety
    /// `sid` must point at a valid SID for the duration of the call.
    pub unsafe fn from_psid(sid: PSID) -> io::Result<Self> {
        // SAFETY: the caller guarantees `sid` is readable; IsValidSid only reads it.
        if sid.is_null() || unsafe { IsValidSid(sid) } == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "not a valid SID",
            ));
        }
        // SAFETY: `sid` is a valid SID, so its length can be read.
        let len = unsafe { GetLengthSid(sid) };
        let mut bytes = vec![0_u8; len as usize];
        // SAFETY: `bytes` holds `len` writable bytes, and `sid` is a valid SID of that length.
        if unsafe { CopySid(len, bytes.as_mut_ptr().cast(), sid) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(bytes))
    }

    /// Parses `S-1-...` text or a well-known SDDL alias (`SY`, `BA`, `LA`, ...).
    pub fn parse(text: &str) -> io::Result<Self> {
        let wide = U16CString::from_str(text)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        let mut sid: PSID = ptr::null_mut();
        // SAFETY: `wide` is NUL-terminated and lives through the call; `sid` receives a LocalAlloc'd SID.
        if unsafe { ConvertStringSidToSidW(wide.as_ptr(), &mut sid) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the call succeeded, so `sid` is a valid SID until it is freed below.
        let copied = unsafe { Self::from_psid(sid) };
        // SAFETY: `sid` was allocated by ConvertStringSidToSidW and is not used after this.
        unsafe { LocalFree(sid) };
        copied
    }

    /// A pointer to the SID for calls that only read it, valid while `self` lives.
    fn as_psid(&self) -> PSID {
        self.0.as_ptr().cast_mut().cast()
    }
}

impl PartialEq for Sid {
    fn eq(&self, other: &Self) -> bool {
        owner_matches(self, std::slice::from_ref(other))
    }
}

impl Eq for Sid {}

impl fmt::Display for Sid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match sid_to_string(self.as_psid()) {
            Ok(text) => f.write_str(&text),
            Err(_) => f.write_str("an unreadable SID"),
        }
    }
}

impl fmt::Debug for Sid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Sid({self})")
    }
}

/// This process's token user (TokenUser).
fn token_user() -> io::Result<Sid> {
    let token = current_process_token()?;
    let user = token_information(&token, TokenUser)?;
    // SAFETY: the buffer starts with a TOKEN_USER whose SID points into the same buffer, alive here.
    unsafe { Sid::from_psid((*user.as_ptr().cast::<TOKEN_USER>()).User.Sid) }
}

/// The SIDs a path's DACL grants, one per entry, in order. An entry that is not an allow is listed as the null SID
/// (`S-1-0-0`), which no owner check accepts.
pub fn dacl_grantees(path: &Path) -> io::Result<Vec<Sid>> {
    Ok(dacl_entries(path)?
        .1
        .into_iter()
        .map(|entry| entry.sid)
        .collect())
}

/// The error for a directory whose owner is not acceptable. One owned by Administrators was left by an elevated run;
/// the message says how to fix it.
fn owner_refusal(dir: &Path, owner: &Sid) -> io::Error {
    let administrators = Sid::parse(ADMINISTRATORS).ok();
    let message = if administrators.is_some_and(|administrators| *owner == administrators) {
        format!(
            "{}: owned by Administrators ({ADMINISTRATORS}) from an elevated run; run MassOrca elevated once, which \
             makes it yours, or take ownership of it",
            dir.display()
        )
    } else {
        format!(
            "{}: owned by {owner}, not by this user, its token's default owner or SYSTEM",
            dir.display()
        )
    };
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}

/// BUILTIN\Administrators, the default owner of an elevated administrator's new objects.
const ADMINISTRATORS: &str = "S-1-5-32-544";

/// The owners this process accepts: its token's user (TokenUser), its token's default owner (TokenOwner) and SYSTEM.
fn accepted_owners() -> io::Result<Vec<Sid>> {
    let token = current_process_token()?;
    let user = token_information(&token, TokenUser)?;
    // SAFETY: the buffer starts with a TOKEN_USER whose SID points into the same buffer, alive here.
    let user = unsafe { Sid::from_psid((*user.as_ptr().cast::<TOKEN_USER>()).User.Sid)? };
    let owner = token_information(&token, TokenOwner)?;
    // SAFETY: the buffer starts with a TOKEN_OWNER whose SID points into the same buffer, alive here.
    let owner = unsafe { Sid::from_psid((*owner.as_ptr().cast::<TOKEN_OWNER>()).Owner)? };
    Ok(vec![user, owner, Sid::parse(LOCAL_SYSTEM)?])
}

/// Whether `owner` is one of `accepted`, compared with `EqualSid`.
pub fn owner_matches(owner: &Sid, accepted: &[Sid]) -> bool {
    accepted.iter().any(|candidate| {
        // SAFETY: both SIDs are valid copies that live through the call, which only reads them.
        unsafe { EqualSid(owner.as_psid(), candidate.as_psid()) != 0 }
    })
}

/// Whether the SID `owner` points at is acceptable to this process: its token's user, its token's default owner, or
/// SYSTEM, compared with `EqualSid`. A null or invalid SID is an error, never a match.
///
/// `owner` must point at a SID that stays valid for the duration of the call; it is copied before it is compared.
#[expect(
    clippy::not_unsafe_ptr_arg_deref,
    reason = "the agreed signature takes a PSID; it is validated with IsValidSid and copied before any comparison"
)]
pub fn owner_is_acceptable(owner: PSID) -> io::Result<bool> {
    // SAFETY: the caller passes a SID that stays valid for the call; from_psid validates it before reading it.
    let owner = unsafe { Sid::from_psid(owner)? };
    Ok(owner_matches(&owner, &accepted_owners()?))
}

/// The owner of the object behind `handle`, read through it; the handle needs `READ_CONTROL` access.
fn handle_owner(handle: &File) -> io::Result<Sid> {
    let mut owner: PSID = ptr::null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
    // SAFETY: the handle is open for the call; only the owner and descriptor out-pointers are requested, and the
    // call allocates the descriptor with LocalAlloc.
    let status = unsafe {
        GetSecurityInfo(
            handle.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &mut owner,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    // SAFETY: `owner` points into `descriptor`, which is still allocated here.
    let result = unsafe { Sid::from_psid(owner) };
    // SAFETY: the descriptor was allocated by GetSecurityInfo, and nothing points into it after the copy.
    unsafe { LocalFree(descriptor) };
    result
}

/// Whether the directory open as `handle` (with `READ_CONTROL`, as [`PinnedDir`] opens it) has an acceptable owner.
pub fn dir_owner_is_acceptable(handle: &File) -> io::Result<bool> {
    Ok(owner_matches(&handle_owner(handle)?, &accepted_owners()?))
}

/// Replaces `dir`'s DACL with [`owner_only_dir_sddl`], cuts inheritance from its parent, and makes the token user its
/// owner (a process may always assign its own user SID): children that inherit get the owner-only entries, and a
/// directory an elevated run left owned by Administrators becomes the user's, so a non-elevated run accepts it too. A
/// directory made with [`create_dir_owner_only`] needs no call to this.
pub fn restrict_dir_to_owner(dir: &Path) -> io::Result<()> {
    let user = token_user()?;
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
    // SAFETY: `name` is a NUL-terminated path, `user` a valid SID and `acl` points into `descriptor`; all outlive the
    // call, which only reads them. Group and SACL are not set, so their null pointers are not read.
    let status = unsafe {
        SetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION
                | DACL_SECURITY_INFORMATION
                | PROTECTED_DACL_SECURITY_INFORMATION,
            user.as_psid(),
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
/// one. Its owner must be acceptable ([`dir_owner_is_acceptable`]): an owner keeps the right to rewrite the access
/// list, so a directory any other principal owns is refused. Only [`PinnedDir::rename_to`], through this handle, can
/// still move it.
#[derive(Debug)]
pub struct PinnedDir {
    file: File,
}

impl PinnedDir {
    /// Opens and pins `dir`. Fails with `InvalidInput` when it is a reparse point or not a directory, with
    /// `PermissionDenied` when its owner is not acceptable, and with a sharing violation when another handle
    /// already holds it without sharing delete.
    pub fn open(dir: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .access_mode(FILE_READ_ATTRIBUTES | DELETE | READ_CONTROL)
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
        let pinned = Self { file };
        let owner = pinned.owner_sid()?;
        if !owner_matches(&owner, &accepted_owners()?) {
            return Err(owner_refusal(dir, &owner));
        }
        Ok(pinned)
    }

    /// The pinned directory's owner, read through the handle.
    pub fn owner_sid(&self) -> io::Result<Sid> {
        handle_owner(&self.file)
    }

    /// Whether the token user itself owns the pinned directory (not only an acceptable owner).
    pub fn owned_by_token_user(&self) -> io::Result<bool> {
        Ok(owner_matches(&self.owner_sid()?, &[token_user()?]))
    }

    /// The SID of the pinned directory's owner, read through the handle.
    pub fn owner(&self) -> io::Result<String> {
        Ok(handle_owner(&self.file)?.to_string())
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
        DaclEntry, PinnedDir, Sid, create_dir_owner_only, dacl_entries, dacl_is_owner_only,
        dir_owner_is_acceptable, has_owner_only_dacl, owner_matches, owner_refusal,
        restrict_dir_to_owner,
    };

    fn sid(text: &str) -> Sid {
        Sid::parse(text).unwrap()
    }

    /// The accepted owners of an elevated administrator: its user, the Administrators group (its token's default
    /// owner) and SYSTEM, injected instead of read from a real elevated token.
    fn elevated(user: &str) -> Vec<Sid> {
        vec![sid(user), sid("S-1-5-32-544"), sid("S-1-5-18")]
    }

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

    /// 1. The token's user is an acceptable owner, as is whatever owns a directory this process makes with std.
    #[test]
    fn the_token_user_is_an_acceptable_owner() {
        use std::os::windows::fs::OpenOptionsExt;
        let user = sid(&current_user_sid().unwrap());
        assert!(owner_matches(&user, std::slice::from_ref(&user)));
        assert!(super::owner_is_acceptable(user.as_psid()).unwrap());
        assert!(!super::owner_is_acceptable(sid("S-1-5-21-9-9-9-1005").as_psid()).unwrap());
        let parent = tempfile::tempdir().unwrap();
        let dir = std::fs::OpenOptions::new()
            .access_mode(super::READ_CONTROL)
            .custom_flags(super::FILE_FLAG_BACKUP_SEMANTICS)
            .open(parent.path())
            .unwrap();
        assert!(dir_owner_is_acceptable(&dir).unwrap());
    }

    /// 2. An elevated administrator's new objects are owned by the Administrators group, its token's default owner.
    #[test]
    fn the_default_owner_of_an_elevated_token_is_accepted() {
        let accepted = elevated("S-1-5-21-1-2-3-1001");
        assert!(owner_matches(&sid("S-1-5-32-544"), &accepted));
        assert!(
            owner_matches(&sid("BA"), &accepted),
            "an alias is the same SID"
        );
        assert!(owner_matches(&sid("SY"), &accepted));
    }

    /// 3. The built-in Administrator (RID 500) is written `LA` in SDDL; compared as SIDs, both spellings match.
    #[test]
    fn the_built_in_administrator_is_compared_as_a_sid_not_as_its_alias() {
        let alias = sid("LA");
        let numeric = alias.to_string();
        assert!(
            numeric.starts_with("S-1-5-21-") && numeric.ends_with("-500"),
            "{numeric}"
        );
        assert!(owner_matches(&sid(&numeric), &elevated(&numeric)));
        assert!(owner_matches(&alias, &elevated(&numeric)));
        assert!(owner_matches(&sid(&numeric), std::slice::from_ref(&alias)));
    }

    /// 4. Any other principal is still refused.
    #[test]
    fn an_unrelated_principal_is_refused() {
        let accepted = elevated("S-1-5-21-1-2-3-1001");
        for other in [
            "S-1-5-21-1-2-3-1005",
            "S-1-5-21-9-9-9-1001",
            "S-1-1-0",
            "S-1-5-11",
            "S-1-5-32-545",
        ] {
            assert!(!owner_matches(&sid(other), &accepted), "{other}");
        }
        assert!(!owner_matches(
            &sid("S-1-5-32-544"),
            &[sid("S-1-5-21-1-2-3-1001")]
        ));
    }

    /// A directory made owner-only names the token user as owner, elevated or not; a plain one is owned by the
    /// token's default owner (Administrators when elevated), which this process accepts.
    #[test]
    fn a_directory_this_user_creates_is_owned_by_it_and_pinned() {
        let user = sid(&current_user_sid().unwrap());
        let parent = tempfile::tempdir().unwrap();
        let dir = parent.path().join("owned");
        create_dir_owner_only(&dir).unwrap();
        let pin = PinnedDir::open(&dir).unwrap();
        assert!(owner_matches(
            &pin.owner_sid().unwrap(),
            std::slice::from_ref(&user)
        ));
        let plain = parent.path().join("plain");
        std::fs::create_dir(&plain).unwrap();
        let pinned = PinnedDir::open(&plain).unwrap();
        assert!(dir_owner_is_acceptable(&pinned.file).unwrap());
    }

    /// A non-elevated token's accepted owners: its user twice (TokenUser and TokenOwner) and SYSTEM.
    fn not_elevated(user: &str) -> Vec<Sid> {
        vec![sid(user), sid(user), sid("S-1-5-18")]
    }

    #[test]
    fn an_elevated_token_accepts_a_user_owned_and_an_administrators_owned_directory() {
        let user = "S-1-5-21-1-2-3-1001";
        assert!(owner_matches(&sid(user), &elevated(user)));
        assert!(owner_matches(&sid("S-1-5-32-544"), &elevated(user)));
    }

    #[test]
    fn a_non_elevated_token_refuses_an_administrators_owned_directory() {
        let user = "S-1-5-21-1-2-3-1001";
        assert!(owner_matches(&sid(user), &not_elevated(user)));
        assert!(!owner_matches(&sid("S-1-5-32-544"), &not_elevated(user)));
    }

    #[test]
    fn a_refused_administrators_owner_says_how_to_fix_it() {
        let dir = std::path::Path::new("C:/profiles/default/agents");
        let error = owner_refusal(dir, &sid("S-1-5-32-544"));
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        let text = error.to_string();
        assert!(text.contains("Administrators"), "{text}");
        assert!(text.contains("elevated"), "{text}");
        assert!(text.contains("take ownership"), "{text}");
        let other = owner_refusal(dir, &sid("S-1-5-21-9-9-9-1005")).to_string();
        assert!(!other.contains("elevated"), "{other}");
    }

    #[test]
    fn a_user_and_system_dacl_is_accepted_by_both_kinds_of_token() {
        let user = "S-1-5-21-1-2-3-1001";
        let entries = [allow(user), allow("SY")];
        assert!(dacl_is_owner_only(true, &entries, &elevated(user)));
        assert!(dacl_is_owner_only(true, &entries, &not_elevated(user)));
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
        assert!(has_owner_only_dacl(&dir).unwrap());
        std::fs::write(dir.join("file"), b"fake").unwrap();
        let user_and_system = [sid(&current_user_sid().unwrap()), sid("SY")];
        let (_, file) = dacl_entries(&dir.join("file")).unwrap();
        assert!(!file.is_empty());
        for entry in &file {
            assert!(
                entry.allowed && owner_matches(&entry.sid, &user_and_system),
                "{:?}",
                entry.sid
            );
        }
        let again = create_dir_owner_only(&dir).unwrap_err();
        assert_eq!(again.kind(), std::io::ErrorKind::AlreadyExists);
        assert!(!has_owner_only_dacl(parent.path()).unwrap());
    }

    fn allow(text: &str) -> DaclEntry {
        DaclEntry {
            allowed: true,
            inherited: false,
            sid: sid(text),
        }
    }

    #[test]
    fn only_a_protected_dacl_for_accepted_owners_counts_as_owner_only() {
        let user = "S-1-5-21-1-2-3-1001";
        let accepted = elevated(user);
        assert!(dacl_is_owner_only(
            true,
            &[allow(user), allow("SY")],
            &accepted
        ));
        assert!(
            dacl_is_owner_only(true, &[allow("BA"), allow("SY")], &accepted),
            "an entry for the token's default owner is accepted"
        );
        let la = sid("LA").to_string();
        assert!(
            dacl_is_owner_only(true, &[allow("LA")], &elevated(&la)),
            "the alias and the numeric RID-500 SID are one principal"
        );
        assert!(
            !dacl_is_owner_only(false, &[allow(user)], &accepted),
            "not protected"
        );
        assert!(!dacl_is_owner_only(true, &[], &accepted), "empty");
        assert!(!dacl_is_owner_only(
            true,
            &[allow(user), allow("AU")],
            &accepted
        ));
        assert!(!dacl_is_owner_only(
            true,
            &[allow("S-1-5-21-9-9-9-1005")],
            &accepted
        ));
        let mut inherited = allow(user);
        inherited.inherited = true;
        assert!(!dacl_is_owner_only(true, &[inherited], &accepted));
        let mut denied = allow(user);
        denied.allowed = false;
        assert!(!dacl_is_owner_only(true, &[denied], &accepted));
    }

    /// After `restrict_dir_to_owner` a directory is owned by the token user (whatever the token's default owner) and
    /// allows only accepted owners; its new files inherit entries for the user and SYSTEM only. All compared as SIDs.
    #[test]
    fn a_restricted_directory_has_a_protected_dacl_for_the_owner_and_system_only() {
        let user = sid(&current_user_sid().unwrap());
        let parent = tempfile::tempdir().unwrap();
        let dir = parent.path().join("secret");
        std::fs::create_dir(&dir).unwrap();
        restrict_dir_to_owner(&dir).unwrap();
        std::fs::write(dir.join("file"), b"fake").unwrap();

        let owner = PinnedDir::open(&dir).unwrap().owner_sid().unwrap();
        assert!(
            owner_matches(&owner, std::slice::from_ref(&user)),
            "owner {owner}"
        );
        assert!(has_owner_only_dacl(&dir).unwrap());
        let user_and_system = [user.clone(), sid("SY")];
        let (protected, entries) = dacl_entries(&dir).unwrap();
        assert!(protected);
        assert!(
            entries
                .iter()
                .any(|entry| owner_matches(&entry.sid, std::slice::from_ref(&user)))
        );
        let (_, file) = dacl_entries(&dir.join("file")).unwrap();
        assert!(!file.is_empty());
        for entry in entries.iter().chain(&file) {
            assert!(entry.allowed, "{:?}", entry.sid);
            assert!(
                owner_matches(&entry.sid, &user_and_system),
                "{:?}",
                entry.sid
            );
        }
        assert!(
            file.iter()
                .any(|entry| owner_matches(&entry.sid, std::slice::from_ref(&user))),
            "the file inherits the user's entry"
        );
    }
}
