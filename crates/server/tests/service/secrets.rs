//! The secret vault methods (decision 0071): round trips without values, refusals with typed codes, and the events.

use std::path::Path;

use massorca_client::{Client, ClientError};
use massorca_protocol::methods::events::{self, ChangeEvent};
use massorca_protocol::methods::secrets::{
    self, SecretDecline, SecretEntry, SecretLink, SecretList, SecretListParams, SecretName,
    SecretRemove, SecretRemoved, SecretRequest, SecretRequestParams, SecretRequests,
    SecretRequestsParams, SecretSet, SecretStatus,
};
use massorca_protocol::{ErrorBody, codes};
use massorca_server::{RunningService, Service, VaultOptions};
use serde::Serialize;
use tokio::time::timeout;

use super::common::{connect, remote_code, service_config, stop, temp_home};

const VALUE: &str = "vault-test-value-7f3a9c";

/// A service whose vault key goes to a file in the temporary profile, never to a real keyring.
async fn start_file_vault(home: &Path) -> RunningService {
    Service::start_with(
        service_config(home, "default"),
        VaultOptions { force_file: true },
    )
    .await
    .expect("the service starts")
}

fn set_params(scope: &str, name: &str, replace: bool) -> SecretSet {
    SecretSet {
        scope: scope.to_owned(),
        name: name.to_owned(),
        value: VALUE.to_owned(),
        replace,
    }
}

/// Fails when the serialized reply carries the test value.
fn assert_no_value<T: Serialize>(reply: &T) {
    let text = serde_json::to_string(reply).expect("the reply serializes");
    assert!(!text.contains(VALUE), "a reply carries the value: {text}");
}

fn error_body(error: &ClientError) -> &ErrorBody {
    match error {
        ClientError::Remote(body) => body,
        other => panic!("expected a remote error, got {other:?}"),
    }
}

async fn set(client: &Client, scope: &str, name: &str) -> SecretEntry {
    client
        .call(secrets::SET, &set_params(scope, name, false))
        .await
        .expect("secret.set answers")
}

async fn refused<P: Serialize>(client: &Client, method: &str, params: &P) -> ErrorBody {
    let error = client
        .call::<_, serde_json::Value>(method, params)
        .await
        .expect_err("the call is refused");
    let body = error_body(&error).clone();
    assert_no_value(&body);
    body
}

#[tokio::test]
async fn set_list_and_status_round_trip_without_the_value() {
    let home = temp_home();
    let running = start_file_vault(home.path()).await;
    let client = connect(home.path(), "default").await;

    let before: SecretStatus = client.call(secrets::STATUS, &()).await.expect("status");
    assert_eq!(before.protection, "none");
    assert_eq!(before.count, 0);

    let entry = set(&client, "global", "API_TOKEN").await;
    assert_no_value(&entry);
    assert_eq!(
        (entry.scope.as_str(), entry.name.as_str()),
        ("global", "API_TOKEN")
    );
    assert!(!entry.linked);

    let rotated: SecretEntry = client
        .call(secrets::SET, &set_params("global", "API_TOKEN", true))
        .await
        .expect("a rotation with replace answers");
    assert_no_value(&rotated);

    let list: SecretList = client
        .call(secrets::LIST, &SecretListParams::default())
        .await
        .expect("list");
    assert_no_value(&list);
    assert_eq!(list.secrets.len(), 1);

    let status: SecretStatus = client.call(secrets::STATUS, &()).await.expect("status");
    assert_eq!(
        (
            status.protection.as_str(),
            status.state.as_str(),
            status.count
        ),
        ("file", "ready", 1)
    );
    stop(running).await;
}

#[tokio::test]
async fn an_existing_name_without_replace_is_name_in_use() {
    let home = temp_home();
    let running = start_file_vault(home.path()).await;
    let client = connect(home.path(), "default").await;
    set(&client, "global", "API_TOKEN").await;

    let body = refused(
        &client,
        secrets::SET,
        &set_params("global", "API_TOKEN", false),
    )
    .await;
    assert_eq!(body.code, codes::SECRET_NAME_IN_USE);
    stop(running).await;
}

#[tokio::test]
async fn setting_through_a_link_is_refused_as_linked() {
    let home = temp_home();
    let running = start_file_vault(home.path()).await;
    let client = connect(home.path(), "default").await;
    set(&client, "global", "API_TOKEN").await;
    let link = SecretLink {
        scope: "project:p1".to_owned(),
        name: "API_TOKEN".to_owned(),
        from_scope: "global".to_owned(),
    };
    let linked: SecretEntry = client.call(secrets::LINK, &link).await.expect("link");
    assert!(linked.linked);
    assert_eq!(linked.owner_scope, "global");

    let body = refused(
        &client,
        secrets::SET,
        &set_params("project:p1", "API_TOKEN", true),
    )
    .await;
    assert_eq!(body.code, codes::SECRET_LINKED);
    stop(running).await;
}

#[tokio::test]
async fn removing_a_linked_secret_needs_include_links() {
    let home = temp_home();
    let running = start_file_vault(home.path()).await;
    let client = connect(home.path(), "default").await;
    set(&client, "global", "API_TOKEN").await;
    let link = SecretLink {
        scope: "project:p1".to_owned(),
        name: "API_TOKEN".to_owned(),
        from_scope: "global".to_owned(),
    };
    let _: SecretEntry = client.call(secrets::LINK, &link).await.expect("link");
    let mut remove = SecretRemove {
        scope: "global".to_owned(),
        name: "API_TOKEN".to_owned(),
        include_links: false,
        force: false,
    };

    let body = refused(&client, secrets::REMOVE, &remove).await;
    assert_eq!(body.code, codes::SECRET_HAS_LINKS);
    let data = body.data.expect("has_links carries data");
    assert_eq!(data["linked"], serde_json::json!(["project:p1"]));

    remove.include_links = true;
    let removed: SecretRemoved = client.call(secrets::REMOVE, &remove).await.expect("remove");
    assert_eq!(removed.links_removed, vec!["project:p1".to_owned()]);
    let list: SecretList = client
        .call(secrets::LIST, &SecretListParams::default())
        .await
        .expect("list");
    assert!(list.secrets.is_empty(), "{list:?}");
    stop(running).await;
}

#[tokio::test]
async fn unlink_removes_only_the_link() {
    let home = temp_home();
    let running = start_file_vault(home.path()).await;
    let client = connect(home.path(), "default").await;
    set(&client, "global", "API_TOKEN").await;
    let link = SecretLink {
        scope: "project:p1".to_owned(),
        name: "API_TOKEN".to_owned(),
        from_scope: "global".to_owned(),
    };
    let _: SecretEntry = client.call(secrets::LINK, &link).await.expect("link");

    let name = SecretName {
        scope: "project:p1".to_owned(),
        name: "API_TOKEN".to_owned(),
    };
    let _: SecretRemoved = client.call(secrets::UNLINK, &name).await.expect("unlink");
    let body = refused(&client, secrets::UNLINK, &name).await;
    assert_eq!(body.code, codes::SECRET_NOT_FOUND);
    let list: SecretList = client
        .call(secrets::LIST, &SecretListParams::default())
        .await
        .expect("list");
    assert_eq!(list.secrets.len(), 1);
    stop(running).await;
}

async fn next_event(subscription: &mut massorca_client::Subscription) -> ChangeEvent {
    timeout(
        super::common::REPLY_WAIT,
        subscription.next_as::<ChangeEvent>(),
    )
    .await
    .expect("an event arrives")
    .expect("the stream is open")
    .expect("the event decodes")
}

#[tokio::test]
async fn a_set_resolves_the_request_and_publishes_the_events() {
    let home = temp_home();
    let running = start_file_vault(home.path()).await;
    let client = connect(home.path(), "default").await;
    let mut subscription = client.subscribe_events().await.expect("subscribe");

    let request: SecretRequest = client
        .call(
            secrets::REQUEST,
            &SecretRequestParams {
                scope: "project:p1".to_owned(),
                name: "DB_PASSWORD".to_owned(),
                reason: "the migrations need it".to_owned(),
                session: Some("session-1".to_owned()),
                agent_token: None,
            },
        )
        .await
        .expect("request");
    assert_eq!(request.state, "pending");
    let requested = next_event(&mut subscription).await;
    assert_eq!(
        (requested.topic.as_str(), requested.id.as_str()),
        (events::TOPIC_SECRET_REQUESTED, request.id.as_str())
    );

    set(&client, "project:p1", "DB_PASSWORD").await;
    let changed = next_event(&mut subscription).await;
    assert_eq!(
        (changed.topic.as_str(), changed.id.as_str()),
        (events::TOPIC_SECRET_CHANGED, "project:p1/DB_PASSWORD")
    );
    let resolved = next_event(&mut subscription).await;
    assert_eq!(
        (resolved.topic.as_str(), resolved.id.as_str()),
        (events::TOPIC_SECRET_REQUEST_CHANGED, request.id.as_str())
    );

    let all: SecretRequests = client
        .call(secrets::REQUESTS, &SecretRequestsParams { all: true })
        .await
        .expect("requests");
    assert_eq!(all.requests[0].state, "stored");
    let open: SecretRequests = client
        .call(secrets::REQUESTS, &SecretRequestsParams::default())
        .await
        .expect("requests");
    assert!(open.requests.is_empty());
    stop(running).await;
}

#[tokio::test]
async fn declining_twice_is_request_resolved() {
    let home = temp_home();
    let running = start_file_vault(home.path()).await;
    let client = connect(home.path(), "default").await;
    let request: SecretRequest = client
        .call(
            secrets::REQUEST,
            &SecretRequestParams {
                scope: "global".to_owned(),
                name: "TOKEN".to_owned(),
                reason: "deploy".to_owned(),
                session: None,
                agent_token: None,
            },
        )
        .await
        .expect("request");
    let mut subscription = client.subscribe_events().await.expect("subscribe");
    let decline = SecretDecline {
        request: request.id.clone(),
    };

    let declined: SecretRequest = client
        .call(secrets::DECLINE, &decline)
        .await
        .expect("decline");
    assert_eq!(declined.state, "declined");
    let changed = next_event(&mut subscription).await;
    assert_eq!(
        (changed.topic.as_str(), changed.id.as_str()),
        (events::TOPIC_SECRET_REQUEST_CHANGED, request.id.as_str())
    );
    let body = refused(&client, secrets::DECLINE, &decline).await;
    assert_eq!(body.code, codes::SECRET_REQUEST_RESOLVED);
    let missing = SecretDecline {
        request: "no-such-request".to_owned(),
    };
    assert_eq!(
        refused(&client, secrets::DECLINE, &missing).await.code,
        codes::SECRET_REQUEST_NOT_FOUND
    );
    stop(running).await;
}

#[tokio::test]
async fn invalid_names_scopes_reasons_and_values_have_typed_codes() {
    let home = temp_home();
    let running = start_file_vault(home.path()).await;
    let client = connect(home.path(), "default").await;

    let bad_name = refused(&client, secrets::SET, &set_params("global", "1BAD", false)).await;
    assert_eq!(bad_name.code, codes::SECRET_INVALID_NAME);
    let bad_scope = refused(
        &client,
        secrets::SET,
        &set_params("nowhere", "TOKEN", false),
    )
    .await;
    assert_eq!(bad_scope.code, codes::SECRET_INVALID_SCOPE);
    let bad_list = refused(
        &client,
        secrets::LIST,
        &SecretListParams {
            scope: Some("project:".to_owned()),
        },
    )
    .await;
    assert_eq!(bad_list.code, codes::SECRET_INVALID_SCOPE);
    let bad_reason = refused(
        &client,
        secrets::REQUEST,
        &SecretRequestParams {
            scope: "global".to_owned(),
            name: "TOKEN".to_owned(),
            reason: "line\nbreak".to_owned(),
            session: None,
            agent_token: None,
        },
    )
    .await;
    assert_eq!(bad_reason.code, codes::SECRET_INVALID_REASON);
    let too_large = SecretSet {
        scope: "global".to_owned(),
        name: "BIG".to_owned(),
        value: "x".repeat(64 * 1024 + 1),
        replace: false,
    };
    let error = client
        .call::<_, serde_json::Value>(secrets::SET, &too_large)
        .await
        .expect_err("too large");
    assert_eq!(remote_code(&error), codes::SECRET_TOO_LARGE);
    let missing = SecretRemove {
        scope: "global".to_owned(),
        name: "NOPE".to_owned(),
        include_links: false,
        force: false,
    };
    assert_eq!(
        refused(&client, secrets::REMOVE, &missing).await.code,
        codes::SECRET_NOT_FOUND
    );
    stop(running).await;
}
