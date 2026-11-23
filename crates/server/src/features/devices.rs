//! Paired devices and pairing offers (`devices/1`) and the network state (`network/1`), decision 0041.

use std::time::{Duration, Instant};

use massorca_core_store::{DeviceRow, now_ms};
use massorca_protocol::methods::devices::{
    self, DeviceForgetParams, DeviceGrantParams, DeviceInfo, DeviceList, DeviceRevokeParams,
    NetworkStatus, PairingCreateParams, PairingCreated, WhoAmI,
};
use massorca_protocol::pairing::Offer;
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::NoParams;
use crate::network::{DEFAULT_TTL, MAX_TTL};
use crate::owner::Owner;
use crate::permission::{Permission, Principal, Scopes, authorize};
use crate::registry::RegistrationHandle;
use crate::store_blocking;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    Ok(vec![
        registries
            .capabilities
            .register(capabilities::DEVICES, Owner::Core)?,
        registries
            .capabilities
            .register(capabilities::NETWORK, Owner::Core)?,
        methods.register(
            Owner::Core,
            devices::LIST,
            capabilities::DEVICES,
            Permission::DeviceAdmin,
            list,
        )?,
        // Any device may revoke itself; revoking another needs device-admin, checked in the handler.
        methods.register(
            Owner::Core,
            devices::REVOKE,
            capabilities::DEVICES,
            Permission::Read,
            revoke,
        )?,
        methods.register(
            Owner::Core,
            devices::FORGET,
            capabilities::DEVICES,
            Permission::HostAdmin,
            forget,
        )?,
        methods.register(
            Owner::Core,
            devices::GRANT,
            capabilities::DEVICES,
            Permission::HostAdmin,
            grant,
        )?,
        methods.register(
            Owner::Core,
            devices::WHOAMI,
            capabilities::DEVICES,
            Permission::Read,
            whoami,
        )?,
        methods.register(
            Owner::Core,
            devices::PAIRING_CREATE,
            capabilities::DEVICES,
            Permission::DeviceAdmin,
            pairing_create,
        )?,
        methods.register(
            Owner::Core,
            devices::NETWORK_STATUS,
            capabilities::NETWORK,
            Permission::Read,
            network_status,
        )?,
    ])
}

fn info(row: DeviceRow, current: Option<&str>) -> DeviceInfo {
    DeviceInfo {
        current: current == Some(row.id.as_str()),
        id: row.id,
        name: row.name,
        scopes: row.scopes,
        key_pin: row.key_pin,
        paired_at_ms: row.paired_at,
        last_seen_ms: row.last_seen_at,
        revoked_at_ms: row.revoked_at,
    }
}

async fn list(ctx: RequestCtx, _params: NoParams) -> Result<DeviceList, ErrorBody> {
    let rows = store_blocking::run(&ctx.state, |store| store.list_devices()).await?;
    let current = ctx.principal.device_id();
    Ok(DeviceList {
        devices: rows.into_iter().map(|row| info(row, current)).collect(),
    })
}

/// Removes a revoked device from the list. The id may name any device; a name is looked up among revoked devices only,
/// since an active one cannot be forgotten.
async fn forget(ctx: RequestCtx, params: DeviceForgetParams) -> Result<DeviceInfo, ErrorBody> {
    let rows = store_blocking::run(&ctx.state, |store| store.list_devices()).await?;
    let by_id = rows.iter().find(|row| row.id == params.device);
    let named: Vec<_> = rows
        .iter()
        .filter(|row| row.name == params.device && row.revoked_at.is_some())
        .collect();
    let row = match (by_id, named.as_slice()) {
        (Some(row), _) => row.clone(),
        (None, [row]) => (*row).clone(),
        (None, []) if rows.iter().any(|row| row.name == params.device) => {
            return Err(not_revoked(&params.device));
        }
        (None, []) => {
            return Err(ErrorBody::new(
                codes::DEVICE_NOT_FOUND,
                format!("no device {:?}", params.device),
            ));
        }
        (None, _) => {
            return Err(ErrorBody::new(
                codes::SELECTOR_AMBIGUOUS,
                format!(
                    "several revoked devices are named {:?}; use the id",
                    params.device
                ),
            ));
        }
    };
    if row.revoked_at.is_none() {
        return Err(not_revoked(&row.name));
    }
    let id = row.id.clone();
    let forgotten = store_blocking::run(&ctx.state, move |store| store.forget_device(&id)).await?;
    if !forgotten {
        return Err(ErrorBody::new(
            codes::DEVICE_NOT_FOUND,
            format!("no device {:?}", params.device),
        ));
    }
    tracing::info!(device = %row.id, "a revoked device was forgotten");
    Ok(info(row, None))
}

fn not_revoked(device: &str) -> ErrorBody {
    ErrorBody::new(
        codes::DEVICE_NOT_REVOKED,
        format!("device {device:?} is still active; revoke it before forgetting it"),
    )
}

/// The device a selector names: its id, or its name when exactly one device has it.
fn find_device(rows: &[DeviceRow], selector: &str) -> Result<DeviceRow, ErrorBody> {
    let by_id = rows.iter().find(|row| row.id == selector);
    let named: Vec<_> = rows.iter().filter(|row| row.name == selector).collect();
    match (by_id, named.as_slice()) {
        (Some(row), _) => Ok(row.clone()),
        (None, [row]) => Ok((*row).clone()),
        (None, []) => Err(ErrorBody::new(
            codes::DEVICE_NOT_FOUND,
            format!("no device {selector:?}"),
        )),
        (None, _) => Err(ErrorBody::new(
            codes::SELECTOR_AMBIGUOUS,
            format!("several devices are named {selector:?}; use the id"),
        )),
    }
}

/// Gives an active device every scope (decision 0041, owner direction of 2026-11-22). It applies from the device's
/// next connection.
async fn grant(ctx: RequestCtx, params: DeviceGrantParams) -> Result<DeviceInfo, ErrorBody> {
    let rows = store_blocking::run(&ctx.state, |store| store.list_devices()).await?;
    let row = find_device(&rows, &params.device)?;
    if row.revoked_at.is_some() {
        return Err(ErrorBody::new(
            codes::DEVICE_REVOKED,
            format!("device {:?} is revoked; pair it again instead", row.name),
        ));
    }
    let id = row.id.clone();
    let scopes = Scopes::ADMIN.names();
    store_blocking::run(&ctx.state, move |store| {
        store.set_device_scopes(&id, &scopes)
    })
    .await?;
    tracing::info!(device = %row.id, "a device was granted full access");
    let id = row.id.clone();
    let stored = store_blocking::run(&ctx.state, move |store| store.get_device(&id)).await?;
    Ok(info(stored.unwrap_or(row), None))
}

async fn revoke(ctx: RequestCtx, params: DeviceRevokeParams) -> Result<DeviceInfo, ErrorBody> {
    let rows = store_blocking::run(&ctx.state, |store| store.list_devices()).await?;
    let row = find_device(&rows, &params.device)?;
    if ctx.principal.device_id() != Some(row.id.as_str()) {
        authorize(&ctx.principal, Permission::DeviceAdmin)?;
    }
    let id = row.id.clone();
    let at = now_ms();
    store_blocking::run(&ctx.state, move |store| store.revoke_device(&id, at)).await?;
    let closed = ctx.state.network.live.revoke(&row.id);
    tracing::info!(device = %row.id, closed, "a device was revoked");
    let id = row.id.clone();
    let stored = store_blocking::run(&ctx.state, move |store| store.get_device(&id)).await?;
    Ok(info(stored.unwrap_or(row), None))
}

async fn whoami(ctx: RequestCtx, _params: NoParams) -> Result<WhoAmI, ErrorBody> {
    let device = match ctx.principal.device_id() {
        Some(id) => {
            let id = id.to_owned();
            let current = id.clone();
            store_blocking::run(&ctx.state, move |store| store.get_device(&id))
                .await?
                .map(|row| info(row, Some(&current)))
        }
        None => None,
    };
    Ok(WhoAmI {
        principal: match ctx.principal {
            Principal::Owner => "owner",
            _ => "device",
        }
        .to_owned(),
        scopes: ctx.principal.scope_names(),
        device,
    })
}

async fn pairing_create(
    ctx: RequestCtx,
    params: PairingCreateParams,
) -> Result<PairingCreated, ErrorBody> {
    let network = &ctx.state.network;
    let Some(pin) = network.server_pin else {
        return Err(unavailable());
    };
    let addresses = network.advertised_addresses();
    if addresses.is_empty() {
        return Err(unavailable());
    }
    let ttl = params
        .ttl_secs
        .map_or(DEFAULT_TTL, |secs| Duration::from_secs(u64::from(secs)));
    if ttl.is_zero() || ttl > MAX_TTL {
        return Err(ErrorBody::new(
            codes::REQUEST_INVALID_PARAMS,
            format!("ttl_secs is 1 to {}", MAX_TTL.as_secs()),
        ));
    }
    // Full access is the default (owner direction of 2026-11-22); a reduced offer is asked for explicitly.
    let scopes = if params.standard {
        Scopes::STANDARD
    } else {
        Scopes::ADMIN
    };
    let secret = network.pairing.create(ttl, scopes, Instant::now())?;
    let expires_at_ms = now_ms() + i64::try_from(ttl.as_millis()).unwrap_or(i64::MAX);
    let offer = Offer {
        server: network.machine_name.clone(),
        addresses: addresses.clone(),
        pin: pin.to_string(),
        secret,
        expires_at_ms,
        scopes: scopes.names(),
    };
    tracing::info!(expires_at_ms, "a pairing offer was created");
    Ok(PairingCreated {
        offer: offer.encode(),
        expires_at_ms,
        scopes: scopes.names(),
        addresses,
        server_pin: pin.to_string(),
    })
}

fn unavailable() -> ErrorBody {
    ErrorBody::new(
        codes::PAIRING_UNAVAILABLE,
        "the service is not listening on the network; start it with --listen",
    )
}

async fn network_status(ctx: RequestCtx, _params: NoParams) -> Result<NetworkStatus, ErrorBody> {
    let network = &ctx.state.network;
    let rows = store_blocking::run(&ctx.state, |store| store.list_devices()).await?;
    let active = rows.iter().filter(|row| row.revoked_at.is_none()).count();
    Ok(NetworkStatus {
        listeners: network.listener_infos(),
        server_pin: network.server_pin.map(|pin| pin.to_string()),
        devices: u32::try_from(active).unwrap_or(u32::MAX),
        pending_offers: u32::try_from(network.pairing.pending(Instant::now())).unwrap_or(u32::MAX),
    })
}
