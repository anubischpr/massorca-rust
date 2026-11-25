//! The Canvas: the Conversation, the regions beside it and the overlay region (D-SH-01, A5).

use std::cell::Cell;
use std::rc::Rc;

use super::Shell;
use super::home_start::{self, HomeFrame};
use super::marketplace::{self, MarketplaceFrame};
use super::pairing::{self, PairingFrame};
use super::session_view::{self, SessionFrame};
use super::settings::{self, SettingsFrame};
use super::setup;
use super::start_page::{self, StartFrame};
use super::teams;
use super::work_panel::{self, WorkFrame};
use super::zones::{ZoneFocus, region_focus_ring};
use crate::model::canvas::Canvas;
use crate::model::navigation::{Mode, Page};
use crate::model::region_cycle::Zone;
use crate::model::regions::Region;
use crate::model::shell::{ServiceNotice, ServiceTone};
use crate::model::shell_state::ShellModel;
use crate::model::teams::{BoardOwner, TeamPage};
use gpui::{
    AnyElement, App, Context, Div, FocusHandle, IntoElement, MouseButton, WeakEntity, Window, div,
    prelude::*, px,
};
use massorca_ui::components::error_state::{ErrorState, ErrorStateVariant};
use massorca_ui::components::rose_logo::rose_logo;
use massorca_ui::theme::box_shadows;
use massorca_ui::tokens::{
    LAYOUT_CONVERSATION_MIN, LAYOUT_CONVERSATION_READING, RADIUS_CANVAS, RADIUS_LG, SIZE_TOOLBAR,
    SPACE_1, SPACE_3, SPACE_4, TYPE_BODY_SM, TYPE_HEADING, TYPE_TITLE_SM,
};
use massorca_ui::{ActiveTheme, TextStyled, Theme};

/// The Canvas: the Conversation and the inline regions, with an open overlay on top (D-SH-01).
/// What the full pages and the start page draw from, gathered by the shell for one frame.
pub(super) struct PageFrames<'a> {
    pub settings: SettingsFrame<'a>,
    pub start: Option<StartFrame>,
    /// The start page with no workspace, while a service can start sessions.
    pub home: Option<HomeFrame>,
    pub setup_rows: Vec<crate::model::setup::SetupRow>,
    /// The Work panel, while its region is on screen.
    pub work_panel: Option<WorkFrame>,
    /// The body of the Project sidebar, when it is on screen.
    pub project: Option<AnyElement>,
    /// The diff of the file open from Source control, which takes the Canvas over the start page.
    pub diff: Option<AnyElement>,
    /// The Marketplace page's frame, while a Marketplace view or item page is on screen.
    pub marketplace: Option<MarketplaceFrame>,
    /// The Pairing page's state and fields (D-PR-01).
    pub pairing: PairingFrame,
    /// The open session's view, while the Canvas shows a session.
    pub session: Option<SessionFrame>,
    /// The Conference room, while the Canvas shows one.
    pub room: Option<teams::RoomFrame>,
}

pub(super) fn canvas(
    model: &ShellModel,
    zones: &ZoneFocus,
    mut pages: PageFrames<'_>,
    conversation_size: Rc<Cell<Option<(f32, f32)>>>,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> Div {
    let theme = *cx.theme();
    let overlay = model.overlay().filter(|region| *region != Region::Sidebar);
    let shell = cx.entity().downgrade();
    let mut work = pages.work_panel.take();
    let mut project = pages.project.take();
    let surface = div()
        .debug_selector(|| "canvas".to_owned())
        .track_focus(zones.get(Zone::Conversation))
        .focus_visible(|style| style.shadow(region_focus_ring(&theme)))
        .relative()
        .flex()
        .flex_row()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .bg(theme.palette.surface.canvas)
        .rounded_tl(px(RADIUS_CANVAS))
        .overflow_hidden()
        .child(conversation(
            model,
            &shell,
            pages,
            conversation_size,
            window,
            cx,
            &theme,
        ))
        .children(model.inline_regions().into_iter().map(|region| {
            let body = region_body(region, &mut work, &mut project, cx);
            region_column(
                region,
                zones.get(Zone::of_region(region)).clone(),
                &theme,
                body,
            )
        }));
    surface.children(overlay.map(|region| {
        let body = region_body(region, &mut work, &mut project, cx);
        overlay_panel(
            region,
            zones.get(Zone::of_region(region)).clone(),
            &theme,
            body,
        )
    }))
}

/// The body of a region, taken once: the Work panel's pane or the Project sidebar's body, and
/// nothing for the other regions.
fn region_body(
    region: Region,
    work: &mut Option<WorkFrame>,
    project: &mut Option<AnyElement>,
    cx: &mut App,
) -> Option<AnyElement> {
    match region {
        Region::WorkPanel => work.take().map(|frame| work_panel::work_panel(frame, cx)),
        Region::ProjectSidebar => project.take(),
        _ => None,
    }
}

fn conversation(
    model: &ShellModel,
    shell: &WeakEntity<Shell>,
    pages: PageFrames<'_>,
    conversation_size: Rc<Cell<Option<(f32, f32)>>>,
    window: &mut Window,
    cx: &mut App,
    theme: &Theme,
) -> AnyElement {
    // The Settings page fills the Canvas, so it is not centred in a box.
    if model.canvas() == Canvas::Page(Page::Settings) {
        return settings::settings_page(pages.settings, window, cx);
    }
    if model.canvas() == Canvas::Page(Page::Setup) {
        return setup::setup_page(&pages.setup_rows, cx);
    }
    if model.canvas() == Canvas::Page(Page::Pairing) {
        return pairing::pairing_page(&pages.pairing, cx);
    }
    // The Marketplace fills the Canvas too, with its own navigation column (D-EX-01).
    if let (Canvas::Page(Page::Marketplace(_)), Some(frame)) = (model.canvas(), pages.marketplace) {
        return marketplace::page(frame, window, cx);
    }
    // A service that cannot be reached takes the Canvas: its page, not a banner over an empty one.
    if let Some(notice) = model
        .snapshot()
        .service_notice
        .as_ref()
        .filter(|notice| notice.tone == ServiceTone::Unreachable)
    {
        return service_down_page(notice, shell);
    }
    // The start page is laid out from the top, so it is not centred in a box either.
    if let Canvas::Workspace(_) = model.canvas() {
        if let Some(diff) = pages.diff {
            return diff;
        }
        return match pages.start {
            Some(frame) => start_page::start_page(frame, window, cx),
            None => placeholder(
                "Workspace",
                "This workspace is not listed by the service.",
                theme,
            )
            .into_any_element(),
        };
    }
    let body: AnyElement = match model.canvas() {
        Canvas::Home(Mode::Workspaces) => match pages.home {
            Some(frame) => home_start::home_page(frame, window, cx),
            None => start_page(theme).into_any_element(),
        },
        Canvas::Home(Mode::BotTeams) => teams::bot_teams_home(model, shell, cx),
        Canvas::Team(TeamPage::Board(team)) => {
            teams::board_page(model, shell, BoardOwner::Team(team), window, cx)
        }
        Canvas::Team(TeamPage::Project(team, project)) => {
            teams::project_page(model, shell, team, project, window, cx)
        }
        Canvas::Team(TeamPage::Routines(team)) => {
            teams::routines_page(model, shell, team, window, cx)
        }
        Canvas::Team(TeamPage::Memory(team)) => teams::memory_page(model, shell, team, window, cx),
        Canvas::Team(TeamPage::Files(team)) => match model.teams().team(team) {
            Some(team) => teams::files_page(team, shell, window, cx),
            None => placeholder("Files", "This team is gone.", theme).into_any_element(),
        },
        Canvas::Team(TeamPage::Conference(_)) => match pages.room {
            Some(frame) => teams::conference_page(frame, window, cx),
            None => placeholder("Conference room", "This team is gone.", theme).into_any_element(),
        },
        Canvas::Session(_) => match pages.session {
            Some(frame) => session_view::session_page(frame, cx),
            None => placeholder(
                "Session",
                "This session is not listed by the service.",
                theme,
            )
            .into_any_element(),
        },
        Canvas::Team(page) => {
            placeholder(page.title(), "This page is not built yet.", theme).into_any_element()
        }
        // Drawn above: the start page is not centred in the box below.
        Canvas::Workspace(_) => placeholder(
            "Workspace",
            "This workspace is not listed by the service.",
            theme,
        )
        .into_any_element(),
        Canvas::Page(page) => {
            placeholder(page.title(), "This page is not built yet.", theme).into_any_element()
        }
    };
    div()
        .relative()
        .flex_1()
        .min_w(px(LAYOUT_CONVERSATION_MIN))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        // Records the Conversation's size as it paints; the probe takes no room.
        .child(
            gpui::canvas(
                |_, _, _| (),
                move |bounds, (), _, _| {
                    conversation_size.set(Some((
                        f32::from(bounds.size.width),
                        f32::from(bounds.size.height),
                    )));
                },
            )
            .absolute()
            .size_full(),
        )
        .child(body)
        .into_any_element()
}

/// The start page on sample data, which has no service to start with: the rose logo and the
/// question (A40).
fn start_page(theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(SPACE_3))
        .w_full()
        .max_w(px(LAYOUT_CONVERSATION_READING))
        .child(rose_logo())
        .child(
            div()
                .text_style(TYPE_HEADING)
                .text_color(theme.palette.text.primary)
                .child("What should we work on?"),
        )
}

/// A centred title and a note for a Canvas content that is not built yet.
fn placeholder(title: &str, note: &str, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(SPACE_1))
        .child(
            div()
                .text_style(TYPE_HEADING)
                .text_color(theme.palette.text.primary)
                .child(title.to_owned()),
        )
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.tertiary)
                .child(note.to_owned()),
        )
}

/// The page for a service that cannot be reached: what it is, Retry, and the start error behind
/// Show details (cross-cutting "Service not running").
fn service_down_page(notice: &ServiceNotice, shell: &WeakEntity<Shell>) -> AnyElement {
    let retry = shell.clone();
    let mut page = ErrorState::new("service-down", "MassOrca service is not running")
        .variant(ErrorStateVariant::Page)
        .explanation(
            "Projects, agents and terminals run in the local service on this PC. Retry starts it. Show details has the last start error.",
        )
        .on_retry(move |_, cx| {
            let _ = retry.update(cx, |shell, _| shell.retry_service_now());
        });
    if let Some(detail) = &notice.detail {
        page = page.details(detail.clone());
    }
    div()
        .debug_selector(|| "service-down".to_owned())
        .w_full()
        .max_w(px(LAYOUT_CONVERSATION_READING))
        .p(px(SPACE_4))
        .child(page)
        .into_any_element()
}

/// A region docked beside the Conversation, at its default width (D-SH-01).
fn region_column(
    region: Region,
    focus: FocusHandle,
    theme: &Theme,
    body: Option<AnyElement>,
) -> impl IntoElement {
    div()
        .track_focus(&focus)
        .focus_visible(|style| style.shadow(region_focus_ring(theme)))
        .flex_none()
        .w(px(region.default_width()))
        .min_w(px(region.min_width()))
        .h_full()
        .flex()
        .flex_col()
        .border_l_1()
        .border_color(theme.palette.border.subtle)
        .child(region_header(region, theme))
        .children(body)
}

/// A region open over the Canvas when the window is too narrow to fit it inline (X1).
fn overlay_panel(
    region: Region,
    focus: FocusHandle,
    theme: &Theme,
    body: Option<AnyElement>,
) -> impl IntoElement {
    div()
        .track_focus(&focus)
        .focus_visible(|style| {
            style.shadow([region_focus_ring(theme), box_shadows(theme.shadows.e2)].concat())
        })
        .absolute()
        .top_0()
        .bottom_0()
        .right_0()
        .w(px(region.default_width()))
        .flex()
        .flex_col()
        .bg(theme.palette.surface.overlay)
        .border_1()
        .border_color(theme.palette.border.default)
        .rounded_l(px(RADIUS_LG))
        .shadow(box_shadows(theme.shadows.e2))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(region_header(region, theme))
        .children(body)
}

fn region_header(region: Region, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .h(px(SIZE_TOOLBAR))
        .px(px(SPACE_3))
        .text_style(TYPE_TITLE_SM)
        .text_color(theme.palette.text.primary)
        .truncate()
        .child(region.name())
}
