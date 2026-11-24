//! One discovery: scans every root, keeps each skill once and reports each root with why it was or was not read.

use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::frontmatter::read_skill_summary;
use super::roots::{SkillProvider, SkillRoot, SkillRootEnv, SkillSourceKind, discovery_roots};
use super::walk::find_skill_files;
use crate::source::sha256_hex;

/// What to scan: the user's home folder, and the host's folder overrides.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscoveryRequest {
    /// The user's home folder, where agent tools keep their skills.
    pub home: PathBuf,
    /// Registered folders, git repositories or plain folders; each contributes its own skill roots.
    pub folders: Vec<PathBuf>,
    pub env: SkillRootEnv,
    /// Only skills of these kinds; empty keeps every kind.
    pub source_kinds: Vec<SkillSourceKind>,
}

/// One skill found on disk. A skill in several roots is listed once, with every root in `root_paths`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredSkill {
    /// A stable id: a hash of the canonical path of its `SKILL.md`.
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub providers: Vec<SkillProvider>,
    pub source_kind: SkillSourceKind,
    pub source_label: String,
    /// The first root that reached this skill.
    pub root_path: PathBuf,
    pub root_paths: Vec<PathBuf>,
    pub directory_path: PathBuf,
    pub skill_file_path: PathBuf,
    /// Every skill found on disk is installed, and a copy in any root keeps the listed skill installed.
    pub installed: bool,
    pub updated_at_ms: Option<i64>,
    /// Frontmatter `disable-slash-command: true`: discoverable, but not offered in a `/` menu.
    pub disable_slash_command: bool,
}

/// Why a root was not read: it does not exist, or it exists but cannot be listed as a folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    Missing,
    Unreadable,
}

/// One scanned root, as the report shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillSource {
    pub id: String,
    pub label: String,
    pub path: PathBuf,
    pub source_kind: SkillSourceKind,
    pub providers: Vec<SkillProvider>,
    /// The agent that owns the root; `None` is the shared `.agents` scope.
    pub owner: Option<String>,
    pub exists: bool,
    pub skipped_reason: Option<SkipReason>,
}

/// The result of one scan. Nothing here is persisted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDiscovery {
    pub skills: Vec<DiscoveredSkill>,
    pub sources: Vec<SkillSource>,
    pub scanned_at_ms: i64,
}

/// Scans the roots `request` names. Reads only: no file or folder is written, moved or deleted.
pub fn discover(request: &DiscoveryRequest) -> SkillDiscovery {
    let wanted = &request.source_kinds;
    let mut sources = Vec::new();
    let mut skills: Vec<DiscoveredSkill> = Vec::new();
    let mut position: HashMap<PathBuf, usize> = HashMap::new();
    for root in discovery_roots(request)
        .iter()
        .filter(|root| root_may_hold(root, wanted))
    {
        let (exists, skipped_reason, found) = scan_root(root);
        sources.push(SkillSource {
            id: root.id.clone(),
            label: root.label.clone(),
            path: root.path.clone(),
            source_kind: root.kind,
            providers: root.providers.clone(),
            owner: root.owner.map(str::to_owned),
            exists,
            skipped_reason,
        });
        for (real, skill) in found {
            if !wanted.is_empty() && !wanted.contains(&skill.source_kind) {
                continue;
            }
            match position.get(&real) {
                Some(&index) => merge(&mut skills[index], skill),
                None => {
                    position.insert(real, skills.len());
                    skills.push(skill);
                }
            }
        }
    }
    skills.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.source_label.cmp(&b.source_label))
            .then_with(|| a.skill_file_path.cmp(&b.skill_file_path))
    });
    sources.sort_by_key(|source| source.label.to_lowercase());
    SkillDiscovery {
        skills,
        sources,
        scanned_at_ms: to_ms(SystemTime::now()),
    }
}

/// Whether a root can hold a wanted kind. A home root holds both `home` and `bundled` skills.
fn root_may_hold(root: &SkillRoot, wanted: &[SkillSourceKind]) -> bool {
    if wanted.is_empty() {
        return true;
    }
    match root.kind {
        SkillSourceKind::Home => wanted
            .iter()
            .any(|kind| matches!(kind, SkillSourceKind::Home | SkillSourceKind::Bundled)),
        kind => wanted.contains(&kind),
    }
}

/// How many folders a `SKILL.md` may sit below its root: five for home roots, ten for plugin caches.
fn max_file_depth(kind: SkillSourceKind) -> usize {
    match kind {
        SkillSourceKind::Plugin => 10,
        _ => 5,
    }
}

/// Whether the root exists, why it was skipped, and each skill it holds keyed by the real path of its `SKILL.md`.
fn scan_root(root: &SkillRoot) -> (bool, Option<SkipReason>, Vec<(PathBuf, DiscoveredSkill)>) {
    match fs::metadata(&root.path) {
        Err(_) => (false, Some(SkipReason::Missing), Vec::new()),
        Ok(meta) if !meta.is_dir() => (true, Some(SkipReason::Unreadable), Vec::new()),
        Ok(_) => match find_skill_files(&root.path, max_file_depth(root.kind)) {
            Err(_) => (true, Some(SkipReason::Unreadable), Vec::new()),
            Ok(files) => (
                true,
                None,
                files
                    .into_iter()
                    .filter_map(|file| scan_file(root, file))
                    .collect(),
            ),
        },
    }
}

/// One skill file as a skill of `root`. `None` when the file cannot be read, so it is not a skill.
fn scan_file(root: &SkillRoot, skill_file: PathBuf) -> Option<(PathBuf, DiscoveredSkill)> {
    let summary = read_skill_summary(&skill_file)?;
    let directory = skill_file.parent()?.to_path_buf();
    let real = real_path(&skill_file);
    let bundled = root.kind == SkillSourceKind::Home && in_system_folder(root, &skill_file);
    let (kind, source_label) = if bundled {
        (SkillSourceKind::Bundled, format!("{} bundled", root.label))
    } else {
        (root.kind, root.label.clone())
    };
    let name = summary.name.unwrap_or_else(|| {
        directory
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    let updated_at_ms = fs::metadata(&skill_file)
        .and_then(|meta| meta.modified())
        .ok()
        .map(to_ms);
    let skill = DiscoveredSkill {
        id: sha256_hex(real.to_string_lossy().as_bytes())[..16].to_owned(),
        name,
        description: summary.description,
        providers: root.providers.clone(),
        source_kind: kind,
        source_label,
        root_path: root.path.clone(),
        root_paths: vec![root.path.clone()],
        directory_path: directory,
        skill_file_path: skill_file,
        installed: true,
        updated_at_ms,
        disable_slash_command: summary.disable_slash_command,
    };
    Some((real, skill))
}

/// Whether a skill file sits under the `.system` folder an agent tool ships in its home root.
fn in_system_folder(root: &SkillRoot, skill_file: &Path) -> bool {
    skill_file
        .strip_prefix(&root.path)
        .ok()
        .and_then(|relative| relative.components().next())
        == Some(Component::Normal(".system".as_ref()))
}

/// The canonical path without the `\\?\` prefix Windows adds, so it reads like the other paths in the report.
fn real_path(path: &Path) -> PathBuf {
    let real = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let plain = real
        .to_str()
        .and_then(|text| text.strip_prefix(r"\\?\"))
        .filter(|rest| !rest.starts_with(r"UNC\"))
        .map(PathBuf::from);
    plain.unwrap_or(real)
}

/// Folds a second copy of a skill into the first: every root that reached it is kept, and so is every provider.
fn merge(first: &mut DiscoveredSkill, copy: DiscoveredSkill) {
    if !first.root_paths.contains(&copy.root_path) {
        first.root_paths.push(copy.root_path);
    }
    for provider in copy.providers {
        if !first.providers.contains(&provider) {
            first.providers.push(provider);
        }
    }
    first.installed |= copy.installed;
}

fn to_ms(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(elapsed) => elapsed.as_millis() as i64,
        Err(before) => -(before.duration().as_millis() as i64),
    }
}
