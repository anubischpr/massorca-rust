//! The model picker's rules without a window: the choice, the favourites, the rail view, the
//! search and the highlighted row. The popover in `model_picker.rs` draws them.

use gpui::SharedString;

use crate::components::agent_icon::AgentKind;

/// Whether an agent can serve models right now; the rail says why in its tooltip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentAvailability {
    Ready,
    SignedOut,
    Missing,
}

/// Whether an agent's model list has loaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelList {
    Loaded,
    Loading,
    Failed,
}

/// One model the picker offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelOption {
    pub id: SharedString,
    pub name: SharedString,
    /// Reasoning levels in order ("Low" to "Max"); empty when the model has none.
    pub efforts: Vec<SharedString>,
    /// The level taken when the model is chosen and the previous level is not offered.
    pub default_effort: usize,
    /// Whether the model offers Fast speed.
    pub fast: bool,
}

/// One agent or provider and its models.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentOption {
    pub kind: AgentKind,
    pub availability: AgentAvailability,
    pub list: ModelList,
    pub models: Vec<ModelOption>,
}

/// The chosen model, reasoning level and speed; what the trigger shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub model: SharedString,
    /// Index into the model's levels; `None` when the model has none.
    pub effort: Option<usize>,
    pub fast: bool,
}

/// Which list the rail shows: the favourites or one agent's models.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerView {
    Favourites,
    Agent(usize),
}

/// A model row: its agent and its position in that agent's list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowRef {
    pub agent: usize,
    pub model: usize,
}

/// The picker's rules without any window: the choice, favourites, the rail view, the
/// search and the highlighted row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerState {
    agents: Vec<AgentOption>,
    choice: Choice,
    favourites: Vec<SharedString>,
    view: PickerView,
    query: String,
    expanded: bool,
    highlight: usize,
}

impl PickerState {
    /// Rows an agent's list shows before "Show more", and the `Mod+1` to `Mod+4` targets.
    pub const FEATURED_ROWS: usize = 4;

    /// Starts on the agent that offers the chosen model, or on favourites when none does.
    pub fn new(agents: Vec<AgentOption>, choice: Choice) -> Self {
        let view = agents
            .iter()
            .position(|agent| agent.models.iter().any(|model| model.id == choice.model))
            .map_or(PickerView::Favourites, PickerView::Agent);
        Self {
            agents,
            choice,
            favourites: Vec::new(),
            view,
            query: String::new(),
            expanded: false,
            highlight: 0,
        }
    }

    /// Starred models, in the order the user starred them.
    pub fn with_favourites(mut self, ids: Vec<SharedString>) -> Self {
        self.favourites = ids;
        self
    }

    pub fn agents(&self) -> &[AgentOption] {
        &self.agents
    }

    pub fn choice(&self) -> &Choice {
        &self.choice
    }

    pub fn view(&self) -> PickerView {
        self.view
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn expanded(&self) -> bool {
        self.expanded
    }

    pub fn highlight(&self) -> usize {
        self.highlight
    }

    pub fn favourites(&self) -> &[SharedString] {
        &self.favourites
    }

    pub fn is_favourite(&self, id: &str) -> bool {
        self.favourites
            .iter()
            .any(|favourite| favourite.as_ref() == id)
    }

    /// The row of a model id, across every agent.
    pub fn locate(&self, id: &str) -> Option<RowRef> {
        self.agents.iter().enumerate().find_map(|(agent, option)| {
            option
                .models
                .iter()
                .position(|model| model.id.as_ref() == id)
                .map(|model| RowRef { agent, model })
        })
    }

    pub fn model(&self, row: RowRef) -> Option<&ModelOption> {
        self.agents.get(row.agent)?.models.get(row.model)
    }

    pub fn current_model(&self) -> Option<&ModelOption> {
        self.model(self.locate(&self.choice.model)?)
    }

    /// The agent the chosen model belongs to; the first agent when the model is unknown.
    pub fn current_kind(&self) -> AgentKind {
        self.locate(&self.choice.model)
            .and_then(|row| self.agents.get(row.agent))
            .or_else(|| self.agents.first())
            .map_or(AgentKind::Unknown, |agent| agent.kind)
    }

    /// The model name and the reasoning level the trigger shows; the level is absent when the
    /// model has none.
    pub fn chip_labels(&self) -> (SharedString, Option<SharedString>) {
        let Some(model) = self.current_model() else {
            return (SharedString::default(), None);
        };
        let effort = self
            .choice
            .effort
            .and_then(|index| model.efforts.get(index).cloned());
        (model.name.clone(), effort)
    }

    /// The rows the list shows, in order. A search reads every agent; favourites read in
    /// starring order; an agent shows its featured rows until "Show more".
    pub fn rows(&self) -> Vec<RowRef> {
        if !self.query.is_empty() {
            let needle = self.query.to_lowercase();
            let mut rows = Vec::new();
            for (agent, option) in self.agents.iter().enumerate() {
                let agent_matches = option.kind.label().to_lowercase().contains(&needle);
                for (model, item) in option.models.iter().enumerate() {
                    if agent_matches || item.name.to_lowercase().contains(&needle) {
                        rows.push(RowRef { agent, model });
                    }
                }
            }
            return rows;
        }
        match self.view {
            PickerView::Favourites => self
                .favourites
                .iter()
                .filter_map(|id| self.locate(id))
                .collect(),
            PickerView::Agent(agent) => {
                let count = self
                    .agents
                    .get(agent)
                    .map_or(0, |option| option.models.len());
                let shown = if self.expanded {
                    count
                } else {
                    count.min(Self::FEATURED_ROWS)
                };
                (0..shown).map(|model| RowRef { agent, model }).collect()
            }
        }
    }

    /// The row a `Mod+number` shortcut selects: the nth of the first four rows of the view.
    pub fn shortcut_row(&self, number: usize) -> Option<RowRef> {
        if !(1..=Self::FEATURED_ROWS).contains(&number) {
            return None;
        }
        self.rows().get(number - 1).copied()
    }

    pub fn highlighted_row(&self) -> Option<RowRef> {
        self.rows().get(self.highlight).copied()
    }

    /// Moves the highlight by `delta` rows, stopping at the first and the last.
    pub fn move_highlight(&mut self, delta: isize) {
        let last = self.rows().len().saturating_sub(1);
        self.highlight = (self.highlight as isize + delta).clamp(0, last as isize) as usize;
    }

    pub fn select_view(&mut self, view: PickerView) {
        self.view = view;
        self.expanded = false;
        self.highlight = 0;
    }

    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_owned();
        self.highlight = 0;
    }

    pub fn expand(&mut self) {
        self.expanded = true;
    }

    /// Chooses a model. The reasoning level carries over when the model offers a level with
    /// the same name; otherwise the model's default. Fast stays only when the model offers it.
    pub fn select_model(&mut self, row: RowRef) -> bool {
        let Some(model) = self.model(row) else {
            return false;
        };
        let kept = self
            .choice
            .effort
            .and_then(|index| self.current_model()?.efforts.get(index).cloned());
        let effort = if model.efforts.is_empty() {
            None
        } else {
            let carried =
                kept.and_then(|name| model.efforts.iter().position(|level| *level == name));
            Some(carried.unwrap_or(model.default_effort.min(model.efforts.len() - 1)))
        };
        self.choice = Choice {
            model: model.id.clone(),
            effort,
            fast: self.choice.fast && model.fast,
        };
        true
    }

    /// Chooses a reasoning level of the chosen model; `false` when there is no such level.
    pub fn select_effort(&mut self, index: usize) -> bool {
        let offered = self
            .current_model()
            .is_some_and(|model| index < model.efforts.len());
        if offered {
            self.choice.effort = Some(index);
        }
        offered
    }

    /// Chooses Standard or Fast speed; Fast needs a model that offers it.
    pub fn set_fast(&mut self, fast: bool) -> bool {
        let supported = self.current_model().is_some_and(|model| model.fast);
        if fast && !supported {
            return false;
        }
        self.choice.fast = fast;
        true
    }

    /// Stars a model, or removes its star.
    pub fn toggle_favourite(&mut self, id: &str) {
        match self
            .favourites
            .iter()
            .position(|favourite| favourite.as_ref() == id)
        {
            Some(index) => {
                self.favourites.remove(index);
            }
            None => self.favourites.push(SharedString::from(id.to_owned())),
        }
    }
}

/// Fixtures shared by the picker's tests: two agents, one with levels, Fast and a model without
/// levels, and one that is signed out.
#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;

    pub(crate) fn model(
        id: &str,
        name: &str,
        efforts: &[&str],
        default: usize,
        fast: bool,
    ) -> ModelOption {
        ModelOption {
            id: id.to_owned().into(),
            name: name.to_owned().into(),
            efforts: efforts
                .iter()
                .map(|level| (*level).to_owned().into())
                .collect(),
            default_effort: default,
            fast,
        }
    }

    pub(crate) fn agent(kind: AgentKind, models: Vec<ModelOption>) -> AgentOption {
        AgentOption {
            kind,
            availability: AgentAvailability::Ready,
            list: ModelList::Loaded,
            models,
        }
    }

    pub(crate) fn sample() -> PickerState {
        let levels = ["Low", "Medium", "High", "Max"];
        let claude = agent(
            AgentKind::Claude,
            vec![
                model("opus", "Opus 5.5", &levels, 1, true),
                model("sonnet", "Sonnet 5", &levels[..3], 0, false),
                model("haiku", "Haiku 5.5", &[], 0, false),
                model("opus-old", "Opus 4.1", &levels, 2, false),
                model("sonnet-old", "Sonnet 4", &levels, 1, false),
                model("haiku-old", "Haiku 4", &[], 0, false),
            ],
        );
        let codex = agent(
            AgentKind::Codex,
            vec![model("gpt", "GPT-5 Codex", &["Low", "High"], 0, true)],
        );
        PickerState::new(
            vec![claude, codex],
            Choice {
                model: "opus".into(),
                effort: Some(1),
                fast: false,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::sample;
    use super::*;

    fn names(state: &PickerState) -> Vec<String> {
        state
            .rows()
            .into_iter()
            .filter_map(|row| state.model(row).map(|model| model.name.to_string()))
            .collect()
    }

    #[test]
    fn an_agent_shows_four_featured_rows_until_show_more() {
        let mut state = sample();
        assert_eq!(names(&state).len(), PickerState::FEATURED_ROWS);
        state.expand();
        assert_eq!(names(&state).len(), 6);
    }

    #[test]
    fn a_search_reads_every_agent_and_matches_names_and_agents_case_insensitively() {
        let mut state = sample();
        state.set_query("CODEX");
        assert_eq!(names(&state), vec!["GPT-5 Codex"]);
        state.set_query("opus");
        assert_eq!(names(&state), vec!["Opus 5.5", "Opus 4.1"]);
    }

    #[test]
    fn favourites_read_in_the_order_they_were_starred() {
        let mut state = sample();
        state.toggle_favourite("gpt");
        state.toggle_favourite("opus");
        state.select_view(PickerView::Favourites);
        assert_eq!(names(&state), vec!["GPT-5 Codex", "Opus 5.5"]);
        state.toggle_favourite("gpt");
        assert_eq!(names(&state), vec!["Opus 5.5"]);
    }

    #[test]
    fn mod_digits_select_the_first_four_rows_of_the_view() {
        let state = sample();
        assert_eq!(state.shortcut_row(1), state.locate("opus"));
        assert_eq!(state.shortcut_row(4), state.locate("opus-old"));
        assert_eq!(state.shortcut_row(5), None);
        assert_eq!(state.shortcut_row(0), None);
    }

    #[test]
    fn choosing_a_model_keeps_the_level_name_when_offered_else_takes_its_default() {
        let mut state = sample();
        let sonnet = state.locate("sonnet").unwrap();
        assert!(state.select_model(sonnet));
        assert_eq!(state.choice().effort, Some(1), "Medium carries over");
        let haiku = state.locate("haiku").unwrap();
        assert!(state.select_model(haiku));
        assert_eq!(
            state.choice().effort,
            None,
            "a model without levels has none"
        );
        let opus_old = state.locate("opus-old").unwrap();
        state.select_model(state.locate("opus").unwrap());
        state.select_effort(3);
        state.select_model(opus_old);
        assert_eq!(
            state.choice().effort,
            Some(3),
            "Max is offered, so it stays"
        );
    }

    #[test]
    fn fast_stays_only_on_models_that_offer_it() {
        let mut state = sample();
        assert!(state.set_fast(true));
        assert!(state.select_model(state.locate("sonnet").unwrap()));
        assert!(!state.choice().fast);
        assert!(!state.set_fast(true), "Sonnet has no Fast mode");
    }

    #[test]
    fn an_effort_outside_the_model_levels_is_refused() {
        let mut state = sample();
        assert!(!state.select_effort(9));
        assert_eq!(state.choice().effort, Some(1));
        state.select_model(state.locate("haiku").unwrap());
        assert!(!state.select_effort(0));
    }

    #[test]
    fn the_trigger_shows_the_model_and_only_a_level_the_model_has() {
        let mut state = sample();
        assert_eq!(
            state.chip_labels(),
            ("Opus 5.5".into(), Some("Medium".into()))
        );
        state.select_model(state.locate("haiku").unwrap());
        assert_eq!(state.chip_labels(), ("Haiku 5.5".into(), None));
    }

    #[test]
    fn the_highlight_stays_inside_the_visible_rows() {
        let mut state = sample();
        state.move_highlight(-1);
        assert_eq!(state.highlight(), 0);
        state.move_highlight(10);
        assert_eq!(state.highlight(), PickerState::FEATURED_ROWS - 1);
    }

    #[test]
    fn a_model_without_a_match_shows_no_rows() {
        let mut state = sample();
        state.set_query("zzz");
        assert!(state.rows().is_empty());
        assert_eq!(state.highlighted_row(), None);
    }
}
