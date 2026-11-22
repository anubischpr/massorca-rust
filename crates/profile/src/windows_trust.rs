//! Who a profile trusts on Windows (ADR 0073): owners as `owner_matches` accepts them (the token's user, the token's
//! default owner, SYSTEM), and, as holders of write access in a DACL, those owners plus Administrators, which Windows
//! grants full access on the user's own folders by default. SIDs are compared as SIDs, never as text.
//! Every unsafe call states, in its `SAFETY` comment, the invariant it relies on.
#![allow(unsafe_code)]

use std::io;

use windows_sys::Win32::Security::{TOKEN_OWNER, TOKEN_USER, TokenOwner, TokenUser};

use crate::windows_dir_access::{Sid, owner_matches};
use crate::windows_endpoint_access::{current_process_token, token_information};

const LOCAL_SYSTEM: &str = "S-1-5-18";
const ADMINISTRATORS: &str = "S-1-5-32-544";

/// The user this process's token runs as (TokenUser).
pub(crate) fn current_user() -> io::Result<Sid> {
    let buffer = token_information(&current_process_token()?, TokenUser)?;
    // SAFETY: the buffer starts with a TOKEN_USER whose SID points into the same buffer, alive here.
    unsafe { Sid::from_psid((*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid) }
}

/// The default owner of this process's token (TokenOwner): Administrators on an elevated administrator token.
pub(crate) fn token_owner() -> io::Result<Sid> {
    let buffer = token_information(&current_process_token()?, TokenOwner)?;
    // SAFETY: the buffer starts with a TOKEN_OWNER whose SID points into the same buffer, alive here.
    unsafe { Sid::from_psid((*buffer.as_ptr().cast::<TOKEN_OWNER>()).Owner) }
}

#[derive(Debug, Clone)]
pub(crate) struct Trusted {
    user: Sid,
    accepted: Vec<Sid>,
    administrators: Sid,
}

impl Trusted {
    /// For this process's token.
    pub(crate) fn current() -> io::Result<Self> {
        Self::for_token(current_user()?, token_owner()?)
    }

    /// For a token with this user and default owner; tests pass an elevated or aliased token here.
    pub(crate) fn for_token(user: Sid, token_owner: Sid) -> io::Result<Self> {
        Ok(Self {
            accepted: vec![user.clone(), token_owner, Sid::parse(LOCAL_SYSTEM)?],
            user,
            administrators: Sid::parse(ADMINISTRATORS)?,
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

#[cfg(test)]
mod tests {
    use super::{Trusted, current_user, token_owner};
    use crate::windows_dir_access::Sid;

    fn sid(text: &str) -> Sid {
        Sid::parse(text).expect("a SID")
    }

    #[test]
    fn the_token_user_its_default_owner_and_system_are_owners() {
        let trusted = Trusted::current().expect("this token");
        assert!(trusted.owns(&current_user().unwrap()));
        assert!(trusted.owns(&token_owner().unwrap()));
        assert!(trusted.owns(&sid("SY")));
    }

    #[test]
    fn administrators_owns_only_for_an_elevated_token_but_may_always_write() {
        let user = current_user().unwrap();
        let elevated = Trusted::for_token(user.clone(), sid("BA")).unwrap();
        assert!(elevated.owns(&sid("S-1-5-32-544")));

        let ordinary = Trusted::for_token(user.clone(), user).unwrap();
        assert!(!ordinary.owns(&sid("BA")));
        assert!(ordinary.may_write(&sid("BA")));
    }

    #[test]
    fn an_aliased_account_is_trusted_whichever_way_it_is_spelled() {
        let administrator = sid("LA");
        let trusted = Trusted::for_token(administrator.clone(), administrator.clone()).unwrap();
        assert!(trusted.owns(&sid(&administrator.to_string())));
    }

    #[test]
    fn a_foreign_sid_is_neither_owner_nor_writer() {
        let trusted = Trusted::for_token(current_user().unwrap(), sid("BA")).unwrap();
        for other in ["WD", "AU", "BU", "S-1-5-21-1-2-3-4242"] {
            assert!(!trusted.owns(&sid(other)), "{other}");
            assert!(!trusted.may_write(&sid(other)), "{other}");
        }
    }
}
