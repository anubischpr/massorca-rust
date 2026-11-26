//! The Marketplace's extension calls (decision 0062). The installed packages and the built-in catalogue are read
//! on each connect and after an `extension.*` event. A write answers its caller, then both lists are read again,
//! so the store follows the service and never the answer. Each read takes a number and the store keeps the newest.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use massorca_client::{Client, ClientError, Hosts};
use massorca_profile::Profile;
use massorca_protocol::capabilities;
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{AgentKind, AgentKinds};
use massorca_protocol::methods::extensions::{
    ExtensionAsset, ExtensionAssetParams, ExtensionEnabled, ExtensionInstallParams,
    ExtensionInstalled, ExtensionItem, ExtensionList, ExtensionLog, ExtensionRemoveParams,
    ExtensionRemoved, ExtensionReview, ExtensionRollbackParams, ExtensionRun,
    ExtensionSetEnabledParams, ExtensionUpdateParams,
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
    /// What the service offers for rollback and the run log, and each installed package's runs.
    pub history: ExtensionHistory,
}

/// What the service offers for rollback and the run log, read with the lists.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExtensionHistory {
    /// The service offers `extensions.rollback` (capability `extensions-rollback/1`).
    pub rollback_offered: bool,
    /// The service offers `extensions.log` (capability `extensions-log/1`).
    pub log_offered: bool,
    /// Each installed package's runs, oldest first, by package id. A package whose log is refused has none.
    pub runs: BTreeMap<String, Vec<ExtensionRun>>,
}

/// What `extensions.rollback` answered: the package as restored, or the consent its restored version needs first.
#[derive(Clone, Debug, PartialEq)]
pub enum RollbackAnswer {
    Restored(ExtensionInstalled),
    /// The version a rollback restores and the consent fingerprint of its review; the call is sent again with it.
    ConsentNeeded {
        version: String,
        consent: String,
    },
}

/// A paired host's agent kinds and extension lists, read together on the host's own connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostLists {
    /// The agent kinds the host declares, as `agent.kinds` lists them.
    pub kinds: Vec<AgentKind>,
    /// The packages the host has installed, in install order.
    pub installed: Vec<ExtensionItem>,
    /// Every built-in catalogue entry the host lists.
    pub catalogue: Vec<ExtensionItem>,
}

/// What a paired host's Marketplace reads answered: its lists, or why it cannot show them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostListing {
    Lists(HostLists),
    /// The host's MassOrca predates the Marketplace: it lacks `extensions/1` or does not know one of its methods.
    UpdateNeeded,
    /// The host could not be read; the refusal says why.
    Refused(Refusal),
}

/// The answer to a read of a paired host, tagged with the host it was asked of and the request it answers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostMarketplaceRead {
    pub host: String,
    pub request: u64,
    pub listing: HostListing,
}

/// Whether a host's welcome offers the `extensions/1` capability the Marketplace calls need.
fn has_extensions(offered: &[String]) -> bool {
    offered
        .iter()
        .any(|name| name.as_str() == capabilities::EXTENSIONS)
}

/// A host's answer to its reads: its lists, that it predates the Marketplace, or why it refused.
fn listing_of(lists: Result<HostLists, ClientError>) -> HostListing {
    match lists {
        Ok(lists) => HostListing::Lists(lists),
        Err(error) if predates(&error) => HostListing::UpdateNeeded,
        Err(error) => HostListing::Refused(Refusal::from(&error)),
    }
}

/// Whether a host answered that it has no such method: a MassOrca that predates the Marketplace calls.
pub(super) fn predates(error: &ClientError) -> bool {
    matches!(error, ClientError::Remote(body) if body.code == codes::METHOD_NOT_FOUND)
}

fn lists_of(
    kinds: Result<AgentKinds, ClientError>,
    installed: Result<ExtensionList, ClientError>,
    catalogue: Result<ExtensionList, ClientError>,
) -> Result<HostLists, ClientError> {
    Ok(HostLists {
        kinds: kinds?.kinds,
        installed: installed?.extensions,
        catalogue: catalogue?.extensions,
    })
}

/// A paired host's answer to the Marketplace reads, taken on the host's own connection.
async fn host_listing(client: &Client) -> HostListing {
    if !has_extensions(&client.welcome().capabilities) {
        return HostListing::UpdateNeeded;
    }
    let (kinds, installed, catalogue) = tokio::join!(
        client.agent_kinds(),
        client.extension_list(),
        client.extension_catalogue()
    );
    listing_of(lists_of(kinds, installed, catalogue))
}

/// Connects to a paired host on its own connection: a call to one host needs no connection to this computer's service.
pub(super) async fn connect_host(profile: &Profile, host: &str) -> Result<Client, ClientError> {
    Client::connect_host(&Hosts::of(profile), host, &super::hosts::peer()).await
}

/// What a host that cannot be connected to says about it: one that speaks another protocol version needs an update.
pub(super) fn connect_listing(error: &ClientError) -> HostListing {
    let refusal = Refusal::from(error);
    if refusal.code() == Some(codes::PROTOCOL_UNSUPPORTED_VERSION) {
        HostListing::UpdateNeeded
    } else {
        HostListing::Refused(refusal)
    }
}

/// Reads a paired host's lists on its own connection; the answer is a store update tagged with `request`.
pub(super) fn spawn_host_read(
    profile: Profile,
    host: String,
    request: u64,
    updates: mpsc::UnboundedSender<StoreUpdate>,
) {
    tokio::spawn(async move {
        let listing = match connect_host(&profile, &host).await {
            Ok(client) => host_listing(&client).await,
            Err(error) => connect_listing(&error),
        };
        let _ = updates.send(StoreUpdate::HostMarketplace(HostMarketplaceRead {
            host,
            request,
            listing,
        }));
    });
}

/// Runs one Marketplace call on a paired host on its own task. A write answers its caller, then the host's lists are read
/// again; a read that changes nothing sends no store update.
pub(super) fn spawn_host_call(
    profile: Profile,
    host: String,
    request: u64,
    command: ExtensionCommand,
    updates: mpsc::UnboundedSender<StoreUpdate>,
) {
    let changes = !matches!(
        command,
        ExtensionCommand::Inspect { .. }
            | ExtensionCommand::Asset { .. }
            | ExtensionCommand::Log { .. }
    );
    tokio::spawn(async move {
        let listing = match connect_host(&profile, &host).await {
            Ok(client) => {
                answer_host_call(&client, command).await;
                if !changes {
                    return;
                }
                host_listing(&client).await
            }
            Err(error) => {
                refuse_while_down(command, Refusal::from(&error));
                if !changes {
                    return;
                }
                connect_listing(&error)
            }
        };
        let _ = updates.send(StoreUpdate::HostMarketplace(HostMarketplaceRead {
            host,
            request,
            listing,
        }));
    });
}

/// Answers a Marketplace call on a connected paired host, as the local calls are answered.
async fn answer_host_call(client: &Client, command: ExtensionCommand) {
    match command {
        ExtensionCommand::Inspect { source, reply } => {
            let result = client.extension_inspect(&source).await;
            let _ = reply.send(result.map_err(|error| Refusal::from(&error)));
        }
        ExtensionCommand::Asset { params, reply } => {
            let result = client.extension_asset(&params).await;
            let _ = reply.send(result.map_err(|error| Refusal::from(&error)));
        }
        ExtensionCommand::Install { params, reply } => {
            let result = client.extension_install(&params).await;
            let _ = reply.send(result.map_err(|error| Refusal::from(&error)));
        }
        ExtensionCommand::SetEnabled { params, reply } => {
            let result = client.extension_set_enabled(&params).await;
            let _ = reply.send(result.map_err(|error| Refusal::from(&error)));
        }
        ExtensionCommand::Update { params, reply } => {
            let result = client.extension_update(&params).await;
            let _ = reply.send(result.map_err(|error| Refusal::from(&error)));
        }
        ExtensionCommand::Remove { params, reply } => {
            let result = remove_with_uninstall_consent(params, |params| async move {
                client.extension_remove(&params).await
            })
            .await;
            let _ = reply.send(result.map_err(|error| Refusal::from(&error)));
        }
        ExtensionCommand::Rollback { params, reply } => {
            let result = rollback_answer(client.extension_rollback(&params).await);
            let _ = reply.send(result.map_err(|error| Refusal::from(&error)));
        }
        ExtensionCommand::Log { id, reply } => {
            let result = client.extension_log(&id).await;
            let _ = reply.send(result.map_err(|error| Refusal::from(&error)));
        }
    }
}

/// A Marketplace call the UI makes. Each answers on its oneshot; the store changes through the reads that follow.
pub(crate) enum ExtensionCommand {
    /// `extensions.inspect`: the review of a source. Changes nothing, so no read follows it.
    Inspect {
        source: String,
        reply: oneshot::Sender<Result<ExtensionReview, Refusal>>,
    },
    /// `extensions.asset`: one listed image of a package. Changes nothing, so no read follows it.
    Asset {
        params: ExtensionAssetParams,
        reply: oneshot::Sender<Result<ExtensionAsset, Refusal>>,
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
    /// `extensions.update`: brings an installed package up to date, with the consent of its new review.
    Update {
        params: ExtensionUpdateParams,
        reply: oneshot::Sender<Result<ExtensionInstalled, Refusal>>,
    },
    /// `extensions.rollback`: restores the version an update replaced. Without consent it answers with the consent needed.
    Rollback {
        params: ExtensionRollbackParams,
        reply: oneshot::Sender<Result<RollbackAnswer, Refusal>>,
    },
    /// `extensions.log`: a package's recent runs. Changes nothing, so no read follows it.
    Log {
        id: String,
        reply: oneshot::Sender<Result<ExtensionLog, Refusal>>,
    },
}

/// Reads both lists. A refusal from the service is the answer for its list; a lost link is passed on, so the
/// bridge reconnects instead of showing the packages as absent.
pub(super) async fn read(client: &Client, reads: &Sequence) -> Result<ExtensionsRead, ClientError> {
    let number = reads.next();
    let (installed, catalogue) =
        tokio::join!(client.extension_list(), client.extension_catalogue());
    let installed = part(installed)?;
    let runs = match &installed {
        Ok(items) if client.has_capability(capabilities::EXTENSIONS_LOG) => {
            runs_of(client, items).await?
        }
        _ => BTreeMap::new(),
    };
    Ok(ExtensionsRead {
        number,
        installed,
        catalogue: part(catalogue)?,
        history: ExtensionHistory {
            rollback_offered: client.has_capability(capabilities::EXTENSIONS_ROLLBACK),
            log_offered: client.has_capability(capabilities::EXTENSIONS_LOG),
            runs,
        },
    })
}

/// Each installed package's runs, read with each list read so a rollback target stays current. A package whose log is
/// refused has none; a lost link is passed on, as the lists pass it.
async fn runs_of(
    client: &Client,
    installed: &[ExtensionItem],
) -> Result<BTreeMap<String, Vec<ExtensionRun>>, ClientError> {
    let mut runs = BTreeMap::new();
    for item in installed {
        match client.extension_log(&item.id).await {
            Ok(log) => {
                runs.insert(item.id.clone(), log.runs);
            }
            Err(ClientError::Remote(_) | ClientError::Protocol(_)) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(runs)
}

/// A rollback's answer: the restored package, or the consent its version needs first. That consent is an answer, not a
/// refusal; any other refusal is returned as it came.
fn rollback_answer(
    result: Result<ExtensionInstalled, ClientError>,
) -> Result<RollbackAnswer, ClientError> {
    match result {
        Ok(installed) => Ok(RollbackAnswer::Restored(installed)),
        Err(error) => consent_needed(&error).ok_or(error),
    }
}

/// The version and consent a refusal names for a rollback that needs the restored version's consent.
fn consent_needed(error: &ClientError) -> Option<RollbackAnswer> {
    match error {
        ClientError::Remote(body) if body.code == codes::EXTENSION_CONSENT_REQUIRED => {
            let data = body.data.as_ref()?;
            Some(RollbackAnswer::ConsentNeeded {
                version: data.get("version")?.as_str()?.to_owned(),
                consent: data.get("consent")?.as_str()?.to_owned(),
            })
        }
        _ => None,
    }
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

    /// `extensions.update`. The list changes through the reads that follow the answer, not from the answer.
    pub async fn update_extension(
        &self,
        params: ExtensionUpdateParams,
    ) -> Result<ExtensionInstalled, Refusal> {
        self.ask(|reply| Command::Extension(ExtensionCommand::Update { params, reply }))
            .await
    }

    /// `extensions.rollback`. Without the consent of the restored version the answer names it, and the call is sent again
    /// with that consent. The list changes through the reads that follow the answer.
    pub async fn rollback_extension(
        &self,
        params: ExtensionRollbackParams,
    ) -> Result<RollbackAnswer, Refusal> {
        self.ask(|reply| Command::Extension(ExtensionCommand::Rollback { params, reply }))
            .await
    }

    /// `extensions.log`: a package's recent runs, oldest first. Nothing changes on the service.
    pub async fn extension_log(&self, id: String) -> Result<ExtensionLog, Refusal> {
        self.ask(|reply| Command::Extension(ExtensionCommand::Log { id, reply }))
            .await
    }

    /// `extensions.asset`: a listed image of a package, base64. Nothing changes on the service.
    pub async fn extension_asset(
        &self,
        params: ExtensionAssetParams,
    ) -> Result<ExtensionAsset, Refusal> {
        self.ask(|reply| Command::Extension(ExtensionCommand::Asset { params, reply }))
            .await
    }

    /// Reads a paired host's agent kinds and extension lists; the answer arrives as a store update tagged with `request`.
    pub fn read_host_marketplace(&self, host: String, request: u64) {
        self.send(Command::HostRead { host, request });
    }

    /// `extensions.inspect` on a paired host. Nothing changes there, so no list is read again.
    pub async fn inspect_host_extension(
        &self,
        host: String,
        source: String,
    ) -> Result<ExtensionReview, Refusal> {
        self.ask_host(host, 0, |reply| ExtensionCommand::Inspect { source, reply })
            .await
    }

    /// `extensions.asset` on a paired host. Nothing changes there.
    pub async fn host_extension_asset(
        &self,
        host: String,
        params: ExtensionAssetParams,
    ) -> Result<ExtensionAsset, Refusal> {
        self.ask_host(host, 0, |reply| ExtensionCommand::Asset { params, reply })
            .await
    }

    /// `extensions.install` on a paired host; the host's lists are read again under `request`.
    pub async fn install_host_extension(
        &self,
        host: String,
        request: u64,
        params: ExtensionInstallParams,
    ) -> Result<ExtensionInstalled, Refusal> {
        self.ask_host(host, request, |reply| ExtensionCommand::Install {
            params,
            reply,
        })
        .await
    }

    /// `extensions.set_enabled` on a paired host; the host's lists are read again under `request`.
    pub async fn set_host_extension_enabled(
        &self,
        host: String,
        request: u64,
        params: ExtensionSetEnabledParams,
    ) -> Result<ExtensionEnabled, Refusal> {
        self.ask_host(host, request, |reply| ExtensionCommand::SetEnabled {
            params,
            reply,
        })
        .await
    }

    /// `extensions.remove` on a paired host; the host's lists are read again under `request`.
    pub async fn remove_host_extension(
        &self,
        host: String,
        request: u64,
        params: ExtensionRemoveParams,
    ) -> Result<ExtensionRemoved, Refusal> {
        self.ask_host(host, request, |reply| ExtensionCommand::Remove {
            params,
            reply,
        })
        .await
    }

    async fn ask_host<T>(
        &self,
        host: String,
        request: u64,
        command: impl FnOnce(oneshot::Sender<Result<T, Refusal>>) -> ExtensionCommand,
    ) -> Result<T, Refusal> {
        self.ask(|reply| Command::HostExtension {
            host,
            request,
            command: command(reply),
        })
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
        ExtensionCommand::Asset { params, reply } => {
            let client = client.clone();
            tokio::spawn(async move {
                let result = client
                    .extension_asset(&params)
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
        ExtensionCommand::Update { params, reply } => {
            write(client, updates, reads, reply, move |client| async move {
                client.extension_update(&params).await
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
        ExtensionCommand::Rollback { params, reply } => {
            write(client, updates, reads, reply, move |client| async move {
                rollback_answer(client.extension_rollback(&params).await)
            });
        }
        ExtensionCommand::Log { id, reply } => {
            let client = client.clone();
            tokio::spawn(async move {
                let result = client
                    .extension_log(&id)
                    .await
                    .map_err(|error| Refusal::from(&error));
                let _ = reply.send(result);
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
        ExtensionCommand::Asset { reply, .. } => {
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
        ExtensionCommand::Update { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        ExtensionCommand::Rollback { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        ExtensionCommand::Log { reply, .. } => {
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

    use super::{
        HostListing, HostLists, Refusal, RollbackAnswer, connect_listing, consent_needed,
        has_extensions, listing_of, remove_with_uninstall_consent, uninstall_fingerprint,
    };

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

    fn no_lists() -> HostLists {
        HostLists {
            kinds: Vec::new(),
            installed: Vec::new(),
            catalogue: Vec::new(),
        }
    }

    #[test]
    fn a_host_must_offer_the_extensions_capability_to_be_read() {
        assert!(has_extensions(&[
            "service/1".to_owned(),
            "extensions/1".to_owned()
        ]));
        assert!(!has_extensions(&["service/1".to_owned()]));
    }

    #[test]
    fn a_host_that_does_not_know_a_marketplace_method_needs_an_update() {
        let missing =
            ClientError::Remote(ErrorBody::new(codes::METHOD_NOT_FOUND, "no such method"));
        assert_eq!(listing_of(Err(missing)), HostListing::UpdateNeeded);
    }

    #[test]
    fn any_other_refusal_of_a_host_is_shown_as_its_refusal() {
        let refused =
            ClientError::Remote(ErrorBody::new("auth.revoked", "This device is not paired."));
        let expected = Refusal::from(&refused);
        assert_eq!(listing_of(Err(refused)), HostListing::Refused(expected));
    }

    #[test]
    fn a_host_that_speaks_another_protocol_version_needs_an_update() {
        let other = ClientError::Remote(ErrorBody::new(
            codes::PROTOCOL_UNSUPPORTED_VERSION,
            "no common protocol version",
        ));
        assert_eq!(connect_listing(&other), HostListing::UpdateNeeded);
    }

    #[test]
    fn a_host_that_answers_its_lists_gives_them() {
        assert_eq!(listing_of(Ok(no_lists())), HostListing::Lists(no_lists()));
    }

    #[test]
    fn a_rollback_refusal_names_the_restored_version_and_its_consent() {
        let mut body = ErrorBody::new(
            codes::EXTENSION_CONSENT_REQUIRED,
            "the restored version needs its review",
        );
        body.data = Some(serde_json::json!({ "version": "1.0.0", "consent": "fp-1" }));
        assert_eq!(
            consent_needed(&ClientError::Remote(body)),
            Some(RollbackAnswer::ConsentNeeded {
                version: "1.0.0".to_owned(),
                consent: "fp-1".to_owned(),
            })
        );
        let busy = ClientError::Remote(ErrorBody::new("extension.busy", "busy"));
        assert_eq!(consent_needed(&busy), None);
    }
}
