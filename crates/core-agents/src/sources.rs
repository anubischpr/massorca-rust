//! Where a kind's CLI keeps its credential files by default, for an import without a folder (decision 0097).
//!
//! Each declared file may name its default folder, per platform, and the vendor's own override variable. Resolution is a
//! pure function of the platform, the user's home folder and an environment lookup, so tests never touch the real
//! home. Detection reads metadata only, never a file's contents.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use massorca_protocol::codes;
use massorca_protocol::methods::agents::{CandidateFile, ImportCandidate};

use crate::catalog::Catalog;
use crate::credentials;
use crate::kind::{CredentialSource, CredentialsDecl, KindDecl, remote_path};
use crate::sessions::{AgentError, Agents};

/// The platform a source's folder template is chosen for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    Macos,
    Linux,
    Other,
}

impl Platform {
    /// The platform this process runs on.
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }
}

/// An environment lookup: the value of a variable, or `None` when it is unset.
pub type EnvLookup<'a> = &'a dyn Fn(&str) -> Option<OsString>;

/// The folder a declared file is looked for in, and the override variable when that folder came from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    pub dir: PathBuf,
    pub env: Option<String>,
}

/// One declared file as an import without a folder sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedFile {
    pub name: String,
    pub required: bool,
    /// Where it is looked for; `None` when the kind declares no location for it, or the location cannot be resolved.
    pub path: Option<PathBuf>,
    /// The override variable, when it was set and used.
    pub env: Option<String>,
    /// The import would accept it, judged from its metadata alone.
    pub found: bool,
    /// Why the import would not use a file that is there: `a link`, `a directory`, `larger than 1 MiB`...
    pub reason: Option<String>,
}

/// The value of `name` when it is set, not empty and an absolute path; anything else counts as unset.
fn absolute_var(env: EnvLookup<'_>, name: &str) -> Option<PathBuf> {
    env(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

/// The user's home folder as the vendor CLIs read it: `USERPROFILE` on Windows, `HOME` elsewhere, when it is set to an
/// absolute path.
pub fn user_home(platform: Platform, env: EnvLookup<'_>) -> Option<PathBuf> {
    let name = if platform == Platform::Windows {
        "USERPROFILE"
    } else {
        "HOME"
    };
    absolute_var(env, name)
}

/// The home of this process: [`user_home`] over its own environment, or else the platform's own answer.
pub fn service_home() -> Option<PathBuf> {
    user_home(Platform::current(), &|name| std::env::var_os(name)).or_else(std::env::home_dir)
}

/// A folder template with `{home}` replaced, as an absolute path; `None` when it needs a home and there is none, when it
/// climbs with `..`, or when the result is not absolute on this platform.
pub fn expand(template: &str, home: Option<&Path>) -> Option<PathBuf> {
    let (mut path, rest) = match template.strip_prefix("{home}") {
        Some(rest) => (home?.to_path_buf(), rest),
        None => (PathBuf::from(template), ""),
    };
    for part in rest.split(['/', '\\']).filter(|part| !part.is_empty()) {
        path.push(part);
    }
    let climbs = path
        .components()
        .any(|component| component == Component::ParentDir);
    (!climbs && path.is_absolute()).then_some(path)
}

/// The override variable `name` as a folder: set to a local absolute path that does not climb with `..`. A network or
/// device path (`\\server`, `\\?\`, `//server`) counts as unset, so detection never reaches another machine.
pub(crate) fn override_dir(env: EnvLookup<'_>, name: &str) -> Option<PathBuf> {
    absolute_var(env, name).filter(|dir| {
        !remote_path(&dir.to_string_lossy())
            && !dir
                .components()
                .any(|component| component == Component::ParentDir)
    })
}

/// Where `source` points on `platform`: the override variable's folder when it is set to a usable path (see
/// [`override_dir`]), else the platform's template, else `dir`.
pub fn resolve(
    source: &CredentialSource,
    platform: Platform,
    home: Option<&Path>,
    env: EnvLookup<'_>,
) -> Option<Located> {
    if let Some(name) = &source.env
        && let Some(dir) = override_dir(env, name)
    {
        return Some(Located {
            dir,
            env: Some(name.clone()),
        });
    }
    let template = match platform {
        Platform::Windows => source.windows.as_ref(),
        Platform::Macos => source.macos.as_ref(),
        Platform::Linux => source.linux.as_ref(),
        Platform::Other => None,
    }
    .unwrap_or(&source.dir);
    expand(template, home).map(|dir| Located { dir, env: None })
}

/// Every declared file of `credentials`, where it is looked for and whether the import would accept it there.
pub fn detect(
    credentials: &CredentialsDecl,
    platform: Platform,
    home: Option<&Path>,
    env: EnvLookup<'_>,
) -> Vec<DetectedFile> {
    credentials
        .files
        .iter()
        .map(|name| {
            let located = credentials
                .sources
                .iter()
                .find(|source| &source.file == name)
                .and_then(|source| resolve(source, platform, home, env));
            let path = located.as_ref().map(|located| located.dir.join(name));
            let cap = credentials.source_cap(name);
            let judged = path.as_deref().map(|path| credentials::judge(path, cap));
            DetectedFile {
                name: name.clone(),
                required: credentials.required(name),
                found: matches!(judged, Some(Ok(()))),
                reason: match judged {
                    Some(Err(reason)) if reason != credentials::MISSING => Some(reason),
                    _ => None,
                },
                env: located.and_then(|located| located.env),
                path,
            }
        })
        .collect()
}

/// The wire form of a kind's detection: `ready` when every required file was found, `folder` the folder of the first
/// required file.
pub fn candidate(kind: &str, files: &[DetectedFile]) -> ImportCandidate {
    let ready = files
        .iter()
        .filter(|file| file.required)
        .all(|file| file.found)
        && files.iter().any(|file| file.found);
    let folder = files
        .iter()
        .find(|file| file.required)
        .and_then(|file| file.path.as_deref())
        .and_then(Path::parent)
        .map(|dir| dir.display().to_string());
    ImportCandidate {
        kind: kind.to_owned(),
        ready,
        folder,
        files: files
            .iter()
            .map(|file| CandidateFile {
                name: file.name.clone(),
                required: file.required,
                found: file.found,
                path: file.path.as_ref().map(|path| path.display().to_string()),
                env: file.env.clone(),
                reason: file.reason.clone(),
            })
            .collect(),
    }
}

/// [`detect`] for this process: its platform, its home and its environment.
pub fn detect_here(credentials: &CredentialsDecl) -> Vec<DetectedFile> {
    let home = service_home();
    detect(credentials, Platform::current(), home.as_deref(), &|name| {
        std::env::var_os(name)
    })
}

/// The declared credentials of a kind, when it declares files to import.
fn importable(decl: &KindDecl) -> Option<&CredentialsDecl> {
    decl.credentials
        .as_ref()
        .filter(|credentials| !credentials.files.is_empty())
}

impl Agents {
    /// Where an import without a folder would copy each kind's declared files from (decision 0097): one kind, or every
    /// valid, enabled kind that declares credential files. Metadata only; nothing is read or copied.
    pub fn import_candidates(
        &self,
        kind: Option<&str>,
    ) -> Result<Vec<ImportCandidate>, AgentError> {
        let catalog = Catalog::load(self.kind_packages(), self.profile_dir());
        if let Some(kind) = kind {
            let entry = catalog.find(kind).ok_or_else(|| {
                AgentError::new(codes::AGENT_KIND_NOT_FOUND, format!("no agent kind {kind}"))
            })?;
            let decl = entry.valid().ok_or_else(|| {
                AgentError::new(
                    codes::AGENT_CREDENTIALS_UNSUPPORTED,
                    format!(
                        "agent kind {kind} is invalid, so it has no usable credentials: {}",
                        entry.problems.join("; ")
                    ),
                )
            })?;
            let credentials = importable(decl).ok_or_else(|| {
                AgentError::new(
                    codes::AGENT_CREDENTIALS_UNSUPPORTED,
                    format!("agent kind {kind} declares no credential files to import"),
                )
            })?;
            return Ok(vec![candidate(&decl.id, &detect_here(credentials))]);
        }
        Ok(catalog
            .entries
            .iter()
            .filter(|entry| entry.enabled())
            .filter_map(|entry| entry.valid())
            .filter_map(|decl| {
                importable(decl).map(|credentials| candidate(&decl.id, &detect_here(credentials)))
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use super::*;
    use crate::credentials::MAX_FILE_BYTES;

    fn source(file: &str, env: Option<&str>, dir: &str) -> CredentialSource {
        CredentialSource {
            file: file.to_owned(),
            env: env.map(str::to_owned),
            dir: dir.to_owned(),
            windows: None,
            macos: None,
            linux: None,
        }
    }

    /// An environment of exactly `vars`.
    fn env_of(vars: &[(&str, &Path)]) -> impl Fn(&str) -> Option<OsString> + use<> {
        let vars: BTreeMap<String, OsString> = vars
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.as_os_str().to_owned()))
            .collect();
        move |name| vars.get(name).cloned()
    }

    fn no_env(_: &str) -> Option<OsString> {
        None
    }

    fn fake_credentials(dir_a: &str, dir_b: &str) -> CredentialsDecl {
        CredentialsDecl {
            home_env: "FAKE_AGENT_HOME".to_owned(),
            files: vec!["token.fake".to_owned(), "extra.fake".to_owned()],
            strip_env: Vec::new(),
            optional: vec!["extra.fake".to_owned()],
            sources: vec![
                source("token.fake", Some("FAKE_VENDOR_DIR"), dir_a),
                source("extra.fake", Some("FAKE_VENDOR_DIR"), dir_b),
            ],
            state: None,
        }
    }

    #[test]
    fn home_comes_from_userprofile_on_windows_and_home_elsewhere() {
        let windows_home = tempfile::tempdir().unwrap();
        let unix_home = tempfile::tempdir().unwrap();
        let env = env_of(&[
            ("USERPROFILE", windows_home.path()),
            ("HOME", unix_home.path()),
        ]);
        assert_eq!(
            user_home(Platform::Windows, &env).as_deref(),
            Some(windows_home.path())
        );
        for platform in [Platform::Linux, Platform::Macos, Platform::Other] {
            assert_eq!(user_home(platform, &env).as_deref(), Some(unix_home.path()));
        }
        let relative = env_of(&[("HOME", Path::new("relative/home"))]);
        assert_eq!(user_home(Platform::Linux, &relative), None);
        assert_eq!(user_home(Platform::Linux, &no_env), None);
    }

    #[test]
    fn home_is_substituted_and_the_platform_template_wins_over_dir() {
        let home = tempfile::tempdir().unwrap();
        let mut declared = source("token.fake", None, "{home}/.fake");
        declared.windows = Some("{home}\\AppData\\Fake".to_owned());
        declared.linux = Some("{home}/.config/fake".to_owned());

        let on = |platform| {
            resolve(&declared, platform, Some(home.path()), &no_env)
                .unwrap()
                .dir
        };
        assert_eq!(
            on(Platform::Windows),
            home.path().join("AppData").join("Fake")
        );
        assert_eq!(
            on(Platform::Linux),
            home.path().join(".config").join("fake")
        );
        assert_eq!(on(Platform::Macos), home.path().join(".fake"));
        assert_eq!(on(Platform::Other), home.path().join(".fake"));

        assert_eq!(
            expand("{home}", Some(home.path())).as_deref(),
            Some(home.path())
        );
        assert_eq!(expand("{home}/.fake", None), None, "no home, no folder");
        assert_eq!(expand("{home}/../elsewhere", Some(home.path())), None);
        let absolute = home.path().join("fixed");
        assert_eq!(
            expand(absolute.to_str().unwrap(), None).as_deref(),
            Some(absolute.as_path())
        );
    }

    #[test]
    fn the_override_variable_is_used_only_when_set_and_absolute() {
        let home = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let declared = source("token.fake", Some("FAKE_VENDOR_DIR"), "{home}/.fake");

        let set = env_of(&[("FAKE_VENDOR_DIR", elsewhere.path())]);
        assert_eq!(
            resolve(&declared, Platform::Linux, Some(home.path()), &set),
            Some(Located {
                dir: elsewhere.path().to_path_buf(),
                env: Some("FAKE_VENDOR_DIR".to_owned()),
            })
        );
        let unset = resolve(&declared, Platform::Linux, Some(home.path()), &no_env).unwrap();
        assert_eq!(unset.dir, home.path().join(".fake"));
        assert_eq!(unset.env, None);

        let relative = env_of(&[("FAKE_VENDOR_DIR", Path::new("relative/dir"))]);
        let ignored = resolve(&declared, Platform::Linux, Some(home.path()), &relative).unwrap();
        assert_eq!(
            ignored.dir,
            home.path().join(".fake"),
            "a relative override is ignored"
        );
        assert_eq!(ignored.env, None);

        let empty = |name: &str| (name == "FAKE_VENDOR_DIR").then(OsString::new);
        assert_eq!(
            resolve(&declared, Platform::Linux, Some(home.path()), &empty)
                .unwrap()
                .env,
            None
        );
    }

    #[test]
    fn detection_finds_each_file_in_its_own_folder_and_marks_missing_ones() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join(".fake")).unwrap();
        fs::write(home.path().join(".fake").join("token.fake"), "fake").unwrap();
        fs::write(home.path().join("extra.fake"), "fake").unwrap();
        let credentials = fake_credentials("{home}/.fake", "{home}");

        let files = detect(&credentials, Platform::Linux, Some(home.path()), &no_env);
        assert_eq!(
            files,
            [
                DetectedFile {
                    name: "token.fake".to_owned(),
                    required: true,
                    path: Some(home.path().join(".fake").join("token.fake")),
                    env: None,
                    found: true,
                    reason: None,
                },
                DetectedFile {
                    name: "extra.fake".to_owned(),
                    required: false,
                    path: Some(home.path().join("extra.fake")),
                    env: None,
                    found: true,
                    reason: None,
                },
            ]
        );
        let shown = candidate("fake", &files);
        assert!(shown.ready);
        assert_eq!(
            shown.folder,
            Some(home.path().join(".fake").display().to_string())
        );

        fs::remove_file(home.path().join("extra.fake")).unwrap();
        let files = detect(&credentials, Platform::Linux, Some(home.path()), &no_env);
        assert!(!files[1].found);
        assert!(
            candidate("fake", &files).ready,
            "a missing optional file keeps it ready"
        );

        fs::remove_file(home.path().join(".fake").join("token.fake")).unwrap();
        let files = detect(&credentials, Platform::Linux, Some(home.path()), &no_env);
        assert!(!files[0].found);
        assert!(!candidate("fake", &files).ready);
    }

    #[test]
    fn a_state_file_is_found_up_to_its_own_cap_not_the_one_mib_of_other_files() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join(".fake")).unwrap();
        fs::write(home.path().join(".fake").join("token.fake"), "fake").unwrap();
        let two_mib = 2 * 1024 * 1024;
        fs::write(home.path().join("extra.fake"), vec![b' '; two_mib]).unwrap();
        let mut credentials = fake_credentials("{home}/.fake", "{home}");
        assert_eq!(
            detect(&credentials, Platform::Linux, Some(home.path()), &no_env)[1].reason,
            Some("larger than 1 MiB".to_owned()),
            "without a state rule the file is held to 1 MiB"
        );

        credentials.state = Some(crate::kind::StateDecl {
            file: "extra.fake".to_owned(),
            keep: vec!["theme".to_owned()],
            set: BTreeMap::new(),
            max_source_bytes: 3 * 1024 * 1024,
        });
        let files = detect(&credentials, Platform::Linux, Some(home.path()), &no_env);
        assert!(files[1].found && files[1].reason.is_none(), "{files:?}");

        credentials.state.as_mut().unwrap().max_source_bytes = 1024 * 1024 + 1;
        let files = detect(&credentials, Platform::Linux, Some(home.path()), &no_env);
        assert!(!files[1].found);
        assert_eq!(
            files[1].reason.as_deref(),
            Some("larger than 1048577 bytes")
        );
    }

    #[test]
    fn detection_follows_the_override_variable() {
        let home = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        fs::write(elsewhere.path().join("token.fake"), "fake").unwrap();
        let env = env_of(&[("FAKE_VENDOR_DIR", elsewhere.path())]);
        let files = detect(
            &fake_credentials("{home}/.fake", "{home}"),
            Platform::Linux,
            Some(home.path()),
            &env,
        );
        assert_eq!(files[0].path, Some(elsewhere.path().join("token.fake")));
        assert_eq!(files[0].env.as_deref(), Some("FAKE_VENDOR_DIR"));
        assert!(files[0].found);
    }

    #[test]
    fn detection_refuses_links_directories_and_oversized_files_without_reading_them() {
        let home = tempfile::tempdir().unwrap();
        let credentials = fake_credentials("{home}", "{home}");
        let token = home.path().join("token.fake");

        // Oversized: a sparse file of the limit plus one byte, judged by its length.
        let file = fs::File::create(&token).unwrap();
        file.set_len(MAX_FILE_BYTES + 1).unwrap();
        drop(file);
        let files = detect(&credentials, Platform::Linux, Some(home.path()), &no_env);
        assert!(!files[0].found, "larger than the limit");
        assert_eq!(files[0].reason.as_deref(), Some("larger than 1 MiB"));
        assert_eq!(files[1].reason, None, "a missing file has no reason");

        fs::File::create(&token)
            .unwrap()
            .set_len(MAX_FILE_BYTES)
            .unwrap();
        assert!(detect(&credentials, Platform::Linux, Some(home.path()), &no_env)[0].found);

        fs::remove_file(&token).unwrap();
        fs::create_dir(&token).unwrap();
        assert!(!detect(&credentials, Platform::Linux, Some(home.path()), &no_env)[0].found);
        fs::remove_dir(&token).unwrap();

        let target = home.path().join("target.fake");
        fs::write(&target, "fake").unwrap();
        if make_link(&target, &token).is_ok() {
            assert!(
                !detect(&credentials, Platform::Linux, Some(home.path()), &no_env)[0].found,
                "a link is refused"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn detection_never_opens_the_file() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().unwrap();
        let token = home.path().join("token.fake");
        fs::write(&token, "fake").unwrap();
        fs::set_permissions(&token, fs::Permissions::from_mode(0o000)).unwrap();
        let files = detect(
            &fake_credentials("{home}", "{home}"),
            Platform::Linux,
            Some(home.path()),
            &no_env,
        );
        assert!(
            files[0].found,
            "an unreadable file is judged by its metadata alone"
        );
    }

    #[cfg(unix)]
    fn make_link(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn make_link(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_file(target, link)
    }

    #[test]
    fn a_network_device_or_climbing_override_counts_as_unset() {
        let home = tempfile::tempdir().unwrap();
        let declared = source("token.fake", Some("FAKE_VENDOR_DIR"), "{home}/.fake");
        let climbing = home.path().join("..").join("elsewhere");
        let refused: Vec<PathBuf> = vec![
            PathBuf::from(r"\\server\share\fake"),
            PathBuf::from(r"\\?\C:\fake"),
            PathBuf::from(r"\\.\pipe\fake"),
            PathBuf::from("//server/share/fake"),
            climbing,
        ];
        for value in refused {
            let env = env_of(&[("FAKE_VENDOR_DIR", value.as_path())]);
            let located = resolve(&declared, Platform::Linux, Some(home.path()), &env).unwrap();
            assert_eq!(
                located.dir,
                home.path().join(".fake"),
                "{value:?} is ignored"
            );
            assert_eq!(located.env, None, "{value:?} is ignored");
        }
    }

    #[test]
    fn a_file_without_a_declared_location_is_not_found_and_has_no_path() {
        let mut credentials = fake_credentials("{home}", "{home}");
        credentials.sources.clear();
        let files = detect(&credentials, Platform::Linux, None, &no_env);
        assert!(files.iter().all(|file| file.path.is_none() && !file.found));
        let shown = candidate("fake", &files);
        assert!(!shown.ready);
        assert_eq!(shown.folder, None);
    }
}
