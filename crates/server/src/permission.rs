//! Permission classes, principals, and the one check every request passes through (ADR 0002, extensions.md section 9).

use massorca_protocol::{ErrorBody, codes};

/// What a method needs (`read`, `workspace` and `host-admin` in ADR 0002).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    Read,
    Workspace,
    HostAdmin,
}

/// Who is calling. Phase 1 has only the local owner; device and extension principals arrive later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Principal {
    Owner,
}

impl Principal {
    fn holds(self, _permission: Permission) -> bool {
        match self {
            Self::Owner => true,
        }
    }
}

/// The single permission check. Handlers take the principal from their request context and never look at the connection.
pub fn authorize(principal: &Principal, permission: Permission) -> Result<(), ErrorBody> {
    if principal.holds(permission) {
        Ok(())
    } else {
        Err(ErrorBody::new(
            codes::PERMISSION_DENIED,
            format!("{principal:?} may not use {permission:?}"),
        ))
    }
}
