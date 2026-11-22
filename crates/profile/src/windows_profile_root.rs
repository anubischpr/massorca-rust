//! Windows access of the profile root (ADR 0073): created with a protected owner-only DACL, checked and repaired on start.
//! Descriptors are compared as SDDL text, so SIDs and access masks are spelled the same way on both sides.
//! Every unsafe call states, in its `SAFETY` comment, the invariant it relies on.
#![allow(unsafe_code)]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
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

use crate::root_access::{
    MAX_LISTED_ENTRIES, RootAccess, RootAccessError, create_private_dir as create_dir_and_parents,
};
use crate::windows_endpoint_access::current_user_sid;

/// How many levels below the root are checked for entries another user owns.
const MAX_SCAN_DEPTH: usize = 4;

/// Owners trusted below the root besides the current user: SYSTEM and the Administrators group.
const TRUSTED_OWNERS: [&str; 4] = ["SY", "BA", "S-1-5-18", "S-1-5-32-544"];

/// The DACL of a profile root: full access for `user`, inherited by every file and folder below, nothing inherited from above.
fn owner_only_dir_dacl_sddl(user: &str) -> String {
    format!("D:P(A;OICI;FA;;;{user})")
}

pub(crate) fn secure_root(dir: &Path) -> Result<RootAccess, RootAccessError> {
    secure_root_with(dir, &read_sddl)
}

/// `secure_root` with the descriptor reader passed in, so tests can report an owner no unprivileged test can set.
fn secure_root_with(
    dir: &Path,
    read: &dyn Fn(&Path) -> io::Result<String>,
) -> Result<RootAccess, RootAccessError> {
    if create_dir_and_parents(dir)? {
        return Ok(RootAccess::Created);
    }
    let user_sid = current_user_sid()?;
    let user = owner_sddl(&user_sid)?;
    let root_sddl = read(dir)?;
    let (owner, dacl) = split_sddl(&root_sddl);
    if owner != user {
        return Err(RootAccessError::ForeignEntries {
            paths: vec![dir.to_path_buf()],
        });
    }
    let foreign = foreign_entries(dir, &user, read)?;
    if !foreign.is_empty() {
        return Err(RootAccessError::ForeignEntries { paths: foreign });
    }
    let expected = canonical_dacl_sddl(&owner_only_dir_dacl_sddl(&user_sid))?;
    if is_owner_only(dacl, &expected) {
        return Ok(RootAccess::Private);
    }
    set_dacl(dir, &expected, true)?;
    Ok(RootAccess::Repaired)
}

/// Creates `dir` with the owner-only descriptor in one call, so it is never reachable by others; false when it existed.
pub(crate) fn create_private_dir(dir: &Path) -> io::Result<bool> {
    let user = current_user_sid()?;
    let descriptor = descriptor(&format!("O:{user}{}", owner_only_dir_dacl_sddl(&user)))?;
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

/// Up to `MAX_LISTED_ENTRIES` entries below `root` whose owner is not trusted or cannot be read. Links are not followed.
fn foreign_entries(
    root: &Path,
    user: &str,
    read: &dyn Fn(&Path) -> io::Result<String>,
) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let mut pending = vec![(root.to_path_buf(), 1)];
    while let Some((dir, depth)) = pending.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if dir == root => return Err(error),
            // A folder that cannot be listed may hide planted content.
            Err(_) => {
                found.push(dir);
                continue;
            }
        };
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let owner = read(&path).ok().map(|sddl| split_sddl(&sddl).0.to_owned());
            if !is_trusted_owner(owner.as_deref(), user) {
                found.push(path);
            } else if depth < MAX_SCAN_DEPTH && entry.file_type()?.is_dir() {
                pending.push((path, depth + 1));
            }
            if found.len() >= MAX_LISTED_ENTRIES {
                found.truncate(MAX_LISTED_ENTRIES);
                return Ok(found);
            }
        }
    }
    Ok(found)
}

/// Whether an entry's owner may stay: the current user, SYSTEM or Administrators. An unreadable owner is not trusted.
fn is_trusted_owner(owner: Option<&str>, user: &str) -> bool {
    owner.is_some_and(|owner| owner == user || TRUSTED_OWNERS.contains(&owner))
}

/// Whether `actual` is the protected owner-only DACL `expected`; the auto-inherited flags `AI` and `AR` are ignored.
fn is_owner_only(actual: &str, expected: &str) -> bool {
    let (actual_flags, actual_aces) = split_dacl(actual);
    let (_, expected_aces) = split_dacl(expected);
    actual_flags.contains('P') && actual_aces == expected_aces
}

/// The control flags and the ACEs of a `D:` SDDL section.
fn split_dacl(dacl: &str) -> (&str, &str) {
    let body = dacl.strip_prefix("D:").unwrap_or(dacl);
    body.split_at(body.find('(').unwrap_or(body.len()))
}

/// The owner and the DACL sections of an `O:…D:…` SDDL string.
fn split_sddl(sddl: &str) -> (&str, &str) {
    let owner_start = sddl.find("O:").map_or(0, |index| index + 2);
    let dacl_start = sddl.find("D:").unwrap_or(sddl.len());
    let owner = sddl
        .get(owner_start..dacl_start.max(owner_start))
        .unwrap_or("");
    (owner, &sddl[dacl_start..])
}

/// How SDDL spells `sid` as an owner, for example `SY` for SYSTEM.
fn owner_sddl(sid: &str) -> io::Result<String> {
    serialize(
        &descriptor(&format!("O:{sid}"))?,
        OWNER_SECURITY_INFORMATION,
    )
    .map(|text| text.trim_start_matches("O:").to_owned())
}

/// `dacl` as Windows writes it back, so it compares equal with a DACL read from disk.
fn canonical_dacl_sddl(dacl: &str) -> io::Result<String> {
    serialize(&descriptor(dacl)?, DACL_SECURITY_INFORMATION)
}

fn descriptor(sddl: &str) -> io::Result<SecurityDescriptor> {
    let text = U16CString::from_str(sddl)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    SecurityDescriptor::deserialize(&text)
}

fn serialize(descriptor: &SecurityDescriptor, selector: u32) -> io::Result<String> {
    descriptor.serialize(selector, U16CStr::to_string_lossy)
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

/// The owner and DACL of `path` as SDDL. A link is read itself, never its target.
fn read_sddl(path: &Path) -> io::Result<String> {
    let wide = wide_path(path)?;
    // SAFETY: `wide` is NUL-terminated and outlives the call; the result is checked against INVALID_HANDLE_VALUE.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            READ_CONTROL,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
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

fn wide_path(path: &Path) -> io::Result<U16CString> {
    U16CString::from_os_str(path)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};

    use super::{
        MAX_LISTED_ENTRIES, RootAccess, RootAccessError, canonical_dacl_sddl, current_user_sid,
        is_owner_only, is_trusted_owner, owner_only_dir_dacl_sddl, owner_sddl, read_sddl,
        secure_root, secure_root_with, set_dacl, split_dacl, split_sddl,
    };

    /// A user SID no account on the test machine has.
    const STRANGER: &str = "S-1-5-21-1-2-3-4242";

    fn user() -> String {
        owner_sddl(&current_user_sid().expect("the current user")).expect("the owner form")
    }

    fn dacl_of(path: &Path) -> String {
        split_sddl(&read_sddl(path).expect("the descriptor"))
            .1
            .to_owned()
    }

    /// A root that also grants Everyone full access, inherited by what is created in it.
    fn broad_root(home: &Path) -> PathBuf {
        let root = home.join("profiles").join("default");
        fs::create_dir_all(&root).expect("the root");
        let dacl = format!("D:(A;OICI;FA;;;WD)(A;OICI;FA;;;{})", user());
        set_dacl(&root, &dacl, false).expect("a broad DACL");
        root
    }

    /// Reads descriptors from disk, but reports `STRANGER` as the owner of `foreign`.
    fn reader_with_foreign(foreign: Vec<PathBuf>) -> impl Fn(&Path) -> io::Result<String> {
        move |path| {
            let sddl = read_sddl(path)?;
            if foreign.iter().any(|planted| planted == path) {
                let (_, dacl) = split_sddl(&sddl);
                return Ok(format!("O:{STRANGER}{dacl}"));
            }
            Ok(sddl)
        }
    }

    #[test]
    fn a_new_profile_root_has_a_protected_dacl_with_one_ace_for_the_current_user() {
        let home = tempfile::tempdir().expect("a home");
        let root = home.path().join("profiles").join("default");

        assert_eq!(secure_root(&root).expect("created"), RootAccess::Created);

        let sddl = read_sddl(&root).expect("the descriptor");
        let (owner, dacl) = split_sddl(&sddl);
        assert_eq!(owner, user());
        let (flags, aces) = split_dacl(dacl);
        assert!(flags.contains('P'), "the DACL is protected: {dacl}");
        assert_eq!(aces, format!("(A;OICI;FA;;;{})", user()));
        assert_eq!(secure_root(&root).expect("checked"), RootAccess::Private);
    }

    #[test]
    fn a_folder_created_later_inherits_only_the_owner_entry() {
        let home = tempfile::tempdir().expect("a home");
        let root = home.path().join("profiles").join("default");
        secure_root(&root).expect("created");
        let child = root.join("agents").join("kinds");
        fs::create_dir_all(&child).expect("a child folder");

        let dacl = dacl_of(&child);
        assert_eq!(split_dacl(&dacl).1, format!("(A;OICIID;FA;;;{})", user()));
    }

    #[test]
    fn a_root_with_inherited_broader_access_is_repaired_with_everything_below_it() {
        let home = tempfile::tempdir().expect("a home");
        let root = broad_root(home.path());
        let kinds = root.join("agents").join("kinds");
        fs::create_dir_all(&kinds).expect("a child folder");
        fs::write(kinds.join("tool.json"), b"{}").expect("a child file");
        assert!(dacl_of(&kinds.join("tool.json")).contains(";;;WD)"));

        assert_eq!(secure_root(&root).expect("repaired"), RootAccess::Repaired);

        let expected = canonical_dacl_sddl(&owner_only_dir_dacl_sddl(
            &current_user_sid().expect("the current user"),
        ))
        .expect("the expected DACL");
        assert!(
            is_owner_only(&dacl_of(&root), &expected),
            "{}",
            dacl_of(&root)
        );
        for below in [kinds.clone(), kinds.join("tool.json")] {
            let dacl = dacl_of(&below);
            assert!(
                !dacl.contains(";;;WD)"),
                "{} keeps Everyone: {dacl}",
                below.display()
            );
        }
        assert_eq!(secure_root(&root).expect("checked"), RootAccess::Private);
    }

    #[test]
    fn an_entry_owned_by_another_user_is_refused_before_anything_is_repaired() {
        let home = tempfile::tempdir().expect("a home");
        let root = broad_root(home.path());
        let kinds = root.join("agents").join("kinds");
        fs::create_dir_all(&kinds).expect("a child folder");
        let planted = kinds.join("planted.json");
        fs::write(&planted, b"{}").expect("a planted file");

        let refused = secure_root_with(&root, &reader_with_foreign(vec![planted.clone()]));

        match refused {
            Err(RootAccessError::ForeignEntries { paths }) => assert_eq!(paths, vec![planted]),
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert!(dacl_of(&root).contains(";;;WD)"), "nothing was repaired");
    }

    #[test]
    fn a_root_owned_by_another_user_is_refused() {
        let home = tempfile::tempdir().expect("a home");
        let root = broad_root(home.path());

        let refused = secure_root_with(&root, &reader_with_foreign(vec![root.clone()]));

        assert!(
            matches!(&refused, Err(RootAccessError::ForeignEntries { paths }) if paths == &vec![root.clone()]),
            "{refused:?}"
        );
    }

    #[test]
    fn a_refusal_names_at_most_ten_entries() {
        let home = tempfile::tempdir().expect("a home");
        let root = broad_root(home.path());
        let planted: Vec<PathBuf> = (0..12)
            .map(|index| root.join(format!("{index}.json")))
            .collect();
        for path in &planted {
            fs::write(path, b"{}").expect("a planted file");
        }

        let refused = secure_root_with(&root, &reader_with_foreign(planted.clone()));

        match refused {
            Err(RootAccessError::ForeignEntries { paths }) => {
                assert_eq!(paths.len(), MAX_LISTED_ENTRIES);
                assert!(paths.iter().all(|path| planted.contains(path)));
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn system_administrators_and_the_user_are_trusted_owners_and_nobody_else() {
        let user = user();
        for owner in ["SY", "BA", "S-1-5-18", "S-1-5-32-544", user.as_str()] {
            assert!(is_trusted_owner(Some(owner), &user), "{owner}");
        }
        assert!(!is_trusted_owner(Some(STRANGER), &user));
        assert!(!is_trusted_owner(Some("WD"), &user));
        assert!(
            !is_trusted_owner(None, &user),
            "an unreadable owner is not trusted"
        );
    }

    #[test]
    fn an_owner_only_dacl_must_be_protected_and_hold_only_the_owner_entry() {
        let expected = "D:P(A;OICI;FA;;;S-1-5-21-9)";
        assert!(is_owner_only("D:PAI(A;OICI;FA;;;S-1-5-21-9)", expected));
        assert!(!is_owner_only("D:AI(A;OICI;FA;;;S-1-5-21-9)", expected));
        assert!(!is_owner_only(
            "D:P(A;OICI;FA;;;S-1-5-21-9)(A;OICI;FA;;;WD)",
            expected
        ));
        assert!(!is_owner_only("D:NO_ACCESS_CONTROL", expected));
    }
}
