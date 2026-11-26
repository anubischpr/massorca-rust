//! Element tests of the transcript: each row kind draws in timeline order, the answers reach only a pending and reachable
//! request, an opened thinking row shows its text, and the list keeps the newest row in view while rows arrive.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    Context, Entity, Modifiers, SharedString, TestAppContext, VisualTestContext, Window, px,
};

use super::*;
use crate::components::diff_view::{DiffModel, FileDiff, FileStatus};
use crate::components::plan_view::{Plan, PlanStep, StepStatus};

/// Draws one transcript and records what it asks for.
struct Host {
    rows: Rc<Vec<TranscriptRow>>,
    host: Option<SharedString>,
    asked: Rc<RefCell<Vec<TranscriptAction>>>,
    /// How many times the list said its first row is in view.
    reached: Rc<Cell<u32>>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let asked = self.asked.clone();
        let reached = self.reached.clone();
        TranscriptView::new("transcript", self.rows.clone())
            .host_unreachable(self.host.clone())
            .on_action(move |action, _, _| asked.borrow_mut().push(action))
            .on_reach_top(move |_, _| reached.set(reached.get() + 1))
    }
}

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

type Asked = Rc<RefCell<Vec<TranscriptAction>>>;

fn open(
    cx: &mut TestAppContext,
    rows: Vec<TranscriptRow>,
    host: Option<SharedString>,
) -> (Entity<Host>, Asked, &mut VisualTestContext) {
    let asked: Asked = Rc::default();
    let sink = asked.clone();
    let (view, cx) = cx.add_window_view(|_, _| Host {
        rows: Rc::new(rows),
        host,
        asked: sink,
        reached: Rc::default(),
    });
    draw(cx);
    (view, asked, cx)
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is not drawn"));
    cx.simulate_click(bounds.center(), Modifiers::none());
    cx.run_until_parked();
    draw(cx);
}

fn prose(id: &str, text: &str) -> TranscriptRow {
    TranscriptRow::Prose {
        id: id.into(),
        text: text.into(),
    }
}

fn approval(id: &str, state: ApprovalState, can_always: bool) -> TranscriptRow {
    TranscriptRow::Approval {
        id: id.into(),
        approval: ApprovalRow {
            request_id: format!("req-{id}").into(),
            title: "Run command?".into(),
            input: "cargo test".into(),
            truncated: false,
            state,
            can_always,
            suggestions: if can_always {
                vec!["Bash(cargo test) for this session".into()]
            } else {
                Vec::new()
            },
        },
    }
}

#[gpui::test]
fn text_rows_draw_in_timeline_order(cx: &mut TestAppContext) {
    let rows = vec![
        TranscriptRow::UserMessage {
            id: "ask".into(),
            text: "Fix the parser.".into(),
        },
        prose("answer", "Reading the parser now."),
        TranscriptRow::Thinking {
            id: "think".into(),
            text: "The cursor is stale.".into(),
            streaming: false,
        },
        TranscriptRow::Step {
            id: "tool".into(),
            step: StepRow {
                title: "Read".into(),
                detail: "src/parser.rs".into(),
                state: StepState::Done,
                output: None,
                truncated: false,
            },
        },
        TranscriptRow::Notice {
            id: "note".into(),
            level: NoticeLevel::Warning,
            text: "Retrying the prompt.".into(),
        },
        TranscriptRow::Turn {
            id: "turn".into(),
            state: TurnState::Running,
        },
    ];
    let (_view, _asked, cx) = open(cx, rows, None);

    let tops: Vec<gpui::Pixels> = [
        "transcript-row-ask",
        "transcript-row-answer",
        "transcript-row-think",
        "transcript-row-tool",
        "transcript-row-note",
        "transcript-row-turn",
    ]
    .iter()
    .map(|selector| {
        cx.debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} is not drawn"))
            .origin
            .y
    })
    .collect();
    assert!(
        tops.windows(2).all(|pair| pair[0] < pair[1]),
        "rows out of order: {tops:?}"
    );
}

#[gpui::test]
fn a_plan_and_a_truncated_diff_draw_in_their_rows(cx: &mut TestAppContext) {
    let plan = Rc::new(Plan {
        steps: vec![
            PlanStep::new("Read the spec", StepStatus::Done),
            PlanStep::new("Write the view", StepStatus::InProgress),
        ],
        document: None,
    });
    let diff = Rc::new(DiffModel::new(FileDiff {
        path: "src/parser.rs".into(),
        status: FileStatus::Modified,
        hunks: Vec::new(),
        hidden_after: Vec::new(),
    }));
    let rows = vec![
        TranscriptRow::Plan {
            id: "plan".into(),
            plan,
        },
        TranscriptRow::Diff {
            id: "diff".into(),
            diff,
            truncated: true,
        },
    ];
    let (_view, _asked, cx) = open(cx, rows, None);

    assert!(cx.debug_bounds("transcript-row-plan").is_some());
    assert!(cx.debug_bounds("transcript-row-diff").is_some());
}

#[gpui::test]
fn a_pending_approval_offers_deny_and_approve_and_always_only_when_it_can(cx: &mut TestAppContext) {
    let rows = vec![
        approval("plain", ApprovalState::Pending, false),
        approval("offered", ApprovalState::Pending, true),
    ];
    let (_view, _asked, cx) = open(cx, rows, None);

    assert!(cx.debug_bounds("button:plain-Deny").is_some());
    assert!(cx.debug_bounds("button:plain-Approve").is_some());
    assert!(cx.debug_bounds("button:plain-Always").is_none());
    assert!(cx.debug_bounds("button:offered-Always").is_some());
}

#[gpui::test]
fn choosing_approve_asks_for_the_allow_answer_to_that_request(cx: &mut TestAppContext) {
    let (_view, asked, cx) = open(
        cx,
        vec![approval("plain", ApprovalState::Pending, false)],
        None,
    );

    click(cx, "button:plain-Approve");

    assert_eq!(
        *asked.borrow(),
        vec![TranscriptAction::Answer {
            request_id: "req-plain".into(),
            answer: ApprovalAnswer::Allow,
        }]
    );
}

#[gpui::test]
fn choosing_always_asks_for_the_always_answer_when_it_is_offered(cx: &mut TestAppContext) {
    let (_view, asked, cx) = open(
        cx,
        vec![approval("offered", ApprovalState::Pending, true)],
        None,
    );

    click(cx, "button:offered-Always");

    assert_eq!(
        *asked.borrow(),
        vec![TranscriptAction::Answer {
            request_id: "req-offered".into(),
            answer: ApprovalAnswer::Always,
        }]
    );
}

#[gpui::test]
fn an_unreachable_host_disables_the_answers(cx: &mut TestAppContext) {
    let (_view, asked, cx) = open(
        cx,
        vec![approval("plain", ApprovalState::Pending, true)],
        Some("VPS".into()),
    );

    click(cx, "button:plain-Approve");

    assert!(asked.borrow().is_empty());
}

#[gpui::test]
fn a_settled_approval_is_one_line_and_offers_no_answer(cx: &mut TestAppContext) {
    let rows = vec![
        approval("done", ApprovalState::Allowed, true),
        approval("cut", ApprovalState::Cancelled, true),
    ];
    let (_view, _asked, cx) = open(cx, rows, None);

    assert!(cx.debug_bounds("transcript-row-done").is_some());
    assert!(cx.debug_bounds("transcript-row-cut").is_some());
    assert!(cx.debug_bounds("button:done-Approve").is_none());
    assert!(cx.debug_bounds("button:cut-Deny").is_none());
}

#[gpui::test]
fn opening_a_thinking_row_shows_its_text(cx: &mut TestAppContext) {
    let rows = vec![TranscriptRow::Thinking {
        id: "think".into(),
        text: "The cursor is stale.".into(),
        streaming: false,
    }];
    let (_view, _asked, cx) = open(cx, rows, None);
    assert!(cx.debug_bounds("think-thinking-text").is_none());

    click(cx, "think-thinking-toggle");
    assert!(cx.debug_bounds("think-thinking-text").is_some());

    click(cx, "think-thinking-toggle");
    assert!(cx.debug_bounds("think-thinking-text").is_none());
}

#[gpui::test]
fn the_list_keeps_the_newest_row_in_view_while_rows_arrive(cx: &mut TestAppContext) {
    let mut rows: Vec<TranscriptRow> = (0..40)
        .map(|i| prose(&format!("p{i}"), &format!("Paragraph {i}.")))
        .collect();
    let (view, _asked, cx) = open(cx, rows.clone(), None);
    assert_newest_in_view(cx, "transcript-row-p39");

    rows.push(prose("p40", "Paragraph 40."));
    view.update(cx, |host, cx| {
        host.rows = Rc::new(rows);
        cx.notify();
    });
    draw(cx);
    assert_newest_in_view(cx, "transcript-row-p40");
}

fn assert_newest_in_view(cx: &mut VisualTestContext, row: &'static str) {
    let viewport = cx
        .debug_bounds("transcript-list")
        .expect("the list is drawn");
    let newest = cx
        .debug_bounds(row)
        .unwrap_or_else(|| panic!("{row} is not drawn"));
    assert!(
        newest.bottom() <= viewport.bottom() + px(1.0),
        "{row} is below the viewport: {newest:?} in {viewport:?}"
    );
}

#[gpui::test]
fn an_always_approval_lists_what_it_would_write_and_a_plain_one_lists_nothing(
    cx: &mut TestAppContext,
) {
    let rows = vec![
        approval("plain", ApprovalState::Pending, false),
        approval("offered", ApprovalState::Pending, true),
    ];
    let (_view, _asked, cx) = open(cx, rows, None);

    assert!(cx.debug_bounds("approval-always-offered").is_some());
    assert!(cx.debug_bounds("approval-always-plain").is_none());
}

fn numbered(range: std::ops::Range<u32>) -> Vec<TranscriptRow> {
    range
        .map(|i| prose(&format!("p{i}"), &format!("Paragraph {i}.")))
        .collect()
}

#[gpui::test]
fn rows_added_above_keep_the_newest_in_view_and_ask_for_nothing(cx: &mut TestAppContext) {
    let (view, _asked, cx) = open(cx, numbered(40..80), None);
    assert_newest_in_view(cx, "transcript-row-p79");

    view.update(cx, |host, cx| {
        host.rows = Rc::new(numbered(0..80));
        cx.notify();
    });
    draw(cx);

    assert_newest_in_view(cx, "transcript-row-p79");
    assert_eq!(
        view.read_with(cx, |host, _| host.reached.get()),
        0,
        "drawing rows never asks for older ones; only a scroll to the first row does"
    );
}
