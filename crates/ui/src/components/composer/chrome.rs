//! The composer's chrome: the attachment chips, the access and attach menus, the primary and
//! access buttons, the status lines, and the layout that holds them (Composer.md, the session
//! body: placeholder, bottom row, primary button, status lines).

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, AppContext, ClickEvent, Context, ExternalPaths, InteractiveElement,
    IntoElement, ParentElement, Render, Role, SharedString, StatefulInteractiveElement, Styled,
    Window, div, px,
};

use super::policy::{PrimaryAction, primary_action, truncate_name, visible_attachments};
use super::{AccessMode, Attachment, Composer, ComposerEvent, KEY_CONTEXT, OpenModelPicker};
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::icon_button::IconButton;
use crate::components::menu::{MenuEntry, MenuEvent, MenuItem, MenuOrigin, MenuView, menu_overlay};
use crate::components::tooltip::{Tooltip, TooltipTrigger, tooltip_view};
use crate::icon::{Icon, IconName, IconSize};
use crate::style::FocusRing;
use crate::theme::{ActiveTheme, TextStyled, box_shadows};
use crate::tokens::{
    LAYOUT_CONVERSATION_READING, RADIUS_FULL, RADIUS_XL, SIZE_CONTROL_COMPACT,
    SIZE_CONTROL_DEFAULT, SPACE_1, SPACE_2, SPACE_3, SPACE_4, TYPE_BODY_SM, TYPE_CAPTION,
    TYPE_CHAT, TYPE_LABEL,
};

const CHIP_NAME_MAX: usize = 24;

impl Composer {
    /// The agent's name, used for the accessible label of the text ("Message to Claude").
    pub fn agent_name(mut self, name: impl Into<SharedString>) -> Self {
        self.agent_name = name.into();
        self
    }

    /// Whether the microphone button is offered.
    pub fn voice(mut self, voice: bool) -> Self {
        self.voice = voice;
        self
    }

    /// Whether the attach button, its menu and dropped files are offered.
    pub fn attach(mut self, offered: bool) -> Self {
        self.attach_offered = offered;
        self
    }

    /// Whether the model picker chip is offered.
    pub fn model_picker(mut self, offered: bool) -> Self {
        self.model_picker_offered = offered;
        self
    }

    /// Whether a message sent while the agent runs queues behind its turn; otherwise the button stops the run.
    pub fn queueing(mut self, offered: bool) -> Self {
        self.queueing_offered = offered;
        self
    }

    /// A control shown in the bottom row after the access chip, before the flexible gap; `build`
    /// runs on each render.
    pub fn leading_control(
        mut self,
        build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.leading = Some(Box::new(build));
        self
    }

    /// Why sending is not possible yet: the primary button stays disabled with this as its
    /// tooltip, Enter does not send, and the draft stays in the composer.
    pub fn send_unavailable(mut self, reason: impl Into<SharedString>) -> Self {
        self.send_unavailable = Some(reason.into());
        self
    }

    /// Why voice input is not possible yet: the microphone stays disabled with this as its reason.
    pub fn voice_unavailable(mut self, reason: impl Into<SharedString>) -> Self {
        self.voice_unavailable = Some(reason.into());
        self
    }

    /// The reason voice input is unavailable, or `None` when the microphone works.
    pub fn voice_unavailable_reason(&self) -> Option<&SharedString> {
        self.voice_unavailable.as_ref()
    }

    pub fn set_attachments(&mut self, attachments: Vec<Attachment>, cx: &mut Context<Self>) {
        self.attachments = attachments;
        cx.notify();
    }

    /// Replaces the access modes and selects `selected`, or the first mode when it is out of range. An empty list
    /// hides the chip.
    pub fn set_access_modes(
        &mut self,
        modes: Vec<AccessMode>,
        selected: usize,
        cx: &mut Context<Self>,
    ) {
        self.access_index = if selected < modes.len() { selected } else { 0 };
        if self.access != modes {
            self.access = modes;
            self.access_details.clear();
            self.access_menu = None;
        }
        cx.notify();
    }

    /// The access modes the chip offers.
    pub fn access_modes(&self) -> &[AccessMode] {
        &self.access
    }

    /// Sets each access mode's secondary text by index (`None` for none): the second line of its menu row and,
    /// while it is selected, the chip's tooltip. Call it after `set_access_modes`, which clears it when the modes change.
    pub fn set_access_details(
        &mut self,
        details: Vec<Option<SharedString>>,
        cx: &mut Context<Self>,
    ) {
        if self.access_details != details {
            self.access_details = details;
            self.access_menu = None;
            cx.notify();
        }
    }

    /// The secondary text of access mode `index`, if it has one.
    pub fn access_detail(&self, index: usize) -> Option<&SharedString> {
        self.access_details.get(index).and_then(Option::as_ref)
    }

    /// The selected access mode's index.
    pub fn access_index(&self) -> usize {
        self.access_index
    }

    /// Sets the caption after the access chip.
    pub fn set_access_note(&mut self, note: Option<SharedString>, cx: &mut Context<Self>) {
        if self.access_note != note {
            self.access_note = note;
            cx.notify();
        }
    }

    /// Selects an access mode by index; out-of-range indexes are ignored.
    pub fn set_access(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.access.len() {
            self.access_index = index;
            cx.notify();
        }
    }

    /// The reason a send failed; the draft stays in the composer.
    pub fn set_error(&mut self, reason: Option<SharedString>, cx: &mut Context<Self>) {
        self.error = reason;
        cx.notify();
    }

    fn primary_clicked(&mut self, action: PrimaryAction, cx: &mut Context<Self>) {
        match action {
            PrimaryAction::Send => self.submit(false, cx),
            PrimaryAction::Queue => self.submit(true, cx),
            PrimaryAction::Stop => self.stop(cx),
            PrimaryAction::Disabled => {}
        }
    }

    fn open_access_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let entries: Vec<MenuEntry> = self
            .access
            .iter()
            .enumerate()
            .map(|(index, mode)| {
                let item = MenuItem::new(mode.name.clone())
                    .icon(mode.icon)
                    .checked(index == self.access_index);
                match self.access_detail(index) {
                    Some(detail) => item.secondary(detail.clone()).into(),
                    None => item.into(),
                }
            })
            .collect();
        let menu = cx.new(|cx| MenuView::new(entries, window, cx));
        let events = cx.subscribe(&menu, |this, _, event: &MenuEvent, cx| {
            if let MenuEvent::Chosen(choice) = event {
                this.choose_access(choice.row, cx);
            }
            this.access_menu = None;
            cx.notify();
        });
        self.access_menu = Some((menu, events));
        cx.notify();
    }

    fn choose_access(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.access.len() {
            self.access_index = index;
            cx.emit(ComposerEvent::AccessChanged(index));
        }
    }

    fn open_attach_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let entries: Vec<MenuEntry> = vec![
            MenuItem::new("Attach files")
                .icon(IconName::Paperclip)
                .into(),
            MenuItem::new("Add from clipboard")
                .icon(IconName::Clipboard)
                .into(),
        ];
        let menu = cx.new(|cx| MenuView::new(entries, window, cx));
        let events = cx.subscribe(&menu, |this, _, event: &MenuEvent, cx| {
            if let MenuEvent::Chosen(choice) = event {
                cx.emit(match choice.row {
                    0 => ComposerEvent::AttachFiles,
                    _ => ComposerEvent::PasteFromClipboard,
                });
            }
            this.attach_menu = None;
            cx.notify();
        });
        self.attach_menu = Some((menu, events));
        cx.notify();
    }

    fn attachment_chip(
        &self,
        attachment: &Attachment,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let p = cx.theme().palette;
        let id = attachment.id;
        let label = truncate_name(&attachment.name, CHIP_NAME_MAX);
        let composer = cx.entity();
        let chip_group: SharedString = format!("composer-chip-{id}").into();
        let remove_focus = window
            .use_keyed_state(("composer-chip-focus", id as usize), cx, |_, cx| {
                cx.focus_handle()
            })
            .read(cx)
            .clone();
        let remove_focused = remove_focus.is_focused(window);
        div()
            .group(chip_group.clone())
            .flex()
            .flex_none()
            .items_center()
            .gap(px(SPACE_1))
            .h(px(SIZE_CONTROL_COMPACT))
            .pl(px(SPACE_2))
            .pr(px(SPACE_1))
            .rounded(px(RADIUS_FULL))
            .bg(p.surface.sunken)
            .border_1()
            .border_color(p.border.subtle)
            .text_style(TYPE_LABEL)
            .text_color(p.text.primary)
            .child(
                Icon::new(IconName::File)
                    .size(IconSize::Small)
                    .color(p.text.secondary),
            )
            .child(div().min_w_0().child(label))
            // The remove x shows on hover and focus only (Composer.md Anatomy, chip row); at rest
            // it is transparent, so it keeps its place and stays reachable with Tab.
            .child(
                div()
                    .debug_selector(move || format!("chip-remove-{id}"))
                    .when(!remove_focused, |slot| {
                        slot.opacity(0.)
                            .group_hover(chip_group.clone(), |slot| slot.opacity(1.))
                    })
                    .child(
                        IconButton::new(
                            ("composer-chip-remove", id as usize),
                            IconName::X,
                            "Remove attachment",
                        )
                        .size(ButtonSize::Compact)
                        .focus_handle(remove_focus)
                        .on_click(move |_, cx| {
                            composer
                                .update(cx, |_, cx| cx.emit(ComposerEvent::RemoveAttachment(id)));
                        }),
                    ),
            )
    }

    /// The chip row: attachments wrap; past four, three and a "+N" chip.
    fn chips(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<gpui::Div> {
        if self.attachments.is_empty() {
            return None;
        }
        let (shown, hidden) = visible_attachments(self.attachments.len());
        let mut row = div().flex().flex_wrap().gap(px(SPACE_1));
        for attachment in self.attachments.iter().take(shown) {
            row = row.child(self.attachment_chip(attachment, window, cx));
        }
        if hidden > 0 {
            let p = cx.theme().palette;
            row = row.child(
                div()
                    .flex()
                    .items_center()
                    .h(px(SIZE_CONTROL_COMPACT))
                    .px(px(SPACE_2))
                    .text_style(TYPE_LABEL)
                    .text_color(p.text.secondary)
                    .child(format!("+{hidden}")),
            );
        }
        Some(row)
    }

    /// The circle at the end of the bottom row: arrow-up to send, with a clock to queue, a
    /// square to stop, and a disabled grey circle when there is nothing to send.
    fn primary_button(
        &self,
        action: PrimaryAction,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let theme = *cx.theme();
        let p = theme.palette;
        let unavailable = self.send_unavailable.clone().or_else(|| {
            self.no_agents_installed(cx)
                .then(|| SharedString::from("No agents installed"))
        });
        let (icon, label, enabled, fill, ink) = match action {
            PrimaryAction::Send => (
                IconName::ArrowUp,
                "Send message",
                true,
                p.accent.default,
                p.text.on_accent,
            ),
            PrimaryAction::Queue => (
                IconName::ArrowUp,
                "Queue message",
                true,
                p.accent.default,
                p.text.on_accent,
            ),
            PrimaryAction::Stop => (
                IconName::Square,
                "Stop",
                true,
                p.surface.raised,
                p.text.primary,
            ),
            PrimaryAction::Disabled => (
                IconName::ArrowUp,
                "Send message",
                false,
                p.fill.disabled,
                p.text.disabled,
            ),
        };
        let queueing = action == PrimaryAction::Queue;
        div()
            .id("composer-primary")
            .debug_selector(|| "composer-primary".to_owned())
            .role(Role::Button)
            .aria_label(label)
            .track_focus(&self.primary_focus)
            .focus_ring(&theme)
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(SIZE_CONTROL_DEFAULT))
            .rounded(px(RADIUS_FULL))
            .bg(fill)
            .text_color(ink)
            .when_some(
                unavailable.filter(|_| action == PrimaryAction::Disabled),
                |button, reason| button.tooltip(tooltip_view(Tooltip::new(reason))),
            )
            .when(action == PrimaryAction::Stop, |button| {
                button.border_1().border_color(p.border.strong)
            })
            .when(enabled, |button| {
                button
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.primary_clicked(action, cx);
                    }))
            })
            .child(Icon::new(icon).size(IconSize::Small).color(ink))
            .when(queueing, |button| {
                button.child(
                    div()
                        .absolute()
                        .top(px(-SPACE_1))
                        .right(px(-SPACE_1))
                        .child(
                            Icon::new(IconName::Clock)
                                .size(IconSize::XSmall)
                                .color(p.text.secondary),
                        ),
                )
            })
    }

    /// The access chip: shield, the mode's name and a chevron; it opens the access menu. A bypass mode shows a
    /// warning glyph in place of the shield.
    fn access_chip(&self, cx: &mut Context<Self>) -> gpui::Div {
        let theme = *cx.theme();
        let p = theme.palette;
        let selected = self.access.get(self.access_index);
        let name = selected.map(|mode| mode.name.clone()).unwrap_or_default();
        let (glyph, glyph_color) = if selected.is_some_and(|mode| mode.bypass) {
            (IconName::TriangleAlert, p.status.warning_fg)
        } else {
            (IconName::Shield, p.text.secondary)
        };
        let detail = self.access_detail(self.access_index).cloned();
        let composer = cx.entity();
        let chip = div()
            .id("composer-access")
            .debug_selector(|| "composer-access".to_owned())
            .role(Role::Button)
            .aria_label(format!("Access: {name}"))
            .when_some(detail.clone(), |chip, detail| chip.aria_description(detail))
            .track_focus(&self.access_focus)
            .focus_ring(&theme)
            .flex()
            .flex_none()
            .items_center()
            .gap(px(SPACE_1))
            .h(px(SIZE_CONTROL_DEFAULT))
            .px(px(SPACE_2))
            .rounded(px(RADIUS_FULL))
            .text_style(TYPE_LABEL)
            .text_color(p.text.primary)
            .cursor_pointer()
            .hover(move |s| s.bg(p.fill.hover_raised))
            .when(self.access_menu.is_some(), |chip| {
                chip.bg(p.fill.selected_raised)
            })
            .on_click(move |_, window, cx| {
                composer.update(cx, |this, cx| this.open_access_menu(window, cx));
            })
            .child(Icon::new(glyph).size(IconSize::Small).color(glyph_color))
            .child(name)
            .child(
                Icon::new(IconName::ChevronDown)
                    .size(IconSize::XSmall)
                    .color(p.text.tertiary),
            );
        // The tooltip shows on hover and, through the focus handle, on keyboard focus.
        let chip = match detail {
            Some(text) => TooltipTrigger::new("composer-access-tip", text, chip)
                .focus_handle(self.access_focus.clone())
                .into_any_element(),
            None => chip.into_any_element(),
        };
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(SPACE_2))
            .child(self.access_trigger.track(div().child(chip)))
            .when_some(self.access_note.clone(), |row, note| {
                row.child(
                    div()
                        .debug_selector(|| "composer-access-note".to_owned())
                        .text_style(TYPE_CAPTION)
                        .text_color(p.text.secondary)
                        .child(note),
                )
            })
    }

    /// Status lines above the bottom row: the disabled reason, host state, a failed send with
    /// Retry, and the escape hint.
    fn status_lines(&self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let p = cx.theme().palette;
        let mut lines = Vec::new();
        if let Some(reason) = &self.disabled_reason {
            let mut line = div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(SPACE_2))
                .text_style(TYPE_BODY_SM)
                .text_color(p.text.tertiary)
                .child(reason.clone());
            let action = if self.sign_in {
                Some(("composer-sign-in", "Sign in", ComposerEvent::SignIn))
            } else if self.resume {
                Some(("composer-resume", "Resume", ComposerEvent::Resume))
            } else {
                None
            };
            if let Some((id, label, event)) = action {
                let composer = cx.entity();
                line = line.child(
                    div().debug_selector(move || id.into()).child(
                        Button::new(id, label)
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Compact)
                            .on_click(move |_, cx| {
                                composer.update(cx, |_, cx| cx.emit(event.clone()));
                            }),
                    ),
                );
            }
            lines.push(line.into_any_element());
        }
        if !self.host_confirmed {
            lines.push(
                div()
                    .flex()
                    .items_center()
                    .gap(px(SPACE_1))
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.text.tertiary)
                    .child(
                        Icon::new(IconName::CloudOff)
                            .size(IconSize::XSmall)
                            .color(p.agent.unreachable),
                    )
                    .child("VPS can't be reached. Your message stays in the composer.")
                    .into_any_element(),
            );
        }
        lines
    }

    /// The send-failed line, drawn above the text (Composer.md States, error).
    fn error_line(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let p = cx.theme().palette;
        self.error.as_ref().map(|reason| {
            let composer = cx.entity();
            div()
                .debug_selector(|| "composer-error".into())
                .flex()
                .items_center()
                .justify_between()
                .gap(px(SPACE_2))
                .text_style(TYPE_BODY_SM)
                .text_color(p.status.danger_fg)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(SPACE_1))
                        .child(
                            Icon::new(IconName::OctagonAlert)
                                .size(IconSize::XSmall)
                                .color(p.status.danger_fg),
                        )
                        .child(format!("Message not sent: {reason}")),
                )
                .child(
                    Button::new("composer-retry", "Retry")
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Compact)
                        .on_click(move |_, cx| {
                            composer.update(cx, |_, cx| cx.emit(ComposerEvent::RetrySend));
                        }),
                )
                .into_any_element()
        })
    }

    /// The hint under the text while Escape is armed.
    fn hint_line(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let p = cx.theme().palette;
        self.hint_shown.then(|| {
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(p.text.secondary)
                .child("Press Escape again to stop")
                .into_any_element()
        })
    }
}

impl Render for Composer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let situation = self.situation(cx);
        let action = primary_action(situation);
        let composer = cx.entity();
        let attach_entity = composer.clone();
        let attach = self.attach_offered.then(|| {
            let button = IconButton::new("composer-attach", IconName::Plus, "Attach")
                .size(ButtonSize::Default)
                .on_click(move |window, cx| {
                    attach_entity.update(cx, |this, cx| this.open_attach_menu(window, cx));
                });
            self.attach_trigger.track(
                div()
                    .debug_selector(|| "composer-attach".to_owned())
                    .child(button),
            )
        });
        let leading = self.leading.as_ref().map(|build| build(window, cx));
        // No modes means the host declares none: no chip rather than an empty menu.
        let access = (!self.access.is_empty()).then(|| self.access_chip(cx));
        let mut bottom = div()
            .flex()
            .items_center()
            .gap(px(SPACE_2))
            .h(px(SIZE_CONTROL_DEFAULT))
            .when_some(attach, |row, attach| row.child(attach))
            .when_some(access, |row, chip| row.child(chip))
            .when_some(leading, |row, leading| row.child(leading))
            .child(div().flex_1())
            .when(self.model_picker_offered, |row| {
                row.child(self.picker.clone())
            });
        if self.voice {
            let dictate = move |_: &mut Window, cx: &mut App| {
                composer.update(cx, |_, cx| cx.emit(ComposerEvent::Dictate));
            };
            bottom = bottom.child(microphone(self.voice_unavailable.clone(), dictate));
        }
        bottom = bottom.child(self.primary_button(action, cx));

        let mut column = div()
            .debug_selector(|| "composer-column".to_owned())
            .w_full()
            .max_w(px(LAYOUT_CONVERSATION_READING))
            .flex()
            .flex_col()
            .gap(px(SPACE_2))
            .p(px(SPACE_3))
            .rounded(px(RADIUS_XL))
            .bg(p.surface.raised)
            .border_1()
            .border_color(p.border.default)
            .shadow(box_shadows(theme.shadows.e1))
            .text_style(TYPE_CHAT)
            .text_color(p.text.primary)
            .text_color(p.text.primary);
        if self.attach_offered {
            column = column
                .drag_over::<ExternalPaths>(move |style, _, _, _| style.border_color(p.accent.fg))
                .on_drop(cx.listener(|_, paths: &ExternalPaths, _, cx| {
                    cx.emit(ComposerEvent::DroppedFiles(paths.0.to_vec()));
                }));
        }
        if let Some(chips) = self.chips(window, cx) {
            column = column.child(chips);
        }
        if let Some(error) = self.error_line(cx) {
            column = column.child(error);
        }
        column = column.child(self.text.clone());
        for line in self.status_lines(cx) {
            column = column.child(line);
        }
        if let Some(hint) = self.hint_line(cx) {
            column = column.child(hint);
        }
        column = column.child(bottom);

        let menus = [
            self.access_menu.as_ref().and_then(|(menu, _)| {
                self.access_trigger
                    .get()
                    .map(|bounds| (menu.clone(), bounds))
            }),
            self.attach_menu.as_ref().and_then(|(menu, _)| {
                self.attach_trigger
                    .get()
                    .map(|bounds| (menu.clone(), bounds))
            }),
        ];
        let mut root = div()
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(|this, _: &OpenModelPicker, window, cx| {
                if this.model_picker_offered {
                    this.picker.update(cx, |picker, cx| picker.open(window, cx));
                }
            }))
            .w_full()
            .flex()
            .justify_center()
            .when(!self.joined_top, |root| root.px(px(SPACE_4)))
            .child(column);
        for (menu, bounds) in menus.into_iter().flatten() {
            root = root.child(menu_overlay(&menu, MenuOrigin::Below(bounds), cx));
        }
        root
    }
}

/// The composer's top corner radius: square under a placement card that joins its top edge.
/// The microphone of the bottom row: disabled with its reason while voice input is not there yet.
fn microphone(
    reason: Option<SharedString>,
    on_dictate: impl Fn(&mut Window, &mut App) + 'static,
) -> IconButton {
    let mic =
        IconButton::new("composer-dictate", IconName::Mic, "Dictate").size(ButtonSize::Default);
    match reason {
        Some(reason) => mic.disabled(true).disabled_reason(reason),
        None => mic.on_click(on_dictate),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a11y::probe::{reported, root};
    use gpui::{RenderOnce, TestAppContext};

    #[gpui::test]
    fn a_microphone_without_voice_input_is_disabled_and_gives_its_reason(cx: &mut TestAppContext) {
        let node = root(cx, |window, cx| {
            let mic = microphone(Some("Voice input comes later.".into()), |_, _| {});
            reported(mic.render(window, cx))
        })
        .expect("reported");
        assert!(node.is_disabled());
        assert_eq!(node.description(), Some("Voice input comes later."));
    }
}
