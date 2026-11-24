//! The desktop's folder in the profile, private to its owner like the profile's other private folders (ADR 0073).

use crate::private_dir::{PrivateDir, open_private_dir};
use crate::profile::{Profile, ensure_dir};
use crate::root_access::RootAccessError;

/// Opens `<profile>/desktop`, creating the profile and that folder when they are missing.
pub fn open_desktop_dir(profile: &Profile) -> Result<PrivateDir, RootAccessError> {
    ensure_dir(profile)?;
    open_private_dir(&profile.desktop_dir())
}
