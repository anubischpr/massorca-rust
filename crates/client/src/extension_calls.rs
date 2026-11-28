//! Typed helpers over the `extensions/1` methods (decision 0062).

use massorca_protocol::methods::extensions::{
    self, ExtensionAsset, ExtensionAssetParams, ExtensionEnabled, ExtensionInspectParams,
    ExtensionInstallParams, ExtensionInstalled, ExtensionItem, ExtensionList, ExtensionLog,
    ExtensionRef, ExtensionRemoveParams, ExtensionRemoved, ExtensionReview,
    ExtensionRollbackParams, ExtensionSetEnabledParams, ExtensionUpdateParams, ProgramUpdateParams,
    ProgramUpdateReview, ProgramUpdated,
};

use massorca_protocol::methods::extension_points::{self, ExtensionPointsList};

use crate::client::Client;
use crate::error::ClientError;

impl Client {
    /// `extensions.list`: the installed packages, in install order.
    pub async fn extension_list(&self) -> Result<ExtensionList, ClientError> {
        self.call(extensions::LIST, &()).await
    }

    /// `extensions.points`: what each owner registered on the tool, skill, view, theme and status-rule points (0153).
    pub async fn extension_points(&self) -> Result<ExtensionPointsList, ClientError> {
        self.call(extension_points::POINTS, &()).await
    }

    /// `extensions.get`: one item, installed or from the catalogue.
    pub async fn extension_get(&self, id: &str) -> Result<ExtensionItem, ClientError> {
        let params = ExtensionRef { id: id.to_owned() };
        self.call(extensions::GET, &params).await
    }

    /// `extensions.catalogue`: the built-in catalogue with each entry's state.
    pub async fn extension_catalogue(&self) -> Result<ExtensionList, ClientError> {
        self.call(extensions::CATALOGUE, &()).await
    }

    /// `extensions.inspect`: the review of a package source. Changes nothing.
    pub async fn extension_inspect(&self, source: &str) -> Result<ExtensionReview, ClientError> {
        let params = ExtensionInspectParams {
            source: source.to_owned(),
        };
        self.call(extensions::INSPECT, &params).await
    }

    /// `extensions.asset`: a listed image of an item, base64.
    pub async fn extension_asset(
        &self,
        params: &ExtensionAssetParams,
    ) -> Result<ExtensionAsset, ClientError> {
        self.call(extensions::ASSET, params).await
    }

    /// `extensions.install`: installs a package after its review was accepted. May run a vendor installer.
    pub async fn extension_install(
        &self,
        params: &ExtensionInstallParams,
    ) -> Result<ExtensionInstalled, ClientError> {
        self.call(extensions::INSTALL, params).await
    }

    /// `extensions.set_enabled`: enables or disables an installed package.
    pub async fn extension_set_enabled(
        &self,
        params: &ExtensionSetEnabledParams,
    ) -> Result<ExtensionEnabled, ClientError> {
        self.call(extensions::SET_ENABLED, params).await
    }

    /// `extensions.update`: brings an installed package up to date in place, after its new review was accepted.
    pub async fn extension_update(
        &self,
        params: &ExtensionUpdateParams,
    ) -> Result<ExtensionInstalled, ClientError> {
        self.call(extensions::UPDATE, params).await
    }

    /// `extensions.rollback`: restores the version an update replaced. Runs no vendor command.
    pub async fn extension_rollback(
        &self,
        params: &ExtensionRollbackParams,
    ) -> Result<ExtensionInstalled, ClientError> {
        self.call(extensions::ROLLBACK, params).await
    }

    /// `extensions.program_update_review`: what updating a host program would run (decision 0151). Changes nothing.
    pub async fn extension_program_update_review(
        &self,
        id: &str,
        program: &str,
    ) -> Result<ProgramUpdateReview, ClientError> {
        let params = ProgramUpdateParams {
            id: id.to_owned(),
            program: program.to_owned(),
            consent: None,
            agent_token: None,
        };
        self.call(extensions::PROGRAM_UPDATE_REVIEW, &params).await
    }

    /// `extensions.program_update`: runs the reviewed vendor update of a host program the package installed.
    pub async fn extension_program_update(
        &self,
        params: &ProgramUpdateParams,
    ) -> Result<ProgramUpdated, ClientError> {
        self.call(extensions::PROGRAM_UPDATE, params).await
    }

    /// `extensions.log`: the recent runs of a package, oldest first.
    pub async fn extension_log(&self, id: &str) -> Result<ExtensionLog, ClientError> {
        let params = ExtensionRef { id: id.to_owned() };
        self.call(extensions::LOG, &params).await
    }

    /// `extensions.remove`: removes a package. May run a vendor uninstaller.
    pub async fn extension_remove(
        &self,
        params: &ExtensionRemoveParams,
    ) -> Result<ExtensionRemoved, ClientError> {
        self.call(extensions::REMOVE, params).await
    }
}
