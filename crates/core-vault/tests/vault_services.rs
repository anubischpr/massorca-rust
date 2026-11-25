//! Service connection entries: a token, a base URL and an email per hosted service, sealed like secrets. The token is
//! write-only: no read, listing, error or `Debug` output carries it, and no agent receives it.

use massorca_core_store::Store;
use massorca_core_vault::{Scope, SecretValue, ServiceConnection, ServiceField, Vault, VaultError};

const TOKEN: &str = "glpat-service-token-0123456789";

fn vault(dir: &std::path::Path) -> (Vault, Store) {
    let store = Store::open_in_memory().expect("store");
    (Vault::new(store.clone(), dir.join("vault"), true), store)
}

fn connect(
    vault: &Vault,
    service: &str,
    token: &SecretValue,
    base_url: Option<&str>,
    email: Option<&str>,
) -> Result<(), VaultError> {
    vault.set_service(
        service,
        &ServiceConnection {
            token,
            base_url,
            email,
        },
    )
}

fn token(text: &str) -> SecretValue {
    SecretValue::new(text.to_owned()).expect("value")
}

#[test]
fn a_stored_service_reports_its_address_and_never_its_token() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (vault, store) = vault(dir.path());
    assert!(vault.service("gitea").expect("status").is_none());

    let secret = token(TOKEN);
    connect(
        &vault,
        "gitea",
        &secret,
        Some("https://gitea.example.com"),
        None,
    )
    .expect("set");

    let stored = vault.service("gitea").expect("status").expect("stored");
    assert_eq!(
        stored.base_url.as_deref(),
        Some("https://gitea.example.com")
    );
    assert_eq!(stored.email, None);
    assert!(stored.updated_at > 0);
    assert!(!format!("{stored:?}").contains(TOKEN));

    // The ciphertext in the store is not the token.
    let row = store
        .find_secret("service:gitea", ServiceField::Token.name())
        .expect("find")
        .expect("row");
    assert!(!String::from_utf8_lossy(&row.ciphertext).contains(TOKEN));

    // Not a secret: not listed, not counted, never in an agent's environment.
    assert!(vault.list(None).expect("list").is_empty());
    assert_eq!(vault.status().expect("status").count, 0);
    assert!(vault.environment_for("p1").expect("env").is_empty());
}

#[test]
fn connecting_again_replaces_every_field() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (vault, _) = vault(dir.path());
    let first = token("first-token-value");
    connect(
        &vault,
        "bitbucket",
        &first,
        Some("https://bitbucket.example.com"),
        Some("me@example.com"),
    )
    .expect("set");
    let second = token("second-token-value");
    connect(&vault, "bitbucket", &second, None, None).expect("replace");
    let stored = vault.service("bitbucket").expect("status").expect("stored");
    assert_eq!((stored.base_url, stored.email), (None, None));
}

#[test]
fn removing_a_service_deletes_all_its_entries() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (vault, store) = vault(dir.path());
    let secret = token(TOKEN);
    connect(
        &vault,
        "azure-devops",
        &secret,
        Some("https://dev.azure.example.com/org"),
        Some("me@example.com"),
    )
    .expect("set");
    assert!(vault.remove_service("azure-devops").expect("remove"));
    assert!(vault.service("azure-devops").expect("status").is_none());
    assert!(store.list_secrets().expect("rows").is_empty());
    assert!(!vault.remove_service("azure-devops").expect("remove again"));
}

#[test]
fn a_bad_service_id_or_field_is_refused_without_the_token() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (vault, _) = vault(dir.path());
    let secret = token(TOKEN);
    for service in ["", "GitLab", "git/lab", "a b", &"x".repeat(65)] {
        let error = connect(&vault, service, &secret, None, None).expect_err("refused");
        assert!(matches!(error, VaultError::InvalidService), "{service}");
        assert!(!error.to_string().contains(TOKEN));
        assert!(!format!("{error:?}").contains(TOKEN));
    }
    let error = vault
        .set_service(
            "gitea",
            &ServiceConnection {
                token: &secret,
                base_url: Some("https://gitea.example.com\n"),
                email: None,
            },
        )
        .expect_err("control character");
    assert!(matches!(
        error,
        VaultError::InvalidServiceField(ServiceField::BaseUrl)
    ));
    assert!(vault.service("gitea").expect("status").is_none());
}

#[test]
fn secret_methods_cannot_reach_a_service_entry() {
    assert!("service:gitea".parse::<Scope>().is_err());
}

#[test]
fn the_key_is_not_reset_while_a_service_is_stored() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (vault, _) = vault(dir.path());
    let secret = token(TOKEN);
    connect(&vault, "gitea", &secret, None, None).expect("set");
    let error = vault
        .reset_key()
        .expect_err("a service row is sealed by the key");
    assert!(matches!(error, VaultError::VaultNotEmpty { .. }));
    assert_eq!(
        error.to_string(),
        "the vault holds 1 service connection; disconnect it with `massorca marketplace services disconnect` before resetting its key"
    );
}

#[test]
fn a_locked_vault_reports_locked_for_a_stored_service() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (vault, store) = vault(dir.path());
    let secret = token(TOKEN);
    connect(
        &vault,
        "gitea",
        &secret,
        Some("https://gitea.example.com"),
        None,
    )
    .expect("set");
    std::fs::remove_dir_all(dir.path().join("vault")).expect("lose the key");
    let reopened = Vault::new(store, dir.path().join("vault"), true);
    let error = reopened.service("gitea").expect_err("locked");
    assert!(matches!(error, VaultError::Locked(_)));
    assert!(!error.to_string().contains(TOKEN));
}

#[test]
fn a_vault_from_before_the_key_check_accepts_its_key_with_a_service_stored() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (vault, store) = vault(dir.path());
    let secret = token(TOKEN);
    connect(
        &vault,
        "gitea",
        &secret,
        Some("https://gitea.example.com"),
        None,
    )
    .expect("set");
    store.delete_secret_key_check().expect("drop the check");
    let reopened = Vault::new(store.clone(), dir.path().join("vault"), true);
    let stored = reopened.service("gitea").expect("opens").expect("stored");
    assert_eq!(
        stored.base_url.as_deref(),
        Some("https://gitea.example.com")
    );
    assert!(store.secret_key_check().expect("check").is_some());
}
