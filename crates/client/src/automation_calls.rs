//! Typed helpers over the `automations/1` methods: scheduled automation definitions (decision 0123).

use massorca_protocol::methods::automations::{
    self, Automation, AutomationCreateParams, AutomationList, AutomationRef, AutomationUpdateParams,
};

use crate::client::Client;
use crate::error::ClientError;

impl Client {
    /// `automation.create`: defines an automation in a project; a repeated `creation_key` returns the first one.
    pub async fn automation_create(
        &self,
        params: &AutomationCreateParams,
    ) -> Result<Automation, ClientError> {
        self.call(automations::CREATE, params).await
    }

    /// `automation.list`: every automation, in creation order.
    pub async fn automation_list(&self) -> Result<AutomationList, ClientError> {
        self.call(automations::LIST, &()).await
    }

    /// `automation.show`: one automation, by id or exact name.
    pub async fn automation_show(&self, params: &AutomationRef) -> Result<Automation, ClientError> {
        self.call(automations::SHOW, params).await
    }

    /// `automation.update`: changes the fields of an automation that are given.
    pub async fn automation_update(
        &self,
        params: &AutomationUpdateParams,
    ) -> Result<Automation, ClientError> {
        self.call(automations::UPDATE, params).await
    }

    /// `automation.rm`: removes an automation; returns the removed automation.
    pub async fn automation_remove(
        &self,
        params: &AutomationRef,
    ) -> Result<Automation, ClientError> {
        self.call(automations::REMOVE, params).await
    }

    /// `automation.enable`: enables an automation and schedules its next run from now.
    pub async fn automation_enable(
        &self,
        params: &AutomationRef,
    ) -> Result<Automation, ClientError> {
        self.call(automations::ENABLE, params).await
    }

    /// `automation.disable`: disables an automation; it has no next run until enabled.
    pub async fn automation_disable(
        &self,
        params: &AutomationRef,
    ) -> Result<Automation, ClientError> {
        self.call(automations::DISABLE, params).await
    }
}
