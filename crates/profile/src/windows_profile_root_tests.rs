use std::cell::RefCell;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::{Disk, Links, RealDisk, read_sddl, secure_root, secure_root_with, set_dacl};
use crate::root_access::{MAX_LISTED_ENTRIES, RootAccess, RootAccessError};
use crate::windows_dir_access::Sid;
use crate::windows_sddl::Descriptor;
use crate::windows_trust::{Trusted, current_user, token_owner};

/// A user SID no account on the test machine has.
const STRANGER: &str = "S-1-5-21-1-2-3-4242";

/// A home inside a temporary folder; the temporary folder itself may be writable by others on a test machine.
struct Layout {
    _temp: TempDir,
    home: PathBuf,
    root: PathBuf,
}

impl Layout {
    fn new() -> Self {
        Self::under(&[])
    }

    fn under(segments: &[&str]) -> Self {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let mut home = temp.path().to_path_buf();
        for segment in segments {
            home.push(segment);
        }
        home.push("home");
        let root = home.join("profiles").join("default");
        Self {
            _temp: temp,
            home,
            root,
        }
    }

    fn secure(&self) -> Result<RootAccess, RootAccessError> {
        secure_root(&self.home, &self.root)
    }

    /// As `user`, with this process's real default owner, as a start would run: on an elevated administrator token
    /// that owner is Administrators, and what the test itself creates is owned by it.
    fn secure_as(&self, user: Sid, disk: &dyn Disk) -> Result<RootAccess, RootAccessError> {
        self.secure_for_token(user, token_owner().expect("the default owner"), disk)
    }

    /// As a token with this user and default owner.
    fn secure_for_token(
        &self,
        user: Sid,
        token_owner: Sid,
        disk: &dyn Disk,
    ) -> Result<RootAccess, RootAccessError> {
        let trusted = Trusted::for_token(user, token_owner).expect("the trusted SIDs");
        secure_root_with(&self.home, &self.root, trusted, disk)
    }

    /// Private ancestors, and a root made the ordinary way with `dacl` added to what it inherits.
    fn existing_root(dacl: &str) -> Self {
        let layout = Self::new();
        secure_root(&layout.home, &layout.home.join("profiles").join("seed"))
            .expect("safe ancestors");
        fs::create_dir(&layout.root).expect("an ordinary root");
        set_dacl(&layout.root, dacl, false).expect("the root's DACL");
        layout
    }
}

fn user() -> Sid {
    current_user().expect("the current user")
}

fn user_string() -> String {
    user().to_string()
}

fn descriptor_of(path: &Path) -> Descriptor {
    Descriptor::parse(&read_sddl(path, Links::Open).expect("the descriptor"))
}

fn sddl_of(path: &Path) -> String {
    read_sddl(path, Links::Open).expect("the descriptor")
}

/// Reads real descriptors, optionally rewritten, and records repairs instead of applying them when asked to.
#[derive(Default)]
struct TestDisk {
    /// Reported as owned by `STRANGER`.
    foreign: Vec<PathBuf>,
    /// Reported as owned by Administrators, as an elevated administrator token creates them.
    administrators_own: Vec<PathBuf>,
    /// The current user's SID string is reported as the alias `LA`, the built-in Administrator.
    as_administrator: bool,
    /// Repairs are recorded here and not applied.
    record_repairs: bool,
    repairs: RefCell<Vec<(PathBuf, String)>>,
    /// Removed when the scan reads them, as a service starting at the same time removes files: a folder after its
    /// descriptor is read, so the scan goes on to list it; a file before, so its read fails.
    vanish: Vec<PathBuf>,
}

impl Disk for TestDisk {
    fn read(&self, path: &Path, links: Links) -> io::Result<String> {
        if self.vanish.iter().any(|gone| gone == path) {
            if path.is_dir() {
                let sddl = RealDisk.read(path, links);
                fs::remove_dir_all(path).expect("removed");
                return sddl;
            }
            fs::remove_file(path).expect("removed");
        }
        let mut sddl = RealDisk.read(path, links)?;
        if self.as_administrator {
            sddl = sddl.replace(&user_string(), "LA");
        }
        let dacl = sddl[sddl.find("D:").unwrap_or(sddl.len())..].to_owned();
        if self.foreign.iter().any(|planted| planted == path) {
            sddl = format!("O:{STRANGER}{dacl}");
        } else if self.administrators_own.iter().any(|made| made == path) {
            sddl = format!("O:BA{dacl}");
        }
        Ok(sddl)
    }

    fn protect(&self, path: &Path, dacl: &str) -> io::Result<()> {
        if self.record_repairs {
            self.repairs
                .borrow_mut()
                .push((path.to_path_buf(), dacl.to_owned()));
            return Ok(());
        }
        RealDisk.protect(path, dacl)
    }
}

fn foreign(paths: Vec<PathBuf>) -> TestDisk {
    TestDisk {
        foreign: paths,
        ..TestDisk::default()
    }
}

fn refused_paths(result: Result<RootAccess, RootAccessError>) -> (&'static str, Vec<PathBuf>) {
    match result {
        Err(RootAccessError::ForeignEntries { paths }) => ("foreign", paths),
        Err(RootAccessError::WritableByOthers { paths }) => ("writable", paths),
        Err(RootAccessError::UnsafeLocation { path }) => ("unsafe", vec![path]),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_new_profile_root_and_its_ancestors_have_a_protected_dacl_with_one_ace_for_the_current_user() {
    let layout = Layout::new();

    assert_eq!(layout.secure().expect("created"), RootAccess::Created);

    for path in [&layout.home, &layout.home.join("profiles"), &layout.root] {
        let descriptor = descriptor_of(path);
        assert_eq!(descriptor.owner, Some(user()), "{}", path.display());
        assert!(
            descriptor.is_owner_only(&user()),
            "{}: {}",
            path.display(),
            sddl_of(path)
        );
    }
    assert_eq!(layout.secure().expect("checked"), RootAccess::Private);
}

#[test]
fn a_folder_created_later_inherits_only_the_owner_entry() {
    let layout = Layout::new();
    layout.secure().expect("created");
    let child = layout.root.join("agents").join("kinds");
    fs::create_dir_all(&child).expect("a child folder");

    // Compared as a SID: SDDL writes the built-in Administrator as `LA`, not as its `S-1-5-21-...-500` string.
    let sddl = sddl_of(&child);
    assert_eq!(
        descriptor_of(&child).sole_inherited_allow(),
        Some(&user()),
        "{sddl}"
    );
}

#[test]
fn a_root_only_trusted_users_could_reach_is_repaired_with_everything_below_it() {
    let layout = Layout::existing_root(&format!(
        "D:(A;OICI;FA;;;SY)(A;OICI;FA;;;{})",
        user_string()
    ));
    let kinds = layout.root.join("agents").join("kinds");
    fs::create_dir_all(&kinds).expect("a child folder");
    fs::write(kinds.join("tool.json"), b"{}").expect("a child file");
    assert!(sddl_of(&kinds.join("tool.json")).contains(";;;SY)"));

    assert_eq!(layout.secure().expect("repaired"), RootAccess::Repaired);

    assert!(descriptor_of(&layout.root).is_owner_only(&user()));
    for below in [kinds.clone(), kinds.join("tool.json")] {
        let sddl = sddl_of(&below);
        assert!(
            !sddl.contains(";;;SY)"),
            "{} keeps SYSTEM: {sddl}",
            below.display()
        );
    }
    assert_eq!(layout.secure().expect("checked"), RootAccess::Private);
}

#[test]
fn the_built_in_administrator_is_not_refused_as_a_foreign_owner() {
    let layout = Layout::new();
    layout.secure().expect("created");
    let kinds = layout.root.join("agents").join("kinds");
    fs::create_dir_all(&kinds).expect("a child folder");
    fs::write(kinds.join("tool.json"), b"{}").expect("a child file");
    let disk = TestDisk {
        as_administrator: true,
        record_repairs: true,
        ..TestDisk::default()
    };
    assert!(
        disk.read(&layout.root, Links::Open)
            .unwrap()
            .contains("O:LA")
    );

    let administrator = Sid::parse("LA").expect("the built-in Administrator");
    assert_eq!(
        layout.secure_as(administrator, &disk).expect("checked"),
        RootAccess::Private
    );
    assert!(
        disk.repairs.borrow().is_empty(),
        "a private root is not repaired again"
    );
}

#[test]
fn a_repair_for_the_built_in_administrator_grants_that_account_and_nobody_else() {
    let layout = Layout::existing_root(&format!(
        "D:(A;OICI;FA;;;SY)(A;OICI;FA;;;{})",
        user_string()
    ));
    let disk = TestDisk {
        as_administrator: true,
        record_repairs: true,
        ..TestDisk::default()
    };
    let administrator = Sid::parse("LA").expect("the built-in Administrator");

    assert_eq!(
        layout
            .secure_as(administrator.clone(), &disk)
            .expect("repaired"),
        RootAccess::Repaired
    );

    let repairs = disk.repairs.borrow();
    let [(path, dacl)] = repairs.as_slice() else {
        panic!("one repair, got {repairs:?}");
    };
    assert_eq!(path, &layout.root);
    let written = Descriptor::parse(&format!("O:LA{dacl}"));
    assert!(written.is_owner_only(&administrator), "{dacl}");
    assert!(
        !dacl.contains(";LA)"),
        "the account is written as its SID, not an alias: {dacl}"
    );
}

#[test]
fn a_root_another_sid_could_write_is_refused_and_left_as_it_is() {
    let layout = Layout::existing_root(&format!(
        "D:(A;OICI;FA;;;WD)(A;OICI;FA;;;{})",
        user_string()
    ));

    let (kind, paths) = refused_paths(layout.secure());

    assert_eq!((kind, paths), ("writable", vec![layout.root.clone()]));
    assert!(
        sddl_of(&layout.root).contains(";;;WD)"),
        "nothing was repaired"
    );
}

#[test]
fn write_access_the_root_inherited_from_profiles_is_refused() {
    let layout = Layout::new();
    let profiles = layout.home.join("profiles");
    secure_root(&layout.home, &profiles.join("seed")).expect("safe ancestors");
    // Inherit-only, so `profiles` itself stays safe and only what is created in it gets the entry.
    let dacl = format!("D:P(A;OICI;FA;;;{})(A;OICIIO;0x1301bf;;;AU)", user_string());
    set_dacl(&profiles, &dacl, true).expect("an inheritable grant");
    fs::create_dir(&layout.root).expect("an ordinary root");

    let (kind, paths) = refused_paths(layout.secure());

    assert_eq!((kind, paths), ("writable", vec![layout.root.clone()]));
}

#[test]
fn an_entry_below_a_private_root_that_others_can_write_is_refused() {
    let layout = Layout::new();
    layout.secure().expect("created");
    let settings = layout.root.join("agents").join("homes").join("claude");
    fs::create_dir_all(&settings).expect("an agent home");
    let file = settings.join("settings.json");
    fs::write(&file, b"{}").expect("a file");
    set_dacl(&file, "D:(A;;FA;;;WD)", false).expect("an explicit grant");

    let (kind, paths) = refused_paths(layout.secure());

    assert_eq!((kind, paths), ("writable", vec![file]));
}

/// A service starting at the same time creates and removes files in the root, such as the store's journal, while
/// this scan lists it.
#[test]
fn entries_removed_while_the_scan_lists_them_are_skipped() {
    let layout = Layout::new();
    layout.secure().expect("created");
    let file = layout.root.join("store.db-journal");
    fs::write(&file, b"x").expect("a file");
    let folder = layout.root.join("gone");
    fs::create_dir_all(folder.join("inner")).expect("folders");
    let disk = TestDisk {
        vanish: vec![file, folder],
        ..TestDisk::default()
    };

    let scanned = layout.secure_as(user(), &disk);

    assert!(matches!(scanned, Ok(RootAccess::Private)), "{scanned:?}");
}

#[test]
fn a_deep_entry_owned_by_another_user_is_refused() {
    let layout = Layout::new();
    layout.secure().expect("created");
    let deep = layout
        .root
        .join("agents")
        .join("homes")
        .join("claude")
        .join(".claude")
        .join("hooks");
    fs::create_dir_all(&deep).expect("deep folders");
    let planted = deep.join("planted.json");
    fs::write(&planted, b"{}").expect("a planted file");

    let refused = layout.secure_as(user(), &foreign(vec![planted.clone()]));

    assert_eq!(refused_paths(refused), ("foreign", vec![planted]));
}

#[test]
fn a_root_owned_by_another_user_is_refused_before_anything_is_repaired() {
    let layout = Layout::existing_root(&format!(
        "D:(A;OICI;FA;;;SY)(A;OICI;FA;;;{})",
        user_string()
    ));

    let refused = layout.secure_as(user(), &foreign(vec![layout.root.clone()]));

    assert_eq!(
        refused_paths(refused),
        ("foreign", vec![layout.root.clone()])
    );
    assert!(
        sddl_of(&layout.root).contains(";;;SY)"),
        "nothing was repaired"
    );
}

#[test]
fn a_refusal_names_at_most_ten_entries() {
    let layout = Layout::new();
    layout.secure().expect("created");
    let planted: Vec<PathBuf> = (0..12)
        .map(|index| layout.root.join(format!("{index}.json")))
        .collect();
    for path in &planted {
        fs::write(path, b"{}").expect("a planted file");
    }

    let (kind, paths) = refused_paths(layout.secure_as(user(), &foreign(planted.clone())));

    assert_eq!(kind, "foreign");
    assert_eq!(paths.len(), MAX_LISTED_ENTRIES);
    assert!(paths.iter().all(|path| planted.contains(path)));
}

#[test]
fn a_root_that_is_a_junction_is_refused() {
    let layout = Layout::new();
    let target = layout.home.join("profiles").join("seed");
    secure_root(&layout.home, &target).expect("safe ancestors");
    let made = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&layout.root)
        .arg(&target)
        .output()
        .expect("mklink runs");
    assert!(
        made.status.success(),
        "{}",
        String::from_utf8_lossy(&made.stderr)
    );

    assert_eq!(
        refused_paths(layout.secure()),
        ("unsafe", vec![layout.root.clone()])
    );
}

#[test]
fn a_home_other_users_can_change_is_refused() {
    let layout = Layout::new();
    layout.secure().expect("created");
    let dacl = format!("D:(A;;FA;;;WD)(A;OICI;FA;;;{})", user_string());
    set_dacl(&layout.home, &dacl, true).expect("a broad home");

    assert_eq!(
        refused_paths(layout.secure()),
        ("unsafe", vec![layout.home.clone()])
    );
}

#[test]
fn a_profiles_folder_another_user_owns_is_refused() {
    let layout = Layout::new();
    layout.secure().expect("created");
    let profiles = layout.home.join("profiles");

    let refused = layout.secure_as(user(), &foreign(vec![profiles.clone()]));

    assert_eq!(refused_paths(refused), ("unsafe", vec![profiles]));
}

#[test]
fn a_profile_past_the_old_path_limit_is_created_and_repaired() {
    let long = "l".repeat(120);
    let layout = Layout::under(&[&long, &long]);
    assert!(
        layout.root.as_os_str().len() > 260,
        "{}",
        layout.root.display()
    );

    assert_eq!(layout.secure().expect("created"), RootAccess::Created);
    let dacl = format!("D:(A;OICI;FA;;;SY)(A;OICI;FA;;;{})", user_string());
    set_dacl(&layout.root, &dacl, false).expect("a trusted but unprotected DACL");
    assert_eq!(layout.secure().expect("repaired"), RootAccess::Repaired);
    assert!(descriptor_of(&layout.root).is_owner_only(&user()));
}

#[test]
fn entries_an_elevated_token_created_are_owned_by_administrators_and_accepted() {
    let layout = Layout::new();
    layout.secure().expect("created");
    let agents = layout.root.join("agents");
    fs::create_dir_all(agents.join("kinds")).expect("folders");
    let disk = TestDisk {
        administrators_own: vec![agents.clone(), agents.join("kinds")],
        ..TestDisk::default()
    };
    let administrators = Sid::parse("BA").expect("Administrators");

    assert_eq!(
        layout
            .secure_for_token(user(), administrators, &disk)
            .expect("checked"),
        RootAccess::Private
    );
}

/// Makes the next start run a full scan, as a day-old cache would.
fn age_cache(root: &Path) {
    let cache = root.join(crate::scan_cache::CACHE_FILE);
    let text = fs::read_to_string(&cache).expect("the cache");
    let (header, rest) = text.split_once('\n').expect("a header");
    let (version, _) = header.split_once('\t').expect("version and time");
    fs::write(&cache, format!("{version}\t0\n{rest}")).expect("a day-old cache");
}

#[test]
fn an_elevated_token_accepts_the_root_folders_and_cache_it_created_owned_by_administrators() {
    let layout = Layout::new();
    layout.secure().expect("created");
    let kinds = layout.root.join("agents").join("kinds");
    fs::create_dir_all(&kinds).expect("folders");
    fs::write(kinds.join("tool.json"), b"{}").expect("a file");
    layout.secure().expect("scanned, which writes the cache");
    age_cache(&layout.root);
    let disk = TestDisk {
        administrators_own: vec![
            layout.root.clone(),
            layout.root.join("agents"),
            kinds.clone(),
            kinds.join("tool.json"),
            layout.root.join(crate::scan_cache::CACHE_FILE),
        ],
        ..TestDisk::default()
    };
    let administrators = Sid::parse("BA").expect("Administrators");

    assert_eq!(
        layout
            .secure_for_token(user(), administrators, &disk)
            .expect("an elevated start"),
        RootAccess::Private
    );
}

#[test]
fn entries_owned_by_administrators_are_foreign_to_a_token_that_does_not_default_to_them() {
    let layout = Layout::new();
    layout.secure().expect("created");
    let agents = layout.root.join("agents");
    fs::create_dir_all(&agents).expect("a folder");
    let disk = TestDisk {
        administrators_own: vec![agents.clone()],
        ..TestDisk::default()
    };

    let refused = layout.secure_for_token(user(), user(), &disk);

    assert_eq!(refused_paths(refused), ("foreign", vec![agents]));
}

/// Prints how long a start takes on a profile with a large agent home. Run with `-- --ignored --nocapture`.
#[test]
#[ignore = "measures the scan; slow"]
fn scan_cost_of_a_large_agent_home() {
    let layout = Layout::new();
    layout.secure().expect("created");
    let home = layout.root.join("agents").join("homes").join("claude");
    for outer in 0..30 {
        for inner in 0..10 {
            let dir = home.join(format!("pkg{outer}")).join(format!("lib{inner}"));
            fs::create_dir_all(&dir).expect("folders");
            for file in 0..100 {
                fs::write(dir.join(format!("f{file}.js")), b"x").expect("a file");
            }
        }
    }
    for round in 0..3 {
        let started = std::time::Instant::now();
        assert_eq!(layout.secure().expect("checked"), RootAccess::Private);
        let elapsed = started.elapsed();
        let _ = writeln!(io::stderr(), "scan round {round}: {elapsed:?}");
    }
}

#[test]
fn an_entry_planted_after_a_clean_scan_is_found_in_its_changed_folder() {
    let layout = Layout::new();
    layout.secure().expect("created");
    let kinds = layout.root.join("agents").join("kinds");
    fs::create_dir_all(&kinds).expect("a folder");
    fs::write(kinds.join("tool.json"), b"{}").expect("a file");
    assert_eq!(layout.secure().expect("scanned"), RootAccess::Private);
    let planted = kinds.join("planted.json");
    fs::write(&planted, b"{}").expect("a planted file");

    let refused = layout.secure_as(user(), &foreign(vec![planted.clone()]));

    assert_eq!(refused_paths(refused), ("foreign", vec![planted]));
}

/// A junction home whose parent folder `parent_dacl` describes.
fn link_home(parent_dacl: &str) -> (TempDir, PathBuf, PathBuf) {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let parent = temp.path().join("parent");
    crate::root_access::create_home(&parent).expect("a private parent");
    let real = parent.join("real-home");
    crate::root_access::create_home(&real).expect("a private home");
    let home = parent.join("home");
    let made = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&home)
        .arg(&real)
        .output()
        .expect("mklink runs");
    assert!(
        made.status.success(),
        "{}",
        String::from_utf8_lossy(&made.stderr)
    );
    set_dacl(&parent, parent_dacl, true).expect("the parent's DACL");
    let root = home.join("profiles").join("default");
    (temp, home, root)
}

#[test]
fn a_junction_home_whose_parent_others_can_change_is_refused() {
    let (_temp, home, root) = link_home(&format!("D:P(A;OICI;FA;;;{})(A;;FA;;;WD)", user_string()));

    assert_eq!(
        refused_paths(secure_root(&home, &root)),
        ("unsafe", vec![home.clone()])
    );
}

#[test]
fn a_junction_home_in_a_private_parent_is_accepted() {
    let (_temp, home, root) = link_home(&format!("D:P(A;OICI;FA;;;{})", user_string()));

    assert_eq!(
        secure_root(&home, &root).expect("accepted"),
        RootAccess::Created
    );
}

#[test]
fn a_changed_access_list_on_an_existing_deep_entry_is_caught_by_the_next_full_scan() {
    let layout = Layout::new();
    layout.secure().expect("created");
    let deep = layout
        .root
        .join("agents")
        .join("homes")
        .join("claude")
        .join(".claude");
    fs::create_dir_all(&deep).expect("deep folders");
    let settings = deep.join("settings.json");
    fs::write(&settings, b"{}").expect("a file");
    assert_eq!(layout.secure().expect("full scan"), RootAccess::Private);

    set_dacl(&settings, "D:(A;;FA;;;WD)", false).expect("a grant to everyone");
    // The folder's time did not change, so a start within the day skips it where the cache is used.
    if super::folder_times_are_reliable(&layout.root) {
        assert_eq!(layout.secure().expect("cached scan"), RootAccess::Private);
    }
    age_cache(&layout.root);

    let (kind, paths) = refused_paths(layout.secure());

    assert_eq!((kind, paths), ("writable", vec![settings]));
}

#[test]
fn the_temporary_folder_of_this_test_machine_is_on_local_ntfs() {
    let layout = Layout::new();
    layout.secure().expect("created");
    assert!(super::folder_times_are_reliable(&layout.root));
}
