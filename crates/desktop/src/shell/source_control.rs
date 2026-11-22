//! The Source control panel's state and the calls it makes (screen D-DF-02, decisions 0010 and 0012):
//! which workspace the panel follows, the watch that keeps its status current, and the stage,
//! unstage and commit calls. The panel reads the service's answers from the store; these flows only
//! hold what the panel asked for and what the service refused.

use std::rc::Rc;

use gpui::{Context, Entity, Subscription, Window};
use massorca_protocol::methods::git::ChangeKind;
use massorca_protocol::methods::git::diff::{DiffTarget, GitDiffFile, GitDiffFileParams};
use massorca_protocol::methods::git::index::GitPathsParams;
use massorca_ui::components::diff_view::{DiffModel, DiffStatus};
use massorca_ui::components::text_field::TextField;

use super::Shell;
use crate::model::regions::Region;
use crate::model::source_control_diff::{diff_status, file_status, parse_patch};
use crate::model::source_control_flow::{CommitForm, DiscardFlow, DiscardStage, DiscardTarget};
use crate::service::Refusal;

/// The Project sidebar's tabs in their fixed order (ProjectSidebarTabs.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProjectTab {
    Files,
    Sessions,
    SourceControl,
    Checks,
}

impl ProjectTab {
    const ALL: [Self; 4] = [
        Self::Files,
        Self::Sessions,
        Self::SourceControl,
        Self::Checks,
    ];

    pub(super) fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|tab| *tab == self)
            .unwrap_or_default()
    }

    pub(super) fn at(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Files => "Files",
            Self::Sessions => "Sessions",
            Self::SourceControl => "Source control",
            Self::Checks => "Checks",
        }
    }
}

/// A file the Canvas shows the diff of: its path, the side of the change, and the kind of change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct OpenFile {
    pub path: String,
    pub target: DiffTarget,
    pub original_path: Option<String>,
    pub kind: ChangeKind,
}

/// The diff of the open file, as the Canvas draws it.
#[derive(Debug)]
pub(super) enum DiffLoad {
    Loading,
    Failed(String),
    Ready {
        model: Rc<DiffModel>,
        status: DiffStatus,
        truncated: bool,
    },
}

/// The panel's own state. The store holds the service's answers; this holds what the panel asked
/// for and what it waits on.
#[derive(Debug)]
pub(super) struct SourceControl {
    pub tab: ProjectTab,
    /// The workspace the bridge follows for the open panel, if any.
    pub watching: Option<String>,
    /// The workspace the panel's state belongs to. Another selection resets it.
    pub workspace: Option<String>,
    pub commit: CommitForm,
    /// The message field. Its text is the commit form's message.
    pub commit_field: Entity<TextField>,
    /// Set after a commit, so the next frame (which has the window) empties the field.
    pub clear_field: bool,
    /// A stage or unstage call is out, and its controls wait for the answer.
    pub busy: bool,
    /// What the last stage or unstage refused, under the panel's header.
    pub error: Option<String>,
    /// The file whose diff the Canvas shows, and the diff it is showing.
    pub open: Option<OpenFile>,
    pub diff: Option<DiffLoad>,
    /// Counts diff requests, so an answer for a file that is no longer open is dropped.
    diff_request: u64,
    /// The discard dialog's flow, while the dialog is open.
    pub discard: Option<DiscardFlow>,
    _field_changes: Subscription,
}

impl SourceControl {
    pub(super) fn new(commit_field: Entity<TextField>, field_changes: Subscription) -> Self {
        Self {
            tab: ProjectTab::Files,
            watching: None,
            workspace: None,
            commit: CommitForm::default(),
            commit_field,
            clear_field: false,
            busy: false,
            error: None,
            open: None,
            diff: None,
            diff_request: 0,
            discard: None,
            _field_changes: field_changes,
        }
    }
}

impl Shell {
    /// Opens the Project sidebar on its Source control tab: the start page's "Source control" action.
    pub(super) fn open_source_control(&mut self, cx: &mut Context<Self>) {
        if !self.model.region_visible(Region::ProjectSidebar) {
            self.model.toggle_region(Region::ProjectSidebar);
        }
        self.source_control.tab = ProjectTab::SourceControl;
        cx.notify();
    }

    pub(super) fn select_project_tab(&mut self, tab: ProjectTab, cx: &mut Context<Self>) {
        self.source_control.tab = tab;
        cx.notify();
    }

    /// Brings the panel's state and its watch in line with the selection and the layout. It runs on
    /// each frame and sends a call only when something changed.
    pub(super) fn sync_source_control(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self.model.selected_workspace().map(str::to_owned);
        if self.source_control.workspace != selected {
            self.source_control.workspace = selected.clone();
            self.source_control.error = None;
            // A message typed for one workspace is not carried to another; the spec keeps no draft.
            self.source_control.commit = CommitForm::default();
            self.source_control.clear_field = true;
            self.source_control.open = None;
            self.source_control.diff = None;
            self.source_control.discard = None;
            self.source_control.diff_request += 1;
        }
        if self.source_control.clear_field {
            self.source_control.clear_field = false;
            let field = self.source_control.commit_field.clone();
            field.update(cx, |field, cx| field.set_text("", window, cx));
        }
        let panel_open = self.model.region_visible(Region::ProjectSidebar)
            && self.source_control.tab == ProjectTab::SourceControl;
        let wanted = selected.filter(|_| panel_open);
        if wanted == self.source_control.watching {
            return;
        }
        if let Some(handle) = &self.service {
            if let Some(old) = &self.source_control.watching {
                handle.git_unwatch(old);
            }
            if let Some(new) = &wanted {
                handle.git_watch(new);
            }
        }
        self.source_control.watching = wanted;
    }

    /// Copies the message box's text into the commit form as it changes.
    pub(super) fn read_commit_message(&mut self, cx: &mut Context<Self>) {
        let text = self.source_control.commit_field.read(cx).text().to_owned();
        self.source_control.commit.message = text;
        self.source_control.commit.message_error = None;
        cx.notify();
    }

    /// Stages or unstages `path`, or every change when `path` is `None`. The status follows through
    /// the refetch the bridge makes after the call.
    pub(super) fn set_staged(
        &mut self,
        path: Option<String>,
        staged: bool,
        cx: &mut Context<Self>,
    ) {
        let (Some(workspace), Some(handle)) =
            (self.source_control.workspace.clone(), self.service.clone())
        else {
            return;
        };
        if self.source_control.busy {
            return;
        }
        let (paths, all) = match path {
            Some(path) => (vec![path], false),
            None => (Vec::new(), true),
        };
        let answer_for = workspace.clone();
        let params = GitPathsParams {
            workspace,
            paths,
            all,
        };
        self.source_control.busy = true;
        self.source_control.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = if staged {
                handle.git_unstage(params).await
            } else {
                handle.git_stage(params).await
            };
            this.update(cx, |shell, cx| shell.finish_index(&answer_for, result, cx))
                .ok();
        })
        .detach();
    }

    /// The stage call's answer. A refusal is shown only while the panel still shows that workspace.
    fn finish_index(
        &mut self,
        workspace: &str,
        result: Result<(), Refusal>,
        cx: &mut Context<Self>,
    ) {
        self.source_control.busy = false;
        if self.source_control.workspace.as_deref() == Some(workspace) {
            self.source_control.error = result.err().map(|refusal| refusal.message());
        }
        cx.notify();
    }

    /// Commits the staged changes with the message in the box. The answer empties the box or says why not.
    pub(super) fn submit_commit(&mut self, cx: &mut Context<Self>) {
        let (Some(workspace), Some(handle)) =
            (self.source_control.workspace.clone(), self.service.clone())
        else {
            return;
        };
        let answer_for = workspace.clone();
        let Some(params) = self.source_control.commit.submit(&workspace) else {
            return;
        };
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = handle.git_commit(params).await.map(|_| ());
            this.update(cx, |shell, cx| shell.finish_commit(&answer_for, result, cx))
                .ok();
        })
        .detach();
    }

    /// The commit call's answer. An answer for a workspace the panel no longer shows changes nothing it shows.
    fn finish_commit(
        &mut self,
        workspace: &str,
        result: Result<(), Refusal>,
        cx: &mut Context<Self>,
    ) {
        if self.source_control.workspace.as_deref() != Some(workspace) {
            cx.notify();
            return;
        }
        match result {
            Ok(()) => {
                self.source_control.commit.committed();
                self.source_control.clear_field = true;
            }
            Err(refusal) => self.source_control.commit.refused(&refusal),
        }
        cx.notify();
    }

    /// Shows the diff of `file` in the Canvas. An answer counts only while this file is the open one.
    pub(super) fn open_file(&mut self, file: OpenFile, cx: &mut Context<Self>) {
        let (Some(workspace), Some(handle)) =
            (self.source_control.workspace.clone(), self.service.clone())
        else {
            return;
        };
        self.source_control.diff_request += 1;
        let request = self.source_control.diff_request;
        self.source_control.diff = Some(DiffLoad::Loading);
        let params = GitDiffFileParams {
            workspace,
            target: file.target.clone(),
            path: file.path.clone(),
            original_path: file.original_path.clone(),
            max_bytes: None,
        };
        self.source_control.open = Some(file.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = handle.git_diff_file(params).await;
            this.update(cx, |shell, cx| shell.finish_diff(request, file, result, cx))
                .ok();
        })
        .detach();
    }

    fn finish_diff(
        &mut self,
        request: u64,
        file: OpenFile,
        result: Result<GitDiffFile, Refusal>,
        cx: &mut Context<Self>,
    ) {
        if request != self.source_control.diff_request {
            return;
        }
        self.source_control.diff = Some(match result {
            Ok(diff) => {
                let patch = diff.patch.as_deref().unwrap_or_default();
                let model = DiffModel::new(parse_patch(&file.path, file_status(file.kind), patch));
                DiffLoad::Ready {
                    model: Rc::new(model),
                    status: diff_status(&diff),
                    truncated: diff.truncated,
                }
            }
            Err(refusal) => DiffLoad::Failed(refusal.message()),
        });
        cx.notify();
    }

    /// Closes the file's diff and drops an answer that is still on its way.
    pub(super) fn close_file(&mut self, cx: &mut Context<Self>) {
        self.source_control.open = None;
        self.source_control.diff = None;
        self.source_control.diff_request += 1;
        cx.notify();
    }

    /// Opens the discard dialog for `paths`. Nothing is discarded until the dialog is confirmed.
    pub(super) fn open_discard(&mut self, targets: Vec<DiscardTarget>, cx: &mut Context<Self>) {
        let Some(workspace) = self.source_control.workspace.clone() else {
            return;
        };
        if self.service.is_none() || self.source_control.discard.is_some() {
            return;
        }
        self.source_control.discard = Some(DiscardFlow::new(workspace, targets));
        cx.notify();
    }

    /// The dialog's action: gives the waiver its stage asks for and discards with it.
    pub(super) fn confirm_discard(&mut self, cx: &mut Context<Self>) {
        let Some(handle) = self.service.clone() else {
            return;
        };
        let Some(flow) = self.source_control.discard.as_mut() else {
            return;
        };
        if !flow.confirm() {
            let plain = matches!(flow.stage(), DiscardStage::Refused { .. });
            if plain {
                self.close_discard(cx);
            }
            return;
        }
        let params = flow.params();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = handle.git_discard(params).await.map(|_| ());
            this.update(cx, |shell, cx| shell.finish_discard(result, cx))
                .ok();
        })
        .detach();
    }

    fn finish_discard(&mut self, result: Result<(), Refusal>, cx: &mut Context<Self>) {
        let Some(flow) = self.source_control.discard.as_mut() else {
            return;
        };
        if !flow.answered(result) {
            cx.notify();
            return;
        }
        let discarded = self
            .source_control
            .open
            .as_ref()
            .is_some_and(|open| flow.paths().contains(&open.path));
        self.source_control.discard = None;
        if discarded {
            self.close_file(cx);
        } else {
            cx.notify();
        }
    }

    pub(super) fn close_discard(&mut self, cx: &mut Context<Self>) {
        self.source_control.discard = None;
        cx.notify();
    }

    /// Asks the service again for a panel that could not read its changes.
    pub(super) fn retry_source_control(&mut self, cx: &mut Context<Self>) {
        if let (Some(handle), Some(workspace)) = (&self.service, &self.source_control.watching) {
            handle.git_unwatch(workspace);
            handle.git_watch(workspace);
        }
        cx.notify();
    }
}
