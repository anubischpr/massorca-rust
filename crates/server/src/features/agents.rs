//! The agent methods (capability `agents/1`, `docs/architecture/agents.md`). The rules live in `core-agents`;
//! handlers run its blocking calls on the blocking pool. `core-agents` announces every status change; the service
//! turns each into `agent.changed` on the event bus and an item for `agent.watch` (0052).

use std::path::Path;

use massorca_core_agents::oneshot::{TitleRequest, title_timeout};
use massorca_core_agents::{AgentError, Agents, Change, MergeResult, RepoOps, SecretGrant};
use massorca_core_git::{Git, GitError, MergeOutcome};
use massorca_core_workspace::DomainError;
use massorca_protocol::methods::agents::{
    self, AccountSummary, AccountsActivateParams, AccountsAddParams, AccountsAdded,
    AccountsCheckParams, AccountsImportParams, AccountsImported, AccountsList, AccountsListParams,
    AccountsRemoveParams, AccountsRemoved, AccountsSignInParams, AgentArchive,
    AgentArchiveSessionParams, AgentBringBackParams, AgentDelegateParams, AgentDelegated,
    AgentForgetParams, AgentForgetPreview, AgentForgotten, AgentForkParams, AgentHookParams,
    AgentKinds, AgentListParams, AgentMarkSeenParams, AgentModeSet, AgentModels, AgentModelsParams,
    AgentOptionsSet, AgentPromptParams, AgentQueueEditParams, AgentQueueItemRef, AgentQueueList,
    AgentQueueParams, AgentRenameParams, AgentResumeParams, AgentRetainParams, AgentSession,
    AgentSessionList, AgentSessionRef, AgentSetModeParams, AgentSetOptionsParams, AgentStartParams,
    AgentStopParams, AgentTree, AgentWatchEvent, AgentWatchParams, BringBackResult,
    CredentialsImportParams, CredentialsImported, CredentialsKindRef, CredentialsRemoved,
    CredentialsShown, GenerateTitleParams, GenerateTitleResult, ImportCandidate, ImportCandidates,
    ImportCandidatesParams, PromptAttachment, PromptReceipt, QueuedPrompt, WorkspaceRemovalError,
};
use massorca_protocol::methods::git::status::GitOperation;
use massorca_protocol::methods::terminals::TerminalState;
use massorca_protocol::{ErrorBody, capabilities, codes};
use tokio::sync::{broadcast, mpsc};

use super::agent_redaction::{for_caller, is_host_admin, watch_item_for};
use super::teams::start_gate;
use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::{NoParams, first_prompt_rename, sees_account_details};
use crate::method::{StreamSource, encode};
use crate::owner::Owner;
use crate::permission::{Permission, Principal, authorize};
use crate::registry::RegistrationHandle;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    let agents_cap = capabilities::AGENTS;
    Ok(vec![
        registries.capabilities.register(agents_cap, Owner::Core)?,
        // Permission modes (decision 0083) ride on the agent methods; the capability tells a client they are there.
        registries
            .capabilities
            .register(capabilities::AGENT_MODES, Owner::Core)?,
        registries
            .capabilities
            .register(capabilities::AGENT_SEEN, Owner::Core)?,
        // The prompt queue (decision 0084) has its own capability, so a client knows the service keeps queued prompts.
        registries
            .capabilities
            .register(capabilities::AGENT_QUEUE, Owner::Core)?,
        // Files of the session's workspace sent with a prompt or a queued prompt (decision 0172).
        registries
            .capabilities
            .register(capabilities::AGENT_PROMPT_ATTACHMENTS, Owner::Core)?,
        // Release and retain of a session, and its archived output (decision 0090).
        registries
            .capabilities
            .register(capabilities::AGENT_RELEASE, Owner::Core)?,
        // A start with no workspace runs in the conversations folder (decision 0094).
        registries
            .capabilities
            .register(capabilities::AGENT_NO_PROJECT, Owner::Core)?,
        // `model` and `effort` on a start (decision 0094).
        registries
            .capabilities
            .register(capabilities::AGENT_OPTIONS, Owner::Core)?,
        // `agent.set_options`: a session's model, effort and fast mode changed while it runs, or at its next resume
        // (decision 0171).
        registries
            .capabilities
            .register(capabilities::AGENT_OPTIONS_LIVE, Owner::Core)?,
        // Fast mode on a start, a kind, a model and a session (decision 0165).
        registries
            .capabilities
            .register(capabilities::AGENT_FAST, Owner::Core)?,
        // `agent.fork` (decision 0168).
        registries
            .capabilities
            .register(capabilities::AGENT_FORK, Owner::Core)?,
        // Import source detection, and an import without a folder (decision 0097).
        registries
            .capabilities
            .register(capabilities::AGENT_IMPORT_DETECT, Owner::Core)?,
        // The models a kind's picker offers for an account (decision 0166).
        registries
            .capabilities
            .register(capabilities::AGENT_MODELS, Owner::Core)?,
        // Archive and delete of a session, and the delete's preview (decision 0163).
        registries
            .capabilities
            .register(capabilities::AGENT_ARCHIVE, Owner::Core)?,
        methods.register(
            Owner::Core,
            agents::ARCHIVE_SESSION,
            capabilities::AGENT_ARCHIVE,
            Permission::Workspace,
            archive_session,
        )?,
        methods.register(
            Owner::Core,
            agents::RENAME,
            capabilities::AGENT_ARCHIVE,
            Permission::Workspace,
            rename,
        )?,
        methods.register(
            Owner::Core,
            agents::FORGET_PREVIEW,
            capabilities::AGENT_ARCHIVE,
            Permission::Read,
            forget_preview,
        )?,
        // Chat titles (decision 0167): `agent.generate_title`, a one-shot run of the kind that names a chat.
        registries
            .capabilities
            .register(capabilities::AGENT_TITLE, Owner::Core)?,
        methods.register(
            Owner::Core,
            agents::KINDS,
            agents_cap,
            Permission::Read,
            kinds,
        )?,
        methods.register(
            Owner::Core,
            agents::MODELS,
            agents_cap,
            Permission::Workspace,
            models,
        )?,
        methods.register(
            Owner::Core,
            agents::START,
            agents_cap,
            Permission::Workspace,
            start,
        )?,
        methods.register(
            Owner::Core,
            agents::LIST,
            agents_cap,
            Permission::Read,
            list,
        )?,
        methods.register(
            Owner::Core,
            agents::SHOW,
            agents_cap,
            Permission::Read,
            show,
        )?,
        methods.register(
            Owner::Core,
            agents::STOP,
            agents_cap,
            Permission::Workspace,
            stop,
        )?,
        methods.register(
            Owner::Core,
            agents::PROMPT,
            agents_cap,
            Permission::Workspace,
            prompt,
        )?,
        methods.register(
            Owner::Core,
            agents::RESUME,
            agents_cap,
            Permission::Workspace,
            resume,
        )?,
        methods.register(
            Owner::Core,
            agents::FORK,
            capabilities::AGENT_FORK,
            Permission::Workspace,
            fork,
        )?,
        methods.register(
            Owner::Core,
            agents::SET_MODE,
            capabilities::AGENT_MODES,
            Permission::Workspace,
            set_mode,
        )?,
        methods.register(
            Owner::Core,
            agents::SET_OPTIONS,
            capabilities::AGENT_OPTIONS_LIVE,
            Permission::Workspace,
            set_options,
        )?,
        methods.register(
            Owner::Core,
            agents::MARK_SEEN,
            capabilities::AGENT_SEEN,
            Permission::Workspace,
            mark_seen,
        )?,
        methods.register(
            Owner::Core,
            agents::FORGET,
            agents_cap,
            Permission::Workspace,
            forget,
        )?,
        methods.register(
            Owner::Core,
            agents::RELEASE,
            capabilities::AGENT_RELEASE,
            Permission::Workspace,
            release,
        )?,
        methods.register(
            Owner::Core,
            agents::RETAIN,
            capabilities::AGENT_RELEASE,
            Permission::Workspace,
            retain,
        )?,
        methods.register(
            Owner::Core,
            agents::ARCHIVE,
            capabilities::AGENT_RELEASE,
            Permission::Read,
            archive,
        )?,
        methods.register(
            Owner::Core,
            agents::HOOK,
            agents_cap,
            Permission::Workspace,
            hook,
        )?,
        methods.register(
            Owner::Core,
            agents::DELEGATE,
            agents_cap,
            Permission::Workspace,
            delegate,
        )?,
        methods.register(
            Owner::Core,
            agents::TREE,
            agents_cap,
            Permission::Read,
            tree,
        )?,
        methods.register(
            Owner::Core,
            agents::BRING_BACK,
            agents_cap,
            Permission::Workspace,
            bring_back,
        )?,
        methods.register(
            Owner::Core,
            agents::QUEUE,
            capabilities::AGENT_QUEUE,
            Permission::Workspace,
            queue,
        )?,
        methods.register(
            Owner::Core,
            agents::QUEUE_LIST,
            capabilities::AGENT_QUEUE,
            Permission::Read,
            queue_list,
        )?,
        methods.register(
            Owner::Core,
            agents::QUEUE_EDIT,
            capabilities::AGENT_QUEUE,
            Permission::Workspace,
            queue_edit,
        )?,
        methods.register(
            Owner::Core,
            agents::QUEUE_REMOVE,
            capabilities::AGENT_QUEUE,
            Permission::Workspace,
            queue_remove,
        )?,
        methods.register(
            Owner::Core,
            agents::QUEUE_SEND_NOW,
            capabilities::AGENT_QUEUE,
            Permission::Workspace,
            queue_send_now,
        )?,
        methods.register(
            Owner::Core,
            agents::QUEUE_RESUME,
            capabilities::AGENT_QUEUE,
            Permission::Workspace,
            queue_resume,
        )?,
        methods.register_stream(
            Owner::Core,
            agents::WATCH,
            agents_cap,
            Permission::Read,
            watch,
        )?,
        methods.register(
            Owner::Core,
            agents::CREDENTIALS_IMPORT,
            agents_cap,
            Permission::Workspace,
            credentials_import,
        )?,
        methods.register(
            Owner::Core,
            agents::CREDENTIALS_SHOW,
            agents_cap,
            Permission::Read,
            credentials_show,
        )?,
        methods.register(
            Owner::Core,
            agents::CREDENTIALS_REMOVE,
            agents_cap,
            Permission::Workspace,
            credentials_remove,
        )?,
        methods.register(
            Owner::Core,
            agents::ACCOUNTS_LIST,
            agents_cap,
            Permission::Read,
            accounts_list,
        )?,
        methods.register(
            Owner::Core,
            agents::ACCOUNTS_ADD,
            agents_cap,
            Permission::Workspace,
            accounts_add,
        )?,
        methods.register(
            Owner::Core,
            agents::ACCOUNTS_SIGN_IN,
            agents_cap,
            Permission::Workspace,
            accounts_sign_in,
        )?,
        methods.register(
            Owner::Core,
            agents::ACCOUNTS_IMPORT,
            agents_cap,
            Permission::Workspace,
            accounts_import,
        )?,
        methods.register(
            Owner::Core,
            agents::ACCOUNTS_IMPORT_CANDIDATES,
            capabilities::AGENT_IMPORT_DETECT,
            Permission::Read,
            accounts_import_candidates,
        )?,
        methods.register(
            Owner::Core,
            agents::ACCOUNTS_ACTIVATE,
            agents_cap,
            Permission::Workspace,
            accounts_activate,
        )?,
        methods.register(
            Owner::Core,
            agents::ACCOUNTS_REMOVE,
            agents_cap,
            Permission::Workspace,
            accounts_remove,
        )?,
        methods.register(
            Owner::Core,
            agents::ACCOUNTS_CHECK,
            agents_cap,
            Permission::Workspace,
            accounts_check,
        )?,
        methods.register(
            Owner::Core,
            agents::GENERATE_TITLE,
            capabilities::AGENT_TITLE,
            Permission::Workspace,
            generate_title,
        )?,
    ])
}

/// Items queued for one watcher before its forwarder waits for the connection to catch up.
const WATCH_QUEUE_CAPACITY: usize = 64;

/// Every declared kind, read again from the profile on each call, with programs resolved on the service's `PATH`.
async fn kinds(ctx: RequestCtx, _params: NoParams) -> Result<AgentKinds, ErrorBody> {
    on_agents(&ctx, |agents| Ok(agents.kinds())).await
}

/// The models a kind's picker offers for an account (decision 0166). The vendor CLI may take seconds to answer, so the
/// listing runs on the service's worker rather than on the connection.
async fn models(ctx: RequestCtx, params: AgentModelsParams) -> Result<AgentModels, ErrorBody> {
    on_agents(&ctx, move |agents| {
        agents.agent_models(&params.kind, params.account.as_deref(), params.refresh)
    })
    .await
}

async fn start(ctx: RequestCtx, params: AgentStartParams) -> Result<AgentSession, ErrorBody> {
    let grant = secret_grant(&ctx);
    let (model, prompt) = (params.model.clone(), params.prompt.clone());
    let session = on_agents(&ctx, move |agents| agents.start(params, grant)).await?;
    first_prompt_rename::note_started(&ctx.state, &session, model, prompt.as_deref());
    Ok(for_caller(&ctx, session))
}

async fn list(ctx: RequestCtx, params: AgentListParams) -> Result<AgentSessionList, ErrorBody> {
    let sessions = on_agents(&ctx, move |agents| {
        agents.list_visible(params.workspace.as_deref(), params.include_archived)
    })
    .await?;
    Ok(AgentSessionList {
        sessions: sessions
            .into_iter()
            .map(|session| for_caller(&ctx, session))
            .collect(),
    })
}

async fn show(ctx: RequestCtx, params: AgentSessionRef) -> Result<AgentSession, ErrorBody> {
    let session = on_agents(&ctx, move |agents| agents.show(&params.session)).await?;
    Ok(for_caller(&ctx, session))
}

async fn stop(ctx: RequestCtx, params: AgentStopParams) -> Result<AgentSession, ErrorBody> {
    let session = on_agents(&ctx, move |agents| {
        agents.stop(&params.session, params.force)
    })
    .await?;
    Ok(for_caller(&ctx, session))
}

/// Marks a session's latest `waiting` or `done` as seen (inventory STAT-19); watchers get the change.
async fn mark_seen(
    ctx: RequestCtx,
    params: AgentMarkSeenParams,
) -> Result<AgentSession, ErrorBody> {
    let session = on_agents(&ctx, move |agents| {
        agents.mark_seen(&params.session, params.at_ms)
    })
    .await?;
    Ok(for_caller(&ctx, session))
}

/// Forgets an exited session; with `remove_workspace`, also removes its workspace (decision 0163). Every check runs
/// before anything changes: the forget's, that no other session uses the workspace, and the removal's own refusals. The
/// session is then forgotten and the workspace removed through `workspace.remove`'s path; a removal that fails after
/// the forget is reported in `workspace_error`, never dropped.
async fn forget(ctx: RequestCtx, params: AgentForgetParams) -> Result<AgentForgotten, ErrorBody> {
    let AgentForgetParams {
        session,
        remove_workspace,
        force,
        discard_ignored,
        expect_dirty,
    } = params;
    let waivers = massorca_core_workspace::RemovalWaivers {
        force,
        discard_ignored,
        ..massorca_core_workspace::RemovalWaivers::NONE
    };
    let expect_clean = expect_dirty == Some(false);
    let workspace = if remove_workspace {
        let selector = session.clone();
        Some(
            on_agents(&ctx, move |agents| {
                agents.check_forget_with_workspace(&selector, waivers, expect_clean)
            })
            .await?,
        )
    } else {
        None
    };
    let forgotten = on_agents(&ctx, move |agents| agents.forget(&session)).await?;
    let mut result = AgentForgotten {
        session: for_caller(&ctx, forgotten),
        workspace_removed: None,
        workspace_error: None,
    };
    if let Some(workspace) = workspace {
        // Checked again where no start, resume or terminal can interleave; a refusal now lands in `workspace_error`.
        let begun = on_agents(&ctx, move |agents| {
            agents.begin_workspace_removal(&workspace, waivers, expect_clean)
        })
        .await;
        let removed = match begun {
            Ok(begun) => super::workspaces::finish_begun_removal(&ctx, begun.id, waivers).await,
            Err(error) => Err(error),
        };
        match removed {
            Ok(removed) => result.workspace_removed = Some(removed),
            Err(error) => {
                tracing::warn!(code = %error.code, "a forgotten session's workspace was not removed");
                result.workspace_error = Some(WorkspaceRemovalError {
                    code: error.code,
                    message: error.message,
                });
            }
        }
    }
    Ok(result)
}

/// Sets a live or exited session's title (decision 0163).
async fn rename(ctx: RequestCtx, params: AgentRenameParams) -> Result<AgentSession, ErrorBody> {
    let session = on_agents(&ctx, move |agents| {
        agents.rename(&params.session, &params.title)
    })
    .await?;
    Ok(for_caller(&ctx, session))
}

/// Archives (hides) an exited session, or clears that (decision 0163).
async fn archive_session(
    ctx: RequestCtx,
    params: AgentArchiveSessionParams,
) -> Result<AgentSession, ErrorBody> {
    let session = on_agents(&ctx, move |agents| {
        agents.set_archived(&params.session, params.archived)
    })
    .await?;
    Ok(for_caller(&ctx, session))
}

/// What `agent.forget` with `remove_workspace` would meet, read without changing anything (decision 0163). A caller
/// below `workspace` scope gets no path, and a fixed blocker message with the blocker's code, since a refusal (a git
/// error among them) can name paths.
async fn forget_preview(
    ctx: RequestCtx,
    params: AgentSessionRef,
) -> Result<AgentForgetPreview, ErrorBody> {
    let mut preview = on_agents(&ctx, move |agents| agents.forget_preview(&params.session)).await?;
    if authorize(&ctx.principal, Permission::Workspace).is_err() {
        preview.workspace.path = None;
        if preview.workspace.blocker.is_some() {
            preview.workspace.blocker =
                Some("the workspace cannot be removed now; blocker_code says why".to_owned());
        }
        if preview.forget_blocker.is_some() {
            preview.forget_blocker = Some("the session cannot be forgotten now".to_owned());
        }
    }
    Ok(preview)
}

/// Releases an exited session: its output is archived, its terminal freed, its record kept (decision 0090).
async fn release(ctx: RequestCtx, params: AgentSessionRef) -> Result<AgentSession, ErrorBody> {
    let session = on_agents(&ctx, move |agents| agents.release(&params.session)).await?;
    Ok(for_caller(&ctx, session))
}

/// Sets or clears a session's durable retain (decision 0090).
async fn retain(ctx: RequestCtx, params: AgentRetainParams) -> Result<AgentSession, ErrorBody> {
    let session = on_agents(&ctx, move |agents| {
        agents.set_retained(&params.session, params.retained)
    })
    .await?;
    Ok(for_caller(&ctx, session))
}

/// The archived output of a released session. A run that holds vault secrets shows its text to host-admin callers only,
/// as its result is (decision 0071).
async fn archive(ctx: RequestCtx, params: AgentSessionRef) -> Result<AgentArchive, ErrorBody> {
    let (session, output) =
        on_agents(&ctx, move |agents| agents.archived_output(&params.session)).await?;
    let withheld = session.holds_secrets && !is_host_admin(&ctx);
    Ok(AgentArchive {
        session: session.id,
        released: session.released,
        text: match (&output, withheld) {
            (Some(output), false) => Some(output.text.clone()),
            _ => None,
        },
        truncated: !withheld && output.as_ref().is_some_and(|output| output.truncated),
        withheld: withheld && output.is_some(),
    })
}

/// Starts a new run of an exited session, which keeps its id (0054), in the mode decision 0083 gives it.
async fn resume(ctx: RequestCtx, params: AgentResumeParams) -> Result<AgentSession, ErrorBody> {
    require_admin_for_session(&ctx, params.session.clone()).await?;
    let grant = secret_grant(&ctx);
    let session = on_agents(&ctx, move |agents| {
        // A chat resume probes its CLI's version, which can take seconds: before the gate (decision 0101).
        agents.prepare_chat_resume(&params.session);
        // A resume can revive a task's session, so it waits for task starts and for done, reopen and bot changes.
        let _gate = start_gate();
        agents.resume_at(
            &params.session,
            params.permission_mode.as_deref(),
            grant,
            params.cols,
            params.rows,
        )
    })
    .await?;
    Ok(for_caller(&ctx, session))
}

/// Forks an exited session into a new one (decision 0168). The fork copies the source's conversation, so a source holding
/// vault secrets needs host-admin scope, as its resume does.
async fn fork(ctx: RequestCtx, params: AgentForkParams) -> Result<AgentSession, ErrorBody> {
    require_admin_for_session(&ctx, params.session.clone()).await?;
    let grant = secret_grant(&ctx);
    let session = on_agents(&ctx, move |agents| {
        // As a start, the fork waits for task starts and for done, reopen and bot changes.
        let _gate = start_gate();
        agents.fork(params, grant)
    })
    .await?;
    Ok(for_caller(&ctx, session))
}

/// Switches a session's permission mode, live or at its next resume (decision 0083). A live switch types into the run's
/// terminal, so a run holding vault secrets needs host-admin scope, as a prompt does. A bypass mode needs no more than
/// the method's own permission.
async fn set_mode(ctx: RequestCtx, params: AgentSetModeParams) -> Result<AgentModeSet, ErrorBody> {
    require_admin_for_session(&ctx, params.session.clone()).await?;
    let mut set = on_agents(&ctx, move |agents| {
        agents.set_mode(&params.session, &params.permission_mode, params.force)
    })
    .await?;
    set.session = for_caller(&ctx, set.session);
    Ok(set)
}

/// The workspace paths of a prompt's attachments, in the order the caller gave them (decision 0172).
fn attachment_paths(attachments: Vec<PromptAttachment>) -> Vec<String> {
    attachments.into_iter().map(|file| file.path).collect()
}

/// Changes a session's model, effort or fast mode (decision 0171): typed into a live run where the kind declares a live
/// command for it, otherwise at the session's next resume. A live change types into the run's terminal, so a run holding
/// vault secrets needs host-admin scope, as a mode switch does.
async fn set_options(
    ctx: RequestCtx,
    params: AgentSetOptionsParams,
) -> Result<AgentOptionsSet, ErrorBody> {
    require_admin_for_session(&ctx, params.session.clone()).await?;
    let mut set = on_agents(&ctx, move |agents| agents.set_options(&params)).await?;
    set.session = for_caller(&ctx, set.session);
    Ok(set)
}

/// Types a prompt into a run's terminal (0054), with the files it names (decision 0172). The text is never logged.
async fn prompt(ctx: RequestCtx, params: AgentPromptParams) -> Result<PromptReceipt, ErrorBody> {
    require_admin_for_session(&ctx, params.session.clone()).await?;
    let files = attachment_paths(params.attachments);
    on_agents(&ctx, move |agents| {
        agents.prompt_with_attachments(&params.session, &params.text, params.force, &files)
    })
    .await
}

/// Queues a prompt behind a session's busy agent (decision 0084), with the files it names (decision 0172). A queued
/// prompt needs host-admin scope exactly where a prompt does: its run holds vault secrets. The text is never logged.
async fn queue(ctx: RequestCtx, params: AgentQueueParams) -> Result<QueuedPrompt, ErrorBody> {
    require_admin_for_session(&ctx, params.session.clone()).await?;
    // The caller's scope is kept with the prompt, so a run that later holds secrets types only prompts an admin queued.
    let host_admin = is_host_admin(&ctx);
    let files = attachment_paths(params.attachments);
    on_agents(&ctx, move |agents| {
        agents.queue_add_with_attachments(
            &params.session,
            &params.text,
            params.request_id.as_deref(),
            host_admin,
            &files,
        )
    })
    .await
}

/// A session's queued prompts, head first (decision 0084).
async fn queue_list(ctx: RequestCtx, params: AgentSessionRef) -> Result<AgentQueueList, ErrorBody> {
    require_admin_for_session(&ctx, params.session.clone()).await?;
    on_agents(&ctx, move |agents| agents.queue_list(&params.session)).await
}

/// Edits a queued prompt's text (decision 0084). The admin check follows the prompt's session.
async fn queue_edit(
    ctx: RequestCtx,
    params: AgentQueueEditParams,
) -> Result<QueuedPrompt, ErrorBody> {
    let id = params.id.clone();
    let session = on_agents(&ctx, move |agents| agents.queue_session_of(&id)).await?;
    require_admin_for_session(&ctx, session).await?;
    // An edit makes the text the caller's, so the prompt takes the caller's scope, as a queued prompt does.
    let host_admin = is_host_admin(&ctx);
    on_agents(&ctx, move |agents| {
        agents.queue_edit(&params.id, &params.text, host_admin)
    })
    .await
}

/// Removes a queued prompt (decision 0084). The admin check follows the prompt's session.
async fn queue_remove(
    ctx: RequestCtx,
    params: AgentQueueItemRef,
) -> Result<QueuedPrompt, ErrorBody> {
    let id = params.id.clone();
    let session = on_agents(&ctx, move |agents| agents.queue_session_of(&id)).await?;
    require_admin_for_session(&ctx, session).await?;
    on_agents(&ctx, move |agents| agents.queue_remove(&params.id)).await
}

/// Sends a queued prompt at once, with force, skipping the busy hold and the queue's order (decision 0084). The admin
/// check follows the prompt's session.
async fn queue_send_now(
    ctx: RequestCtx,
    params: AgentQueueItemRef,
) -> Result<QueuedPrompt, ErrorBody> {
    let id = params.id.clone();
    let session = on_agents(&ctx, move |agents| agents.queue_session_of(&id)).await?;
    require_admin_for_session(&ctx, session).await?;
    on_agents(&ctx, move |agents| agents.queue_send_now(&params.id)).await
}

/// Lifts a session's queue pauses and delivers its head if the agent is ready (decision 0084).
async fn queue_resume(
    ctx: RequestCtx,
    params: AgentSessionRef,
) -> Result<AgentQueueList, ErrorBody> {
    require_admin_for_session(&ctx, params.session.clone()).await?;
    on_agents(&ctx, move |agents| agents.queue_resume(&params.session)).await
}

/// A report from a run's hook (0053). The payload, the result and the token are never logged.
async fn hook(ctx: RequestCtx, params: AgentHookParams) -> Result<(), ErrorBody> {
    on_agents(&ctx, move |agents| agents.hook(&params)).await
}

/// Starts a child session of a parent session (0055). The prompt is never logged.
async fn delegate(
    ctx: RequestCtx,
    params: AgentDelegateParams,
) -> Result<AgentDelegated, ErrorBody> {
    require_admin_for_session(&ctx, params.parent.clone()).await?;
    let grant = secret_grant(&ctx);
    let mut delegated = on_agents(&ctx, move |agents| agents.delegate(params, grant)).await?;
    delegated.session = for_caller(&ctx, delegated.session);
    Ok(delegated)
}

/// A session's delegation tree: its root and the root's children (0055).
async fn tree(ctx: RequestCtx, params: AgentSessionRef) -> Result<AgentTree, ErrorBody> {
    let tree = on_agents(&ctx, move |agents| agents.tree(&params.session)).await?;
    Ok(AgentTree {
        root: for_caller(&ctx, tree.root),
        children: tree
            .children
            .into_iter()
            .map(|child| for_caller(&ctx, child))
            .collect(),
    })
}

/// Brings a finished child back into its parent (0055). The parent workspace's git mutation lock is held for the whole
/// call, so no git command of a client interleaves with the clean check and the merge.
async fn bring_back(
    ctx: RequestCtx,
    params: AgentBringBackParams,
) -> Result<BringBackResult, ErrorBody> {
    // A bring-back reads the child's result and types into the parent's run.
    require_admin_for_session(&ctx, params.child.clone()).await?;
    let parent = on_agents(&ctx, {
        let child = params.child.clone();
        move |agents| agents.show(&child)
    })
    .await?
    .parent_id;
    if let Some(parent) = parent {
        require_admin_for_session(&ctx, parent).await?;
    }
    let child = params.child.clone();
    let merge = params.merge;
    // The workspace a merge would go into: only a child with a workspace of its own is merged (0055).
    let merge_into = on_agents(&ctx, move |agents| {
        let child = agents.show(&child)?;
        let Some(parent) = child.parent_id else {
            return Ok(None);
        };
        let parent = agents.show(&parent)?;
        Ok((merge && parent.workspace_id != child.workspace_id).then_some(parent.workspace_id))
    })
    .await?;
    // A merge never runs unlocked: a workspace that is not a git worktree, is missing or is being removed refuses the
    // bring-back here.
    let _guard = match merge_into {
        Some(workspace) => Some(super::git::repository(&ctx, workspace).await?.lock().await),
        None => None,
    };
    let git = WorktreeGit(ctx.state.workspaces.git().clone());
    let mut result = on_agents(&ctx, move |agents| agents.bring_back(&params, &git)).await?;
    result.child = for_caller(&ctx, result.child);
    Ok(result)
}

/// The bring-back's git operations, through `core-git`.
struct WorktreeGit(Git);

impl RepoOps for WorktreeGit {
    fn is_dirty(&self, dir: &Path) -> Result<bool, AgentError> {
        self.0.status_dirty(dir).map_err(agent_git_error)
    }

    fn operation_in_progress(&self, dir: &Path) -> Result<Option<String>, AgentError> {
        let operation = self.0.operation_in_progress(dir).map_err(agent_git_error)?;
        Ok(operation.map(|operation| {
            match operation {
                GitOperation::Merge => "a merge",
                GitOperation::Rebase => "a rebase",
                GitOperation::CherryPick => "a cherry-pick",
                GitOperation::Revert => "a revert",
                GitOperation::ApplyMailbox => "a mailbox apply",
                GitOperation::Unknown => "a git operation",
            }
            .to_owned()
        }))
    }

    fn merge_no_ff(&self, dir: &Path, branch: &str) -> Result<MergeResult, AgentError> {
        Ok(
            match self.0.merge_no_ff(dir, branch).map_err(agent_git_error)? {
                MergeOutcome::Merged { commit } => MergeResult::Merged { commit },
                MergeOutcome::Conflict { paths } => MergeResult::Conflict { paths },
            },
        )
    }
}

/// A git failure as the protocol reports it, as `git_error` maps it for the git methods.
fn agent_git_error(error: GitError) -> AgentError {
    match massorca_core_git::failure_code(&error) {
        Some(code) => AgentError::new(code, error.to_string()),
        None => DomainError::from(error).into(),
    }
}

/// The sessions a watcher selected, as ids resolved once when the stream opens.
#[derive(Debug, Clone)]
struct Selection {
    workspace_id: Option<String>,
    session_id: Option<String>,
}

impl Selection {
    fn resolve(agents: &Agents, params: &AgentWatchParams) -> Result<Self, AgentError> {
        let workspace_id = match &params.workspace {
            Some(selector) => Some(agents.workspace_id(selector)?),
            None => None,
        };
        let session_id = match &params.session {
            Some(selector) => Some(agents.show(selector)?.id),
            None => None,
        };
        Ok(Self {
            workspace_id,
            session_id,
        })
    }

    fn selects(&self, session: &AgentSession) -> bool {
        self.workspace_id
            .as_ref()
            .is_none_or(|id| *id == session.workspace_id)
            && self.session_id.as_ref().is_none_or(|id| *id == session.id)
    }

    /// Every selected session now, newest first.
    fn snapshot(&self, agents: &Agents) -> Result<AgentWatchEvent, AgentError> {
        let sessions = agents
            .list(self.workspace_id.as_deref())?
            .into_iter()
            .filter(|session| self.selects(session))
            .collect();
        Ok(AgentWatchEvent::Snapshot { sessions })
    }
}

/// Streams the selected sessions: a `snapshot` first, then a `changed` item after every change, in the order the store
/// applied them. The subscription is taken before the snapshot, so no change falls between; a watcher that falls
/// behind is given a fresh `snapshot` (0052).
async fn watch(ctx: RequestCtx, params: AgentWatchParams) -> Result<StreamSource, ErrorBody> {
    let mut changes = ctx.state.agent_changes.subscribe();
    let (selection, first) = on_agents(&ctx, move |agents| {
        let selection = Selection::resolve(agents, &params)?;
        let first = selection.snapshot(agents)?;
        Ok((selection, first))
    })
    .await?;
    let (items, source) = mpsc::channel(WATCH_QUEUE_CAPACITY);
    let agents = ctx.state.agents.clone();
    let admin = is_host_admin(&ctx);
    tokio::spawn(async move {
        let mut next = Some(first);
        loop {
            if let Some(item) = next.take() {
                let item = watch_item_for(admin, item);
                let Ok(value) = encode(&item) else { break };
                if items.send(value).await.is_err() {
                    break;
                }
            }
            let change = tokio::select! {
                () = items.closed() => break,
                change = changes.recv() => change,
            };
            next = match change {
                Ok(Change::Updated(session)) if selection.selects(&session) => {
                    Some(AgentWatchEvent::Changed { session })
                }
                Ok(_) => None,
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let agents = agents.clone();
                    let selection = selection.clone();
                    match tokio::task::spawn_blocking(move || selection.snapshot(&agents)).await {
                        Ok(Ok(snapshot)) => Some(snapshot),
                        _ => break,
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
            };
        }
    });
    Ok(source)
}

/// Copies a kind's declared credential files into the profile's home for it (0056). Only names and sizes come back;
/// nothing about the files is logged beyond that.
/// Deprecated (0056, decision 0059): imports into the kind's active account. Refused while a run of the kind is live
/// when it replaces the account.
async fn credentials_import(
    ctx: RequestCtx,
    params: CredentialsImportParams,
) -> Result<CredentialsImported, ErrorBody> {
    on_agents(&ctx, move |agents| {
        if params.replace {
            refuse_live_runs(agents, &params.kind)?;
        }
        agents.credentials_import(&params.kind, &params.from, params.replace)
    })
    .await
}

/// Deprecated (0056): the kind's active account's files, names and sizes only. Below `workspace` the home is left
/// empty and a refusal keeps its code but not its message, which may name the home ([`sees_account_details`]).
async fn credentials_show(
    ctx: RequestCtx,
    params: CredentialsKindRef,
) -> Result<CredentialsShown, ErrorBody> {
    let kind = params.kind.clone();
    let shown = on_agents(&ctx, move |agents| agents.credentials_show(&params.kind)).await;
    if sees_account_details(&ctx.principal) {
        return shown;
    }
    match shown {
        Ok(mut shown) => {
            shown.home = String::new();
            Ok(shown)
        }
        Err(error) => Err(ErrorBody::new(
            error.code,
            format!("the credential home of agent kind {kind} cannot be shown"),
        )),
    }
}

/// Deprecated (0056): removes the kind's active account. Refused while a run of the kind is live.
async fn credentials_remove(
    ctx: RequestCtx,
    params: CredentialsKindRef,
) -> Result<CredentialsRemoved, ErrorBody> {
    on_agents(&ctx, move |agents| {
        refuse_live_runs(agents, &params.kind)?;
        agents.credentials_remove(&params.kind)
    })
    .await
}

/// The accounts of a kind (decision 0059).
async fn accounts_list(
    ctx: RequestCtx,
    params: AccountsListParams,
) -> Result<AccountsList, ErrorBody> {
    let accounts = on_agents(&ctx, move |agents| agents.accounts_list(&params.kind)).await?;
    Ok(AccountsList {
        accounts: accounts
            .into_iter()
            .map(|account| account_for_caller(&ctx.principal, account))
            .collect(),
    })
}

/// Sign-in: a new account and the terminal that runs the kind's login (decision 0059).
async fn accounts_add(
    ctx: RequestCtx,
    params: AccountsAddParams,
) -> Result<AccountsAdded, ErrorBody> {
    let mut added = on_agents(&ctx, move |agents| {
        agents.accounts_add(&params.kind, params.label)
    })
    .await?;
    added.account = account_for_caller(&ctx.principal, added.account);
    Ok(added)
}

/// Signs an existing account in again (decision 0059). The same caller rules as the sign-in of a new account apply.
async fn accounts_sign_in(
    ctx: RequestCtx,
    params: AccountsSignInParams,
) -> Result<AccountsAdded, ErrorBody> {
    let mut added = on_agents(&ctx, move |agents| {
        agents.accounts_sign_in(&params.kind, &params.account)
    })
    .await?;
    added.account = account_for_caller(&ctx.principal, added.account);
    Ok(added)
}

/// Import into a new account from a directory the person names (decision 0059), or without one from where the kind's
/// CLI keeps each file (decision 0097).
async fn accounts_import(
    ctx: RequestCtx,
    params: AccountsImportParams,
) -> Result<AccountsImported, ErrorBody> {
    let mut imported = on_agents(&ctx, move |agents| {
        agents.accounts_import(&params.kind, params.from.as_deref(), params.label)
    })
    .await?;
    imported.account = imported
        .account
        .map(|account| account_for_caller(&ctx.principal, account));
    Ok(imported)
}

async fn accounts_activate(
    ctx: RequestCtx,
    params: AccountsActivateParams,
) -> Result<AccountSummary, ErrorBody> {
    let account = on_agents(&ctx, move |agents| {
        agents.accounts_activate(&params.kind, &params.account, params.project.as_deref())
    })
    .await?;
    Ok(account_for_caller(&ctx.principal, account))
}

async fn accounts_remove(
    ctx: RequestCtx,
    params: AccountsRemoveParams,
) -> Result<AccountsRemoved, ErrorBody> {
    on_agents(&ctx, move |agents| {
        agents.accounts_remove(&params.kind, &params.account)
    })
    .await
}

async fn accounts_check(
    ctx: RequestCtx,
    params: AccountsCheckParams,
) -> Result<AccountSummary, ErrorBody> {
    let account = on_agents(&ctx, move |agents| {
        agents.accounts_check(&params.kind, params.account.as_deref())
    })
    .await?;
    Ok(account_for_caller(&ctx.principal, account))
}

/// `agent.generate_title`: names a chat from its first prompt and reply by one run of the kind's one-shot form, with the
/// time limit of [`title_timeout`] (decision 0167). A caller below host-admin may name only the kind's active account.
async fn generate_title(
    ctx: RequestCtx,
    params: GenerateTitleParams,
) -> Result<GenerateTitleResult, ErrorBody> {
    let host_admin = is_host_admin(&ctx);
    if !host_admin && params.account.is_some() {
        let kind = params.kind.clone();
        let accounts = on_agents(&ctx, move |agents| agents.accounts_list(&kind)).await?;
        let active = accounts
            .iter()
            .find(|account| account.active)
            .map(|account| account.id.as_str());
        if let Some(refusal) = title_account_refusal(host_admin, params.account.as_deref(), active)
        {
            return Err(refusal);
        }
    }
    let request = TitleRequest {
        kind: params.kind,
        account: params.account,
        model: params.model,
        prompt: params.prompt,
        reply: params.reply,
    };
    let slug = on_agents(&ctx, move |agents| {
        agents.generate_title(request, title_timeout())
    })
    .await?;
    Ok(GenerateTitleResult { slug })
}

/// Whether a caller may name `requested` as the account of a title run (decision 0167). Host-admin may name any account. Below
/// it, only the kind's `active` account: the method takes no project, so no project's own choice is reachable. No account
/// named means the active account runs, which every caller may have.
pub(super) fn title_account_refusal(
    host_admin: bool,
    requested: Option<&str>,
    active: Option<&str>,
) -> Option<ErrorBody> {
    match requested {
        None => None,
        Some(_) if host_admin => None,
        Some(requested) if Some(requested) == active => None,
        Some(_) => Some(ErrorBody::new(
            codes::PERMISSION_DENIED,
            "only the kind's active account may name a chat for this caller",
        )),
    }
}

/// Where an import without a folder would copy each kind's files from (decision 0097). Metadata only; host paths only
/// for a caller that sees account details ([`sees_account_details`]).
async fn accounts_import_candidates(
    ctx: RequestCtx,
    params: ImportCandidatesParams,
) -> Result<ImportCandidates, ErrorBody> {
    let candidates = on_agents(&ctx, move |agents| {
        agents.import_candidates(params.kind.as_deref())
    })
    .await?;
    let details = sees_account_details(&ctx.principal);
    Ok(ImportCandidates {
        candidates: candidates
            .into_iter()
            .map(|candidate| candidate_for_caller(details, candidate))
            .collect(),
    })
}

/// A candidate as a caller may see it: without its folder, paths and override variables when it does not see account
/// details.
fn candidate_for_caller(details: bool, mut candidate: ImportCandidate) -> ImportCandidate {
    if !details {
        candidate.folder = None;
        for file in &mut candidate.files {
            file.path = None;
            file.env = None;
        }
    }
    candidate
}

/// An account as `principal` may see it: without its identity below `workspace` ([`sees_account_details`]). Every
/// `agent.accounts.*` answer that carries an account passes through here.
fn account_for_caller(principal: &Principal, mut account: AccountSummary) -> AccountSummary {
    if !sees_account_details(principal) {
        account.identity = None;
    }
    account
}

/// Refuses with `agent.busy` while any session of `kind` has a run that has not exited: replacing or removing its home
/// would pull its login away under it.
fn refuse_live_runs(agents: &Agents, kind: &str) -> Result<(), AgentError> {
    let live: Vec<String> = agents
        .list(None)?
        .into_iter()
        .filter(|session| session.kind == kind && session.execution.state != TerminalState::Exited)
        .map(|session| session.id)
        .collect();
    if live.is_empty() {
        return Ok(());
    }
    let mut error = AgentError::new(
        codes::AGENT_BUSY,
        format!(
            "a run of agent kind {kind} is still live (session {}); stop it before replacing or removing its credentials",
            live.join(", ")
        ),
    );
    error.data = Some(serde_json::json!({ "sessions": live }));
    Err(error)
}

/// Runs one agents call on the blocking pool and maps its refusal to the protocol error.
/// Whether a run started by this caller receives the vault's variables: only for a host-admin caller (decision 0071).
pub(crate) fn secret_grant(ctx: &RequestCtx) -> SecretGrant {
    if is_host_admin(ctx) {
        SecretGrant::Provide
    } else {
        SecretGrant::Withhold("vault secrets go only to runs started by a host-admin caller")
    }
}

/// Refuses a caller below host-admin scope when `session`'s run holds vault secrets (decision 0071).
pub(super) async fn require_admin_for_session(
    ctx: &RequestCtx,
    session: String,
) -> Result<(), ErrorBody> {
    if authorize(&ctx.principal, Permission::HostAdmin).is_ok() {
        return Ok(());
    }
    let holds = on_agents(ctx, move |agents| Ok(agents.show(&session)?.holds_secrets)).await?;
    if holds {
        return authorize(&ctx.principal, Permission::HostAdmin);
    }
    Ok(())
}

pub(super) async fn on_agents<T, F>(ctx: &RequestCtx, call: F) -> Result<T, ErrorBody>
where
    T: Send + 'static,
    F: FnOnce(&Agents) -> Result<T, AgentError> + Send + 'static,
{
    let agents = ctx.state.agents.clone();
    match tokio::task::spawn_blocking(move || call(&agents)).await {
        Ok(result) => result.map_err(ErrorBody::from),
        Err(error) => Err(ErrorBody::new(
            codes::INTERNAL,
            format!("the agents task failed: {error}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::agents::{CandidateFile, ImportCandidate};

    use massorca_protocol::codes;

    use super::{candidate_for_caller, title_account_refusal};

    fn detected() -> ImportCandidate {
        ImportCandidate {
            kind: "fake".to_owned(),
            ready: true,
            folder: Some("<home>/.fakevendor".to_owned()),
            files: vec![CandidateFile {
                name: "token.fake".to_owned(),
                required: true,
                found: true,
                path: Some("<home>/.fakevendor/token.fake".to_owned()),
                env: Some("FAKE_VENDOR_DIR".to_owned()),
                reason: None,
            }],
        }
    }

    #[test]
    fn a_caller_below_workspace_sees_no_folder_path_or_override_variable() {
        assert_eq!(candidate_for_caller(true, detected()), detected());
        let masked = candidate_for_caller(false, detected());
        assert_eq!(masked.folder, None);
        assert_eq!(masked.files[0].path, None);
        assert_eq!(masked.files[0].env, None);
        assert!(masked.ready && masked.files[0].found && masked.files[0].required);
    }

    #[test]
    fn below_host_admin_a_title_may_name_only_the_active_account() {
        assert_eq!(
            title_account_refusal(true, Some("other"), Some("active")),
            None
        );
        assert_eq!(title_account_refusal(false, None, Some("active")), None);
        assert_eq!(
            title_account_refusal(false, Some("active"), Some("active")),
            None
        );
        let refused = title_account_refusal(false, Some("other"), Some("active"))
            .expect("a caller below host-admin is refused another account");
        assert_eq!(refused.code, codes::PERMISSION_DENIED);
        let no_active = title_account_refusal(false, Some("other"), None)
            .expect("nothing is active, so no named account is the active one");
        assert_eq!(no_active.code, codes::PERMISSION_DENIED);
    }
}
