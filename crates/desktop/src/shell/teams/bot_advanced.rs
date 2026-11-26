//! The Advanced options of the Add and Edit bot forms: a collapsed disclosure over the bot's Model and Effort, offered
//! from `agent.kinds` (decision 0096). A select left on "Picked from the job" sends nothing.

use super::super::Shell;
use super::dialogs::TeamDialog;
use crate::model::teams::{KindOptions, OptionRow, effort_rows, model_rows, options_of, row_index};
use gpui::{
    App, AppContext, Context, Entity, IntoElement, ParentElement, Styled, Subscription, WeakEntity,
    div, px,
};
use massorca_ui::IconName;
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::popover::OverlayLayer;
use massorca_ui::components::select::{SelectEvent, SelectView};
use massorca_ui::tokens::SPACE_3;

/// Why a stored model or effort the kind no longer offers stays on the form and cannot be chosen again.
const NOT_OFFERED: &str = "Not offered by this agent.";

/// The Advanced options of a bot form: whether they show, and the Model and Effort selects with the rows they offer.
#[derive(Debug)]
pub(super) struct AdvancedOptions {
    /// Whether the disclosure shows its selects; collapsed when the form opens.
    pub(super) open: bool,
    catalogue: Vec<KindOptions>,
    kind: String,
    pub(super) model: Entity<SelectView>,
    model_rows: Vec<OptionRow>,
    pub(super) effort: Entity<SelectView>,
    effort_rows: Vec<OptionRow>,
    _model_events: Subscription,
}

impl AdvancedOptions {
    /// The options of `kind`, opened on the bot's stored `model` and `effort`; a new bot starts on the job.
    pub(super) fn new(
        catalogue: Vec<KindOptions>,
        kind: &str,
        model: Option<&str>,
        effort: Option<&str>,
        cx: &mut Context<Shell>,
    ) -> Self {
        let options = options_of(&catalogue, kind);
        let models = model_rows(options, model);
        let levels = effort_rows(options, model, effort);
        let model_select = select("Model", &models, row_index(&models, model), cx);
        let effort_select = select("Effort", &levels, row_index(&levels, effort), cx);
        Self {
            open: false,
            catalogue,
            kind: kind.to_owned(),
            model: model_select.clone(),
            model_rows: models,
            effort: effort_select,
            effort_rows: levels,
            _model_events: on_model_changed(&model_select, cx),
        }
    }

    /// Re-offers the efforts the model at `index` accepts, keeping the effort when that model still offers it.
    pub(super) fn choose_model(&mut self, index: usize, cx: &mut Context<Shell>) {
        let model = value_at(&self.model_rows, Some(index));
        let effort = value_at(&self.effort_rows, self.effort.read(cx).selected_index());
        self.offer_efforts(model.as_deref(), effort, cx);
    }

    /// Offers the models and efforts of `kind`, keeping the model and effort where the kind still offers them.
    pub(super) fn choose_kind(&mut self, kind: &str, cx: &mut Context<Shell>) {
        let model = value_at(&self.model_rows, self.model.read(cx).selected_index());
        let effort = value_at(&self.effort_rows, self.effort.read(cx).selected_index());
        let models = model_rows(options_of(&self.catalogue, kind), None);
        let index = row_index(&models, model.as_deref());
        let kept = value_at(&models, Some(index));
        self.kind = kind.to_owned();
        self.model = select("Model", &models, index, cx);
        self.model_rows = models;
        self._model_events = on_model_changed(&self.model, cx);
        self.offer_efforts(kept.as_deref(), effort, cx);
    }

    /// The model and effort the form sends; a select on the job's row sends nothing for its value.
    pub(super) fn values(&self, cx: &App) -> (Option<String>, Option<String>) {
        (
            value_at(&self.model_rows, self.model.read(cx).selected_index()),
            value_at(&self.effort_rows, self.effort.read(cx).selected_index()),
        )
    }

    fn offer_efforts(
        &mut self,
        model: Option<&str>,
        effort: Option<String>,
        cx: &mut Context<Shell>,
    ) {
        let levels = effort_rows(options_of(&self.catalogue, &self.kind), model, None);
        let index = row_index(&levels, effort.as_deref());
        self.effort = select("Effort", &levels, index, cx);
        self.effort_rows = levels;
    }
}

impl Shell {
    /// Opens or closes the Advanced options of the bot form.
    pub(super) fn toggle_bot_advanced(&mut self, cx: &mut Context<Self>) {
        if let Some(TeamDialog::AddBot(dialog)) = &mut self.team_dialog {
            dialog.advanced.open = !dialog.advanced.open;
        }
        cx.notify();
    }

    /// Chooses the model at `index` of the bot form's Model select.
    pub(super) fn set_add_bot_model(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(TeamDialog::AddBot(dialog)) = &mut self.team_dialog {
            dialog.advanced.choose_model(index, cx);
        }
        cx.notify();
    }
}

/// The disclosure, with its Model and Effort selects below it while it is open.
pub(super) fn advanced_view(
    advanced: &AdvancedOptions,
    shell: &WeakEntity<Shell>,
) -> impl IntoElement {
    let toggle = {
        let shell = shell.clone();
        let chevron = if advanced.open {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        };
        Button::new("bot-form-advanced", "Advanced options")
            .variant(ButtonVariant::Ghost)
            .trailing_icon(chevron)
            .on_click(move |_, cx| {
                let _ = shell.update(cx, |shell, cx| shell.toggle_bot_advanced(cx));
            })
    };
    let section = div().flex().flex_col().gap(px(SPACE_3)).child(toggle);
    if advanced.open {
        section
            .child(advanced.model.clone())
            .child(advanced.effort.clone())
    } else {
        section
    }
}

/// A Model or Effort select over `rows`, showing row `selected`; a row the kind no longer offers stays unavailable.
fn select(
    label: &'static str,
    rows: &[OptionRow],
    selected: usize,
    cx: &mut Context<Shell>,
) -> Entity<SelectView> {
    let labels: Vec<String> = rows.iter().map(|row| row.label.clone()).collect();
    cx.new(|cx| {
        let mut view = SelectView::new(labels, cx)
            .label(label)
            .selected(Some(selected))
            .disabled(rows.len() == 1)
            .layer(OverlayLayer::Dialog);
        for (index, _) in rows.iter().enumerate().filter(|(_, row)| !row.available) {
            view = view.option_unavailable(index, NOT_OFFERED);
        }
        view
    })
}

/// A model picked in the Model select re-offers the efforts it accepts.
fn on_model_changed(model: &Entity<SelectView>, cx: &mut Context<Shell>) -> Subscription {
    cx.subscribe(model, |shell, _, event: &SelectEvent, cx| match event {
        SelectEvent::Changed(index) => shell.set_add_bot_model(*index, cx),
    })
}

/// The value of the row at `index`; a missing row or the job's row has none.
fn value_at(rows: &[OptionRow], index: Option<usize>) -> Option<String> {
    index
        .and_then(|index| rows.get(index))
        .and_then(|row| row.value.clone())
}
