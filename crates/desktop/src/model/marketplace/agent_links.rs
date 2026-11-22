//! The installed agents that Settings lists, each linking to its Marketplace page. Settings never
//! installs anything, so an agent that is only available is not listed (decision 0059).

use super::item_page::state_chip;
use super::types::{ItemKind, MarketplaceItemId, MarketplaceSnapshot};

/// One installed agent row in Settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentLink {
    pub item: MarketplaceItemId,
    pub name: String,
    /// The state word, such as "Connected".
    pub state: &'static str,
}

/// The installed agents, sorted by name.
pub fn installed_agent_links(snapshot: &MarketplaceSnapshot) -> Vec<AgentLink> {
    snapshot
        .search("")
        .into_iter()
        .filter(|item| item.kind == ItemKind::Agent && item.state.is_installed())
        .map(|item| AgentLink {
            item: item.id,
            name: item.name.clone(),
            state: state_chip(item, snapshot),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::marketplace::{MarketplaceSource, SampleMarketplaceSource};

    #[test]
    fn settings_lists_only_the_installed_agents_by_name() {
        let links = installed_agent_links(&SampleMarketplaceSource::default().snapshot());
        let names: Vec<&str> = links.iter().map(|link| link.name.as_str()).collect();
        assert_eq!(names, ["Claude", "Gemini CLI"]);
        assert_eq!(links[0].state, "Connected");
        assert_eq!(links[0].item, MarketplaceItemId(1));
    }
}
