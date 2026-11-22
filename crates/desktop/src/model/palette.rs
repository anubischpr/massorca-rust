//! Command palette state: grouping, filtering, the active row (D-CP-01, A13).

use super::navigation::{MarketplacePage, Page};
use super::regions::Region;
use super::shell::{Agent, AgentState, SessionRow, ShellSnapshot};
use std::ops::Range;

/// Result groups, in the order the palette shows them (A13).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PaletteGroup {
    /// The last commands and workspaces run, shown while the query is empty.
    Recent,
    Workspaces,
    Sessions,
    Tabs,
    Commands,
}

impl PaletteGroup {
    pub fn header(self) -> &'static str {
        match self {
            PaletteGroup::Recent => "Recent",
            PaletteGroup::Workspaces => "Workspaces",
            PaletteGroup::Sessions => "Sessions",
            PaletteGroup::Tabs => "Tabs",
            PaletteGroup::Commands => "Commands",
        }
    }
}

/// What the palette searches: the commands and rows (Mod+K), or files (Mod+P, "Go to file").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PaletteMode {
    #[default]
    Commands,
    GoToFile,
}

/// What running an item does; the shell interprets it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaletteAction {
    OpenWorkspace(String),
    OpenSession(String),
    Command(Command),
}

/// A command the palette lists, with the same effect as its control on the shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// The theme is app-wide, so the shell flips it rather than the shell model.
    ToggleTheme,
    /// Opens a full page.
    Open(Page),
    /// Shows or hides a region.
    Toggle(Region),
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
    mode: PaletteMode,
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

    /// The mode the palette opened in; it sets the label and the shortcut the view shows.
    pub fn with_mode(mut self, mode: PaletteMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn mode(&self) -> PaletteMode {
        self.mode
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
        let browsing = self.browsing();
        let mut matches: Vec<PaletteMatch> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| (item.group == PaletteGroup::Recent) == browsing)
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

    /// Makes a row active, as a click does; a disabled row cannot become active.
    pub fn select(&mut self, index: usize) {
        if let Some(found) = self.matches.get(index)
            && self.items[found.item].disabled_reason.is_none()
        {
            self.active = index;
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

    /// The "No matches for" line when a query hides everything.
    pub fn no_match_text(&self) -> Option<String> {
        (self.matches.is_empty() && !self.browsing())
            .then(|| format!("No matches for \u{201c}{}\u{201d}", self.query))
    }

    /// "Type to search" when the empty palette has no recent rows to show (CommandPalette spec).
    pub fn empty_text(&self) -> Option<String> {
        (self.matches.is_empty() && self.browsing()).then(|| "Type to search".to_owned())
    }

    /// With no query the palette shows its Recent rows; a query shows the other groups instead.
    fn browsing(&self) -> bool {
        self.query.trim().is_empty()
    }
}

/// The rows the palette lists for a snapshot: workspaces, then sessions, then the commands.
pub fn palette_items(snapshot: &ShellSnapshot) -> Vec<PaletteItem> {
    let mut items = Vec::new();
    for project in &snapshot.projects {
        for host in &project.hosts {
            for workspace in &host.workspaces {
                items.push(PaletteItem {
                    group: PaletteGroup::Workspaces,
                    title: workspace.name.clone(),
                    detail: project.name.clone(),
                    agent: None,
                    state: None,
                    shortcut: None,
                    disabled_reason: None,
                    action: PaletteAction::OpenWorkspace(workspace.id.clone()),
                });
                let location = format!("{} / {}", project.name, workspace.name);
                for session in &workspace.sessions {
                    push_session(&mut items, session, &location);
                }
            }
        }
    }
    items.extend(command_items());
    items
}

fn push_session(items: &mut Vec<PaletteItem>, session: &SessionRow, location: &str) {
    items.push(PaletteItem {
        group: PaletteGroup::Sessions,
        title: session.title.clone(),
        detail: location.to_owned(),
        agent: Some(session.agent),
        state: Some(session.state),
        shortcut: None,
        disabled_reason: None,
        action: PaletteAction::OpenSession(session.id.clone()),
    });
    for child in &session.children {
        push_session(items, child, location);
    }
}

fn command_items() -> Vec<PaletteItem> {
    let commands: [(&str, Option<&'static str>, Command); 9] = [
        ("Toggle theme", None, Command::ToggleTheme),
        (
            "Open settings",
            Some("Mod+,"),
            Command::Open(Page::Settings),
        ),
        ("Toggle sidebar", None, Command::Toggle(Region::Sidebar)),
        (
            "Toggle session summary",
            None,
            Command::Toggle(Region::SessionSummary),
        ),
        (
            "Toggle work panel",
            None,
            Command::Toggle(Region::WorkPanel),
        ),
        (
            "Toggle project sidebar",
            None,
            Command::Toggle(Region::ProjectSidebar),
        ),
        ("Open tasks", None, Command::Open(Page::Tasks)),
        ("Open calendar", None, Command::Open(Page::Calendar)),
        (
            "Open marketplace",
            None,
            Command::Open(Page::Marketplace(MarketplacePage::Discover)),
        ),
    ];
    commands
        .into_iter()
        .map(|(title, shortcut, command)| PaletteItem {
            group: PaletteGroup::Commands,
            title: title.to_owned(),
            detail: String::new(),
            agent: None,
            state: None,
            shortcut,
            disabled_reason: None,
            action: PaletteAction::Command(command),
        })
        .collect()
}

/// The byte ranges of matched characters, for drawing the match highlight (match.bg).
pub fn highlight_ranges(text: &str, positions: &[usize]) -> Vec<Range<usize>> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    positions
        .iter()
        .filter_map(|&position| {
            chars
                .get(position)
                .map(|&(start, c)| start..start + c.len_utf8())
        })
        .collect()
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
            action: PaletteAction::Command(Command::ToggleTheme),
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
    fn the_sample_snapshot_lists_workspaces_sessions_then_commands() {
        use crate::data::{DataSource, SampleDataSource};
        let items = palette_items(&SampleDataSource.snapshot());
        let count = |group| items.iter().filter(|item| item.group == group).count();
        assert_eq!(count(PaletteGroup::Workspaces), 4);
        assert_eq!(count(PaletteGroup::Sessions), 6);
        assert_eq!(count(PaletteGroup::Commands), 9);
        assert_eq!(
            items.first().map(|item| item.group),
            Some(PaletteGroup::Workspaces)
        );
        assert_eq!(
            items.last().map(|item| item.group),
            Some(PaletteGroup::Commands)
        );
    }

    #[test]
    fn a_session_row_carries_its_agent_and_where_it_lives() {
        use crate::data::{DataSource, SampleDataSource};
        let items = palette_items(&SampleDataSource.snapshot());
        let session = items
            .iter()
            .find(|item| item.title == "Implementation of master catalogs")
            .expect("the lead session is listed");
        assert_eq!(session.agent, Some(Agent::Claude));
        assert_eq!(session.detail, "massorca-rust / desktop-shell");
        assert_eq!(session.action, PaletteAction::OpenSession("s1".to_owned()));
    }

    #[test]
    fn match_highlights_map_character_positions_to_bytes() {
        // "Café" has a two-byte "é" at character 3 and byte 3.
        assert_eq!(highlight_ranges("Café", &[0, 3]), vec![0..1, 3..5]);
        assert!(highlight_ranges("abc", &[7]).is_empty());
    }

    #[test]
    fn a_click_activates_an_enabled_row_only() {
        let mut m = model();
        m.set_query("t");
        m.select(2);
        assert_eq!(m.active_index(), 2);
        m.select(3);
        assert_eq!(m.active_index(), 2, "the disabled row cannot become active");
        m.select(99);
        assert_eq!(m.active_index(), 2);
    }

    #[test]
    fn groups_run_workspaces_sessions_tabs_commands() {
        let mut m = model();
        m.set_query("t");
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
        m.set_query("t");
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
        assert_eq!(
            m.confirm(),
            Some(&PaletteAction::Command(Command::ToggleTheme))
        );
    }

    fn recent(title: &str) -> PaletteItem {
        PaletteItem {
            group: PaletteGroup::Recent,
            ..item(PaletteGroup::Commands, title, false)
        }
    }

    fn groups_of(m: &PaletteModel) -> Vec<PaletteGroup> {
        m.matches()
            .iter()
            .map(|found| m.items()[found.item].group)
            .collect()
    }

    #[test]
    fn recent_rows_show_only_while_the_query_is_empty() {
        let mut m = PaletteModel::new(vec![
            recent("Toggle theme"),
            item(PaletteGroup::Sessions, "Feed endpoints", false),
        ]);
        assert_eq!(groups_of(&m), vec![PaletteGroup::Recent]);
        m.set_query("feed");
        assert_eq!(groups_of(&m), vec![PaletteGroup::Sessions]);
        m.set_query("  ");
        assert_eq!(groups_of(&m), vec![PaletteGroup::Recent]);
    }

    #[test]
    fn an_empty_query_with_nothing_recent_asks_for_a_search() {
        let mut m = PaletteModel::new(vec![item(PaletteGroup::Commands, "Close tab", false)]);
        assert!(m.matches().is_empty());
        assert_eq!(m.empty_text().as_deref(), Some("Type to search"));
        assert_eq!(m.no_match_text(), None);
        m.set_query("zzz");
        assert_eq!(m.empty_text(), None);
        assert!(m.no_match_text().is_some());
    }

    #[test]
    fn go_to_file_has_no_rows_until_files_are_listed() {
        let mut m = PaletteModel::new(Vec::new()).with_mode(PaletteMode::GoToFile);
        assert_eq!(m.mode(), PaletteMode::GoToFile);
        assert_eq!(m.empty_text().as_deref(), Some("Type to search"));
        m.set_query("main.rs");
        assert_eq!(
            m.no_match_text().as_deref(),
            Some("No matches for \u{201c}main.rs\u{201d}")
        );
    }
}
