//! Methods the service dispatches: typed handlers keyed by name, each with the capability and permission it needs.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use massorca_protocol::methods::RESERVED_EXTENSION_METHOD_PREFIX;
use massorca_protocol::{ErrorBody, codes};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use tokio::sync::mpsc;

use crate::context::RequestCtx;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::{Entry, RegistrationError, RegistrationHandle, Registry};

/// The items of a stream method. Each becomes an `event` frame; the stream ends when the sender is dropped.
pub type StreamSource = mpsc::Receiver<Value>;

/// One item of a stream: a JSON event, or raw bytes sent as a stream-bytes frame.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamItem {
    Event(Value),
    /// Needs the client's `binary-frames/1`; see [`MethodRegistry::register_byte_stream`].
    Bytes(Vec<u8>),
}

impl From<Value> for StreamItem {
    fn from(value: Value) -> Self {
        Self::Event(value)
    }
}

/// The items of a stream method that may send bytes. The stream ends when the sender is dropped.
pub type ByteStreamSource = mpsc::Receiver<StreamItem>;

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;
type CallFn = dyn Fn(RequestCtx, Value) -> BoxFuture<Result<Value, ErrorBody>> + Send + Sync;
type StreamFn =
    dyn Fn(RequestCtx, Value) -> BoxFuture<Result<OpenedSource, ErrorBody>> + Send + Sync;

/// The source a stream handler opened, as the dispatcher forwards it.
pub(crate) enum OpenedSource {
    Events(StreamSource),
    Items(ByteStreamSource),
}

/// What runs when a method is called.
#[derive(Clone)]
pub(crate) enum Handler {
    /// Answers with one result.
    Call(Arc<CallFn>),
    /// Opens a stream: the dispatcher replies `Subscribed` and forwards the items.
    Stream(Arc<StreamFn>),
}

/// Dispatch data of one registered method.
#[derive(Clone)]
pub(crate) struct MethodEntry {
    pub(crate) capability: String,
    pub(crate) permission: Permission,
    pub(crate) handler: Handler,
}

/// One registered method, for listings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodInfo {
    pub method: String,
    pub owner: Owner,
    pub capability: String,
    pub permission: Permission,
}

/// Why a method registration was refused.
#[derive(Debug, thiserror::Error)]
pub enum MethodError {
    #[error("core may not define {method}: the ext. prefix is reserved for extensions")]
    ReservedPrefix { method: String },
    #[error("{method} belongs to an extension, so it must start with ext.")]
    ExtensionPrefixMissing { method: String },
    #[error(transparent)]
    Registration(#[from] RegistrationError),
}

/// The methods the service dispatches. Clones share one table; each registration returns a handle that removes its method when dropped.
#[derive(Clone, Default)]
pub struct MethodRegistry {
    entries: Registry<MethodEntry>,
}

impl MethodRegistry {
    /// Registers a request/response method. `handler` gets the request context and the params decoded as `P`; its `Ok` value becomes the result.
    pub fn register<P, R, F, Fut>(
        &self,
        owner: Owner,
        method: &str,
        capability: &str,
        permission: Permission,
        handler: F,
    ) -> Result<RegistrationHandle, MethodError>
    where
        P: DeserializeOwned + Send + 'static,
        R: Serialize + Send + 'static,
        F: Fn(RequestCtx, P) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<R, ErrorBody>> + Send + 'static,
    {
        self.insert(owner, method, capability, permission, call_handler(handler))
    }

    /// Registers a method that opens a stream. Its result is `Subscribed { stream }` and its items follow as events.
    pub fn register_stream<P, F, Fut>(
        &self,
        owner: Owner,
        method: &str,
        capability: &str,
        permission: Permission,
        handler: F,
    ) -> Result<RegistrationHandle, MethodError>
    where
        P: DeserializeOwned + Send + 'static,
        F: Fn(RequestCtx, P) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<StreamSource, ErrorBody>> + Send + 'static,
    {
        self.insert(
            owner,
            method,
            capability,
            permission,
            stream_handler(handler, OpenedSource::Events),
        )
    }

    /// Registers a method that opens a stream whose items may be bytes. Like `register_stream`, but `Bytes` items go out
    /// as stream-bytes frames. The dispatcher does not check the client's `binary-frames/1`: a handler that opens one must
    /// check `ctx.client_capabilities` first and refuse otherwise.
    pub fn register_byte_stream<P, F, Fut>(
        &self,
        owner: Owner,
        method: &str,
        capability: &str,
        permission: Permission,
        handler: F,
    ) -> Result<RegistrationHandle, MethodError>
    where
        P: DeserializeOwned + Send + 'static,
        F: Fn(RequestCtx, P) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ByteStreamSource, ErrorBody>> + Send + 'static,
    {
        self.insert(
            owner,
            method,
            capability,
            permission,
            stream_handler(handler, OpenedSource::Items),
        )
    }

    /// Every registered method, ordered by name.
    pub fn methods(&self) -> Vec<MethodInfo> {
        self.entries
            .list()
            .iter()
            .map(|entry| MethodInfo {
                method: entry.key.clone(),
                owner: entry.owner.clone(),
                capability: entry.value.capability.clone(),
                permission: entry.value.permission,
            })
            .collect()
    }

    pub(crate) fn lookup(&self, method: &str) -> Option<Arc<Entry<MethodEntry>>> {
        self.entries.get(method)
    }

    fn insert(
        &self,
        owner: Owner,
        method: &str,
        capability: &str,
        permission: Permission,
        handler: Handler,
    ) -> Result<RegistrationHandle, MethodError> {
        check_prefix(&owner, method)?;
        let entry = MethodEntry {
            capability: capability.to_owned(),
            permission,
            handler,
        };
        Ok(self.entries.register(owner, method, entry)?)
    }
}

impl fmt::Debug for MethodRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MethodRegistry").finish_non_exhaustive()
    }
}

/// Wraps a typed handler: decodes the params, runs the handler and encodes its result.
fn call_handler<P, R, F, Fut>(handler: F) -> Handler
where
    P: DeserializeOwned + Send + 'static,
    R: Serialize + Send + 'static,
    F: Fn(RequestCtx, P) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<R, ErrorBody>> + Send + 'static,
{
    let handler = Arc::new(handler);
    Handler::Call(Arc::new(
        move |ctx: RequestCtx, params: Value| -> BoxFuture<Result<Value, ErrorBody>> {
            let handler = Arc::clone(&handler);
            Box::pin(async move {
                let params = decode_params::<P>(params)?;
                let result = handler(ctx, params).await?;
                encode(&result)
            })
        },
    ))
}

/// Wraps a stream handler: decodes the params and hands back the stream's source, tagged by `wrap`.
fn stream_handler<P, S, F, Fut>(handler: F, wrap: fn(S) -> OpenedSource) -> Handler
where
    P: DeserializeOwned + Send + 'static,
    S: Send + 'static,
    F: Fn(RequestCtx, P) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<S, ErrorBody>> + Send + 'static,
{
    let handler = Arc::new(handler);
    Handler::Stream(Arc::new(
        move |ctx: RequestCtx, params: Value| -> BoxFuture<Result<OpenedSource, ErrorBody>> {
            let handler = Arc::clone(&handler);
            Box::pin(async move {
                let params = decode_params::<P>(params)?;
                handler(ctx, params).await.map(wrap)
            })
        },
    ))
}

/// Decodes params. `null` counts as an empty object, so a params struct whose fields are all optional needs no params.
fn decode_params<P: DeserializeOwned>(params: Value) -> Result<P, ErrorBody> {
    if params.is_null()
        && let Ok(decoded) = serde_json::from_value(Value::Object(Map::new()))
    {
        return Ok(decoded);
    }
    serde_json::from_value(params)
        .map_err(|error| ErrorBody::new(codes::REQUEST_INVALID_PARAMS, error.to_string()))
}

/// Encodes a result. The protocol's own types always encode, so a failure is reported as an internal error.
pub(crate) fn encode<T: Serialize>(value: &T) -> Result<Value, ErrorBody> {
    serde_json::to_value(value).map_err(|error| ErrorBody::new(codes::INTERNAL, error.to_string()))
}

fn check_prefix(owner: &Owner, method: &str) -> Result<(), MethodError> {
    let extension_method = method.starts_with(RESERVED_EXTENSION_METHOD_PREFIX);
    match owner {
        Owner::Core if extension_method => Err(MethodError::ReservedPrefix {
            method: method.to_owned(),
        }),
        Owner::Extension(_) if !extension_method => Err(MethodError::ExtensionPrefixMissing {
            method: method.to_owned(),
        }),
        Owner::Core | Owner::Extension(_) => Ok(()),
    }
}
