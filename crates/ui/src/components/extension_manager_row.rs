//! One installed extension as a row of the Installed list (docs/design/components/ExtensionManagerRow.md): the
//! plate, the name and its byline, the status word, the Enabled switch and the More button. Desktop only.

use super::agent_icon::{AgentIcon, AgentIconSize, AgentKind};
use super::badge::{Badge, BadgeTone};
use super::button::{Button, ButtonSize, ButtonVariant};
use super::extension_card::{
    ActionHandler, ExtensionAction, ExtensionState, route, route_click, route_toggle, status_word,
};
use super::icon_button::{IconButton, IconButtonVariant};
use super::switch::Switch;
use crate::icon::{Icon, IconName, IconSize};
use crate::style::FocusRing;
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    Palette, RADIUS_MD, SPACE_2, SPACE_3, SPACE_4, TYPE_BODY_SM, TYPE_BODY_STRONG,
};
use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, Role,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use std::rc::Rc;

/// The plate is 36 px square (teams-extensions-settings.md 7.4).
const PLATE_SIZE: f32 = 36.0;
/// The status column is 200 px wide, as the artboard's row grid has it.
const STATUS_WIDTH: f32 = 200.0;

/// One row of the Installed list. The caller keeps the state and acts on each action the row sends.
#[derive(IntoElement)]
pub struct ExtensionManagerRow {
    key: SharedString,
    name: SharedString,
    publisher: SharedString,
    version: SharedString,
    contribution: SharedString,
    icon: IconName,
    agent: Option<AgentKind>,
    state: ExtensionState,
    status_word: SharedString,
    on_action: Option<ActionHandler>,
}

impl std::fmt::Debug for ExtensionManagerRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtensionManagerRow")
            .field("key", &self.key)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl ExtensionManagerRow {
    pub fn new(
        key: impl Into<SharedString>,
        name: impl Into<SharedString>,
        state: ExtensionState,
    ) -> Self {
        Self {
            key: key.into(),
            name: name.into(),
            publisher: SharedString::default(),
            version: SharedString::default(),
            contribution: SharedString::default(),
            icon: IconName::Puzzle,
            agent: None,
            state,
            status_word: SharedString::default(),
            on_action: None,
        }
    }

    pub fn publisher(mut self, publisher: impl Into<SharedString>) -> Self {
        self.publisher = publisher.into();
        self
    }

    pub fn version(mut self, version: impl Into<SharedString>) -> Self {
        self.version = version.into();
        self
    }

    /// What the extension adds, in one line: "Rail item, Settings".
    pub fn contribution(mut self, contribution: impl Into<SharedString>) -> Self {
        self.contribution = contribution.into();
        self
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = icon;
        self
    }

    /// The agent's own mark, drawn in the plate in place of the icon.
    pub fn agent(mut self, agent: AgentKind) -> Self {
        self.agent = Some(agent);
        self
    }

    /// The status line's word, such as "Enabled" or "Update available 2.1.0", as the desktop model words it.
    pub fn status_word(mut self, word: impl Into<SharedString>) -> Self {
        self.status_word = word.into();
        self
    }

    /// Runs with the action a press asks for: Open, Toggle, More or ViewLog.
    pub fn on_action(
        mut self,
        listener: impl Fn(ExtensionAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(listener));
        self
    }
}

impl RenderOnce for ExtensionManagerRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let key = self.key.clone();
        let handler = self.on_action.clone();
        let name_focus = window
            .use_keyed_state(
                SharedString::from(format!("{key}-name-focus")),
                cx,
                |_, cx| cx.focus_handle(),
            )
            .read(cx)
            .clone();
        let enabled = matches!(self.state, ExtensionState::Installed { enabled: true, .. });
        let installing = matches!(self.state, ExtensionState::Installing { .. });
        let name_color = if matches!(self.state, ExtensionState::Installed { enabled: false, .. }) {
            p.text.secondary
        } else {
            p.text.primary
        };

        let plate = div()
            .flex_none()
            .size(px(PLATE_SIZE))
            .rounded(px(RADIUS_MD))
            .bg(p.surface.sunken)
            .flex()
            .items_center()
            .justify_center()
            .child(match self.agent {
                Some(agent) => AgentIcon::new(agent)
                    .size(AgentIconSize::Small)
                    .into_any_element(),
                None => Icon::new(self.icon)
                    .size(IconSize::Small)
                    .color(p.text.secondary)
                    .into_any_element(),
            });

        let name_selector = key.clone();
        let name = div()
            .id(SharedString::from(format!("{key}-name")))
            .debug_selector(move || format!("row-name:{name_selector}"))
            .track_focus(&name_focus)
            .tab_stop(true)
            .focus_ring(&theme)
            .cursor_pointer()
            .min_w_0()
            .truncate()
            .text_style(TYPE_BODY_STRONG)
            .text_color(name_color)
            .on_click(route_click(&handler, ExtensionAction::Open))
            .child(self.name.clone());
        let byline = div()
            .min_w_0()
            .truncate()
            .text_style(TYPE_BODY_SM)
            .text_color(p.text.secondary)
            .child(byline_text(
                &self.publisher,
                &self.version,
                &self.contribution,
            ));
        let text = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .child(name)
            .child(byline);

        let status = div()
            .w(px(STATUS_WIDTH))
            .flex_none()
            .min_w_0()
            .child(status_content(
                &self.state,
                self.status_word.clone(),
                &p,
                &key,
                &handler,
            ));

        let switch_selector = key.clone();
        let switch = div()
            .flex_none()
            .debug_selector(move || format!("row-switch:{switch_selector}"))
            .child(
                Switch::new(SharedString::from(format!("{key}-switch")), enabled)
                    .accessible_name(format!("Enable {}", self.name))
                    .disabled(installing)
                    .on_change(route_toggle(&handler)),
            );

        let more_id = format!("{key}-more");
        let more_selector = more_id.clone();
        let more = div()
            .flex_none()
            .debug_selector(move || format!("button:{more_selector}"))
            .child(
                IconButton::new(
                    SharedString::from(more_id),
                    IconName::Ellipsis,
                    format!("More actions for {}", self.name),
                )
                .variant(IconButtonVariant::Ghost)
                .size(ButtonSize::Compact)
                .on_click(route(&handler, ExtensionAction::More)),
            );

        div()
            .id(key.clone())
            .role(Role::ListItem)
            .aria_label(self.name.clone())
            .flex()
            .items_center()
            .gap(px(SPACE_3))
            .px(px(SPACE_4))
            .py(px(SPACE_3))
            .child(plate)
            .child(text)
            .child(status)
            .child(switch)
            .child(more)
    }
}

/// The byline under the name: publisher, version and what it adds, the parts that are known, joined by a middle dot.
fn byline_text(publisher: &str, version: &str, contribution: &str) -> String {
    [publisher, version, contribution]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The status column: the word with its glyph, a badge for an update or a disabled row, and for a failed start
/// the word with its View log button.
fn status_content(
    state: &ExtensionState,
    word: SharedString,
    p: &Palette,
    key: &SharedString,
    handler: &Option<ActionHandler>,
) -> AnyElement {
    match state {
        ExtensionState::Installed {
            update: Some(_), ..
        } => Badge::text(word).tone(BadgeTone::Info).into_any_element(),
        ExtensionState::Installed { enabled: true, .. } => {
            status_word(IconName::CircleCheck, p.status.success_fg, &word)
        }
        ExtensionState::Installed { enabled: false, .. } => Badge::text(word)
            .tone(BadgeTone::Neutral)
            .into_any_element(),
        ExtensionState::Failed { .. } => div()
            .flex()
            .items_center()
            .gap(px(SPACE_2))
            .min_w_0()
            .child(status_word(
                IconName::OctagonAlert,
                p.status.danger_fg,
                &word,
            ))
            .child(
                Button::new(SharedString::from(format!("{key}-log")), "View log")
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Compact)
                    .on_click(route(handler, ExtensionAction::ViewLog)),
            )
            .into_any_element(),
        ExtensionState::Catalog
        | ExtensionState::Installing { .. }
        | ExtensionState::Incompatible { .. } => div()
            .min_w_0()
            .truncate()
            .text_style(TYPE_BODY_SM)
            .text_color(p.text.secondary)
            .child(word)
            .into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct RowHarness {
        state: ExtensionState,
        seen: Rc<RefCell<Vec<ExtensionAction>>>,
    }

    impl gpui::Render for RowHarness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let seen = self.seen.clone();
            ExtensionManagerRow::new("row-1", "Quick notes", self.state.clone())
                .publisher("MassOrca extras")
                .version("0.4.1")
                .contribution("Project sidebar tab")
                .status_word("Enabled")
                .on_action(move |action, _, _| seen.borrow_mut().push(action))
        }
    }

    fn drawn(
        cx: &mut gpui::TestAppContext,
        state: ExtensionState,
    ) -> (
        Rc<RefCell<Vec<ExtensionAction>>>,
        &mut gpui::VisualTestContext,
    ) {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let (_, cx) = cx.add_window_view(|_, _| RowHarness {
            state,
            seen: seen.clone(),
        });
        cx.run_until_parked();
        (seen, cx)
    }

    #[gpui::test]
    fn the_name_the_switch_and_more_send_their_actions(cx: &mut gpui::TestAppContext) {
        let (seen, cx) = drawn(
            cx,
            ExtensionState::Installed {
                enabled: true,
                update: None,
            },
        );
        let name = cx
            .debug_bounds("row-name:row-1")
            .expect("the name is drawn");
        cx.simulate_click(name.center(), gpui::Modifiers::none());
        let switch = cx
            .debug_bounds("row-switch:row-1")
            .expect("the switch is drawn");
        cx.simulate_click(switch.center(), gpui::Modifiers::none());
        let more = cx.debug_bounds("button:row-1-more").expect("More is drawn");
        cx.simulate_click(more.center(), gpui::Modifiers::none());
        assert_eq!(
            *seen.borrow(),
            vec![
                ExtensionAction::Open,
                ExtensionAction::Toggle(false),
                ExtensionAction::More
            ]
        );
    }

    #[gpui::test]
    fn a_failed_row_offers_view_log_and_its_switch_stays_usable(cx: &mut gpui::TestAppContext) {
        let (seen, cx) = drawn(
            cx,
            ExtensionState::Failed {
                reason: "The CLI did not start".into(),
            },
        );
        let log = cx
            .debug_bounds("button:row-1-log")
            .expect("View log is drawn");
        cx.simulate_click(log.center(), gpui::Modifiers::none());
        let switch = cx
            .debug_bounds("row-switch:row-1")
            .expect("the switch is drawn");
        cx.simulate_click(switch.center(), gpui::Modifiers::none());
        assert_eq!(
            *seen.borrow(),
            vec![ExtensionAction::ViewLog, ExtensionAction::Toggle(true)]
        );
    }

    #[gpui::test]
    fn an_installing_row_has_a_switch_that_sends_nothing(cx: &mut gpui::TestAppContext) {
        let (seen, cx) = drawn(cx, ExtensionState::Installing { percent: 40 });
        let switch = cx
            .debug_bounds("row-switch:row-1")
            .expect("the switch is drawn");
        cx.simulate_click(switch.center(), gpui::Modifiers::none());
        assert!(seen.borrow().is_empty(), "a switch mid-install is disabled");
    }

    #[gpui::test]
    fn an_update_is_named_in_the_status_and_the_row_has_no_update_button(
        cx: &mut gpui::TestAppContext,
    ) {
        let (_, cx) = drawn(
            cx,
            ExtensionState::Installed {
                enabled: true,
                update: Some("2.1.0".into()),
            },
        );
        assert!(
            cx.debug_bounds("row-switch:row-1").is_some(),
            "the switch stays with an update waiting"
        );
        assert!(
            cx.debug_bounds("button:row-1-update").is_none(),
            "the update is named in the status; Discover and Updates keep the Update button"
        );
    }
}
