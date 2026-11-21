//! DelegationTree: who is working on what when a lead bot or a coordinator hands parts of a task
//! to children (docs/design/components/DelegationTree.md). This part is the model: the nodes, the
//! counts and the summary, the rows that show for the collapsed set and the depth limit, and the
//! focus moves. The view draws what the model says.

use crate::components::agent_icon::AgentKind;
use crate::components::agent_status::AgentState;
use crate::tokens::{SIZE_ICON_MD, SPACE_2};
use gpui::{App, SharedString, Window};
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
            commits_ready: 0,
            run: None,
            children: Vec::new(),
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
