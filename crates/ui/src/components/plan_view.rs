//! PlanView: the plan the agent reported, as steps with progress and an optional plan document
//! (docs/design/components/PlanView.md). This part is the model: the steps, the progress, the
//! current and next steps, the rows that show for the expansion state, and the card rows. The
//! view draws what the model says.

use crate::components::agent_status::{AgentState, AgentStatus, AgentStatusSize};
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::empty_state::EmptyState;
use crate::components::error_state::ErrorState;
use crate::components::segmented_control::{Segment, SegmentedControl};
use crate::components::skeleton::{LineWidth, Skeleton};
use crate::icon::{Icon, IconName, IconSize};
use crate::style::FocusRing;
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    Palette, RADIUS_FULL, RADIUS_MD, SIZE_ICON_SM, SIZE_ICON_XS, SIZE_PROGRESS_HEIGHT, SPACE_1,
    SPACE_2, SPACE_3, SPACE_4, SPACE_HALF, TYPE_BODY, TYPE_BODY_SM, TYPE_CAPTION, TYPE_TITLE_SM,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, Div, ElementId, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, RenderOnce, Role, ScrollHandle, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px, relative,
};
use std::collections::HashSet;
use std::rc::Rc;

/// Rows the Session summary card shows at most.
pub const CARD_ROWS: usize = 6;

/// The status of one step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    InProgress,
    Done,
    Skipped,
    Blocked,
}

impl StepStatus {
    /// The state word: the end of the row's accessible name.
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "not started",
            Self::InProgress => "in progress",
            Self::Done => "done",
            Self::Skipped => "skipped",
            Self::Blocked => "blocked",
        }
    }
}

/// One step, with its sub-steps. A blocked step carries its reason, shown under it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanStep {
    pub text: SharedString,
    pub status: StepStatus,
    pub reason: Option<SharedString>,
    pub children: Vec<PlanStep>,
}

impl PlanStep {
    pub fn new(text: impl Into<SharedString>, status: StepStatus) -> Self {
        Self {
            text: text.into(),
            status,
            reason: None,
            children: Vec::new(),
        }
    }

    pub fn with_children(mut self, children: Vec<PlanStep>) -> Self {
        self.children = children;
        self
    }

    pub fn blocked_by(mut self, reason: impl Into<SharedString>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

/// Where a step sits: its index at each level, from the top.
pub type StepPath = Vec<usize>;

/// The plan the agent reported: its steps and, when it wrote one, the plan document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub steps: Vec<PlanStep>,
    pub document: Option<SharedString>,
}

/// One visible row of the step list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub path: StepPath,
    pub depth: usize,
    pub text: SharedString,
    pub status: StepStatus,
    pub reason: Option<SharedString>,
    /// `None` for a step without sub-steps; otherwise whether its sub-steps show.
    pub expanded: Option<bool>,
}

impl Plan {
    /// Every step in document order, with its path.
    pub fn all(&self) -> Vec<(StepPath, &PlanStep)> {
        let mut out = Vec::new();
        collect(&self.steps, &mut Vec::new(), &mut out);
        out
    }

    /// The step at a path.
    pub fn step(&self, path: &[usize]) -> Option<&PlanStep> {
        let (first, rest) = path.split_first()?;
        let mut step = self.steps.get(*first)?;
        for index in rest {
            step = step.children.get(*index)?;
        }
        Some(step)
    }

    /// Finished steps and all steps. A skipped step counts as finished.
    pub fn progress(&self) -> (usize, usize) {
        let all = self.all();
        let done = all
            .iter()
            .filter(|(_, step)| matches!(step.status, StepStatus::Done | StepStatus::Skipped))
            .count();
        (done, all.len())
    }

    /// "3 of 7 steps", as the header reads it.
    pub fn progress_text(&self) -> String {
        let (done, total) = self.progress();
        format!("{done} of {total} steps")
    }

    /// The finished share of the steps, from 0 to 1.
    pub fn fraction(&self) -> f32 {
        let (done, total) = self.progress();
        if total == 0 {
            0.0
        } else {
            done as f32 / total as f32
        }
    }

    /// The first step in progress.
    pub fn current(&self) -> Option<StepPath> {
        self.all()
            .into_iter()
            .find(|(_, step)| step.status == StepStatus::InProgress)
            .map(|(path, _)| path)
    }

    /// The first pending step after the current one, or the first pending step when none is in
    /// progress.
    pub fn next(&self) -> Option<StepPath> {
        let all = self.all();
        let start = match self.current() {
            Some(current) => all
                .iter()
                .position(|(path, _)| *path == current)
                .map_or(0, |index| index + 1),
            None => 0,
        };
        all.into_iter()
            .skip(start)
            .find(|(_, step)| step.status == StepStatus::Pending)
            .map(|(path, _)| path)
    }

    /// The row for one step, with no sub-steps: the card's current and next lines.
    pub fn row_at(&self, path: &StepPath) -> Option<Row> {
        let step = self.step(path)?;
        Some(Row {
            path: path.clone(),
            depth: path.len().saturating_sub(1),
            text: step.text.clone(),
            status: step.status,
            reason: step.reason.clone(),
            expanded: None,
        })
    }

    /// The visible rows. Sub-steps show under the parents in `expanded` and nowhere else.
    pub fn rows(&self, expanded: &HashSet<StepPath>) -> Vec<Row> {
        let mut out = Vec::new();
        push_rows(&self.steps, &mut Vec::new(), 0, expanded, &mut out);
        out
    }

    /// The expansion that shows the current step: its ancestors and the step itself.
    pub fn default_expanded(&self) -> HashSet<StepPath> {
        let mut out = HashSet::new();
        if let Some(path) = self.current() {
            for len in 1..=path.len() {
                out.insert(path[..len].to_vec());
            }
        }
        out
    }

    /// The card form: every step with its sub-steps, cut to `CARD_ROWS`.
    pub fn card_rows(&self) -> Vec<Row> {
        let expanded: HashSet<StepPath> = self
            .all()
            .into_iter()
            .filter(|(_, step)| !step.children.is_empty())
            .map(|(path, _)| path)
            .collect();
        let mut rows = self.rows(&expanded);
        rows.truncate(CARD_ROWS);
        rows
    }
}

fn collect<'a>(
    steps: &'a [PlanStep],
    prefix: &mut StepPath,
    out: &mut Vec<(StepPath, &'a PlanStep)>,
) {
    for (index, step) in steps.iter().enumerate() {
        prefix.push(index);
        out.push((prefix.clone(), step));
        collect(&step.children, prefix, out);
        prefix.pop();
    }
}

fn push_rows(
    steps: &[PlanStep],
    prefix: &mut StepPath,
    depth: usize,
    expanded: &HashSet<StepPath>,
    out: &mut Vec<Row>,
) {
    for (index, step) in steps.iter().enumerate() {
        prefix.push(index);
        let open = expanded.contains(prefix);
        out.push(Row {
            path: prefix.clone(),
            depth,
            text: step.text.clone(),
            status: step.status,
            reason: step.reason.clone(),
            expanded: (!step.children.is_empty()).then_some(open),
        });
        if open {
            push_rows(&step.children, prefix, depth + 1, expanded, out);
        }
        prefix.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seven steps: three finished (one of them skipped), the second in progress with one
    /// sub-step done and one pending, and the last blocked with a reason.
    fn sample() -> Plan {
        Plan {
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
            document: None,
        }
    }

    #[test]
    fn progress_counts_done_and_skipped_steps_over_every_step() {
        let plan = sample();
        assert_eq!(plan.progress(), (3, 7));
        assert_eq!(plan.progress_text(), "3 of 7 steps");
        assert!((plan.fraction() - 3.0 / 7.0).abs() < 1e-6);
        assert_eq!(Plan::default().fraction(), 0.0);
    }

    #[test]
    fn current_is_the_step_in_progress_and_next_the_pending_step_after_it() {
        let plan = sample();
        assert_eq!(plan.current(), Some(vec![1]));
        assert_eq!(plan.next(), Some(vec![1, 1]));
        assert_eq!(
            plan.row_at(&vec![4]).and_then(|row| row.reason).as_deref(),
            Some("Waiting for review")
        );
    }

    #[test]
    fn next_without_a_step_in_progress_is_the_first_pending_step() {
        let plan = Plan {
            steps: vec![
                PlanStep::new("Done", StepStatus::Done),
                PlanStep::new("Later", StepStatus::Pending),
                PlanStep::new("Later still", StepStatus::Pending),
            ],
            document: None,
        };
        assert_eq!(plan.current(), None);
        assert_eq!(plan.next(), Some(vec![1]));
    }

    #[test]
    fn rows_show_sub_steps_only_under_expanded_parents() {
        let plan = sample();
        let folded = plan.rows(&HashSet::new());
        assert_eq!(folded.len(), 5);
        assert_eq!(folded[1].expanded, Some(false));
        assert_eq!(folded[0].expanded, None);
        let open = plan.rows(&plan.default_expanded());
        assert_eq!(open.len(), 7);
        assert_eq!(open[2].depth, 1);
        assert_eq!(open[2].text.as_ref(), "Tokens");
    }

    #[test]
    fn card_rows_show_every_step_with_sub_steps_up_to_the_limit() {
        let rows = sample().card_rows();
        assert_eq!(rows.len(), CARD_ROWS);
        assert_eq!(
            rows.last().map(|row| row.text.to_string()).as_deref(),
            Some("Publish notes")
        );
    }

    #[test]
    fn step_lookup_follows_the_path_and_empty_plans_have_nothing_to_show() {
        let plan = sample();
        assert_eq!(
            plan.step(&[1, 1])
                .map(|step| step.text.to_string())
                .as_deref(),
            Some("Grammar")
        );
        assert!(plan.step(&[9]).is_none());
        let empty = Plan::default();
        assert_eq!(empty.progress(), (0, 0));
        assert_eq!(empty.current(), None);
        assert_eq!(empty.next(), None);
        assert!(empty.rows(&HashSet::new()).is_empty());
    }

    #[test]
    fn status_labels_are_the_state_words() {
        let words: Vec<_> = [
            StepStatus::Pending,
            StepStatus::InProgress,
            StepStatus::Done,
            StepStatus::Skipped,
            StepStatus::Blocked,
        ]
        .map(StepStatus::label)
        .into();
        assert_eq!(
            words,
            vec!["not started", "in progress", "done", "skipped", "blocked"]
        );
    }
}

/// How the plan is loaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanStatus {
    /// The steps and the document draw.
    Ready,
    /// Skeleton rows while the plan loads.
    Loading,
    /// "The agent has not shared a plan yet."
    Empty,
    /// "Could not load the plan." with Retry; the reason is shown under it.
    Failed(SharedString),
    /// The host cannot be reached: the last plan stays, dated.
    Stale(SharedString),
}

/// The tab shows the whole plan; the card shows its first rows and an Open plan link.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlanVariant {
    #[default]
    Tab,
    Card,
}

/// What the reader asked for. The owner acts on it; the cursor and the expansion stay in the view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanAction {
    /// Open the plan as a Work panel tab.
    Open,
    /// Copy the text of a step.
    Copy(SharedString),
    /// Load the plan again.
    Retry,
}

/// Receives the actions of a PlanView.
pub type PlanActionHandler = Rc<dyn Fn(PlanAction, &mut Window, &mut App)>;

/// The view's own state, kept by its element id.
struct PlanUi {
    focus: FocusHandle,
    scroll: ScrollHandle,
    expanded: HashSet<StepPath>,
    cursor: usize,
    document: bool,
}

/// The plan: its header and progress, the steps or the document, and its states (PlanView.md).
#[derive(IntoElement)]
pub struct PlanView {
    id: ElementId,
    plan: Rc<Plan>,
    status: PlanStatus,
    variant: PlanVariant,
    updated: Option<SharedString>,
    on_action: Option<PlanActionHandler>,
}

impl std::fmt::Debug for PlanView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlanView")
            .field("id", &self.id)
            .field("status", &self.status)
            .finish()
    }
}

impl PlanView {
    pub fn new(id: impl Into<ElementId>, plan: impl Into<Rc<Plan>>) -> Self {
        Self {
            id: id.into(),
            plan: plan.into(),
            status: PlanStatus::Ready,
            variant: PlanVariant::Tab,
            updated: None,
            on_action: None,
        }
    }

    pub fn status(mut self, status: PlanStatus) -> Self {
        self.status = status;
        self
    }

    pub fn variant(mut self, variant: PlanVariant) -> Self {
        self.variant = variant;
        self
    }

    /// "Updated 2 min ago", as the caller words it.
    pub fn updated(mut self, text: impl Into<SharedString>) -> Self {
        self.updated = Some(text.into());
        self
    }

    pub fn on_action(
        mut self,
        listener: impl Fn(PlanAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(listener));
        self
    }
}

/// A click listener that sends one action to the handler.
fn on(
    handler: &Option<PlanActionHandler>,
    action: PlanAction,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |window, cx| {
        if let Some(handler) = &handler {
            handler(action.clone(), window, cx);
        }
    }
}

fn emit(
    handler: &Option<PlanActionHandler>,
    action: PlanAction,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(handler) = handler {
        handler(action, window, cx);
    }
}

/// Flips a step between shown and hidden sub-steps.
fn toggle(expanded: &mut HashSet<StepPath>, path: &StepPath) {
    if !expanded.remove(path) {
        expanded.insert(path.clone());
    }
}

/// Applies a key to the step list. Returns whether the key was used, and the action it asks for.
fn handle_key(
    rows: &[Row],
    state: &mut PlanUi,
    key: &str,
    secondary: bool,
) -> (bool, Option<PlanAction>) {
    let last = rows.len().saturating_sub(1);
    let row = rows.get(state.cursor);
    match key {
        "down" => state.cursor = (state.cursor + 1).min(last),
        "up" => state.cursor = state.cursor.saturating_sub(1),
        "home" => state.cursor = 0,
        "end" => state.cursor = last,
        "right" | "left" | "enter" => {
            let Some(row) = row else {
                return (false, None);
            };
            let Some(open) = row.expanded else {
                return (false, None);
            };
            let flip = match key {
                "right" => !open,
                "left" => open,
                _ => true,
            };
            if flip {
                toggle(&mut state.expanded, &row.path);
            }
        }
        "c" if secondary => return (true, row.map(|row| PlanAction::Copy(row.text.clone()))),
        _ => return (false, None),
    }
    (true, None)
}

/// The header's progress bar: a 4 px track with the accent fill to the finished share.
fn progress_track(fraction: f32, p: &Palette) -> Div {
    div()
        .relative()
        .w_full()
        .h(px(SIZE_PROGRESS_HEIGHT))
        .rounded(px(RADIUS_FULL))
        .bg(p.progress.track)
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left_0()
                .w(relative(fraction))
                .rounded(px(RADIUS_FULL))
                .bg(p.accent.r#default),
        )
}

/// The status glyph of a step. Done follows AgentStatus (ring with a check, agent.done).
fn glyph(status: StepStatus, stale: bool, p: &Palette) -> AnyElement {
    match status {
        StepStatus::Pending => AgentStatus::new(AgentState::Idle).into_any_element(),
        StepStatus::InProgress if stale => {
            AgentStatus::new(AgentState::Unreachable).into_any_element()
        }
        StepStatus::InProgress => AgentStatus::new(AgentState::Running).into_any_element(),
        StepStatus::Done => AgentStatus::new(AgentState::Done).into_any_element(),
        StepStatus::Blocked => AgentStatus::new(AgentState::Error).into_any_element(),
        StepStatus::Skipped => div()
            .relative()
            .flex_none()
            .size(px(SIZE_ICON_SM))
            .child(
                Icon::new(IconName::Circle)
                    .size(IconSize::Small)
                    .color(p.text.tertiary),
            )
            .child(
                div()
                    .absolute()
                    .top(px(SPACE_HALF))
                    .left(px(SPACE_HALF))
                    .child(
                        Icon::new(IconName::Minus)
                            .size(IconSize::XSmall)
                            .color(p.text.tertiary),
                    ),
            )
            .into_any_element(),
    }
}

/// What a step row needs from the view.
struct Look {
    p: Palette,
    stale: bool,
    current: Option<StepPath>,
    cursor: usize,
    focused: bool,
    interactive: bool,
    state: Entity<PlanUi>,
}

/// One step row: indent by depth, the expander, the glyph and the text.
fn step_row(index: usize, row: &Row, look: &Look) -> AnyElement {
    let p = &look.p;
    let current = look.current.as_ref() == Some(&row.path);
    let text_color = if row.status == StepStatus::Skipped {
        p.text.tertiary
    } else {
        p.text.primary
    };
    let expander = match row.expanded {
        Some(open) => {
            let icon = if open {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            };
            Icon::new(icon)
                .size(IconSize::XSmall)
                .color(p.text.secondary)
                .into_any_element()
        }
        None => div().flex_none().w(px(SIZE_ICON_XS)).into_any_element(),
    };
    let mut text = div().flex().flex_col().flex_1().min_w_0().child(
        div()
            .text_style(TYPE_BODY)
            .text_color(text_color)
            .child(row.text.clone()),
    );
    if let Some(reason) = row.reason.clone() {
        text = text.child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(p.text.secondary)
                .child(reason),
        );
    }
    let state = look.state.clone();
    let path = row.path.clone();
    let parent = row.expanded.is_some();
    let interactive = look.interactive;
    let hover = p.fill.hover_canvas;
    let focus_ring = p.border.focus;
    let mut el = div()
        .id(("plan-row", index))
        .relative()
        .flex()
        .items_start()
        .gap(px(SPACE_2))
        .py(px(SPACE_1))
        .pl(px(SPACE_2 + row.depth as f32 * SPACE_4))
        .pr(px(SPACE_2))
        .rounded(px(RADIUS_MD))
        .role(Role::ListItem)
        .aria_label(format!("{}, {}", row.text, row.status.label()))
        .hover(move |s| s.bg(hover))
        .when(current, |el| el.bg(p.accent.subtle_canvas))
        .when(look.focused && look.cursor == index, |el| {
            el.border_2().border_color(focus_ring)
        })
        .on_click(move |_, window, cx| {
            state.update(cx, |s, _| {
                s.cursor = index;
                if parent && interactive {
                    toggle(&mut s.expanded, &path);
                }
            });
            window.refresh();
        })
        .child(expander)
        .child(glyph(row.status, look.stale, p))
        .child(text);
    if current {
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
    el.into_any_element()
}

/// The skeleton of a plan that is loading: a header and six rows of two lengths.
fn loading_body(id: &ElementId, p: &Palette) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .p(px(SPACE_3))
        .id((id.clone(), "loading"))
        .children((0..6).map(|i| {
            let width = if i % 2 == 0 {
                LineWidth::Full
            } else {
                LineWidth::Most
            };
            Skeleton::line(width)
        }))
        .bg(p.surface.canvas)
        .into_any_element()
}

/// The plan document as plain paragraphs: no Markdown renderer exists yet, so the text is shown as
/// written, split at blank lines.
fn document_body(text: &str, p: &Palette) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .px(px(SPACE_3))
        .py(px(SPACE_3))
        .children(
            text.split("\n\n")
                .map(str::trim)
                .filter(|paragraph| !paragraph.is_empty())
                .map(|paragraph| {
                    div()
                        .text_style(TYPE_BODY)
                        .text_color(p.text.primary)
                        .child(paragraph.to_owned())
                }),
        )
        .into_any_element()
}

impl RenderOnce for PlanView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let id = self.id.clone();
        let plan = self.plan.clone();
        let init = plan.clone();
        let state = window.use_keyed_state(id.clone(), cx, move |_, cx| PlanUi {
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            expanded: init.default_expanded(),
            cursor: 0,
            document: init.steps.is_empty(),
        });
        let (focus, scroll, expanded, cursor, document) = {
            let s = state.read(cx);
            (
                s.focus.clone(),
                s.scroll.clone(),
                s.expanded.clone(),
                s.cursor,
                s.document,
            )
        };
        let card = self.variant == PlanVariant::Card;
        let interactive = !card;
        let stale = matches!(self.status, PlanStatus::Stale(_));
        let show_document = plan.document.is_some() && (plan.steps.is_empty() || document) && !card;
        let rows = if card {
            plan.card_rows()
        } else {
            plan.rows(&expanded)
        };
        let current = plan.current();
        let look = Look {
            p,
            stale,
            current: current.clone(),
            cursor,
            focused: focus.is_focused(window),
            interactive,
            state: state.clone(),
        };
        let handler = self.on_action.clone();

        let header = (!card).then(|| {
            let mode = (!plan.steps.is_empty() && plan.document.is_some()).then(|| {
                let state = state.clone();
                SegmentedControl::new(
                    "plan-mode",
                    "Plan view",
                    vec![Segment::text("Steps"), Segment::text("Document")],
                )
                .selected(usize::from(show_document))
                .on_change(move |index, window, cx| {
                    state.update(cx, |s, _| s.document = index == 1);
                    window.refresh();
                })
            });
            let caption = match &self.status {
                PlanStatus::Stale(age) => div()
                    .flex()
                    .items_center()
                    .gap(px(SPACE_1))
                    .child(AgentStatus::new(AgentState::Unreachable).size(AgentStatusSize::XSmall))
                    .child(
                        div()
                            .text_style(TYPE_CAPTION)
                            .text_color(p.text.tertiary)
                            .child(format!("Last update {age}. VPS can't be reached.")),
                    )
                    .into_any_element(),
                _ => div()
                    .text_style(TYPE_CAPTION)
                    .text_color(p.text.tertiary)
                    .child(self.updated.clone().unwrap_or_default())
                    .into_any_element(),
            };
            div()
                .flex()
                .flex_col()
                .gap(px(SPACE_2))
                .px(px(SPACE_3))
                .pt(px(SPACE_3))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(SPACE_2))
                        .child(
                            Icon::new(IconName::ListChecks)
                                .size(IconSize::Small)
                                .color(p.text.secondary),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_style(TYPE_TITLE_SM)
                                .text_color(p.text.primary)
                                .child("Plan"),
                        )
                        .child(
                            div()
                                .text_style(TYPE_CAPTION)
                                .tabular_figures()
                                .text_color(p.text.secondary)
                                .child(plan.progress_text()),
                        ),
                )
                .child(progress_track(plan.fraction(), &p))
                .child(caption)
                .children(mode)
        });

        let empty = plan.steps.is_empty() && plan.document.is_none();
        let body = match self.status.clone() {
            PlanStatus::Loading => loading_body(&id, &p),
            PlanStatus::Failed(reason) => ErrorState::new("plan-error", "Could not load the plan.")
                .explanation(reason)
                .on_retry(on(&handler, PlanAction::Retry))
                .into_any_element(),
            PlanStatus::Empty => {
                EmptyState::new("plan-empty", "The agent has not shared a plan yet.")
                    .into_any_element()
            }
            PlanStatus::Ready | PlanStatus::Stale(_) if empty => {
                EmptyState::new("plan-empty", "The agent has not shared a plan yet.")
                    .into_any_element()
            }
            PlanStatus::Ready | PlanStatus::Stale(_) if show_document => {
                document_body(plan.document.as_deref().unwrap_or_default(), &p)
            }
            PlanStatus::Ready | PlanStatus::Stale(_) => {
                let keys = plan.clone();
                let key_state = state.clone();
                let key_handler = handler.clone();
                let key_scroll = scroll.clone();
                let jump_index = current
                    .as_ref()
                    .and_then(|path| rows.iter().position(|row| &row.path == path));
                let jump = jump_index.map(|index| {
                    let state = state.clone();
                    Button::new("plan-jump", "Jump to current step")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Compact)
                        .leading_icon(IconName::Locate)
                        .on_click(move |window, cx| {
                            state.update(cx, |s, _| {
                                s.cursor = index;
                                s.scroll.scroll_to_item(index);
                            });
                            window.refresh();
                        })
                });
                let elements: Vec<AnyElement> = rows
                    .iter()
                    .enumerate()
                    .map(|(index, row)| step_row(index, row, &look))
                    .collect();
                let list = div()
                    .id((id.clone(), "steps"))
                    .role(Role::List)
                    .aria_label("Plan steps")
                    .track_focus(&focus)
                    .tab_stop(interactive)
                    .focus_ring(&theme)
                    .track_scroll(&scroll)
                    .overflow_y_scroll()
                    .flex_1()
                    .min_h_0()
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        if !interactive {
                            return;
                        }
                        let secondary = event.keystroke.modifiers.secondary(cx);
                        let key = event.keystroke.key.as_str();
                        let (used, action) = key_state.update(cx, |s, _| {
                            let rows = if card {
                                keys.card_rows()
                            } else {
                                keys.rows(&s.expanded)
                            };
                            let result = handle_key(&rows, s, key, secondary);
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
                    .children(elements);
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .child(list)
                    .children(jump.map(|button| {
                        div()
                            .flex()
                            .justify_end()
                            .px(px(SPACE_3))
                            .pb(px(SPACE_2))
                            .child(button)
                    }))
                    .into_any_element()
            }
        };

        let footer = card.then(|| {
            div()
                .flex()
                .justify_end()
                .px(px(SPACE_3))
                .pb(px(SPACE_2))
                .child(
                    Button::new("plan-open", "Open plan")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Compact)
                        .on_click(on(&handler, PlanAction::Open)),
                )
        });

        div()
            .id(id)
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .min_w_0()
            .children(header)
            .child(body)
            .children(footer)
    }
}
