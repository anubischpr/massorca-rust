//! Requests for a secret by name, the backend of the in-chat request card.

use massorca_core_store::{SecretRequestRow, new_id, now_ms};

use super::{Vault, VaultError, check_name};
use crate::names::{Scope, is_valid_reason, is_valid_session};

impl Vault {
    /// Opens a pending request for `name` in `scope`. Name, reason and session are untrusted and checked.
    ///
    /// # Errors
    /// `InvalidName`, `InvalidReason`, `InvalidSession`, or a store failure.
    pub fn request(
        &self,
        scope: &Scope,
        name: &str,
        reason: &str,
        session: Option<String>,
    ) -> Result<SecretRequestRow, VaultError> {
        check_name(name)?;
        if !is_valid_reason(reason) {
            return Err(VaultError::InvalidReason);
        }
        if session
            .as_deref()
            .is_some_and(|session| !is_valid_session(session))
        {
            return Err(VaultError::InvalidSession);
        }
        let row = SecretRequestRow {
            id: new_id(),
            scope: scope.to_string(),
            name: name.to_owned(),
            reason: reason.to_owned(),
            session,
            requester: None,
            state: "pending".to_owned(),
            created_at: now_ms(),
            resolved_at: None,
        };
        self.store.insert_secret_request(&row)?;
        Ok(row)
    }

    /// Pending requests, or every request with `all`.
    ///
    /// # Errors
    /// A store failure.
    pub fn requests(&self, all: bool) -> Result<Vec<SecretRequestRow>, VaultError> {
        Ok(self.store.list_secret_requests(!all)?)
    }

    /// Declines a pending request.
    ///
    /// # Errors
    /// `RequestNotFound`, or `RequestResolved` when it is no longer pending.
    pub fn decline(&self, id: &str) -> Result<SecretRequestRow, VaultError> {
        let _guard = self.lock();
        let request = self
            .store
            .secret_request(id)?
            .ok_or_else(|| VaultError::RequestNotFound(id.to_owned()))?;
        if !self
            .store
            .resolve_secret_request(id, "declined", now_ms())?
        {
            return Err(VaultError::RequestResolved {
                id: id.to_owned(),
                state: request.state,
            });
        }
        self.store
            .secret_request(id)?
            .ok_or_else(|| VaultError::RequestNotFound(id.to_owned()))
    }
}
