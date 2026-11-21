//! `events.subscribe`: streams the event bus to a client (capability `events/1`).

use massorca_protocol::methods::events;
use massorca_protocol::{ErrorBody, capabilities};
use tokio::sync::mpsc;

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::NoParams;
use crate::method::{StreamSource, encode};
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

/// Items queued for one subscriber before its forwarder waits for the connection to catch up.
const QUEUE_CAPACITY: usize = 64;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![
        registries
            .capabilities
            .register(capabilities::EVENTS, Owner::Core)?,
        registries.methods.register_stream(
            Owner::Core,
            events::SUBSCRIBE,
            capabilities::EVENTS,
            Permission::Read,
            subscribe,
        )?,
    ])
}

/// Subscribes before answering, so an event published after the answer is never missed.
async fn subscribe(ctx: RequestCtx, _params: NoParams) -> Result<StreamSource, ErrorBody> {
    let mut subscription = ctx.state.events.subscribe();
    let (items, source) = mpsc::channel(QUEUE_CAPACITY);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                () = items.closed() => break,
                event = subscription.next() => {
                    let Some(event) = event else { break };
                    match encode(&event) {
                        Ok(value) => {
                            if items.send(value).await.is_err() {
                                break;
                            }
                        }
                        Err(error) => {
                            tracing::warn!(message = %error.message, "an event could not be encoded");
                            break;
                        }
                    }
                }
            }
        }
    });
    Ok(source)
}
