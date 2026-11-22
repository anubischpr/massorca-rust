//! Permission classes, principals, and the one check every request passes through (ADR 0002, 0041, extensions.md section 9).

use std::fmt;
use std::sync::Arc;

use massorca_protocol::methods::devices::scopes;
use massorca_protocol::{ErrorBody, codes};

/// What a method needs (`read`, `workspace`, `host-admin` in ADR 0002; `device-admin` in 0041).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    Read,
    Workspace,
    HostAdmin,
    DeviceAdmin,
}

impl Permission {
    const ALL: [Self; 4] = [
        Self::Read,
        Self::Workspace,
        Self::HostAdmin,
        Self::DeviceAdmin,
    ];

    /// The wire name of the class.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => scopes::READ,
            Self::Workspace => scopes::WORKSPACE,
            Self::HostAdmin => scopes::HOST_ADMIN,
            Self::DeviceAdmin => scopes::DEVICE_ADMIN,
        }
    }

    const fn bit(self) -> u8 {
        match self {
            Self::Read => 1,
            Self::Workspace => 2,
            Self::HostAdmin => 4,
            Self::DeviceAdmin => 8,
        }
    }
}

/// A set of permission classes: what a paired device holds.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct Scopes(u8);

impl Scopes {
    /// What a device gets by default: `read` and `workspace`.
    pub const STANDARD: Self = Self(1 | 2);
    /// What `--admin` adds the administrative classes to.
    pub const ADMIN: Self = Self(1 | 2 | 4 | 8);

    pub const fn contains(self, permission: Permission) -> bool {
        self.0 & permission.bit() != 0
    }

    /// The wire names, in a fixed order.
    pub fn names(self) -> Vec<String> {
        Permission::ALL
            .into_iter()
            .filter(|permission| self.contains(*permission))
            .map(|permission| permission.as_str().to_owned())
            .collect()
    }

    /// Reads stored wire names. Names this build does not know grant nothing.
    pub fn from_names<S: AsRef<str>>(names: &[S]) -> Self {
        let bits = Permission::ALL
            .into_iter()
            .filter(|permission| {
                names
                    .iter()
                    .any(|name| name.as_ref() == permission.as_str())
            })
            .fold(0, |bits, permission| bits | permission.bit());
        Self(bits)
    }
}

impl fmt::Debug for Scopes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.names()).finish()
    }
}

/// Who is calling: the local owner of the profile, or a paired network device.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Principal {
    Owner,
    Device { id: Arc<str>, scopes: Scopes },
}

impl Principal {
    fn holds(&self, permission: Permission) -> bool {
        match self {
            Self::Owner => true,
            Self::Device { scopes, .. } => scopes.contains(permission),
        }
    }

    /// The device id, for a device principal.
    pub fn device_id(&self) -> Option<&str> {
        match self {
            Self::Owner => None,
            Self::Device { id, .. } => Some(id),
        }
    }

    /// The classes this principal holds, as wire names.
    pub fn scope_names(&self) -> Vec<String> {
        match self {
            Self::Owner => Scopes::ADMIN.names(),
            Self::Device { scopes, .. } => scopes.names(),
        }
    }
}

/// The single permission check. Handlers take the principal from their request context and never look at the connection.
pub fn authorize(principal: &Principal, permission: Permission) -> Result<(), ErrorBody> {
    if principal.holds(permission) {
        Ok(())
    } else {
        let who = match principal {
            Principal::Owner => "the owner".to_owned(),
            Principal::Device { id, .. } => format!("device {id}"),
        };
        Err(ErrorBody::new(
            codes::PERMISSION_DENIED,
            format!("{who} may not use {}", permission.as_str()),
        ))
    }
}
