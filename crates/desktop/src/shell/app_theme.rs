//! The app theme in the shell. The rail toggle and the Settings row write `appearance.theme` through
//! the service, and the theme follows what the store holds: the stored choice, with `system` taken
//! from the operating system. A refused write keeps the theme and shows the service's sentence.

use gpui::{App, Context, IntoElement, WeakEntity, WindowAppearance, div, prelude::*, px};
use massorca_protocol::methods::config::ConfigSet;
use massorca_ui::components::banner::{Banner, BannerVariant};
use massorca_ui::tokens::{SPACE_2, SPACE_3};
use massorca_ui::{ActiveTheme, Appearance, Theme};

use super::Shell;
use crate::model::app_theme::{APPEARANCE_THEME_KEY, ThemeChoice};

impl Shell {
    /// The choice the store holds for `appearance.theme`, or `None` until a read has listed it.
    pub(super) fn stored_app_theme(&self, cx: &App) -> Option<ThemeChoice> {
        ThemeChoice::from_setting(
            self.store
                .read(cx)
                .state()
                .config_value(APPEARANCE_THEME_KEY),
        )
    }

    /// Applies the stored choice to the app theme. It runs on every store change and when the
    /// operating system's appearance changes, because `system` shows that appearance.
    pub(super) fn follow_app_theme(&self, cx: &mut Context<Self>) {
        let Some(choice) = self.stored_app_theme(cx) else {
            return;
        };
        let appearance = choice.resolve(appearance_of(cx.window_appearance()));
        if cx.theme().appearance != appearance {
            Theme::set(appearance, cx);
        }
    }

    /// The rail toggle and the palette's Toggle theme write the other appearance. A shell without
    /// a service (sample data) has nothing to store it with, so it only changes its theme.
    pub(super) fn toggle_app_theme(&mut self, cx: &mut Context<Self>) {
        if self.service.is_none() {
            Theme::toggle(cx);
            return;
        }
        let choice = ThemeChoice::other_than(cx.theme().appearance);
        self.save_app_theme(choice, cx);
    }

    /// Writes the choice with the service. The theme follows only what the store reports back; a
    /// refusal keeps the theme and shows the service's sentence above the Canvas.
    pub(super) fn save_app_theme(&mut self, choice: ThemeChoice, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let result = service
                .config_set(ConfigSet {
                    owner: None,
                    key: APPEARANCE_THEME_KEY.to_owned(),
                    value: choice.word().into(),
                })
                .await;
            let _ = this.update(cx, |shell, cx| {
                shell.theme_notice = result.err().map(|refusal| refusal.sentence());
                cx.notify();
            });
        })
        .detach();
    }
}

/// The app's appearance for an operating-system appearance; the vibrant variants show the plain ones.
fn appearance_of(system: WindowAppearance) -> Appearance {
    match system {
        WindowAppearance::Light | WindowAppearance::VibrantLight => Appearance::Light,
        WindowAppearance::Dark | WindowAppearance::VibrantDark => Appearance::Dark,
    }
}

/// The notice of a refused theme write, above the Canvas. Dismissing it clears the notice.
pub(super) fn theme_notice_banner(notice: &str, shell: &WeakEntity<Shell>) -> impl IntoElement {
    let shell = shell.clone();
    let banner = Banner::new("theme-notice", BannerVariant::Warning, notice.to_owned()).on_dismiss(
        move |_, cx| {
            let _ = shell.update(cx, |shell, cx| {
                shell.theme_notice = None;
                cx.notify();
            });
        },
    );
    div().px(px(SPACE_3)).pt(px(SPACE_2)).child(banner)
}
