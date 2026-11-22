//! Debug only: a background launch with `MASSORCA_DESKTOP_DEBUG_FOCUS` puts the focus in a field
//! and logs what it settles on, so an automated check can type into the field without activating
//! the window.

use super::Shell;
use crate::launch_options::DebugFocus;
use gpui::{Context, Window};
use massorca_ui::components::search_field::SearchFieldEvent;
use std::io::Write;

impl Shell {
    /// Focuses `field` in the next frame, before it is drawn, and logs each query the search field
    /// settles on to stderr.
    pub fn start_debug_input(
        &mut self,
        field: DebugFocus,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match field {
            DebugFocus::Search => {
                self._debug_input = Some(cx.subscribe(
                    &self.search,
                    |_, _, event: &SearchFieldEvent, _| {
                        if let SearchFieldEvent::Search(query) = event {
                            let _ = writeln!(
                                std::io::stderr(),
                                "massorca-desktop: debug search query={query:?}"
                            );
                        }
                    },
                ));
                let focus = self.search.read(cx).focus_handle(cx);
                window.on_next_frame(move |window, cx| window.focus(&focus, cx));
            }
        }
    }
}
