//! The Marketplace's extension calls (decision 0062). The installed packages and the built-in catalogue are read
//! on each connect and after an `extension.*` event. A write answers its caller, then both lists are read again,
//! so the store follows the service and never the answer. Each read takes a number and the store keeps the newest.

use std::future::Future;
use std::sync::Arc;

use massorca_client::{Client, ClientError};
use massorca_protocol::codes;
use massorca_protocol::methods::extensions::{
    ExtensionEnabled, ExtensionInstallParams, ExtensionInstalled, ExtensionItem, ExtensionList,
    ExtensionRemoveParams, ExtensionRemoved, ExtensionReview, ExtensionSetEnabledParams,
};
use tokio::sync::{mpsc, oneshot};

use super::bridge::{Command, ServiceHandle};
use super::refusal::Refusal;
use super::teams::Sequence;
use super::update::StoreUpdate;

/// The installed packages and the built-in catalogue, read together. Each list is refused or listed on its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionsRead {
    pub number: u64,
    /// The packages this service has installed, with their state, in install order.
    pub installed: Result<Vec<ExtensionItem>, Refusal>,
    /// Every built-in catalogue entry with its state, installed or not.
    pub catalogue: Result<Vec<ExtensionItem>, Refusal>,
}

/// A Marketplace call the UI makes. Each answers on its oneshot; the store changes through the reads that follow.
pub(crate) enum ExtensionCommand {
    /// `extensions.inspect`: the review of a source. Changes nothing, so no read follows it.
    Inspect {
        source: String,
        reply: oneshot::Sender<Result<ExtensionReview, Refusal>>,
    },
    Install {
        params: ExtensionInstallParams,
        reply: oneshot::Sender<Result<ExtensionInstalled, Refusal>>,
    },
    SetEnabled {
        params: ExtensionSetEnabledParams,
        reply: oneshot::Sender<Result<ExtensionEnabled, Refusal>>,
    },
    Remove {
        params: ExtensionRemoveParams,
        reply: oneshot::Sender<Result<ExtensionRemoved, Refusal>>,
    },
}

/// Reads both lists. A refusal from the service is the answer for its list; a lost link is passed on, so the
/// bridge reconnects instead of showing the packages as absent.
pub(super) async fn read(client: &Client, reads: &Sequence) -> Result<ExtensionsRead, ClientError> {
    let number = reads.next();
    let (installed, catalogue) =
        tokio::join!(client.extension_list(), client.extension_catalogue());
    Ok(ExtensionsRead {
        number,
        installed: part(installed)?,
        catalogue: part(catalogue)?,
    })
}

fn part(
    result: Result<ExtensionList, ClientError>,
) -> Result<Result<Vec<ExtensionItem>, Refusal>, ClientError> {
    match result {
        Ok(list) => Ok(Ok(list.extensions)),
        Err(error @ (ClientError::Remote(_) | ClientError::Protocol(_))) => {
            Ok(Err(Refusal::from(&error)))
        }
        Err(error) => Err(error),
    }
}

impl ServiceHandle {
    /// `extensions.inspect`: the review of a source the person is about to install. Nothing changes on the service.
    pub async fn inspect_extension(&self, source: String) -> Result<ExtensionReview, Refusal> {
        self.ask(|reply| Command::Extension(ExtensionCommand::Inspect { source, reply }))
            .await
    }

    /// `extensions.install`. The list changes through the reads that follow the answer, not from the answer.
    pub async fn install_extension(
        &self,
        params: ExtensionInstallParams,
    ) -> Result<ExtensionInstalled, Refusal> {
        self.ask(|reply| Command::Extension(ExtensionCommand::Install { params, reply }))
            .await
    }

    /// `extensions.set_enabled`. The list changes through the reads that follow the answer.
    pub async fn set_extension_enabled(
        &self,
        params: ExtensionSetEnabledParams,
    ) -> Result<ExtensionEnabled, Refusal> {
        self.ask(|reply| Command::Extension(ExtensionCommand::SetEnabled { params, reply }))
            .await
    }

    /// `extensions.remove`. The list changes through the reads that follow the answer.
    pub async fn remove_extension(
        &self,
        params: ExtensionRemoveParams,
    ) -> Result<ExtensionRemoved, Refusal> {
        self.ask(|reply| Command::Extension(ExtensionCommand::Remove { params, reply }))
            .await
    }
}

/// Runs one Marketplace call on its own task, so a slow installer never holds up events.
pub(super) fn spawn_call(
    client: &Client,
    command: ExtensionCommand,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    reads: &Arc<Sequence>,
) {
    match command {
        ExtensionCommand::Inspect { source, reply } => {
            let client = client.clone();
            tokio::spawn(async move {
                let result = client
                    .extension_inspect(&source)
                    .await
                    .map_err(|error| Refusal::from(&error));
                let _ = reply.send(result);
            });
        }
        ExtensionCommand::Install { params, reply } => {
            write(client, updates, reads, reply, move |client| async move {
                client.extension_install(&params).await
            });
        }
        ExtensionCommand::SetEnabled { params, reply } => {
            write(client, updates, reads, reply, move |client| async move {
                client.extension_set_enabled(&params).await
            });
        }
        ExtensionCommand::Remove { params, reply } => {
            write(client, updates, reads, reply, move |client| async move {
                let call = &client;
                remove_with_uninstall_consent(params, |params| async move {
                    call.extension_remove(&params).await
                })
                .await
            });
        }
    }
}

/// Runs a write, answers its caller, then reads both lists again whatever the answer was.
fn write<T, Fut>(
    client: &Client,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    reads: &Arc<Sequence>,
    reply: oneshot::Sender<Result<T, Refusal>>,
    call: impl FnOnce(Client) -> Fut + Send + 'static,
) where
    T: Send + 'static,
    Fut: Future<Output = Result<T, ClientError>> + Send + 'static,
{
    let client = client.clone();
    let updates = updates.clone();
    let reads = Arc::clone(reads);
    tokio::spawn(async move {
        let result = call(client.clone()).await;
        let _ = reply.send(result.map_err(|error| Refusal::from(&error)));
        refresh(&client, &updates, &reads).await;
    });
}

/// Sends a fresh read. A lost link sends nothing: the bridge reconnects and reads again on connect.
async fn refresh(client: &Client, updates: &mpsc::UnboundedSender<StoreUpdate>, reads: &Sequence) {
    if let Ok(read) = read(client, reads).await {
        let _ = updates.send(StoreUpdate::Extensions(read));
    }
}

/// Answers a Marketplace call with `refusal`, while there is no connection to ask.
pub(super) fn refuse_while_down(command: ExtensionCommand, refusal: Refusal) {
    match command {
        ExtensionCommand::Inspect { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        ExtensionCommand::Install { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        ExtensionCommand::SetEnabled { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        ExtensionCommand::Remove { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
    }
}

/// Runs `extensions.remove`. When the person ticked the CLI's removal, the service first names the uninstall commands it
/// needs accepted, with their fingerprint, and the removal is sent again with that fingerprint. The tick is the
/// acceptance, as `--yes` is on the CLI. A refusal without a fingerprint is returned as it came.
async fn remove_with_uninstall_consent<F, Fut>(
    params: ExtensionRemoveParams,
    mut remove: F,
) -> Result<ExtensionRemoved, ClientError>
where
    F: FnMut(ExtensionRemoveParams) -> Fut,
    Fut: Future<Output = Result<ExtensionRemoved, ClientError>>,
{
    let asks_cli = params.uninstall_cli && params.uninstall_consent.is_none();
    match remove(params.clone()).await {
        Err(error) if asks_cli => match uninstall_fingerprint(&error) {
            Some(fingerprint) => {
                remove(ExtensionRemoveParams {
                    uninstall_consent: Some(fingerprint),
                    ..params
                })
                .await
            }
            None => Err(error),
        },
        result => result,
    }
}

/// The fingerprint a refusal names for the uninstall commands it needs accepted; none for any other refusal.
fn uninstall_fingerprint(error: &ClientError) -> Option<String> {
    match error {
        ClientError::Remote(body) if body.code == codes::EXTENSION_CONSENT_REQUIRED => body
            .data
            .as_ref()?
            .get("fingerprint")?
            .as_str()
            .map(str::to_owned),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use massorca_client::ClientError;
    use massorca_protocol::ErrorBody;
    use massorca_protocol::codes;
    use massorca_protocol::methods::extensions::{ExtensionRemoveParams, ExtensionRemoved};

    use super::{remove_with_uninstall_consent, uninstall_fingerprint};

    /// The refusal the service gives when a removal needs its uninstall commands accepted first.
    fn consent_refusal(fingerprint: &str) -> ClientError {
        let mut body = ErrorBody::new(
            codes::EXTENSION_CONSENT_REQUIRED,
            "removing runs a vendor uninstall command",
        );
        body.data = Some([("fingerprint", fingerprint)].into_iter().collect());
        ClientError::Remote(body)
    }

    fn removed() -> ExtensionRemoved {
        ExtensionRemoved {
            id: "massorca.codex".to_owned(),
            version: "1.0.0".to_owned(),
            failures: Vec::new(),
            cli: Vec::new(),
        }
    }

    fn params(uninstall_cli: bool) -> ExtensionRemoveParams {
        ExtensionRemoveParams {
            id: "massorca.codex".to_owned(),
            uninstall_cli,
            ..Default::default()
        }
    }

    #[test]
    fn the_fingerprint_is_read_only_from_a_consent_refusal() {
        assert_eq!(
            uninstall_fingerprint(&consent_refusal("fp-1")),
            Some("fp-1".to_owned())
        );
        let busy = ClientError::Remote(ErrorBody::new("extension.busy", "busy"));
        assert_eq!(uninstall_fingerprint(&busy), None);
    }

    #[tokio::test]
    async fn a_removal_with_the_cli_is_sent_again_with_the_fingerprint_the_service_names() {
        let sent = RefCell::new(Vec::new());
        let answers = RefCell::new(vec![Err(consent_refusal("fp-1")), Ok(removed())]);
        let result = remove_with_uninstall_consent(params(true), |params| {
            sent.borrow_mut().push(params);
            let answer = answers.borrow_mut().remove(0);
            std::future::ready(answer)
        })
        .await;
        assert!(result.is_ok(), "the second call removes the package");
        let sent = sent.into_inner();
        assert_eq!(sent.len(), 2, "the refusal is answered with one more call");
        assert_eq!(sent[0].uninstall_consent, None);
        assert_eq!(sent[1].uninstall_consent.as_deref(), Some("fp-1"));
        assert!(sent.iter().all(|params| params.uninstall_cli));
    }

    #[tokio::test]
    async fn a_removal_that_keeps_the_cli_is_sent_once_without_consent() {
        let sent = RefCell::new(Vec::new());
        let result = remove_with_uninstall_consent(params(false), |params| {
            sent.borrow_mut().push(params);
            std::future::ready(Ok::<ExtensionRemoved, ClientError>(removed()))
        })
        .await;
        assert!(result.is_ok());
        let sent = sent.into_inner();
        assert_eq!(sent.len(), 1);
        assert!(!sent[0].uninstall_cli);
        assert_eq!(sent[0].uninstall_consent, None);
    }

    #[tokio::test]
    async fn a_refusal_that_names_no_fingerprint_is_returned_without_another_call() {
        let calls = RefCell::new(0_usize);
        let result = remove_with_uninstall_consent(params(true), |_| {
            *calls.borrow_mut() += 1;
            let refusal = ClientError::Remote(ErrorBody::new("extension.busy", "busy"));
            std::future::ready(Err::<ExtensionRemoved, ClientError>(refusal))
        })
        .await;
        assert_eq!(calls.into_inner(), 1);
        assert!(matches!(result, Err(ClientError::Remote(body)) if body.code == "extension.busy"));
    }
}
