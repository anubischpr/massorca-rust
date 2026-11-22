//! Vault refusals added after review: malformed params never echo a value, a case-only name clash is `name_in_use`,
//! an empty vault's key can be reset, and requests are limited.

use std::path::Path;

use massorca_client::{Client, ClientError};
use massorca_protocol::methods::secrets::{
    self, SecretEntry, SecretRemove, SecretRequestParams, SecretSet, SecretStatus,
};
use massorca_protocol::{ErrorBody, codes};
use massorca_server::{RunningService, Service, VaultOptions};
use serde::Serialize;
use serde_json::{Value, json};

use super::common::{connect, service_config, stop, temp_home};

const VALUE: &str = "hardening-value-61d0b8";

async fn start_file_vault(home: &Path) -> RunningService {
    Service::start_with(
        service_config(home, "default"),
        VaultOptions { force_file: true },
    )
    .await
    .expect("the service starts")
}

async fn refused<P: Serialize + ?Sized>(client: &Client, method: &str, params: &P) -> ErrorBody {
    match client.call::<_, Value>(method, params).await {
        Err(ClientError::Remote(body)) => {
            let text = serde_json::to_string(&body).expect("serializes");
            assert!(!text.contains(VALUE), "a refusal carries the value: {text}");
            body
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

async fn set(client: &Client, scope: &str, name: &str) -> SecretEntry {
    client
        .call(
            secrets::SET,
            &SecretSet {
                scope: scope.to_owned(),
                name: name.to_owned(),
                value: VALUE.to_owned(),
                replace: false,
            },
        )
        .await
        .expect("secret.set answers")
}

#[tokio::test]
async fn malformed_set_params_name_the_field_and_never_the_value() {
    let home = temp_home();
    let running = start_file_vault(home.path()).await;
    let client = connect(home.path(), "default").await;
    for (params, field) in [
        (
            json!({ "scope": "global", "name": "A", "value": 61_008 }),
            "value",
        ),
        (
            json!({ "scope": "global", "name": "A", "value": { "inner": VALUE } }),
            "value",
        ),
        (
            json!({ "scope": "global", "name": "A", "value": "x", "replace": VALUE }),
            "replace",
        ),
    ] {
        let body = refused(&client, secrets::SET, &params).await;
        assert_eq!(body.code, codes::REQUEST_INVALID_PARAMS);
        assert!(
            body.message.contains(&format!("`{field}`")),
            "{}",
            body.message
        );
        assert!(!body.message.contains("61008"), "{}", body.message);
    }
    let wrong_field = refused(
        &client,
        secrets::SET,
        &json!({ "scope": VALUE, "name": "A", "value": "x" }),
    )
    .await;
    assert_eq!(wrong_field.code, codes::SECRET_INVALID_SCOPE);
    stop(running).await;
}

#[tokio::test]
async fn a_name_differing_only_by_case_is_name_in_use() {
    let home = temp_home();
    let running = start_file_vault(home.path()).await;
    let client = connect(home.path(), "default").await;
    set(&client, "global", "TOKEN").await;
    let params = SecretSet {
        scope: "project:p1".to_owned(),
        name: "token".to_owned(),
        value: VALUE.to_owned(),
        replace: false,
    };
    let body = refused(&client, secrets::SET, &params).await;
    assert_eq!(body.code, codes::SECRET_NAME_IN_USE);
    assert!(body.message.contains("only by case"), "{}", body.message);
    stop(running).await;
}

#[tokio::test]
async fn reset_key_is_refused_while_a_secret_exists() {
    let home = temp_home();
    let running = start_file_vault(home.path()).await;
    let client = connect(home.path(), "default").await;
    set(&client, "global", "TOKEN").await;
    let body = refused(&client, secrets::RESET_KEY, &()).await;
    assert_eq!(body.code, codes::SECRET_VAULT_NOT_EMPTY);
    assert_eq!(body.data.expect("data")["count"], 1);

    let removed: Value = client
        .call(
            secrets::REMOVE,
            &SecretRemove {
                scope: "global".to_owned(),
                name: "TOKEN".to_owned(),
                include_links: false,
                force: false,
            },
        )
        .await
        .expect("removed");
    assert_eq!(removed["name"], "TOKEN");
    let status: SecretStatus = client
        .call(secrets::RESET_KEY, &())
        .await
        .expect("an empty vault resets");
    assert_eq!((status.protection.as_str(), status.count), ("none", 0));
    set(&client, "global", "TOKEN").await;
    stop(running).await;
}

#[tokio::test]
async fn requests_past_the_limit_are_request_limit() {
    let home = temp_home();
    let running = start_file_vault(home.path()).await;
    let client = connect(home.path(), "default").await;
    let params = |index: usize| SecretRequestParams {
        scope: "global".to_owned(),
        name: format!("N{index}"),
        reason: "needed".to_owned(),
        session: None,
        agent_token: None,
    };
    for index in 0..10 {
        let _: Value = client
            .call(secrets::REQUEST, &params(index))
            .await
            .expect("within the limit");
    }
    let body = refused(&client, secrets::REQUEST, &params(10)).await;
    assert_eq!(body.code, codes::SECRET_REQUEST_LIMIT);
    stop(running).await;
}

#[tokio::test]
async fn an_agent_token_that_matches_no_live_run_is_refused() {
    let home = temp_home();
    let running = start_file_vault(home.path()).await;
    let client = connect(home.path(), "default").await;
    for session in [Some("no-such-session".to_owned()), None] {
        let params = SecretRequestParams {
            scope: "global".to_owned(),
            name: "TOKEN".to_owned(),
            reason: "needed".to_owned(),
            session,
            agent_token: Some("forged-token".to_owned()),
        };
        let body = refused(&client, secrets::REQUEST, &params).await;
        assert_eq!(body.code, codes::SECRET_INVALID_SESSION);
        assert!(!body.message.contains("forged-token"));
    }
    stop(running).await;
}
