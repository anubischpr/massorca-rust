//! `marketplace.service.*` for the vault-backed services and the status-only rows. Each call names one service, so no
//! test runs `glab`. No reply or refusal carries the token.

use std::path::Path;

use massorca_client::{Client, ClientError};
use massorca_protocol::codes;
use massorca_protocol::methods::hosted_services::{
    self as wire, ServiceList, ServiceSource, ServiceState, ServiceStatus,
};
use massorca_server::{RunningService, Service, VaultOptions};
use serde_json::{Value, json};

use super::common::{connect, service_config, stop, temp_home};

const TOKEN: &str = "service-test-token-91c2";

async fn start_file_vault(home: &Path) -> RunningService {
    Service::start_with(
        service_config(home, "default"),
        VaultOptions { force_file: true },
    )
    .await
    .expect("the service starts")
}

async fn status(client: &Client, service: &str) -> ServiceStatus {
    let list: ServiceList = client
        .call(wire::STATUS, &json!({ "service": service }))
        .await
        .expect("status answers");
    assert!(!serde_json::to_string(&list).expect("json").contains(TOKEN));
    list.services.into_iter().next().expect("one row")
}

/// The refusal's code; its message and data never carry the token.
async fn refused(client: &Client, method: &str, params: Value) -> String {
    match client.call::<_, Value>(method, &params).await {
        Err(ClientError::Remote(body)) => {
            let text = serde_json::to_string(&body).expect("json");
            assert!(!text.contains(TOKEN), "a refusal carries the token: {text}");
            body.code
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn a_vault_service_connects_reports_its_address_and_disconnects() {
    let home = temp_home();
    let running = start_file_vault(home.path()).await;
    let client = connect(home.path(), "default").await;
    assert_eq!(
        status(&client, "gitea").await.state,
        ServiceState::NotConnected
    );

    let connected: ServiceStatus = client
        .call(
            wire::CONNECT,
            &json!({ "service": "gitea", "token": TOKEN, "base_url": "https://gitea.example.com/" }),
        )
        .await
        .expect("connect answers");
    assert!(
        !serde_json::to_string(&connected)
            .expect("json")
            .contains(TOKEN)
    );
    assert_eq!(
        (connected.state, connected.source),
        (ServiceState::Connected, ServiceSource::Stored)
    );
    assert_eq!(
        connected.base_url.as_deref(),
        Some("https://gitea.example.com")
    );
    assert_eq!(status(&client, "gitea").await, connected);

    let gone: ServiceStatus = client
        .call(wire::DISCONNECT, &json!({ "service": "gitea" }))
        .await
        .expect("disconnect answers");
    assert_eq!(
        (gone.state, gone.source),
        (ServiceState::NotConnected, ServiceSource::None)
    );
    stop(running).await;
}

#[tokio::test]
async fn bad_input_and_status_only_rows_are_refused_without_the_token() {
    let home = temp_home();
    let running = start_file_vault(home.path()).await;
    let client = connect(home.path(), "default").await;
    let linear = status(&client, "linear").await;
    assert_eq!(
        (linear.state, linear.detail.is_some()),
        (ServiceState::Unknown, true)
    );

    for (params, code) in [
        (
            json!({ "service": "nope", "token": TOKEN }),
            codes::SERVICE_UNKNOWN,
        ),
        (
            json!({ "service": "jira", "token": TOKEN }),
            codes::SERVICE_UNAVAILABLE,
        ),
        (
            json!({ "service": "gitea", "token": 7 }),
            codes::REQUEST_INVALID_PARAMS,
        ),
        (
            json!({ "service": "gitea", "token": format!("{TOKEN} x") }),
            codes::REQUEST_INVALID_PARAMS,
        ),
        (
            json!({ "service": "gitea", "token": TOKEN, "base_url": format!("ftp://{TOKEN}") }),
            codes::REQUEST_INVALID_PARAMS,
        ),
        (
            json!({ "service": "gitea", "token": TOKEN, "email": "me@example.com" }),
            codes::REQUEST_INVALID_PARAMS,
        ),
    ] {
        assert_eq!(refused(&client, wire::CONNECT, params).await, code);
    }
    assert_eq!(
        refused(&client, wire::DISCONNECT, json!({ "service": "linear" })).await,
        codes::SERVICE_UNAVAILABLE
    );
    stop(running).await;
}
