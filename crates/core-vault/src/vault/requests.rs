//! Requests for a secret by name, the backend of the in-chat request card.

use massorca_core_store::{SecretRequestRow, new_id, now_ms};

use super::{Vault, VaultError, check_name};
use crate::names::{Scope, is_valid_reason, is_valid_session};

/// Pending requests one requester may hold for one scope, whatever session it names.
pub const MAX_PENDING_REQUESTS: u64 = 20;
/// New requests one requester may open per minute, across scopes and sessions.
pub const MAX_REQUESTS_PER_MINUTE: u64 = 10;
/// Answered requests are kept this long after they were answered, then pruned.
pub const ANSWERED_RETENTION_DAYS: i64 = 7;
/// Pending requests expire and are pruned this long after they were made.
pub const PENDING_EXPIRY_DAYS: i64 = 30;
const MINUTE_MS: i64 = 60_000;
const DAY_MS: i64 = 24 * 60 * MINUTE_MS;

impl Vault {
    /// Opens a pending request for `name` in `scope`. Name, reason and session are untrusted and checked.
    /// `requester` is who asks (`owner`, `device:<id>` or `agent:<session>`), which the limits count by. Old
    /// requests are pruned first.
    ///
    /// # Errors
    /// `InvalidName`, `InvalidReason`, `InvalidSession`, `RequestLimit`, or a store failure.
    pub fn request(
        &self,
        scope: &Scope,
        name: &str,
        reason: &str,
        session: Option<String>,
        requester: &str,
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
        let _guard = self.lock();
        let (scope_text, now) = (scope.to_string(), now_ms());
        self.store.prune_secret_requests(
            now - ANSWERED_RETENTION_DAYS * DAY_MS,
            now - PENDING_EXPIRY_DAYS * DAY_MS,
        )?;
        if self
            .store
            .count_pending_secret_requests(&scope_text, requester)?
            >= MAX_PENDING_REQUESTS
        {
            return Err(VaultError::RequestLimit(
                "at most 20 pending requests per scope",
            ));
        }
        if self
            .store
            .count_secret_requests_since(requester, now - MINUTE_MS)?
            >= MAX_REQUESTS_PER_MINUTE
        {
            return Err(VaultError::RequestLimit("at most 10 new requests a minute"));
        }
        let row = SecretRequestRow {
            id: new_id(),
            scope: scope_text,
            name: name.to_owned(),
            reason: reason.to_owned(),
            session,
            requester: Some(requester.to_owned()),
            state: "pending".to_owned(),
            created_at: now,
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
