//! Element tests of a session's model chip (AG-14, decision 0171): it shows the session's options, a pick sends only
//! what changed through `agent.set_options`, a next-resume answer leaves a note, and a refusal keeps the old choice.

use gpui::{Entity, TestAppContext, VisualTestContext};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    AgentKind, AgentOptionsSet, AgentSetOptionsParams, KindEffort, KindEffortLevel, KindModel,
    KindModelChoice, OptionApplied, OptionsApplied, PackageState,
};
use massorca_ui::components::composer::{Composer, ComposerEvent};
use massorca_ui::components::model_picker::Choice;
use tokio::sync::oneshot;

use super::session_view_tests::{Rig, chat_s1, draw, session_rig};
use crate::service::fixtures::{agent_kind, base_agent_options_set, base_kind_model_choice};
use crate::service::{Command, Refusal, Script, StoreUpdate};

fn claude() -> AgentKind {
    let mut kind = agent_kind("claude", "Claude Code", PackageState::Enabled);
    kind.model = Some(KindModel {
        choices: ["opus", "sonnet"]
            .into_iter()
            .map(|id| KindModelChoice {
                id: id.into(),
                label: id.into(),
                default: id == "sonnet",
                ..base_kind_model_choice()
            })
            .collect(),
    });
    kind.effort = Some(KindEffort {
        levels: ["low", "high"]
            .into_iter()
            .map(|id| KindEffortLevel {
                id: id.into(),
                label: id.into(),
            })
            .collect(),
        default: Some("low".into()),
    });
    kind
}

/// A chat session s1 on sonnet at low effort, on a service that does or does not change options live.
fn rig(cx: &mut TestAppContext, live: bool) -> (Rig, &mut VisualTestContext) {
    let mut session = chat_s1(false);
    session.meta.model = Some("sonnet".into());
    session.meta.effort = Some("low".into());
    let (rig, cx) = session_rig(cx, vec![claude()], vec![session]);
    rig.updates
        .send(StoreUpdate::AgentOptionsLiveOffered(live))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    (rig, cx)
}

fn composer(rig: &Rig, cx: &mut VisualTestContext) -> Entity<Composer> {
    rig.shell
        .update(cx, |shell, cx| shell.session_composer("s1", cx))
}

fn shown(rig: &Rig, cx: &mut VisualTestContext) -> (bool, Choice) {
    let composer = composer(rig, cx);
    composer.read_with(cx, |composer, cx| {
        (
            composer.model_picker_offered(),
            composer.picker().read(cx).state().choice().clone(),
        )
    })
}

fn pick(rig: &Rig, cx: &mut VisualTestContext, model: &str, effort: usize) {
    let composer = composer(rig, cx);
    let choice = Choice {
        model: model.to_owned().into(),
        effort: Some(effort),
        fast: false,
    };
    composer.update(cx, |_, cx| cx.emit(ComposerEvent::ModelChanged(choice)));
    cx.run_until_parked();
}

type SetOptionsCall = (
    AgentSetOptionsParams,
    oneshot::Sender<Result<AgentOptionsSet, Refusal>>,
);

fn set_options_call(script: &mut Script) -> Option<SetOptionsCall> {
    while let Some(command) = script.next() {
        if let Command::AgentSetOptions { params, reply } = command {
            return Some((params, reply));
        }
    }
    None
}

fn applied(applied: OptionsApplied) -> AgentOptionsSet {
    AgentOptionsSet {
        applied,
        ..base_agent_options_set()
    }
}

#[gpui::test]
fn the_chip_shows_the_session_options_and_a_pick_sends_only_what_changed(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, true);
    let (offered, choice) = shown(&rig, cx);
    assert!(offered, "the service changes options live");
    assert_eq!((choice.model.as_ref(), choice.effort), ("sonnet", Some(0)));

    pick(&rig, cx, "opus", 0);
    let (params, reply) = set_options_call(&mut rig.script).expect("agent.set_options is sent");
    assert_eq!(params.session, "s1");
    assert_eq!(params.model.as_deref(), Some("opus"));
    assert_eq!(
        (params.effort, params.fast),
        (None, None),
        "only the model changed"
    );

    reply
        .send(Ok(applied(OptionsApplied {
            model: Some(OptionApplied::NextResume),
            ..OptionsApplied::default()
        })))
        .expect("the shell waits for the answer");
    cx.run_until_parked();
    draw(cx);
    let note = composer(&rig, cx).read_with(cx, |composer, _| composer.model_note().cloned());
    assert_eq!(
        note.as_deref(),
        Some("Model applies when the session resumes")
    );
    assert!(cx.debug_bounds("composer-model-note").is_some());
}

#[gpui::test]
fn a_live_answer_shows_no_note(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, true);
    pick(&rig, cx, "sonnet", 1);
    let (params, reply) = set_options_call(&mut rig.script).expect("agent.set_options is sent");
    assert_eq!(
        (params.model, params.effort.as_deref()),
        (None, Some("high"))
    );
    reply
        .send(Ok(applied(OptionsApplied {
            effort: Some(OptionApplied::Live),
            ..OptionsApplied::default()
        })))
        .expect("the shell waits for the answer");
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("composer-model-note").is_none());
}

#[gpui::test]
fn a_refused_pick_says_why_and_the_chip_keeps_the_old_choice(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, true);
    pick(&rig, cx, "opus", 0);
    let (_, reply) = set_options_call(&mut rig.script).expect("agent.set_options is sent");
    reply
        .send(Err(Refusal::Failed {
            code: codes::AGENT_OPTION_UNSUPPORTED.to_owned(),
            message: "claude does not take model opus".to_owned(),
        }))
        .expect("the shell waits for the answer");
    cx.run_until_parked();
    draw(cx);

    assert!(
        cx.debug_bounds("session-notice").is_some(),
        "the sentence is shown"
    );
    let (_, choice) = shown(&rig, cx);
    assert_eq!(
        choice.model.as_ref(),
        "sonnet",
        "the session's model stays chosen"
    );
}

#[gpui::test]
fn without_live_options_the_session_composer_has_no_chip_and_a_pick_sends_nothing(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = rig(cx, false);
    let (offered, _) = shown(&rig, cx);
    assert!(!offered, "today's behaviour: no chip");
    pick(&rig, cx, "opus", 0);
    assert!(set_options_call(&mut rig.script).is_none());
}
