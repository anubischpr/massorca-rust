//! The agent kinds a new bot can run on, read from the service's `agent.kinds`. Sample data offers the four
//! built-in kinds under the ids the service declares for them.

use massorca_protocol::methods::agents::{AgentKind, PackageState};

/// A kind an Add bot form offers: the service's id for it, sent as the bot's kind, and the name the form shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KindChoice {
    pub id: String,
    pub name: String,
}

/// The kinds a new bot can run on, in the order the service lists them: installed and on, with no problems.
pub fn bot_kind_choices(kinds: &[AgentKind]) -> Vec<KindChoice> {
    kinds
        .iter()
        .filter(|kind| kind.package.state == PackageState::Enabled && kind.problems.is_empty())
        .map(|kind| KindChoice {
            id: kind.id.clone(),
            name: kind.name.clone(),
        })
        .collect()
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
}
