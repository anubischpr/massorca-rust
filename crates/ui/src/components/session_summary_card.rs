//! SessionSummaryCard: the cards of the Session summary column, in a fixed order, each shown only
//! when it holds information (docs/design/components/SessionSummaryCard.md). This part is the
//! model: which cards show, the value in each header, and the data behind them. The view draws
//! what the model says.

use crate::components::agent_status::{AgentState, AgentStatus, AgentStatusSize};
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::delegation_tree::{DelegationNode, DelegationTree, TreeAction, TreeVariant};
use crate::components::diff_view::file_icon;
use crate::components::plan_view::{Plan, PlanAction, PlanVariant, PlanView, StepStatus};
use crate::components::switch::ease;
use crate::icon::{Icon, IconName, IconSize};
use crate::style::{FocusRing, StateFill, Surface};
use crate::theme::{ActiveTheme, TextStyled, box_shadows};
use crate::tokens::{
    Color, DURATION_CARD_INSERT, EASE_ENTER, Palette, RADIUS_LG, RADIUS_MD, SIZE_CONTROL_DEFAULT,
    SIZE_PROGRESS_HEIGHT, SPACE_1, SPACE_2, SPACE_3, TYPE_BODY_SM, TYPE_BODY_STRONG, TYPE_CAPTION,
    TYPE_MONO_SM,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    Animation, AnimationExt, AnyElement, App, Div, ElementId, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement,
    Styled, Window, div, px, relative,
};
use std::collections::HashSet;
use std::rc::Rc;

/// Rows a collapsed card shows at most.
pub const COLLAPSED_ROWS: usize = 6;

/// The column's line while no card holds information (D-AG-27 States, empty).
pub const EMPTY_TEXT: &str = "Nothing to summarise yet.";

/// The subtitle of the expanded Results card.
pub const RESULTS_SUBTITLE: &str =
    "URLs of the environment, file changes and visuals recorded in this chat.";

/// The cards of the column, in the order it shows them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CardKind {
    Environment,
    Plan,
    Checklist,
    Context,
    Results,
    ChildSessions,
    Subagents,
    Connectors,
    BackgroundTasks,
    QaServers,
}

impl CardKind {
    /// Every core card, in the fixed order.
    pub const ALL: [CardKind; 10] = [
        CardKind::Environment,
        CardKind::Plan,
        CardKind::Checklist,
        CardKind::Context,
        CardKind::Results,
        CardKind::ChildSessions,
        CardKind::Subagents,
        CardKind::Connectors,
        CardKind::BackgroundTasks,
        CardKind::QaServers,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Environment => "Environment",
            Self::Plan => "Plan",
            Self::Checklist => "Checklist",
            Self::Context => "Context",
            Self::Results => "Results",
            Self::ChildSessions => "Child sessions",
            Self::Subagents => "Subagents",
            Self::Connectors => "Connectors",
            Self::BackgroundTasks => "Background tasks",
            Self::QaServers => "QA servers",
        }
    }

    /// The icon on the left of the title (A23).
    pub fn icon(self) -> IconName {
        match self {
            Self::Environment => IconName::Server,
            Self::Plan => IconName::List,
            Self::Checklist => IconName::ListChecks,
            Self::Context => IconName::Gauge,
            Self::Results => IconName::WandSparkles,
            Self::ChildSessions => IconName::GitBranch,
            Self::Subagents => IconName::Users,
            Self::Connectors => IconName::Plug,
            Self::BackgroundTasks => IconName::LoaderCircle,
            Self::QaServers => IconName::Globe,
        }
    }

    /// Whether the host feeds the card, so the card dims while the host cannot be reached.
    pub fn host_fed(self) -> bool {
        !matches!(
            self,
            Self::Plan | Self::Checklist | Self::Context | Self::Results
        )
    }
}

/// The environment card: the host and the repository the session runs in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Environment {
    pub host: SharedString,
    pub host_state: AgentState,
    /// When the host was last seen, such as "5 min ago", while it cannot be reached.
    pub last_known: Option<SharedString>,
    pub account: Option<SharedString>,
    pub repository: Option<SharedString>,
    pub branch: Option<SharedString>,
    pub pipeline: Option<SharedString>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChecklistItem {
    pub text: SharedString,
    pub done: bool,
}

/// Context in use: the tokens used out of the total.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContextUse {
    pub used: u64,
    pub total: u64,
}

impl ContextUse {
    /// The share used, from 0 to 1 (above 1 once the total is exceeded).
    pub fn fraction(self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.used as f32 / self.total as f32
        }
    }

    /// The whole percent in the header.
    pub fn percent(self) -> u32 {
        (self.fraction() * 100.0).round() as u32
    }
}

/// One item of the Results card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResultItem {
    /// A URL on the environment; `remote` when it is on the remote host and cannot open here.
    Url { url: SharedString, remote: bool },
    /// A file the session changed; it opens as a Work panel tab.
    File { path: SharedString },
    /// An artifact, with its kind.
    Artifact {
        name: SharedString,
        kind: SharedString,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subagent {
    pub name: SharedString,
    pub task: SharedString,
    pub state: AgentState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Connector {
    pub name: SharedString,
    pub connected: bool,
    pub reason: Option<SharedString>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackgroundTask {
    pub name: SharedString,
    pub state: AgentState,
    pub duration: Option<SharedString>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QaServer {
    pub name: SharedString,
    pub port: u16,
    pub url: SharedString,
    pub state: AgentState,
}

/// What the session reports, as the Session summary shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionSummary {
    pub environment: Option<Environment>,
    pub plan: Option<Plan>,
    pub checklist: Vec<ChecklistItem>,
    pub context: Option<ContextUse>,
    pub results: Vec<ResultItem>,
    pub child_sessions: Vec<DelegationNode>,
    pub subagents: Vec<Subagent>,
    pub connectors: Vec<Connector>,
    pub background: Vec<BackgroundTask>,
    pub qa_servers: Vec<QaServer>,
    /// The host cannot be reached: host-fed cards keep their last values and dim.
    pub stale: bool,
}

impl SessionSummary {
    /// Whether a card holds information. A card without it is never drawn (A35).
    pub fn has(&self, kind: CardKind) -> bool {
        match kind {
            CardKind::Environment => self.environment.is_some(),
            CardKind::Plan => self
                .plan
                .as_ref()
                .is_some_and(|plan| !plan.steps.is_empty() || plan.document.is_some()),
            CardKind::Checklist => !self.checklist.is_empty(),
            CardKind::Context => self.context.is_some(),
            CardKind::Results => !self.results.is_empty(),
            CardKind::ChildSessions => !self.child_sessions.is_empty(),
            CardKind::Subagents => !self.subagents.is_empty(),
            CardKind::Connectors => !self.connectors.is_empty(),
            CardKind::BackgroundTasks => !self.background.is_empty(),
            CardKind::QaServers => !self.qa_servers.is_empty(),
        }
    }

    /// The cards that show, in the fixed order.
    pub fn visible(&self) -> Vec<CardKind> {
        CardKind::ALL
            .into_iter()
            .filter(|kind| self.has(*kind))
            .collect()
    }

    /// The Environment header's glyph and text: the host with its status, or, while the host
    /// cannot be reached, the unreachable glyph and "Last known state, 5 min ago".
    pub fn environment_value(&self) -> Option<(AgentState, String)> {
        let env = self.environment.as_ref()?;
        Some(match (self.stale, &env.last_known) {
            (true, Some(age)) => (AgentState::Unreachable, format!("Last known state, {age}")),
            (true, None) => (AgentState::Unreachable, env.host.to_string()),
            (false, _) => (env.host_state, env.host.to_string()),
        })
    }

    /// The value at the right of a header: "VPS", "3 of 7", "35%", "2 reported" and so on.
    pub fn value(&self, kind: CardKind) -> Option<String> {
        match kind {
            CardKind::Environment => self.environment.as_ref().map(|env| env.host.to_string()),
            CardKind::Plan => self
                .plan
                .as_ref()
                .filter(|plan| !plan.steps.is_empty())
                .map(|plan| {
                    let (done, total) = plan.progress();
                    format!("{done} of {total}")
                }),
            CardKind::Checklist => self.has(kind).then(|| {
                let done = self.checklist.iter().filter(|item| item.done).count();
                format!("{done} of {}", self.checklist.len())
            }),
            CardKind::Context => self
                .context
                .map(|context| format!("{}%", context.percent())),
            CardKind::Results => self
                .has(kind)
                .then(|| format!("{} reported", self.results.len())),
            CardKind::ChildSessions => self.has(kind).then(|| {
                let working = self
                    .child_sessions
                    .iter()
                    .filter(|node| node.state == AgentState::Running)
                    .count();
                format!("{} · {working} working", self.child_sessions.len())
            }),
            CardKind::Subagents => self.has(kind).then(|| {
                let running = self
                    .subagents
                    .iter()
                    .filter(|agent| agent.state == AgentState::Running)
                    .count();
                format!("{running} running")
            }),
            CardKind::Connectors => self.has(kind).then(|| {
                let connected = self.connectors.iter().filter(|link| link.connected).count();
                format!("{connected} connected")
            }),
            CardKind::BackgroundTasks => self.has(kind).then(|| {
                let running = self
                    .background
                    .iter()
                    .filter(|task| task.state == AgentState::Running)
                    .count();
                format!("{running} running")
            }),
            CardKind::QaServers => self
                .has(kind)
                .then(|| format!("{} detected", self.qa_servers.len())),
        }
    }
}

/// What the reader asked for. The owner keeps which cards are expanded and acts on the rest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SummaryAction {
    /// Expand or collapse a card.
    Toggle(CardKind),
    /// Open the plan as a Work panel tab.
    OpenPlan,
    /// Open a changed file in a Work panel tab.
    OpenFile(SharedString),
    /// Open an artifact in a Work panel tab.
    OpenArtifact(SharedString),
    /// Open a URL in the system browser.
    OpenUrl(SharedString),
    /// Copy a URL.
    CopyUrl(SharedString),
    /// An action from the child sessions tree.
    Tree(TreeAction),
}

/// Receives the actions of the Session summary cards.
pub type SummaryHandler = Rc<dyn Fn(SummaryAction, &mut Window, &mut App)>;

fn emit(
    handler: &Option<SummaryHandler>,
    action: SummaryAction,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(handler) = handler {
        handler(action, window, cx);
    }
}

/// A click listener that sends one action to the handler.
fn on(
    handler: &Option<SummaryHandler>,
    action: SummaryAction,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |window, cx| emit(&handler, action.clone(), window, cx)
}

/// One data row: an icon in the field's place and the text, truncated to one line.
fn data_row(
    icon: Option<IconName>,
    text: impl Into<SharedString>,
    color: Color,
    p: &Palette,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(SPACE_2))
        .min_w_0()
        .text_style(TYPE_BODY_SM)
        .text_color(color)
        .children(icon.map(|icon| {
            Icon::new(icon)
                .size(IconSize::XSmall)
                .color(p.text.tertiary)
        }))
        .child(div().min_w_0().truncate().child(text.into()))
}

/// A plan step as a line: its state glyph and text.
fn step_line(text: SharedString, status: StepStatus, color: Color) -> Div {
    let state = match status {
        StepStatus::Pending | StepStatus::Skipped => AgentState::Idle,
        StepStatus::InProgress => AgentState::Running,
        StepStatus::Done => AgentState::Done,
        StepStatus::Blocked => AgentState::Error,
    };
    div()
        .flex()
        .items_center()
        .gap(px(SPACE_2))
        .min_w_0()
        .text_style(TYPE_BODY_SM)
        .text_color(color)
        .child(AgentStatus::new(state).size(AgentStatusSize::XSmall))
        .child(div().min_w_0().truncate().child(text))
}

/// The rows of one card: its collapsed rows, or every row once it is expanded.
fn card_rows(
    kind: CardKind,
    summary: &SessionSummary,
    expanded: bool,
    p: &Palette,
    handler: &Option<SummaryHandler>,
) -> Vec<AnyElement> {
    let text = if summary.stale && kind.host_fed() {
        p.text.tertiary
    } else {
        p.text.secondary
    };
    let mut rows: Vec<AnyElement> = Vec::new();
    match kind {
        CardKind::Environment => {
            if let Some(env) = &summary.environment {
                let fields = [
                    (IconName::User, &env.account),
                    (IconName::Book, &env.repository),
                    (IconName::GitBranch, &env.branch),
                    (IconName::Workflow, &env.pipeline),
                ];
                rows.extend(fields.into_iter().filter_map(|(icon, value)| {
                    value
                        .clone()
                        .map(|value| data_row(Some(icon), value, text, p).into_any_element())
                }));
                if expanded {
                    rows.push(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(SPACE_2))
                            .child(AgentStatus::new(env.host_state).size(AgentStatusSize::XSmall))
                            .child(data_row(Some(IconName::Server), env.host.clone(), text, p))
                            .into_any_element(),
                    );
                    if let (true, Some(age)) = (summary.stale, &env.last_known) {
                        rows.push(
                            data_row(None, format!("Last known state, {age}"), p.text.tertiary, p)
                                .into_any_element(),
                        );
                    }
                }
            }
        }
        CardKind::Plan => {
            if let Some(plan) = &summary.plan {
                if expanded {
                    let plan_handler = handler.clone();
                    let view = PlanView::new("summary-plan", plan.clone())
                        .variant(PlanVariant::Card)
                        .on_action(move |action, window, cx| {
                            if action == PlanAction::Open {
                                emit(&plan_handler, SummaryAction::OpenPlan, window, cx);
                            }
                        });
                    rows.push(view.into_any_element());
                } else {
                    rows.extend(
                        [plan.current(), plan.next()]
                            .into_iter()
                            .flatten()
                            .filter_map(|path| plan.row_at(&path))
                            .map(|row| step_line(row.text, row.status, text).into_any_element()),
                    );
                }
            }
        }
        CardKind::Checklist => {
            rows.extend(summary.checklist.iter().map(|item| {
                let (icon, color) = if item.done {
                    (IconName::CircleCheck, p.status.success_fg)
                } else {
                    (IconName::Circle, p.text.tertiary)
                };
                div()
                    .flex()
                    .items_center()
                    .gap(px(SPACE_2))
                    .min_w_0()
                    .text_style(TYPE_BODY_SM)
                    .text_color(text)
                    .child(Icon::new(icon).size(IconSize::XSmall).color(color))
                    .child(div().min_w_0().truncate().child(item.text.clone()))
                    .into_any_element()
            }));
        }
        CardKind::Context => {
            if let Some(context) = summary.context {
                rows.push(
                    div()
                        .relative()
                        .w_full()
                        .h(px(SIZE_PROGRESS_HEIGHT))
                        .rounded_full()
                        .bg(p.progress.track)
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left_0()
                                .w(relative(context.fraction().min(1.0)))
                                .rounded_full()
                                .bg(p.accent.r#default),
                        )
                        .into_any_element(),
                );
                if expanded {
                    rows.push(
                        data_row(
                            None,
                            format!("{} of {} tokens used", context.used, context.total),
                            text,
                            p,
                        )
                        .into_any_element(),
                    );
                }
            }
        }
        CardKind::Results => {
            if expanded {
                rows.push(
                    div()
                        .text_style(TYPE_BODY_SM)
                        .text_color(p.text.secondary)
                        .child(RESULTS_SUBTITLE)
                        .into_any_element(),
                );
            }
            for (index, item) in summary.results.iter().enumerate() {
                let element = match item {
                    ResultItem::Url { url, remote } => {
                        let (open, copy) = (
                            Button::new(("summary-open", index), "Open")
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Compact)
                                .leading_icon(IconName::ExternalLink)
                                .disabled(*remote)
                                .on_click(on(handler, SummaryAction::OpenUrl(url.clone()))),
                            Button::new(("summary-copy", index), "Copy URL")
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Compact)
                                .leading_icon(IconName::Copy)
                                .on_click(on(handler, SummaryAction::CopyUrl(url.clone()))),
                        );
                        let mut column = div()
                            .flex()
                            .flex_col()
                            .gap(px(SPACE_1))
                            .child(data_row(Some(IconName::Globe), url.clone(), text, p))
                            .child(div().flex().gap(px(SPACE_2)).child(open).child(copy));
                        if *remote {
                            column = column.child(
                                div()
                                    .text_style(TYPE_CAPTION)
                                    .text_color(p.text.secondary)
                                    .child("This URL is on the remote host. Forward its port to open it on this PC."),
                            );
                        }
                        column.into_any_element()
                    }
                    ResultItem::File { path } => {
                        let file = handler.clone();
                        let path = path.clone();
                        div()
                            .id(("summary-file", index))
                            .flex()
                            .items_center()
                            .gap(px(SPACE_2))
                            .min_w_0()
                            .cursor_pointer()
                            .child(file_icon(&path, p))
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_style(TYPE_MONO_SM)
                                    .text_color(p.accent.fg)
                                    .underline()
                                    .child(path.clone()),
                            )
                            .on_click(move |_, window, cx| {
                                emit(&file, SummaryAction::OpenFile(path.clone()), window, cx)
                            })
                            .into_any_element()
                    }
                    ResultItem::Artifact { name, kind } => {
                        let artifact = handler.clone();
                        let name = name.clone();
                        div()
                            .id(("summary-artifact", index))
                            .cursor_pointer()
                            .child(data_row(Some(IconName::File), name.clone(), text, p))
                            .child(
                                div()
                                    .text_style(TYPE_CAPTION)
                                    .text_color(p.text.tertiary)
                                    .child(kind.clone()),
                            )
                            .on_click(move |_, window, cx| {
                                emit(
                                    &artifact,
                                    SummaryAction::OpenArtifact(name.clone()),
                                    window,
                                    cx,
                                )
                            })
                            .into_any_element()
                    }
                };
                rows.push(element);
            }
        }
        CardKind::ChildSessions => {
            let tree_handler = handler.clone();
            let variant = if expanded {
                TreeVariant::Workspaces
            } else {
                TreeVariant::Card
            };
            rows.push(
                DelegationTree::new("summary-children", summary.child_sessions.clone())
                    .variant(variant)
                    .on_action(move |action, window, cx| {
                        emit(&tree_handler, SummaryAction::Tree(action), window, cx)
                    })
                    .into_any_element(),
            );
        }
        CardKind::Subagents => {
            rows.extend(summary.subagents.iter().map(|agent| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(SPACE_2))
                    .min_w_0()
                    .child(AgentStatus::new(agent.state).size(AgentStatusSize::XSmall))
                    .child(data_row(None, agent.name.clone(), text, p))
                    .child(data_row(None, agent.task.clone(), p.text.tertiary, p))
                    .into_any_element()
            }));
        }
        CardKind::Connectors => {
            rows.extend(summary.connectors.iter().map(|link| {
                let state = if link.connected {
                    AgentState::Done
                } else {
                    AgentState::Error
                };
                let word = if link.connected {
                    "Connected"
                } else {
                    "Not connected"
                };
                let mut line = div()
                    .flex()
                    .items_center()
                    .gap(px(SPACE_2))
                    .min_w_0()
                    .child(AgentStatus::new(state).size(AgentStatusSize::XSmall))
                    .child(data_row(None, link.name.clone(), text, p))
                    .child(data_row(None, word, p.text.tertiary, p));
                if let (true, Some(reason)) = (expanded, &link.reason) {
                    line = line.child(data_row(None, reason.clone(), p.status.danger_fg, p));
                }
                line.into_any_element()
            }));
        }
        CardKind::BackgroundTasks => {
            rows.extend(summary.background.iter().map(|task| {
                let mut line = div()
                    .flex()
                    .items_center()
                    .gap(px(SPACE_2))
                    .min_w_0()
                    .child(AgentStatus::new(task.state).size(AgentStatusSize::XSmall))
                    .child(data_row(None, task.name.clone(), text, p));
                if let Some(duration) = &task.duration {
                    line = line.child(data_row(None, duration.clone(), p.text.tertiary, p));
                }
                line.into_any_element()
            }));
        }
        CardKind::QaServers => {
            rows.extend(summary.qa_servers.iter().map(|server| {
                let mut line = div()
                    .flex()
                    .items_center()
                    .gap(px(SPACE_2))
                    .min_w_0()
                    .child(AgentStatus::new(server.state).size(AgentStatusSize::XSmall))
                    .child(data_row(None, server.name.clone(), text, p))
                    .child(
                        div()
                            .text_style(TYPE_MONO_SM)
                            .text_color(p.text.tertiary)
                            .child(format!(":{}", server.port)),
                    );
                if expanded {
                    line = line.child(
                        div()
                            .text_style(TYPE_MONO_SM)
                            .text_color(text)
                            .child(server.url.clone()),
                    );
                }
                line.into_any_element()
            }));
        }
    }
    if !expanded {
        rows.truncate(COLLAPSED_ROWS);
    }
    rows
}

/// One card of the Session summary: a header button with the title and value, and its rows.
#[derive(IntoElement)]
pub struct SessionSummaryCard {
    kind: CardKind,
    summary: Rc<SessionSummary>,
    expanded: bool,
    fresh: bool,
    on_action: Option<SummaryHandler>,
}

impl std::fmt::Debug for SessionSummaryCard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionSummaryCard")
            .field("kind", &self.kind)
            .field("expanded", &self.expanded)
            .finish()
    }
}

impl SessionSummaryCard {
    pub fn new(kind: CardKind, summary: Rc<SessionSummary>) -> Self {
        Self {
            kind,
            summary,
            expanded: false,
            fresh: false,
            on_action: None,
        }
    }

    /// Shows every row of the card (the header's chevron points down).
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// The card has just appeared: it plays the insert over `duration.card-insert`.
    pub fn fresh(mut self, fresh: bool) -> Self {
        self.fresh = fresh;
        self
    }

    pub fn on_action(
        mut self,
        listener: impl Fn(SummaryAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(listener));
        self
    }
}

impl RenderOnce for SessionSummaryCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let kind = self.kind;
        let id: ElementId = format!("summary-card-{}", kind.title()).into();
        let focus = window
            .use_keyed_state(id.clone(), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let value = self.summary.value(kind);
        let env_value = (kind == CardKind::Environment)
            .then(|| self.summary.environment_value())
            .flatten();
        let dim = self.summary.stale && kind.host_fed();
        let title_color = if dim { p.text.tertiary } else { p.text.primary };
        let toggle = self.on_action.clone();
        let key_toggle = self.on_action.clone();
        let expanded = self.expanded;
        let header = div()
            .id((id.clone(), "header"))
            .role(Role::Button)
            .aria_label(match (&value, &env_value) {
                (Some(value), Some((_, shown))) if shown != value => {
                    format!("{}, {}, {}", kind.title(), value, shown)
                }
                (Some(value), _) => format!("{}, {}", kind.title(), value),
                (None, _) => kind.title().to_owned(),
            })
            .aria_expanded(expanded)
            .track_focus(&focus)
            .tab_stop(true)
            .focus_ring(&theme)
            .flex()
            .items_center()
            .gap(px(SPACE_2))
            .h(px(SIZE_CONTROL_DEFAULT))
            .px(px(SPACE_1))
            .rounded(px(RADIUS_MD))
            .cursor_pointer()
            .state_fills(&theme, Surface::Raised)
            .on_click(move |_, window, cx| emit(&toggle, SummaryAction::Toggle(kind), window, cx))
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    cx.stop_propagation();
                    emit(&key_toggle, SummaryAction::Toggle(kind), window, cx);
                }
            })
            .child(
                Icon::new(kind.icon())
                    .size(IconSize::Small)
                    .color(p.text.secondary),
            )
            .child(
                div()
                    .flex_none()
                    .text_style(TYPE_BODY_STRONG)
                    .text_color(title_color)
                    .child(kind.title()),
            )
            // The value takes the rest and truncates before the title does.
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(px(SPACE_1))
                    .text_style(TYPE_CAPTION)
                    .tabular_figures()
                    .text_color(p.text.secondary)
                    .children(
                        env_value.as_ref().map(|(state, _)| {
                            AgentStatus::new(*state).size(AgentStatusSize::XSmall)
                        }),
                    )
                    .children(
                        env_value
                            .map(|(_, text)| text)
                            .or(value)
                            .map(|text| div().min_w_0().truncate().child(text)),
                    ),
            )
            .child(
                Icon::new(if expanded {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .size(IconSize::Small)
                .color(p.text.secondary),
            );

        let rows = card_rows(kind, &self.summary, expanded, &p, &self.on_action);
        let card = div()
            .id(id.clone())
            .relative()
            .flex()
            .flex_col()
            .gap(px(SPACE_2))
            .min_w_0()
            .p(px(SPACE_3))
            .rounded(px(RADIUS_LG))
            .border_1()
            .border_color(p.border.default)
            .bg(p.surface.raised)
            .shadow(box_shadows(theme.shadows.e1))
            .child(header)
            .when(!rows.is_empty(), |el| {
                el.child(div().flex().flex_col().gap(px(SPACE_2)).children(rows))
            });
        if self.fresh && !cx.reduce_motion() {
            card.with_animation(
                (id, "card-insert"),
                Animation::new(DURATION_CARD_INSERT).with_easing(|t| ease(EASE_ENTER, t)),
                |card, t| card.opacity(t).top(px(SPACE_2 * (1.0 - t))),
            )
            .into_any_element()
        } else {
            card.into_any_element()
        }
    }
}

/// The Session summary column: the visible cards in their fixed order.
pub fn summary_column(
    summary: Rc<SessionSummary>,
    expanded: &HashSet<CardKind>,
    fresh: &HashSet<CardKind>,
    on_action: Option<SummaryHandler>,
) -> Div {
    if summary.visible().is_empty() {
        return div().child(EmptyLine);
    }
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .min_w_0()
        .children(summary.visible().into_iter().map(|kind| {
            let mut card = SessionSummaryCard::new(kind, summary.clone())
                .expanded(expanded.contains(&kind))
                .fresh(fresh.contains(&kind));
            card.on_action = on_action.clone();
            card
        }))
}

/// The column's text while no card holds information, in `text.tertiary`.
#[derive(IntoElement)]
struct EmptyLine;

impl RenderOnce for EmptyLine {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .debug_selector(|| "summary-empty".into())
            .text_style(TYPE_BODY_SM)
            .text_color(cx.theme().palette.text.tertiary)
            .child(EMPTY_TEXT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::plan_view::{PlanStep, StepStatus};
    use std::collections::HashSet;

    fn summary() -> SessionSummary {
        SessionSummary {
            environment: Some(Environment {
                host: "VPS".into(),
                host_state: AgentState::Running,
                last_known: None,
                account: Some("ada".into()),
                repository: Some("massorca".into()),
                branch: Some("main".into()),
                pipeline: None,
            }),
            plan: Some(Plan {
                steps: vec![
                    PlanStep::new("Read", StepStatus::Done),
                    PlanStep::new("Write", StepStatus::InProgress),
                    PlanStep::new("Ship", StepStatus::Pending),
                ],
                document: None,
            }),
            context: Some(ContextUse {
                used: 35,
                total: 100,
            }),
            ..SessionSummary::default()
        }
    }

    #[test]
    fn only_cards_with_information_show_in_the_fixed_order() {
        assert_eq!(
            summary().visible(),
            vec![CardKind::Environment, CardKind::Plan, CardKind::Context]
        );
    }

    #[test]
    fn a_plan_without_steps_or_document_is_not_a_card() {
        let mut empty = summary();
        empty.plan = Some(Plan::default());
        assert!(!empty.has(CardKind::Plan));
        empty.plan = Some(Plan {
            steps: Vec::new(),
            document: Some("Read the spec first.".into()),
        });
        assert!(empty.has(CardKind::Plan));
        assert_eq!(empty.value(CardKind::Plan), None);
    }

    #[test]
    fn headers_carry_the_values_the_card_reads() {
        let s = summary();
        assert_eq!(s.value(CardKind::Environment).as_deref(), Some("VPS"));
        assert_eq!(s.value(CardKind::Plan).as_deref(), Some("1 of 3"));
        assert_eq!(s.value(CardKind::Context).as_deref(), Some("35%"));
        assert_eq!(s.value(CardKind::Checklist), None);
    }

    #[test]
    fn counted_headers_follow_the_states() {
        let node = |id: &str, state| {
            DelegationNode::new(
                id,
                crate::components::agent_icon::AgentKind::Claude,
                state,
                "Task",
            )
        };
        let s = SessionSummary {
            checklist: vec![
                ChecklistItem {
                    text: "a".into(),
                    done: true,
                },
                ChecklistItem {
                    text: "b".into(),
                    done: true,
                },
                ChecklistItem {
                    text: "c".into(),
                    done: false,
                },
            ],
            results: vec![
                ResultItem::File {
                    path: "a.rs".into(),
                },
                ResultItem::File {
                    path: "b.rs".into(),
                },
            ],
            child_sessions: vec![
                node("a", AgentState::Running),
                node("b", AgentState::Done),
                node("c", AgentState::Done),
            ],
            background: vec![BackgroundTask {
                name: "build".into(),
                state: AgentState::Running,
                duration: None,
            }],
            qa_servers: vec![
                QaServer {
                    name: "web".into(),
                    port: 3000,
                    url: "http://localhost:3000".into(),
                    state: AgentState::Running,
                },
                QaServer {
                    name: "api".into(),
                    port: 8080,
                    url: "http://localhost:8080".into(),
                    state: AgentState::Running,
                },
            ],
            ..SessionSummary::default()
        };
        assert_eq!(s.value(CardKind::Checklist).as_deref(), Some("2 of 3"));
        assert_eq!(s.value(CardKind::Results).as_deref(), Some("2 reported"));
        assert_eq!(
            s.value(CardKind::ChildSessions).as_deref(),
            Some("3 · 1 working")
        );
        assert_eq!(
            s.value(CardKind::BackgroundTasks).as_deref(),
            Some("1 running")
        );
        assert_eq!(s.value(CardKind::QaServers).as_deref(), Some("2 detected"));
    }

    #[test]
    fn the_environment_header_shows_the_host_glyph_or_the_last_known_state() {
        let mut live = summary();
        assert_eq!(
            live.environment_value(),
            Some((AgentState::Running, "VPS".to_owned()))
        );
        live.stale = true;
        if let Some(env) = live.environment.as_mut() {
            env.last_known = Some("5 min ago".into());
        }
        assert_eq!(
            live.environment_value(),
            Some((
                AgentState::Unreachable,
                "Last known state, 5 min ago".to_owned()
            ))
        );
        assert_eq!(SessionSummary::default().environment_value(), None);
    }

    #[gpui::test]
    fn an_empty_column_says_nothing_to_summarise(cx: &mut gpui::TestAppContext) {
        struct Empty;
        impl gpui::Render for Empty {
            fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
                summary_column(
                    Rc::new(SessionSummary::default()),
                    &HashSet::new(),
                    &HashSet::new(),
                    None,
                )
            }
        }
        let (_, cx) = cx.add_window_view(|_, _| Empty);
        cx.run_until_parked();
        assert!(cx.debug_bounds("summary-empty").is_some());
        assert_eq!(EMPTY_TEXT, "Nothing to summarise yet.");
    }

    #[test]
    fn context_rounds_to_a_whole_percent_and_an_empty_total_reads_zero() {
        assert_eq!(ContextUse { used: 1, total: 3 }.percent(), 33);
        assert_eq!(ContextUse { used: 5, total: 0 }.percent(), 0);
    }

    #[test]
    fn every_core_card_is_listed_once_and_six_of_them_are_host_fed() {
        let titles: HashSet<&str> = CardKind::ALL.iter().map(|kind| kind.title()).collect();
        assert_eq!(titles.len(), CardKind::ALL.len());
        assert_eq!(
            CardKind::ALL.iter().filter(|kind| kind.host_fed()).count(),
            6
        );
    }
}
