//! Gallery stories for the layout group: the splitter, the scroll area and the sheet.

use super::{Story, variant};
use crate::components::button::{Button, ButtonVariant};
use crate::components::scroll_area::{ROW_STEP_PX, ScrollArea, ScrollAxis, ScrollPreview};
use crate::components::sheet::{Sheet, SheetVariant};
use crate::components::splitter::{
    Extent, RegionSide, Splitter, SplitterLimits, SplitterModel, SplitterPreview,
};
use crate::icon::{Icon, IconName, IconSize};
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    Color, LAYOUT_SIDEBAR_DEFAULT, LAYOUT_SIDEBAR_MAX, LAYOUT_SIDEBAR_MIN,
    LAYOUT_WORKPANEL_DEFAULT, LAYOUT_WORKPANEL_MIN, Palette, SIZE_TOUCH_LARGE, SPACE_2, SPACE_3,
    SPACE_4, TYPE_A_BODY,
};
use gpui::{
    AnyElement, App, Div, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};

/// The fixed size of one scroll area in the stories.
const SCROLL_BOX_WIDTH: f32 = 240.0;
const SCROLL_BOX_HEIGHT: f32 = 160.0;
/// Width of one cell in the horizontal story.
const CELL_WIDTH: f32 = 96.0;

/// Height of one lane in the splitter stories.
const LANE_HEIGHT: f32 = 48.0;

pub fn stories() -> Vec<Story> {
    vec![
        Story {
            name: "Splitter",
            spec: "Splitter.md",
            render: splitter_story,
        },
        Story {
            name: "ScrollArea",
            spec: "ScrollArea.md",
            render: scroll_story,
        },
        Story {
            name: "Sheet",
            spec: "Sheet.md",
            render: sheet_story,
        },
    ]
}

/// The Sidebar's width rules from the layout tokens, on the left of its splitter.
fn sidebar(extent: Extent) -> SplitterModel {
    SplitterModel::new(
        RegionSide::Left,
        SplitterLimits {
            default: LAYOUT_SIDEBAR_DEFAULT,
            min: LAYOUT_SIDEBAR_MIN,
            max: LAYOUT_SIDEBAR_MAX,
            room: f32::INFINITY,
        },
        extent,
    )
}

/// The Work panel's width rules, on the right of its splitter; the room is a fixed example.
fn work_panel(extent: Extent) -> SplitterModel {
    SplitterModel::new(
        RegionSide::Right,
        SplitterLimits {
            default: LAYOUT_WORKPANEL_DEFAULT,
            min: LAYOUT_WORKPANEL_MIN,
            max: 720.0,
            room: 720.0,
        },
        extent,
    )
}

/// One lane: the region the splitter resizes, the splitter and the canvas beside it.
fn lane(cx: &App, id: usize, model: SplitterModel, preview: Option<SplitterPreview>) -> AnyElement {
    let p = cx.theme().palette;
    let mut splitter = Splitter::new(("layout-story-splitter", id), "Sidebar", model);
    if let Some(preview) = preview {
        splitter = splitter.preview(preview);
    }
    let region = div()
        .w(px(model.width()))
        .h_full()
        .flex_none()
        .bg(p.surface.raised);
    let canvas = div().flex_1().h_full().bg(p.surface.canvas);
    let row = div().flex().w_full().h(px(LANE_HEIGHT)).bg(p.surface.frame);
    match model.side {
        RegionSide::Left => row.child(region).child(splitter).child(canvas),
        RegionSide::Right => row.child(canvas).child(splitter).child(region),
    }
    .into_any_element()
}

fn splitter_story(window: &mut Window, cx: &mut App) -> AnyElement {
    let live = window.use_keyed_state("layout-story/splitter", cx, |_, _| {
        Extent::Open(LAYOUT_SIDEBAR_DEFAULT)
    });
    let live_extent = *live.read(cx);
    let live_model = sidebar(live_extent);
    let on_live = {
        let live = live.clone();
        move |next: Extent, _: &mut Window, cx: &mut App| {
            live.update(cx, |extent, cx| {
                *extent = next;
                cx.notify();
            });
        }
    };
    let live_lane = {
        let p = cx.theme().palette;
        let splitter = Splitter::new("layout-story-live", "Sidebar", live_model).on_change(on_live);
        div()
            .flex()
            .w_full()
            .h(px(LANE_HEIGHT))
            .bg(p.surface.frame)
            .child(
                div()
                    .w(px(live_model.width()))
                    .h_full()
                    .flex_none()
                    .bg(p.surface.raised),
            )
            .child(splitter)
            .child(div().flex_1().h_full().bg(p.surface.canvas))
    };

    let states = [
        ("Rest", sidebar(Extent::Open(LAYOUT_SIDEBAR_DEFAULT)), None),
        (
            "Hover over the hit area",
            sidebar(Extent::Open(LAYOUT_SIDEBAR_DEFAULT)),
            Some(SplitterPreview::Hover),
        ),
        (
            "Pressed while dragging",
            sidebar(Extent::Open(LAYOUT_SIDEBAR_DEFAULT)),
            Some(SplitterPreview::Pressed),
        ),
        (
            "Keyboard focus",
            sidebar(Extent::Open(LAYOUT_SIDEBAR_DEFAULT)),
            Some(SplitterPreview::Focus),
        ),
        (
            "At the minimum",
            sidebar(Extent::Open(LAYOUT_SIDEBAR_MIN)),
            None,
        ),
        (
            "At the maximum",
            sidebar(Extent::Open(LAYOUT_SIDEBAR_MAX)),
            None,
        ),
        ("Collapsed", sidebar(Extent::Collapsed), None),
        (
            "Work panel on the right, at its minimum",
            work_panel(Extent::Open(LAYOUT_WORKPANEL_MIN)),
            None,
        ),
    ];
    let mut column = div().flex().flex_col().gap(px(SPACE_3));
    for (index, (label, model, preview)) in states.into_iter().enumerate() {
        column = column.child(variant(label, lane(cx, index, model, preview), cx));
    }
    column = column.child(variant(
        "Live: drag, double-click or press Left and Right",
        live_lane,
        cx,
    ));
    column.into_any_element()
}

/// Rows in the long sheet, enough to scroll under its header.
const LONG_ROWS: usize = 20;

/// One 48 dp row of a sheet's list: an icon, then its label.
fn sheet_row(
    icon: IconName,
    label: impl Into<SharedString>,
    icon_colour: Color,
    label_colour: Color,
) -> Div {
    div()
        .h(px(SIZE_TOUCH_LARGE))
        .flex()
        .items_center()
        .gap(px(SPACE_3))
        .px(px(SPACE_4))
        .text_style(TYPE_A_BODY)
        .text_color(label_colour)
        .child(Icon::new(icon).size(IconSize::Medium).color(icon_colour))
        .child(label.into())
}

/// An option row: the choice, with a check mark on the chosen one.
fn option_row(label: &'static str, selected: bool, p: Palette) -> Div {
    let row =
        sheet_row(IconName::Bot, label, p.text.secondary, p.text.primary).child(div().flex_1());
    if selected {
        row.child(
            Icon::new(IconName::Check)
                .size(IconSize::Medium)
                .color(p.accent.fg),
        )
    } else {
        row
    }
}

fn sheet_story(_: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.theme().palette;
    let actions = Sheet::new("story-sheet-action")
        .title("Workspace actions")
        .open(true)
        .inline()
        .child(sheet_row(
            IconName::Pencil,
            "Rename",
            p.text.secondary,
            p.text.primary,
        ))
        .child(sheet_row(
            IconName::Copy,
            "Duplicate",
            p.text.secondary,
            p.text.primary,
        ))
        .child(sheet_row(
            IconName::Trash2,
            "Delete workspace",
            p.status.danger_fg,
            p.status.danger_fg,
        ));
    let options = Sheet::new("story-sheet-option")
        .variant(SheetVariant::Option)
        .title("Model")
        .open(true)
        .inline()
        .child(option_row("Claude", true, p))
        .child(option_row("Codex", false, p))
        .child(option_row("Gemini", false, p));
    let content = Sheet::new("story-sheet-content")
        .variant(SheetVariant::Content)
        .title("Background work")
        .open(true)
        .inline()
        .child(div().text_body().text_color(p.text.secondary).child(
            "Runs in the background. Use Back to leave; a notification tells you when it ends.",
        ))
        .footer(Button::new("story-sheet-done", "Done").variant(ButtonVariant::Primary));
    let loading = Sheet::new("story-sheet-loading")
        .title("Model")
        .open(true)
        .loading(true)
        .inline();
    let empty = Sheet::new("story-sheet-empty")
        .title("Model")
        .open(true)
        .empty("No models match this search.")
        .inline();
    let mut long = Sheet::new("story-sheet-long")
        .title("Projects")
        .open(true)
        .inline();
    for index in 0..LONG_ROWS {
        long = long.child(sheet_row(
            IconName::Folder,
            format!("Project {index}"),
            p.text.secondary,
            p.text.primary,
        ));
    }
    let column = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(variant(
            "Action: a list of actions, destructive last",
            actions,
            cx,
        ))
        .child(variant(
            "Option: single choice with a check mark",
            options,
            cx,
        ))
        .child(variant(
            "Content: a header, a body and a footer",
            content,
            cx,
        ))
        .child(variant("Loading: placeholder rows", loading, cx))
        .child(variant("Empty: one line with the reason", empty, cx))
        .child(variant(
            "Long content: the body scrolls under the header",
            long,
            cx,
        ));
    column.into_any_element()
}

/// `count` rows of one row height each, `width` px wide, with a line of text in each.
fn rows(count: usize, width: f32) -> Div {
    div()
        .flex()
        .flex_col()
        .w(px(width))
        .children((0..count).map(|row| {
            div()
                .h(px(ROW_STEP_PX))
                .px(px(SPACE_2))
                .text_body()
                .child(format!("Row {row}"))
        }))
}

/// `cells` cells side by side, wider than a scroll box.
fn cells(count: usize) -> Div {
    div()
        .flex()
        .w(px(CELL_WIDTH * count as f32))
        .h(px(ROW_STEP_PX))
        .children((0..count).map(|cell| {
            div()
                .w(px(CELL_WIDTH))
                .h_full()
                .px(px(SPACE_2))
                .text_body()
                .child(format!("Cell {cell}"))
        }))
}

fn scroll_box(content: impl IntoElement) -> impl IntoElement {
    div()
        .w(px(SCROLL_BOX_WIDTH))
        .h(px(SCROLL_BOX_HEIGHT))
        .child(content)
}

fn scroll_story(_: &mut Window, cx: &mut App) -> AnyElement {
    let mut column = div().flex().flex_col().gap(px(SPACE_3));
    column = column.child(variant(
        "Thumb shown while scrolling or hovering",
        scroll_box(
            ScrollArea::new("story-scroll-thumbs")
                .preview(ScrollPreview::Thumbs)
                .child(rows(40, SCROLL_BOX_WIDTH)),
        ),
        cx,
    ));
    column = column.child(variant(
        "Keyboard focus: inset ring",
        scroll_box(
            ScrollArea::new("story-scroll-focus")
                .preview(ScrollPreview::Focus)
                .child(rows(40, SCROLL_BOX_WIDTH)),
        ),
        cx,
    ));
    column = column.child(variant(
        "Horizontal overflow",
        scroll_box(
            ScrollArea::new("story-scroll-horizontal")
                .axis(ScrollAxis::Horizontal)
                .preview(ScrollPreview::Thumbs)
                .child(cells(12)),
        ),
        cx,
    ));
    column = column.child(variant(
        "Both axes",
        scroll_box(
            ScrollArea::new("story-scroll-both")
                .axis(ScrollAxis::Both)
                .preview(ScrollPreview::Thumbs)
                .child(rows(40, SCROLL_BOX_WIDTH * 1.5)),
        ),
        cx,
    ));
    column = column.child(variant(
        "Content fits: no thumb",
        scroll_box(
            ScrollArea::new("story-scroll-fits")
                .preview(ScrollPreview::Thumbs)
                .child(rows(3, SCROLL_BOX_WIDTH)),
        ),
        cx,
    ));
    column = column.child(variant(
        "Live: wheel, Up and Down, or drag the thumb",
        scroll_box(ScrollArea::new("story-scroll-live").child(rows(40, SCROLL_BOX_WIDTH))),
        cx,
    ));
    column.into_any_element()
}
