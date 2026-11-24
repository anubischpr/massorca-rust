//! Where skills live: pv96's root table (`skill-discovery-sources.ts`) for the agent tools' home folders, the
//! registered folders' own skill roots, and the Claude plugins that apply to them.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::claude_plugins::claude_plugin_roots;
use super::discover::DiscoveryRequest;
use crate::source::sha256_hex;
use SkillProvider::{AgentSkills, Claude, Codex};
use SkillSourceKind::{Home, Plugin, Repo};

/// The kind of a skill's root. `Bundled` is a home skill under the `.system` folder an agent tool ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillSourceKind {
    Home,
    Repo,
    Bundled,
    Plugin,
}

/// The agent tools a skill is offered to. `AgentSkills` is the shared convention several tools read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillProvider {
    Codex,
    Claude,
    AgentSkills,
}

/// Folders that some agent tools read from instead of their default home folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillRootEnv {
    /// `CLAUDE_CONFIG_DIR`: Claude reads `skills` below it.
    pub claude_config_dir: Option<PathBuf>,
    /// `GROK_HOME`: Grok reads `skills` below it.
    pub grok_home: Option<PathBuf>,
    /// `HERMES_HOME`: moves the whole Hermes profile, so its `skills` folder moves with it.
    pub hermes_home: Option<PathBuf>,
}

/// One root a scan reads. `owner` names the agent that owns it; `None` is the shared scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SkillRoot {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) path: PathBuf,
    pub(super) kind: SkillSourceKind,
    pub(super) providers: Vec<SkillProvider>,
    pub(super) owner: Option<&'static str>,
}

/// A root in the table: its path is `relative` below the home folder, with `/` between the parts.
struct RootSpec {
    id: &'static str,
    label: &'static str,
    relative: &'static str,
    kind: SkillSourceKind,
    providers: &'static [SkillProvider],
    owner: Option<&'static str>,
}

const fn spec(
    id: &'static str,
    label: &'static str,
    relative: &'static str,
    kind: SkillSourceKind,
    providers: &'static [SkillProvider],
    owner: Option<&'static str>,
) -> RootSpec {
    RootSpec {
        id,
        label,
        relative,
        kind,
        providers,
        owner,
    }
}

// One entry per line: the table reads as data, so rustfmt keeps it that way.
#[rustfmt::skip]
const HOME_ROOTS: &[RootSpec] = &[
    spec("home-codex", "Codex home", ".codex/skills", Home, &[Codex], Some("codex")),
    spec("home-agents", "Agent skills home", ".agents/skills", Home, &[AgentSkills], None),
    spec("home-claude", "Claude home", ".claude/skills", Home, &[Claude], Some("claude")),
    spec("codex-plugin-cache", "Codex plugin cache", ".codex/plugins/cache", Plugin, &[Codex, AgentSkills], Some("codex")),
    spec("home-grok", "Grok home", ".grok/skills", Home, &[AgentSkills], Some("grok")),
    spec("home-opencode", "OpenCode home", ".config/opencode/skills", Home, &[AgentSkills], Some("opencode")),
    spec("home-pi", "Pi home", ".pi/agent/skills", Home, &[AgentSkills], Some("pi")),
    spec("home-omp", "OMP home", ".omp/agent/skills", Home, &[AgentSkills], Some("omp")),
    spec("home-hermes", "Hermes home", ".hermes/skills", Home, &[AgentSkills], Some("hermes")),
    spec("home-prime-agent", "Prime Agent home", ".prime/agent/skills", Home, &[AgentSkills], Some("prime-agent")),
    spec("home-gemini", "Gemini home", ".gemini/skills", Home, &[AgentSkills], Some("gemini")),
    spec("home-antigravity", "Antigravity home", ".gemini/config/skills", Home, &[AgentSkills], Some("antigravity")),
    spec("home-antigravity-cli", "Antigravity CLI skills", ".gemini/antigravity-cli/skills", Home, &[AgentSkills], Some("antigravity")),
    spec("home-antigravity-builtin", "Antigravity built-in skills", ".gemini/antigravity-cli/builtin/skills", Plugin, &[AgentSkills], Some("antigravity")),
    spec("home-cursor", "Cursor home", ".cursor/skills", Home, &[AgentSkills], Some("cursor")),
    spec("home-droid", "Droid home", ".factory/skills", Home, &[AgentSkills], Some("droid")),
    spec("home-continue", "Continue home", ".continue/skills", Home, &[AgentSkills], Some("continue")),
    spec("home-trae", "Trae home", ".trae-cn/skills", Home, &[AgentSkills], Some("trae")),
    spec("home-aug", "Augment home", ".augment/skills", Home, &[AgentSkills], Some("aug")),
    spec("home-muse", "Muse home", ".config/muse/skills", Home, &[AgentSkills], Some("muse")),
    spec("home-zcode", "ZCode home", ".zcode/skills", Home, &[AgentSkills], Some("zcode")),
];

/// A skill root inside a registered folder, named after the dotted tool folder it sits in.
struct FolderSpec {
    relative: &'static str,
    providers: &'static [SkillProvider],
    owner: Option<&'static str>,
}

const fn folder_spec(
    relative: &'static str,
    providers: &'static [SkillProvider],
    owner: Option<&'static str>,
) -> FolderSpec {
    FolderSpec {
        relative,
        providers,
        owner,
    }
}

#[rustfmt::skip]
const FOLDER_ROOTS: &[FolderSpec] = &[
    folder_spec(".agents/skills", &[AgentSkills], None),
    folder_spec(".claude/skills", &[Claude], Some("claude")),
    folder_spec(".factory/skills", &[AgentSkills], Some("droid")),
    folder_spec(".continue/skills", &[AgentSkills], Some("continue")),
    folder_spec(".trae/skills", &[AgentSkills], Some("trae")),
    folder_spec(".grok/skills", &[AgentSkills], Some("grok")),
    folder_spec(".augment/skills", &[AgentSkills], Some("aug")),
];

/// Every root a discovery reads for `request`: the home roots, each registered folder's roots, then the Claude
/// plugins that apply to the home folder and to each folder.
pub(super) fn discovery_roots(request: &DiscoveryRequest) -> Vec<SkillRoot> {
    let mut roots: Vec<SkillRoot> = HOME_ROOTS
        .iter()
        .map(|spec| {
            let override_dir = match spec.id {
                "home-claude" => request.env.claude_config_dir.as_deref(),
                "home-grok" => request.env.grok_home.as_deref(),
                "home-hermes" => request.env.hermes_home.as_deref(),
                _ => None,
            };
            let path = match override_dir {
                Some(dir) => dir.join("skills"),
                None => join(&request.home, spec.relative),
            };
            SkillRoot {
                id: spec.id.to_owned(),
                label: spec.label.to_owned(),
                path,
                kind: spec.kind,
                providers: spec.providers.to_vec(),
                owner: spec.owner,
            }
        })
        .collect();
    // Sorted and deduplicated, so a folder given twice is scanned once.
    let folders: BTreeSet<&Path> = request.folders.iter().map(PathBuf::as_path).collect();
    for folder in &folders {
        roots.extend(folder_roots(folder));
    }
    let contexts: Vec<PathBuf> = folders
        .iter()
        .map(|folder| folder.to_path_buf())
        .chain(std::iter::once(request.home.clone()))
        .collect();
    roots.extend(claude_plugin_roots(&request.home, &contexts));
    roots
}

fn folder_roots(folder: &Path) -> Vec<SkillRoot> {
    let name = folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let key = sha256_hex(folder.to_string_lossy().as_bytes());
    FOLDER_ROOTS
        .iter()
        .map(|spec| {
            let dotted = spec.relative.split('/').next().unwrap_or_default();
            SkillRoot {
                id: format!("repo-{}-{}", dotted.trim_start_matches('.'), &key[..16]),
                label: format!("Repo {name} {dotted}"),
                path: join(folder, spec.relative),
                kind: Repo,
                providers: spec.providers.to_vec(),
                owner: spec.owner,
            }
        })
        .collect()
}

fn join(base: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(base.to_path_buf(), |path, part| path.join(part))
}
