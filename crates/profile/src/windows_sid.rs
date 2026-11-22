//! Security identifiers compared as SIDs, never as text: SDDL writes some accounts as aliases (`SY`, `BA`, `LA` for the
//! built-in Administrator), so two spellings can name one SID (ADR 0073).
//! Every unsafe call states, in its `SAFETY` comment, the invariant it relies on.
#![allow(unsafe_code)]

use std::fmt;
use std::io;
use std::ptr;

use widestring::{U16CStr, U16CString};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::{ConvertSidToStringSidW, ConvertStringSidToSidW};
use windows_sys::Win32::Security::{
    CreateWellKnownSid, EqualSid, GetLengthSid, GetTokenInformation, PSID, SECURITY_MAX_SID_SIZE,
    TOKEN_OWNER, TOKEN_QUERY, TokenOwner, WinBuiltinAdministratorsSid, WinLocalSystemSid,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use crate::windows_dir_access::{accepted_owners_for, owner_matches};
use crate::windows_endpoint_access::current_user_sid;

/// A security identifier in its binary form, compared with `EqualSid`. `u32` words keep it aligned for the SID's sub-authorities.
#[derive(Clone)]
pub struct Sid(Vec<u32>);

impl Sid {
    /// Parses a SID string or an SDDL alias such as `SY`, `BA` or `LA`.
    pub fn parse(text: &str) -> io::Result<Self> {
        let wide = U16CString::from_str(text)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        let mut sid: PSID = ptr::null_mut();
        // SAFETY: `wide` is NUL-terminated and outlives the call; `sid` receives a LocalAlloc'd SID freed below.
        if unsafe { ConvertStringSidToSidW(wide.as_ptr(), &mut sid) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let copied = Self::copy(sid);
        // SAFETY: the SID was allocated by ConvertStringSidToSidW, so LocalFree releases it exactly once.
        unsafe { LocalFree(sid) };
        Ok(copied)
    }

    /// The user this process runs as.
    pub fn current_user() -> io::Result<Self> {
        Self::parse(&current_user_sid()?)
    }

    /// The default owner of this process's token: Administrators on an elevated administrator token, else the user.
    pub fn token_owner() -> io::Result<Self> {
        token_owner_sid()
    }

    pub fn local_system() -> io::Result<Self> {
        Self::well_known(WinLocalSystemSid)
    }

    pub fn administrators() -> io::Result<Self> {
        Self::well_known(WinBuiltinAdministratorsSid)
    }

    fn well_known(kind: i32) -> io::Result<Self> {
        let mut words = vec![0_u32; SECURITY_MAX_SID_SIZE.div_ceil(4) as usize];
        let mut size = SECURITY_MAX_SID_SIZE;
        // SAFETY: the buffer holds `size` bytes and is aligned for a SID; machine-independent SIDs need no domain.
        if unsafe {
            CreateWellKnownSid(kind, ptr::null_mut(), words.as_mut_ptr().cast(), &mut size)
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        words.truncate((size as usize).div_ceil(4));
        Ok(Self(words))
    }

    /// Copies a valid SID out of memory the caller frees.
    pub(crate) fn copy(sid: PSID) -> Self {
        // SAFETY: `sid` is a valid SID, so GetLengthSid returns its size in bytes.
        let len = unsafe { GetLengthSid(sid) } as usize;
        let mut words = vec![0_u32; len.div_ceil(4)];
        // SAFETY: `sid` is readable for `len` bytes and `words` holds at least `len` bytes; the regions do not overlap.
        unsafe { ptr::copy_nonoverlapping(sid.cast::<u8>(), words.as_mut_ptr().cast::<u8>(), len) };
        Self(words)
    }

    fn as_psid(&self) -> PSID {
        self.0.as_ptr().cast_mut().cast()
    }

    /// The `S-1-…` form, never an alias, so a descriptor built from it names exactly this SID.
    pub fn to_sid_string(&self) -> io::Result<String> {
        let mut text: *mut u16 = ptr::null_mut();
        // SAFETY: the SID is valid and lives through the call; `text` receives a LocalAlloc'd string freed below.
        if unsafe { ConvertSidToStringSidW(self.as_psid(), &mut text) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: on success `text` is NUL-terminated and valid until it is freed below.
        let value = unsafe { U16CStr::from_ptr_str(text) }.to_string_lossy();
        // SAFETY: the string was allocated with LocalAlloc by the conversion, so LocalFree releases it exactly once.
        unsafe { LocalFree(text.cast()) };
        Ok(value)
    }
}

impl PartialEq for Sid {
    fn eq(&self, other: &Self) -> bool {
        // SAFETY: both buffers hold valid SIDs for the duration of the call, which only reads them.
        unsafe { EqualSid(self.as_psid(), other.as_psid()) != 0 }
    }
}

impl Eq for Sid {}

impl fmt::Debug for Sid {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_sid_string() {
            Ok(text) => formatter.write_str(&text),
            Err(_) => formatter.write_str("<invalid SID>"),
        }
    }
}

/// Who a profile trusts (ADR 0073): owners that `owner_matches` accepts, and, as holders of write access in a DACL,
/// those owners plus Administrators, which Windows grants full access on the user's own folders by default.
#[derive(Debug, Clone)]
pub(crate) struct Trusted {
    user: Sid,
    accepted: Vec<Sid>,
    administrators: Sid,
}

impl Trusted {
    /// For this process's token.
    pub(crate) fn current() -> io::Result<Self> {
        Self::for_token(Sid::current_user()?, Sid::token_owner()?)
    }

    /// For a token with this user and default owner; tests pass an elevated or aliased token here.
    pub(crate) fn for_token(user: Sid, token_owner: Sid) -> io::Result<Self> {
        Ok(Self {
            accepted: accepted_owners_for(user.clone(), token_owner)?,
            user,
            administrators: Sid::administrators()?,
        })
    }

    /// The token's user, whom a private folder grants access to.
    pub(crate) fn user(&self) -> &Sid {
        &self.user
    }

    pub(crate) fn owns(&self, owner: &Sid) -> bool {
        owner_matches(owner, &self.accepted)
    }

    pub(crate) fn may_write(&self, trustee: &Sid) -> bool {
        self.owns(trustee) || trustee == &self.administrators
    }
}

/// A token handle closed on drop.
struct Token(HANDLE);

impl Drop for Token {
    fn drop(&mut self) {
        // SAFETY: the handle came from OpenProcessToken and is closed only here, once.
        unsafe { CloseHandle(self.0) };
    }
}

/// The default owner SID of this process's token.
fn token_owner_sid() -> io::Result<Sid> {
    let mut handle: HANDLE = ptr::null_mut();
    // SAFETY: GetCurrentProcess returns a pseudo-handle valid for this call, and `handle` is a valid out-pointer.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = Token(handle);
    let mut needed = 0_u32;
    // SAFETY: a null buffer of length 0 only asks for the size, which the call writes to `needed`.
    unsafe { GetTokenInformation(token.0, TokenOwner, ptr::null_mut(), 0, &mut needed) };
    let words = (needed as usize).div_ceil(size_of::<usize>()).max(1);
    // A usize buffer keeps the TOKEN_OWNER aligned for its pointer field.
    let mut buffer = vec![0_usize; words];
    // SAFETY: the buffer holds `needed` bytes and is aligned for TOKEN_OWNER; the call fills it with the default owner.
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenOwner,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: on success the buffer starts with a TOKEN_OWNER whose SID points into the same buffer, alive here.
    let owner = unsafe { (*buffer.as_ptr().cast::<TOKEN_OWNER>()).Owner };
    Ok(Sid::copy(owner))
}

#[cfg(test)]
mod tests {
    use super::{Sid, Trusted};

    #[test]
    fn aliases_and_sid_strings_name_the_same_sid() {
        assert_eq!(Sid::parse("SY").unwrap(), Sid::parse("S-1-5-18").unwrap());
        assert_eq!(
            Sid::parse("BA").unwrap(),
            Sid::parse("S-1-5-32-544").unwrap()
        );
        assert_eq!(Sid::parse("SY").unwrap(), Sid::local_system().unwrap());
        assert_eq!(Sid::parse("BA").unwrap(), Sid::administrators().unwrap());
        assert_ne!(Sid::parse("SY").unwrap(), Sid::parse("BA").unwrap());
    }

    #[test]
    fn the_built_in_administrator_is_one_sid_as_alias_or_string() {
        let administrator = Sid::parse("LA").expect("the built-in Administrator");
        let text = administrator.to_sid_string().expect("its SID string");
        assert!(
            text.starts_with("S-1-5-21-") && text.ends_with("-500"),
            "{text}"
        );
        assert_eq!(Sid::parse(&text).unwrap(), administrator);
    }

    #[test]
    fn administrators_may_hold_write_access_but_owns_nothing_for_an_ordinary_token() {
        let user = Sid::current_user().unwrap();
        let trusted = Trusted::for_token(user.clone(), user).unwrap();
        assert!(trusted.may_write(&Sid::parse("BA").unwrap()));
        assert!(!trusted.owns(&Sid::parse("BA").unwrap()));
        assert!(!trusted.may_write(&Sid::parse("S-1-5-21-1-2-3-4242").unwrap()));
    }
}
