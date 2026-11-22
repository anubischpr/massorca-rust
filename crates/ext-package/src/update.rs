//! What an update adds over the installed version: new permissions, wider scopes, and new risk.
//!
//! Updates never widen permissions silently (`docs/architecture/extensions.md` §5). A review is required whenever
//! [`UpdateReview::requires_review`] holds.

use crate::manifest::Manifest;
use crate::permission::{Permission, PermissionKind, PermissionScope};
use crate::risk::{RiskLevel, RiskReason, assess, reasons};

/// The permission and risk changes between two manifests of one extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateReview {
    /// Permission kinds the new version declares and the old one did not.
    pub added_permissions: Vec<Permission>,
    /// Kinds whose scope grew: the new entries, or any scope at all.
    pub widened_permissions: Vec<WidenedPermission>,
    /// Risk reasons the new version has that the old one did not.
    pub added_reasons: Vec<RiskReason>,
    pub risk_before: RiskLevel,
    pub risk_after: RiskLevel,
}

/// A permission whose scope gained entries, or whose scope became "any".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidenedPermission {
    pub kind: PermissionKind,
    /// The entries the new scope adds. Empty when the scope became "any".
    pub added: Vec<String>,
    pub to_any: bool,
}

impl UpdateReview {
    /// Whether the update must go back through the permission review before it is installed.
    pub fn requires_review(&self) -> bool {
        !self.added_permissions.is_empty()
            || !self.widened_permissions.is_empty()
            || !self.added_reasons.is_empty()
            || self.risk_after > self.risk_before
    }
}

/// Compares an installed manifest with its update.
pub fn review_update(old: &Manifest, new: &Manifest) -> UpdateReview {
    let mut added_permissions = Vec::new();
    let mut widened_permissions = Vec::new();
    for permission in &new.permissions {
        match old
            .permissions
            .iter()
            .find(|previous| previous.kind == permission.kind)
        {
            None => added_permissions.push(permission.clone()),
            Some(previous) => widened_permissions.extend(widening(previous, permission)),
        }
    }
    let before = assess(old);
    let old_reasons = reasons(old);
    let added_reasons = reasons(new)
        .into_iter()
        .filter(|reason| {
            !old_reasons
                .iter()
                .any(|known| known.subject == reason.subject && known.copy == reason.copy)
        })
        .collect();
    UpdateReview {
        added_permissions,
        widened_permissions,
        added_reasons,
        risk_before: before.level,
        risk_after: assess(new).level,
    }
}

fn widening(old: &Permission, new: &Permission) -> Option<WidenedPermission> {
    match (&old.scope, &new.scope) {
        (None, _) => None,
        (Some(_), None) => Some(WidenedPermission {
            kind: new.kind,
            added: Vec::new(),
            to_any: true,
        }),
        (Some(old_scope), Some(new_scope)) => {
            let known = entries(old_scope);
            let added: Vec<String> = entries(new_scope)
                .iter()
                .filter(|entry| !known.contains(entry))
                .cloned()
                .collect();
            (!added.is_empty()).then_some(WidenedPermission {
                kind: new.kind,
                added,
                to_any: false,
            })
        }
    }
}

fn entries(scope: &PermissionScope) -> &[String] {
    match scope {
        PermissionScope::Files(items)
        | PermissionScope::Net(items)
        | PermissionScope::Secrets(items)
        | PermissionScope::Process(items)
        | PermissionScope::Credentials(items) => items,
    }
}
