//! SessionSummaryCard: the cards of the Session summary column, in a fixed order, each shown only
//! when it holds information (docs/design/components/SessionSummaryCard.md). This part is the
//! model: which cards show, the value in each header, and the data behind them. The view draws
//! what the model says.

use crate::components::agent_status::AgentState;
use crate::components::delegation_tree::DelegationNode;
use crate::components::plan_view::Plan;
use crate::icon::IconName;
use gpui::SharedString;

/// Rows a collapsed card shows at most.
pub const COLLAPSED_ROWS: usize = 6;

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
