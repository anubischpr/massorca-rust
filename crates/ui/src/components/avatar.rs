//! A round mark for an agent or a team, with an optional status glyph at its bottom right
//! (Avatar.md, A16, A57). The agent's own icon is drawn in full colour; a team has no icon of its
//! own yet, so it shows the team glyph on the team fill.

use crate::components::agent_icon::{AgentIcon, AgentIconSize, AgentKind};
use crate::components::agent_status::{AgentState, AgentStatus, AgentStatusSize};
use crate::icon::{Icon, IconName, IconSize};
use crate::theme::{ActiveTheme, Theme};
use crate::tokens::{BORDER_WIDTH_THICK, SIZE_AVATAR_LG, SIZE_AVATAR_MD, SIZE_AVATAR_SM};
use gpui::{
    AnyElement, App, ElementId, IntoElement, ParentElement, RenderOnce, Role, SharedString, Styled,
    Window, div, prelude::*, px,
};
use std::fmt;

/// The three Avatar sizes: 20 px in lists and the status bar, 28 px in rows and cards, 40 px in
/// headers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AvatarSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl AvatarSize {
    pub fn px(self) -> f32 {
        match self {
            AvatarSize::Small => SIZE_AVATAR_SM,
            AvatarSize::Medium => SIZE_AVATAR_MD,
            AvatarSize::Large => SIZE_AVATAR_LG,
        }
    }

    fn agent_size(self) -> AgentIconSize {
        match self {
            AvatarSize::Small => AgentIconSize::Name,
            AvatarSize::Medium => AgentIconSize::Medium,
            AvatarSize::Large => AgentIconSize::Large,
        }
    }

    fn status_size(self) -> AgentStatusSize {
        match self {
            AvatarSize::Small => AgentStatusSize::XSmall,
            AvatarSize::Medium | AvatarSize::Large => AgentStatusSize::Medium,
        }
    }
}

/// What the mark stands for: an agent with its own icon, or a team.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AvatarKind {
    Agent(AgentKind),
    Team,
}

/// A mark with its accessible name ("Claude, Running" when it carries a status).
#[derive(IntoElement)]
pub struct Avatar {
    id: ElementId,
    kind: AvatarKind,
    size: AvatarSize,
    name: SharedString,
    status: Option<AgentState>,
    selected: bool,
}

impl fmt::Debug for Avatar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Avatar")
            .field("kind", &self.kind)
            .field("size", &self.size)
            .field("name", &self.name)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

impl Avatar {
    pub fn new(id: impl Into<ElementId>, kind: AvatarKind, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            kind,
            size: AvatarSize::default(),
            name: name.into(),
            status: None,
            selected: false,
        }
    }

    pub fn agent(id: impl Into<ElementId>, kind: AgentKind, name: impl Into<SharedString>) -> Self {
        Self::new(id, AvatarKind::Agent(kind), name)
    }

    pub fn team(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        Self::new(id, AvatarKind::Team, name)
    }

    pub fn size(mut self, size: AvatarSize) -> Self {
        self.size = size;
        self
    }

    /// The status glyph at the bottom right; its word joins the accessible name.
    pub fn status(mut self, state: AgentState) -> Self {
        self.status = Some(state);
        self
    }

    /// A 2 px accent ring outside the circle, for the chosen mark.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The name a screen reader reads, with the status word when a glyph is shown.
    pub fn accessible_name(&self) -> String {
        match self.status {
            Some(state) => format!("{}, {}", self.name, state.label()),
            None => self.name.to_string(),
        }
    }
}

impl RenderOnce for Avatar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = &theme.palette;
        let name = self.accessible_name();
        let mark = match self.kind {
            AvatarKind::Agent(kind) => AgentIcon::new(kind)
                .size(self.size.agent_size())
                .into_any_element(),
            AvatarKind::Team => team_mark(self.size, &theme),
        };
        let fill = match self.kind {
            AvatarKind::Agent(_) => p.surface.raised,
            AvatarKind::Team => p.accent.subtle_raised,
        };
        let mut circle = div()
            .id(self.id.clone())
            .role(Role::Image)
            .aria_label(name)
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(self.size.px()))
            .rounded_full()
            .bg(fill)
            .child(mark);
        if matches!(self.kind, AvatarKind::Agent(_)) {
            circle = circle.border_1().border_color(p.border.default);
        }
        if self.selected {
            circle = circle
                .border(px(BORDER_WIDTH_THICK))
                .border_color(p.accent.r#default);
        }
        if let Some(state) = self.status {
            circle = circle.child(
                div()
                    .absolute()
                    .bottom(px(-2.))
                    .right(px(-2.))
                    .child(AgentStatus::new(state).size(self.size.status_size())),
            );
        }
        circle
    }
}

/// The team glyph on the team fill, sized with the mark's share of the circle.
fn team_mark(size: AvatarSize, theme: &Theme) -> AnyElement {
    let icon = match size {
        AvatarSize::Small => IconSize::XSmall,
        AvatarSize::Medium => IconSize::Medium,
        AvatarSize::Large => IconSize::Large,
    };
    Icon::new(IconName::Users)
        .size(icon)
        .color(theme.palette.accent.fg)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_status_joins_the_name_the_screen_reader_reads() {
        let claude =
            Avatar::agent("claude", AgentKind::Claude, "Claude").status(AgentState::Running);
        assert_eq!(claude.accessible_name(), "Claude, Running");
        assert_eq!(
            Avatar::team("team", "Desarrollo").accessible_name(),
            "Desarrollo"
        );
    }
}
