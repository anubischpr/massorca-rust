//! Debug only: a background launch with `MASSORCA_DESKTOP_DEBUG_FOCUS` puts the focus in a field
//! and logs what it settles on, so an automated check can type into the field without activating
//! the window.

use super::Shell;
use crate::launch_options::DebugFocus;
use crate::service::StoreUpdate;
use gpui::{Context, Window};
use massorca_protocol::methods::projects::{Project, ProjectKind};
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceKind, WorkspaceState};
use massorca_ui::components::search_field::SearchFieldEvent;
use std::io::Write;

const SAMPLE_PROJECT: &str = "sample-project";
const SAMPLE_WORKSPACE: &str = "sample-workspace";

/// One project and one workspace for the sample shell's start page.
fn sample_start_records() -> Vec<StoreUpdate> {
    vec![StoreUpdate::Resync {
        projects: vec![Project {
            id: SAMPLE_PROJECT.into(),
            kind: ProjectKind::Git,
            path: "/repos/sample".into(),
            name: "sample".into(),
            workspaces_root: "/repos/sample-worktrees".into(),
            added_at_ms: 1,
            base_ref: None,
        }],
        workspaces: vec![Workspace {
            id: SAMPLE_WORKSPACE.into(),
            project_id: SAMPLE_PROJECT.into(),
            kind: WorkspaceKind::Worktree,
            name: "sample-workspace".into(),
            path: "/repos/sample".into(),
            branch: Some("feat/sample".into()),
            base_ref: None,
            created_branch: true,
            state: WorkspaceState::Ready,
            missing: false,
            created_at_ms: 1,
            locked: None,
            prunable: None,
        }],
    }]
}

impl Shell {
    /// Focuses `field` in the next frame, before it is drawn, and logs what it settles on to
    /// stderr: each query of the Search field, each draft of the start page composer.
    pub fn start_debug_input(
        &mut self,
        field: DebugFocus,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.debug_focus = Some(field);
        match field {
            DebugFocus::Search => {
                self._debug_input = Some(cx.subscribe(
                    &self.search,
                    |_, _, event: &SearchFieldEvent, _| {
                        if let SearchFieldEvent::Search(query) = event {
                            let _ = writeln!(
                                std::io::stderr(),
                                "massorca-desktop: debug search query={query:?}"
                            );
                        }
                    },
                ));
                let focus = self.search.read(cx).focus_handle(cx);
                window.on_next_frame(move |window, cx| window.focus(&focus, cx));
            }
            DebugFocus::Composer => {
                // The start page needs a workspace record, and the sample shell has none, so the
                // debug launch adds one. A real service's first workspace is opened as it is.
                let workspace = match self.service {
                    None => {
                        self.store
                            .update(cx, |store, _| store.apply_quietly(sample_start_records()));
                        Some(SAMPLE_WORKSPACE.to_owned())
                    }
                    Some(_) => self
                        .model
                        .snapshot()
                        .projects
                        .iter()
                        .flat_map(|project| &project.hosts)
                        .flat_map(|host| &host.workspaces)
                        .next()
                        .map(|row| row.id.clone()),
                };
                if let Some(id) = workspace {
                    self.model.open_workspace(id);
                }
                let shell = cx.weak_entity();
                let sample = self.service.is_none();
                window.on_next_frame(move |window, cx| {
                    let _ = shell.update(cx, |shell, cx| {
                        // The sample shell opens with a session, which shows instead of the start page.
                        // Closing it here, after the store's sample snapshot has been applied, keeps it closed.
                        if sample {
                            shell.model.close_session();
                        }
                        shell.focus_start_composer(window, cx);
                    });
                });
            }
        }
    }
}
