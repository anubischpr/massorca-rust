//! The window's identity on Linux: the app id that X11 and Wayland shells read, and the desktop
//! entry that matches it.

/// The app id: the X11 WM_CLASS and the Wayland app_id. It names the desktop entry `massorca.desktop`.
pub const APP_ID: &str = "massorca";

#[cfg(test)]
mod tests {
    use super::APP_ID;

    const DESKTOP_ENTRY: &str = include_str!("../linux/massorca.desktop");

    #[test]
    fn the_desktop_entry_matches_the_app_id_and_names_the_icon_after_it() {
        assert!(
            DESKTOP_ENTRY
                .lines()
                .any(|line| line == format!("StartupWMClass={APP_ID}"))
        );
        assert!(
            DESKTOP_ENTRY
                .lines()
                .any(|line| line == format!("Icon={APP_ID}"))
        );
    }
}
