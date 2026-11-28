//! An account's credential, lent to a closure for one vendor request (decision 0176).
//!
//! [`Agents::with_account_credential`] reads the credential file of one account and lends its contents to a closure as a
//! [`Secret`]; it returns only what the closure returns. It runs only while the one global switch `usage.enabled` is on
//! ([`UsageGate`], read at each call): off, it refuses before touching the store or the disk. The file is reached
//! through the same checks as showing credentials and launching ([`crate::credentials`]), and only for a kind with an
//! official vendor origin ([`crate::vendor_endpoint`]); the caller checks every URL it sends the secret to with
//! [`crate::vendor_endpoint::check_url`].
//!
//! The contents are never logged, never put in an argument list or a child's environment, and never put in a protocol
//! type: [`Secret`] has no `Display`, `Serialize` or `Clone`, its `Debug` is `[redacted]`, and its bytes are zeroed when
//! it is dropped.

use std::fmt;
use std::sync::Arc;

use zeroize::{Zeroize, Zeroizing};

use crate::credentials;
use crate::sessions::{AgentError, Agents};
use crate::vendor_endpoint;

/// The settings key of the global switch, owned by `core`. Off (false) by default; there is no per-account opt-in.
pub const USAGE_ENABLED_SETTING: &str = "usage.enabled";

/// Whether account credentials may be read now: the global `usage.enabled` switch. It is asked at every call, never
/// cached, so turning the switch off takes effect for the next call. [`UsageGate::default`] is off.
#[derive(Clone)]
pub struct UsageGate(Arc<dyn Fn() -> bool + Send + Sync>);

impl UsageGate {
    /// A gate that asks `is_on` at every call. It should answer false when the switch cannot be read.
    pub fn new(is_on: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        Self(Arc::new(is_on))
    }

    /// A gate that is always off.
    pub fn off() -> Self {
        Self::new(|| false)
    }

    /// Whether the switch is on now.
    pub fn is_on(&self) -> bool {
        (self.0)()
    }
}

impl Default for UsageGate {
    fn default() -> Self {
        Self::off()
    }
}

impl fmt::Debug for UsageGate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UsageGate")
    }
}

/// The contents of an account's credential file, lent to a closure. Zeroed when dropped; `Debug` prints `[redacted]`;
/// no `Display`, `Serialize` or `Clone`, so it cannot be formatted, sent or kept by accident.
///
/// ```compile_fail
/// use massorca_core_agents::account_credential::Secret;
/// fn copy(secret: &Secret) -> Secret {
///     Secret::clone(secret)
/// }
/// ```
///
/// ```compile_fail
/// fn show(secret: &massorca_core_agents::account_credential::Secret) -> String {
///     format!("{secret}")
/// }
/// ```
///
/// ```compile_fail
/// fn send(secret: &massorca_core_agents::account_credential::Secret) -> String {
///     serde_json::to_string(secret).unwrap()
/// }
/// ```
pub struct Secret(Zeroizing<String>);

impl Secret {
    /// The credential file's text. Use it only to build the request to the kind's official origin.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

/// Why an account's credential was not lent.
#[derive(Debug, thiserror::Error)]
pub enum CredentialAccessError {
    /// `usage.enabled` is off; nothing was read.
    #[error("{USAGE_ENABLED_SETTING} is off, so account credentials are never read")]
    UsageDisabled,
    /// The kind has no official vendor origin, so its credential has nowhere it may go; nothing was read.
    #[error("agent kind {kind} has no official vendor endpoint, so its credential is never read")]
    NoVendorEndpoint { kind: String },
    /// The kind, the account or its files were refused, with the code showing credentials would give
    /// (`agent.kind_not_found`, `agent.account_not_found`, `agent.credentials_unsupported`,
    /// `agent.credentials_invalid`...). The message never holds anything read from a file.
    #[error("{}", .0.message)]
    Refused(AgentError),
}

impl From<AgentError> for CredentialAccessError {
    fn from(error: AgentError) -> Self {
        Self::Refused(error)
    }
}

impl Agents {
    /// Lends the credential of `account` of `kind` to `use_secret` and returns what it returns (decision 0176).
    ///
    /// Refused, in this order: while `usage.enabled` is off (before anything is read); for an unknown or invalid kind;
    /// for a kind with no official vendor origin; for an account that is not one of `kind`'s; for a link anywhere from
    /// `<profile>/agents` to the credential file, or a file that is missing, not a regular file, over 1 MiB or not
    /// UTF-8. The homes lock is held only while the file is read, never while `use_secret` runs.
    ///
    /// Blocking: it reads the store and the disk.
    pub fn with_account_credential<T>(
        &self,
        kind: &str,
        account: &str,
        use_secret: impl FnOnce(&Secret) -> T,
    ) -> Result<T, CredentialAccessError> {
        if !self.usage_gate().is_on() {
            return Err(CredentialAccessError::UsageDisabled);
        }
        credentials::declared_kind(self.profile_dir(), self.kind_packages(), kind)?;
        if vendor_endpoint::official_origin(kind).is_none() {
            return Err(CredentialAccessError::NoVendorEndpoint {
                kind: kind.to_owned(),
            });
        }
        self.account_of(kind, account)?;
        let mut bytes = {
            let _gate = credentials::lock_homes();
            credentials::read_account_credential(
                self.profile_dir(),
                self.kind_packages(),
                kind,
                account,
            )?
        };
        let secret = match String::from_utf8(std::mem::take(&mut *bytes)) {
            Ok(text) => Secret(Zeroizing::new(text)),
            Err(error) => {
                error.into_bytes().zeroize();
                return Err(CredentialAccessError::Refused(credentials::not_text(
                    kind, account,
                )));
            }
        };
        Ok(use_secret(&secret))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use massorca_core_store::{AgentAccountRow, now_ms};
    use massorca_protocol::codes;

    use super::*;
    use crate::paths;
    use crate::sessions::tests::service_gated;

    const ACCOUNT: &str = "0195c0de-0000-7000-8000-0000000000a1";
    const FAKE_SECRET: &str = r#"{"fakeOauth":{"accessToken":"fake-token-for-tests"}}"#;

    /// A service whose switch follows the returned flag (off at first), with one account of `claude` whose fake
    /// credential file is in a temporary profile.
    fn fixture() -> (Agents, Arc<AtomicBool>, tempfile::TempDir) {
        let switch = Arc::new(AtomicBool::new(false));
        let read = Arc::clone(&switch);
        let (agents, _workspace, profile) =
            service_gated(UsageGate::new(move || read.load(Ordering::SeqCst)));
        add_account(&agents, "claude", ACCOUNT);
        let home = paths::account_home(profile.path(), "claude", ACCOUNT);
        fs::create_dir_all(&home).unwrap();
        fs::write(home.join(".credentials.json"), FAKE_SECRET).unwrap();
        (agents, switch, profile)
    }

    fn add_account(agents: &Agents, kind: &str, account: &str) {
        agents
            .store()
            .insert_agent_account(&AgentAccountRow {
                id: account.to_owned(),
                kind: kind.to_owned(),
                label: "account".to_owned(),
                created_at: now_ms(),
                identity: None,
                state: "signed_in".to_owned(),
                origin: "import".to_owned(),
            })
            .unwrap();
    }

    fn refused_code<T: fmt::Debug>(result: Result<T, CredentialAccessError>) -> String {
        match result {
            Err(CredentialAccessError::Refused(error)) => error.code.to_string(),
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn the_gate_is_off_by_default() {
        assert!(!UsageGate::default().is_on());
        assert!(!UsageGate::off().is_on());
        assert!(UsageGate::new(|| true).is_on());
        assert_eq!(format!("{:?}", UsageGate::default()), "UsageGate");
    }

    #[test]
    fn with_the_switch_off_it_is_refused_and_the_home_is_untouched() {
        let (agents, _switch, profile) = fixture();
        // Nothing on disk would be readable anyway: the agents directory is gone, so a read would fail differently.
        fs::remove_dir_all(paths::agents_dir(profile.path())).unwrap();
        let mut called = false;
        let result = agents.with_account_credential("claude", ACCOUNT, |_| called = true);
        assert!(matches!(result, Err(CredentialAccessError::UsageDisabled)));
        assert!(!called);
        assert!(
            !paths::agents_dir(profile.path()).exists(),
            "nothing was created under the profile"
        );
        // Off refuses even an unknown kind before looking it up.
        assert!(matches!(
            agents.with_account_credential("nope", ACCOUNT, |_| ()),
            Err(CredentialAccessError::UsageDisabled)
        ));
    }

    #[test]
    fn the_switch_is_read_at_each_call() {
        let (agents, switch, _profile) = fixture();
        switch.store(true, Ordering::SeqCst);
        assert!(
            agents
                .with_account_credential("claude", ACCOUNT, |_| ())
                .is_ok()
        );
        switch.store(false, Ordering::SeqCst);
        assert!(matches!(
            agents.with_account_credential("claude", ACCOUNT, |_| ()),
            Err(CredentialAccessError::UsageDisabled)
        ));
    }

    #[test]
    fn with_the_switch_on_the_closure_sees_the_fake_secret_and_its_value_is_returned() {
        let (agents, switch, _profile) = fixture();
        switch.store(true, Ordering::SeqCst);
        let length = agents
            .with_account_credential("claude", ACCOUNT, |secret| {
                assert_eq!(secret.expose(), FAKE_SECRET);
                secret.expose().len()
            })
            .unwrap();
        assert_eq!(length, FAKE_SECRET.len());
    }

    #[test]
    fn debug_is_redacted() {
        let (agents, switch, _profile) = fixture();
        switch.store(true, Ordering::SeqCst);
        let printed = agents
            .with_account_credential("claude", ACCOUNT, |secret| format!("{secret:?}"))
            .unwrap();
        assert_eq!(printed, "[redacted]");
        let direct = Secret(Zeroizing::new(FAKE_SECRET.to_owned()));
        assert_eq!(format!("{direct:?}"), "[redacted]");
        assert_eq!(format!("{direct:#?}"), "[redacted]");
    }

    #[test]
    fn an_unknown_kind_or_account_is_refused() {
        let (agents, switch, _profile) = fixture();
        switch.store(true, Ordering::SeqCst);
        assert_eq!(
            refused_code(agents.with_account_credential("nope", ACCOUNT, |_| ())),
            codes::AGENT_KIND_NOT_FOUND
        );
        let unknown = "0195c0de-0000-7000-8000-0000000000ff";
        assert_eq!(
            refused_code(agents.with_account_credential("claude", unknown, |_| ())),
            codes::AGENT_ACCOUNT_NOT_FOUND
        );
        // An account of another kind is not this kind's.
        assert_eq!(
            refused_code(agents.with_account_credential("codex", ACCOUNT, |_| ())),
            codes::AGENT_ACCOUNT_NOT_FOUND
        );
        assert_eq!(
            refused_code(agents.with_account_credential("claude", "../escape", |_| ())),
            codes::AGENT_ACCOUNT_NOT_FOUND
        );
    }

    #[test]
    fn a_kind_without_an_official_endpoint_is_refused_before_anything_is_read() {
        let (agents, switch, _profile) = fixture();
        switch.store(true, Ordering::SeqCst);
        // `gentle-shell` is a valid built-in kind with no vendor origin.
        assert!(matches!(
            agents.with_account_credential("gentle-shell", ACCOUNT, |_| ()),
            Err(CredentialAccessError::NoVendorEndpoint { kind }) if kind == "gentle-shell"
        ));
    }

    #[test]
    fn a_fake_vendor_endpoint_lets_a_test_kind_lend_its_credential() {
        let kind = "fake-usage-vendor";
        let account = "0195c0de-0000-7000-8000-0000000000b2";
        let (agents, switch, profile) = fixture();
        switch.store(true, Ordering::SeqCst);
        let kinds = paths::kinds_dir(profile.path());
        fs::create_dir_all(&kinds).unwrap();
        let decl = serde_json::json!({
            "id": kind, "name": "Fake", "program": ["fake-agent"],
            "credentials": { "home_env": "FAKE_AGENT_HOME", "files": ["token.json"] }
        });
        fs::write(kinds.join(format!("{kind}.json")), decl.to_string()).unwrap();
        add_account(&agents, kind, account);
        let home = paths::account_home(profile.path(), kind, account);
        fs::create_dir_all(&home).unwrap();
        fs::write(home.join("token.json"), "fake-token").unwrap();
        assert!(matches!(
            agents.with_account_credential(kind, account, |_| ()),
            Err(CredentialAccessError::NoVendorEndpoint { .. })
        ));
        let _fake = vendor_endpoint::test_hooks::override_origin(kind, "http://127.0.0.1:9");
        let seen = agents
            .with_account_credential(kind, account, |secret| secret.expose().to_owned())
            .unwrap();
        assert_eq!(seen, "fake-token");
    }

    #[test]
    fn a_linked_or_missing_credential_file_is_refused() {
        let (agents, switch, profile) = fixture();
        switch.store(true, Ordering::SeqCst);
        let home = paths::account_home(profile.path(), "claude", ACCOUNT);
        fs::remove_file(home.join(".credentials.json")).unwrap();
        assert_eq!(
            refused_code(agents.with_account_credential("claude", ACCOUNT, |_| ())),
            codes::AGENT_CREDENTIALS_INVALID
        );
        fs::create_dir(home.join(".credentials.json")).unwrap();
        assert_eq!(
            refused_code(agents.with_account_credential("claude", ACCOUNT, |_| ())),
            codes::AGENT_CREDENTIALS_INVALID
        );
    }

    #[test]
    fn a_file_that_is_not_text_is_refused_without_its_contents() {
        let (agents, switch, profile) = fixture();
        switch.store(true, Ordering::SeqCst);
        let home = paths::account_home(profile.path(), "claude", ACCOUNT);
        fs::write(home.join(".credentials.json"), b"\xff\xfeZqx9").unwrap();
        let result = agents.with_account_credential("claude", ACCOUNT, |_| ());
        let Err(CredentialAccessError::Refused(error)) = result else {
            panic!("expected a refusal");
        };
        assert_eq!(error.code, codes::AGENT_CREDENTIALS_INVALID);
        assert!(!error.message.contains("Zqx9"), "{}", error.message);
    }
}
