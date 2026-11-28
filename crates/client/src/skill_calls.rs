//! Typed helpers over the `skills/1`, `skills-place/1`, `skills-place/2` and `skills-bundled/1` methods (SKL-01, SKL-02,
//! SKL-04, decision 0150).

use massorca_protocol::methods::skill_bundled::{
    self as bundled, SkillsBundledInstallParams, SkillsBundledListParams, SkillsBundledListResult,
    SkillsBundledNameParams,
};
use massorca_protocol::methods::skill_placement::{
    self as placement, SkillAgentsParams, SkillPlaceParams, SkillPlacementResult,
    SkillUnplaceParams, SkillsManagedParams, SkillsManagedResult,
};
use massorca_protocol::methods::skills::{self, SkillDiscovery, SkillsDiscoverParams};

use crate::client::Client;
use crate::error::ClientError;

impl Client {
    /// `skills.discover`: the skills this host finds, and each root they were read from or skipped. Changes nothing.
    pub async fn skills_discover(
        &self,
        params: &SkillsDiscoverParams,
    ) -> Result<SkillDiscovery, ClientError> {
        self.call(skills::DISCOVER, params).await
    }

    /// `skills.place-preview`: what placing a home skill for the agents would do. Changes nothing.
    pub async fn skills_place_preview(
        &self,
        params: &SkillPlaceParams,
    ) -> Result<SkillPlacementResult, ClientError> {
        self.call(placement::PREVIEW, params).await
    }

    /// `skills.place`: copies a home skill into the chosen agents' folders, recording what it placed.
    pub async fn skills_place(
        &self,
        params: &SkillPlaceParams,
    ) -> Result<SkillPlacementResult, ClientError> {
        self.call(placement::PLACE, params).await
    }

    /// `skills.unplace`: removes the copies MassOrca placed for a skill; the ones the user changed stay.
    pub async fn skills_unplace(
        &self,
        params: &SkillUnplaceParams,
    ) -> Result<SkillPlacementResult, ClientError> {
        self.call(placement::UNPLACE, params).await
    }

    /// `skills.agents` (capability `skills-place/2`): adds agents to and drops agents from a placed skill in one scope.
    pub async fn skills_agents(
        &self,
        params: &SkillAgentsParams,
    ) -> Result<SkillPlacementResult, ClientError> {
        self.call(placement::AGENTS, params).await
    }

    /// `skills.managed`: the skills MassOrca placed on this host.
    pub async fn skills_managed(
        &self,
        params: &SkillsManagedParams,
    ) -> Result<SkillsManagedResult, ClientError> {
        self.call(placement::MANAGED, params).await
    }

    /// `skills.bundled.list`: the skills MassOrca ships, and whether each is placed on this host. Changes nothing.
    pub async fn skills_bundled_list(&self) -> Result<SkillsBundledListResult, ClientError> {
        self.call(bundled::LIST, &SkillsBundledListParams::default())
            .await
    }

    /// `skills.bundled.install`: places a skill MassOrca ships for the chosen agents, or for every agent folder on this host.
    pub async fn skills_bundled_install(
        &self,
        params: &SkillsBundledInstallParams,
    ) -> Result<SkillPlacementResult, ClientError> {
        self.call(bundled::INSTALL, params).await
    }

    /// `skills.bundled.refresh`: brings a placed skill MassOrca ships to this build's text, for the agents it is placed for.
    pub async fn skills_bundled_refresh(
        &self,
        params: &SkillsBundledNameParams,
    ) -> Result<SkillPlacementResult, ClientError> {
        self.call(bundled::REFRESH, params).await
    }
}
