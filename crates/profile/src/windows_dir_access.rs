//! Whether a folder's owner is one this process accepts (ADR 0073): the token's user, the token's default owner, or
//! SYSTEM. An elevated administrator token's default owner is Administrators, so what it creates is owned by
//! Administrators, not by the user. SIDs are compared with `EqualSid`, never as text or names.
//! Every unsafe call states, in its `SAFETY` comment, the invariant it relies on.
#![allow(unsafe_code)]

use std::io;
use std::ptr;

use windows_sys::Win32::Foundation::{ERROR_SUCCESS, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::{GetSecurityInfo, SE_FILE_OBJECT};
use windows_sys::Win32::Security::{
    IsValidSid, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
};

use crate::windows_sid::Sid;

/// Whether `owner` is the token's user, the token's default owner, or SYSTEM. A null or invalid SID is an error.
///
/// # Safety
///
/// `owner` must be null or point at memory readable as a SID for the duration of the call.
pub unsafe fn owner_is_acceptable(owner: PSID) -> io::Result<bool> {
    // SAFETY: the caller guarantees `owner` is readable as a SID; IsValidSid only reads it.
    if owner.is_null() || unsafe { IsValidSid(owner) } == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a valid SID",
        ));
    }
    Ok(owner_matches(&Sid::copy(owner), &accepted_owners()?))
}

/// Whether `owner` equals one of `accepted`, compared as SIDs.
pub fn owner_matches(owner: &Sid, accepted: &[Sid]) -> bool {
    accepted.contains(owner)
}

/// Reads the owner of the file or folder `handle` refers to and applies `owner_is_acceptable`. The handle needs
/// `READ_CONTROL` access.
///
/// # Safety
///
/// `handle` must be a handle this process owns, open for the duration of the call.
pub unsafe fn dir_owner_is_acceptable(handle: HANDLE) -> io::Result<bool> {
    let mut owner: PSID = ptr::null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
    // SAFETY: the call reports a bad handle as an error; `owner` points into `descriptor`, a LocalAlloc'd copy freed
    // below after the owner is copied out.
    let status = unsafe {
        GetSecurityInfo(
            handle,
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
    // SAFETY: on success `owner` points at the owner SID inside `descriptor`, which is freed only after this call.
    let accepted = unsafe { owner_is_acceptable(owner) };
    // SAFETY: GetSecurityInfo allocated `descriptor` with LocalAlloc, so LocalFree releases it exactly once.
    unsafe { LocalFree(descriptor) };
    accepted
}

/// The owners this process's token accepts.
pub(crate) fn accepted_owners() -> io::Result<Vec<Sid>> {
    accepted_owners_for(Sid::current_user()?, Sid::token_owner()?)
}

/// The owners accepted for a token with this user and default owner.
pub(crate) fn accepted_owners_for(user: Sid, token_owner: Sid) -> io::Result<Vec<Sid>> {
    Ok(vec![user, token_owner, Sid::local_system()?])
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::os::windows::io::AsRawHandle;

    use super::{accepted_owners, accepted_owners_for, dir_owner_is_acceptable, owner_matches};
    use crate::windows_sid::Sid;

    fn sid(text: &str) -> Sid {
        Sid::parse(text).expect("a SID")
    }

    #[test]
    fn the_token_user_its_default_owner_and_system_are_accepted() {
        let accepted = accepted_owners().expect("this token's owners");
        assert!(owner_matches(&Sid::current_user().unwrap(), &accepted));
        assert!(owner_matches(&Sid::token_owner().unwrap(), &accepted));
        assert!(owner_matches(&sid("SY"), &accepted));
    }

    #[test]
    fn administrators_is_accepted_only_as_an_elevated_tokens_default_owner() {
        let user = Sid::current_user().unwrap();
        let elevated = accepted_owners_for(user.clone(), sid("BA")).unwrap();
        assert!(owner_matches(&sid("S-1-5-32-544"), &elevated));

        let ordinary = accepted_owners_for(user.clone(), user).unwrap();
        assert!(!owner_matches(&sid("BA"), &ordinary));
    }

    #[test]
    fn an_aliased_account_is_accepted_whichever_way_it_is_spelled() {
        let administrator = sid("LA");
        let accepted = accepted_owners_for(administrator.clone(), administrator.clone()).unwrap();
        let spelled = sid(&administrator.to_sid_string().unwrap());
        assert!(owner_matches(&spelled, &accepted));
    }

    #[test]
    fn a_foreign_sid_is_refused() {
        let elevated = accepted_owners_for(Sid::current_user().unwrap(), sid("BA")).unwrap();
        for foreign in ["WD", "AU", "BU", "S-1-5-21-1-2-3-4242"] {
            assert!(!owner_matches(&sid(foreign), &elevated), "{foreign}");
        }
    }

    #[test]
    fn a_file_this_process_created_has_an_acceptable_owner() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let file = File::create(temp.path().join("owned")).expect("a file");
        // SAFETY: `file` owns the handle and stays open through the call.
        let acceptable = unsafe { dir_owner_is_acceptable(file.as_raw_handle()) };
        assert!(acceptable.expect("its owner"));
    }
}
