//! A kind's standing launch settings (decision 0174): one sealed row per kind, which no secret method lists, counts,
//! links or hands to a run's environment. Every value here is a placeholder.

use massorca_core_store::Store;
use massorca_core_vault::{SecretValue, Vault, VaultError};

const VALUE: &str = r#"{"env":{"API_TOKEN":"placeholder-kind-token-41af"}}"#;

fn vault(dir: &std::path::Path) -> (Vault, Store) {
    let store = Store::open_in_memory().expect("store");
    (Vault::new(store.clone(), dir.join("vault"), true), store)
}

fn value(text: &str) -> SecretValue {
    SecretValue::new(text.to_owned()).expect("value")
}

#[test]
fn settings_are_sealed_per_kind_and_read_back() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (vault, store) = vault(dir.path());
    assert!(vault.agent_kind_settings("claude").expect("read").is_none());

    vault
        .set_agent_kind_settings("claude", &value(VALUE))
        .expect("set");
    let read = vault
        .agent_kind_settings("claude")
        .expect("read")
        .expect("stored");
    assert_eq!(read.expose(), VALUE);
    assert!(vault.agent_kind_settings("codex").expect("read").is_none());

    let row = store
        .find_secret("agent-kind:claude", "SETTINGS")
        .expect("find")
        .expect("row");
    assert!(!String::from_utf8_lossy(&row.ciphertext).contains("placeholder-kind-token-41af"));

    // Setting again replaces the one row.
    vault
        .set_agent_kind_settings("claude", &value("{}"))
        .expect("replace");
    assert_eq!(
        vault
            .agent_kind_settings("claude")
            .expect("read")
            .expect("stored")
            .expose(),
        "{}"
    );
    assert_eq!(store.list_secrets().expect("rows").len(), 1);
}

#[test]
fn settings_are_no_secret_and_reach_no_run() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (vault, _) = vault(dir.path());
    vault
        .set_agent_kind_settings("claude", &value(VALUE))
        .expect("set");
    assert!(vault.list(None).expect("list").is_empty());
    assert_eq!(vault.status().expect("status").count, 0);
    assert!(vault.environment_for("p1").expect("env").is_empty());
    assert!(vault.name_clashes().expect("clashes").is_empty());
}

#[test]
fn removing_needs_no_key_and_says_whether_anything_was_stored() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (vault, _) = vault(dir.path());
    assert!(!vault.remove_agent_kind_settings("claude").expect("remove"));
    vault
        .set_agent_kind_settings("claude", &value(VALUE))
        .expect("set");
    assert!(vault.remove_agent_kind_settings("claude").expect("remove"));
    assert!(vault.agent_kind_settings("claude").expect("read").is_none());
}

#[test]
fn an_invalid_kind_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (vault, _) = vault(dir.path());
    for kind in ["", "Claude", "a/b", "x".repeat(33).as_str()] {
        let error = vault
            .set_agent_kind_settings(kind, &value("{}"))
            .expect_err("refused");
        assert!(matches!(error, VaultError::InvalidAgentKind), "{kind}");
    }
}

#[test]
fn the_key_is_not_reset_while_a_kind_has_settings() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (vault, _) = vault(dir.path());
    vault
        .set_agent_kind_settings("claude", &value(VALUE))
        .expect("set");
    let error = vault
        .reset_key()
        .expect_err("a settings row is sealed by the key");
    assert!(
        matches!(&error, VaultError::AgentSettingsStored { kinds } if kinds == &["claude".to_owned()]),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        "the vault holds the launch settings of agent kind claude; clear them with `massorca agent settings clear <kind>` before resetting its key"
    );
}

#[test]
fn a_reopened_vault_opens_the_settings_with_its_key() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open_in_memory().expect("store");
    let first = Vault::new(store.clone(), dir.path().join("vault"), true);
    first
        .set_agent_kind_settings("claude", &value(VALUE))
        .expect("set");
    drop(first);
    let again = Vault::new(store, dir.path().join("vault"), true);
    assert_eq!(
        again
            .agent_kind_settings("claude")
            .expect("read")
            .expect("stored")
            .expose(),
        VALUE
    );
}
