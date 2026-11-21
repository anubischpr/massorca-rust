//! FileTree: the tree of workspace files (docs/design/components/FileTree.md).
//!
//! `FileTreeState` holds the rules without a window: the tree built from a flat listing,
//! the folders the user opened, the name filter and its matches, the cursor and the open
//! file. `FileTreeView` draws them: the search field with its Names and Content scope, and
//! the rows. Paths use `/` and carry no leading slash.

use crate::components::agent_icon::{AgentIcon, AgentIconSize, AgentKind};
use crate::components::list_row::{
    ListMove, RowDensity, RowLines, TREE_INDENT, list_move, move_index, row_height,
};
use crate::components::search_field::{SearchField, SearchFieldEvent};
use crate::components::segmented_control::{Segment, SegmentedControl};
use crate::components::tooltip::{Tooltip, tooltip_view};
use crate::icon::{Icon, IconName, IconSize};
use crate::style::Surface;
use crate::theme::{ActiveTheme, TextStyled, Theme};
use crate::tokens::{
    BORDER_WIDTH_THICK, Color, Palette, RADIUS_MD, RADIUS_XS, SIZE_ICON_SM, SIZE_ICON_XS,
    SIZE_ROW_TWO_LINE, SPACE_1, SPACE_2, TYPE_BODY, TYPE_CAPTION, TYPE_LABEL, TYPE_MONO_SM,
};
use gpui::{
    AnyElement, AppContext, Context, ElementId, Entity, EventEmitter, FocusHandle, HighlightStyle,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, ParentElement, Render, Role,
    ScrollStrategy, SharedString, StatefulInteractiveElement, Styled, StyledText, Subscription,
    UniformListScrollHandle, Window, div, px, uniform_list,
};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::Range;

/// Git status of a file. Renamed and conflicted files need a letter and a token that the
/// spec does not give yet (FileTree.md, open points), so they are not modelled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GitStatus {
    Modified,
    Added,
    Deleted,
    Untracked,
}

impl GitStatus {
    /// The letter at the right edge of the row.
    pub fn letter(self) -> char {
        match self {
            Self::Modified => 'M',
            Self::Added => 'A',
            Self::Deleted => 'D',
            Self::Untracked => '?',
        }
    }

    /// The word in the row's accessible name, such as "README.md, modified".
    pub fn word(self) -> &'static str {
        match self {
            Self::Modified => "modified",
            Self::Added => "added",
            Self::Deleted => "deleted",
            Self::Untracked => "untracked",
        }
    }
}

/// One entry of a workspace listing. Folders may be listed or implied by their files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    pub path: SharedString,
    pub is_dir: bool,
    pub git: Option<GitStatus>,
    pub ignored: bool,
}

impl FileEntry {
    pub fn file(path: impl Into<SharedString>) -> Self {
        Self {
            path: path.into(),
            is_dir: false,
            git: None,
            ignored: false,
        }
    }

    pub fn dir(path: impl Into<SharedString>) -> Self {
        Self {
            is_dir: true,
            ..Self::file(path)
        }
    }

    pub fn git(mut self, status: GitStatus) -> Self {
        self.git = Some(status);
        self
    }

    pub fn ignored(mut self) -> Self {
        self.ignored = true;
        self
    }
}

/// Which search the field runs: file names (filters the tree) or file contents.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchScope {
    #[default]
    Names,
    Content,
}

/// A line that matches the query in the Content scope. The caller's search supplies it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentMatch {
    pub path: SharedString,
    /// One-based line number.
    pub line: usize,
    pub text: SharedString,
}

/// What the tree body shows besides its rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FileTreeStatus {
    #[default]
    Ready,
    /// Tree-shaped skeleton rows in place of the rows.
    Loading,
    /// "Could not read the file list." with Retry. The last rows stay above it.
    Error,
}

/// A row of the visible tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisibleRow {
    pub path: SharedString,
    pub name: SharedString,
    pub depth: usize,
    pub is_dir: bool,
    /// `Some(open)` for a folder, `None` for a file. A folder shown open by the filter
    /// reports `Some(true)` even though the user did not open it.
    pub expanded: Option<bool>,
    pub git: Option<GitStatus>,
    pub ignored: bool,
    /// Byte ranges of the name that the filter matches.
    pub matches: Vec<Range<usize>>,
}

/// What Enter did at the cursor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeActivation {
    /// A file became the open file.
    Open(SharedString),
    /// A folder was opened or closed.
    Toggled { path: SharedString, expanded: bool },
}

/// One node of the tree. Node 0 is the root and has an empty path.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Node {
    path: SharedString,
    name: SharedString,
    parent: Option<usize>,
    depth: usize,
    is_dir: bool,
    git: Option<GitStatus>,
    ignored: bool,
    children: Vec<usize>,
}

impl Node {
    fn new(path: &str, name: &str, parent: Option<usize>, depth: usize, is_dir: bool) -> Self {
        Self {
            path: path.to_owned().into(),
            name: name.to_owned().into(),
            parent,
            depth,
            is_dir,
            git: None,
            ignored: false,
            children: Vec::new(),
        }
    }
}

fn text(value: &SharedString) -> &str {
    value.as_ref()
}

/// Folders first, then names in alphabetical order that ignores ASCII case.
fn child_order(left: &Node, right: &Node) -> Ordering {
    right
        .is_dir
        .cmp(&left.is_dir)
        .then_with(|| {
            text(&left.name)
                .to_ascii_lowercase()
                .cmp(&text(&right.name).to_ascii_lowercase())
        })
        .then_with(|| text(&left.name).cmp(text(&right.name)))
}

/// Builds the tree from a flat listing, creating the folders that only files imply.
fn build(entries: &[FileEntry]) -> Vec<Node> {
    let mut nodes = vec![Node::new("", "", None, 0, true)];
    let mut index: HashMap<String, usize> = HashMap::from([(String::new(), 0)]);
    for entry in entries {
        let segments: Vec<&str> = entry
            .path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect();
        let mut parent = 0;
        let mut path = String::new();
        for (depth, segment) in segments.iter().enumerate() {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(segment);
            let node = match index.get(&path) {
                Some(&existing) => existing,
                None => {
                    nodes.push(Node::new(&path, segment, Some(parent), depth, true));
                    let created = nodes.len() - 1;
                    index.insert(path.clone(), created);
                    nodes[parent].children.push(created);
                    created
                }
            };
            if depth + 1 == segments.len() {
                let target = &mut nodes[node];
                target.is_dir = entry.is_dir;
                target.git = entry.git;
                target.ignored = entry.ignored;
            }
            parent = node;
        }
    }
    let orders: Vec<Vec<usize>> = (0..nodes.len())
        .map(|node| {
            let mut children = nodes[node].children.clone();
            children.sort_by(|&a, &b| child_order(&nodes[a], &nodes[b]));
            children
        })
        .collect();
    for (node, children) in nodes.iter_mut().zip(orders) {
        node.children = children;
    }
    nodes
}

/// ASCII case-insensitive occurrences of `query` in `name`. ASCII folding keeps byte
/// offsets valid for the original name.
fn matches_query(name: &str, query: &str) -> Vec<Range<usize>> {
    if query.is_empty() {
        return Vec::new();
    }
    let haystack = name.to_ascii_lowercase();
    let needle = query.to_ascii_lowercase();
    haystack
        .match_indices(&needle)
        .map(|(start, found)| start..start + found.len())
        .collect()
}

/// The tree, the folders the user opened, the filter, the cursor and the open file.
#[derive(Clone, Debug, PartialEq)]
pub struct FileTreeState {
    nodes: Vec<Node>,
    expanded: HashSet<SharedString>,
    query: String,
    scope: SearchScope,
    content: Vec<ContentMatch>,
    cursor: usize,
    active: Option<SharedString>,
    status: FileTreeStatus,
}

impl FileTreeState {
    pub fn new(entries: Vec<FileEntry>) -> Self {
        Self {
            nodes: build(&entries),
            expanded: HashSet::new(),
            query: String::new(),
            scope: SearchScope::default(),
            content: Vec::new(),
            cursor: 0,
            active: None,
            status: FileTreeStatus::default(),
        }
    }

    /// Replaces the listing. Open folders and the open file stay as they are while they exist.
    pub fn set_entries(&mut self, entries: Vec<FileEntry>) {
        self.nodes = build(&entries);
        self.expanded.retain(|path| {
            self.nodes
                .iter()
                .any(|node| node.is_dir && &node.path == path)
        });
        if self
            .active
            .as_ref()
            .is_some_and(|path| !self.has_file(path))
        {
            self.active = None;
        }
        self.clamp_cursor();
    }

    pub fn status(&self) -> FileTreeStatus {
        self.status
    }

    pub fn set_status(&mut self, status: FileTreeStatus) {
        self.status = status;
    }

    pub fn scope(&self) -> SearchScope {
        self.scope
    }

    pub fn set_scope(&mut self, scope: SearchScope) {
        self.scope = scope;
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    /// Sets the query. A new query starts again at the top of the tree.
    pub fn set_query(&mut self, query: &str) {
        if self.query != query {
            self.query = query.to_owned();
            self.cursor = 0;
        }
    }

    pub fn content(&self) -> &[ContentMatch] {
        &self.content
    }

    pub fn set_content(&mut self, matches: Vec<ContentMatch>) {
        self.content = matches;
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The open file, which the Work panel shows.
    pub fn active(&self) -> Option<&SharedString> {
        self.active.as_ref()
    }

    /// The rows that are drawn, in order. Closed folders hide their children; a filter
    /// shows only the matches and their ancestors, with those folders open.
    pub fn visible(&self) -> Vec<VisibleRow> {
        let kept = (!self.query.is_empty()).then(|| self.kept_nodes());
        let mut rows = Vec::new();
        self.push_children(0, kept.as_ref(), &mut rows);
        rows
    }

    /// Moves the cursor by one row, a page or to an end. Moves stop at the ends.
    pub fn move_cursor(&mut self, movement: ListMove) {
        let count = self.visible().len();
        if let Some(index) = move_index(self.cursor, count, movement) {
            self.cursor = index;
        }
    }

    /// Opens the folder under the cursor or, when it is open, moves to its first child.
    /// Returns whether anything changed.
    pub fn right(&mut self) -> bool {
        let rows = self.visible();
        let Some(row) = rows.get(self.cursor) else {
            return false;
        };
        match row.expanded {
            Some(false) => {
                self.expanded.insert(row.path.clone());
                true
            }
            Some(true) => {
                if rows
                    .get(self.cursor + 1)
                    .is_some_and(|next| next.depth > row.depth)
                {
                    self.cursor += 1;
                }
                true
            }
            None => false,
        }
    }

    /// Closes the folder under the cursor when the user opened it, otherwise moves to the
    /// parent row.
    pub fn left(&mut self) {
        let rows = self.visible();
        let Some(row) = rows.get(self.cursor) else {
            return;
        };
        if self.expanded.contains(&row.path) {
            self.expanded.remove(&row.path);
        } else if let Some(parent) = rows[..self.cursor]
            .iter()
            .rposition(|candidate| candidate.depth < row.depth)
        {
            self.cursor = parent;
        }
    }

    /// Enter: a file becomes the open file; a folder opens or closes.
    pub fn activate(&mut self) -> Option<TreeActivation> {
        let row = self.visible().into_iter().nth(self.cursor)?;
        if row.is_dir {
            let expanded = self.toggle(&row.path);
            Some(TreeActivation::Toggled {
                path: row.path,
                expanded,
            })
        } else {
            self.active = Some(row.path.clone());
            Some(TreeActivation::Open(row.path))
        }
    }

    /// Opens or closes a folder and returns whether it is open now.
    pub fn toggle(&mut self, path: &SharedString) -> bool {
        if self.expanded.remove(path) {
            false
        } else {
            self.expanded.insert(path.clone());
            true
        }
    }

    /// A pointer click on a file makes it the open file and moves the cursor to it.
    pub fn open_path(&mut self, path: &SharedString) -> bool {
        if !self.has_file(path) {
            return false;
        }
        self.focus_path(path);
        self.active = Some(path.clone());
        true
    }

    /// Moves the cursor onto a visible row. Returns false when the row is hidden.
    pub fn focus_path(&mut self, path: &SharedString) -> bool {
        match self.visible().iter().position(|row| &row.path == path) {
            Some(index) => {
                self.cursor = index;
                true
            }
            None => false,
        }
    }

    /// Moves the cursor to the next visible name that starts with `letter`, wrapping at the
    /// end. Returns whether the cursor moved onto one.
    pub fn type_ahead(&mut self, letter: char) -> bool {
        let rows = self.visible();
        let letter = letter.to_ascii_lowercase();
        let count = rows.len();
        for step in 1..=count {
            let index = (self.cursor + step) % count;
            let starts_with = text(&rows[index].name)
                .chars()
                .next()
                .is_some_and(|first| first.to_ascii_lowercase() == letter);
            if starts_with {
                self.cursor = index;
                return true;
            }
        }
        false
    }

    fn has_file(&self, path: &SharedString) -> bool {
        self.nodes
            .iter()
            .any(|node| !node.is_dir && &node.path == path)
    }

    /// The matches of the query and their ancestors.
    fn kept_nodes(&self) -> HashSet<usize> {
        let mut kept = HashSet::new();
        for (index, node) in self.nodes.iter().enumerate().skip(1) {
            if matches_query(text(&node.name), &self.query).is_empty() {
                continue;
            }
            let mut current = Some(index);
            while let Some(node_index) = current {
                if !kept.insert(node_index) {
                    break;
                }
                current = self.nodes[node_index].parent;
            }
        }
        kept
    }

    fn push_children(
        &self,
        parent: usize,
        kept: Option<&HashSet<usize>>,
        rows: &mut Vec<VisibleRow>,
    ) {
        for &child in &self.nodes[parent].children {
            if kept.is_some_and(|kept| !kept.contains(&child)) {
                continue;
            }
            let node = &self.nodes[child];
            let expanded = node
                .is_dir
                .then(|| self.expanded.contains(&node.path) || kept.is_some());
            rows.push(VisibleRow {
                path: node.path.clone(),
                name: node.name.clone(),
                depth: node.depth,
                is_dir: node.is_dir,
                expanded,
                git: node.git,
                ignored: node.ignored,
                matches: if kept.is_some() {
                    matches_query(text(&node.name), &self.query)
                } else {
                    Vec::new()
                },
            });
            if expanded == Some(true) {
                self.push_children(child, kept, rows);
            }
        }
    }

    fn clamp_cursor(&mut self) {
        self.cursor = self.cursor.min(self.visible().len().saturating_sub(1));
    }
}

/// The icon before a file name: the agent mark for CLAUDE.md and AGENTS.md, a typed badge
/// for the file types with a colour, and a grey document for every other file (A46 to A48).
fn file_glyph(name: &str, p: &Palette) -> AnyElement {
    let agent = match name {
        "CLAUDE.md" => Some(AgentKind::Claude),
        "AGENTS.md" => Some(AgentKind::Codex),
        _ => None,
    };
    if let Some(kind) = agent {
        return AgentIcon::new(kind)
            .size(AgentIconSize::Small)
            .into_any_element();
    }
    let badge = name
        .rsplit_once('.')
        .and_then(|(_, extension)| typed_badge(&extension.to_ascii_lowercase()));
    match badge {
        Some(badge) => {
            let ink = colour(badge.ink);
            let tile = div()
                .flex()
                .items_center()
                .justify_center()
                .flex_none()
                .min_w(px(SIZE_ICON_SM))
                .h(px(SIZE_ICON_SM))
                .px(px(SPACE_1))
                .rounded(px(RADIUS_XS))
                .text_style(TYPE_CAPTION)
                .text_color(ink)
                .child(badge.label);
            match badge.fill {
                Some(fill) => tile.bg(colour(fill)),
                None => tile.border_1().border_color(ink),
            }
            .into_any_element()
        }
        None => div()
            .flex_none()
            .w(px(SIZE_ICON_XS))
            .h(px(SIZE_ICON_SM))
            .rounded(px(RADIUS_XS))
            .bg(p.text.tertiary)
            .into_any_element(),
    }
}

/// A typed file's badge: its short name, its colour and an optional fill behind it.
struct TypedBadge {
    label: &'static str,
    ink: u32,
    fill: Option<u32>,
}

/// Representative colours of the file types (A47). They are literal values, not tokens yet
/// (decision 0027).
fn typed_badge(extension: &str) -> Option<TypedBadge> {
    let (label, ink, fill) = match extension {
        "ts" => ("TS", 0x3178c6, None),
        "tsx" => ("TSX", 0x61dafb, None),
        "js" => ("JS", 0xf7df1e, None),
        "jsx" => ("JSX", 0x61dafb, None),
        "rs" => ("RS", 0xdea584, None),
        "py" => ("PY", 0xffd43b, Some(0x3776ab)),
        "css" => ("CSS", 0x663399, None),
        "json" => ("{}", 0xcbcb41, None),
        _ => return None,
    };
    Some(TypedBadge { label, ink, fill })
}

fn colour(rgb: u32) -> Color {
    Color::rgba8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 1.0)
}

/// The git letter's colour (FileTree.md, Anatomy).
fn git_ink(git: GitStatus, p: &Palette) -> Color {
    match git {
        GitStatus::Modified => p.status.warning_fg,
        GitStatus::Added => p.diff.add_fg,
        GitStatus::Deleted => p.diff.remove_fg,
        GitStatus::Untracked => p.status.info_fg,
    }
}

/// What a file tree reports to its owner. The tree has already moved its cursor, opened or
/// closed the folder, or made the file the open one by the time the owner hears of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileTreeEvent {
    /// A file was opened: Enter, a click, or a content match.
    Opened(SharedString),
    /// A folder was opened or closed.
    Toggled { path: SharedString, expanded: bool },
    /// The query changed. In the Content scope the owner runs the search and sets the results.
    Query(SharedString),
    /// The search scope changed between Names and Content.
    Scope(SearchScope),
    /// A row asked for its menu: a right click, Shift+F10 or the Menu key.
    ContextMenu(SharedString),
}

/// Fully transparent: no fill at rest, and no focus ring unless the row has the cursor.
const CLEAR: Color = Color::rgba8(0, 0, 0, 0.0);

/// The file tree: a search field with its Names and Content scope, above the rows. The
/// entity owns its [`FileTreeState`] and emits [`FileTreeEvent`]s.
pub struct FileTreeView {
    id: ElementId,
    label: SharedString,
    state: FileTreeState,
    density: RowDensity,
    surface: Surface,
    search: Entity<SearchField>,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    _search_events: Subscription,
}

impl EventEmitter<FileTreeEvent> for FileTreeView {}

impl fmt::Debug for FileTreeView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileTreeView")
            .field("id", &self.id)
            .field("scope", &self.state.scope())
            .field("cursor", &self.state.cursor())
            .finish_non_exhaustive()
    }
}

impl FileTreeView {
    /// `label` is the tree's accessible name, "Files" or "Team files".
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        state: FileTreeState,
        cx: &mut Context<Self>,
    ) -> Self {
        let query = state.query().to_owned();
        let search = cx.new(|cx| SearchField::with_query("Search files", &query, cx));
        let search_events = cx.subscribe(&search, |this, _, event: &SearchFieldEvent, cx| {
            this.on_search(event, cx);
        });
        Self {
            id: id.into(),
            label: label.into(),
            state,
            density: RowDensity::default(),
            surface: Surface::default(),
            search,
            focus: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            _search_events: search_events,
        }
    }

    pub fn density(mut self, density: RowDensity) -> Self {
        self.density = density;
        self
    }

    /// The surface the rows sit on: the canvas by default, the frame in the Project sidebar.
    pub fn surface(mut self, surface: Surface) -> Self {
        self.surface = surface;
        self
    }

    pub fn state(&self) -> &FileTreeState {
        &self.state
    }

    /// The keyboard focus of the tree, which is one Tab stop after the search field.
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    /// The search field, for an owner that sets its query.
    pub fn search_field(&self) -> Entity<SearchField> {
        self.search.clone()
    }

    /// Replaces the listing. Open folders and the open file stay while they still exist.
    pub fn set_entries(&mut self, entries: Vec<FileEntry>, cx: &mut Context<Self>) {
        self.state.set_entries(entries);
        cx.notify();
    }

    /// The matching lines of the Content scope, from the owner's search.
    pub fn set_content(&mut self, matches: Vec<ContentMatch>, cx: &mut Context<Self>) {
        self.state.set_content(matches);
        cx.notify();
    }

    fn on_search(&mut self, event: &SearchFieldEvent, cx: &mut Context<Self>) {
        if let SearchFieldEvent::Search(query) = event {
            self.state.set_query(query);
            cx.emit(FileTreeEvent::Query(query.clone()));
            cx.notify();
        }
    }

    fn set_scope(&mut self, index: usize, cx: &mut Context<Self>) {
        let scope = if index == 0 {
            SearchScope::Names
        } else {
            SearchScope::Content
        };
        if self.state.scope() != scope {
            self.state.set_scope(scope);
            cx.emit(FileTreeEvent::Scope(scope));
            cx.notify();
        }
    }

    fn move_cursor(&mut self, movement: ListMove, cx: &mut Context<Self>) {
        self.state.move_cursor(movement);
        self.reveal_cursor();
        cx.notify();
    }

    fn reveal_cursor(&self) {
        self.scroll
            .scroll_to_item(self.state.cursor(), ScrollStrategy::Nearest);
    }

    /// Enter: opens the file under the cursor or opens and closes its folder.
    fn activate(&mut self, cx: &mut Context<Self>) {
        match self.state.activate() {
            Some(TreeActivation::Open(path)) => cx.emit(FileTreeEvent::Opened(path)),
            Some(TreeActivation::Toggled { path, expanded }) => {
                cx.emit(FileTreeEvent::Toggled { path, expanded });
            }
            None => {}
        }
        cx.notify();
    }

    /// A click focuses the row and then acts on it, as Enter does.
    fn click(&mut self, path: &SharedString, cx: &mut Context<Self>) {
        self.state.focus_path(path);
        self.activate(cx);
    }

    fn open_match(&mut self, path: &SharedString, cx: &mut Context<Self>) {
        self.state.open_path(path);
        cx.emit(FileTreeEvent::Opened(path.clone()));
        cx.notify();
    }

    fn context_menu(&mut self, path: &SharedString, cx: &mut Context<Self>) {
        self.state.focus_path(path);
        cx.emit(FileTreeEvent::ContextMenu(path.clone()));
        cx.notify();
    }

    fn menu_at_cursor(&mut self, cx: &mut Context<Self>) {
        let cursor = self.state.cursor();
        if let Some(path) = self.state.visible().get(cursor).map(|row| row.path.clone()) {
            cx.emit(FileTreeEvent::ContextMenu(path));
        }
    }

    /// Tree keys work in the Names scope only; the Content list is driven by the pointer.
    fn on_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        if self.state.scope() == SearchScope::Content {
            return;
        }
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        let handled = if let Some(movement) = list_move(key) {
            self.move_cursor(movement, cx);
            true
        } else {
            match key {
                "right" | "left" => {
                    if key == "right" {
                        self.state.right();
                    } else {
                        self.state.left();
                    }
                    self.reveal_cursor();
                    cx.notify();
                    true
                }
                "enter" => {
                    self.activate(cx);
                    true
                }
                "menu" => {
                    self.menu_at_cursor(cx);
                    true
                }
                "f10" if modifiers.shift => {
                    self.menu_at_cursor(cx);
                    true
                }
                _ => self.type_ahead(key, modifiers, cx),
            }
        };
        if handled {
            cx.stop_propagation();
        }
    }

    /// A single letter jumps to the next name that starts with it.
    fn type_ahead(
        &mut self,
        key: &str,
        modifiers: gpui::Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let mut chars = key.chars();
        let (Some(letter), None) = (chars.next(), chars.next()) else {
            return false;
        };
        if !letter.is_alphanumeric() || modifiers.control || modifiers.alt || modifiers.platform {
            return false;
        }
        if self.state.type_ahead(letter) {
            self.reveal_cursor();
            cx.notify();
        }
        true
    }

    /// The rows of the current scope.
    fn list(&self, context: RowContext) -> AnyElement {
        match self.state.scope() {
            SearchScope::Names => {
                let rows = self.state.visible();
                uniform_list((self.id.clone(), "rows"), rows.len(), move |range, _, _| {
                    range
                        .map(|index| render_row(&context, index, &rows[index]))
                        .collect()
                })
                .track_scroll(&self.scroll)
                .flex_1()
                .min_h_0()
                .into_any_element()
            }
            SearchScope::Content => {
                let matches = self.state.content().to_vec();
                uniform_list(
                    (self.id.clone(), "matches"),
                    matches.len(),
                    move |range, _, _| {
                        range
                            .map(|index| render_match(&context, index, &matches[index]))
                            .collect()
                    },
                )
                .flex_1()
                .min_h_0()
                .into_any_element()
            }
        }
    }
}

/// What every row needs from its tree, captured once per frame.
#[derive(Clone)]
struct RowContext {
    entity: Entity<FileTreeView>,
    theme: Theme,
    surface: Surface,
    height: f32,
    focused: bool,
    cursor: usize,
    active: Option<SharedString>,
}

fn render_row(context: &RowContext, index: usize, row: &VisibleRow) -> AnyElement {
    let p = context.theme.palette;
    let fills = context.surface.fills(&context.theme);
    let active = context.active.as_ref() == Some(&row.path);
    let at_cursor = context.focused && context.cursor == index;
    let hover = if active { fills.selected } else { fills.hover };
    let ink = if row.ignored {
        p.text.tertiary
    } else {
        p.text.primary
    };
    let word = match (row.git, row.ignored) {
        (Some(git), _) => Some(git.word()),
        (None, true) => Some("ignored"),
        (None, false) => None,
    };
    let label = match word {
        Some(word) => format!("{}, {word}", row.name),
        None => row.name.to_string(),
    };
    let highlights = row.matches.iter().map(|range| {
        (
            range.clone(),
            HighlightStyle {
                background_color: Some(p.r#match.bg.into()),
                ..Default::default()
            },
        )
    });
    let (click_entity, click_path) = (context.entity.clone(), row.path.clone());
    let (menu_entity, menu_path) = (context.entity.clone(), row.path.clone());
    let mut line = div()
        .id(("file-row", index))
        .debug_selector(move || format!("file-row-{index}"))
        .role(Role::TreeItem)
        .aria_label(label)
        .aria_selected(active)
        .aria_level(row.depth + 1)
        .flex()
        .items_center()
        .gap(px(SPACE_1))
        .h(px(context.height))
        .pl(px(SPACE_2 + row.depth as f32 * TREE_INDENT))
        .pr(px(SPACE_2))
        .rounded(px(RADIUS_MD))
        .border_2()
        .border_color(if at_cursor { p.border.focus } else { CLEAR })
        .bg(if active { fills.selected } else { CLEAR })
        .hover(move |s| s.bg(hover))
        .active(move |s| s.bg(fills.pressed))
        .cursor_pointer()
        .tooltip(tooltip_view(Tooltip::new(row.path.clone())))
        .on_click(move |_, _, cx| {
            click_entity.update(cx, |tree, cx| tree.click(&click_path, cx));
        })
        .on_mouse_down(MouseButton::Right, move |_, _, cx| {
            menu_entity.update(cx, |tree, cx| tree.context_menu(&menu_path, cx));
        });
    if let Some(open) = row.expanded {
        line = line.aria_expanded(open);
    }
    line = line.child(match row.expanded {
        Some(open) => Icon::new(if open {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        })
        .size(IconSize::XSmall)
        .color(p.text.secondary)
        .into_any_element(),
        None => div().flex_none().w(px(SIZE_ICON_XS)).into_any_element(),
    });
    line = line.child(if row.is_dir {
        Icon::new(IconName::Folder)
            .size(IconSize::Small)
            .color(p.text.secondary)
            .into_any_element()
    } else {
        file_glyph(&row.name, &p)
    });
    line = line.child(
        div()
            .flex_1()
            .min_w_0()
            .truncate()
            .text_color(ink)
            .child(StyledText::new(row.name.clone()).with_highlights(highlights)),
    );
    if let Some(git) = row.git {
        line = line.child(
            div()
                .flex_none()
                .w(px(SIZE_ICON_SM))
                .flex()
                .justify_end()
                .text_style(TYPE_LABEL)
                .text_color(git_ink(git, &p))
                .child(git.letter().to_string()),
        );
    }
    if active {
        line = line.child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left_0()
                .w(px(BORDER_WIDTH_THICK))
                .bg(p.accent.fg),
        );
    }
    line.into_any_element()
}

/// A content match: its path in mono-sm above the matching line.
fn render_match(context: &RowContext, index: usize, found: &ContentMatch) -> AnyElement {
    let p = context.theme.palette;
    let hover = context.surface.fills(&context.theme).hover;
    let (entity, path) = (context.entity.clone(), found.path.clone());
    div()
        .id(("match-row", index))
        .debug_selector(move || format!("match-row-{index}"))
        .role(Role::ListItem)
        .aria_label(format!(
            "{}, line {}: {}",
            found.path, found.line, found.text
        ))
        .flex()
        .flex_col()
        .justify_center()
        .h(px(SIZE_ROW_TWO_LINE))
        .px(px(SPACE_2))
        .rounded(px(RADIUS_MD))
        .hover(move |s| s.bg(hover))
        .cursor_pointer()
        .on_click(move |_, _, cx| {
            entity.update(cx, |tree, cx| tree.open_match(&path, cx));
        })
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_style(TYPE_MONO_SM)
                .text_color(p.text.tertiary)
                .child(found.path.clone()),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_style(TYPE_BODY)
                .text_color(p.text.primary)
                .child(found.text.clone()),
        )
        .into_any_element()
}

impl Render for FileTreeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let entity = cx.entity();
        let context = RowContext {
            entity: entity.clone(),
            theme,
            surface: self.surface,
            height: row_height(RowLines::One, self.density),
            focused: self.focus.is_focused(window),
            cursor: self.state.cursor(),
            active: self.state.active().cloned(),
        };
        let scope = match self.state.scope() {
            SearchScope::Names => 0,
            SearchScope::Content => 1,
        };
        let scope_control = SegmentedControl::new(
            (self.id.clone(), "scope"),
            "Search in",
            vec![Segment::text("Names"), Segment::text("Content")],
        )
        .selected(scope)
        .on_change(move |index, _, cx| {
            entity.update(cx, |tree, cx| tree.set_scope(index, cx));
        });

        let body = self.list(context);

        div()
            .id(self.id.clone())
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .text_style(TYPE_BODY)
            .text_color(p.text.primary)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(SPACE_2))
                    .p(px(SPACE_2))
                    .child(self.search.clone())
                    .child(scope_control),
            )
            .child(
                div()
                    .id((self.id.clone(), "tree"))
                    .track_focus(&self.focus)
                    .tab_stop(true)
                    .role(Role::Tree)
                    .aria_label(self.label.clone())
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        this.on_key(event, cx);
                    }))
                    .child(body),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> FileTreeState {
        FileTreeState::new(vec![
            FileEntry::file("README.md"),
            FileEntry::file("src/main.rs").git(GitStatus::Modified),
            FileEntry::file("src/lib/util.rs"),
            FileEntry::file("target/debug/app").ignored(),
            FileEntry::dir("docs"),
            FileEntry::file("Cargo.toml").git(GitStatus::Added),
        ])
    }

    fn names(state: &FileTreeState) -> Vec<String> {
        state
            .visible()
            .iter()
            .map(|row| row.name.to_string())
            .collect()
    }

    #[test]
    fn folders_come_first_and_names_sort_ignoring_case() {
        assert_eq!(
            names(&project()),
            ["docs", "src", "target", "Cargo.toml", "README.md"]
        );
    }

    #[test]
    fn a_folder_implied_by_its_files_is_listed_and_its_children_show_only_when_open() {
        let mut state = project();
        assert!(state.toggle(&"src".into()));
        assert_eq!(
            names(&state),
            [
                "docs",
                "src",
                "lib",
                "main.rs",
                "target",
                "Cargo.toml",
                "README.md"
            ]
        );
        assert!(!state.toggle(&"src".into()));
        assert_eq!(names(&state).len(), 5);
    }

    #[test]
    fn right_opens_a_folder_then_enters_it_and_left_leaves_it_for_the_parent() {
        let mut state = project();
        state.move_cursor(ListMove::Next);
        assert!(state.right(), "src opens");
        assert_eq!(state.cursor(), 1);
        assert!(state.right(), "an open folder moves to its first child");
        assert_eq!(state.cursor(), 2, "lib");
        state.left();
        assert_eq!(state.cursor(), 1, "lib is a child of src");
        state.left();
        assert_eq!(names(&state).len(), 5, "left on an opened folder closes it");
    }

    #[test]
    fn enter_opens_a_file_and_toggles_a_folder() {
        let mut state = project();
        state.move_cursor(ListMove::Last);
        assert_eq!(
            state.activate(),
            Some(TreeActivation::Open("README.md".into()))
        );
        assert_eq!(state.active(), Some(&SharedString::from("README.md")));
        state.move_cursor(ListMove::First);
        assert_eq!(
            state.activate(),
            Some(TreeActivation::Toggled {
                path: "docs".into(),
                expanded: true,
            })
        );
    }

    #[test]
    fn a_name_filter_keeps_the_ancestors_open_and_marks_the_matched_characters() {
        let mut state = project();
        state.set_query("util");
        let rows = state.visible();
        assert_eq!(
            rows.iter()
                .map(|row| row.name.to_string())
                .collect::<Vec<_>>(),
            ["src", "lib", "util.rs"]
        );
        assert_eq!(
            rows[0].expanded,
            Some(true),
            "the filter opens the ancestors"
        );
        assert_eq!(rows[2].matches, vec![0..4]);
    }

    #[test]
    fn the_filter_ignores_ascii_case() {
        let mut state = project();
        state.set_query("readme");
        assert_eq!(names(&state), ["README.md"]);
        assert_eq!(state.visible()[0].matches, vec![0..6]);
    }

    #[test]
    fn typing_jumps_to_the_next_name_that_starts_with_the_letter_and_wraps() {
        let mut state = project();
        assert!(state.type_ahead('c'));
        assert_eq!(state.cursor(), 3, "Cargo.toml");
        assert!(state.type_ahead('r'));
        assert_eq!(state.cursor(), 4, "README.md");
        assert!(state.type_ahead('d'));
        assert_eq!(state.cursor(), 0, "wraps to docs");
        assert!(!state.type_ahead('z'));
    }

    #[test]
    fn git_letters_and_words_come_from_the_status() {
        assert_eq!(GitStatus::Untracked.letter(), '?');
        assert_eq!(GitStatus::Modified.letter(), 'M');
        assert_eq!(GitStatus::Added.word(), "added");
        let mut state = project();
        state.toggle(&"src".into());
        let main = state
            .visible()
            .into_iter()
            .find(|row| row.name == "main.rs");
        assert_eq!(main.and_then(|row| row.git), Some(GitStatus::Modified));
    }

    #[test]
    fn a_new_listing_keeps_open_folders_and_drops_a_file_that_is_gone() {
        let mut state = project();
        state.toggle(&"src".into());
        state.open_path(&"README.md".into());
        state.set_entries(vec![FileEntry::file("src/main.rs")]);
        assert_eq!(state.active(), None);
        assert_eq!(names(&state), ["src", "main.rs"]);
    }

    #[test]
    fn the_cursor_stops_at_both_ends() {
        let mut state = project();
        state.move_cursor(ListMove::Previous);
        assert_eq!(state.cursor(), 0);
        state.move_cursor(ListMove::Last);
        assert_eq!(state.cursor(), 4);
        state.move_cursor(ListMove::Next);
        assert_eq!(state.cursor(), 4);
    }
}
