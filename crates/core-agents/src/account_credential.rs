//! An account's access token, lent to a closure for one vendor request (decision 0176).
//!
//! [`Agents::with_account_credential`] checks the target URL against the kind's official vendor origin, reads the
//! credential file of one account, and lends only its access token to a closure as a [`Secret`], beside the
//! [`CheckedUrl`]; it returns only what the closure returns. It runs only while the one global switch `usage.enabled`
//! is on ([`UsageGate`], read at each call): off, or with a URL the check refuses, it refuses before touching the store
//! or the disk. The file is reached through the same checks as showing credentials and launching
//! ([`crate::credentials`]), with its directories pinned on Windows while it is read.
//!
//! The refresh token and the rest of the file never leave this crate. The token is never logged, never put in an
//! argument list or a child's environment, and never put in a protocol type: [`Secret`] has no `Display`, `Serialize`
//! or `Clone`, its `Debug` is `[redacted]`, and its bytes are zeroed when it is dropped, as is the whole file read.

use std::fmt;
use std::sync::Arc;

use zeroize::Zeroizing;

use crate::access_token;
use crate::credentials;
use crate::sessions::{AgentError, Agents};
use crate::vendor_endpoint::{self, CheckedUrl, EndpointRefused};

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

/// An account's access token, lent to a closure. Zeroed when dropped; `Debug` prints `[redacted]`; no `Display`,
/// `Serialize` or `Clone`, so it cannot be formatted, sent or kept by accident.
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
    /// The access token. Use it only in the request to the [`CheckedUrl`] it was lent with.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

/// Why an account's access token was not lent. No variant ever holds anything read from a file.
#[derive(Debug, thiserror::Error)]
pub enum CredentialAccessError {
    /// `usage.enabled` is off; nothing was read.
    #[error("{USAGE_ENABLED_SETTING} is off, so account credentials are never read")]
    UsageDisabled,
    /// The URL is not on the kind's official vendor origin, or the kind has none; nothing was read.
    #[error(transparent)]
    Endpoint(#[from] EndpointRefused),
    /// This crate knows no access token in the kind's credential file; nothing was read.
    #[error("agent kind {kind} has no known access token in its credential file")]
    TokenUnsupported { kind: String },
    /// The credential file has no access token as a plain non-empty string where the kind keeps it.
    #[error(
        "the credential file of account {account} of agent kind {kind} holds no usable access token"
    )]
    TokenMissing { kind: String, account: String },
    /// The kind, the account or its files were refused, with the code showing credentials would give
    /// (`agent.kind_not_found`, `agent.account_not_found`, `agent.credentials_unsupported`,
    /// `agent.credentials_invalid`...).
    #[error("{}", .0.message)]
    Refused(AgentError),
}

impl From<AgentError> for CredentialAccessError {
    fn from(error: AgentError) -> Self {
        Self::Refused(error)
    }
}

impl Agents {
    /// Lends the access token of `account` of `kind` to `use_secret`, with `url` once it is checked, and returns what
    /// `use_secret` returns (decision 0176).
    ///
    /// Refused, in this order: while `usage.enabled` is off; for a `url` that [`vendor_endpoint::check_url`] refuses
    /// (not https, not the kind's official origin, or a kind with none); for an unknown or invalid kind; for a kind
    /// whose token this crate does not know; for an account that is not one of `kind`'s; for a link anywhere from
    /// `<profile>/agents` to the credential file, or a file that is missing, not a regular file, over 1 MiB, changed
    /// while read or not UTF-8; for a file with no access token. The first two touch neither the store nor the disk.
    /// The homes lock is held only while the file is read, never while `use_secret` runs.
    ///
    /// The request to `url` must not follow redirects (see [`CheckedUrl`]). Blocking: it reads the store and the disk.
    pub fn with_account_credential<T>(
        &self,
        kind: &str,
        account: &str,
        url: &str,
        use_secret: impl FnOnce(&CheckedUrl, &Secret) -> T,
    ) -> Result<T, CredentialAccessError> {
        if !self.usage_gate().is_on() {
            return Err(CredentialAccessError::UsageDisabled);
        }
        let checked = vendor_endpoint::check_url(kind, url)?;
        credentials::declared_kind(self.profile_dir(), self.kind_packages(), kind)?;
        let path = access_token::token_path(kind).ok_or_else(|| {
            CredentialAccessError::TokenUnsupported {
                kind: kind.to_owned(),
            }
        })?;
        self.account_of(kind, account)?;
        let bytes = {
            let _gate = credentials::lock_homes();
            credentials::read_account_credential(
                self.profile_dir(),
                self.kind_packages(),
                kind,
                account,
            )?
        };
        // Borrowed from the zeroed buffer: neither the text nor the token is copied until the token alone is.
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| CredentialAccessError::Refused(credentials::not_text(kind, account)))?;
        let token = access_token::find_token(text, &path).ok_or_else(|| {
            CredentialAccessError::TokenMissing {
                kind: kind.to_owned(),
                account: account.to_owned(),
            }
        })?;
        let secret = Secret(Zeroizing::new(token.to_owned()));
        drop(bytes);
        Ok(use_secret(&checked, &secret))
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
    const CODEX_ACCOUNT: &str = "0195c0de-0000-7000-8000-0000000000a2";
    const URL: &str = "https://api.anthropic.com/api/oauth/usage";
    const CODEX_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
    /// The shape of Claude Code's `.credentials.json`, with fake values.
    const FAKE_CLAUDE_FILE: &str = r#"{"claudeAiOauth":{"accessToken":"fake-access-token","refreshToken":"fake-refresh-token","expiresAt":1,"scopes":["user:inference"]}}"#;
    /// The shape of Codex's `auth.json`, with fake values.
    const FAKE_CODEX_FILE: &str = r#"{"OPENAI_API_KEY":null,"tokens":{"id_token":"fake-id-token","access_token":"fake-codex-access","refresh_token":"fake-codex-refresh","account_id":"fake-account"},"last_refresh":"2026-01-01T00:00:00Z"}"#;

    /// A service whose switch follows the returned flag (off at first), with one account of `claude` and one of `codex`
    /// whose fake credential files are in a temporary profile.
    fn fixture() -> (Agents, Arc<AtomicBool>, tempfile::TempDir) {
        let switch = Arc::new(AtomicBool::new(false));
        let read = Arc::clone(&switch);
        let (agents, _workspace, profile) =
            service_gated(UsageGate::new(move || read.load(Ordering::SeqCst)));
        for (kind, account, file, text) in [
            ("claude", ACCOUNT, ".credentials.json", FAKE_CLAUDE_FILE),
            ("codex", CODEX_ACCOUNT, "auth.json", FAKE_CODEX_FILE),
        ] {
            add_account(&agents, kind, account);
            let home = paths::account_home(profile.path(), kind, account);
            fs::create_dir_all(&home).unwrap();
            fs::write(home.join(file), text).unwrap();
        }
        (agents, switch, profile)
    }

    fn claude_file(profile: &tempfile::TempDir) -> std::path::PathBuf {
        paths::account_home(profile.path(), "claude", ACCOUNT).join(".credentials.json")
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
        let result = agents.with_account_credential("claude", ACCOUNT, URL, |_, _| called = true);
        assert!(matches!(result, Err(CredentialAccessError::UsageDisabled)));
        assert!(!called);
        assert!(
            !paths::agents_dir(profile.path()).exists(),
            "nothing was created under the profile"
        );
        // Off refuses even an unknown kind or a bad URL before looking at them.
        assert!(matches!(
            agents.with_account_credential("nope", ACCOUNT, "http://example.com", |_, _| ()),
            Err(CredentialAccessError::UsageDisabled)
        ));
    }

    #[test]
    fn the_switch_is_read_at_each_call() {
        let (agents, switch, _profile) = fixture();
        switch.store(true, Ordering::SeqCst);
        assert!(
            agents
                .with_account_credential("claude", ACCOUNT, URL, |_, _| ())
                .is_ok()
        );
        switch.store(false, Ordering::SeqCst);
        assert!(matches!(
            agents.with_account_credential("claude", ACCOUNT, URL, |_, _| ()),
            Err(CredentialAccessError::UsageDisabled)
        ));
    }

    #[test]
    fn only_the_access_token_is_lent_with_the_checked_url_and_the_closure_value_is_returned() {
        let (agents, switch, _profile) = fixture();
        switch.store(true, Ordering::SeqCst);
        let length = agents
            .with_account_credential("claude", ACCOUNT, URL, |checked, secret| {
                assert_eq!(checked.as_str(), URL);
                assert_eq!(checked.kind(), "claude");
                assert_eq!(secret.expose(), "fake-access-token");
                secret.expose().len()
            })
            .unwrap();
        assert_eq!(length, "fake-access-token".len());
        let codex = agents
            .with_account_credential("codex", CODEX_ACCOUNT, CODEX_URL, |_, secret| {
                secret.expose().to_owned()
            })
            .unwrap();
        assert_eq!(codex, "fake-codex-access");
    }

    #[test]
    fn a_url_off_the_official_origin_is_refused_before_the_home_is_read() {
        let (agents, switch, profile) = fixture();
        switch.store(true, Ordering::SeqCst);
        // Were the home read, a missing agents directory would be refused differently.
        fs::remove_dir_all(paths::agents_dir(profile.path())).unwrap();
        for url in [
            "http://api.anthropic.com/api/oauth/usage",
            "https://example.com/api/oauth/usage",
            "https://chatgpt.com/backend-api/wham/usage",
            "https://api.anthropic.com.example.com/",
            "https://api.anthropic.com@example.com/",
            "https://api.anthropic.com\\@example.com/",
            "https://api.anthr\u{043e}pic.com/",
            "https://api.anthropic.com:8443/",
            "not a url",
        ] {
            let mut called = false;
            let result =
                agents.with_account_credential("claude", ACCOUNT, url, |_, _| called = true);
            assert!(
                matches!(result, Err(CredentialAccessError::Endpoint(_))),
                "{url}: {result:?}"
            );
            assert!(!called, "{url}");
        }
        assert!(!paths::agents_dir(profile.path()).exists());
    }

    #[test]
    fn a_file_without_the_access_token_is_refused_naming_no_value() {
        let (agents, switch, profile) = fixture();
        switch.store(true, Ordering::SeqCst);
        for text in [
            r#"{"claudeAiOauth":{"refreshToken":"fake-refresh-token"}}"#,
            r#"{"claudeAiOauth":{"accessToken":12345,"refreshToken":"fake-refresh-token"}}"#,
            r#"{"claudeAiOauth":{"accessToken":"","refreshToken":"fake-refresh-token"}}"#,
            r#"{"fake-refresh-token":true}"#,
        ] {
            fs::write(claude_file(&profile), text).unwrap();
            let result = agents.with_account_credential("claude", ACCOUNT, URL, |_, _| ());
            let Err(error @ CredentialAccessError::TokenMissing { .. }) = result else {
                panic!("{text}: expected a missing token, got {result:?}");
            };
            let message = error.to_string();
            assert!(!message.contains("fake"), "{message}");
            assert!(!message.contains("12345"), "{message}");
            assert!(!format!("{error:?}").contains("fake"));
        }
    }

    #[test]
    fn debug_is_redacted() {
        let (agents, switch, _profile) = fixture();
        switch.store(true, Ordering::SeqCst);
        let printed = agents
            .with_account_credential("claude", ACCOUNT, URL, |_, secret| format!("{secret:?}"))
            .unwrap();
        assert_eq!(printed, "[redacted]");
        let direct = Secret(Zeroizing::new("fake-access-token".to_owned()));
        assert_eq!(format!("{direct:?}"), "[redacted]");
        assert_eq!(format!("{direct:#?}"), "[redacted]");
    }

    #[test]
    fn an_unknown_kind_or_account_is_refused() {
        let (agents, switch, _profile) = fixture();
        switch.store(true, Ordering::SeqCst);
        // An unknown kind has no official origin, so its URL is refused first.
        assert!(matches!(
            agents.with_account_credential("nope", ACCOUNT, URL, |_, _| ()),
            Err(CredentialAccessError::Endpoint(
                EndpointRefused::NoOfficialOrigin { .. }
            ))
        ));
        let unknown = "0195c0de-0000-7000-8000-0000000000ff";
        assert_eq!(
            refused_code(agents.with_account_credential("claude", unknown, URL, |_, _| ())),
            codes::AGENT_ACCOUNT_NOT_FOUND
        );
        // An account of another kind is not this kind's.
        assert_eq!(
            refused_code(agents.with_account_credential("codex", ACCOUNT, CODEX_URL, |_, _| ())),
            codes::AGENT_ACCOUNT_NOT_FOUND
        );
        assert_eq!(
            refused_code(agents.with_account_credential("claude", "../escape", URL, |_, _| ())),
            codes::AGENT_ACCOUNT_NOT_FOUND
        );
    }

    #[test]
    fn a_kind_without_an_official_endpoint_is_refused_before_anything_is_read() {
        let (agents, switch, _profile) = fixture();
        switch.store(true, Ordering::SeqCst);
        // `gentle-shell` is a valid built-in kind with no vendor origin.
        assert!(matches!(
            agents.with_account_credential("gentle-shell", ACCOUNT, URL, |_, _| ()),
            Err(CredentialAccessError::Endpoint(EndpointRefused::NoOfficialOrigin { kind }))
                if kind == "gentle-shell"
        ));
    }

    #[test]
    fn a_fake_vendor_origin_does_not_lend_a_kind_whose_token_is_unknown() {
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
        let _fake = vendor_endpoint::test_hooks::override_origin(kind, "http://127.0.0.1:9");
        assert!(matches!(
            agents.with_account_credential(kind, account, "http://127.0.0.1:9/usage", |_, _| ()),
            Err(CredentialAccessError::TokenUnsupported { .. })
        ));
    }

    #[test]
    fn a_linked_or_missing_credential_file_is_refused() {
        let (agents, switch, profile) = fixture();
        switch.store(true, Ordering::SeqCst);
        fs::remove_file(claude_file(&profile)).unwrap();
        assert_eq!(
            refused_code(agents.with_account_credential("claude", ACCOUNT, URL, |_, _| ())),
            codes::AGENT_CREDENTIALS_INVALID
        );
        fs::create_dir(claude_file(&profile)).unwrap();
        assert_eq!(
            refused_code(agents.with_account_credential("claude", ACCOUNT, URL, |_, _| ())),
            codes::AGENT_CREDENTIALS_INVALID
        );
    }

    #[test]
    fn a_file_that_is_not_text_is_refused_without_its_contents() {
        let (agents, switch, profile) = fixture();
        switch.store(true, Ordering::SeqCst);
        fs::write(claude_file(&profile), b"\xff\xfeZqx9").unwrap();
        let result = agents.with_account_credential("claude", ACCOUNT, URL, |_, _| ());
        let Err(CredentialAccessError::Refused(error)) = result else {
            panic!("expected a refusal");
        };
        assert_eq!(error.code, codes::AGENT_CREDENTIALS_INVALID);
        assert!(!error.message.contains("Zqx9"), "{}", error.message);
    }
}
