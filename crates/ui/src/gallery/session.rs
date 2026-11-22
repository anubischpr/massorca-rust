//! Gallery stories for the session group: the session views of the Work panel and the Session summary.

use super::{Story, variant};
use crate::components::agent_icon::AgentKind;
use crate::components::agent_status::AgentState;
use crate::components::delegation_tree::{DelegationNode, DelegationTree, RunOrder, TreeVariant};
use crate::components::diff_view::{
    DiffLine, DiffModel, DiffStatus, DiffView, FileDiff, FileStatus, Hunk,
};
use crate::components::plan_view::{Plan, PlanStatus, PlanStep, PlanVariant, PlanView, StepStatus};
use crate::components::session_summary_card::{
    BackgroundTask, CardKind, ChecklistItem, Connector, ContextUse, Environment, QaServer,
    ResultItem, SessionSummary, Subagent, summary_column,
};
use crate::tokens::SPACE_4;
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};
use std::collections::HashSet;
use std::ops::Range;
use std::rc::Rc;

pub fn stories() -> Vec<Story> {
    vec![
        Story {
            name: "Diff view",
            spec: "DiffView.md",
            render: diff_view,
        },
        Story {
            name: "Plan view",
            spec: "PlanView.md",
            render: plan_view,
        },
        Story {
            name: "Delegation tree",
            spec: "DelegationTree.md",
            render: delegation_tree,
        },
        Story {
            name: "Session summary",
            spec: "SessionSummaryCard.md",
            render: session_summary,
        },
        Story {
            name: "Session summary, host unreachable",
            spec: "SessionSummaryCard.md",
            render: session_summary_stale,
        },
    ]
}

/// One file with two changes: 30 unchanged lines hidden before the first, 5 after the second.
fn sample_file(path: &str) -> FileDiff {
    let first = Hunk {
        heading: Some("fn load()".into()),
        old_start: 40,
        old_len: 4,
        new_start: 40,
        new_len: 4,
        hidden_before: (10..40u32)
            .map(|n| DiffLine::context(n, n, format!("let step_{n} = next();")))
            .collect(),
        lines: vec![
            DiffLine::context(40, 40, "let config = Config::default();"),
            DiffLine::removed(41, "let path = \"settings.toml\";")
                .with_changed(vec![Range { start: 20, end: 24 }]),
            DiffLine::added(41, "let path = \"settings.json\";")
                .with_changed(vec![Range { start: 20, end: 24 }]),
            DiffLine::context(42, 42, "let loaded = read(&path)?;"),
        ],
    };
    let second = Hunk {
        heading: None,
        old_start: 80,
        old_len: 1,
        new_start: 80,
        new_len: 2,
        hidden_before: Vec::new(),
        lines: vec![
            DiffLine::context(80, 80, "Ok(loaded)"),
            DiffLine::added(81, "// Settings are read once per session."),
        ],
    };
    FileDiff {
        path: path.into(),
        status: FileStatus::Modified,
        hunks: vec![first, second],
        hidden_after: (81..86u32)
            .map(|n| DiffLine::context(n, n + 1, format!("fn helper_{n}() {{}}")))
            .collect(),
    }
}

fn diff_view(_: &mut Window, cx: &mut App) -> AnyElement {
    let ready = DiffModel::new(sample_file("src/settings/load.rs"));
    let mut drafted = DiffModel::new(sample_file("src/settings/load.rs"));
    drafted.add_draft(0, 1, "Keep the old format readable for one release.");
    drafted.add_draft(1, 1, "Mention the new file in the changelog.");
    let empty = DiffModel::new(FileDiff {
        path: "src/settings/empty.rs".into(),
        status: FileStatus::Added,
        hunks: Vec::new(),
        hidden_after: Vec::new(),
    });
    let frame = |height: f32, child: DiffView| div().h(px(height)).w_full().child(child);

    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant(
            "Ready: a collapsed block, two changes and context, with the base named",
            frame(
                360.,
                DiffView::new("gallery-diff-ready", ready).compare("vs origin/main"),
            ),
            cx,
        ))
        .child(variant(
            "Draft notes; the host cannot be reached, so Send to agent is off",
            frame(
                360.,
                DiffView::new("gallery-diff-offline", drafted).disconnected(true),
            ),
            cx,
        ))
        .child(variant(
            "Conflicted hunks skipped, read-only compare",
            frame(
                240.,
                DiffView::new(
                    "gallery-diff-conflicts",
                    DiffModel::new(sample_file("src/settings/merge.rs")),
                )
                .conflicts_skipped(true)
                .read_only(true),
            ),
            cx,
        ))
        .child(variant(
            "Loading: skeleton lines, the spinner after 200 ms",
            frame(
                200.,
                DiffView::new("gallery-diff-loading", empty.clone()).status(DiffStatus::Loading),
            ),
            cx,
        ))
        .child(variant(
            "Empty: no changes in this file",
            frame(
                160.,
                DiffView::new("gallery-diff-empty", empty.clone()).status(DiffStatus::Empty),
            ),
            cx,
        ))
        .child(variant(
            "Failed, with Retry",
            frame(
                160.,
                DiffView::new("gallery-diff-failed", empty.clone())
                    .status(DiffStatus::Failed("The file could not be read.".into())),
            ),
            cx,
        ))
        .child(variant(
            "Large: confirmed before it draws",
            frame(
                160.,
                DiffView::new("gallery-diff-large", empty.clone())
                    .status(DiffStatus::Large { lines: 12_431 }),
            ),
            cx,
        ))
        .child(variant(
            "Binary file changed",
            frame(
                120.,
                DiffView::new("gallery-diff-binary", empty).status(DiffStatus::Binary {
                    before: "2.1 MB".into(),
                    after: "2.3 MB".into(),
                }),
            ),
            cx,
        ))
        .into_any_element()
}

fn plan_view(_: &mut Window, cx: &mut App) -> AnyElement {
    let plan = Rc::new(Plan {
        steps: vec![
            PlanStep::new("Read the spec", StepStatus::Done),
            PlanStep::new("Write the parser", StepStatus::InProgress).with_children(vec![
                PlanStep::new("Tokens", StepStatus::Done),
                PlanStep::new("Grammar", StepStatus::Pending),
            ]),
            PlanStep::new("Write the tests", StepStatus::Pending),
            PlanStep::new("Publish notes", StepStatus::Skipped),
            PlanStep::new("Ship", StepStatus::Blocked).blocked_by("Waiting for review"),
        ],
        document: Some(
            "The parser reads the spec in one pass.\n\nTests cover every token kind.".into(),
        ),
    });
    let document_only = Plan {
        steps: Vec::new(),
        document: Some("First, read the spec.\n\nThen write the parser.".into()),
    };
    let frame = |height: f32, child: PlanView| div().h(px(height)).w_full().child(child);

    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant(
            "Tab: progress, the current step with its sub-steps, a blocked step with its reason",
            frame(
                360.,
                PlanView::new("gallery-plan-tab", plan.clone()).updated("Updated 2 min ago"),
            ),
            cx,
        ))
        .child(variant(
            "Document only: the plan text as paragraphs",
            frame(200., PlanView::new("gallery-plan-doc", document_only)),
            cx,
        ))
        .child(variant(
            "Card form: the first rows and an Open plan link",
            frame(
                240.,
                PlanView::new("gallery-plan-card", plan.clone()).variant(PlanVariant::Card),
            ),
            cx,
        ))
        .child(variant(
            "Loading",
            frame(
                160.,
                PlanView::new("gallery-plan-loading", Plan::default()).status(PlanStatus::Loading),
            ),
            cx,
        ))
        .child(variant(
            "Empty",
            frame(
                120.,
                PlanView::new("gallery-plan-empty", Plan::default()).status(PlanStatus::Empty),
            ),
            cx,
        ))
        .child(variant(
            "Failed, with Retry",
            frame(
                120.,
                PlanView::new("gallery-plan-failed", Plan::default())
                    .status(PlanStatus::Failed("The agent stopped responding.".into())),
            ),
            cx,
        ))
        .child(variant(
            "Host unreachable: the last plan, dated",
            frame(
                300.,
                PlanView::new("gallery-plan-stale", plan)
                    .status(PlanStatus::Stale("5 min ago".into())),
            ),
            cx,
        ))
        .into_any_element()
}

/// A coordinator with one child per state, and a sequence of two steps under the last child.
fn delegation_sample() -> Vec<DelegationNode> {
    vec![DelegationNode {
        role: Some("Coordinator".into()),
        model: Some("Opus 5.5, high".into()),
        run: Some(RunOrder::Parallel),
        children: vec![
            DelegationNode {
                model: Some("Sonnet 5.5, medium".into()),
                branch: Some("task/research-api".into()),
                duration: Some("2 min".into()),
                task: "Read the sync endpoints and list their fields".into(),
                ..DelegationNode::new(
                    "research",
                    AgentKind::Claude,
                    AgentState::Running,
                    "Research the API",
                )
            },
            DelegationNode {
                model: Some("GPT-5.5, medium".into()),
                branch: Some("task/client".into()),
                task: "Needs a decision on the auth flow".into(),
                ..DelegationNode::new(
                    "write",
                    AgentKind::Codex,
                    AgentState::Waiting,
                    "Write the client",
                )
            },
            DelegationNode {
                reason: Some("The auth tests failed on the token refresh".into()),
                task: "Review the patch".into(),
                ..DelegationNode::new(
                    "review",
                    AgentKind::Gemini,
                    AgentState::Error,
                    "Review the patch",
                )
            },
            DelegationNode {
                last_known: Some("5 min ago".into()),
                last_state: Some(AgentState::Running),
                host: Some("VPS".into()),
                task: "Deploy to staging".into(),
                ..DelegationNode::new(
                    "deploy",
                    AgentKind::Cursor,
                    AgentState::Unreachable,
                    "Deploy to staging",
                )
            },
            DelegationNode {
                commits_ready: 2,
                duration: Some("6 min".into()),
                task: "Run the whole suite".into(),
                run: Some(RunOrder::Sequence),
                children: vec![
                    DelegationNode::new(
                        "build",
                        AgentKind::Antigravity,
                        AgentState::Done,
                        "Build the package",
                    ),
                    DelegationNode::new(
                        "suite",
                        AgentKind::OpenCode,
                        AgentState::Done,
                        "Run the suite",
                    ),
                ],
                ..DelegationNode::new(
                    "tests",
                    AgentKind::Antigravity,
                    AgentState::Done,
                    "Run the tests",
                )
            },
        ],
        ..DelegationNode::new(
            "lead",
            AgentKind::Claude,
            AgentState::Running,
            "Plan the release",
        )
    }]
}

fn delegation_tree(_: &mut Window, cx: &mut App) -> AnyElement {
    let bots = vec![DelegationNode {
        role: Some("Lead bot".into()),
        run: Some(RunOrder::Parallel),
        children: vec![
            DelegationNode {
                competence: Some("Research".into()),
                ..DelegationNode::new("mara", AgentKind::Codex, AgentState::Running, "Mara")
            },
            DelegationNode {
                competence: Some("Frontend".into()),
                ..DelegationNode::new("lio", AgentKind::Gemini, AgentState::Idle, "Lio")
            },
        ],
        ..DelegationNode::new("ada", AgentKind::Claude, AgentState::Running, "Ada")
    }];
    let frame = |height: f32, child: DelegationTree| div().h(px(height)).w_full().child(child);

    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant(
            "Workspaces: the agent's icon, its model and branch, the states, commits ready and a sequence",
            frame(
                420.,
                DelegationTree::new("gallery-tree-ws", delegation_sample())
                    .selected(Some("write".into())),
            ),
            cx,
        ))
        .child(variant(
            "Bot Teams: competence chips and plain words",
            frame(
                220.,
                DelegationTree::new("gallery-tree-bots", bots).variant(TreeVariant::BotTeams),
            ),
            cx,
        ))
        .child(variant(
            "Card form: one line per child",
            frame(
                200.,
                DelegationTree::new("gallery-tree-card", delegation_sample())
                    .variant(TreeVariant::Card),
            ),
            cx,
        ))
        .into_any_element()
}

/// One session that reports every core card, with the plan and the results expanded.
fn summary_sample(stale: bool) -> SessionSummary {
    SessionSummary {
        environment: Some(Environment {
            host: "VPS".into(),
            host_state: if stale {
                AgentState::Unreachable
            } else {
                AgentState::Running
            },
            last_known: Some("5 min ago".into()),
            account: Some("ada".into()),
            repository: Some("massorca".into()),
            branch: Some("main".into()),
            pipeline: Some("ci / build".into()),
        }),
        plan: Some(Plan {
            steps: vec![
                PlanStep::new("Read the spec", StepStatus::Done),
                PlanStep::new("Write the parser", StepStatus::InProgress),
                PlanStep::new("Ship", StepStatus::Pending),
            ],
            document: None,
        }),
        checklist: vec![
            ChecklistItem {
                text: "Tokens defined".into(),
                done: true,
            },
            ChecklistItem {
                text: "Grammar reviewed".into(),
                done: false,
            },
        ],
        context: Some(ContextUse {
            used: 35_000,
            total: 100_000,
        }),
        results: vec![
            ResultItem::Url {
                url: "http://localhost:3000/preview".into(),
                remote: false,
            },
            ResultItem::Url {
                url: "http://10.0.0.5:8080/docs".into(),
                remote: true,
            },
            ResultItem::File {
                path: "src/parser.rs".into(),
            },
            ResultItem::Artifact {
                name: "Parser notes".into(),
                kind: "Document".into(),
            },
        ],
        child_sessions: delegation_sample(),
        subagents: vec![Subagent {
            name: "Explore".into(),
            task: "Find the tokenizer".into(),
            state: AgentState::Running,
        }],
        connectors: vec![
            Connector {
                name: "Issue tracker".into(),
                connected: true,
                reason: None,
            },
            Connector {
                name: "Chat".into(),
                connected: false,
                reason: Some("Token expired".into()),
            },
        ],
        background: vec![BackgroundTask {
            name: "Watch build".into(),
            state: AgentState::Running,
            duration: Some("3 min".into()),
        }],
        qa_servers: vec![QaServer {
            name: "web".into(),
            port: 3000,
            url: "http://localhost:3000".into(),
            state: AgentState::Running,
        }],
        stale,
    }
}

/// The Session summary column at its default width, with the plan and the results expanded.
fn summary_story(stale: bool) -> AnyElement {
    let expanded: HashSet<CardKind> = [CardKind::Plan, CardKind::Results].into_iter().collect();
    let fresh: HashSet<CardKind> = [CardKind::Checklist].into_iter().collect();
    let column = summary_column(Rc::new(summary_sample(stale)), &expanded, &fresh, None);
    div().w(px(288.)).child(column).into_any_element()
}

fn session_summary(_: &mut Window, _: &mut App) -> AnyElement {
    summary_story(false)
}

fn session_summary_stale(_: &mut Window, _: &mut App) -> AnyElement {
    summary_story(true)
}
