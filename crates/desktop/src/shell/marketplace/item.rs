//! One item's page: its nine parts in the owner's order (teams-extensions-settings.md 7.2, decision
//! 0059). Parts 1 to 3 are the header; the rest follow it in the order the model gives.

use super::MarketplaceFrame;
use super::catalogue::{item_mark, kind_icon};
use super::host_pick::host_picker_row;
use super::item_images;
use crate::model::home_start::HOME_START;
use crate::model::marketplace::{
    AccountActivity, AccountState, AccountView, ItemState, MarketplaceItem, MarketplaceItemId,
    PARTS, Part, ProblemAlert, ProblemTone, absent_reason, account_block, advanced_rows,
    banner_palette_slot, banner_tag, connection_rows, information, information_links,
    problem_alert, settings_rows, state_chip, uninstall_body,
};
use crate::model::settings_sections::NOT_AVAILABLE;
use crate::shell::Shell;
use gpui::{
    AnyElement, App, ClipboardItem, Context, ElementId, ImageSource, IntoElement, ObjectFit,
    ParentElement, Role, SharedString, Styled, StyledImage, WeakEntity, Window, div, img,
    prelude::*, px,
};
use massorca_ui::components::account_connection_list::{
    AccountConnectionList, AccountRow, AccountStatus,
};
use massorca_ui::components::badge::{Badge, BadgeTone};
use massorca_ui::components::banner::{Banner, BannerVariant};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::danger_zone::DangerZone;
use massorca_ui::components::empty_state::EmptyState;
use massorca_ui::components::invocation_banner::{BannerFill, InvocationBanner, hex_colour};
use massorca_ui::components::marketplace_item_header::MarketplaceItemHeader;
use massorca_ui::components::switch::Switch;
use massorca_ui::tokens::{
    Color, Palette, RADIUS_LG, SPACE_1, SPACE_2, SPACE_3, SPACE_4, SPACE_6, TYPE_BODY,
    TYPE_BODY_SM, TYPE_BODY_STRONG, TYPE_TITLE_SM,
};
use massorca_ui::{ActiveTheme, IconName, TextStyled};

/// The height of an item's banner image; its width is the page's.
const BANNER_IMAGE_HEIGHT: f32 = 180.0;

/// Turns a button off with `reason` as its tooltip, when there is a reason.
fn gated(button: Button, reason: Option<&str>) -> Button {
    match reason {
        Some(reason) => button.disabled(true).tooltip(reason.to_owned()),
        None => button,
    }
}

/// The page of one item, or a note when the catalogue has no item with that id.
pub(super) fn item_page(
    frame: &MarketplaceFrame,
    id: MarketplaceItemId,
    cx: &mut App,
) -> AnyElement {
    let Some(item) = frame.snapshot.item(id) else {
        return EmptyState::new("marketplace-missing", "This item is not in the catalogue.")
            .explanation("It may have been removed from your sources.")
            .into_any_element();
    };
    let mut page = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_6))
        .max_w(px(880.0))
        .min_w_0();
    if let Some(picture) = item_images::banner(frame, item, cx) {
        page = page.child(banner_image(picture));
    }
    page = page.child(header(frame, item, cx));
    if let Some(alert) = problem_alert(item) {
        page = page.child(problem_banner(item.id, &alert));
    }
    for part in PARTS {
        let section = match part {
            Part::Logo | Part::Name | Part::Host => continue,
            Part::Banner => match banner(frame, item, cx) {
                Some(section) => section,
                None => continue,
            },
            Part::Connection => connection(frame, item, cx),
            Part::Information => information_section(item, cx),
            Part::Settings => settings_section(frame, item, cx),
            Part::AdvancedOptions => advanced_section(frame, item, cx),
            Part::DangerZone => danger_section(frame, item),
        };
        page = page.child(section);
    }
    page.into_any_element()
}

/// A button whose press runs one action on the shell.
fn shell_button(
    shell: &WeakEntity<Shell>,
    id: impl Into<ElementId>,
    label: &'static str,
    variant: ButtonVariant,
    action: impl Fn(&mut Shell, &mut Context<Shell>) + 'static,
) -> Button {
    let shell = shell.clone();
    Button::new(id, label)
        .variant(variant)
        .on_click(move |_, cx| {
            let _ = shell.update(cx, |shell, cx| action(shell, cx));
        })
}

/// The alert of an installed package's problem: danger for a tampered package, a warning for any other.
pub(super) fn problem_banner(id: MarketplaceItemId, alert: &ProblemAlert) -> AnyElement {
    let variant = match alert.tone {
        ProblemTone::Danger => BannerVariant::Danger,
        ProblemTone::Warning => BannerVariant::Warning,
    };
    div()
        .debug_selector(|| "marketplace-problem".to_owned())
        .child(
            Banner::new(
                SharedString::from(format!("marketplace-problem-{}", id.0)),
                variant,
                alert.message.clone(),
            )
            .title("This package has a problem"),
        )
        .into_any_element()
}

/// Parts 1 to 3: the plate with the listed logo or the kind icon, the name with the state chip and
/// Copy link, the meta line, the host and the install action, which is not available yet.
fn header(frame: &MarketplaceFrame, item: &MarketplaceItem, cx: &mut App) -> AnyElement {
    let url = item.source_url.clone();
    let copy = Button::new("marketplace-copy-link", "Copy link")
        .variant(ButtonVariant::Ghost)
        .on_click(move |_, cx| cx.write_to_clipboard(ClipboardItem::new_string(url.clone())));
    let mut header = MarketplaceItemHeader::new(
        SharedString::from(format!("marketplace-name-{}", item.id.0)),
        item.name.clone(),
        kind_icon(item.kind),
    )
    .state(state_chip(item, &frame.snapshot))
    .meta(format!(
        "{} · Marketplace · {}",
        item.publisher, item.version
    ))
    .copy_link(copy)
    .host(host_picker_row(frame, cx));
    header = match item_mark(item) {
        Some(mark) => header.agent(mark),
        None => match item_images::logo(frame, item, cx) {
            Some(logo) => header.logo(logo),
            None => header,
        },
    };
    if !item.state.is_installed() {
        header = header.action(install_action(frame, item));
    } else if frame.updatable {
        let id = item.id;
        header = header.action(shell_button(
            &frame.shell,
            "marketplace-program-update",
            "Update",
            ButtonVariant::Primary,
            move |shell, cx| shell.open_program_update(id, cx),
        ));
    }
    div()
        .debug_selector(|| "marketplace-part-logo".to_owned())
        .child(header)
        .into_any_element()
}

/// Review and install of a catalogue package opens its review. Sample items and agent kinds with no package show the
/// action unavailable; an agent that carries its package installs like any other package.
fn install_action(frame: &MarketplaceFrame, item: &MarketplaceItem) -> Button {
    if item.extension.is_none() {
        return Button::new("marketplace-install", "Review and install")
            .disabled(true)
            .tooltip(NOT_AVAILABLE);
    }
    let id = item.id;
    shell_button(
        &frame.shell,
        "marketplace-install",
        "Review and install",
        ButtonVariant::Primary,
        move |shell, cx| shell.open_extension_review(id, cx),
    )
}

/// The package's banner image, drawn above the header once the service has sent it.
fn banner_image(picture: ImageSource) -> AnyElement {
    div()
        .debug_selector(|| "marketplace-banner-image".to_owned())
        .w_full()
        .h(px(BANNER_IMAGE_HEIGHT))
        .rounded(px(RADIUS_LG))
        .overflow_hidden()
        .child(img(picture).size_full().object_fit(ObjectFit::Cover))
        .into_any_element()
}

/// Part 4: the example prompts, each a pill that opens a new chat with it, and how to ask the item in any chat. The
/// part is left out when the item has no example prompts.
fn banner(frame: &MarketplaceFrame, item: &MarketplaceItem, cx: &App) -> Option<AnyElement> {
    if item.examples.is_empty() {
        return None;
    }
    let mention = item
        .extension
        .as_deref()
        .and_then(|entry| entry.listing.mention.as_deref());
    let theme = *cx.theme();
    let fill = banner_fill(banner_tint(item), banner_palette_slot(item), &theme.palette);
    let shell = frame.shell.clone();
    let mut banner = InvocationBanner::new(
        SharedString::from(format!("marketplace-banner-{}", item.id.0)),
        item.name.clone(),
        banner_tag(mention, &item.name),
        kind_icon(item.kind),
    )
    .fill(fill)
    .on_select(move |prompt, window, cx| {
        let _ = shell.update(cx, |shell, cx| {
            shell.start_chat_with_prompt(&prompt, window, cx)
        });
    });
    if let Some(agent) = item_mark(item) {
        banner = banner.agent(agent);
    }
    for example in &item.examples {
        banner = banner.example(example.clone());
    }
    Some(
        div()
            .debug_selector(|| "marketplace-part-banner".to_owned())
            .child(banner)
            .into_any_element(),
    )
}

/// Where the banner's gradient starts: the listing's own colours, or else the palette colour of the item's slot.
pub(super) fn banner_fill(
    listed: Option<(Color, Option<Color>)>,
    slot: usize,
    palette: &Palette,
) -> BannerFill {
    match listed {
        Some((first, second)) => BannerFill::Listed(first, second),
        None => BannerFill::Palette(palette_colour(palette, slot)),
    }
}

/// The palette colour of a slot: `banner.palette.1` to `.8`, slots counted from one.
fn palette_colour(palette: &Palette, slot: usize) -> Color {
    let banner = &palette.banner;
    let colours = [
        banner.palette_1,
        banner.palette_2,
        banner.palette_3,
        banner.palette_4,
        banner.palette_5,
        banner.palette_6,
        banner.palette_7,
        banner.palette_8,
    ];
    colours[slot.saturating_sub(1) % colours.len()]
}

impl Shell {
    /// A banner pill's press: opens a new chat with the prompt in its composer, not sent, as the Canvas would open it.
    pub(super) fn start_chat_with_prompt(
        &mut self,
        prompt: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let composer = match self.model.selected_workspace().map(str::to_owned) {
            Some(workspace) => {
                self.open_new_session(&workspace, cx);
                self.start_composer(&workspace, cx)
            }
            None => {
                self.model.close_session();
                self.model.select_workspace(None);
                self.start_composer(HOME_START, cx)
            }
        };
        composer.update(cx, |composer, cx| {
            composer.set_draft(prompt, cx);
            composer.focus(window, cx);
        });
        cx.notify();
    }
}

/// The colours the listing gives the banner, parsed; none when it gives none or one is not `#RRGGBB`.
fn banner_tint(item: &MarketplaceItem) -> Option<(Color, Option<Color>)> {
    let colors = &item.extension.as_deref()?.listing.banner.as_ref()?.colors;
    let first = hex_colour(colors.first()?)?;
    Some((first, colors.get(1).and_then(|text| hex_colour(text))))
}

/// Part 5: the agent's accounts, with Add account and where the sign-in token lives; or why there are none.
fn connection(frame: &MarketplaceFrame, item: &MarketplaceItem, cx: &App) -> AnyElement {
    if let Some(reason) = absent_reason(item, Part::Connection) {
        return part_section(
            "marketplace-part-connection",
            "Connection",
            absent_note(reason, cx),
            cx,
        );
    }
    let kind = item.agent_kind.clone().unwrap_or_default();
    let block = account_block(item);
    let add = gated(
        shell_button(
            &frame.shell,
            "marketplace-add-account",
            "Add account",
            ButtonVariant::Secondary,
            move |shell, cx| shell.add_marketplace_account(&kind, cx),
        ),
        block.as_deref(),
    );
    let mut list = AccountConnectionList::new(SharedString::from(format!(
        "marketplace-connection-{}",
        item.id.0
    )))
    .empty("No accounts yet. Add one to use this agent.")
    .add_account(add);
    if frame.live_accounts {
        let kind = item.agent_kind.clone().unwrap_or_default();
        let import = shell_button(
            &frame.shell,
            "marketplace-import-account",
            "Import",
            ButtonVariant::Ghost,
            move |shell, cx| shell.open_account_import(&kind, cx),
        );
        list = list.import(gated(import, block.as_deref()));
    }
    let agent = item.agent_kind.clone().unwrap_or_default();
    let activity = AccountActivity {
        busy: Some(&frame.busy),
        signing_in: frame.signing_in,
    };
    for view in connection_rows(&frame.snapshot, &agent, activity) {
        list = list.account(account_row(frame, view));
    }
    let mut body = div().flex().flex_col().gap(px(SPACE_3)).min_w_0();
    let listing = frame
        .list_refusals
        .iter()
        .find(|(kind, _)| *kind == agent)
        .map(|(_, message)| message);
    if let Some(message) = listing.or(match &frame.error {
        Some((None, message)) => Some(message),
        _ => None,
    }) {
        body = body.child(alert(message, cx));
    }
    part_section(
        "marketplace-part-connection",
        "Connection",
        body.child(list),
        cx,
    )
}

/// One account row, with the actions its state allows: none while busy, Make active only while it is not active.
fn account_row(frame: &MarketplaceFrame, view: AccountView) -> AccountRow {
    let account = view.id;
    let shell = &frame.shell;
    let mut row = AccountRow::new(
        SharedString::from(format!("marketplace-account-{}", account.0)),
        view.label,
        account_status(view.state),
    )
    .identity(view.signed_in)
    .busy(view.busy)
    .active(view.active)
    .used_by(view.used_by);
    if view.can_check {
        row = row.check(shell_button(
            shell,
            SharedString::from(format!("marketplace-check-{}", account.0)),
            "Check now",
            ButtonVariant::Ghost,
            move |shell, cx| shell.check_marketplace_account(account, cx),
        ));
    }
    if view.can_remove {
        row = row.remove(shell_button(
            shell,
            SharedString::from(format!("marketplace-remove-{}", account.0)),
            "Remove",
            ButtonVariant::Ghost,
            move |shell, cx| shell.remove_marketplace_account(account, cx),
        ));
    }
    if view.can_make_active {
        row = row.make_active(shell_button(
            shell,
            SharedString::from(format!("marketplace-make-active-{}", account.0)),
            "Make active",
            ButtonVariant::Secondary,
            move |shell, cx| shell.make_marketplace_account_active(account, cx),
        ));
    }
    if let Some((Some(refused), message)) = &frame.error
        && *refused == account
    {
        row = row.error(message.clone());
    }
    row
}

/// The word-for-word state of an account, as the connection list draws it.
fn account_status(state: AccountState) -> AccountStatus {
    match state {
        AccountState::SignedIn => AccountStatus::SignedIn,
        AccountState::Expired => AccountStatus::Expired,
        AccountState::NeedsSignIn => AccountStatus::NeedsSignIn,
        AccountState::NotSetUp => AccountStatus::NotSetUp,
        AccountState::Migrated => AccountStatus::Migrated,
        AccountState::Conflict => AccountStatus::Conflict,
        AccountState::Unknown => AccountStatus::Unknown,
    }
}

/// The one sentence that says why a part does not apply, in the part's place.
fn absent_note(reason: &'static str, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.secondary)
        .child(reason)
        .into_any_element()
}

/// A refusal that is not about one row, announced as an alert.
fn alert(message: &str, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    div()
        .id("marketplace-connection-alert")
        .debug_selector(|| "marketplace-connection-alert".to_owned())
        .role(Role::Alert)
        .aria_label(message.to_owned())
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.status.danger_fg)
        .child(message.to_owned())
        .into_any_element()
}

/// Part 6: the description, then the rows: Developer, Category, and what else applies.
fn information_section(item: &MarketplaceItem, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let p = &theme.palette;
    let mut rows = div().flex().flex_col().gap(px(SPACE_2));
    for (label, value) in information(item) {
        rows = rows.child(
            div()
                .flex()
                .items_start()
                .gap(px(SPACE_4))
                .child(
                    div()
                        .w(px(160.0))
                        .flex_none()
                        .text_style(TYPE_BODY_SM)
                        .text_color(p.text.tertiary)
                        .child(label),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_style(TYPE_BODY_SM)
                        .text_color(p.text.primary)
                        .child(value),
                ),
        );
    }
    for (label, url) in information_links(item) {
        rows = rows.child(link_row(label, url, cx));
    }
    let body = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(
            div()
                .text_style(TYPE_BODY)
                .text_color(p.text.primary)
                .child(item.description.clone()),
        )
        .child(rows);
    part_section("marketplace-part-information", "Information", body, cx)
}

/// One Information row whose value is a link: pressing it opens the page in the browser.
fn link_row(label: &'static str, url: String, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let p = &theme.palette;
    let id = SharedString::from(format!(
        "marketplace-link-{}",
        label.to_lowercase().replace(' ', "-")
    ));
    let open = url.clone();
    div()
        .flex()
        .items_start()
        .gap(px(SPACE_4))
        .child(
            div()
                .w(px(160.0))
                .flex_none()
                .text_style(TYPE_BODY_SM)
                .text_color(p.text.tertiary)
                .child(label),
        )
        .child(
            div().flex_1().min_w_0().child(
                Button::new(id, url)
                    .variant(ButtonVariant::Ghost)
                    .on_click(move |_, cx| cx.open_url(&open)),
            ),
        )
        .into_any_element()
}

/// Part 7: the item's settings, or the reason there are none yet. The default agent is not
/// available yet, so its control shows that instead of a value.
fn settings_section(frame: &MarketplaceFrame, item: &MarketplaceItem, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let p = &theme.palette;
    let rows = settings_rows(item);
    let mut body = div().flex().flex_col().gap(px(SPACE_2));
    let switch = match (&item.extension, &item.state) {
        (Some(_), ItemState::Installed { enabled, .. }) => Some(*enabled),
        _ => None,
    };
    if let Some(enabled) = switch {
        body = body.child(enabled_row(frame, item, enabled, cx));
    }
    if rows.is_empty() && switch.is_none() {
        let text = if item.state.is_installed() {
            "This item has no settings."
        } else {
            "Settings appear here after the item is installed."
        };
        body = body.child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(p.text.secondary)
                .child(text),
        );
    }
    for (label, helper) in rows {
        body = body.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(SPACE_4))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(SPACE_1))
                        .child(
                            div()
                                .text_style(TYPE_BODY_STRONG)
                                .text_color(p.text.primary)
                                .child(label),
                        )
                        .child(
                            div()
                                .text_style(TYPE_BODY_SM)
                                .text_color(p.text.secondary)
                                .child(helper),
                        ),
                )
                .child(Badge::text(NOT_AVAILABLE).tone(BadgeTone::Neutral)),
        );
    }
    if let Some((_, text)) = frame.item_error.as_ref().filter(|(id, _)| *id == item.id) {
        body = body.child(alert(text, cx));
    }
    part_section("marketplace-part-settings", "Settings", body, cx)
}

/// The Enabled switch of an installed package: it shows the state the service reports, and sends the change.
fn enabled_row(
    frame: &MarketplaceFrame,
    item: &MarketplaceItem,
    enabled: bool,
    cx: &App,
) -> AnyElement {
    let theme = *cx.theme();
    let p = &theme.palette;
    let id = item.id;
    let shell = frame.shell.clone();
    let switch = Switch::new(
        SharedString::from(format!("marketplace-enabled-{}", item.id.0)),
        enabled,
    )
    .on_change(move |checked, _, cx| {
        let _ = shell.update(cx, |shell, cx| {
            shell.set_marketplace_enabled(id, checked, cx)
        });
    });
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(SPACE_4))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(SPACE_1))
                .child(
                    div()
                        .text_style(TYPE_BODY_STRONG)
                        .text_color(p.text.primary)
                        .child("Enabled"),
                )
                .child(
                    div()
                        .text_style(TYPE_BODY_SM)
                        .text_color(p.text.secondary)
                        .child("Off, the package stops running on this computer. Its files and data stay."),
                ),
        )
        .child(
            div()
                .debug_selector(|| "marketplace-enabled-switch".to_owned())
                .child(switch),
        )
        .into_any_element()
}

/// Part 8: the versions, the source and who can use the item, collapsed until it is opened.
fn advanced_section(frame: &MarketplaceFrame, item: &MarketplaceItem, cx: &App) -> AnyElement {
    if let Some(reason) = absent_reason(item, Part::AdvancedOptions) {
        return div()
            .debug_selector(|| "marketplace-part-advanced".to_owned())
            .child(absent_note(reason, cx))
            .into_any_element();
    }
    let theme = *cx.theme();
    let p = &theme.palette;
    let open = frame.advanced_open;
    let toggle = shell_button(
        &frame.shell,
        "marketplace-advanced-toggle",
        "Advanced Options",
        ButtonVariant::Ghost,
        |shell, cx| shell.toggle_marketplace_advanced(cx),
    )
    .trailing_icon(if open {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    });
    let mut section = div()
        .debug_selector(|| "marketplace-part-advanced".to_owned())
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(
            div()
                .debug_selector(|| "marketplace-advanced-toggle-row".to_owned())
                .child(toggle),
        );
    if open {
        let mut rows = div().flex().flex_col().gap(px(SPACE_2));
        for (label, value) in advanced_rows(item) {
            rows = rows.child(
                div()
                    .flex()
                    .gap(px(SPACE_4))
                    .child(
                        div()
                            .w(px(160.0))
                            .flex_none()
                            .text_style(TYPE_BODY_SM)
                            .text_color(p.text.tertiary)
                            .child(label),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_style(TYPE_BODY_SM)
                            .text_color(p.text.primary)
                            .child(value),
                    ),
            );
        }
        section = section.child(rows);
    }
    section.into_any_element()
}

/// Part 9: uninstall, behind a confirm. An item that is not installed has nothing to remove.
fn danger_section(frame: &MarketplaceFrame, item: &MarketplaceItem) -> AnyElement {
    let id = item.id;
    let mut zone = DangerZone::new(SharedString::from(format!(
        "marketplace-danger-{}",
        item.id.0
    )));
    if item.state.is_installed() {
        zone = zone.description(uninstall_body(item)).action(shell_button(
            &frame.shell,
            "marketplace-uninstall",
            "Uninstall",
            ButtonVariant::Danger,
            move |shell, cx| shell.ask_marketplace_uninstall(id, cx),
        ));
    } else {
        zone =
            zone.description("Not installed on this computer, so there is nothing to uninstall.");
    }
    div()
        .debug_selector(|| "marketplace-part-danger".to_owned())
        .child(zone)
        .into_any_element()
}

/// A part with its heading, on the raised surface of the page (teams-extensions-settings.md 7.2).
fn part_section(
    selector: &'static str,
    title: &'static str,
    body: impl IntoElement,
    cx: &App,
) -> AnyElement {
    let theme = *cx.theme();
    let p = &theme.palette;
    div()
        .debug_selector(move || selector.to_owned())
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .p(px(SPACE_4))
        .min_w_0()
        .rounded(px(RADIUS_LG))
        .border_1()
        .border_color(p.border.subtle)
        .bg(p.surface.raised)
        .child(
            div()
                .text_style(TYPE_TITLE_SM)
                .text_color(p.text.primary)
                .child(title),
        )
        .child(body)
        .into_any_element()
}
