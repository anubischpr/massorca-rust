//! The watches' reconnect: a stream that loses its service connects again with backoff and opens itself afresh,
//! printing a line when contact is lost and when it returns (decision 0040).

use std::future::Future;
use std::pin::Pin;

use massorca_client::{
    Backoff, Client, ClientError, Connect, Link, Reopen, Resilient, ResilientSubscription,
    StreamItem, Subscription,
};
use serde_json::{Value, json};

use crate::connect::reconnect;
use crate::error::CliError;
use crate::output::Output;
use crate::target::Target;

/// Opens a watch's stream on a connected client with the watch's parameters.
pub type OpenStream<P> =
    for<'a> fn(
        &'a Client,
        &'a P,
    ) -> Pin<Box<dyn Future<Output = Result<Subscription, ClientError>> + Send + 'a>>;

/// Connects for a resilient stream. The first connection is the command's own; a reconnect never starts the service,
/// which may have been stopped on purpose.
#[derive(Debug)]
pub struct Reconnector {
    target: Target,
    first: Option<Client>,
}

impl Reconnector {
    pub fn new(target: &Target, client: &Client) -> Self {
        Self {
            target: target.clone(),
            first: Some(client.clone()),
        }
    }
}

impl Connect for Reconnector {
    fn connect(&mut self) -> impl Future<Output = Result<Client, ClientError>> + Send {
        let first = self.first.take();
        let target = self.target.clone();
        async move {
            match first {
                Some(client) => Ok(client),
                None => reconnect(&target).await,
            }
        }
    }
}

/// A watch's stream, opened afresh on every connection.
pub struct Reconnect<P> {
    connector: Reconnector,
    open: OpenStream<P>,
    params: P,
}

impl<P> std::fmt::Debug for Reconnect<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reconnect")
            .field("connector", &self.connector)
            .finish_non_exhaustive()
    }
}

impl<P: Clone + Send + Sync + 'static> Reopen for Reconnect<P> {
    fn connect(&mut self) -> impl Future<Output = Result<Client, ClientError>> + Send {
        self.connector.connect()
    }

    fn open(
        &mut self,
        client: &Client,
    ) -> impl Future<Output = Result<Subscription, ClientError>> + Send {
        let open = self.open;
        let params = self.params.clone();
        let client = client.clone();
        async move { open(&client, &params).await }
    }
}

/// A watch stream on `client` that reconnects to `target` when it is cut off, opened with `params` on every attempt.
pub fn resilient<P: Clone + Send + Sync + 'static>(
    target: &Target,
    client: &Client,
    open: OpenStream<P>,
    params: P,
) -> ResilientSubscription<Reconnect<P>> {
    ResilientSubscription::new(
        Reconnect {
            connector: Reconnector::new(target, client),
            open,
            params,
        },
        Backoff::default(),
    )
}

/// The sentence for a link change.
pub fn link_text(link: &Link) -> String {
    match link {
        Link::Unverifiable {
            attempt,
            retry_in,
            reason,
        } => format!(
            "connection lost ({reason}); unverifiable until it returns, attempt {attempt} in {:.1} s",
            retry_in.as_secs_f64()
        ),
        Link::Live => "connected again; live".to_owned(),
    }
}

/// Prints a link change as its own line: text, or `{"ok":true,"result":{"link":...}}` with `--json`.
pub fn print_link(link: &Link, output: Output) {
    let text = link_text(link);
    match link {
        Link::Unverifiable {
            attempt,
            retry_in,
            reason,
        } => output.success(
            &json!({
                "link": "unverifiable",
                "attempt": attempt,
                "retry_in_ms": u64::try_from(retry_in.as_millis()).unwrap_or(u64::MAX),
                "reason": reason,
            }),
            &[text],
        ),
        Link::Live => output.success(&json!({ "link": "live" }), &[text]),
    }
}

/// The next event of a resilient watch, with link changes printed as they happen. `None` once the service ended the
/// stream normally.
pub async fn next_item<P: Clone + Send + Sync + 'static>(
    stream: &mut ResilientSubscription<Reconnect<P>>,
    output: Output,
) -> Option<Result<Value, CliError>> {
    loop {
        match stream.next().await? {
            Ok(Resilient::Item(StreamItem::Event(value))) => return Some(Ok(value)),
            Ok(Resilient::Item(StreamItem::Bytes(_))) => {}
            Ok(Resilient::Link(link)) => print_link(&link, output),
            Err(error) => return Some(Err(error.into())),
        }
    }
}
