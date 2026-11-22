//! A temporary profile with a service over a fake host.

use std::sync::Arc;

use massorca_core_extensions::source::Source;
use massorca_core_extensions::{Error, Extensions};
use massorca_profile::Profile;
use serde_json::Value;

use crate::fakes::FakeHost;
use crate::fixture::{package_files, write_dir};

pub struct Setup {
    pub home: tempfile::TempDir,
    pub profile: Profile,
    pub host: Arc<FakeHost>,
    pub service: Extensions,
}

pub fn setup(host: FakeHost) -> Setup {
    let home = tempfile::tempdir().expect("temp home");
    let profile = Profile::resolve_with(
        Some(home.path().to_path_buf()),
        Some("test".to_owned()),
        |_| None,
    )
    .expect("profile");
    let host = Arc::new(host);
    let service = Extensions::new(&profile, host.clone(), host.clone());
    Setup {
        home,
        profile,
        host,
        service,
    }
}

impl Setup {
    /// Writes a package folder under the temporary home and returns its source.
    pub fn package_dir(&self, name: &str, manifest: &Value) -> Source {
        let dir = self.home.path().join("src").join(name);
        write_dir(&dir, &package_files(manifest));
        Source::Dir(dir)
    }

    /// Installs with the consent of a fresh review.
    pub fn install(&self, source: &Source, run_installer: bool) -> Result<(), Error> {
        let review = self.service.inspect(source).expect("inspect");
        self.service
            .install(source, Some(&review.consent), run_installer, None)
            .map(|_| ())
    }

    pub fn lock_json(&self) -> Option<Value> {
        let path = self.service.layout().lock_path();
        path.exists().then(|| {
            serde_json::from_slice(&std::fs::read(path).expect("read lock")).expect("json")
        })
    }

    pub fn exists(&self, rel: &str) -> bool {
        self.service.layout().resolve(rel).exists()
    }
}
