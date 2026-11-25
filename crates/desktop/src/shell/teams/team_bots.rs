//! The Bots section of the Edit team form (pv96): each bot with Edit bot and Remove bot, then Add a bot.

use super::super::Shell;
use super::dialogs::TeamConfirm;
use crate::model::teams::{Bot, TeamId, TeamsState};
use gpui::{App, IntoElement, ParentElement, Styled, WeakEntity, div, px};
use massorca_ui::components::button::{Button, ButtonSize, ButtonVariant};
use massorca_ui::components::icon_button::IconButton;
use massorca_ui::tokens::{SPACE_1, SPACE_2, TYPE_BODY_SM, TYPE_BODY_STRONG};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};

/// The Bots section of an Edit team form: the team's bots, then Add a bot.
pub(super) fn bots_section(
    team: TeamId,
    teams: &TeamsState,
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> impl IntoElement {
    let theme = *cx.theme();
    let bots = teams
        .team(team)
        .map(|team| team.bots.as_slice())
        .unwrap_or_default();
    let add = {
        let shell = shell.clone();
        Button::new("team-form-add-bot", "Add a bot")
            .variant(ButtonVariant::Secondary)
            .on_click(move |window, cx| {
                let _ = shell.update(cx, |shell, cx| shell.open_add_bot(team, window, cx));
            })
    };
    let header = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_style(TYPE_BODY_STRONG)
                .text_color(theme.palette.text.primary)
                .child("Bots"),
        )
        .child(add);
    bots.iter().fold(
        div().flex().flex_col().gap(px(SPACE_1)).child(header),
        |section, bot| section.child(bot_row(team, bot, shell, &theme)),
    )
}

/// One bot of the section: its name and job, with Edit bot and Remove bot.
fn bot_row(team: TeamId, bot: &Bot, shell: &WeakEntity<Shell>, theme: &Theme) -> impl IntoElement {
    let (id, name) = (bot.id, bot.name.clone());
    let edit = {
        let shell = shell.clone();
        IconButton::new(
            format!("team-form-bot-edit-{}", id.0),
            IconName::Pencil,
            "Edit bot",
        )
        .size(ButtonSize::Compact)
        .on_click(move |window, cx| {
            let _ = shell.update(cx, |shell, cx| shell.open_edit_bot(team, id, window, cx));
        })
    };
    let remove = {
        let shell = shell.clone();
        IconButton::new(
            format!("team-form-bot-remove-{}", id.0),
            IconName::Trash2,
            "Remove bot",
        )
        .size(ButtonSize::Compact)
        .on_click(move |_, cx| {
            let confirm = TeamConfirm::RemoveBot {
                team,
                bot: id,
                name: name.clone(),
                error: None,
            };
            let _ = shell.update(cx, |shell, cx| shell.open_team_confirm(confirm, cx));
        })
    };
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(SPACE_2))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_style(TYPE_BODY_STRONG)
                        .text_color(theme.palette.text.primary)
                        .child(bot.name.clone()),
                )
                .child(
                    div()
                        .text_style(TYPE_BODY_SM)
                        .text_color(theme.palette.text.secondary)
                        .child(bot.role.clone()),
                ),
        )
        .child(edit)
        .child(remove)
}
