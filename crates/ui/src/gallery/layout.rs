//! Gallery stories for the layout group: the splitter, the scroll area and the sheet.

use super::{Story, variant};
use crate::components::splitter::{
    Extent, RegionSide, Splitter, SplitterLimits, SplitterModel, SplitterPreview,
};
use crate::theme::ActiveTheme;
use crate::tokens::{
    LAYOUT_SIDEBAR_DEFAULT, LAYOUT_SIDEBAR_MAX, LAYOUT_SIDEBAR_MIN, LAYOUT_WORKPANEL_DEFAULT,
    LAYOUT_WORKPANEL_MIN, SPACE_3,
};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

/// Height of one lane in the splitter stories.
const LANE_HEIGHT: f32 = 48.0;

pub fn stories() -> Vec<Story> {
    vec![Story {
        name: "Splitter",
        spec: "Splitter.md",
        render: splitter_story,
    }]
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
