//! The terminal surface in the gallery: a recorded shell session, replayed through
//! its feed (docs/design/components/TerminalSurface.md).

use gpui::{AnyElement, App, ParentElement, Styled, Window, div, px};

use crate::assets::Assets;
use crate::components::terminal_surface::TerminalSurface;
use crate::components::terminal_surface::feed::FeedEvent;
use crate::gallery::{Story, variant};
use crate::tokens::RADIUS_MD;

/// The recorded session in the bundled assets (`crates/ui/assets/terminal`).
const SESSION: &str = "terminal/recorded-session.vt";
/// The grid the surface starts with; it refits to the pane on the first paint.
const COLUMNS: u16 = 80;
const LINES: u16 = 18;
/// The pane the session is shown in, in logical pixels.
const PANE_WIDTH: f32 = 760.0;
const PANE_HEIGHT: f32 = 288.0;

pub fn stories() -> Vec<Story> {
    vec![Story {
        name: "Terminal surface",
        spec: "TerminalSurface.md",
        render: terminal_story,
    }]
}

fn terminal_story(window: &mut Window, cx: &mut App) -> AnyElement {
    let surface = window.use_keyed_state("gallery-terminal-surface", cx, |_, cx| {
        let mut surface = TerminalSurface::new(COLUMNS, LINES, cx);
        let session = Assets::get(SESSION).unwrap_or_default();
        let _ = surface.apply(FeedEvent::Attached { offset: 0 }, cx);
        let _ = surface.apply(
            FeedEvent::Output {
                offset: 0,
                bytes: session,
            },
            cx,
        );
        surface
    });
    variant(
        "Recorded shell session",
        div()
            .w(px(PANE_WIDTH))
            .h(px(PANE_HEIGHT))
            .rounded(px(RADIUS_MD))
            .overflow_hidden()
            .child(surface),
        cx,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::terminal_surface::feed::TerminalFeed;
    use crate::tokens::Palette;

    #[test]
    fn the_recorded_session_replays_with_its_title_and_palette_colours() {
        let session = Assets::get(SESSION).expect("the recorded session is bundled");
        let mut feed = TerminalFeed::new(COLUMNS, LINES);
        feed.apply(FeedEvent::Attached { offset: 0 })
            .expect("attach");
        feed.apply(FeedEvent::Output {
            offset: 0,
            bytes: session,
        })
        .expect("replay");
        assert_eq!(feed.title().as_deref(), Some("me@vps: ~/massorca"));
        let palette = Palette::DARK.terminal;
        let snapshot = feed.snapshot(&palette);
        let first_line: String = snapshot.cells[..snapshot.columns]
            .iter()
            .map(|cell| cell.ch)
            .collect();
        assert!(first_line.starts_with("me@vps:~/massorca$ ls --color=always"));
        assert_eq!(snapshot.cells[0].fg, palette.ansi_green);
    }
}
