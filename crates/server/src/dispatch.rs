//! Runs one request in dispatch order: method, capability, permission, params, handler (ADR 0002).

use std::future::Future;
use std::sync::Arc;

use massorca_protocol::capabilities::BINARY_FRAMES;
use massorca_protocol::methods::events::Subscribed;
use massorca_protocol::{ErrorBody, Frame, Message, codes};
use serde_json::Value;

use crate::abort_guard::AbortOnDrop;
use crate::activity::Activity;
use crate::context::RequestCtx;
use crate::method::{Handler, OpenedSource, encode};
use crate::permission::authorize;
use crate::session::Session;
use crate::streams::forward;

pub(crate) async fn run(session: Arc<Session>, id: u64, method: String, params: Value) {
    let Some(entry) = session.state.registries.methods.lookup(&method) else {
        return reply(
            &session,
            id,
            Err(ErrorBody::new(
                codes::METHOD_NOT_FOUND,
                format!("there is no method named {method}"),
            )),
        )
        .await;
    };
    let capability = entry.value.capability.as_str();
    if !session.capabilities.contains(capability) {
        return reply(
            &session,
            id,
            Err(ErrorBody::new(
                codes::CAPABILITY_NOT_NEGOTIATED,
                format!("{method} needs {capability}, which this connection was not offered"),
            )),
        )
        .await;
    }
    // Older clients close the connection on a frame they cannot read, so they never get a byte stream.
    if entry.value.sends_bytes && !session.client_capabilities.contains(BINARY_FRAMES) {
        return reply(
            &session,
            id,
            Err(ErrorBody::new(
                codes::STREAM_BINARY_FRAMES_REQUIRED,
                format!(
                    "{method} sends binary frames, and this client did not offer {BINARY_FRAMES}"
                ),
            )),
        )
        .await;
    }

    let ctx = RequestCtx {
        principal: session.principal,
        state: Arc::clone(&session.state),
        connection_id: session.connection_id.clone(),
        client_capabilities: Arc::clone(&session.client_capabilities),
    };
    if let Err(error) = authorize(&ctx.principal, entry.value.permission) {
        return reply(&session, id, Err(error)).await;
    }

    match &entry.value.handler {
        Handler::Call(call) => {
            let result = guarded(&session.state.activity, &method, call(ctx, params)).await;
            reply(&session, id, result).await;
        }
        Handler::Stream(open) => {
            match guarded(&session.state.activity, &method, open(ctx, params)).await {
                Ok(source) => open_stream(&session, id, source).await,
                Err(error) => reply(&session, id, Err(error)).await,
            }
        }
    }
}

/// Runs a handler on its own task, so a panic in it is answered as `internal` and the request is never left waiting.
/// The handler counts as activity until its future is dropped, and dropping this one aborts the task, so an aborted
/// connection cannot leave a handler running.
async fn guarded<T, F>(activity: &Activity, method: &str, future: F) -> Result<T, ErrorBody>
where
    T: Send + 'static,
    F: Future<Output = Result<T, ErrorBody>> + Send + 'static,
{
    let work = activity.enter();
    let handler = tokio::spawn(async move {
        let _work = work;
        future.await
    });
    let _abort = AbortOnDrop::new(handler.abort_handle());
    match handler.await {
        Ok(result) => result,
        Err(error) if error.is_panic() => {
            tracing::error!(method, "a method handler panicked");
            Err(ErrorBody::new(
                codes::INTERNAL,
                "the method failed unexpectedly",
            ))
        }
        Err(_) => Err(ErrorBody::new(codes::INTERNAL, "the method was cancelled")),
    }
}

async fn reply(session: &Session, id: u64, result: Result<Value, ErrorBody>) {
    let message = match result {
        Ok(value) => Message::response_ok(id, value),
        Err(error) => Message::response_err(id, error),
    };
    // A connection that has closed has nobody left to answer.
    let _ = session.outbound.send(Frame::Message(message)).await;
}

/// Replies `Subscribed` and then starts forwarding. The stream is registered before the reply, so a cancel that follows it always finds the stream.
async fn open_stream(session: &Session, id: u64, source: OpenedSource) {
    let (stream, cancelled) = session.streams.open();
    let subscribed = match encode(&Subscribed { stream }) {
        Ok(value) => value,
        Err(error) => {
            session.streams.release(stream);
            return reply(session, id, Err(error)).await;
        }
    };
    if session
        .outbound
        .send(Frame::Message(Message::response_ok(id, subscribed)))
        .await
        .is_ok()
    {
        match source {
            OpenedSource::Events(items) => {
                tokio::spawn(forward(
                    stream,
                    items,
                    session.outbound.clone(),
                    cancelled,
                    session.streams.downgrade(),
                    session.max_frame_len,
                ));
            }
            OpenedSource::Items(items) => {
                tokio::spawn(forward(
                    stream,
                    items,
                    session.outbound.clone(),
                    cancelled,
                    session.streams.downgrade(),
                    session.max_frame_len,
                ));
            }
        }
    } else {
        session.streams.release(stream);
    }
}
