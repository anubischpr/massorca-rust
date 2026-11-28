//! Mention suggestions of the Conference room composer (bt-conference-room, AUTO-13): while the last word of the field
//! starts with `@`, the service's `team.room_mentions` names what it can reach (bots, teams, projects and files), and
//! picking one puts its text in place of that word.

use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Styled, WeakEntity,
    Window, div, px,
};
use massorca_protocol::methods::team_room::{RoomMention, RoomMentionKind, TeamRoomMentionsParams};
use massorca_ui::components::button::{Button, ButtonSize, ButtonVariant};
use massorca_ui::tokens::SPACE_1;
use massorca_ui::{IconName, Theme};

use super::super::Shell;
use crate::model::teams::TeamId;

/// The suggestions on screen: the team, the word they are for, and what it can name.
#[derive(Debug, Default)]
pub(in crate::shell) struct MentionSuggestions {
    asked: Option<(TeamId, String)>,
    shown: Vec<RoomMention>,
}

/// The text after the `@` of the field's last word, while that word starts with `@` and no space follows it.
pub(in crate::shell) fn mention_query(text: &str) -> Option<&str> {
    if text.ends_with(char::is_whitespace) {
        return None;
    }
    let word = text.rsplit(char::is_whitespace).next()?;
    word.strip_prefix('@')
}

/// `text` with its last word replaced by `insert`, then a space unless `insert` asks for more (`@file:`, a folder).
pub(in crate::shell) fn with_mention(text: &str, insert: &str) -> String {
    let start = text.rfind(char::is_whitespace).map_or(0, |at| {
        at + text[at..].chars().next().map_or(1, char::len_utf8)
    });
    let mut next = format!("{}{insert}", &text[..start]);
    if !insert.ends_with([':', '/']) {
        next.push(' ');
    }
    next
}

impl Shell {
    /// Asks the service what the field's last `@` word of `team`'s room can name; another word clears the suggestions.
    pub(in crate::shell) fn suggest_mentions(
        &mut self,
        team: TeamId,
        text: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(query) = mention_query(text).map(str::to_owned) else {
            if self.room_page.mentions.asked.take().is_some() {
                self.room_page.mentions.shown.clear();
                cx.notify();
            }
            return;
        };
        let asked = Some((team, query.clone()));
        if self.room_page.mentions.asked == asked {
            return;
        }
        self.room_page.mentions.asked = asked.clone();
        let (Some(service), Some(key)) = (self.service.clone(), self.team_key(team, cx)) else {
            return;
        };
        let params = TeamRoomMentionsParams {
            team: key,
            query,
            limit: Some(8),
        };
        cx.spawn(async move |this, cx| {
            let answer = service.team_room_mentions(params).await;
            this.update(cx, |shell, cx| {
                // An answer for a word no longer typed is dropped.
                if shell.room_page.mentions.asked == asked {
                    shell.room_page.mentions.shown =
                        answer.map(|found| found.mentions).unwrap_or_default();
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Puts `insert` in place of the field's last `@` word.
    pub(in crate::shell) fn pick_mention(
        &mut self,
        team: TeamId,
        insert: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(field) = self.room_composer_field(team) else {
            return;
        };
        let next = with_mention(field.read(cx).text(), insert);
        self.room_page.mentions = MentionSuggestions::default();
        field.update(cx, |field, cx| {
            field.set_text(next, window, cx);
            field.focus(window, cx);
        });
        cx.notify();
    }

    /// The suggestions shown for `team`'s composer.
    pub(in crate::shell) fn mentions_of(&self, team: TeamId) -> Vec<RoomMention> {
        match &self.room_page.mentions.asked {
            Some((asked, _)) if *asked == team => self.room_page.mentions.shown.clone(),
            _ => Vec::new(),
        }
    }
}

/// The row of suggestions above the composer's field; nothing while there are none.
pub(in crate::shell) fn suggestions_row(
    team: TeamId,
    mentions: &[RoomMention],
    shell: &WeakEntity<Shell>,
    _theme: &Theme,
) -> Option<AnyElement> {
    if mentions.is_empty() {
        return None;
    }
    let chips = mentions.iter().enumerate().map(|(index, mention)| {
        let (shell, insert) = (shell.clone(), mention.insert.clone());
        let icon = match mention.kind {
            RoomMentionKind::Bot => IconName::Bot,
            RoomMentionKind::Team => IconName::Users,
            RoomMentionKind::Project => IconName::Folder,
            RoomMentionKind::File | RoomMentionKind::Unknown => IconName::File,
        };
        Button::new(("room-mention", index), mention.insert.clone())
            .variant(ButtonVariant::Ghost)
            .size(ButtonSize::Compact)
            .leading_icon(icon)
            .on_click(move |window, cx| {
                let _ = shell.update(cx, |shell, cx| {
                    shell.pick_mention(team, &insert, window, cx)
                });
            })
    });
    Some(
        div()
            .flex()
            .flex_wrap()
            .gap(px(SPACE_1))
            .debug_selector(|| "room-mentions".to_owned())
            .children(chips)
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_word_starting_with_an_at_is_the_query() {
        assert_eq!(mention_query("ask @ma"), Some("ma"));
        assert_eq!(mention_query("@"), Some(""));
        assert_eq!(mention_query("see @file:src/"), Some("file:src/"));
        assert_eq!(mention_query("ask @mara "), None, "a space ends the word");
        assert_eq!(mention_query("mail me@x"), None);
        assert_eq!(mention_query(""), None);
    }

    #[test]
    fn a_picked_mention_replaces_the_word_and_a_folder_waits_for_more() {
        assert_eq!(with_mention("ask @ma", "@Mara"), "ask @Mara ");
        assert_eq!(with_mention("@fi", "@file:"), "@file:");
        assert_eq!(with_mention("see @file:sr", "@file:src/"), "see @file:src/");
    }
}
