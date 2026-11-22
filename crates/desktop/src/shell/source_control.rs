//! The Source control panel's state and the calls it makes (screen D-DF-02, decisions 0010 and 0012):
//! which workspace the panel follows, the watch that keeps its status current, and the stage,
//! unstage and commit calls. The panel reads the service's answers from the store; these flows only
//! hold what the panel asked for and what the service refused.

use gpui::{Context, Entity, Subscription, Window};
use massorca_ui::components::text_field::TextField;

use super::Shell;
use crate::model::regions::Region;
use crate::model::source_control_flow::CommitForm;
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
    /// What the last stage or unstage refused, under the panel's header.
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

    /// Commits the staged changes with the message in the box. The answer empties the box or says why not.
    pub(super) fn submit_commit(&mut self, cx: &mut Context<Self>) {
        let (Some(workspace), Some(handle)) =
            (self.source_control.workspace.clone(), self.service.clone())
        else {
            return;
        };
        let Some(params) = self.source_control.commit.submit(&workspace) else {
            return;
        };
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = handle.git_commit(params).await.map(|_| ());
            this.update(cx, |shell, cx| shell.finish_commit(result, cx))
                .ok();
        })
        .detach();
    }

    fn finish_commit(&mut self, result: Result<(), Refusal>, cx: &mut Context<Self>) {
        match result {
            Ok(()) => {
                self.source_control.commit.committed();
                self.source_control.clear_field = true;
            }
            Err(refusal) => self.source_control.commit.refused(&refusal),
        }
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
