//! The Skills chips of the Add and Edit bot forms (D-AG-33): the skills the bot keeps, each removable, and a picker of
//! the skills this computer's service discovered (`skills.discover`). None are chosen by default.

use super::super::Shell;
use super::dialogs::TeamDialog;
use crate::model::teams::{
    NO_SKILLS_FOUND, SKILLS_FULL, discovered_skill_names, skills_to_add, with_skill, without_skill,
};
use crate::service::SkillsListing;
use gpui::{
    App, AppContext, Context, Entity, IntoElement, ParentElement, Styled, Subscription, WeakEntity,
    div, px,
};
use massorca_protocol::methods::teams::MAX_BOT_SKILLS;
use massorca_ui::components::badge::Badge;
use massorca_ui::components::button::ButtonSize;
use massorca_ui::components::icon_button::IconButton;
use massorca_ui::components::popover::OverlayLayer;
use massorca_ui::components::select::{SelectEvent, SelectView};
use massorca_ui::tokens::{SPACE_1, SPACE_2, TYPE_BODY_SM, TYPE_CAPTION};
use massorca_ui::{ActiveTheme, IconName, TextStyled};

/// The Skills field of a bot form: the chosen names and the picker that offers the discovered ones not yet chosen.
#[derive(Debug)]
pub(super) struct SkillChips {
    pub(super) chosen: Vec<String>,
    /// The discovered names; `None` until the service answers.
    discovered: Option<Vec<String>>,
    offered: Vec<String>,
    pub(super) picker: Entity<SelectView>,
    _events: Subscription,
}

impl SkillChips {
    pub(super) fn new(
        chosen: Vec<String>,
        discovered: Option<Vec<String>>,
        cx: &mut Context<Shell>,
    ) -> Self {
        let offered = skills_to_add(discovered.as_deref().unwrap_or_default(), &chosen);
        let picker = picker(&offered, chosen.len(), cx);
        Self {
            _events: on_pick(&picker, cx),
            chosen,
            discovered,
            offered,
            picker,
        }
    }

    /// Adds the offered skill at `index` and offers the rest.
    fn pick(&mut self, index: usize, cx: &mut Context<Shell>) {
        if let Some(skill) = self.offered.get(index) {
            self.chosen = with_skill(&self.chosen, skill);
            self.reoffer(cx);
        }
    }

    fn remove(&mut self, skill: &str, cx: &mut Context<Shell>) {
        self.chosen = without_skill(&self.chosen, skill);
        self.reoffer(cx);
    }

    /// Takes a newer discovery, so a form opened before the service answered offers its skills.
    fn follow(&mut self, discovered: Option<Vec<String>>, cx: &mut Context<Shell>) {
        if discovered.is_some() && discovered != self.discovered {
            self.discovered = discovered;
            self.reoffer(cx);
        }
    }

    fn reoffer(&mut self, cx: &mut Context<Shell>) {
        self.offered = skills_to_add(self.discovered.as_deref().unwrap_or_default(), &self.chosen);
        self.picker = picker(&self.offered, self.chosen.len(), cx);
        self._events = on_pick(&self.picker, cx);
    }
}

impl Shell {
    /// The skill names this computer's service discovered, when it has answered with them.
    pub(super) fn discovered_skills(&self, cx: &App) -> Option<Vec<String>> {
        match &self.store.read(cx).state().skills.get(&None)?.listing {
            SkillsListing::Found(discovery) => Some(discovered_skill_names(discovery)),
            SkillsListing::UpdateNeeded | SkillsListing::Refused(_) => None,
        }
    }

    /// Asks this computer's service for its skills when none are held, under the Marketplace's current request so its
    /// own read is not disturbed.
    pub(super) fn ask_skills_once(&self, cx: &App) {
        if self.store.read(cx).state().skills.contains_key(&None) {
            return;
        }
        if let Some(service) = &self.service {
            service.read_skills(self.marketplace_skills_request, false);
        }
    }

    /// Offers the newest discovery to an open bot form.
    pub(in crate::shell) fn follow_bot_skills(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.team_dialog, Some(TeamDialog::AddBot(_))) {
            return;
        }
        let discovered = self.discovered_skills(cx);
        if let Some(TeamDialog::AddBot(dialog)) = &mut self.team_dialog {
            dialog.skills.follow(discovered, cx);
        }
    }

    fn pick_bot_skill(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(TeamDialog::AddBot(dialog)) = &mut self.team_dialog {
            dialog.skills.pick(index, cx);
        }
        cx.notify();
    }

    fn remove_bot_skill(&mut self, skill: &str, cx: &mut Context<Self>) {
        if let Some(TeamDialog::AddBot(dialog)) = &mut self.team_dialog {
            dialog.skills.remove(skill, cx);
        }
        cx.notify();
    }
}

/// "Skills": a chip per chosen skill with its remove button, then the picker.
pub(super) fn skills_view(
    skills: &SkillChips,
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> impl IntoElement {
    let theme = *cx.theme();
    let label = div()
        .text_style(TYPE_CAPTION)
        .text_color(theme.palette.text.secondary)
        .child("Skills");
    let chips =
        div()
            .flex()
            .flex_wrap()
            .gap(px(SPACE_2))
            .children(skills.chosen.iter().enumerate().map(|(index, skill)| {
                let (shell, name) = (shell.clone(), skill.clone());
                div()
                    .flex()
                    .items_center()
                    .gap(px(SPACE_1))
                    .child(Badge::tag(skill.clone()))
                    .child(
                        IconButton::new(
                            format!("bot-skill-remove-{index}"),
                            IconName::X,
                            format!("Remove {skill}"),
                        )
                        .size(ButtonSize::Compact)
                        .on_click(move |_, cx| {
                            let _ = shell.update(cx, |shell, cx| shell.remove_bot_skill(&name, cx));
                        }),
                    )
            }));
    let note = skills.offered.is_empty().then(|| {
        let reason = if skills.chosen.len() >= MAX_BOT_SKILLS {
            SKILLS_FULL
        } else {
            NO_SKILLS_FOUND
        };
        div()
            .text_style(TYPE_BODY_SM)
            .text_color(theme.palette.text.secondary)
            .child(reason)
    });
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .child(label)
        .child(chips)
        .child(skills.picker.clone())
        .children(note)
}

/// The picker over `offered`; it is off with the reason when nothing is offered or the bot keeps the most it may.
fn picker(offered: &[String], chosen: usize, cx: &mut Context<Shell>) -> Entity<SelectView> {
    let full = chosen >= MAX_BOT_SKILLS;
    cx.new(|cx| {
        SelectView::new(offered.to_vec(), cx)
            .placeholder("Add a skill")
            .empty_text(if full { SKILLS_FULL } else { NO_SKILLS_FOUND })
            .selected(None)
            .disabled(full || offered.is_empty())
            .layer(OverlayLayer::Dialog)
    })
}

fn on_pick(picker: &Entity<SelectView>, cx: &mut Context<Shell>) -> Subscription {
    cx.subscribe(picker, |shell, _, event: &SelectEvent, cx| match event {
        SelectEvent::Changed(index) => shell.pick_bot_skill(*index, cx),
    })
}
