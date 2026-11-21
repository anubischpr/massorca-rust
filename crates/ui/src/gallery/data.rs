//! Stories of the data components: Table, file tree and Picker. A stateful
//! component keeps its entity across frames under a key of its own.

use super::{Story, variant};
use crate::components::agent_icon::AgentKind;
use crate::components::agent_status::AgentState;
use crate::components::file_tree::{
    ContentMatch, FileEntry, FileTreeState, FileTreeStatus, FileTreeView, GitStatus, SearchScope,
};
use crate::components::picker::{PickerItem, PickerState, PickerStatus, PickerView};
use crate::components::table::{
    SortDirection, TableCell, TableColumn, TableDensity, TableRow, TableState, TableStatus,
    TableView,
};
use crate::icon::IconName;
use crate::tokens::SPACE_4;
use gpui::{AnyElement, App, Context, Entity, IntoElement, ParentElement, Styled, Window, div, px};

/// Height of a gallery frame around a live component. A layout size of the gallery, not a token.
const FRAME_HEIGHT: f32 = 180.0;

pub fn stories() -> Vec<Story> {
    vec![
        Story {
            name: "Table",
            spec: "Table.md",
            render: table,
        },
        Story {
            name: "File tree",
            spec: "FileTree.md",
            render: file_tree,
        },
        Story {
            name: "Picker",
            spec: "Picker.md",
            render: picker_story,
        },
    ]
}

fn table(window: &mut Window, cx: &mut App) -> AnyElement {
    let default = keyed_table(window, cx, "table-default", |cx| {
        TableView::new("table-default", "Checks", checks(), cx)
    });
    let sorted = keyed_table(window, cx, "table-sorted", |cx| {
        let state = checks().sorted_by(3, SortDirection::Descending);
        TableView::new("table-sorted", "Checks sorted by duration", state, cx)
    });
    let selected = keyed_table(window, cx, "table-selected", |cx| {
        let mut state = checks().selectable();
        state.select(&"lint".into());
        TableView::new("table-selected", "Checks with a selection", state, cx)
            .density(TableDensity::Comfortable)
    });
    let loading = keyed_table(window, cx, "table-loading", |cx| {
        let state = with_status(TableStatus::Loading);
        TableView::new("table-loading", "Checks loading", state, cx)
    });
    let empty = keyed_table(window, cx, "table-empty", |cx| {
        let state = with_status(TableStatus::Empty {
            title: "No check runs for this branch.".into(),
            explanation: Some("Checks appear after the first push.".into()),
            action: Some("Refresh".into()),
        });
        TableView::new("table-empty", "Checks empty", state, cx)
    });
    let error = keyed_table(window, cx, "table-error", |cx| {
        let state = with_status(TableStatus::Error);
        TableView::new("table-error", "Checks error", state, cx)
    });

    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant(
            "Default, with a long name that truncates",
            frame(default),
            cx,
        ))
        .child(variant("Sorted by duration, descending", frame(sorted), cx))
        .child(variant(
            "Selectable, comfortable rows, one row selected",
            frame(selected),
            cx,
        ))
        .child(variant("Loading: five skeleton rows", frame(loading), cx))
        .child(variant("Empty, with one action", frame(empty), cx))
        .child(variant("Error, with Retry", frame(error), cx))
        .into_any_element()
}

/// The table of one variant, kept across frames under its own key.
fn keyed_table(
    window: &mut Window,
    cx: &mut App,
    key: &'static str,
    build: fn(&mut Context<TableView>) -> TableView,
) -> Entity<TableView> {
    window.use_keyed_state(key, cx, |_, cx| build(cx))
}

fn frame(table: Entity<TableView>) -> impl IntoElement {
    div().h(px(FRAME_HEIGHT)).child(table)
}

fn with_status(status: TableStatus) -> TableState {
    let mut state = checks();
    state.set_status(status);
    state
}

/// Checks of one branch. The last name is long enough to truncate.
fn checks() -> TableState {
    TableState::new(
        vec![
            TableColumn::new("Name").sortable(),
            TableColumn::new("Agent"),
            TableColumn::new("Status"),
            TableColumn::new("Duration").numeric().sortable(),
            TableColumn::new("Log"),
        ],
        vec![
            check(
                "build",
                "Build",
                AgentKind::Claude,
                AgentState::Running,
                72.0,
                "1m 12s",
            ),
            check(
                "lint",
                "Lint",
                AgentKind::Codex,
                AgentState::Done,
                14.0,
                "14s",
            ),
            check(
                "endpoints",
                "Feed endpoints for the workspace status checks, including the slow ones",
                AgentKind::Gemini,
                AgentState::Waiting,
                301.0,
                "5m 1s",
            ),
            check(
                "sync",
                "Sync",
                AgentKind::Codex,
                AgentState::Unreachable,
                0.0,
                "0s",
            ),
        ],
    )
}

fn check(
    key: &str,
    name: &str,
    agent: AgentKind,
    state: AgentState,
    seconds: f64,
    duration: &str,
) -> TableRow {
    TableRow::new(
        key,
        vec![
            TableCell::Text(name.into()),
            TableCell::Agent(agent),
            TableCell::Status(state),
            TableCell::Numeric {
                value: seconds,
                text: duration.into(),
            },
            TableCell::Action {
                icon: IconName::ExternalLink,
                name: "Open log".into(),
                disabled: false,
            },
        ],
    )
}

/// Height of a gallery frame around a file tree, which has a search field above its rows.
const TREE_FRAME_HEIGHT: f32 = 300.0;

fn file_tree(window: &mut Window, cx: &mut App) -> AnyElement {
    let default = keyed_tree(window, cx, "tree-default", |cx| {
        let mut state = files();
        state.toggle(&"src".into());
        state.open_path(&"src/main.rs".into());
        FileTreeView::new("tree-default", "Files", state, cx)
    });
    let filtered = keyed_tree(window, cx, "tree-filtered", |cx| {
        let mut state = files();
        state.set_query("main");
        FileTreeView::new("tree-filtered", "Files", state, cx)
    });
    let content = keyed_tree(window, cx, "tree-content", |cx| {
        let mut state = files();
        state.set_scope(SearchScope::Content);
        state.set_content(vec![ContentMatch {
            path: "src/main.rs".into(),
            line: 2,
            text: "fn main() {}".into(),
        }]);
        FileTreeView::new("tree-content", "Files", state, cx)
    });
    let loading = keyed_tree(window, cx, "tree-loading", |cx| {
        let mut state = files();
        state.set_status(FileTreeStatus::Loading);
        FileTreeView::new("tree-loading", "Files", state, cx)
    });
    let empty = keyed_tree(window, cx, "tree-empty", |cx| {
        FileTreeView::new("tree-empty", "Files", FileTreeState::new(Vec::new()), cx)
    });
    let error = keyed_tree(window, cx, "tree-error", |cx| {
        let mut state = files();
        state.set_status(FileTreeStatus::Error);
        FileTreeView::new("tree-error", "Files", state, cx)
    });

    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant(
            "Folders, with the open file",
            tree_frame(default),
            cx,
        ))
        .child(variant(
            "Names filter: matches and the folders above them",
            tree_frame(filtered),
            cx,
        ))
        .child(variant(
            "Content scope: matching lines with their paths",
            tree_frame(content),
            cx,
        ))
        .child(variant(
            "Loading: tree-shaped placeholders",
            tree_frame(loading),
            cx,
        ))
        .child(variant("Empty folder", tree_frame(empty), cx))
        .child(variant(
            "Error, with the last rows and Retry",
            tree_frame(error),
            cx,
        ))
        .into_any_element()
}

/// The file tree of one variant, kept across frames under its own key.
fn keyed_tree(
    window: &mut Window,
    cx: &mut App,
    key: &'static str,
    build: fn(&mut Context<FileTreeView>) -> FileTreeView,
) -> Entity<FileTreeView> {
    window.use_keyed_state(key, cx, |_, cx| build(cx))
}

fn tree_frame(tree: Entity<FileTreeView>) -> impl IntoElement {
    div().h(px(TREE_FRAME_HEIGHT)).child(tree)
}

/// Files of a small project: two in `src/`, two in the root, one ignored.
fn files() -> FileTreeState {
    FileTreeState::new(vec![
        FileEntry::file("README.md"),
        FileEntry::file("src/main.rs").git(GitStatus::Modified),
        FileEntry::file("src/lib.rs"),
        FileEntry::file("notes.txt").ignored(),
    ])
}

/// Height of a gallery frame around a closed picker: its trigger only.
const PICKER_CLOSED_HEIGHT: f32 = 60.0;
/// Height of a gallery frame around an open picker, which needs room for its panel.
const PICKER_OPEN_HEIGHT: f32 = 340.0;

fn picker_story(window: &mut Window, cx: &mut App) -> AnyElement {
    let closed = keyed_picker(window, cx, "picker-closed", |cx| {
        PickerView::new(
            "picker-closed",
            "Base branch",
            "Choose a base branch",
            branches(),
            cx,
        )
    });
    let grouped = keyed_picker(window, cx, "picker-grouped", |cx| {
        let state = grouped_branches().selected("feature/map");
        PickerView::new(
            "picker-grouped",
            "Base branch",
            "Choose a base branch",
            state,
            cx,
        )
        .open_at_start()
    });
    let folder = keyed_picker(window, cx, "picker-folder", |cx| {
        PickerView::new(
            "picker-folder",
            "Parent folder",
            "Choose a folder",
            folders(),
            cx,
        )
        .open_at_start()
    });
    let creatable = keyed_picker(window, cx, "picker-creatable", |cx| {
        let mut state = branches().creatable("Create branch");
        state.set_query("feat/x");
        PickerView::new(
            "picker-creatable",
            "Base branch",
            "Choose a base branch",
            state,
            cx,
        )
        .open_at_start()
    });
    let loading = keyed_picker(window, cx, "picker-loading", |cx| {
        let mut state = branches();
        state.set_status(PickerStatus::Loading);
        PickerView::new(
            "picker-loading",
            "Base branch",
            "Choose a base branch",
            state,
            cx,
        )
        .open_at_start()
    });
    let error = keyed_picker(window, cx, "picker-error", |cx| {
        let mut state = branches();
        state.set_status(PickerStatus::Error);
        PickerView::new(
            "picker-error",
            "Base branch",
            "Choose a base branch",
            state,
            cx,
        )
        .open_at_start()
    });
    let empty = keyed_picker(window, cx, "picker-empty", |cx| {
        let state = PickerState::new("branches", Vec::new());
        PickerView::new(
            "picker-empty",
            "Base branch",
            "Choose a base branch",
            state,
            cx,
        )
        .open_at_start()
    });
    let disabled = keyed_picker(window, cx, "picker-disabled", |cx| {
        PickerView::new(
            "picker-disabled",
            "Base branch",
            "Choose a base branch",
            branches(),
            cx,
        )
        .disabled(true)
    });

    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant(
            "Closed, with its placeholder",
            picker_frame(closed, PICKER_CLOSED_HEIGHT),
            cx,
        ))
        .child(variant(
            "Open, with overline groups and the chosen value",
            picker_frame(grouped, PICKER_OPEN_HEIGHT),
            cx,
        ))
        .child(variant(
            "Folder mode, with crumbs and a parent row",
            picker_frame(folder, PICKER_OPEN_HEIGHT),
            cx,
        ))
        .child(variant(
            "Create row for the typed name",
            picker_frame(creatable, PICKER_OPEN_HEIGHT),
            cx,
        ))
        .child(variant(
            "Loading: three placeholders",
            picker_frame(loading, PICKER_OPEN_HEIGHT),
            cx,
        ))
        .child(variant(
            "Empty list, with the reason",
            picker_frame(empty, PICKER_OPEN_HEIGHT),
            cx,
        ))
        .child(variant(
            "Error, with Retry",
            picker_frame(error, PICKER_OPEN_HEIGHT),
            cx,
        ))
        .child(variant(
            "Disabled",
            picker_frame(disabled, PICKER_CLOSED_HEIGHT),
            cx,
        ))
        .into_any_element()
}

/// The picker of one variant, kept across frames under its own key.
fn keyed_picker(
    window: &mut Window,
    cx: &mut App,
    key: &'static str,
    build: fn(&mut Context<PickerView>) -> PickerView,
) -> Entity<PickerView> {
    window.use_keyed_state(key, cx, |_, cx| build(cx))
}

fn picker_frame(picker: Entity<PickerView>, height: f32) -> impl IntoElement {
    div().h(px(height)).child(picker)
}

fn branches() -> PickerState {
    PickerState::new(
        "branches",
        vec![
            PickerItem::new("main", "main"),
            PickerItem::new("feature/map", "feature/map"),
            PickerItem::new("release", "release"),
        ],
    )
}

fn grouped_branches() -> PickerState {
    PickerState::new(
        "branches",
        vec![
            PickerItem::new("main", "main").group("RECENT"),
            PickerItem::new("feature/map", "feature/map").group("RECENT"),
            PickerItem::new("release", "release").group("ALL"),
        ],
    )
}

fn folders() -> PickerState {
    PickerState::new(
        "folders",
        vec![
            PickerItem::new("/projects/massorca/crates", "crates").folder(),
            PickerItem::new("/projects/massorca/README.md", "README.md"),
        ],
    )
    .folder("/projects/massorca/app/src")
}
