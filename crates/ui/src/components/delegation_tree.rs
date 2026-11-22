//! DelegationTree: who is working on what when a lead bot or a coordinator hands parts of a task
//! to children (docs/design/components/DelegationTree.md). This part is the model: the nodes, the
//! counts and the summary, the rows that show for the collapsed set and the depth limit, and the
//! focus moves. The view draws what the model says.

use crate::components::agent_icon::{AgentIcon, AgentIconSize, AgentKind};
use crate::components::agent_status::{AgentState, AgentStatus, AgentStatusSize};
use crate::components::badge::Badge;
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::icon_button::IconButton;
use crate::icon::{Icon, IconName, IconSize};
use crate::style::{FocusRing, StateFill, Surface};
use crate::theme::{ActiveTheme, TextStyled, Theme};
use crate::tokens::{
    Palette, RADIUS_LG, RADIUS_MD, RADIUS_SM, SIZE_CONTROL_COMFORTABLE, SIZE_ICON_MD, SIZE_ICON_XS,
    SPACE_1, SPACE_2, SPACE_3, SPACE_HALF, TYPE_BODY_SM, TYPE_BODY_STRONG, TYPE_CAPTION,
    TYPE_MONO_SM,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, ElementId, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, RenderOnce, Role, ScrollHandle, SharedString, StatefulInteractiveElement,
    Styled, Window, div, px,
};
use std::collections::HashSet;
use std::rc::Rc;

/// Children one parent lists before the rest are cut behind "Show N more".
pub const CHILD_LIMIT: usize = 12;

/// Children shown when a list is cut.
pub const CHILD_SHOWN: usize = 8;

/// The depth shown by default; the rows below it show "+N deeper" (D-ST-02).
pub const DEPTH_LIMIT: usize = 3;

/// Horizontal step per depth: a child's icon sits under the first letter of its parent's title,
/// so the step is the parent's icon and the gap after it.
pub const INDENT_STEP: f32 = SIZE_ICON_MD + SPACE_2;

/// How a parent's children run: side by side, or one after another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunOrder {
    Parallel,
    Sequence,
}

impl RunOrder {
    /// The caption above a set of children.
    pub fn caption(self) -> &'static str {
        match self {
            Self::Parallel => "Run in parallel",
            Self::Sequence => "Run in order",
        }
    }
}

/// A state's word in the tree: idle reads "Not started" and unreachable "Can't reach".
fn tree_word(state: AgentState) -> &'static str {
    match state {
        AgentState::Idle => "Not started",
        AgentState::Unreachable => "Can't reach",
        state => state.label(),
    }
}

/// One agent or bot in the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DelegationNode {
    /// Stable across renders: the key of the collapsed set and of the callbacks.
    pub id: SharedString,
    /// The task the child was given (Workspaces), or the bot's name (Bot Teams).
    pub title: SharedString,
    /// The agent's or bot's name: the icon's tooltip and part of the accessible name.
    pub name: SharedString,
    /// The agent's own mark. Bot Teams uses it too, as the closest honest stand-in for characters.
    pub agent: AgentKind,
    pub state: AgentState,
    /// "Lead bot" or "Coordinator", on the root.
    pub role: Option<SharedString>,
    /// Bot Teams competence chip, such as "Research".
    pub competence: Option<SharedString>,
    /// Workspaces: the model and effort, such as "Opus 5.5, medium".
    pub model: Option<SharedString>,
    /// Workspaces: the child's own worktree branch; `None` when it uses the current worktree.
    pub branch: Option<SharedString>,
    pub duration: Option<SharedString>,
    /// The task given to the child, which is also why it was delegated.
    pub task: SharedString,
    /// Why the child failed; shown in place of the task on an error row.
    pub reason: Option<SharedString>,
    /// When an unreachable child was last seen, such as "5 min ago".
    pub last_known: Option<SharedString>,
    /// The state an unreachable child was in when it was last seen.
    pub last_state: Option<AgentState>,
    /// The host an unreachable child runs on, named in "Can't reach VPS".
    pub host: Option<SharedString>,
    pub commits_ready: u32,
    /// How this node's children run; `None` when the order does not matter.
    pub run: Option<RunOrder>,
    pub children: Vec<DelegationNode>,
}

impl DelegationNode {
    pub fn new(
        id: impl Into<SharedString>,
        agent: AgentKind,
        state: AgentState,
        title: impl Into<SharedString>,
    ) -> Self {
        let title = title.into();
        Self {
            id: id.into(),
            title: title.clone(),
            name: agent.label().into(),
            agent,
            state,
            role: None,
            competence: None,
            model: None,
            branch: None,
            duration: None,
            task: title,
            reason: None,
            last_known: None,
            last_state: None,
            host: None,
            commits_ready: 0,
            run: None,
            children: Vec::new(),
        }
    }

    /// The row's status word (DelegationTree.md Copy): an idle child has not started yet, and an
    /// unreachable one names its host.
    pub fn status_word(&self) -> String {
        match (self.state, &self.host) {
            (AgentState::Unreachable, Some(host)) => format!("Can't reach {host}"),
            (state, _) => tree_word(state).to_owned(),
        }
    }

    /// The second line of an unreachable row: "Last known: running, 5 min ago".
    pub fn last_known_line(&self) -> String {
        let age = self.last_known.as_deref().unwrap_or("time unknown");
        match self.last_state {
            Some(state) if state != AgentState::Unreachable => {
                format!("Last known: {}, {age}", tree_word(state).to_lowercase())
            }
            _ => format!("Last known state, {age}"),
        }
    }

    /// The row action: a waiting child is opened to answer it.
    pub fn open_label(&self) -> &'static str {
        if self.state == AgentState::Waiting {
            "Open to answer"
        } else {
            "Open"
        }
    }

    pub fn run(mut self, run: RunOrder) -> Self {
        self.run = Some(run);
        self
    }

    pub fn children(mut self, children: Vec<DelegationNode>) -> Self {
        self.children = children;
        self
    }
}

/// The counts behind the header summary, over every node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub agents: usize,
    pub running: usize,
    pub waiting: usize,
    pub done: usize,
    pub failed: usize,
    pub unreachable: usize,
}

fn collect_nodes<'a>(nodes: &'a [DelegationNode], out: &mut Vec<&'a DelegationNode>) {
    for node in nodes {
        out.push(node);
        collect_nodes(&node.children, out);
    }
}

fn subtree_len(node: &DelegationNode) -> usize {
    1 + node.children.iter().map(subtree_len).sum::<usize>()
}

/// Counts every node under the roots.
pub fn counts(roots: &[DelegationNode]) -> Counts {
    let mut all = Vec::new();
    collect_nodes(roots, &mut all);
    let mut out = Counts {
        agents: all.len(),
        ..Counts::default()
    };
    for node in all {
        match node.state {
            AgentState::Running => out.running += 1,
            AgentState::Waiting => out.waiting += 1,
            AgentState::Done => out.done += 1,
            AgentState::Error => out.failed += 1,
            AgentState::Unreachable => out.unreachable += 1,
            AgentState::Idle => {}
        }
    }
    out
}

/// The header summary: "3 agents, 2 running, 1 done", then what needs the reader and what cannot
/// be reached, counted apart.
pub fn summary(counts: Counts) -> String {
    let agents = if counts.agents == 1 {
        "1 agent".to_owned()
    } else {
        format!("{} agents", counts.agents)
    };
    let mut parts = vec![agents];
    if counts.running > 0 {
        parts.push(format!("{} running", counts.running));
    }
    if counts.waiting > 0 {
        parts.push(format!("{} needs you", counts.waiting));
    }
    if counts.done > 0 {
        parts.push(format!("{} done", counts.done));
    }
    if counts.failed > 0 {
        parts.push(format!("{} failed", counts.failed));
    }
    if counts.unreachable > 0 {
        parts.push(format!("{} can't be reached", counts.unreachable));
    }
    parts.join(", ")
}

/// One visible row of the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// A node. `expanded` is `None` for a node without children; `step` numbers a child of a
    /// sequence.
    Node {
        path: Vec<usize>,
        depth: usize,
        expanded: Option<bool>,
        step: Option<usize>,
    },
    /// The caption above a set of children.
    Caption { depth: usize, run: RunOrder },
    /// Rows below the depth limit, under the node at `path`: "+N deeper".
    Deeper {
        path: Vec<usize>,
        depth: usize,
        count: usize,
    },
    /// The rest of a long list of children under the node at `path`.
    ShowMore {
        path: Vec<usize>,
        depth: usize,
        hidden: usize,
    },
}

impl Row {
    pub fn depth(&self) -> usize {
        match self {
            Self::Node { depth, .. }
            | Self::Caption { depth, .. }
            | Self::Deeper { depth, .. }
            | Self::ShowMore { depth, .. } => *depth,
        }
    }
}

/// What the owner keeps between renders, and the depth at which subtrees fold.
struct Ctx<'a> {
    collapsed: &'a HashSet<SharedString>,
    shown_all: &'a HashSet<SharedString>,
    depth_limit: usize,
}

/// The visible rows: children of collapsed nodes are hidden, long lists are cut unless their
/// parent is in `shown_all`, and rows below `depth_limit` fold into "+N deeper".
pub fn rows(
    roots: &[DelegationNode],
    collapsed: &HashSet<SharedString>,
    shown_all: &HashSet<SharedString>,
    depth_limit: usize,
) -> Vec<Row> {
    let ctx = Ctx {
        collapsed,
        shown_all,
        depth_limit,
    };
    let mut out = Vec::new();
    push_rows(roots, &mut Vec::new(), 0, false, &ctx, &mut out);
    out
}

fn push_rows(
    nodes: &[DelegationNode],
    prefix: &mut Vec<usize>,
    depth: usize,
    sequence: bool,
    ctx: &Ctx<'_>,
    out: &mut Vec<Row>,
) {
    for (index, node) in nodes.iter().enumerate() {
        prefix.push(index);
        let has_children = !node.children.is_empty();
        let open = has_children && !ctx.collapsed.contains(&node.id);
        out.push(Row::Node {
            path: prefix.clone(),
            depth,
            expanded: has_children.then_some(open),
            step: sequence.then_some(index + 1),
        });
        if open {
            push_children(node, prefix, depth, ctx, out);
        }
        prefix.pop();
    }
}

fn push_children(
    node: &DelegationNode,
    prefix: &[usize],
    depth: usize,
    ctx: &Ctx<'_>,
    out: &mut Vec<Row>,
) {
    if depth + 1 > ctx.depth_limit {
        let count = node.children.iter().map(subtree_len).sum();
        out.push(Row::Deeper {
            path: prefix.to_vec(),
            depth: depth + 1,
            count,
        });
        return;
    }
    if let Some(run) = node.run {
        out.push(Row::Caption {
            depth: depth + 1,
            run,
        });
    }
    let shown_all = ctx.shown_all.contains(&node.id);
    let total = node.children.len();
    let limit = if shown_all || total <= CHILD_LIMIT {
        total
    } else {
        CHILD_SHOWN
    };
    let mut path = prefix.to_vec();
    push_rows(
        &node.children[..limit],
        &mut path,
        depth + 1,
        node.run == Some(RunOrder::Sequence),
        ctx,
        out,
    );
    if limit < total {
        out.push(Row::ShowMore {
            path: prefix.to_vec(),
            depth: depth + 1,
            hidden: total - limit,
        });
    }
}

/// The row a row hangs under: the nearest row above it one depth shallower.
pub fn parent_of(rows: &[Row], index: usize) -> Option<usize> {
    let depth = rows.get(index)?.depth();
    (0..index)
        .rev()
        .find(|above| rows[*above].depth() + 1 == depth)
}

/// The next index after `from`, wrapping around, for which `matches` holds: the typeahead step.
pub fn next_matching(count: usize, from: usize, matches: impl Fn(usize) -> bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    (1..=count)
        .map(|step| (from + step) % count)
        .find(|index| matches(*index))
}

/// Where the tree sits. Workspaces rows show the model, branch and commits; Bot Teams shows the
/// competence chips; the card form is one line per child.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TreeVariant {
    #[default]
    Workspaces,
    BotTeams,
    Card,
}

/// What the reader asked for. The owner keeps the collapsed set and acts on the rest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeAction {
    /// Expand or collapse a node.
    Toggle(SharedString),
    /// Select a child: its thread opens in place of the transcript.
    Select(SharedString),
    /// Open a child's thread or session.
    Open(SharedString),
    /// Review the commits a child has ready.
    Review(SharedString),
    /// Show the rest of a long list under a node.
    ShowMore(SharedString),
    /// Open the subtree that folds below the depth limit.
    Deeper(SharedString),
}

/// Receives the actions of a DelegationTree.
pub type TreeActionHandler = Rc<dyn Fn(TreeAction, &mut Window, &mut App)>;

/// The node at a path of child indexes.
pub fn node_at<'a>(roots: &'a [DelegationNode], path: &[usize]) -> Option<&'a DelegationNode> {
    let (first, rest) = path.split_first()?;
    let mut node = roots.get(*first)?;
    for index in rest {
        node = node.children.get(*index)?;
    }
    Some(node)
}

/// The id of the node a row is about: the node itself, or the parent of a folded or cut list.
pub fn row_id(roots: &[DelegationNode], row: &Row) -> Option<SharedString> {
    match row {
        Row::Node { path, .. } | Row::Deeper { path, .. } | Row::ShowMore { path, .. } => {
            node_at(roots, path).map(|node| node.id.clone())
        }
        Row::Caption { .. } => None,
    }
}

/// The title of a node row, for the typeahead.
pub fn row_title(roots: &[DelegationNode], row: &Row) -> Option<SharedString> {
    match row {
        Row::Node { path, .. } => node_at(roots, path).map(|node| node.title.clone()),
        _ => None,
    }
}

/// Applies a key to the tree, moving `cursor` over the visible rows. Returns whether the key was
/// used, and the action it asks for.
pub fn handle_key(
    roots: &[DelegationNode],
    rows: &[Row],
    cursor: &mut usize,
    key: &str,
) -> (bool, Option<TreeAction>) {
    let last = rows.len().saturating_sub(1);
    let current = *cursor;
    let Some(row) = rows.get(current) else {
        return (false, None);
    };
    let id = row_id(roots, row);
    let node = matches!(row, Row::Node { .. });
    let expanded = match row {
        Row::Node { expanded, .. } => *expanded,
        _ => None,
    };
    let typed = key
        .chars()
        .next()
        .filter(|_| key.chars().count() == 1)
        .map(|letter| letter.to_ascii_lowercase());
    let action = match key {
        "down" => {
            *cursor = (current + 1).min(last);
            None
        }
        "up" => {
            *cursor = current.saturating_sub(1);
            None
        }
        "home" => {
            *cursor = 0;
            None
        }
        "end" => {
            *cursor = last;
            None
        }
        "enter" if node => id.map(TreeAction::Open),
        "right" if expanded == Some(false) => id.map(TreeAction::Toggle),
        "left" if expanded == Some(true) => id.map(TreeAction::Toggle),
        "right" if expanded == Some(true) => {
            *cursor = (current + 1).min(last);
            None
        }
        "left" => {
            if let Some(parent) = parent_of(rows, current) {
                *cursor = parent;
            }
            None
        }
        _ => match typed {
            Some(letter) => {
                let hit = next_matching(rows.len(), current, |index| {
                    row_title(roots, &rows[index])
                        .is_some_and(|title| title.to_lowercase().starts_with(letter))
                });
                if let Some(index) = hit {
                    *cursor = index;
                }
                None
            }
            None => return (false, None),
        },
    };
    (true, action)
}

/// The view's own state, kept by its element id.
struct TreeUi {
    focus: FocusHandle,
    scroll: ScrollHandle,
    open: bool,
    cursor: usize,
}

/// What the rows read from the view.
struct Look {
    p: Palette,
    theme: Theme,
    variant: TreeVariant,
    selected: Option<SharedString>,
    cursor: usize,
    focused: bool,
    handler: Option<TreeActionHandler>,
}

/// A click listener that sends one action to the handler.
fn on(
    handler: &Option<TreeActionHandler>,
    action: TreeAction,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |window, cx| emit(&handler, action.clone(), window, cx)
}

fn emit(
    handler: &Option<TreeActionHandler>,
    action: TreeAction,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(handler) = handler {
        handler(action, window, cx);
    }
}

/// One node row: the expander or step number, the status glyph, the agent's icon, the title,
/// and below it the badges, the place it runs and the task or the reason.
fn node_row(index: usize, row: &Row, roots: &[DelegationNode], look: &Look) -> AnyElement {
    let Row::Node {
        path,
        depth,
        expanded,
        step,
    } = row
    else {
        return div().into_any_element();
    };
    let Some(node) = node_at(roots, path) else {
        return div().into_any_element();
    };
    let p = &look.p;
    let card = look.variant == TreeVariant::Card;
    let selected = look.selected.as_ref() == Some(&node.id);
    let unreachable = node.state == AgentState::Unreachable;
    let fill = if unreachable {
        Some(p.agent.unreachable_bg)
    } else {
        node.state.row_tint(p)
    };
    let title_color = if unreachable {
        p.text.tertiary
    } else {
        p.text.primary
    };
    let id = node.id.clone();
    let handler = look.handler.clone();

    let toggle = match expanded {
        Some(open) => {
            let icon = if *open {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            };
            let (toggle_id, toggle_handler) = (id.clone(), handler.clone());
            div()
                .id(("tree-toggle", index))
                .flex_none()
                .w(px(SIZE_ICON_XS))
                .cursor_pointer()
                .child(
                    Icon::new(icon)
                        .size(IconSize::XSmall)
                        .color(p.text.secondary),
                )
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    emit(
                        &toggle_handler,
                        TreeAction::Toggle(toggle_id.clone()),
                        window,
                        cx,
                    );
                })
                .into_any_element()
        }
        None => div()
            .flex_none()
            .w(px(SIZE_ICON_XS))
            .text_style(TYPE_MONO_SM)
            .text_color(p.text.tertiary)
            .child(step.map(|number| number.to_string()).unwrap_or_default())
            .into_any_element(),
    };

    let mut name_line = div()
        .flex()
        .items_center()
        .gap(px(SPACE_2))
        .child(toggle)
        .child(AgentStatus::new(node.state).size(AgentStatusSize::Small))
        .child(AgentIcon::new(node.agent).size(AgentIconSize::Medium))
        .child(
            div()
                .flex_shrink(1.)
                .min_w_0()
                .truncate()
                .text_style(TYPE_BODY_STRONG)
                .text_color(title_color)
                .child(node.title.clone()),
        );
    // The role tag follows the name (DelegationTree.md Anatomy, row name).
    if let (false, Some(role)) = (card, node.role.clone()) {
        name_line = name_line.child(div().flex_none().child(Badge::text(role)));
    }
    name_line = name_line.child(div().flex_1());
    if !card {
        name_line = name_line
            .child(
                div()
                    .flex_none()
                    .text_style(TYPE_CAPTION)
                    .tabular_figures()
                    .text_color(node.state.color(p))
                    .child(node.status_word()),
            )
            .children(node.duration.clone().map(|duration| {
                div()
                    .flex_none()
                    .text_style(TYPE_CAPTION)
                    .tabular_figures()
                    .text_color(p.text.tertiary)
                    .child(duration)
            }));
    }

    let line = if unreachable {
        SharedString::from(node.last_known_line())
    } else if node.state == AgentState::Error {
        node.reason.clone().unwrap_or_else(|| node.task.clone())
    } else {
        node.task.clone()
    };
    let mut meta = div().flex().items_center().gap(px(SPACE_2)).min_w_0();
    if look.variant == TreeVariant::BotTeams
        && let Some(competence) = node.competence.clone()
    {
        meta = meta.child(Badge::text(competence));
    }
    if look.variant == TreeVariant::Workspaces {
        if let Some(model) = node.model.clone() {
            meta = meta.child(
                div()
                    .flex_none()
                    .text_style(TYPE_CAPTION)
                    .text_color(p.text.tertiary)
                    .child(model),
            );
        }
        let place = node
            .branch
            .clone()
            .unwrap_or_else(|| SharedString::from("Current worktree"));
        meta = meta.child(place_chip(place, p));
    }
    meta = meta.child(
        div()
            .flex_1()
            .min_w_0()
            .truncate()
            .text_style(TYPE_BODY_SM)
            .text_color(p.text.secondary)
            .child(line),
    );
    if !card {
        let mut actions = div()
            .id(("tree-actions", index))
            .flex()
            .items_center()
            .gap(px(SPACE_1))
            .flex_none()
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                Button::new(("tree-open", index), node.open_label())
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Compact)
                    .on_click(on(&handler, TreeAction::Open(id.clone()))),
            );
        if node.commits_ready > 0 {
            actions = actions
                .child(Badge::text(format!("{} commits ready", node.commits_ready)))
                .child(
                    Button::new(("tree-review", index), "Review")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Compact)
                        .on_click(on(&handler, TreeAction::Review(id.clone()))),
                );
        }
        meta = meta.child(actions);
    }

    let select_id = id.clone();
    let select_handler = handler.clone();
    let mut el = div()
        .id(("tree-row", index))
        .relative()
        .role(Role::TreeItem)
        .aria_level(depth + 1)
        .aria_selected(selected)
        .aria_label(match &node.duration {
            Some(duration) => format!(
                "{}, {}, {}, {}",
                node.title,
                node.name,
                node.status_word(),
                duration
            ),
            None => format!("{}, {}, {}", node.title, node.name, node.status_word()),
        })
        .flex()
        .flex_col()
        .gap(px(SPACE_1))
        .min_h(px(SIZE_CONTROL_COMFORTABLE))
        .pl(px(SPACE_2 + *depth as f32 * INDENT_STEP))
        .pr(px(SPACE_2))
        .py(px(SPACE_1))
        .rounded(px(RADIUS_MD))
        .cursor_pointer()
        .state_fills(&look.theme, Surface::Raised)
        .when_some(fill, |el, color| el.bg(color))
        .when(selected, |el| el.bg(p.fill.selected_raised))
        .on_click(move |_, window, cx| {
            emit(
                &select_handler,
                TreeAction::Select(select_id.clone()),
                window,
                cx,
            )
        });
    if let Some(open) = expanded {
        el = el.aria_expanded(*open);
    }
    if look.focused && look.cursor == index {
        el = el.border_2().border_color(p.border.focus);
    }
    if selected {
        el = el.child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .w(px(SPACE_HALF))
                .bg(p.accent.fg),
        );
    }
    if card {
        el = el.child(name_line);
    } else {
        el = el.child(name_line).child(meta);
    }
    el.into_any_element()
}

/// The worktree or branch a Workspaces child runs in, as a chip with the branch icon.
fn place_chip(place: SharedString, p: &Palette) -> AnyElement {
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(SPACE_1))
        .px(px(SPACE_1))
        .rounded(px(RADIUS_SM))
        .border_1()
        .border_color(p.border.subtle)
        .bg(p.surface.sunken)
        .text_style(TYPE_MONO_SM)
        .text_color(p.text.secondary)
        .child(
            Icon::new(IconName::GitBranch)
                .size(IconSize::XSmall)
                .color(p.text.tertiary),
        )
        .child(place)
        .into_any_element()
}

/// The caption above a set of children, and the rows that fold or cut a list.
fn other_row(index: usize, row: &Row, roots: &[DelegationNode], look: &Look) -> AnyElement {
    let p = &look.p;
    match row {
        Row::Caption { depth, run } => div()
            .id(("tree-caption", index))
            .pl(px(SPACE_2 + *depth as f32 * INDENT_STEP))
            .py(px(SPACE_1))
            .text_style(TYPE_CAPTION)
            .text_color(p.text.tertiary)
            .child(run.caption())
            .into_any_element(),
        Row::Deeper { depth, count, .. } => div()
            .pl(px(SPACE_2 + *depth as f32 * INDENT_STEP))
            .child(
                Button::new(("tree-deeper", index), format!("+{count} deeper"))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Compact)
                    .on_click(on(
                        &look.handler,
                        TreeAction::Deeper(row_id(roots, row).unwrap_or_default()),
                    )),
            )
            .into_any_element(),
        Row::ShowMore { depth, hidden, .. } => div()
            .pl(px(SPACE_2 + *depth as f32 * INDENT_STEP))
            .child(
                Button::new(("tree-more", index), format!("Show {hidden} more"))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Compact)
                    .on_click(on(
                        &look.handler,
                        TreeAction::ShowMore(row_id(roots, row).unwrap_or_default()),
                    )),
            )
            .into_any_element(),
        Row::Node { .. } => node_row(index, row, roots, look),
    }
}

/// A delegation tree: its header with the summary and a collapse, and its rows, which move with
/// the arrow keys, Enter opens a child, and a letter jumps to the next title that starts with it
/// (DelegationTree.md).
#[derive(IntoElement)]
pub struct DelegationTree {
    id: ElementId,
    roots: Rc<Vec<DelegationNode>>,
    variant: TreeVariant,
    collapsed: HashSet<SharedString>,
    shown_all: HashSet<SharedString>,
    selected: Option<SharedString>,
    depth_limit: usize,
    on_action: Option<TreeActionHandler>,
}

impl std::fmt::Debug for DelegationTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DelegationTree")
            .field("id", &self.id)
            .field("roots", &self.roots.len())
            .finish()
    }
}

impl DelegationTree {
    pub fn new(id: impl Into<ElementId>, roots: impl Into<Rc<Vec<DelegationNode>>>) -> Self {
        Self {
            id: id.into(),
            roots: roots.into(),
            variant: TreeVariant::Workspaces,
            collapsed: HashSet::new(),
            shown_all: HashSet::new(),
            selected: None,
            depth_limit: DEPTH_LIMIT,
            on_action: None,
        }
    }

    pub fn variant(mut self, variant: TreeVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The nodes the owner has collapsed, by id.
    pub fn collapsed(mut self, collapsed: HashSet<SharedString>) -> Self {
        self.collapsed = collapsed;
        self
    }

    /// The parents whose long lists the owner has opened in full, by id.
    pub fn shown_all(mut self, shown_all: HashSet<SharedString>) -> Self {
        self.shown_all = shown_all;
        self
    }

    /// The row whose thread is open: a 2 px accent bar on its leading edge.
    pub fn selected(mut self, id: Option<SharedString>) -> Self {
        self.selected = id;
        self
    }

    pub fn depth_limit(mut self, limit: usize) -> Self {
        self.depth_limit = limit;
        self
    }

    pub fn on_action(
        mut self,
        listener: impl Fn(TreeAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(listener));
        self
    }
}

impl RenderOnce for DelegationTree {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let id = self.id.clone();
        let state = window.use_keyed_state(id.clone(), cx, |_, cx| TreeUi {
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            open: true,
            cursor: 0,
        });
        let (focus, scroll, open, cursor) = {
            let s = state.read(cx);
            (s.focus.clone(), s.scroll.clone(), s.open, s.cursor)
        };
        let roots = self.roots.clone();
        let visible = rows(&roots, &self.collapsed, &self.shown_all, self.depth_limit);
        let totals = counts(&roots);
        let card = self.variant == TreeVariant::Card;
        let handler = self.on_action.clone();
        let look = Look {
            p,
            theme,
            variant: self.variant,
            selected: self.selected.clone(),
            cursor,
            focused: focus.is_focused(window),
            handler: handler.clone(),
        };

        let header = (!card).then(|| {
            let toggle = state.clone();
            div()
                .flex()
                .items_center()
                .gap(px(SPACE_2))
                .child(
                    Icon::new(IconName::Users)
                        .size(IconSize::Small)
                        .color(p.text.secondary),
                )
                .child(
                    div()
                        .flex_1()
                        .text_style(TYPE_BODY_STRONG)
                        .text_color(p.text.primary)
                        .child("Delegation"),
                )
                .child(
                    div()
                        .text_style(TYPE_CAPTION)
                        .tabular_figures()
                        .text_color(p.text.secondary)
                        .child(summary(totals)),
                )
                .child(
                    IconButton::new(
                        "tree-collapse",
                        if open {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        },
                        if open {
                            "Collapse delegation"
                        } else {
                            "Expand delegation"
                        },
                    )
                    .on_click(move |window, cx| {
                        toggle.update(cx, |s, _| s.open = !s.open);
                        window.refresh();
                    }),
                )
        });

        let body = open.then(|| {
            let elements: Vec<AnyElement> = visible
                .iter()
                .enumerate()
                .map(|(index, row)| match row {
                    Row::Node { .. } => node_row(index, row, &roots, &look),
                    _ => other_row(index, row, &roots, &look),
                })
                .collect();
            let key_roots = roots.clone();
            let key_state = state.clone();
            let key_handler = handler.clone();
            let key_scroll = scroll.clone();
            let (key_collapsed, key_shown, depth) = (
                self.collapsed.clone(),
                self.shown_all.clone(),
                self.depth_limit,
            );
            div()
                .id((id.clone(), "tree"))
                .role(Role::Tree)
                .aria_label("Delegation")
                .track_focus(&focus)
                .tab_stop(true)
                .focus_ring(&theme)
                .track_scroll(&scroll)
                .flex()
                .flex_col()
                .gap(px(SPACE_1))
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    let key = event.keystroke.key.as_str();
                    let (used, action) = key_state.update(cx, |s, _| {
                        let rows = rows(&key_roots, &key_collapsed, &key_shown, depth);
                        let result = handle_key(&key_roots, &rows, &mut s.cursor, key);
                        key_scroll.scroll_to_item(s.cursor);
                        result
                    });
                    if used {
                        cx.stop_propagation();
                        window.refresh();
                        if let Some(action) = action {
                            emit(&key_handler, action, window, cx);
                        }
                    }
                })
                .children(elements)
        });

        div()
            .id(id)
            .flex()
            .flex_col()
            .gap(px(SPACE_2))
            .min_w_0()
            .when(!card, |el| {
                el.p(px(SPACE_3))
                    .rounded(px(RADIUS_LG))
                    .border_1()
                    .border_color(p.border.default)
                    .bg(p.surface.raised)
            })
            .children(header)
            .children(body)
    }
}

#[cfg(test)]
mod key_tests {
    use super::*;

    /// A lead with two children; the first has one child of its own.
    fn tree() -> Vec<DelegationNode> {
        vec![
            DelegationNode::new("lead", AgentKind::Claude, AgentState::Running, "Plan").children(
                vec![
                    DelegationNode::new("alpha", AgentKind::Codex, AgentState::Done, "Alpha")
                        .children(vec![DelegationNode::new(
                            "leaf",
                            AgentKind::Gemini,
                            AgentState::Done,
                            "Leaf",
                        )]),
                    DelegationNode::new("write", AgentKind::Codex, AgentState::Waiting, "Write"),
                ],
            ),
        ]
    }

    fn visible(roots: &[DelegationNode], collapsed: &[&str]) -> Vec<Row> {
        let collapsed: HashSet<SharedString> =
            collapsed.iter().map(|id| SharedString::from(*id)).collect();
        rows(roots, &collapsed, &HashSet::new(), DEPTH_LIMIT)
    }

    #[test]
    fn arrows_move_the_cursor_within_the_visible_rows() {
        let roots = tree();
        let rows = visible(&roots, &[]);
        let mut cursor = 0;
        assert_eq!(handle_key(&roots, &rows, &mut cursor, "down"), (true, None));
        assert_eq!(cursor, 1);
        handle_key(&roots, &rows, &mut cursor, "end");
        assert_eq!(cursor, rows.len() - 1);
        handle_key(&roots, &rows, &mut cursor, "down");
        assert_eq!(cursor, rows.len() - 1);
        handle_key(&roots, &rows, &mut cursor, "home");
        assert_eq!(cursor, 0);
        assert_eq!(handle_key(&roots, &rows, &mut cursor, "f5"), (false, None));
    }

    #[test]
    fn right_and_left_fold_and_walk_the_tree() {
        let roots = tree();
        let folded = visible(&roots, &["alpha"]);
        let mut cursor = 1;
        assert_eq!(
            handle_key(&roots, &folded, &mut cursor, "right"),
            (true, Some(TreeAction::Toggle("alpha".into())))
        );
        let open = visible(&roots, &[]);
        handle_key(&roots, &open, &mut cursor, "right");
        assert_eq!(cursor, 2);
        handle_key(&roots, &open, &mut cursor, "left");
        assert_eq!(cursor, 1);
        assert_eq!(
            handle_key(&roots, &open, &mut cursor, "left"),
            (true, Some(TreeAction::Toggle("alpha".into())))
        );
    }

    #[test]
    fn enter_opens_the_row_and_a_letter_jumps_to_the_next_title() {
        let roots = tree();
        let rows = visible(&roots, &[]);
        let mut cursor = 3;
        assert_eq!(
            handle_key(&roots, &rows, &mut cursor, "enter"),
            (true, Some(TreeAction::Open("write".into())))
        );
        handle_key(&roots, &rows, &mut cursor, "a");
        assert_eq!(cursor, 1);
        handle_key(&roots, &rows, &mut cursor, "w");
        assert_eq!(cursor, 3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A lead bot running two children in parallel: one running, one done.
    fn team() -> Vec<DelegationNode> {
        vec![
            DelegationNode::new(
                "lead",
                AgentKind::Claude,
                AgentState::Running,
                "Plan the release",
            )
            .run(RunOrder::Parallel)
            .children(vec![
                DelegationNode::new(
                    "research",
                    AgentKind::Claude,
                    AgentState::Running,
                    "Research the API",
                ),
                DelegationNode::new(
                    "write",
                    AgentKind::Codex,
                    AgentState::Done,
                    "Write the client",
                ),
            ]),
        ]
    }

    fn chain(depth: usize) -> DelegationNode {
        let mut node = DelegationNode::new("leaf", AgentKind::Cursor, AgentState::Idle, "Leaf");
        for level in (0..depth).rev() {
            let id = format!("level-{level}");
            node = DelegationNode::new(id, AgentKind::Cursor, AgentState::Idle, "Level")
                .children(vec![node]);
        }
        node
    }

    #[test]
    fn summary_counts_running_done_and_the_unreachable_apart() {
        let totals = counts(&team());
        assert_eq!(totals.agents, 3);
        assert_eq!(summary(totals), "3 agents, 2 running, 1 done");
        let mut roots = team();
        roots[0].children[0].state = AgentState::Unreachable;
        roots[0].children[0].last_known = Some("5 min ago".into());
        assert_eq!(
            summary(counts(&roots)),
            "3 agents, 1 running, 1 done, 1 can't be reached"
        );
        assert_eq!(
            summary(Counts {
                agents: 1,
                ..Counts::default()
            }),
            "1 agent"
        );
    }

    #[test]
    fn rows_use_the_tree_words_and_actions() {
        let mut node = DelegationNode::new(
            "n",
            AgentKind::Claude,
            AgentState::Idle,
            "Extract the module",
        );
        assert_eq!(node.status_word(), "Not started");
        assert_eq!(node.open_label(), "Open");
        node.state = AgentState::Waiting;
        assert_eq!(node.status_word(), "Waiting for you");
        assert_eq!(node.open_label(), "Open to answer");
        node.state = AgentState::Unreachable;
        node.last_known = Some("5 min ago".into());
        assert_eq!(node.status_word(), "Can't reach");
        assert_eq!(node.last_known_line(), "Last known state, 5 min ago");
        node.host = Some("VPS".into());
        node.last_state = Some(AgentState::Running);
        assert_eq!(node.status_word(), "Can't reach VPS");
        assert_eq!(node.last_known_line(), "Last known: running, 5 min ago");
    }

    #[test]
    fn every_node_shows_and_a_parallel_parent_captions_its_children() {
        let rows = rows(&team(), &HashSet::new(), &HashSet::new(), DEPTH_LIMIT);
        assert_eq!(rows.len(), 4);
        assert_eq!(
            rows[1],
            Row::Caption {
                depth: 1,
                run: RunOrder::Parallel
            }
        );
        assert_eq!(
            rows[0],
            Row::Node {
                path: vec![0],
                depth: 0,
                expanded: Some(true),
                step: None
            }
        );
    }

    #[test]
    fn collapsed_nodes_hide_their_children_and_sequences_number_them() {
        let mut roots = team();
        roots[0].run = Some(RunOrder::Sequence);
        let collapsed: HashSet<SharedString> = ["lead".into()].into_iter().collect();
        let folded = rows(&roots, &collapsed, &HashSet::new(), DEPTH_LIMIT);
        assert_eq!(folded.len(), 1);
        let open = rows(&roots, &HashSet::new(), &HashSet::new(), DEPTH_LIMIT);
        let steps: Vec<_> = open
            .iter()
            .filter_map(|row| match row {
                Row::Node { step, .. } => *step,
                _ => None,
            })
            .collect();
        assert_eq!(steps, vec![1, 2]);
    }

    #[test]
    fn long_lists_cut_to_eight_until_their_parent_is_shown_in_full() {
        let children: Vec<_> = (0..13)
            .map(|i| {
                DelegationNode::new(format!("c{i}"), AgentKind::Codex, AgentState::Done, "Task")
            })
            .collect();
        let roots = vec![
            DelegationNode::new("root", AgentKind::Claude, AgentState::Running, "Root")
                .children(children),
        ];
        let cut = rows(&roots, &HashSet::new(), &HashSet::new(), DEPTH_LIMIT);
        assert!(cut.contains(&Row::ShowMore {
            path: vec![0],
            depth: 1,
            hidden: 5
        }));
        assert_eq!(cut.len(), 1 + CHILD_SHOWN + 1);
        let all: HashSet<SharedString> = ["root".into()].into_iter().collect();
        assert_eq!(
            rows(&roots, &HashSet::new(), &all, DEPTH_LIMIT).len(),
            1 + 13
        );
    }

    #[test]
    fn rows_below_the_depth_limit_fold_into_a_deeper_row() {
        let roots = vec![chain(5)];
        let shown = rows(&roots, &HashSet::new(), &HashSet::new(), DEPTH_LIMIT);
        assert_eq!(shown.len(), DEPTH_LIMIT + 2);
        assert_eq!(
            shown.last(),
            Some(&Row::Deeper {
                path: vec![0, 0, 0, 0],
                depth: DEPTH_LIMIT + 1,
                count: 2
            })
        );
    }

    #[test]
    fn parents_and_typeahead_follow_the_visible_rows() {
        let rows = rows(&team(), &HashSet::new(), &HashSet::new(), DEPTH_LIMIT);
        assert_eq!(parent_of(&rows, 2), Some(0));
        assert_eq!(parent_of(&rows, 0), None);
        let titles = [
            "Plan the release",
            "",
            "Research the API",
            "Write the client",
        ];
        let hit = next_matching(rows.len(), 0, |index| titles[index].starts_with('W'));
        assert_eq!(hit, Some(3));
        assert_eq!(
            next_matching(rows.len(), 3, |index| titles[index].starts_with('P')),
            Some(0)
        );
        assert_eq!(next_matching(0, 0, |_| true), None);
    }

    #[test]
    fn indent_steps_put_a_child_icon_under_its_parent_title() {
        assert_eq!(INDENT_STEP, 28.0);
    }
}
