//! Typed helpers over the `skills/1` and `skills-place/1` methods (SKL-01, SKL-02).

use massorca_protocol::methods::skill_placement::{
    self as placement, SkillPlaceParams, SkillPlacementResult, SkillUnplaceParams,
    SkillsManagedParams, SkillsManagedResult,
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

    /// `skills.managed`: the skills MassOrca placed on this host.
    pub async fn skills_managed(
        &self,
        params: &SkillsManagedParams,
    ) -> Result<SkillsManagedResult, ClientError> {
        self.call(placement::MANAGED, params).await
    }
}
