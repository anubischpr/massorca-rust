//! EmptyState: what a list, page or panel shows when it has nothing yet
//! (docs/design/components/EmptyState.md). It holds one title, one explanation
//! and one action. The Work panel variant (A43) is a centred column of actions
//! with no panel, title or explanation.

use crate::components::button::{Button, ButtonVariant};
use crate::icon::{Icon, IconName, IconSize};
use crate::text_preferences;
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    LAYOUT_EMPTY_STATE_TEXT_MAX_WIDTH, Measure, RADIUS_LG, SPACE_1, SPACE_2, SPACE_4, SPACE_8,
    TYPE_BODY, TYPE_BODY_SM, TYPE_DISPLAY, TYPE_LABEL, TYPE_TITLE_SM, TextStyleToken,
};
use gpui::{
    AnyElement, App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, Role,
    SharedString, StatefulInteractiveElement, Styled, Window, div, font, px,
};

/// Where an empty state sits and how much room it takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EmptyStateVariant {
    /// A whole page (Routines, Tasks, Calendar): display title, centred, one primary action.
    Page,
    /// A sidebar section, card or panel: title-sm, one action.
    #[default]
    Section,
    /// Inside a list where a panel would be too heavy: a label and one line, no panel.
    Compact,
    /// The Work panel (A43): a centred column of actions, no panel, title or explanation.
    WorkPanel,
}

impl EmptyStateVariant {
    fn padding(self) -> f32 {
        match self {
            Self::Page => SPACE_8,
            Self::Section => SPACE_4,
            Self::Compact | Self::WorkPanel => SPACE_2,
        }
    }

    fn title_style(self) -> TextStyleToken {
        match self {
            Self::Page => TYPE_DISPLAY,
            Self::Section => TYPE_TITLE_SM,
            Self::Compact | Self::WorkPanel => TYPE_LABEL,
        }
    }

    /// Whether the panel has the raised fill, the border and the radius.
    fn framed(self) -> bool {
        matches!(self, Self::Page | Self::Section)
    }

    fn centred(self) -> bool {
        matches!(self, Self::Page)
    }
}

/// An empty state: an optional icon, title, explanation, one action and an optional note.
#[derive(IntoElement)]
pub struct EmptyState {
    id: ElementId,
    variant: EmptyStateVariant,
    icon: Option<IconName>,
    title: Option<SharedString>,
    explanation: Option<SharedString>,
    action: Option<Button>,
    actions: Vec<AnyElement>,
    note: Option<SharedString>,
}

impl std::fmt::Debug for EmptyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmptyState")
            .field("id", &self.id)
            .field("variant", &self.variant)
            .field("title", &self.title)
            .field("explanation", &self.explanation)
            .field("actions", &self.actions.len())
            .finish_non_exhaustive()
    }
}

impl EmptyState {
    /// A section-size empty state with a plain fact as its title ("No routines yet").
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            variant: EmptyStateVariant::default(),
            icon: None,
            title: Some(title.into()),
            explanation: None,
            action: None,
            actions: Vec::new(),
            note: None,
        }
    }

    /// The Work panel empty state (A43): the actions, each as wide as its label, in a centred column.
    pub fn work_panel(
        id: impl Into<ElementId>,
        actions: impl IntoIterator<Item = impl IntoElement>,
    ) -> Self {
        Self {
            variant: EmptyStateVariant::WorkPanel,
            title: None,
            actions: actions
                .into_iter()
                .map(IntoElement::into_any_element)
                .collect(),
            ..Self::new(id, "")
        }
    }

    /// "No matches for "query"" with a "Clear search" action that runs `on_clear`.
    pub fn filtered(
        id: impl Into<ElementId>,
        query: &str,
        on_clear: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        let id = id.into();
        let clear = Button::new((id.clone(), "clear-search"), "Clear search")
            .variant(ButtonVariant::Secondary)
            .on_click(on_clear);
        Self {
            title: Some(format!("No matches for \"{query}\"").into()),
            action: Some(clear),
            ..Self::new(id, "")
        }
    }

    pub fn variant(mut self, variant: EmptyStateVariant) -> Self {
        self.variant = variant;
        self
    }

    /// A decorative icon above the title (`size.icon.lg`, `text.secondary`).
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// One sentence on what the thing is for, or what to do next.
    pub fn explanation(mut self, text: impl Into<SharedString>) -> Self {
        self.explanation = Some(text.into());
        self
    }

    /// The one action, below the explanation.
    pub fn action(mut self, button: Button) -> Self {
        self.action = Some(button);
        self
    }

    /// A footer note for host or scope context (`body-sm`, `text.tertiary`).
    pub fn note(mut self, text: impl Into<SharedString>) -> Self {
        self.note = Some(text.into());
        self
    }
}

/// The width of a measure in logical pixels. `Ch` is the advance of `0` in the body face
/// (the `layout.empty-state.text.max-width` token is 60 ch).
fn measure_px(cx: &App, measure: Measure) -> Option<f32> {
    match measure {
        Measure::Px(px) => Some(px),
        Measure::Ch(count) => {
            let text = cx.text_system();
            let face = text.resolve_font(&font(text_preferences::interface_family()));
            let body = px(text_preferences::scale_size(TYPE_BODY.size));
            let zero = text.advance(face, body, '0').ok()?;
            Some(f32::from(zero.width) * count)
        }
        Measure::Percent(_) => None,
    }
}

impl RenderOnce for EmptyState {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let name = self.title.clone();
        if self.variant == EmptyStateVariant::WorkPanel {
            return div()
                .id(self.id.clone())
                .flex()
                .flex_col()
                .items_center()
                .gap(px(SPACE_2))
                .children(self.actions);
        }

        let theme = *cx.theme();
        let p = theme.palette;
        let explanation_lines = if self.variant == EmptyStateVariant::Compact {
            1
        } else {
            2
        };
        let explanation_width = measure_px(cx, LAYOUT_EMPTY_STATE_TEXT_MAX_WIDTH);

        let mut panel = div()
            .id(self.id.clone())
            .role(Role::Group)
            .debug_selector(|| "empty-panel".into())
            .flex()
            .flex_col()
            .items_start()
            .gap(px(SPACE_2))
            .min_w_0()
            .p(px(self.variant.padding()));
        if self.variant.framed() {
            panel = panel
                .rounded(px(RADIUS_LG))
                .border_1()
                .border_color(p.border.default)
                .bg(p.surface.raised);
        }
        if self.variant.centred() {
            panel = panel.items_center().text_center();
        }
        if let Some(icon) = self.icon {
            panel = panel.child(
                Icon::new(icon)
                    .size(IconSize::Large)
                    .color(p.text.secondary),
            );
        }
        if let Some(title) = self.title {
            panel = panel.child(
                div()
                    .text_style(self.variant.title_style())
                    .text_color(p.text.primary)
                    .child(title),
            );
        }
        if let Some(text) = self.explanation {
            let mut line = div()
                .min_w_0()
                .text_body()
                .text_color(p.text.secondary)
                .line_clamp(explanation_lines)
                .child(text);
            if let Some(width) = explanation_width {
                line = line.max_w(px(width));
            }
            panel = panel.child(line);
        }
        if let Some(action) = self.action {
            panel = panel.child(
                div()
                    .debug_selector(|| "empty-action".into())
                    .pt(px(SPACE_1))
                    .child(action),
            );
        }
        if let Some(note) = self.note {
            panel = panel.child(
                div()
                    .pt(px(SPACE_1))
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.text.tertiary)
                    .child(note),
            );
        }
        if let Some(title) = name {
            panel = panel.aria_label(title);
        }
        panel
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::InteractiveElement;

    #[test]
    fn variants_follow_the_spec_metrics() {
        assert_eq!(EmptyStateVariant::Page.padding(), SPACE_8);
        assert_eq!(EmptyStateVariant::Section.padding(), SPACE_4);
        assert_eq!(EmptyStateVariant::Compact.padding(), SPACE_2);
        assert_eq!(EmptyStateVariant::Page.title_style(), TYPE_DISPLAY);
        assert_eq!(EmptyStateVariant::Section.title_style(), TYPE_TITLE_SM);
        assert!(EmptyStateVariant::Page.framed() && EmptyStateVariant::Section.framed());
        assert!(!EmptyStateVariant::Compact.framed());
        assert!(!EmptyStateVariant::WorkPanel.framed());
        assert!(EmptyStateVariant::Page.centred());
        assert!(!EmptyStateVariant::Section.centred());
        assert_eq!(EmptyStateVariant::default(), EmptyStateVariant::Section);
    }

    #[test]
    fn work_panel_has_no_title_or_explanation() {
        let panel = EmptyState::work_panel("work", Vec::<Button>::new());
        assert_eq!(panel.variant, EmptyStateVariant::WorkPanel);
        assert!(panel.title.is_none());
        assert!(panel.explanation.is_none());
    }

    #[test]
    fn filtered_names_the_query_in_its_title() {
        let state = EmptyState::filtered("files", "docs", |_, _| {});
        assert_eq!(
            state.title.as_ref().map(|t| t.as_ref()),
            Some("No matches for \"docs\"")
        );
        assert!(state.action.is_some());
    }

    #[gpui::test]
    fn measures_convert_pixels_and_character_widths(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            assert_eq!(measure_px(cx, Measure::Px(300.0)), Some(300.0));
            assert_eq!(measure_px(cx, Measure::Percent(50.0)), None);
            let sixty = measure_px(cx, Measure::Ch(60.0)).expect("ch resolves");
            let one = measure_px(cx, Measure::Ch(1.0)).expect("ch resolves");
            assert!(one > 0.0);
            assert!((sixty - one * 60.0).abs() < 0.5, "{sixty} vs {one}");
        });
    }

    #[gpui::test]
    fn the_work_panel_actions_form_a_centred_column(cx: &mut gpui::TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| WorkHarness);
        cx.run_until_parked();
        let wrapper = cx.debug_bounds("wrapper").expect("wrapper is drawn");
        for name in ["first", "second", "third"] {
            let bounds = cx.debug_bounds(name).expect("action is drawn");
            assert!(
                bounds.size.width < wrapper.size.width,
                "{name} is as wide as the column"
            );
            let offset = bounds.center().x - wrapper.center().x;
            assert!(
                f32::from(offset).abs() < 0.5,
                "{name} is off centre by {offset:?}"
            );
        }
    }

    #[gpui::test]
    fn a_section_action_is_only_as_wide_as_its_label(cx: &mut gpui::TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| SectionHarness);
        cx.run_until_parked();
        let panel = cx.debug_bounds("empty-panel").expect("panel is drawn");
        let action = cx.debug_bounds("empty-action").expect("action is drawn");
        // The content box: the section padding and the 1 px border on each side.
        let content = f32::from(panel.size.width) - 2.0 * (SPACE_4 + 1.0);
        assert!(
            f32::from(action.size.width) < content,
            "the action is as wide as the panel's content box"
        );
    }

    /// A section-size empty state with one primary action, in a 400 px column.
    struct SectionHarness;

    impl gpui::Render for SectionHarness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            div().w(px(400.)).child(
                EmptyState::new("routines", "No routines yet")
                    .explanation("Routines run a task on a schedule. Create one to start.")
                    .action(
                        Button::new("routines-new", "New routine").variant(ButtonVariant::Primary),
                    ),
            )
        }
    }

    /// Three actions of different widths in a 400 px column, each wrapped to be measured.
    struct WorkHarness;

    impl gpui::Render for WorkHarness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let action = |name: &'static str, id: &'static str, label: &'static str, icon| {
                div()
                    .debug_selector(move || name.into())
                    .child(Button::new(id, label).leading_icon(icon))
            };
            div().child(div().w(px(400.)).debug_selector(|| "wrapper".into()).child(
                EmptyState::work_panel(
                    "work",
                    [
                        action("first", "a", "New terminal", IconName::SquareTerminal),
                        action("second", "b", "New browser tab", IconName::Globe),
                        action("third", "c", "New Markdown", IconName::FileText),
                    ],
                ),
            ))
        }
    }
}

#[cfg(test)]
mod a11y_tests {
    use super::*;
    use crate::a11y::probe::{reported, root};
    use gpui::{TestAppContext, accesskit::Role};

    #[gpui::test]
    fn an_empty_state_is_a_group_named_by_its_title(cx: &mut TestAppContext) {
        let node = root(cx, |window, cx| {
            reported(EmptyState::new("sessions", "No sessions yet").render(window, cx))
        })
        .expect("reported");
        assert_eq!(node.role(), Role::Group);
        assert_eq!(node.label(), Some("No sessions yet"));
    }
}
