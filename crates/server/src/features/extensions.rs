//! `extensions.*` (capability `extensions/1`, decision 0062): list, inspect, install, enable, disable and remove
//! packages. The rules live in `core-extensions`; handlers run its blocking calls on the blocking pool, map its
//! errors onto the protocol codes and publish `extension.*` events after each committed change.
//!
//! The wire types are `massorca_protocol::methods::extensions`. They are built from the service's own serialisable
//! views, so the two shapes cannot drift: a test decodes a real item, review and removal into them.

use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use massorca_core_extensions::source::Source;
use massorca_core_extensions::{Error, Extensions};
use massorca_protocol::methods::events::{
    TOPIC_EXTENSION_DISABLED, TOPIC_EXTENSION_ENABLED, TOPIC_EXTENSION_INSTALLED,
    TOPIC_EXTENSION_REMOVED,
};
use massorca_protocol::methods::extensions::{
    self, ExtensionAsset, ExtensionAssetParams, ExtensionEnabled, ExtensionInspectParams,
    ExtensionInstallParams, ExtensionInstalled, ExtensionItem, ExtensionList, ExtensionRef,
    ExtensionRemoveParams, ExtensionRemoved, ExtensionReview, ExtensionSetEnabledParams,
};
use massorca_protocol::{ErrorBody, capabilities, codes};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::NoParams;
use crate::features::workspace_domain::publish;
use crate::owner::Owner;
use crate::permission::{Permission, authorize};
use crate::registry::RegistrationHandle;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    let cap = capabilities::EXTENSIONS;
    Ok(vec![
        registries.capabilities.register(cap, Owner::Core)?,
        methods.register(Owner::Core, extensions::LIST, cap, Permission::Read, list)?,
        methods.register(Owner::Core, extensions::GET, cap, Permission::Read, get)?,
        methods.register(
            Owner::Core,
            extensions::CATALOGUE,
            cap,
            Permission::Read,
            catalogue,
        )?,
        methods.register(
            Owner::Core,
            extensions::INSPECT,
            cap,
            Permission::Read,
            inspect,
        )?,
        methods.register(Owner::Core, extensions::ASSET, cap, Permission::Read, asset)?,
        // These can run a program on the host, so they need host administration.
        methods.register(
            Owner::Core,
            extensions::INSTALL,
            cap,
            Permission::HostAdmin,
            install,
        )?,
        methods.register(
            Owner::Core,
            extensions::SET_ENABLED,
            cap,
            Permission::HostAdmin,
            set_enabled,
        )?,
        methods.register(
            Owner::Core,
            extensions::REMOVE,
            cap,
            Permission::HostAdmin,
            remove,
        )?,
    ])
}

async fn list(ctx: RequestCtx, _params: NoParams) -> Result<ExtensionList, ErrorBody> {
    let items = on_extensions(&ctx, |extensions| extensions.list()).await?;
    Ok(ExtensionList {
        extensions: wire_all(&items)?,
    })
}

async fn catalogue(ctx: RequestCtx, _params: NoParams) -> Result<ExtensionList, ErrorBody> {
    let items = on_extensions(&ctx, |extensions| extensions.catalogue()).await?;
    Ok(ExtensionList {
        extensions: wire_all(&items)?,
    })
}

async fn get(ctx: RequestCtx, params: ExtensionRef) -> Result<ExtensionItem, ErrorBody> {
    let item = on_extensions(&ctx, move |extensions| extensions.get(&params.id)).await?;
    wire(&item)
}

async fn inspect(
    ctx: RequestCtx,
    params: ExtensionInspectParams,
) -> Result<ExtensionReview, ErrorBody> {
    let source = Source::parse(&params.source).map_err(error_body)?;
    authorize(&ctx.principal, inspect_permission(&source))?;
    let review = on_extensions(&ctx, move |extensions| extensions.inspect(&source)).await?;
    wire(&review)
}

/// The catalogue is built in, so `Read` reviews it. A `dir:` or `file:` source reads a host path, which only a host
/// administrator may probe.
fn inspect_permission(source: &Source) -> Permission {
    match source {
        Source::Catalogue(_) => Permission::Read,
        Source::Dir(_) | Source::File(_) => Permission::HostAdmin,
    }
}

async fn asset(ctx: RequestCtx, params: ExtensionAssetParams) -> Result<ExtensionAsset, ErrorBody> {
    let bytes = {
        let (id, path) = (params.id.clone(), params.path.clone());
        on_extensions(&ctx, move |extensions| extensions.asset(&id, &path)).await?
    };
    Ok(ExtensionAsset {
        id: params.id,
        path: params.path,
        size: bytes.len() as u64,
        data: STANDARD.encode(bytes),
    })
}

async fn install(
    ctx: RequestCtx,
    params: ExtensionInstallParams,
) -> Result<ExtensionInstalled, ErrorBody> {
    let entry = on_extensions(&ctx, move |extensions| {
        extensions.install(
            &Source::parse(&params.source)?,
            params.consent.as_deref(),
            params.run_installer,
            params.sha256.as_deref(),
        )
    })
    .await?;
    publish(&ctx, TOPIC_EXTENSION_INSTALLED, &entry.id);
    wire(&entry)
}

async fn set_enabled(
    ctx: RequestCtx,
    params: ExtensionSetEnabledParams,
) -> Result<ExtensionEnabled, ErrorBody> {
    let outcome = on_extensions(&ctx, move |extensions| {
        extensions.set_enabled(&params.id, params.enabled)
    })
    .await?;
    if outcome.changed {
        let topic = if outcome.enabled {
            TOPIC_EXTENSION_ENABLED
        } else {
            TOPIC_EXTENSION_DISABLED
        };
        publish(&ctx, topic, &outcome.id);
    }
    wire(&outcome)
}

async fn remove(
    ctx: RequestCtx,
    params: ExtensionRemoveParams,
) -> Result<ExtensionRemoved, ErrorBody> {
    let removed = on_extensions(&ctx, move |extensions| {
        extensions.remove(&params.id, params.delete_agent_homes, params.uninstall_cli)
    })
    .await?;
    publish(&ctx, TOPIC_EXTENSION_REMOVED, &removed.id);
    wire(&removed)
}

/// Runs one blocking call of the extension service and maps its refusal to the protocol error.
async fn on_extensions<T, F>(ctx: &RequestCtx, call: F) -> Result<T, ErrorBody>
where
    T: Send + 'static,
    F: FnOnce(&Extensions) -> Result<T, Error> + Send + 'static,
{
    let extensions = Arc::clone(&ctx.state.extensions);
    match tokio::task::spawn_blocking(move || call(&extensions)).await {
        Ok(result) => result.map_err(error_body),
        Err(error) => Err(ErrorBody::new(
            codes::INTERNAL,
            format!("the extensions task failed: {error}"),
        )),
    }
}

/// The protocol error of a refusal: its own code, which for a guard is the guard's (`agent.busy`).
fn error_body(error: Error) -> ErrorBody {
    let mut body = ErrorBody::new(error.code(), error.to_string());
    body.data = error.data();
    body
}

/// A service view in its wire shape.
fn wire<T: Serialize, R: DeserializeOwned>(view: &T) -> Result<R, ErrorBody> {
    serde_json::to_value(view)
        .and_then(serde_json::from_value)
        .map_err(|error| ErrorBody::new(codes::INTERNAL, error.to_string()))
}

fn wire_all<T: Serialize, R: DeserializeOwned>(views: &[T]) -> Result<Vec<R>, ErrorBody> {
    views.iter().map(wire).collect()
}

#[cfg(test)]
mod tests {
    use massorca_core_extensions::codes as core_codes;
    use massorca_ext_package::codes as package_codes;

    use super::*;

    /// The protocol owns the code strings; the crates that raise them must spell them the same.
    #[test]
    fn the_protocol_codes_match_the_codes_the_crates_raise() {
        let pairs = [
            (core_codes::INVALID_SOURCE, codes::EXTENSION_INVALID_SOURCE),
            (core_codes::NOT_FOUND, codes::EXTENSION_NOT_FOUND),
            (core_codes::NOT_INSTALLED, codes::EXTENSION_NOT_INSTALLED),
            (
                core_codes::ALREADY_INSTALLED,
                codes::EXTENSION_ALREADY_INSTALLED,
            ),
            (core_codes::KIND_CONFLICT, codes::EXTENSION_KIND_CONFLICT),
            (core_codes::HASH_REQUIRED, codes::EXTENSION_HASH_REQUIRED),
            (
                core_codes::CONSENT_REQUIRED,
                codes::EXTENSION_CONSENT_REQUIRED,
            ),
            (
                core_codes::CONSENT_MISMATCH,
                codes::EXTENSION_CONSENT_MISMATCH,
            ),
            (
                core_codes::INSTALLER_CONSENT_REQUIRED,
                codes::EXTENSION_INSTALLER_CONSENT_REQUIRED,
            ),
            (
                core_codes::INSTALLER_FAILED,
                codes::EXTENSION_INSTALLER_FAILED,
            ),
            (core_codes::VERIFY_FAILED, codes::EXTENSION_VERIFY_FAILED),
            (
                core_codes::ASSET_NOT_LISTED,
                codes::EXTENSION_ASSET_NOT_LISTED,
            ),
            (
                core_codes::ASSET_TOO_LARGE,
                codes::EXTENSION_ASSET_TOO_LARGE,
            ),
            (
                core_codes::INVALID_LOCKFILE,
                codes::EXTENSION_INVALID_LOCKFILE,
            ),
            (core_codes::IO, codes::EXTENSION_IO),
            (core_codes::TAMPERED, codes::EXTENSION_TAMPERED),
            (
                package_codes::INVALID_MANIFEST,
                codes::EXTENSION_INVALID_MANIFEST,
            ),
            (package_codes::INVALID_HASH, codes::EXTENSION_INVALID_HASH),
            (package_codes::HASH_MISMATCH, codes::EXTENSION_HASH_MISMATCH),
            (
                package_codes::INVALID_ARCHIVE,
                codes::EXTENSION_INVALID_ARCHIVE,
            ),
            (package_codes::UNSAFE_PATH, codes::EXTENSION_UNSAFE_PATH),
            (package_codes::UNSAFE_ENTRY, codes::EXTENSION_UNSAFE_ENTRY),
            (
                package_codes::LIMIT_EXCEEDED,
                codes::EXTENSION_LIMIT_EXCEEDED,
            ),
            (package_codes::MISSING_FILE, codes::EXTENSION_MISSING_FILE),
        ];
        for (raised, owned) in pairs {
            assert_eq!(raised, owned);
            assert!(owned.starts_with(codes::EXTENSION_PREFIX));
        }
    }

    #[test]
    fn a_refusal_keeps_the_code_of_the_feature_that_refused() {
        let refused = Error::Refused {
            owner: "agents".to_owned(),
            error: massorca_core_extensions::HookError::new(codes::AGENT_BUSY, "a session is live"),
        };

        let body = error_body(refused);

        assert_eq!(body.code, codes::AGENT_BUSY);
        assert!(body.message.contains("a session is live"));
    }
}
