//! The team memory (D-AG-26, screens 4.8): what the team remembers, under a Bot and a Team tab. A
//! memory entry is deleted after a confirm, and the page says who can read it.

use super::super::Shell;
use super::dialogs::TeamConfirm;
use super::team_pill;
use crate::model::shell_state::ShellModel;
use crate::model::teams::{MemoryEntry, MemoryTab, Team, TeamId, TeamPage};
use gpui::{
    AnyElement, App, IntoElement, ParentElement, Styled, WeakEntity, Window, div, prelude::*, px,
};
use massorca_ui::components::empty_state::EmptyState;
use massorca_ui::components::icon_button::IconButton;
use massorca_ui::components::list_row::ListRow;
use massorca_ui::components::segmented_control::{Segment, SegmentedControl};
use massorca_ui::tokens::{SPACE_2, SPACE_4, SPACE_6, TYPE_BODY_SM, TYPE_HEADING};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};

/// The Memory page of one team (4.8).
pub(in crate::shell) fn memory_page(
    model: &ShellModel,
    shell: &WeakEntity<Shell>,
    team: TeamId,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let theme = *cx.theme();
    let Some(team_data) = model.teams().team(team) else {
        return note("This team is gone.", &theme).into_any_element();
    };
    let tab = model.teams().memory_tab();
    let header = div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_style(TYPE_HEADING)
                .text_color(theme.palette.text.primary)
                .child("Memory"),
        )
        .child(team_pill(
            TeamPage::Memory(team),
            &team_data.name,
            team_data.character,
            shell,
            window,
            cx,
        ));
    let entries: Vec<&MemoryEntry> = team_data
        .memory
        .iter()
        .filter(|entry| entry.tab == tab)
        .collect();
    let body = if entries.is_empty() {
        EmptyState::new("memory-empty", "Nothing remembered here yet")
            .explanation("What the team remembers shows here. Bots add notes when they learn something that helps later work.")
            .into_any_element()
    } else {
        let mut list = div().flex().flex_col().gap(px(SPACE_2));
        for entry in entries {
            list = list.child(entry_row(entry, team, team_data, shell));
        }
        list.into_any_element()
    };
    div()
        .id("memory-page")
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .p(px(SPACE_6))
        .size_full()
        .overflow_y_scroll()
        .child(header)
        .child(tabs(shell, tab))
        .child(body)
        .child(note(
            "Only the team's bots and you can read this memory.",
            &theme,
        ))
        .into_any_element()
}

/// The Bot and Team tabs; the choice stays while the page is open.
fn tabs(shell: &WeakEntity<Shell>, tab: MemoryTab) -> impl IntoElement {
    let shell = shell.clone();
    let selected = match tab {
        MemoryTab::Bot => 0,
        MemoryTab::Team => 1,
    };
    SegmentedControl::new(
        "memory-tabs",
        "Memory",
        vec![Segment::text("Bot"), Segment::text("Team")],
    )
    .selected(selected)
    .on_change(move |index, _, cx| {
        let tab = if index == 0 {
            MemoryTab::Bot
        } else {
            MemoryTab::Team
        };
        let _ = shell.update(cx, |shell, cx| shell.set_memory_tab(tab, cx));
    })
}

/// One entry: its sentence, who wrote it and when, and a delete that asks first.
fn entry_row(
    entry: &MemoryEntry,
    team: TeamId,
    team_data: &Team,
    shell: &WeakEntity<Shell>,
) -> impl IntoElement {
    let author = entry
        .author
        .and_then(|id| team_data.bot(id))
        .map_or_else(|| "the team".to_owned(), |bot| bot.name.clone());
    let delete = {
        let shell = shell.clone();
        let memory = entry.id;
        IconButton::new(
            format!("memory-delete-{}", entry.id.0),
            IconName::Trash2,
            "Delete memory",
        )
        .on_click(move |_, cx| {
            let confirm = TeamConfirm::DeleteMemory { team, memory };
            let _ = shell.update(cx, |shell, cx| shell.open_team_confirm(confirm, cx));
        })
    };
    ListRow::new(format!("memory-{}", entry.id.0), entry.text.clone())
        .secondary(format!("From {author}, {}", entry.date))
        .trailing(delete)
}

fn note(text: &str, theme: &Theme) -> impl IntoElement {
    div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.tertiary)
        .child(text.to_owned())
}

impl Shell {
    /// Shows the Bot or the Team tab of the memory.
    pub(super) fn set_memory_tab(&mut self, tab: MemoryTab, cx: &mut gpui::Context<Self>) {
        self.model.teams_mut().set_memory_tab(tab);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::SampleDataSource;
    use crate::model::teams::{MemoryId, SampleTeamsSource, TeamPage};
    use gpui::{Entity, TestAppContext, VisualTestContext};
    use massorca_ui::{Appearance, Theme as UiTheme};

    fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
        cx.update(|cx| {
            UiTheme::set(Appearance::Dark, cx);
            crate::shell::bind_keys(cx);
        });
        cx.add_window_view(|window, cx| {
            Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx)
        })
    }

    #[gpui::test]
    fn a_memory_delete_waits_for_its_confirm_and_then_forgets_the_entry(cx: &mut TestAppContext) {
        let (shell, cx) = shell(cx);
        shell.update(cx, |shell, cx| {
            shell.model.press_mode_switch();
            shell.model.open_team_page(TeamPage::Memory(TeamId(1)));
            shell.set_memory_tab(MemoryTab::Bot, cx);
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let count = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
            shell.read_with(cx, |shell, _| {
                shell.model().teams().team(TeamId(1)).unwrap().memory.len()
            })
        };
        assert_eq!(count(&shell, cx), 2);
        shell.update(cx, |shell, cx| {
            shell.open_team_confirm(
                TeamConfirm::DeleteMemory {
                    team: TeamId(1),
                    memory: MemoryId(2),
                },
                cx,
            );
        });
        assert_eq!(
            count(&shell, cx),
            2,
            "nothing is deleted before the confirm"
        );
        shell.update(cx, |shell, cx| shell.confirm_team_delete(cx));
        assert_eq!(count(&shell, cx), 1);
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
}
