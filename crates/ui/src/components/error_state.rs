//! ErrorState: what a page, panel, section or Work panel tab shows when its
//! content failed to load (docs/design/components/ErrorState.md). It says what
//! failed and what is kept, and puts Retry first. "Show details" opens the
//! error code and time in a mono block.

use crate::a11y;
use crate::components::button::{Button, ButtonSize, ButtonVariant, ClickHandler};
use crate::components::icon_button::{IconButton, IconButtonVariant};
use crate::icon::{Icon, IconName, IconSize};
use crate::style::Surface;
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    RADIUS_LG, RADIUS_MD, SPACE_2, SPACE_4, SPACE_8, TYPE_BODY_STRONG, TYPE_MONO_SM, TYPE_TITLE,
    TYPE_TITLE_SM, TextStyleToken,
};
use gpui::accesskit::Live;
use gpui::{
    App, ClipboardItem, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    Role, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

/// Where an error state sits and how much of a panel it draws.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ErrorStateVariant {
    /// A whole page or a Work panel tab body: a panel, the page title, Retry and Show details.
    Page,
    /// A list, card, sidebar section or picker: a panel with title-sm and Retry.
    #[default]
    Section,
    /// A line inside a dialog, popover or sheet: no panel, Retry as a text button.
    Inline,
}

impl ErrorStateVariant {
    /// The panel padding, or `None` when the variant draws no panel.
    fn padding(self) -> Option<f32> {
        match self {
            Self::Page => Some(SPACE_8),
            Self::Section => Some(SPACE_4),
            Self::Inline => None,
        }
    }

    fn title_style(self) -> TextStyleToken {
        match self {
            Self::Page => TYPE_TITLE,
            Self::Section => TYPE_TITLE_SM,
            Self::Inline => TYPE_BODY_STRONG,
        }
    }

    /// The octagon glyph sits with the title in the panel variants only.
    fn glyph(self) -> bool {
        !matches!(self, Self::Inline)
    }

    /// Only the page variant offers Show details.
    fn shows_details(self) -> bool {
        matches!(self, Self::Page)
    }
}

/// A failed load: a title, an optional explanation, Retry and, on a page, details.
#[derive(IntoElement)]
pub struct ErrorState {
    id: ElementId,
    variant: ErrorStateVariant,
    title: SharedString,
    explanation: Option<SharedString>,
    details: Option<SharedString>,
    retrying: bool,
    on_retry: Option<ClickHandler>,
}

impl std::fmt::Debug for ErrorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ErrorState")
            .field("id", &self.id)
            .field("variant", &self.variant)
            .field("title", &self.title)
            .field("retrying", &self.retrying)
            .finish_non_exhaustive()
    }
}

impl ErrorState {
    /// `title` says what failed, plainly: "Could not load routines".
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            variant: ErrorStateVariant::default(),
            title: title.into(),
            explanation: None,
            details: None,
            retrying: false,
            on_retry: None,
        }
    }

    pub fn variant(mut self, variant: ErrorStateVariant) -> Self {
        self.variant = variant;
        self
    }

    /// One or two sentences on what is kept and what can be done.
    pub fn explanation(mut self, text: impl Into<SharedString>) -> Self {
        self.explanation = Some(text.into());
        self
    }

    /// The error code and time, shown in a mono block when Show details is open.
    pub fn details(mut self, text: impl Into<SharedString>) -> Self {
        self.details = Some(text.into());
        self
    }

    /// While a retry runs, Retry shows a spinner and "Retrying" and cannot be activated.
    pub fn retrying(mut self, retrying: bool) -> Self {
        self.retrying = retrying;
        self
    }

    pub fn on_retry(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_retry = Some(std::rc::Rc::new(listener));
        self
    }
}

impl RenderOnce for ErrorState {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (name, live) = (
            self.title.clone(),
            if matches!(self.variant, ErrorStateVariant::Page) {
                Live::Assertive
            } else {
                Live::Polite
            },
        );
        let theme = *cx.theme();
        let p = theme.palette;
        let inline = self.variant == ErrorStateVariant::Inline;
        let details_open =
            window.use_keyed_state((self.id.clone(), "details-open"), cx, |_, _| false);
        let open = *details_open.read(cx);

        let mut retry = Button::new(
            (self.id.clone(), "retry"),
            if self.retrying { "Retrying" } else { "Retry" },
        )
        .variant(if inline {
            ButtonVariant::Ghost
        } else {
            ButtonVariant::Secondary
        })
        .size(ButtonSize::Default)
        .leading_icon(IconName::RefreshCw)
        .loading(self.retrying);
        if let Some(on_retry) = self.on_retry {
            retry = retry.on_click(move |window, cx| on_retry(window, cx));
        }

        let mut panel = div()
            .id(self.id.clone())
            .role(Role::Alert)
            .aria_label(name)
            .flex()
            .flex_col()
            .gap(px(SPACE_2))
            .min_w_0();
        if let Some(padding) = self.variant.padding() {
            panel = panel
                .p(px(padding))
                .rounded(px(RADIUS_LG))
                .border_1()
                .border_color(p.status.danger_border)
                .bg(p.status.danger_bg);
        }

        let mut heading = div().flex().items_center().gap(px(SPACE_2)).min_w_0();
        if self.variant.glyph() {
            heading = heading.child(
                Icon::new(IconName::OctagonAlert)
                    .size(IconSize::Large)
                    .color(p.status.danger_fg),
            );
        }
        panel = panel.child(
            heading.child(
                div()
                    .min_w_0()
                    .text_style(self.variant.title_style())
                    .text_color(p.text.primary)
                    .child(self.title),
            ),
        );

        if let Some(text) = self.explanation {
            panel = panel.child(
                div()
                    .min_w_0()
                    .text_body()
                    .text_color(p.text.secondary)
                    .child(text),
            );
        }

        let details = self.details.filter(|_| self.variant.shows_details());
        if let (true, Some(text)) = (open, details.as_ref()) {
            let copied = text.to_string();
            let copy = IconButton::new(
                (self.id.clone(), "details-copy"),
                IconName::Copy,
                "Copy details",
            )
            .variant(IconButtonVariant::Ghost)
            .size(ButtonSize::Compact)
            .surface(Surface::Raised)
            .on_click(move |_, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()))
            });
            panel = panel.child(
                div()
                    .debug_selector(|| "error-details".into())
                    .flex()
                    .items_start()
                    .gap(px(SPACE_2))
                    .p(px(SPACE_2))
                    .rounded(px(RADIUS_MD))
                    .border_1()
                    .border_color(p.border.subtle)
                    .bg(p.surface.sunken)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_style(TYPE_MONO_SM)
                            .text_color(p.text.primary)
                            .child(text.clone()),
                    )
                    .child(copy),
            );
        }

        let mut actions = div()
            .flex()
            .items_center()
            .gap(px(SPACE_2))
            .child(div().debug_selector(|| "error-retry".into()).child(retry));
        if details.is_some() {
            let toggle = Button::new((self.id.clone(), "details-toggle"), "Show details")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Default)
                .selected(open)
                .on_click(move |_, cx| {
                    details_open.update(cx, |open, cx| {
                        *open = !*open;
                        cx.notify();
                    })
                });
            actions = actions.child(div().debug_selector(|| "error-toggle".into()).child(toggle));
        }
        a11y::annotate(panel.child(actions), move |node| node.set_live(live))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn variants_follow_the_spec_metrics() {
        assert_eq!(ErrorStateVariant::Page.padding(), Some(SPACE_8));
        assert_eq!(ErrorStateVariant::Section.padding(), Some(SPACE_4));
        assert_eq!(ErrorStateVariant::Inline.padding(), None);
        assert_eq!(ErrorStateVariant::Page.title_style(), TYPE_TITLE);
        assert_eq!(ErrorStateVariant::Section.title_style(), TYPE_TITLE_SM);
        assert_eq!(ErrorStateVariant::Inline.title_style(), TYPE_BODY_STRONG);
        assert!(!ErrorStateVariant::Inline.glyph());
        assert!(ErrorStateVariant::Section.glyph());
        assert_eq!(ErrorStateVariant::default(), ErrorStateVariant::Section);
    }

    #[test]
    fn only_the_page_variant_offers_details() {
        assert!(ErrorStateVariant::Page.shows_details());
        assert!(!ErrorStateVariant::Section.shows_details());
        assert!(!ErrorStateVariant::Inline.shows_details());
    }

    #[gpui::test]
    fn retry_runs_its_handler(cx: &mut gpui::TestAppContext) {
        let retries = Rc::new(Cell::new(0_u32));
        let (_, cx) = cx.add_window_view(|_, _| Harness::new(retries.clone(), false));
        cx.run_until_parked();
        let center = cx
            .debug_bounds("error-retry")
            .expect("retry is drawn")
            .center();
        cx.simulate_click(center, gpui::Modifiers::none());
        assert_eq!(retries.get(), 1);
    }

    #[gpui::test]
    fn a_retry_in_flight_cannot_be_activated(cx: &mut gpui::TestAppContext) {
        let retries = Rc::new(Cell::new(0_u32));
        let (_, cx) = cx.add_window_view(|_, _| Harness::new(retries.clone(), true));
        cx.run_until_parked();
        let center = cx
            .debug_bounds("error-retry")
            .expect("retry is drawn")
            .center();
        cx.simulate_click(center, gpui::Modifiers::none());
        assert_eq!(retries.get(), 0);
    }

    #[gpui::test]
    fn show_details_reveals_the_error_block_and_hides_it_again(cx: &mut gpui::TestAppContext) {
        let retries = Rc::new(Cell::new(0_u32));
        let (_, cx) = cx.add_window_view(|_, _| Harness::new(retries.clone(), false));
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("error-details").is_none(),
            "hidden by default"
        );

        let toggle = cx
            .debug_bounds("error-toggle")
            .expect("toggle is drawn")
            .center();
        cx.simulate_click(toggle, gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("error-details").is_some(), "opens on click");

        let toggle = cx
            .debug_bounds("error-toggle")
            .expect("toggle is drawn")
            .center();
        cx.simulate_click(toggle, gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("error-details").is_none(),
            "closes on second click"
        );
    }

    /// Hosts a page-size error state with details, optionally mid-retry.
    struct Harness {
        retries: Rc<Cell<u32>>,
        retrying: bool,
    }

    impl Harness {
        fn new(retries: Rc<Cell<u32>>, retrying: bool) -> Self {
            Self { retries, retrying }
        }
    }

    impl gpui::Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let retries = self.retries.clone();
            div().w(px(480.)).child(
                ErrorState::new("routines", "Could not load routines")
                    .variant(ErrorStateVariant::Page)
                    .explanation("Your changes are kept.")
                    .details("E_TIMEOUT at 10:14:03")
                    .retrying(self.retrying)
                    .on_retry(move |_, _| retries.set(retries.get() + 1)),
            )
        }
    }
}

#[cfg(test)]
mod a11y_tests {
    use super::*;
    use crate::a11y::probe::{reported, root};
    use gpui::{
        TestAppContext,
        accesskit::{Live, Role},
    };

    #[gpui::test]
    fn a_page_error_is_an_assertive_alert_named_by_its_title(cx: &mut TestAppContext) {
        let node = root(cx, |window, cx| {
            reported(
                ErrorState::new("load", "Could not load sessions")
                    .variant(ErrorStateVariant::Page)
                    .render(window, cx),
            )
        })
        .expect("reported");
        assert_eq!(node.role(), Role::Alert);
        assert_eq!(node.label(), Some("Could not load sessions"));
        assert_eq!(node.live(), Some(Live::Assertive));
    }

    #[gpui::test]
    fn a_section_error_is_a_polite_alert(cx: &mut TestAppContext) {
        let node = root(cx, |window, cx| {
            reported(ErrorState::new("list", "Could not load").render(window, cx))
        })
        .expect("reported");
        assert_eq!(node.role(), Role::Alert);
        assert_eq!(node.live(), Some(Live::Polite));
    }
}
