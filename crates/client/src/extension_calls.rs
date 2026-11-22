//! Typed helpers over the `extensions/1` methods (decision 0062).

use massorca_protocol::methods::extensions::{
    self, ExtensionAsset, ExtensionAssetParams, ExtensionEnabled, ExtensionInspectParams,
    ExtensionInstallParams, ExtensionInstalled, ExtensionItem, ExtensionList, ExtensionRef,
    ExtensionRemoveParams, ExtensionRemoved, ExtensionReview, ExtensionSetEnabledParams,
};

use crate::client::Client;
use crate::error::ClientError;

impl Client {
    /// `extensions.list`: the installed packages, in install order.
    pub async fn extension_list(&self) -> Result<ExtensionList, ClientError> {
        self.call(extensions::LIST, &()).await
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

    /// `extensions.remove`: removes a package. May run a vendor uninstaller.
    pub async fn extension_remove(
        &self,
        params: &ExtensionRemoveParams,
    ) -> Result<ExtensionRemoved, ClientError> {
        self.call(extensions::REMOVE, params).await
    }
}
