//! The listed images of a package and their fetch state, keyed by package, version and path. No GPUI types here.

use std::collections::BTreeMap;

use super::types::{HostId, MarketplaceItem};

/// One listed image: the host the package is on, the package, the version it is listed at and the path that names it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ImageKey {
    /// The host the package is on; its listed images are fetched from that host.
    pub host: HostId,
    pub package: String,
    pub version: String,
    pub path: String,
}

impl MarketplaceItem {
    /// The key of the logo the package lists, when it lists one.
    pub fn logo_key(&self) -> Option<ImageKey> {
        let entry = self.extension.as_deref()?;
        let path = entry.listing.logo.as_deref()?;
        Some(self.image_key(&entry.id, path))
    }

    /// The key of the banner image the package lists, when it lists one.
    pub fn banner_key(&self) -> Option<ImageKey> {
        let entry = self.extension.as_deref()?;
        let path = entry.listing.banner.as_ref()?.image.as_deref()?;
        Some(self.image_key(&entry.id, path))
    }

    fn image_key(&self, package: &str, path: &str) -> ImageKey {
        ImageKey {
            host: self.host.clone(),
            package: package.to_owned(),
            version: self.version.clone(),
            path: path.to_owned(),
        }
    }
}

/// Where one listed image's fetch stands.
#[derive(Debug)]
enum ImageSlot<T> {
    Loading,
    Ready(T),
    Failed,
}

/// The fetch state of every listed image, by key. Generic over the loaded value, so the policy tests as plain Rust.
#[derive(Debug)]
pub struct ImageSlots<T> {
    slots: BTreeMap<ImageKey, ImageSlot<T>>,
}

impl<T> Default for ImageSlots<T> {
    fn default() -> Self {
        Self {
            slots: BTreeMap::new(),
        }
    }
}

impl<T: Clone> ImageSlots<T> {
    /// The image a key's fetch loaded, if it did.
    pub fn loaded(&self, key: &ImageKey) -> Option<T> {
        match self.slots.get(key) {
            Some(ImageSlot::Ready(image)) => Some(image.clone()),
            _ => None,
        }
    }

    /// Starts a key's fetch the first time it is asked for. True when the caller must send that fetch.
    pub fn begin(&mut self, key: &ImageKey) -> bool {
        if self.slots.contains_key(key) {
            return false;
        }
        self.slots.insert(key.clone(), ImageSlot::Loading);
        true
    }

    /// Records how a fetch ended: with its image, or without one when the call failed or the data did not decode.
    pub fn finish(&mut self, key: &ImageKey, image: Option<T>) {
        let slot = image.map_or(ImageSlot::Failed, ImageSlot::Ready);
        self.slots.insert(key.clone(), slot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(version: &str) -> ImageKey {
        ImageKey {
            host: HostId("local".to_owned()),
            package: "acme.notes".to_owned(),
            version: version.to_owned(),
            path: "logo.png".to_owned(),
        }
    }

    #[test]
    fn a_loaded_image_is_kept_for_its_version() {
        let mut slots = ImageSlots::default();
        assert!(slots.begin(&key("1.0.0")), "the first ask starts the fetch");
        slots.finish(&key("1.0.0"), Some("logo"));
        assert_eq!(slots.loaded(&key("1.0.0")), Some("logo"));
        assert!(
            !slots.begin(&key("1.0.0")),
            "the same version is not fetched again"
        );
    }

    #[test]
    fn a_new_version_fetches_its_image_again() {
        let mut slots = ImageSlots::default();
        slots.begin(&key("1.0.0"));
        slots.finish(&key("1.0.0"), Some("logo"));
        assert!(slots.begin(&key("2.0.0")));
        assert_eq!(
            slots.loaded(&key("2.0.0")),
            None,
            "nothing is loaded until the new fetch answers"
        );
    }

    #[test]
    fn a_failed_fetch_is_not_started_again() {
        let mut slots = ImageSlots::default();
        slots.begin(&key("1.0.0"));
        slots.finish(&key("1.0.0"), None::<&str>);
        assert_eq!(slots.loaded(&key("1.0.0")), None);
        assert!(
            !slots.begin(&key("1.0.0")),
            "a failed fetch is kept, not retried"
        );
    }
}
