//! The Source control panel's Fetch, Pull and Push calls (GIT-19): each sends one call for the workspace
//! the panel shows, and its answer is a note under the panel or the refusal the service gave. The buttons
//! are drawn by the Project sidebar.

use gpui::Context;
use massorca_protocol::methods::git::remote::{
    GitFetchParams, GitPullParams, GitPushParams, PullMode,
};

use super::Shell;
use crate::model::source_control_sync::{SyncAction, SyncNote, fetched, pulled, pushed};

impl Shell {
    /// Runs one remote action for the workspace the panel shows. One call is out at a time.
    pub(super) fn run_sync(&mut self, action: SyncAction, cx: &mut Context<Self>) {
        let (Some(workspace), Some(handle)) =
            (self.source_control.workspace.clone(), self.service.clone())
        else {
            return;
        };
        if self.source_control.busy {
            return;
        }
        self.source_control.busy = true;
        self.source_control.error = None;
        self.source_control.notice = None;
        cx.notify();
        let answer_for = workspace.clone();
        cx.spawn(async move |this, cx| {
            let result = match action {
                SyncAction::Fetch => handle
                    .git_fetch(fetch_params(workspace))
                    .await
                    .map(|answer| Ok(fetched(&answer))),
                SyncAction::Pull => handle
                    .git_pull(pull_params(workspace))
                    .await
                    .map(|answer| Ok(pulled(&answer))),
                SyncAction::Push => handle
                    .git_push(push_params(workspace))
                    .await
                    .map(|answer| pushed(&answer)),
            }
            .unwrap_or_else(|refusal| Err(refusal.message()));
            this.update(cx, |shell, cx| shell.finish_sync(&answer_for, result, cx))
                .ok();
        })
        .detach();
    }

    /// A sync call's answer. It shows only while the panel still shows the workspace it was asked for.
    fn finish_sync(
        &mut self,
        workspace: &str,
        result: Result<SyncNote, String>,
        cx: &mut Context<Self>,
    ) {
        self.source_control.busy = false;
        if self.source_control.workspace.as_deref() == Some(workspace) {
            match result {
                Ok(note) => self.source_control.notice = Some(note),
                Err(message) => self.source_control.error = Some(message),
            }
        }
        cx.notify();
    }
}

/// The default remote's fetch: no remote named and not every remote.
fn fetch_params(workspace: String) -> GitFetchParams {
    GitFetchParams {
        workspace,
        remote: None,
        all: false,
        timeout_ms: None,
    }
}

/// The pull with git's merge strategy, which pv96 uses by default.
fn pull_params(workspace: String) -> GitPullParams {
    GitPullParams {
        workspace,
        mode: PullMode::Merge,
        timeout_ms: None,
    }
}

/// The push to the branch's upstream, creating it on origin when there is none, never forced.
fn push_params(workspace: String) -> GitPushParams {
    GitPushParams {
        workspace,
        remote: None,
        force_with_lease: None,
        timeout_ms: None,
    }
}
