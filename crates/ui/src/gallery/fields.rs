//! Gallery stories for the fields group.

use gpui::{AnyElement, App, Focusable, IntoElement, ParentElement, Styled, Window, div, px};

use super::{Story, variant};
use crate::components::empty_state::EmptyState;
use crate::components::search_field::SearchField;
use crate::components::text_field::TextField;
use crate::icon::IconName;
use crate::tokens::{SPACE_2, SPACE_4};

pub fn stories() -> Vec<Story> {
    vec![
        Story {
            name: "Text field",
            spec: "TextField.md",
            render: text_field_story,
        },
        Story {
            name: "Search field",
            spec: "SearchField.md",
            render: search_field_story,
        },
    ]
}

fn search_field_story(window: &mut Window, cx: &mut App) -> AnyElement {
    let idle = window.use_keyed_state("search-field/idle", cx, |_, cx| {
        SearchField::new("Search files", cx)
    });
    let results = window.use_keyed_state("search-field/results", cx, |_, cx| {
        SearchField::with_query("Search files", "oppm", cx).result_count(Some(12))
    });
    let no_matches = window.use_keyed_state("search-field/no-matches", cx, |_, cx| {
        SearchField::with_query("Search files", "oppm", cx).result_count(Some(0))
    });

    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant("Idle", idle, cx))
        .child(variant("Results", results, cx))
        .child(variant(
            "No matches: the count, and the filtered empty state with Clear search",
            div()
                .flex()
                .flex_col()
                .gap(px(SPACE_2))
                .child(no_matches)
                .child(EmptyState::filtered("search-no-matches", "oppm", |_, _| {})),
            cx,
        ))
        .into_any_element()
}

fn text_field_story(window: &mut Window, cx: &mut App) -> AnyElement {
    let rest = window.use_keyed_state("text-field/rest", cx, |_, cx| {
        TextField::new(cx).label("Name").placeholder("e.g. oppm")
    });
    let value = window.use_keyed_state("text-field/value", cx, |_, cx| {
        TextField::new(cx).label("Base branch").with_text("main")
    });
    let helper = window.use_keyed_state("text-field/helper", cx, |_, cx| {
        TextField::new(cx)
            .label("Host address")
            .helper("Letters, numbers and dashes only.")
            .with_text("oppm-01")
    });
    let error = window.use_keyed_state("text-field/error", cx, |_, cx| {
        TextField::new(cx)
            .label("Name")
            .error("Name is already in use. Choose another.")
            .with_text("oppm")
    });
    let focused = window.use_keyed_state("text-field/focused", cx, |window, cx| {
        let field = TextField::new(cx).label("Name").with_text("oppm");
        window.focus(&field.focus_handle(cx), cx);
        field
    });
    let leading = window.use_keyed_state("text-field/leading-icon", cx, |_, cx| {
        TextField::new(cx)
            .label("Workspace folder")
            .leading_icon(IconName::Folder)
            .with_text("~/MassOrca")
    });
    let disabled = window.use_keyed_state("text-field/disabled", cx, |_, cx| {
        TextField::new(cx)
            .label("Name")
            .disabled(true)
            .with_text("oppm")
    });
    let read_only = window.use_keyed_state("text-field/read-only", cx, |_, cx| {
        TextField::new(cx)
            .label("Host")
            .read_only(true)
            .helper("Set by the host.")
            .with_text("This PC")
    });
    let mono = window.use_keyed_state("text-field/mono", cx, |_, cx| {
        TextField::new(cx)
            .label("Path")
            .mono()
            .with_text("~/MassOrca/Conversations")
    });
    let comfortable = window.use_keyed_state("text-field/comfortable", cx, |_, cx| {
        TextField::new(cx)
            .label("Host address")
            .comfortable()
            .placeholder("e.g. oppm")
    });
    let clearable = window.use_keyed_state("text-field/clearable", cx, |_, cx| {
        TextField::new(cx)
            .label("Search")
            .clearable()
            .with_text("oppm")
    });
    let masked = window.use_keyed_state("text-field/masked", cx, |_, cx| {
        TextField::new(cx)
            .label("Value")
            .masked()
            .mono()
            .with_text("placeholder-value")
    });

    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant("Rest", rest, cx))
        .child(variant("Value", value, cx))
        .child(variant("Helper", helper, cx))
        .child(variant("Error", error, cx))
        .child(variant("Focused", focused, cx))
        .child(variant("Leading icon", leading, cx))
        .child(variant("Disabled", disabled, cx))
        .child(variant("Read-only", read_only, cx))
        .child(variant("Mono", mono, cx))
        .child(variant("Comfortable", comfortable, cx))
        .child(variant("Clearable", clearable, cx))
        .child(variant("Masked", masked, cx))
        .into_any_element()
}
