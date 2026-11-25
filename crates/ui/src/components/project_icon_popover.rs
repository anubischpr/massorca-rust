//! The project icon control (A41, decision 0091): a 40 px button left of a page heading that opens the
//! "Project icon" popover with the bundled Lucide grid, "Upload image…" and "Reset to folder". The colour
//! row and "Use GitHub avatar" are not drawn: the protocol carries no colour, and there is no GitHub connection.

use std::rc::Rc;

use gpui::{
    App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement, SharedString,
    Styled, Window, div, prelude::*, px,
};

use crate::components::popover::{Popover, PopoverVariant, PopoverWidth};
use crate::components::project_mark::lucide_icon;
use crate::icon::{Icon, IconName, IconSize};
use crate::style::Surface;
use crate::theme::{ActiveTheme, TextStyled, Theme};
use crate::tokens::{
    Color, RADIUS_LG, RADIUS_MD, SIZE_CONTROL_COMFORTABLE, SPACE_1, SPACE_2, SPACE_3, TYPE_BODY,
    TYPE_CAPTION,
};

const CLEAR: Color = Color::rgba8(0, 0, 0, 0.0);
/// The button's side (A41).
pub const PROJECT_ICON_BUTTON_SIZE: f32 = 40.0;
/// The folder glyph, then the Lucide icons the grid offers: short, and present in the bundled set.
pub const PROJECT_ICON_GRID: [&str; 12] = [
    "folder",
    "book",
    "code",
    "git-branch",
    "globe",
    "house",
    "layers",
    "server",
    "shield",
    "sparkles",
    "star",
    "zap",
];

/// What the popover asks its owner to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectIconChoice {
    /// A bundled Lucide icon by name.
    Lucide(&'static str),
    /// Choose a PNG to upload (PNG only, up to 256 KiB, as the protocol accepts).
    Upload,
    /// Clear the stored icon, so the folder glyph returns.
    Reset,
}

type Choose = Rc<dyn Fn(ProjectIconChoice, &mut Window, &mut App)>;

/// The control: the button that toggles the popover, and the popover while it is open. `on_open_change`
/// receives the open state a press or a dismissal asks for.
pub fn project_icon_control(
    id: impl Into<ElementId>,
    mark: impl IntoElement,
    open: bool,
    current: Option<&str>,
    on_open_change: impl Fn(bool, &mut Window, &mut App) + 'static,
    on_choose: impl Fn(ProjectIconChoice, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let theme = *cx.theme();
    let on_open_change: OpenChange = Rc::new(on_open_change);
    let choose: Choose = Rc::new(on_choose);
    let toggle = on_open_change.clone();
    let trigger = project_icon_button(mark, &theme)
        .on_click(move |_: &ClickEvent, window, cx| toggle(!open, window, cx));
    let body = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .child(
            div()
                .text_style(TYPE_CAPTION)
                .text_color(theme.palette.text.tertiary)
                .child("Tell apart projects that come from the same repository."),
        )
        .child(grid(current, choose.clone(), &theme))
        .child(upload_row(choose.clone(), &theme))
        .child(reset_row(choose, &theme));
    Popover::new(id, trigger)
        .open(open)
        .variant(PopoverVariant::Form)
        .width(PopoverWidth::Project)
        .title("Project icon")
        .child(body)
        .on_dismiss(move |window, cx| on_open_change(false, window, cx))
}

type OpenChange = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// The 40 px button that shows the project's current icon.
fn project_icon_button(mark: impl IntoElement, theme: &Theme) -> gpui::Stateful<gpui::Div> {
    let p = theme.palette;
    let fills = Surface::Overlay.fills(theme);
    div()
        .id("project-icon-button")
        .debug_selector(|| "project-icon-button".to_owned())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(PROJECT_ICON_BUTTON_SIZE))
        .rounded(px(RADIUS_LG))
        .bg(fills.selected)
        .hover(move |s| s.bg(p.fill.hover_raised))
        .cursor_pointer()
        .child(mark)
}

fn grid(current: Option<&str>, choose: Choose, theme: &Theme) -> impl IntoElement {
    let mut row = div().flex().flex_wrap().gap(px(SPACE_1));
    for name in PROJECT_ICON_GRID {
        row = row.child(grid_cell(
            name,
            current == Some(name),
            choose.clone(),
            theme,
        ));
    }
    row
}

fn grid_cell(
    name: &'static str,
    selected: bool,
    choose: Choose,
    theme: &Theme,
) -> impl IntoElement {
    let p = theme.palette;
    let fills = Surface::Overlay.fills(theme);
    let id = format!("project-icon-cell-{name}");
    let icon = lucide_icon(name).unwrap_or(IconName::Folder);
    div()
        .id(SharedString::from(id.clone()))
        .debug_selector(move || id.clone())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(SIZE_CONTROL_COMFORTABLE))
        .rounded(px(RADIUS_MD))
        .bg(if selected { fills.selected } else { CLEAR })
        .hover(move |s| s.bg(p.fill.hover_raised))
        .cursor_pointer()
        .child(Icon::new(icon).size(IconSize::Medium).color(p.text.primary))
        .on_click(move |_: &ClickEvent, window, cx| {
            choose(ProjectIconChoice::Lucide(name), window, cx)
        })
}

fn upload_row(choose: Choose, theme: &Theme) -> impl IntoElement {
    let p = theme.palette;
    div()
        .id("project-icon-upload")
        .debug_selector(|| "project-icon-upload".to_owned())
        .flex()
        .flex_col()
        .gap(px(SPACE_1))
        .px(px(SPACE_3))
        .py(px(SPACE_2))
        .rounded(px(RADIUS_MD))
        .hover(move |s| s.bg(p.fill.hover_raised))
        .cursor_pointer()
        .child(
            div()
                .text_style(TYPE_BODY)
                .text_color(p.text.primary)
                .child("Upload image…"),
        )
        .child(
            div()
                .text_style(TYPE_CAPTION)
                .text_color(p.text.tertiary)
                .child("PNG, up to 256 KiB"),
        )
        .on_click(move |_: &ClickEvent, window, cx| choose(ProjectIconChoice::Upload, window, cx))
}

fn reset_row(choose: Choose, theme: &Theme) -> impl IntoElement {
    let p = theme.palette;
    div()
        .id("project-icon-reset")
        .debug_selector(|| "project-icon-reset".to_owned())
        .px(px(SPACE_3))
        .py(px(SPACE_2))
        .rounded(px(RADIUS_MD))
        .text_style(TYPE_BODY)
        .text_color(p.text.primary)
        .hover(move |s| s.bg(p.fill.hover_raised))
        .cursor_pointer()
        .child("Reset to folder")
        .on_click(move |_: &ClickEvent, window, cx| choose(ProjectIconChoice::Reset, window, cx))
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use gpui::{
        Context, Entity, Modifiers, Render, TestAppContext, VisualTestContext, Window, div,
    };

    use super::*;

    /// What the harness's callbacks saw: each choice, and how often the open state was asked to change.
    #[derive(Clone, Default)]
    struct Recorded {
        choices: Rc<RefCell<Vec<ProjectIconChoice>>>,
        requests: Rc<Cell<u32>>,
    }

    /// Draws the control with its open state held by the view, as the desktop holds it.
    struct Harness {
        open: bool,
        recorded: Recorded,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let view = cx.entity();
            let requests = self.recorded.requests.clone();
            let choices = self.recorded.choices.clone();
            project_icon_control(
                "project-icon",
                div().size(px(20.0)),
                self.open,
                Some("folder"),
                move |open, _, cx| {
                    requests.set(requests.get() + 1);
                    view.update(cx, |harness, cx| {
                        harness.open = open;
                        cx.notify();
                    });
                },
                move |choice, _, _| choices.borrow_mut().push(choice),
                cx,
            )
        }
    }

    fn draw(cx: &mut VisualTestContext) {
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
    }

    fn click(cx: &mut VisualTestContext, selector: &'static str) {
        let at = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} is drawn"))
            .center();
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        draw(cx);
    }

    fn control(
        cx: &mut TestAppContext,
        open: bool,
    ) -> (Recorded, Entity<Harness>, &mut VisualTestContext) {
        let recorded = Recorded::default();
        let for_view = recorded.clone();
        let (view, cx) = cx.add_window_view(move |_, _| Harness {
            open,
            recorded: for_view,
        });
        draw(cx);
        (recorded, view, cx)
    }

    #[test]
    fn every_grid_icon_is_in_the_bundled_set() {
        assert_eq!(PROJECT_ICON_GRID.len(), 12);
        assert_eq!(PROJECT_ICON_GRID[0], "folder");
        for name in PROJECT_ICON_GRID {
            assert!(lucide_icon(name).is_some(), "{name}");
        }
    }

    #[gpui::test]
    fn a_closed_control_draws_only_its_button(cx: &mut TestAppContext) {
        let (recorded, _view, cx) = control(cx, false);
        assert!(cx.debug_bounds("project-icon-button").is_some());
        assert!(cx.debug_bounds("project-icon-cell-star").is_none());
        assert_eq!(recorded.requests.get(), 0);
    }

    #[gpui::test]
    fn the_button_opens_the_grid_and_a_cell_chooses_its_icon(cx: &mut TestAppContext) {
        let (recorded, _view, cx) = control(cx, false);
        click(cx, "project-icon-button");
        assert_eq!(recorded.requests.get(), 1);
        assert!(cx.debug_bounds("project-icon-cell-star").is_some());
        click(cx, "project-icon-cell-star");
        assert_eq!(
            *recorded.choices.borrow(),
            vec![ProjectIconChoice::Lucide("star")]
        );
    }

    #[gpui::test]
    fn reset_and_upload_ask_for_their_choices(cx: &mut TestAppContext) {
        let (recorded, _view, cx) = control(cx, true);
        click(cx, "project-icon-reset");
        click(cx, "project-icon-upload");
        assert_eq!(
            *recorded.choices.borrow(),
            vec![ProjectIconChoice::Reset, ProjectIconChoice::Upload]
        );
    }
}
