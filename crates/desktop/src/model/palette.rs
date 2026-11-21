//! Command palette state: grouping, filtering, the active row (D-CP-01, A13).

use super::shell::{Agent, AgentState};

/// Result groups, in the order the palette shows them (A13).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PaletteGroup {
    Workspaces,
    Sessions,
    Tabs,
    Commands,
}

impl PaletteGroup {
    pub fn header(self) -> &'static str {
        match self {
            PaletteGroup::Workspaces => "Workspaces",
            PaletteGroup::Sessions => "Sessions",
            PaletteGroup::Tabs => "Tabs",
            PaletteGroup::Commands => "Commands",
        }
    }
}

/// What running an item does; the shell interprets it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaletteAction {
    OpenWorkspace(String),
    OpenSession(String),
    Command(&'static str),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PaletteItem {
    pub group: PaletteGroup,
    pub title: String,
    /// "project / workspace" for sessions; empty otherwise.
    pub detail: String,
    /// Session rows start with the agent's icon (A13).
    pub agent: Option<Agent>,
    pub state: Option<AgentState>,
    /// Stored as `Mod+K`; the Kbd component renders it per OS.
    pub shortcut: Option<&'static str>,
    /// A command that cannot run now stays listed with its reason.
    pub disabled_reason: Option<String>,
    pub action: PaletteAction,
}

/// One visible row after filtering, with the matched character indices.
#[derive(Clone, Debug, PartialEq)]
pub struct PaletteMatch {
    pub item: usize,
    pub positions: Vec<usize>,
    score: i32,
}

/// The palette's state: items, query, visible matches and the active row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PaletteModel {
    items: Vec<PaletteItem>,
    query: String,
    matches: Vec<PaletteMatch>,
    active: usize,
}

impl PaletteModel {
    pub fn new(items: Vec<PaletteItem>) -> Self {
        let mut model = Self {
            items,
            ..Self::default()
        };
        model.set_query("");
        model
    }

    pub fn items(&self) -> &[PaletteItem] {
        &self.items
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn matches(&self) -> &[PaletteMatch] {
        &self.matches
    }

    pub fn active(&self) -> Option<&PaletteMatch> {
        self.matches.get(self.active)
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    /// Filters by a subsequence match, best first inside each group; groups
    /// keep their fixed order. The active row returns to the first enabled row.
    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_owned();
        let mut matches: Vec<PaletteMatch> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| {
                let (score, positions) = fuzzy(query, &item.title)?;
                Some(PaletteMatch {
                    item: i,
                    positions,
                    score,
                })
            })
            .collect();
        matches.sort_by_key(|m| (self.items[m.item].group, -m.score, m.item));
        self.matches = matches;
        self.active = self.first_enabled_from(0, 1).unwrap_or(0);
    }

    /// Up and Down move the active row, skipping disabled rows and wrapping.
    pub fn move_active(&mut self, delta: isize) {
        let len = self.matches.len();
        if len == 0 {
            return;
        }
        let start = (self.active as isize + delta).rem_euclid(len as isize) as usize;
        if let Some(i) = self.first_enabled_from(start, delta.signum()) {
            self.active = i;
        }
    }

    fn first_enabled_from(&self, start: usize, step: isize) -> Option<usize> {
        let len = self.matches.len();
        (0..len)
            .map(|k| (start as isize + k as isize * step).rem_euclid(len as isize) as usize)
            .find(|&i| self.items[self.matches[i].item].disabled_reason.is_none())
    }

    /// The action Enter runs, if the active row is enabled.
    pub fn confirm(&self) -> Option<&PaletteAction> {
        let item = &self.items[self.active()?.item];
        item.disabled_reason.is_none().then_some(&item.action)
    }

    /// The "No matches for" line when filtering hides everything.
    pub fn no_match_text(&self) -> Option<String> {
        (self.matches.is_empty() && !self.query.is_empty())
            .then(|| format!("No matches for \u{201c}{}\u{201d}", self.query))
    }
}

/// Case-insensitive subsequence match. Consecutive and word-start matches
/// score higher. Returns `None` when `query` is not a subsequence of `text`.
fn fuzzy(query: &str, text: &str) -> Option<(i32, Vec<usize>)> {
    let mut positions = Vec::new();
    let mut score = 0;
    let chars: Vec<char> = text.chars().collect();
    let mut from = 0;
    for q in query.chars().filter(|c| !c.is_whitespace()) {
        let q = q.to_lowercase().next()?;
        let i = (from..chars.len()).find(|&i| chars[i].to_lowercase().next() == Some(q))?;
        score += 1;
        if positions.last() == Some(&(i.wrapping_sub(1))) {
            score += 3;
        }
        if i == 0 || !chars[i - 1].is_alphanumeric() {
            score += 2;
        }
        positions.push(i);
        from = i + 1;
    }
    Some((score, positions))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(group: PaletteGroup, title: &str, disabled: bool) -> PaletteItem {
        PaletteItem {
            group,
            title: title.into(),
            detail: String::new(),
            agent: None,
            state: None,
            shortcut: None,
            disabled_reason: disabled.then(|| "No session is open".into()),
            action: PaletteAction::Command("test"),
        }
    }

    fn model() -> PaletteModel {
        PaletteModel::new(vec![
            item(PaletteGroup::Commands, "Toggle theme", false),
            item(PaletteGroup::Sessions, "Feed endpoints", false),
            item(PaletteGroup::Workspaces, "massorca-rust / main", false),
            item(PaletteGroup::Commands, "Close tab", true),
        ])
    }

    #[test]
    fn groups_run_workspaces_sessions_tabs_commands() {
        let m = model();
        let groups: Vec<_> = m
            .matches()
            .iter()
            .map(|x| m.items()[x.item].group)
            .collect();
        assert_eq!(
            groups,
            vec![
                PaletteGroup::Workspaces,
                PaletteGroup::Sessions,
                PaletteGroup::Commands,
                PaletteGroup::Commands
            ]
        );
    }

    #[test]
    fn filtering_keeps_subsequence_matches_with_positions() {
        let mut m = model();
        m.set_query("fe");
        assert_eq!(m.matches().len(), 1);
        assert_eq!(m.matches()[0].positions, vec![0, 1]);
        m.set_query("zzz");
        assert_eq!(
            m.no_match_text().as_deref(),
            Some("No matches for \u{201c}zzz\u{201d}")
        );
    }

    #[test]
    fn arrows_skip_disabled_rows_and_wrap() {
        let mut m = model();
        assert_eq!(m.active_index(), 0);
        m.move_active(1);
        m.move_active(1);
        assert_eq!(m.active_index(), 2);
        m.move_active(1);
        assert_eq!(
            m.active_index(),
            0,
            "disabled last row is skipped, wraps to top"
        );
        m.move_active(-1);
        assert_eq!(m.active_index(), 2);
    }

    #[test]
    fn enter_never_runs_a_disabled_command() {
        let mut m = model();
        m.set_query("close");
        assert!(m.confirm().is_none());
        m.set_query("toggle");
        assert_eq!(m.confirm(), Some(&PaletteAction::Command("test")));
    }
}
