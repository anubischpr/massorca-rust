//! The component gallery: every component with its states, in the current
//! theme, with a toggle to the other theme (decision 0025). Opened only by a
//! person; automated runs shoot every story headlessly (`crates/ui/tests/screenshots.rs`).

mod composer;
mod controls;
mod feedback;
mod fields;
mod indicators;
mod layout;
mod lists;
mod overlays;
mod panels;
mod session;

use crate::theme::{ActiveTheme, TextStyled, Theme};
use crate::tokens::{SPACE_2, SPACE_3, SPACE_4, SPACE_6, TYPE_BODY, TYPE_HEADING, TYPE_LABEL};
use gpui::{
    AnyElement, App, Context, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};

/// One gallery entry: a component name, the spec it follows and a render
/// function that returns every state of the component side by side.
#[derive(Clone, Copy, Debug)]
pub struct Story {
    pub name: &'static str,
    /// Spec file under `docs/design/components/`.
    pub spec: &'static str,
    pub render: fn(&mut Window, &mut App) -> AnyElement,
}

/// Every story, in the gallery's navigation order.
pub fn stories() -> Vec<Story> {
    let mut all = Vec::new();
    all.extend(controls::stories());
    all.extend(indicators::stories());
    all.extend(fields::stories());
    all.extend(lists::stories());
    all.extend(overlays::stories());
    all.extend(feedback::stories());
    all.extend(layout::stories());
    all.extend(panels::stories());
    all.extend(session::stories());
    all.extend(composer::stories());
    all
}

/// A labelled row of examples inside a story.
pub fn variant(label: impl Into<SharedString>, content: impl IntoElement, cx: &App) -> AnyElement {
    let p = &cx.theme().palette;
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .child(
            div()
                .text_style(TYPE_LABEL)
                .text_color(p.text.tertiary)
                .child(label.into()),
        )
        .child(content)
        .into_any_element()
}

/// The gallery window's root view.
#[derive(Debug, Default)]
pub struct Gallery {
    selected: usize,
}

impl Gallery {
    pub fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = index;
        cx.notify();
    }
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let stories = stories();
        let selected = self.selected.min(stories.len().saturating_sub(1));
        let p = cx.theme().palette;
        let is_dark = cx.theme().is_dark();

        let nav = div()
            .id("gallery-nav")
            .flex()
            .flex_col()
            .w(px(220.))
            .h_full()
            .overflow_y_scroll()
            .p(px(SPACE_3))
            .gap(px(2.))
            .bg(p.surface.frame)
            .children(stories.iter().enumerate().map(|(i, story)| {
                let active = i == selected;
                div()
                    .id(("story", i))
                    .px(px(SPACE_2))
                    .py(px(4.))
                    .rounded(px(6.))
                    .text_body()
                    .text_color(if active {
                        p.text.primary
                    } else {
                        p.text.secondary
                    })
                    .when_active_bg(active, p.fill.selected_frame)
                    .hover(move |s| s.bg(p.fill.hover_frame))
                    .on_click(cx.listener(move |this, _, _, cx| this.select(i, cx)))
                    .child(story.name)
            }));

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .px(px(SPACE_6))
            .py(px(SPACE_4))
            .child(
                div()
                    .text_style(TYPE_HEADING)
                    .text_color(p.text.primary)
                    .child(
                        stories
                            .get(selected)
                            .map_or("No components yet", |s| s.name),
                    ),
            )
            .child(
                div()
                    .id("theme-toggle")
                    .px(px(SPACE_3))
                    .py(px(4.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(p.border.default)
                    .text_style(TYPE_BODY)
                    .text_color(p.text.primary)
                    .hover(move |s| s.bg(p.fill.hover_canvas))
                    .on_click(|_, _, cx| Theme::toggle(cx))
                    .child(if is_dark {
                        "Switch to light"
                    } else {
                        "Switch to dark"
                    }),
            );

        let body = stories
            .get(selected)
            .map(|story| (story.render)(window, cx));

        div()
            .flex()
            .size_full()
            .bg(p.surface.frame)
            .font_family(crate::assets::FONT_SANS)
            .child(nav)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .mt(px(SPACE_2))
                    .rounded_tl(px(crate::tokens::RADIUS_CANVAS))
                    .bg(p.surface.canvas)
                    .child(header)
                    .child(
                        div()
                            .id("story-body")
                            .flex_1()
                            .overflow_y_scroll()
                            .px(px(SPACE_6))
                            .pb(px(SPACE_6))
                            .children(body),
                    ),
            )
    }
}

trait WhenActiveBg: Styled + Sized {
    fn when_active_bg(self, active: bool, color: crate::tokens::Color) -> Self {
        if active { self.bg(color) } else { self }
    }
}

impl<T: Styled> WhenActiveBg for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn story_names_are_unique() {
        let names: Vec<_> = stories().iter().map(|s| s.name).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(names.len(), sorted.len());
    }

    #[gpui::test]
    fn every_story_renders_in_both_themes(cx: &mut gpui::TestAppContext) {
        for appearance in [crate::Appearance::Dark, crate::Appearance::Light] {
            cx.update(|cx| Theme::set(appearance, cx));
            for (i, _) in stories().iter().enumerate() {
                let window = cx.add_window(|_, _| Gallery { selected: i });
                window.update(cx, |_, window, _| window.refresh()).unwrap();
                cx.run_until_parked();
            }
        }
    }
}
