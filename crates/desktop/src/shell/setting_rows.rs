//! The Settings rows backed by one core setting (SET-01). Each control shows the value the store holds, writes a
//! change through `config.set` and offers a reset through `config.reset` while the value differs from its default.
//! The control follows only what the store reports back; a refusal shows the service's sentence on the row.

use gpui::prelude::*;
use gpui::{AnyElement, Context, SharedString, WeakEntity, div, px};
use massorca_protocol::methods::config::{ConfigKey, ConfigSet, ConfigValue, DEFAULT_OWNER};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::checkbox::Checkbox;
use massorca_ui::components::icon_button::{IconButton, IconButtonVariant};
use massorca_ui::components::segmented_control::{Segment, SegmentedControl};
use massorca_ui::components::settings_row::{self, RowStatus, SettingsRow};
use massorca_ui::components::switch::Switch;
use massorca_ui::tokens::{SPACE_1, SPACE_2, SPACE_3, TYPE_CAPTION};
use massorca_ui::{IconName, TextStyled, Theme};
use serde_json::Value;

use super::Shell;
use super::settings::value_box;
use crate::model::setting_rows::{
    Live, checked, choice_index, number, stepped, switch_on, with_option,
};
use crate::model::settings::{LOADING, NOT_CONNECTED};

/// The width of a sound dropdown beside a notification switch.
const SOUND_SELECT_WIDTH: f32 = 120.0;

/// What a live row needs from the page: the store's values, the connection and the last refusal.
pub(super) struct LiveFrame<'a> {
    pub config: &'a [ConfigValue],
    pub connected: bool,
    /// The last refused write, as `(key, sentence)`.
    pub notice: Option<&'a (String, String)>,
    pub shell: &'a WeakEntity<Shell>,
}

/// Adds the state of a live setting to its row and returns its control: off with the reason until the service is
/// connected and has listed the setting, with a reset while the value is not the default.
pub(super) fn live_row(
    row: SettingsRow,
    live: Live,
    id: &SharedString,
    label: &'static str,
    frame: &LiveFrame<'_>,
    theme: &Theme,
) -> (SettingsRow, Option<AnyElement>) {
    let key = live.key();
    let held = frame
        .config
        .iter()
        .find(|value| value.owner == DEFAULT_OWNER && value.key == key);
    let mut row = row;
    let reason = match (frame.connected, held) {
        (false, _) => Some(NOT_CONNECTED),
        (true, None) => Some(LOADING),
        (true, Some(_)) => None,
    };
    if let Some(reason) = reason {
        row = row.disabled_reason(reason);
    }
    if let Some((_, sentence)) = frame.notice.filter(|(refused, _)| refused == key) {
        row = row.status(RowStatus::Error(sentence.clone().into()));
    }
    if held.is_some_and(|value| !value.is_default) && reason.is_none() {
        let shell = frame.shell.clone();
        row = row.reset(move |_, cx| {
            let _ = shell.update(cx, |shell, cx| shell.reset_setting(key, cx));
        });
    }
    let enabled = reason.is_none();
    let control = match live {
        Live::Switch(_) => switch(id, label, key, held, enabled, frame.shell),
        Live::SwitchSound(_) => {
            let sound = settings_row::unavailable_select(
                SharedString::from(format!("{id}-sound")),
                SharedString::from(format!("Sound for {label}")),
                theme,
                None,
            );
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap(px(SPACE_3))
                .w_full()
                .child(switch(id, label, key, held, enabled, frame.shell))
                .child(div().w(px(SOUND_SELECT_WIDTH)).child(sound))
                .into_any_element()
        }
        Live::Choice {
            options, action, ..
        } => {
            let choice = choice(id, label, key, options, held, enabled, frame.shell);
            match action {
                Some(action) => div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(px(SPACE_2))
                    .w_full()
                    .child(choice)
                    .child(
                        Button::new(SharedString::from(format!("{id}-action")), action)
                            .variant(ButtonVariant::Ghost)
                            .disabled(true),
                    )
                    .into_any_element(),
                None => choice,
            }
        }
        Live::Number { unit, step, .. } => Stepper {
            id,
            label,
            key,
            step,
            unit,
            enabled,
        }
        .element(held, frame.shell, theme),
        Live::Checkboxes { options, .. } => {
            row = row.stacked(true);
            checkboxes(id, key, options, held, enabled, frame.shell)
        }
    };
    (row, Some(control))
}

fn switch(
    id: &SharedString,
    label: &'static str,
    key: &'static str,
    held: Option<&ConfigValue>,
    enabled: bool,
    shell: &WeakEntity<Shell>,
) -> AnyElement {
    let shell = shell.clone();
    let switch = Switch::new(id.clone(), held.and_then(switch_on).unwrap_or(false))
        .label(label)
        .disabled(!enabled)
        .on_change(move |on, _, cx| {
            let _ = shell.update(cx, |shell, cx| {
                shell.write_setting(key, Value::from(on), cx)
            });
        });
    found(format!("{id}-switch"), switch)
}

/// The control inside a box a test finds by `selector`.
fn found(selector: String, control: impl IntoElement) -> AnyElement {
    div()
        .debug_selector(move || selector)
        .child(control)
        .into_any_element()
}

fn choice(
    id: &SharedString,
    label: &'static str,
    key: &'static str,
    options: &'static [(&'static str, &'static str)],
    held: Option<&ConfigValue>,
    enabled: bool,
    shell: &WeakEntity<Shell>,
) -> AnyElement {
    let shell = shell.clone();
    SegmentedControl::new(
        id.clone(),
        label,
        options
            .iter()
            .map(|(_, text)| Segment::text(*text))
            .collect(),
    )
    .selected(
        held.and_then(|value| choice_index(options, value))
            .unwrap_or(0),
    )
    .disabled(!enabled)
    .on_change(move |index, _, cx| {
        if let Some((word, _)) = options.get(index) {
            let _ = shell.update(cx, |shell, cx| {
                shell.write_setting(key, Value::from(*word), cx);
            });
        }
    })
    .into_any_element()
}

/// One checkbox per option; a press writes the whole set with that option turned on or off.
fn checkboxes(
    id: &SharedString,
    key: &'static str,
    options: &'static [(&'static str, &'static str)],
    held: Option<&ConfigValue>,
    enabled: bool,
    shell: &WeakEntity<Shell>,
) -> AnyElement {
    let marks = held
        .map(|value| checked(options, value))
        .unwrap_or_default();
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .w_full()
        .children(options.iter().enumerate().map(|(index, (_, text))| {
            let shell = shell.clone();
            let value = held.cloned();
            let mark = SharedString::from(format!("{id}-{index}"));
            let checkbox = Checkbox::new(mark.clone(), marks.get(index).copied().unwrap_or(false))
                .label(*text)
                .disabled(!enabled)
                .on_change(move |on, _, cx| {
                    if let Some(value) = &value {
                        let next = with_option(options, value, index, on);
                        let _ = shell.update(cx, |shell, cx| shell.write_setting(key, next, cx));
                    }
                });
            found(mark.to_string(), checkbox)
        }))
        .into_any_element()
}

/// A number setting: its value with a decrease and an increase button, and its unit.
struct Stepper<'a> {
    id: &'a SharedString,
    label: &'static str,
    key: &'static str,
    step: i64,
    unit: &'static str,
    enabled: bool,
}

impl Stepper<'_> {
    /// A button is off at its end of the range, and both are off while the row is.
    fn element(
        self,
        held: Option<&ConfigValue>,
        shell: &WeakEntity<Shell>,
        theme: &Theme,
    ) -> AnyElement {
        let Self {
            id,
            label,
            key,
            step,
            unit,
            enabled,
        } = self;
        let button = |up: bool| {
            let next = held.and_then(|value| stepped(value, step, up));
            let shell = shell.clone();
            let (icon, direction, word) = if up {
                (IconName::Plus, "Increase", "increase")
            } else {
                (IconName::Minus, "Decrease", "decrease")
            };
            IconButton::new(
                SharedString::from(format!("{id}-{word}")),
                icon,
                format!("{direction} {label}"),
            )
            .variant(IconButtonVariant::Ghost)
            .disabled(!enabled || next.is_none())
            .on_click(move |_, cx| {
                if let Some(next) = next {
                    let _ = shell.update(cx, |shell, cx| {
                        shell.write_setting(key, Value::from(next), cx);
                    });
                }
            })
        };
        let text = held
            .and_then(number)
            .map(|value| value.to_string())
            .unwrap_or_default();
        let selector = format!("{id}-value");
        div()
            .flex()
            .items_center()
            .gap(px(SPACE_1))
            .child(button(false))
            .child(
                div()
                    .debug_selector(move || selector)
                    .child(value_box(text, theme)),
            )
            .child(button(true))
            .child(
                div()
                    .text_style(TYPE_CAPTION)
                    .text_color(theme.palette.text.tertiary)
                    .child(unit),
            )
            .into_any_element()
    }
}

impl Shell {
    /// Writes `value` for the core setting `key`. The store takes the service's answer; a refusal shows on the row.
    pub(super) fn write_setting(
        &mut self,
        key: &'static str,
        value: Value,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.service.clone() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let result = service
                .config_set(ConfigSet {
                    owner: None,
                    key: key.to_owned(),
                    value,
                })
                .await;
            let _ = this.update(cx, |shell, cx| {
                shell.follow_setting_answer(key, result.err().map(|refusal| refusal.sentence()));
                cx.notify();
            });
        })
        .detach();
    }

    /// Puts the core setting `key` back to its default through `config.reset`.
    pub(super) fn reset_setting(&mut self, key: &'static str, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let result = service
                .config_reset(ConfigKey {
                    owner: None,
                    key: key.to_owned(),
                })
                .await;
            let _ = this.update(cx, |shell, cx| {
                shell.follow_setting_answer(key, result.err().map(|refusal| refusal.sentence()));
                cx.notify();
            });
        })
        .detach();
    }

    /// Keeps the refusal of the last write to `key`, or clears it once a write to `key` is stored.
    fn follow_setting_answer(&mut self, key: &'static str, refusal: Option<String>) {
        match refusal {
            Some(sentence) => self.setting_notice = Some((key.to_owned(), sentence)),
            None => {
                if self
                    .setting_notice
                    .as_ref()
                    .is_some_and(|(held, _)| held == key)
                {
                    self.setting_notice = None;
                }
            }
        }
    }
}
