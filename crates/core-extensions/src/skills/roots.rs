//! Where agent tools keep skills in the user's home folder: pv96's root table (`skill-discovery-sources.ts`).

use std::path::{Path, PathBuf};

use super::discover::DiscoveryRequest;
use SkillProvider::{AgentSkills, Claude, Codex};
use SkillSourceKind::{Home, Plugin};

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

/// Every home root for `request`, with the environment overrides applied.
pub(super) fn discovery_roots(request: &DiscoveryRequest) -> Vec<SkillRoot> {
    HOME_ROOTS
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
        .collect()
}

fn join(base: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(base.to_path_buf(), |path, part| path.join(part))
}
