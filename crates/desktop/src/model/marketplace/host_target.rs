//! Where a Marketplace item runs. The key does not move when other hosts are added or the list is sorted: `local` for
//! This computer, `runtime:<name>` for a host paired with this computer (pv96's keys, with the name the host has here).

use super::types::HostId;

/// The host a Marketplace read or change goes to: This computer, or a host paired with this computer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HostTarget {
    /// This computer's service; the pick when nothing else is.
    #[default]
    Local,
    /// A paired host, by the name this computer stores it under.
    Paired(String),
}

impl HostTarget {
    /// The stable key: `local`, or `runtime:` and the paired host's name.
    pub fn key(&self) -> String {
        match self {
            HostTarget::Local => "local".to_owned(),
            HostTarget::Paired(name) => format!("runtime:{name}"),
        }
    }

    /// The target a key names, or None for a key that names no host.
    pub fn from_key(key: &str) -> Option<Self> {
        if key == "local" {
            return Some(HostTarget::Local);
        }
        let name = key.strip_prefix("runtime:")?;
        (!name.is_empty()).then(|| HostTarget::Paired(name.to_owned()))
    }

    /// The id an item on this host carries.
    pub fn id(&self) -> HostId {
        HostId(self.key())
    }
}

#[cfg(test)]
mod tests {
    use super::HostTarget;

    #[test]
    fn this_computer_is_keyed_local_and_a_paired_host_by_its_name() {
        assert_eq!(HostTarget::Local.key(), "local");
        assert_eq!(HostTarget::Paired("vps".to_owned()).key(), "runtime:vps");
    }

    #[test]
    fn a_key_reads_back_to_the_target_it_names() {
        assert_eq!(HostTarget::from_key("local"), Some(HostTarget::Local));
        assert_eq!(
            HostTarget::from_key("runtime:vps"),
            Some(HostTarget::Paired("vps".to_owned()))
        );
    }

    #[test]
    fn a_key_of_another_kind_or_without_a_name_names_no_host() {
        for key in ["ssh:box", "wsl:Ubuntu", "runtime:", "vps", ""] {
            assert_eq!(HostTarget::from_key(key), None, "{key}");
        }
    }

    #[test]
    fn this_computer_is_the_default_pick() {
        assert_eq!(HostTarget::default(), HostTarget::Local);
    }
}
