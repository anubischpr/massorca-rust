//! SidebarRow: one row of the Sidebar tree (docs/design/components/SidebarRow.md).
//!
//! A ListRow on the Frame surface, 28 px high by default, with the kinds of row
//! the shell needs: navigation entries, projects, hosts, workspaces and sessions.
//! Expansion, keys and roving focus are ListRow's; the tree places the rows.

use crate::components::agent_icon::{AgentIconSize, AgentKind};
use crate::components::agent_status::{AgentState, AgentStatusSize};
use crate::components::badge::{Badge, BadgeTone};
use crate::components::button::StatePreview;
use crate::components::list_row::{ListRow, RowDensity, RowLeading};
use crate::components::usage_ring::UsageSlot;
use crate::icon::IconName;
use crate::style::Surface;
use crate::theme::ActiveTheme;
use crate::tokens::{SIZE_ICON_SM, SPACE_2};
use gpui::{
    App, ClickEvent, ElementId, FocusHandle, IntoElement, RenderOnce, SharedString, Window,
};

/// The indent a child session adds beyond its parent: one 16 px icon and a gap, so
/// the child's agent mark lines up with the first letter of the parent's title (A85).
pub const CHILD_SESSION_INDENT: f32 = SIZE_ICON_SM + SPACE_2;

/// The indent of a child session nested under a row indented by `parent`.
pub fn child_session_indent(parent: f32) -> f32 {
    parent + CHILD_SESSION_INDENT
}

/// Whether a workspace is a git worktree or a plain folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceKind {
    Worktree,
    Folder,
}

impl WorkspaceKind {
    fn icon(self) -> IconName {
        match self {
            Self::Worktree => IconName::GitBranch,
            Self::Folder => IconName::Folder,
        }
    }
}

/// Whether a host answers. An unreachable host dims its own row only (A10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostConnection {
    Connected,
    Unreachable,
}

/// A row of the Sidebar tree. Each constructor sets the icons, emphasis and state
/// of one kind of row; the rest of the builder is ListRow's.
#[derive(Debug, IntoElement)]
pub struct SidebarRow {
    row: ListRow,
    usage: Option<UsageSlot>,
}

impl SidebarRow {
    /// A row with a label only. The kind constructors add their icons and states.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            row: ListRow::new(id, label).surface(Surface::Frame),
            usage: None,
        }
    }

    /// A navigation entry (Prototypes, Tasks, Automations): an icon and a label.
    pub fn nav(id: impl Into<ElementId>, icon: IconName, label: impl Into<SharedString>) -> Self {
        Self::new(id, label).map_row(|r| r.leading(RowLeading::Icon(icon)))
    }

    /// A project at the top level: its icon and its name in `body-strong` (A41).
    pub fn project(
        id: impl Into<ElementId>,
        icon: IconName,
        name: impl Into<SharedString>,
    ) -> Self {
        Self::nav(id, icon, name).map_row(|r| r.title_strong(true))
    }

    /// A host under a project (A10). A reachable host shows its connection dot and
    /// "Connected"; an unreachable one shows the dashed glyph, its last known state
    /// and a dimmed row.
    pub fn host(
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        connection: HostConnection,
    ) -> Self {
        match connection {
            HostConnection::Connected => Self::nav(id, IconName::Server, name)
                .secondary("Connected")
                .trailing(Badge::dot().tone(BadgeTone::Success)),
            HostConnection::Unreachable => Self::new(id, name)
                .map_row(|r| {
                    r.leading(RowLeading::Status(
                        AgentState::Unreachable,
                        AgentStatusSize::Small,
                    ))
                })
                .secondary("Last known state, 5 min ago")
                .trailing(Badge::dot().disabled(true))
                .dimmed(true),
        }
    }

    /// A workspace under its host. Its label takes a usage colour, never a status
    /// colour (P29).
    pub fn workspace(
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        kind: WorkspaceKind,
        usage: UsageSlot,
    ) -> Self {
        let mut row = Self::nav(id, kind.icon(), name);
        row.usage = Some(usage);
        row
    }

    /// A session under its workspace: its status glyph, its agent's mark, then its
    /// title (A84). The agent's name is the mark's tooltip and accessible name.
    pub fn session(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        state: AgentState,
        agent: AgentKind,
    ) -> Self {
        Self::new(id, title).map_row(|r| {
            r.leading(RowLeading::Status(state, AgentStatusSize::XSmall))
                .leading(RowLeading::Agent(agent, AgentIconSize::Small))
        })
    }

    /// Indents a child session under a row indented by `parent_indent`. The child's
    /// agent mark then lines up with the parent's title (A83, A85).
    pub fn child_of(self, parent_indent: f32) -> Self {
        self.map_row(|r| r.indent(child_session_indent(parent_indent)))
    }

    /// The tree level: indents by one tree step per level.
    pub fn level(self, level: usize) -> Self {
        self.map_row(|r| r.level(level))
    }

    pub fn density(self, density: RowDensity) -> Self {
        self.map_row(|r| r.density(density))
    }

    /// The text after the label, in the secondary colour (owner, org or state word).
    pub fn secondary(self, text: impl Into<SharedString>) -> Self {
        self.map_row(|r| r.secondary(text))
    }

    /// A count badge at the trailing edge (Prototypes 2). Zero draws nothing.
    pub fn count(self, count: u32) -> Self {
        self.map_row(|r| r.trailing(Badge::count(count)))
    }

    pub fn trailing(self, element: impl IntoElement) -> Self {
        self.map_row(|r| r.trailing(element))
    }

    /// A row action, shown on hover or focus; the row's context menu carries it too.
    pub fn hover_action(self, element: impl IntoElement) -> Self {
        self.map_row(|r| r.hover_action(element))
    }

    /// A group or a parent: the disclosure chevron, `true` when open.
    pub fn expanded(self, expanded: bool) -> Self {
        self.map_row(|r| r.expanded(expanded))
    }

    pub fn selected(self, selected: bool) -> Self {
        self.map_row(|r| r.selected(selected))
    }

    /// Dims the row and its text, for a host that cannot be reached.
    pub fn dimmed(self, dimmed: bool) -> Self {
        self.map_row(|r| r.dimmed(dimmed))
    }

    /// A loading row: one bar where its label goes (SidebarRow.md, States).
    pub fn loading(self, loading: bool) -> Self {
        self.map_row(|r| r.loading(loading).label_placeholder(loading))
    }

    /// A failed row: the alert glyph replaces the leading icon and the reason is the
    /// secondary line.
    pub fn error(self, reason: impl Into<SharedString>) -> Self {
        self.map_row(|r| r.error(reason))
    }

    /// The full name, shown on hover when the label is truncated.
    pub fn tooltip(self, text: impl Into<SharedString>) -> Self {
        self.map_row(|r| r.tooltip(text))
    }

    /// Forces a visual state (gallery and visual checks only).
    pub fn preview(self, state: StatePreview) -> Self {
        self.map_row(|r| r.preview(state))
    }

    /// Whether the tree gives this row its Tab stop.
    pub fn tab_stop(self, tab_stop: bool) -> Self {
        self.map_row(|r| r.tab_stop(tab_stop))
    }

    pub fn focus_handle(self, handle: FocusHandle) -> Self {
        self.map_row(|r| r.focus_handle(handle))
    }

    /// Called on a pointer click, and on Enter or Space.
    pub fn on_click(self, listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.map_row(|r| r.on_click(listener))
    }

    /// Called with the value Right or Left asks for on a group.
    pub fn on_toggle(self, listener: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.map_row(|r| r.on_toggle(listener))
    }

    /// Called on a right click, and on Shift+F10 or the Menu key.
    pub fn on_context_menu(self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.map_row(|r| r.on_context_menu(listener))
    }

    fn map_row(mut self, change: impl FnOnce(ListRow) -> ListRow) -> Self {
        self.row = change(self.row);
        self
    }
}

impl RenderOnce for SidebarRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        match self.usage {
            Some(slot) => self.row.title_colour(slot.color(&cx.theme().palette)),
            None => self.row,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::button::test_support::{focus_and_draw, press_key};
    use crate::components::list_row::TREE_INDENT;
    use crate::tokens::Palette;
    use gpui::{InteractiveElement, Modifiers, ParentElement, Render, Styled, div, px};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Where the agent mark of a session row starts: padding, status glyph and a gap.
    fn agent_x(indent: f32) -> f32 {
        SPACE_2 + indent + AgentStatusSize::XSmall.px() + SPACE_2
    }

    /// Where the title of a session row starts: after the agent mark and a gap.
    fn title_x(indent: f32) -> f32 {
        agent_x(indent) + AgentIconSize::Small.px() + SPACE_2
    }

    #[test]
    fn a_child_session_puts_its_agent_mark_under_the_parent_title() {
        let parent = 3.0 * TREE_INDENT;
        let child = child_session_indent(parent);
        assert_eq!(child - parent, 24.0);
        assert_eq!(agent_x(child), title_x(parent));
        let grandchild = child_session_indent(child);
        assert_eq!(agent_x(grandchild), title_x(child));
    }

    #[test]
    fn a_workspace_label_takes_one_of_the_six_usage_slots() {
        let palette = Palette::DARK;
        let slot = UsageSlot::new(3).expect("slot 3 exists");
        let row = SidebarRow::workspace("w", "massorca", WorkspaceKind::Worktree, slot);
        assert_eq!(row.usage.map(UsageSlot::index), Some(3));
        assert_eq!(slot.color(&palette), palette.usage.ring_3);
        assert!(UsageSlot::new(7).is_none());
    }

    #[test]
    fn workspaces_show_a_branch_for_a_worktree_and_a_folder_otherwise() {
        assert!(matches!(
            WorkspaceKind::Worktree.icon(),
            IconName::GitBranch
        ));
        assert!(matches!(WorkspaceKind::Folder.icon(), IconName::Folder));
    }

    /// Hosts one session row that logs its click and its toggles.
    struct Harness {
        focus: FocusHandle,
        log: Rc<RefCell<Vec<&'static str>>>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let (clicks, toggles) = (self.log.clone(), self.log.clone());
            let row = SidebarRow::session(
                "session",
                "Feed endpoints",
                AgentState::Idle,
                AgentKind::Claude,
            )
            .expanded(false)
            .focus_handle(self.focus.clone())
            .on_click(move |_, _, _| clicks.borrow_mut().push("click"))
            .on_toggle(move |open, _, _| {
                toggles
                    .borrow_mut()
                    .push(if open { "open" } else { "close" })
            });
            div().child(
                div()
                    .absolute()
                    .top(px(0.))
                    .left(px(0.))
                    .debug_selector(|| "row".into())
                    .child(row),
            )
        }
    }

    #[gpui::test]
    fn a_session_row_opens_on_click_and_opens_its_children_on_right(cx: &mut gpui::TestAppContext) {
        let log = Rc::new(RefCell::new(Vec::new()));
        let shared = log.clone();
        let (view, cx) = cx.add_window_view(|_, cx| Harness {
            focus: cx.focus_handle(),
            log: shared,
        });
        cx.run_until_parked();
        let bounds = cx.debug_bounds("row").expect("row is drawn");
        cx.simulate_click(bounds.center(), Modifiers::none());
        let focus = view.read_with(cx, |h, _| h.focus.clone());
        focus_and_draw(cx, &focus);
        press_key(cx, "right");
        assert_eq!(*log.borrow(), vec!["click", "open"]);
    }
}
