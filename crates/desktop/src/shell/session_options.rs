//! The model chip on a session's composer (AG-14, decision 0171): it shows the session's model, effort and Fast, and a
//! pick asks `agent.set_options` while the service offers `agent-options-live/1`. Without the capability the session
//! composer has no chip, as before.

use std::collections::{HashMap, HashSet};

use gpui::{App, Context, Entity, SharedString};
use massorca_protocol::methods::agents::AgentOptionsSet;
use massorca_ui::components::composer::Composer;
use massorca_ui::components::model_picker::Choice;

use super::Shell;
use super::start_page::{agent_brands, picker_agents};
use crate::model::agent_kinds::session_kind_choices;
use crate::model::live_options::{applied_note, options_to_set, session_choice};
use crate::service::Refusal;

/// The option changes in flight and the notes the last answers left, by session.
#[derive(Debug, Default)]
pub(super) struct LiveOptions {
    calls: HashSet<String>,
    notes: HashMap<String, String>,
}

impl LiveOptions {
    /// A resume starts a run with the kept options, so the note about them is done.
    pub(super) fn resumed(&mut self, session: &str) {
        self.notes.remove(session);
    }
}

impl Shell {
    /// Offers the session's composer the model chip of its kind, showing the session's options, while the service
    /// changes them live. The chip keeps a pick while its change is in flight; a refused pick goes back to the session's.
    pub(super) fn sync_session_options(
        &self,
        session: &str,
        composer: &Entity<Composer>,
        cx: &mut App,
    ) {
        let offer = {
            let state = self.store.read(cx).state();
            let record = state.agents.iter().find(|record| record.id == session);
            let kind = record
                .and_then(|record| state.agent_kinds.iter().find(|kind| kind.id == record.kind));
            match (state.agent_options_live, record, kind) {
                (true, Some(record), Some(kind)) => {
                    let choices: Vec<_> = session_kind_choices(&state.agent_kinds)
                        .into_iter()
                        .filter(|choice| choice.id == kind.id)
                        .collect();
                    let brands = {
                        let (installed, catalogue) = state.extension_lists();
                        agent_brands(installed, catalogue)
                    };
                    let agents =
                        picker_agents(&state.agent_kinds, &choices, &brands, state.agent_fast);
                    (!agents.is_empty()).then(|| (agents, session_choice(kind, record)))
                }
                _ => None,
            }
        };
        let in_flight = self.session_live_options.calls.contains(session);
        let note = self
            .session_live_options
            .notes
            .get(session)
            .cloned()
            .map(SharedString::from);
        composer.update(cx, |composer, cx| match offer {
            Some((agents, choice)) => {
                composer.set_picker_agents(agents, Some(choice.model.clone()), cx);
                if !in_flight {
                    composer.set_picker_choice(choice, cx);
                }
                composer.set_model_note(note, cx);
                composer.set_model_picker_offered(true, cx);
            }
            None => {
                composer.set_model_note(None, cx);
                composer.set_model_picker_offered(false, cx);
            }
        });
    }

    /// Asks the service to apply the options the chip picked that differ from the session's.
    pub(super) fn set_session_options(
        &mut self,
        session: &str,
        choice: &Choice,
        cx: &mut Context<Self>,
    ) {
        let params = {
            let state = self.store.read(cx).state();
            if !state.agent_options_live {
                return;
            }
            state
                .agents
                .iter()
                .find(|record| record.id == session)
                .and_then(|record| {
                    let kind = state
                        .agent_kinds
                        .iter()
                        .find(|kind| kind.id == record.kind)?;
                    options_to_set(kind, record, choice)
                })
        };
        let (Some(params), Some(service)) = (params, self.service.clone()) else {
            return;
        };
        self.session_notices.remove(session);
        self.session_live_options.notes.remove(session);
        self.session_live_options.calls.insert(session.to_owned());
        let id = session.to_owned();
        let shell = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let answer = service.agent_set_options(params).await;
            let _ = shell.update(cx, |shell, cx| shell.finish_set_options(&id, answer, cx));
        })
        .detach();
        cx.notify();
    }

    /// The answer to an option change: a note for each option kept for the next resume, nothing more for a live one,
    /// and a refusal's sentence above the composer while the chip goes back to the session's options.
    fn finish_set_options(
        &mut self,
        session: &str,
        answer: Result<AgentOptionsSet, Refusal>,
        cx: &mut Context<Self>,
    ) {
        self.session_live_options.calls.remove(session);
        match answer {
            Ok(set) => {
                if let Some(note) = applied_note(&set.applied) {
                    self.session_live_options
                        .notes
                        .insert(session.to_owned(), note);
                }
            }
            Err(refusal) => {
                self.session_notices.insert(
                    session.to_owned(),
                    format!("Could not change the model: {}", refusal.sentence()),
                );
            }
        }
        cx.notify();
    }
}
