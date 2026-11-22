//! Typed helpers over the `secrets/1` methods (decision 0071). Only `secret_set` sends a value; nothing returns one.

use massorca_protocol::methods::secrets::{
    self, SecretDecline, SecretEntry, SecretLink, SecretList, SecretListParams, SecretName,
    SecretRemove, SecretRemoved, SecretRequest, SecretRequestParams, SecretRequests,
    SecretRequestsParams, SecretSet, SecretStatus,
};

use crate::client::Client;
use crate::error::ClientError;

impl Client {
    /// `secret.status`: the key's protection, whether it opens, and the number of owned secrets.
    pub async fn secret_status(&self) -> Result<SecretStatus, ClientError> {
        self.call(secrets::STATUS, &()).await
    }

    /// `secret.list`: names in one scope or all, owned and linked.
    pub async fn secret_list(&self, params: &SecretListParams) -> Result<SecretList, ClientError> {
        self.call(secrets::LIST, params).await
    }

    /// `secret.set`: creates a secret, or rotates an owned one with `replace`.
    pub async fn secret_set(&self, params: &SecretSet) -> Result<SecretEntry, ClientError> {
        self.call(secrets::SET, params).await
    }

    /// `secret.link`: uses the secret of the same name owned by `from_scope`.
    pub async fn secret_link(&self, params: &SecretLink) -> Result<SecretEntry, ClientError> {
        self.call(secrets::LINK, params).await
    }

    /// `secret.unlink`: removes a link; the owner's secret stays.
    pub async fn secret_unlink(&self, params: &SecretName) -> Result<SecretRemoved, ClientError> {
        self.call(secrets::UNLINK, params).await
    }

    /// `secret.remove`: removes an owned secret; one with links needs `include_links`.
    pub async fn secret_remove(&self, params: &SecretRemove) -> Result<SecretRemoved, ClientError> {
        self.call(secrets::REMOVE, params).await
    }

    /// `secret.request`: asks for a secret by name.
    pub async fn secret_request(
        &self,
        params: &SecretRequestParams,
    ) -> Result<SecretRequest, ClientError> {
        self.call(secrets::REQUEST, params).await
    }

    /// `secret.requests`: pending requests, or every request with `all`.
    pub async fn secret_requests(
        &self,
        params: &SecretRequestsParams,
    ) -> Result<SecretRequests, ClientError> {
        self.call(secrets::REQUESTS, params).await
    }

    /// `secret.reset_key`: forgets the key of a vault that holds no secret; the next set creates a fresh one.
    pub async fn secret_reset_key(&self) -> Result<SecretStatus, ClientError> {
        self.call(secrets::RESET_KEY, &()).await
    }

    /// `secret.decline`: declines a pending request.
    pub async fn secret_decline(
        &self,
        params: &SecretDecline,
    ) -> Result<SecretRequest, ClientError> {
        self.call(secrets::DECLINE, params).await
    }
}
