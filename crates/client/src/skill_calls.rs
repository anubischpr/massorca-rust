//! Typed helpers over the `skills/1` methods (SKL-01).

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
}
