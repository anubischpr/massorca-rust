//! A folder or file field with "Browse…" beside it. The system's picker fills the field, and typing a path keeps
//! working. A folder on another host gets no Browse until a remote folder browser comes.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, WeakEntity, Window, div, px};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::text_field::TextField;
use massorca_ui::tokens::SPACE_2;

use super::Shell;

/// Whether the picker chooses a folder or a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PickKind {
    Folder,
    File,
}

/// The path the person picks, or `None` when the picker is cancelled.
pub(super) type PickedPath = Pin<Box<dyn Future<Output = Option<PathBuf>>>>;

/// The function a picker is: it takes what to pick and answers with the path, or `None` when cancelled.
type PickFn = dyn Fn(PickKind, &mut App) -> PickedPath;

/// The system's picker, held by the shell. Tests replace it with a fake, so no dialog opens during a test.
#[derive(Clone)]
pub(super) struct PathPicker(Rc<PickFn>);

impl PathPicker {
    pub(super) fn new(picker: impl Fn(PickKind, &mut App) -> PickedPath + 'static) -> Self {
        Self(Rc::new(picker))
    }

    pub(super) fn pick(&self, kind: PickKind, cx: &mut App) -> PickedPath {
        (self.0)(kind, cx)
    }
}

impl std::fmt::Debug for PathPicker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PathPicker")
    }
}

/// GPUI's native prompt, which answers with the first path chosen.
#[cfg(not(test))]
pub(super) fn system_picker() -> PathPicker {
    PathPicker::new(|kind: PickKind, cx: &mut App| -> PickedPath {
        let options = gpui::PathPromptOptions {
            files: kind == PickKind::File,
            directories: kind == PickKind::Folder,
            multiple: false,
            prompt: None,
        };
        let picked = cx.prompt_for_paths(options);
        Box::pin(async move {
            match picked.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                _ => None,
            }
        })
    })
}

/// Tests never reach the system's dialog: until a test sets a fake, the picker is cancelled.
#[cfg(test)]
pub(super) fn system_picker() -> PathPicker {
    PathPicker::new(|_kind: PickKind, _cx: &mut App| -> PickedPath {
        Box::pin(async { None::<PathBuf> })
    })
}

impl Shell {
    /// Asks the picker for a path and fills `field` with it, then focuses the field so Enter takes the path. A
    /// cancelled picker changes nothing.
    pub(super) fn browse_into(
        &mut self,
        field: Entity<TextField>,
        kind: PickKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let picked = self.path_picker.pick(kind, cx);
        window
            .spawn(cx, async move |cx| {
                let Some(path) = picked.await else {
                    return;
                };
                let text = path.to_string_lossy().into_owned();
                let _ = field.update_in(cx, |field, window, cx| {
                    field.set_text(text, window, cx);
                    field.focus(window, cx);
                });
            })
            .detach();
    }
}

/// The text field with "Browse…" beside it while `on_this_pc`, and that button off while `enabled` is false. `id` names
/// the button and `field_id` the field's box, for tests.
pub(super) fn path_field(
    id: &'static str,
    field_id: &'static str,
    shell: WeakEntity<Shell>,
    field: &Entity<TextField>,
    kind: PickKind,
    on_this_pc: bool,
    enabled: bool,
) -> AnyElement {
    let row = div()
        .w_full()
        .flex()
        .flex_row()
        .items_end()
        .gap(px(SPACE_2))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .debug_selector(move || field_id.to_owned())
                .child(field.clone()),
        );
    if !on_this_pc {
        return row.into_any_element();
    }
    let target = field.clone();
    row.child(
        Button::new(id, "Browse\u{2026}")
            .variant(ButtonVariant::Secondary)
            .disabled(!enabled)
            .on_click(move |window, cx| {
                let _ = shell.update(cx, |shell, cx| {
                    shell.browse_into(target.clone(), kind, window, cx);
                });
            }),
    )
    .into_any_element()
}

/// The kinds a fake picker was asked for, in order.
#[cfg(test)]
pub(super) type Asked = Rc<std::cell::RefCell<Vec<PickKind>>>;

/// A picker that records what it is asked and answers `answer`, so a test sees the field change without a dialog.
#[cfg(test)]
pub(super) fn answering(answer: Option<&str>) -> (Asked, PathPicker) {
    let asked: Asked = Rc::default();
    let (record, answer) = (asked.clone(), answer.map(PathBuf::from));
    let picker = PathPicker::new(move |kind: PickKind, _cx: &mut App| -> PickedPath {
        record.borrow_mut().push(kind);
        let answer = answer.clone();
        Box::pin(async move { answer })
    });
    (asked, picker)
}

#[cfg(test)]
impl Shell {
    /// Replaces the system's picker, so a test answers without opening a dialog.
    pub(super) fn set_path_picker(&mut self, picker: PathPicker) {
        self.path_picker = picker;
    }
}
