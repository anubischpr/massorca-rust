//! FileTree: the tree of workspace files (docs/design/components/FileTree.md).
//!
//! `FileTreeState` holds the rules without a window: the tree built from a flat listing,
//! the folders the user opened, the name filter and its matches, the cursor and the open
//! file. Paths use `/` and carry no leading slash.

use crate::components::list_row::{ListMove, move_index};
use gpui::SharedString;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
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
