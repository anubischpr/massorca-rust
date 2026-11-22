//! The agent kinds a new bot can run on, read from the service's `agent.kinds`. Sample data offers the four
//! built-in kinds under the ids the service declares for them.

use massorca_protocol::methods::agents::{AgentKind, PackageState};

/// Why a kind the host does not detect cannot be picked for a bot, or cannot run a bot there.
pub const NOT_AVAILABLE_ON_HOST: &str = "Not available on this host";

/// A kind an Add bot form offers: the service's id for it, sent as the bot's kind, and the name the form shows.
/// `available` is false when the kind is installed but its program is not found on the bot's host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KindChoice {
    pub id: String,
    pub name: String,
    pub available: bool,
}

/// The kinds a new bot can run on, in the order the service lists them: installed and on, with no problems.
/// A kind the host does not detect is listed unavailable, so the form shows it but cannot choose it.
pub fn bot_kind_choices(kinds: &[AgentKind]) -> Vec<KindChoice> {
    kinds
        .iter()
        .filter(|kind| kind.package.state == PackageState::Enabled && kind.problems.is_empty())
        .map(|kind| KindChoice {
            id: kind.id.clone(),
            name: kind.name.clone(),
            available: kind.detected,
        })
        .collect()
}

/// The choice at `index` when it can be picked: an unavailable kind is refused here, whatever the form shows.
pub fn choice_at(choices: &[KindChoice], index: usize) -> Option<&KindChoice> {
    choices.get(index).filter(|choice| choice.available)
}

/// The first choice that can be picked, which a new form starts on.
pub fn first_available(choices: &[KindChoice]) -> Option<usize> {
    choices.iter().position(|choice| choice.available)
}

/// Whether the service's kind with this id is known and not detected on this host, so its bots cannot run here.
pub fn kind_unavailable(kinds: &[AgentKind], id: &str) -> bool {
    kinds.iter().any(|kind| kind.id == id && !kind.detected)
}

/// The four built-in kinds, offered by the sample in place of a service's list.
pub fn sample_bot_kind_choices() -> Vec<KindChoice> {
    [
        ("claude", "Claude"),
        ("codex", "Codex"),
        ("antigravity", "Antigravity"),
        ("gentle-shell", "Gentle Shell"),
    ]
    .into_iter()
    .map(|(id, name)| KindChoice {
        id: id.to_owned(),
        name: name.to_owned(),
        available: true,
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::agents::PackageState;

    use super::*;
    use crate::service::fixtures::agent_kind;

    fn ids(choices: &[KindChoice]) -> Vec<&str> {
        choices.iter().map(|choice| choice.id.as_str()).collect()
    }

    #[test]
    fn a_bot_can_run_only_on_an_enabled_kind_with_no_problems() {
        let mut broken = agent_kind("broken", "Broken", PackageState::Enabled);
        broken.problems = vec!["its declaration does not parse".into()];
        let kinds = [
            agent_kind("claude", "Claude", PackageState::Enabled),
            agent_kind("codex", "Codex", PackageState::Disabled),
            agent_kind("cursor", "Cursor", PackageState::Available),
            broken,
        ];
        assert_eq!(ids(&bot_kind_choices(&kinds)), ["claude"]);
    }

    #[test]
    fn the_choices_keep_the_names_and_the_order_the_service_gives() {
        let kinds = [
            agent_kind("gentle-shell", "Gentle Shell", PackageState::Enabled),
            agent_kind("claude", "Claude", PackageState::Enabled),
        ];
        let choices = bot_kind_choices(&kinds);
        assert_eq!(ids(&choices), ["gentle-shell", "claude"]);
        assert_eq!(choices[0].name, "Gentle Shell");
    }

    #[test]
    fn a_service_with_no_installed_kind_offers_no_choice() {
        let kinds = [agent_kind("claude", "Claude", PackageState::Available)];
        assert!(bot_kind_choices(&kinds).is_empty());
    }

    #[test]
    fn the_sample_offers_the_four_built_in_kinds_by_their_service_ids() {
        let choices = sample_bot_kind_choices();
        assert_eq!(
            ids(&choices),
            ["claude", "codex", "antigravity", "gentle-shell"]
        );
        assert_eq!(choices[3].name, "Gentle Shell");
    }

    #[test]
    fn a_kind_the_host_does_not_detect_is_listed_unavailable_and_cannot_be_chosen() {
        let mut missing = agent_kind("codex", "Codex", PackageState::Enabled);
        missing.detected = false;
        let kinds = [
            agent_kind("claude", "Claude", PackageState::Enabled),
            missing,
        ];
        let choices = bot_kind_choices(&kinds);
        assert_eq!(ids(&choices), ["claude", "codex"]);
        let available: Vec<bool> = choices.iter().map(|choice| choice.available).collect();
        assert_eq!(available, [true, false]);
        assert_eq!(
            choice_at(&choices, 1),
            None,
            "an unavailable kind is refused"
        );
        assert_eq!(
            choice_at(&choices, 0).map(|choice| choice.id.as_str()),
            Some("claude")
        );
        assert_eq!(first_available(&choices), Some(0));
    }

    #[test]
    fn a_kind_that_is_not_installed_is_not_listed_even_when_detected() {
        let mut cursor = agent_kind("cursor", "Cursor", PackageState::Available);
        cursor.detected = true;
        assert!(bot_kind_choices(&[cursor]).is_empty());
    }

    #[test]
    fn only_a_kind_the_store_knows_and_does_not_detect_here_is_unavailable() {
        let mut missing = agent_kind("codex", "Codex", PackageState::Enabled);
        missing.detected = false;
        let kinds = [
            agent_kind("claude", "Claude", PackageState::Enabled),
            missing,
        ];
        assert!(kind_unavailable(&kinds, "codex"));
        assert!(!kind_unavailable(&kinds, "claude"));
        assert!(!kind_unavailable(&kinds, "unknown-kind"));
    }

    #[test]
    fn the_sample_kinds_are_all_available() {
        assert!(
            sample_bot_kind_choices()
                .iter()
                .all(|choice| choice.available)
        );
    }
}
