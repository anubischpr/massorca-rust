//! The Conference room of a team (bt-conference-room, screens 4.2, decision 0102): its header with the team pill and
//! the bots, the thread of the room, and a composer whose receiver picks the bot a line naming none goes to.

use std::collections::BTreeMap;
use std::fmt;

use super::super::Shell;
use super::{team_pill, team_shape};
use crate::model::canvas::Canvas;
use crate::model::teams::{
    BotId, EMPTY_ROOM, NO_BOTS, RoomLine, RoomTask, RoomView, Speaker, Team, TeamId, TeamPage,
    receiver, room_view,
};
use crate::service::Refusal;
use gpui::{
    AnyElement, App, AppContext, Context, Entity, IntoElement, ParentElement, Pixels, Point,
    Styled, Subscription, WeakEntity, Window, div, prelude::*, px,
};
use jiff::tz::TimeZone;
use massorca_protocol::methods::team_room::TeamRoomPostParams;
use massorca_ui::components::badge::{Badge, BadgeTone};
use massorca_ui::components::button::{Button, ButtonSize, ButtonVariant};
use massorca_ui::components::card::{Card, CardPadding};
use massorca_ui::components::menu::{MenuEntry, MenuEvent, MenuItem, MenuView};
use massorca_ui::components::popover::OverlayLayer;
use massorca_ui::components::scroll_area::{ScrollArea, ScrollAxis};
use massorca_ui::components::team_character::TeamCharacterMark;
use massorca_ui::components::text_field::{TextField, TextFieldEvent};
use massorca_ui::tokens::{
    RADIUS_LG, SIZE_AVATAR_MD, SPACE_1, SPACE_2, SPACE_3, SPACE_4, SPACE_5, TYPE_BODY,
    TYPE_BODY_SM, TYPE_BODY_STRONG, TYPE_CAPTION, TYPE_TITLE_SM,
};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};

/// What the room says when the shell has no service behind it.
const NO_SERVICE: &str = "The Conference room needs a connected MassOrca service.";

/// The room's state the shell keeps: the composer of the team on screen, each team's picked receiver, and why the
/// last line was refused.
#[derive(Default)]
pub(in crate::shell) struct RoomPage {
    composer: Option<RoomComposer>,
    picks: BTreeMap<TeamId, BotId>,
    sending: bool,
    notice: Option<(TeamId, String)>,
    /// A refused line's text, put back in the field on the next draw.
    restore: Option<String>,
    /// The room reads asked for, by the service's team id, so each team is read once per open page.
    requested: Option<String>,
    pub(in crate::shell) receiver_menu: Option<OpenReceiverMenu>,
}

impl fmt::Debug for RoomPage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RoomPage")
            .field(
                "team",
                &self.composer.as_ref().map(|composer| composer.team),
            )
            .field("picks", &self.picks)
            .field("sending", &self.sending)
            .field("notice", &self.notice)
            .field("receiver_menu", &self.receiver_menu.is_some())
            .finish_non_exhaustive()
    }
}

struct RoomComposer {
    team: TeamId,
    field: Entity<TextField>,
    _events: Subscription,
}

/// The receiver menu on screen: the team's bots, and where it opened.
pub(in crate::shell) struct OpenReceiverMenu {
    pub(in crate::shell) menu: Entity<MenuView>,
    pub(in crate::shell) at: Point<Pixels>,
    _subscription: Subscription,
}

/// What the Conference page draws.
pub(in crate::shell) struct RoomFrame {
    team: Team,
    view: RoomView,
    field: Entity<TextField>,
    /// The bot a line naming none goes to, and whether it is the lead.
    receiver: Option<(String, bool)>,
    sending: bool,
    notice: Option<String>,
    shell: WeakEntity<Shell>,
}

impl Shell {
    /// The Conference page's frame, while the Canvas shows a Conference room. Opening a team's room reads it once; the
    /// service's `team.room_changed` keeps it fresh.
    pub(in crate::shell) fn room_frame(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<RoomFrame> {
        let Canvas::Team(TeamPage::Conference(team_id)) = self.model.canvas() else {
            self.room_page.requested = None;
            return None;
        };
        let team = self.model.teams().team(team_id)?.clone();
        let key = self.team_key(team_id, cx);
        let view = match (&self.service, &key) {
            (Some(service), Some(key)) => {
                if self.room_page.requested.as_deref() != Some(key.as_str()) {
                    self.room_page.requested = Some(key.clone());
                    service.read_team_room(key.clone());
                }
                let state = self.store.read(cx).state();
                room_view(
                    state.team_rooms.get(key),
                    &team,
                    &state.team_ids,
                    &TimeZone::system(),
                )
            }
            _ => RoomView::Unavailable(NO_SERVICE.to_owned()),
        };
        let field = self.room_field(team_id, window, cx);
        if let Some(text) = self.room_page.restore.take() {
            field.update(cx, |field, cx| field.set_text(text, window, cx));
        }
        let picked = self.room_page.picks.get(&team_id).copied();
        let receiver = receiver(&team, picked)
            .and_then(|bot| team.bot(bot))
            .map(|bot| (bot.name.clone(), team.lead == Some(bot.id)));
        let notice = self
            .room_page
            .notice
            .as_ref()
            .filter(|(team, _)| *team == team_id)
            .map(|(_, text)| text.clone());
        Some(RoomFrame {
            team,
            view,
            field,
            receiver,
            sending: self.room_page.sending,
            notice,
            shell: cx.entity().downgrade(),
        })
    }

    /// The composer field of `team`'s room; a new team gets a new field.
    fn room_field(
        &mut self,
        team: TeamId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<TextField> {
        if let Some(composer) = self
            .room_page
            .composer
            .as_ref()
            .filter(|composer| composer.team == team)
        {
            return composer.field.clone();
        }
        let field = cx.new(|cx| {
            TextField::new(cx)
                .label("Message the team")
                .placeholder("Ask the team, or name a bot with @")
        });
        let events = cx.subscribe_in(&field, window, move |shell, _, event, window, cx| {
            if *event == TextFieldEvent::Submitted {
                shell.send_room_line(team, window, cx);
            }
        });
        self.room_page.composer = Some(RoomComposer {
            team,
            field: field.clone(),
            _events: events,
        });
        field
    }

    /// Posts the composer's text to `team`'s room. The field empties at once and takes the text back if the service
    /// refuses it.
    pub(in crate::shell) fn send_room_line(
        &mut self,
        team: TeamId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(composer) = self
            .room_page
            .composer
            .as_ref()
            .filter(|composer| composer.team == team)
        else {
            return;
        };
        let text = composer.field.read(cx).text().trim().to_owned();
        if text.is_empty() || self.room_page.sending {
            return;
        }
        let (Some(service), Some(key), Some(team_data)) = (
            self.service.clone(),
            self.team_key(team, cx),
            self.model.teams().team(team).cloned(),
        ) else {
            return;
        };
        let to = receiver(&team_data, self.room_page.picks.get(&team).copied()).and_then(|bot| {
            self.store
                .read(cx)
                .state()
                .team_ids
                .key(bot.0)
                .map(str::to_owned)
        });
        composer
            .field
            .clone()
            .update(cx, |field, cx| field.clear_text(window, cx));
        self.room_page.sending = true;
        self.room_page.notice = None;
        cx.notify();
        let params = TeamRoomPostParams {
            team: key,
            text: text.clone(),
            bot: None,
            to,
            session: None,
            agent_token: None,
        };
        cx.spawn(async move |this, cx| {
            let answer = service.post_to_team_room(params).await;
            this.update(cx, |shell, cx| {
                shell.room_page.sending = false;
                if let Err(refusal) = answer {
                    shell.room_page.notice = Some((team, refusal_text(&refusal)));
                    shell.room_page.restore = Some(text);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Opens the menu of the bots a line naming none can go to, at the pointer.
    pub(in crate::shell) fn open_receiver_menu(
        &mut self,
        team: TeamId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(team_data) = self.model.teams().team(team) else {
            return;
        };
        let chosen = receiver(team_data, self.room_page.picks.get(&team).copied());
        let bots: Vec<BotId> = team_data.bots.iter().map(|bot| bot.id).collect();
        let entries: Vec<MenuEntry> = team_data
            .bots
            .iter()
            .map(|bot| {
                let label = if team_data.lead == Some(bot.id) {
                    format!("{} (lead)", bot.name)
                } else {
                    bot.name.clone()
                };
                MenuItem::new(label).checked(Some(bot.id) == chosen).into()
            })
            .collect();
        let menu = cx.new(|cx| MenuView::new(entries, window, cx).with_layer(OverlayLayer::Page));
        let subscription =
            cx.subscribe_in(&menu, window, move |shell, _, event: &MenuEvent, _, cx| {
                shell.room_page.receiver_menu = None;
                if let MenuEvent::Chosen(choice) = event
                    && let Some(bot) = bots.get(choice.row)
                {
                    shell.room_page.picks.insert(team, *bot);
                }
                cx.notify();
            });
        self.room_page.receiver_menu = Some(OpenReceiverMenu {
            menu,
            at: window.mouse_position(),
            _subscription: subscription,
        });
        cx.notify();
    }
}

/// The sentence a refused line shows above the composer.
fn refusal_text(refusal: &Refusal) -> String {
    format!("Not sent: {}", refusal.sentence())
}

/// The Conference room page.
pub(in crate::shell) fn conference_page(
    frame: RoomFrame,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let theme = *cx.theme();
    let team_id = frame.team.id;
    let header = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(SPACE_3))
        .px(px(SPACE_5))
        .py(px(SPACE_3))
        .border_b_1()
        .border_color(theme.palette.border.subtle)
        .debug_selector(|| "room-header".to_owned())
        .child(
            div()
                .text_style(TYPE_TITLE_SM)
                .text_color(theme.palette.text.primary)
                .child("Conference room"),
        )
        .child(team_pill(
            TeamPage::Conference(team_id),
            &frame.team.name,
            frame.team.character,
            &frame.shell,
            window,
            cx,
        ))
        .child(div().flex_1())
        .child(bots_row(&frame.team, &theme));
    let thread = match &frame.view {
        RoomView::Loading => note("Reading the room…", &theme).into_any_element(),
        RoomView::Unavailable(sentence) => note(sentence, &theme).into_any_element(),
        RoomView::Lines(lines) if lines.is_empty() => note(EMPTY_ROOM, &theme)
            .debug_selector(|| "room-empty".to_owned())
            .into_any_element(),
        RoomView::Lines(lines) => div()
            .flex()
            .flex_col()
            .gap(px(SPACE_4))
            .children(
                lines
                    .iter()
                    .map(|line| line_view(line, &frame.team, &frame.shell, &theme)),
            )
            .into_any_element(),
    };
    let ready = matches!(frame.view, RoomView::Lines(_)) && !frame.team.bots.is_empty();
    div()
        .size_full()
        .flex()
        .flex_col()
        .child(header)
        .child(
            div().flex_1().min_h_0().child(
                ScrollArea::new("room-thread")
                    .axis(ScrollAxis::Vertical)
                    .child(div().px(px(SPACE_5)).py(px(SPACE_4)).child(thread)),
            ),
        )
        .child(composer(&frame, ready, &theme))
        .into_any_element()
}

/// The team's bots as their characters, then how many there are.
fn bots_row(team: &Team, theme: &Theme) -> impl IntoElement {
    let count = match team.bots.len() {
        1 => "1 bot".to_owned(),
        count => format!("{count} bots"),
    };
    div()
        .flex()
        .items_center()
        .gap(px(SPACE_1))
        .children(
            team.bots
                .iter()
                .map(|bot| TeamCharacterMark::new(team_shape(team.character), bot.name.clone())),
        )
        .child(
            div()
                .ml(px(SPACE_1))
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(count),
        )
}

/// One line: a person's bubble on the right, or a bot's row with its character, name, Lead tag, time and task card.
fn line_view(line: &RoomLine, team: &Team, shell: &WeakEntity<Shell>, theme: &Theme) -> AnyElement {
    let palette = theme.palette;
    match &line.speaker {
        Speaker::Person => div()
            .flex()
            .justify_end()
            .child(
                div()
                    .max_w(px(560.))
                    .px(px(SPACE_3))
                    .py(px(SPACE_2))
                    .rounded(px(RADIUS_LG))
                    .bg(palette.surface.raised)
                    .text_style(TYPE_BODY)
                    .text_color(palette.text.primary)
                    .debug_selector(|| "room-person-line".to_owned())
                    .child(line.text.clone()),
            )
            .into_any_element(),
        speaker => {
            let (name, lead) = match speaker {
                Speaker::Bot { name, lead } => (name.clone(), *lead),
                _ => ("A removed bot".to_owned(), false),
            };
            let meta = div()
                .flex()
                .items_center()
                .gap(px(SPACE_2))
                .child(
                    div()
                        .text_style(TYPE_BODY_STRONG)
                        .text_color(palette.text.primary)
                        .child(name.clone()),
                )
                .when(lead, |meta| {
                    meta.child(Badge::tag("Lead").tone(BadgeTone::Accent))
                })
                .child(
                    div()
                        .text_style(TYPE_CAPTION)
                        .text_color(palette.text.tertiary)
                        .child(line.time.clone()),
                );
            let body = div()
                .flex()
                .flex_col()
                .gap(px(SPACE_1))
                .min_w_0()
                .child(meta)
                .child(
                    div()
                        .text_style(TYPE_BODY)
                        .text_color(palette.text.primary)
                        .child(line.text.clone()),
                )
                .children(
                    line.task
                        .as_ref()
                        .map(|task| task_card(task, team.id, &line.id, shell)),
                );
            div()
                .flex()
                .gap(px(SPACE_3))
                .debug_selector(|| "room-bot-line".to_owned())
                .child(
                    TeamCharacterMark::new(team_shape(team.character), name).size(SIZE_AVATAR_MD),
                )
                .child(body)
                .into_any_element()
        }
    }
}

/// The card of the task a line is about, with the way to its Board.
fn task_card(
    task: &RoomTask,
    team: TeamId,
    line: &str,
    shell: &WeakEntity<Shell>,
) -> impl IntoElement {
    let shell = shell.clone();
    div().max_w(px(420.)).child(
        Card::new(format!("room-task-{line}"))
            .padding(CardPadding::Compact)
            .title(task.title.clone())
            .child(Badge::tag(task.column))
            .footer_action(
                Button::new(format!("room-open-board-{line}"), "Open on Board")
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Compact)
                    .on_click(move |_, cx| {
                        let _ = shell.update(cx, |shell, cx| {
                            shell.model.open_team_page(TeamPage::Board(team));
                            cx.notify();
                        });
                    }),
            ),
    )
}

/// The composer: the refusal of the last line, the field, the receiver and Send.
fn composer(frame: &RoomFrame, ready: bool, theme: &Theme) -> impl IntoElement {
    let team = frame.team.id;
    let (send, pick) = (frame.shell.clone(), frame.shell.clone());
    let reason = if frame.team.bots.is_empty() {
        Some(NO_BOTS.to_owned())
    } else {
        frame.notice.clone()
    };
    let receiver = match &frame.receiver {
        Some((name, true)) => format!("Lead bot: {name}"),
        Some((name, false)) => format!("To: {name}"),
        None => "No bot".to_owned(),
    };
    div()
        .flex()
        .flex_none()
        .flex_col()
        .gap(px(SPACE_2))
        .px(px(SPACE_5))
        .py(px(SPACE_3))
        .border_t_1()
        .border_color(theme.palette.border.subtle)
        .children(reason.map(|text| {
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .debug_selector(|| "room-notice".to_owned())
                .child(text)
        }))
        .child(frame.field.clone())
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(SPACE_2))
                .child(
                    Button::new("room-receiver", receiver)
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Compact)
                        .trailing_icon(IconName::ChevronDown)
                        .disabled(!ready)
                        .on_click(move |window, cx| {
                            let _ = pick.update(cx, |shell, cx| {
                                shell.open_receiver_menu(team, window, cx);
                            });
                        }),
                )
                .child(div().flex_1())
                .child(
                    div().debug_selector(|| "room-send".to_owned()).child(
                        Button::new("room-send", "Send")
                            .variant(ButtonVariant::Primary)
                            .size(ButtonSize::Compact)
                            .loading(frame.sending)
                            .disabled(!ready || frame.sending)
                            .on_click(move |window, cx| {
                                let _ = send.update(cx, |shell, cx| {
                                    shell.send_room_line(team, window, cx);
                                });
                            }),
                    ),
                ),
        )
}

fn note(text: &str, theme: &Theme) -> gpui::Div {
    div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.tertiary)
        .child(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::teams::TeamPage;
    use crate::service::fixtures::team;
    use crate::service::{Command, RoomCommand, StoreUpdate, TeamRoomRead, TeamsRead};
    use crate::shell::service_tests::{click, draw, rig};
    use gpui::{TestAppContext, VisualTestContext};
    use massorca_protocol::methods::team_room::{RoomAuthor, TeamRoomMessage};
    use massorca_protocol::methods::teams::Bot as WireBot;

    fn bot(id: &str, name: &str, lead: bool) -> WireBot {
        WireBot {
            id: id.into(),
            team_id: "t1".into(),
            name: name.into(),
            job: "Builds".into(),
            kind: "claude".into(),
            model: None,
            effort: None,
            lead,
            created_at_ms: 1,
            skills: Vec::new(),
        }
    }

    /// A connected shell showing team t1's Conference room, whose bots are Mara and Bruno, the lead.
    fn open_room(
        cx: &mut TestAppContext,
    ) -> (crate::shell::service_tests::Rig, &mut VisualTestContext) {
        let (mut rig, cx) = rig(cx, &[]);
        let mut record = team("t1", "Desarrollo");
        record.bots = vec![bot("b1", "Mara", false), bot("b2", "Bruno", true)];
        rig.updates
            .send(StoreUpdate::Teams(TeamsRead {
                number: 1,
                teams: Ok(vec![record]),
            }))
            .expect("the store listens");
        cx.run_until_parked();
        rig.shell.update(cx, |shell, cx| {
            shell.model.press_mode_switch();
            shell.model.open_team_page(TeamPage::Conference(TeamId(1)));
            cx.notify();
        });
        draw(cx);
        let Some(Command::Room(RoomCommand::Read { team })) = rig.script.next() else {
            panic!("opening the room reads it");
        };
        assert_eq!(team, "t1");
        (rig, cx)
    }

    fn line(author: RoomAuthor, bot: Option<(&str, &str)>, text: &str) -> TeamRoomMessage {
        TeamRoomMessage {
            id: text.into(),
            team_id: "t1".into(),
            author,
            bot_id: bot.map(|(id, _)| id.into()),
            bot_name: bot.map(|(_, name)| name.into()),
            text: text.into(),
            task: None,
            created_at_ms: 1_700_000_000_000,
        }
    }

    fn room_read(lines: Vec<TeamRoomMessage>) -> StoreUpdate {
        StoreUpdate::TeamRoom(TeamRoomRead {
            team: "t1".into(),
            number: 1,
            room: Ok(lines),
        })
    }

    #[gpui::test]
    fn an_empty_room_says_what_to_do_and_a_read_draws_its_lines(cx: &mut TestAppContext) {
        let (rig, cx) = open_room(cx);
        rig.updates
            .send(room_read(Vec::new()))
            .expect("the store listens");
        cx.run_until_parked();
        draw(cx);
        assert!(
            cx.debug_bounds("room-empty").is_some(),
            "the empty room's line"
        );

        rig.updates
            .send(StoreUpdate::TeamRoom(TeamRoomRead {
                team: "t1".into(),
                number: 2,
                room: Ok(vec![
                    line(RoomAuthor::Person, None, "We need sign-in"),
                    line(
                        RoomAuthor::Bot,
                        Some(("b2", "Bruno")),
                        "Added to the Board: sign-in",
                    ),
                ]),
            }))
            .expect("the store listens");
        cx.run_until_parked();
        draw(cx);
        assert!(cx.debug_bounds("room-person-line").is_some());
        assert!(cx.debug_bounds("room-bot-line").is_some());
        assert!(
            cx.debug_bounds("badge-tag").is_some(),
            "the lead's Lead tag"
        );
        assert!(cx.debug_bounds("room-empty").is_none());
    }

    #[gpui::test]
    fn a_line_goes_to_the_lead_and_a_refused_one_comes_back_with_why(cx: &mut TestAppContext) {
        let (mut rig, cx) = open_room(cx);
        rig.updates
            .send(room_read(Vec::new()))
            .expect("the store listens");
        cx.run_until_parked();
        draw(cx);
        rig.shell.update_in(cx, |shell, window, cx| {
            let field = shell
                .room_page
                .composer
                .as_ref()
                .map(|composer| composer.field.clone())
                .expect("the room has its composer");
            field.update(cx, |field, cx| {
                field.set_text("Add magic link sign-in", window, cx)
            });
        });
        click(cx, "room-send");
        let Some(Command::Room(RoomCommand::Post { params, reply })) = rig.script.next() else {
            panic!("Send posts the line");
        };
        assert_eq!(params.team, "t1");
        assert_eq!(params.text, "Add magic link sign-in");
        assert_eq!(params.to.as_deref(), Some("b2"), "the lead receives it");
        assert_eq!(params.bot, None);
        let _ = reply.send(Err(Refusal::Failed {
            code: "task.no_bot".into(),
            message: "Add a bot to this team first.".into(),
        }));
        cx.run_until_parked();
        draw(cx);
        assert!(
            cx.debug_bounds("room-notice").is_some(),
            "the refusal shows"
        );
        rig.shell.read_with(cx, |shell, cx| {
            let field = shell
                .room_page
                .composer
                .as_ref()
                .map(|composer| composer.field.clone())
                .expect("the room has its composer");
            assert_eq!(
                field.read(cx).text(),
                "Add magic link sign-in",
                "the text comes back"
            );
        });
    }

    #[gpui::test]
    fn a_service_without_a_room_says_so_and_sends_nothing(cx: &mut TestAppContext) {
        let (mut rig, cx) = open_room(cx);
        rig.updates
            .send(StoreUpdate::TeamRoom(TeamRoomRead {
                team: "t1".into(),
                number: 1,
                room: Err(Refusal::Failed {
                    code: "service.capability_missing".into(),
                    message: crate::service::NO_ROOM.into(),
                }),
            }))
            .expect("the store listens");
        cx.run_until_parked();
        draw(cx);
        click(cx, "room-send");
        while let Some(command) = rig.script.next() {
            assert!(
                !matches!(command, Command::Room(RoomCommand::Post { .. })),
                "a room the service cannot read sends nothing"
            );
        }
    }

    #[gpui::test]
    fn the_receiver_picked_takes_lines_that_name_no_bot(cx: &mut TestAppContext) {
        let (mut rig, cx) = open_room(cx);
        rig.updates
            .send(room_read(Vec::new()))
            .expect("the store listens");
        cx.run_until_parked();
        draw(cx);
        rig.shell.update_in(cx, |shell, window, cx| {
            shell.open_receiver_menu(TeamId(1), window, cx);
        });
        let menu = rig.shell.read_with(cx, |shell, _| {
            shell
                .room_page
                .receiver_menu
                .as_ref()
                .map(|open| open.menu.clone())
                .expect("the receiver menu opens")
        });
        menu.update(cx, |_, cx| {
            cx.emit(MenuEvent::Chosen(
                massorca_ui::components::menu::MenuChoice {
                    row: 0,
                    child: None,
                },
            ));
        });
        cx.run_until_parked();
        rig.shell.update_in(cx, |shell, window, cx| {
            let field = shell
                .room_page
                .composer
                .as_ref()
                .map(|composer| composer.field.clone())
                .expect("the room has its composer");
            field.update(cx, |field, cx| field.set_text("Check the logs", window, cx));
        });
        click(cx, "room-send");
        let Some(Command::Room(RoomCommand::Post { params, .. })) = rig.script.next() else {
            panic!("Send posts the line");
        };
        assert_eq!(
            params.to.as_deref(),
            Some("b1"),
            "Mara, picked in the composer"
        );
    }
}
