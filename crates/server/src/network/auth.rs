//! Who a network connection is (decision 0041): a paired device found by its certificate, or a new device pairing with
//! an offer's secret. Every refusal is a `rejected` handshake.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use massorca_core_store::{DeviceRow, StoreError, new_id, now_ms};
use massorca_net::Pin;
use massorca_protocol::pairing::AuthRequest;
use massorca_protocol::{Auth, ErrorBody, codes};

use crate::context::AppState;
use crate::permission::{Principal, Scopes};
use crate::store_blocking;

const MAX_DEVICE_NAME: usize = 64;

/// Decides the principal of a network connection from its client certificate and `hello.auth`.
pub(crate) async fn authenticate(
    state: &AppState,
    client_pin: Pin,
    peer: SocketAddr,
    auth: Option<&Auth>,
) -> Result<Principal, ErrorBody> {
    match AuthRequest::from_auth(auth) {
        Some(AuthRequest::Device {}) => known_device(state, client_pin).await,
        Some(AuthRequest::Pair { secret, name }) => {
            let name = device_name(&name)?;
            let scopes =
                state
                    .network
                    .pairing
                    .redeem(secret.expose(), peer.ip(), Instant::now())?;
            pair(state, client_pin, name, scopes).await
        }
        Some(AuthRequest::Unknown) | None => Err(ErrorBody::new(
            codes::AUTH_REQUIRED,
            "a network connection must authenticate as a paired device or pair with an offer",
        )),
    }
}

async fn known_device(state: &AppState, client_pin: Pin) -> Result<Principal, ErrorBody> {
    let key_pin = client_pin.to_string();
    let row =
        store_blocking::run(state, move |store| store.find_device_by_key_pin(&key_pin)).await?;
    let Some(row) = row else {
        return Err(ErrorBody::new(
            codes::AUTH_UNKNOWN_DEVICE,
            "this device is not paired with the server; pair it again",
        ));
    };
    if row.revoked_at.is_some() {
        return Err(ErrorBody::new(
            codes::AUTH_DEVICE_REVOKED,
            "this device's access was revoked; pair it again to reconnect",
        ));
    }
    let id = row.id.clone();
    // A failed last-seen write must not lock the device out.
    if let Err(error) =
        store_blocking::run(state, move |store| store.touch_device(&id, now_ms())).await
    {
        tracing::warn!(error = %error.message, "recording a device's last connection failed");
    }
    Ok(Principal::Device {
        id: Arc::from(row.id),
        scopes: Scopes::from_names(&row.scopes),
    })
}

async fn pair(
    state: &AppState,
    client_pin: Pin,
    name: String,
    scopes: Scopes,
) -> Result<Principal, ErrorBody> {
    let now = now_ms();
    let row = DeviceRow {
        id: new_id(),
        name,
        key_pin: client_pin.to_string(),
        scopes: scopes.names(),
        paired_at: now,
        last_seen_at: Some(now),
        revoked_at: None,
    };
    let id = row.id.clone();
    let inserted = store_blocking::run(state, move |store| match store.insert_device(&row) {
        Ok(()) => Ok(Ok(())),
        Err(StoreError::Conflict { .. }) => Ok(Err(())),
        Err(error) => Err(error),
    })
    .await?;
    if inserted.is_err() {
        return Err(ErrorBody::new(
            codes::PAIRING_INVALID,
            "this device key is already paired; pair with a new key",
        ));
    }
    tracing::info!(device = %id, "a device paired");
    Ok(Principal::Device {
        id: Arc::from(id),
        scopes,
    })
}

/// A device name the server lists: 1 to 64 characters, no control characters, trimmed.
fn device_name(name: &str) -> Result<String, ErrorBody> {
    let name = name.trim();
    if name.is_empty()
        || name.chars().count() > MAX_DEVICE_NAME
        || name.chars().any(char::is_control)
    {
        return Err(ErrorBody::new(
            codes::REQUEST_INVALID_PARAMS,
            format!("a device name is 1 to {MAX_DEVICE_NAME} printable characters"),
        ));
    }
    Ok(name.to_owned())
}
