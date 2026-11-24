//! The Bot teams page (screens 4.5): a card per team with Open and Add bot, and "New team".

use super::super::Shell;
use super::super::change;
use super::access::access_control;
use crate::model::shell_state::ShellModel;
use crate::model::teams::{Team, TeamError, TeamPage, TeamsState};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, WeakEntity, div, px};
use massorca_ui::components::avatar::{Avatar, AvatarSize};
use massorca_ui::components::badge::Badge;
use massorca_ui::components::banner::{Banner, BannerVariant};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::card::{Card, CardPadding};
use massorca_ui::components::empty_state::EmptyState;
use massorca_ui::components::icon_button::IconButton;
use massorca_ui::tokens::{SPACE_1, SPACE_4, SPACE_6, TYPE_BODY_SM, TYPE_HEADING};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};

/// A team card is this wide; the cards wrap into as many columns as the width allows.
const CARD_WIDTH: f32 = 280.0;

/// The Bot teams page: the mode's home in Bot Teams (P13). Its cards open each team.
pub(in crate::shell) fn bot_teams_home(
    model: &ShellModel,
    shell: &WeakEntity<Shell>,
    cx: &mut App,
) -> AnyElement {
    let theme = *cx.theme();
    let state = model.teams();
    let connected = state.is_connected();
    let mut page = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .p(px(SPACE_6))
        .w_full()
        .child(header(shell, connected, &theme));
    if !connected {
        page = page.child(Banner::new(
            "bot-teams-offline",
            BannerVariant::Warning,
            "Bot Teams is not connected. Nothing can be created until it is back.",
        ));
    }
    if state.teams().is_empty() {
        page = page.child(
            EmptyState::new("bot-teams-empty", "No teams yet")
                .explanation("A team gathers bots around one Conference room and one Board.")
                .action(new_team_button(shell, connected)),
        );
    } else {
        page = page.child(grid(state, shell, &theme));
    }
    page.into_any_element()
}

fn header(shell: &WeakEntity<Shell>, connected: bool, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_style(TYPE_HEADING)
                .text_color(theme.palette.text.primary)
                .child("Bot teams"),
        )
        .child(new_team_button(shell, connected))
}

fn new_team_button(shell: &WeakEntity<Shell>, connected: bool) -> Button {
    let shell = shell.clone();
    Button::new("bot-teams-new-team", "New team")
        .variant(ButtonVariant::Primary)
        .leading_icon(IconName::Plus)
        .disabled(!connected)
        .on_click(move |window, cx| {
            let _ = shell.update(cx, |shell, cx| shell.open_new_team(window, cx));
        })
}

fn grid(state: &TeamsState, shell: &WeakEntity<Shell>, theme: &Theme) -> impl IntoElement {
    let connected = state.is_connected();
    div()
        .flex()
        .flex_wrap()
        .gap(px(SPACE_4))
        .children(state.teams().iter().map(|team| {
            let removal = state.removal_notice(team.id);
            team_card(
                team,
                state.access_notice(team.id),
                removal,
                shell,
                connected,
                theme,
            )
        }))
}

/// A team card: its name and mark, the lead bot, the bot count, the host and Workspaces (4.5).
fn team_card(
    team: &Team,
    notice: Option<&str>,
    removal: Option<&str>,
    shell: &WeakEntity<Shell>,
    connected: bool,
    theme: &Theme,
) -> impl IntoElement {
    let id = team.id;
    let bots = match team.bots.len() {
        1 => "1 bot".to_owned(),
        count => format!("{count} bots"),
    };
    let lead = team.lead_name().map_or_else(
        || "No lead bot yet".to_owned(),
        |name| format!("Lead bot: {name}"),
    );
    let open = {
        let shell = shell.clone();
        Button::new(format!("team-open-{}", id.0), "Open")
            .variant(ButtonVariant::Secondary)
            .on_click(move |_, cx| {
                change(&shell, cx, move |shell| {
                    shell.model.open_team_page(TeamPage::Conference(id));
                });
            })
    };
    let add_bot = {
        let shell = shell.clone();
        Button::new(format!("team-add-bot-{}", id.0), "Add bot")
            .variant(ButtonVariant::Secondary)
            .disabled(!connected)
            .on_click(move |window, cx| {
                let _ = shell.update(cx, |shell, cx| shell.open_add_bot(id, window, cx));
            })
    };
    let more = {
        let shell = shell.clone();
        IconButton::new(format!("team-more-{}", id.0), IconName::Ellipsis, "More")
            .disabled(!connected)
            .disabled_reason(TeamError::Unavailable.message())
            .on_click(move |window, cx| {
                let _ = shell.update(cx, |shell, cx| shell.open_team_menu(id, window, cx));
            })
    };
    let workspaces = if team.workspaces { "on" } else { "off" };
    div().w(px(CARD_WIDTH)).child(
        Card::new(format!("team-card-{}", id.0))
            .padding(CardPadding::Comfortable)
            .title(team.name.clone())
            .header_action(
                Avatar::team(format!("team-mark-{}", id.0), team.name.clone())
                    .size(AvatarSize::Medium),
            )
            .child(line(lead, theme))
            .child(line(bots, theme))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(SPACE_1))
                    .child(Badge::text(format!("Runs on {}", team.host)))
                    .child(line(
                        format!("Workspaces in this team: {workspaces}"),
                        theme,
                    ))
                    .child(access_control(team, notice, shell, connected, theme))
                    .children(removal.map(|text| danger_line(text, theme))),
            )
            .footer_action(open)
            .footer_action(add_bot)
            .footer_action(more),
    )
}

/// A notice in the danger colour, under the card's details.
fn danger_line(text: &str, theme: &Theme) -> impl IntoElement {
    div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.status.danger_fg)
        .child(text.to_owned())
}

pub(super) fn line(text: String, theme: &Theme) -> impl IntoElement {
    div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.secondary)
        .child(text)
}
