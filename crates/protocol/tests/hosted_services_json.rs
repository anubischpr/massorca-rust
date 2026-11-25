//! The `marketplace.service.*` wire shapes: names, kebab-case states, a newer peer's value as `Unknown`, and a token
//! that `Debug` never prints.

use massorca_protocol::methods::hosted_services::{
    CONNECT, DISCONNECT, STATUS, ServiceConnect, ServiceSource, ServiceState, ServiceStatus,
};
use massorca_protocol::{capabilities, codes};
use serde_json::json;

#[test]
fn the_methods_and_capability_are_named() {
    assert_eq!(STATUS, "marketplace.service.status");
    assert_eq!(CONNECT, "marketplace.service.connect");
    assert_eq!(DISCONNECT, "marketplace.service.disconnect");
    assert_eq!(capabilities::SERVICES, "services/1");
    assert_eq!(codes::SERVICE_UNKNOWN, "marketplace.service_unknown");
    assert_eq!(
        codes::SERVICE_UNAVAILABLE,
        "marketplace.service_unavailable"
    );
    assert_eq!(codes::SERVICE_REJECTED, "marketplace.service_rejected");
    assert_eq!(codes::SERVICE_FAILED, "marketplace.service_failed");
}

#[test]
fn a_status_encodes_in_kebab_case_and_leaves_out_what_it_lacks() {
    let status = ServiceStatus {
        service: "gitea".to_owned(),
        name: "Gitea".to_owned(),
        state: ServiceState::NotConnected,
        source: ServiceSource::None,
        account: None,
        base_url: None,
        updated_at_ms: None,
        cli_installed: None,
        detail: None,
    };
    assert_eq!(
        serde_json::to_value(&status).expect("encodes"),
        json!({ "service": "gitea", "name": "Gitea", "state": "not-connected", "source": "none" })
    );
    let state: ServiceState = serde_json::from_value(json!("pending")).expect("decodes");
    let source: ServiceSource = serde_json::from_value(json!("cloud")).expect("decodes");
    assert_eq!(
        (state, source),
        (ServiceState::Unknown, ServiceSource::Unknown)
    );
}

#[test]
fn connect_params_never_print_the_token() {
    let params: ServiceConnect = serde_json::from_value(json!({
        "service": "gitlab", "token": "glpat-wire-token", "base_url": "https://gitlab.example.com"
    }))
    .expect("decodes");
    assert_eq!(params.email, None);
    assert!(!format!("{params:?}").contains("glpat-wire-token"));
}
