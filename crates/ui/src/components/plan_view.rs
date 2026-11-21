//! PlanView: the plan the agent reported, as steps with progress and an optional plan document
//! (docs/design/components/PlanView.md). This part is the model: the steps, the progress, the
//! current and next steps, the rows that show for the expansion state, and the card rows. The
//! view draws what the model says.

use gpui::SharedString;
use std::collections::HashSet;

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
