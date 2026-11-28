//! Typed mentions of the Conference room (AUTO-13): `@Name` names a bot of the team, `@team:Name` another team,
//! `@project:Name` a project of the team and `@file:path` a file inside one of its projects' folders. A typed mention
//! that names nothing is refused with `team.room_mention_unknown`, and a file mention that leaves its project's folder
//! (`..`, an absolute path or a link) with `team.room_file_outside`. A plain `@word` that names no bot stays text.

use std::path::{Component, Path, PathBuf};

use massorca_core_store::{BotRow, Store, TeamProjectRow, TeamRow};
use massorca_core_workspace::Workspaces;
use massorca_protocol::methods::team_room::{
    RoomMention, RoomMentionKind, RoomMentions, TeamRoomMentionsParams, TeamRoomResolveParams,
};
use massorca_protocol::{ErrorBody, codes};

use crate::context::RequestCtx;
use crate::features::workspace_domain::shared;

use super::{blocking, selectors, store_error};

const TEAM_PREFIX: &str = "team:";
const PROJECT_PREFIX: &str = "project:";
const FILE_PREFIX: &str = "file:";
/// Suggestions `team.room_mentions` returns without a limit, and at most.
pub(super) const DEFAULT_SUGGESTIONS: usize = 20;
pub(super) const MAX_SUGGESTIONS: usize = 100;

/// `team.room_mentions`.
pub(super) async fn mentions(
    ctx: RequestCtx,
    params: TeamRoomMentionsParams,
) -> Result<RoomMentions, ErrorBody> {
    let workspaces = shared(&ctx);
    blocking(&ctx, move |store, _agents| {
        let team = selectors::team(store, &params.team)?;
        let bots = store.list_bots(&team.id).map_err(store_error)?;
        let projects = Projects::of(store, &workspaces, &team)?;
        let limit = params.limit.map_or(DEFAULT_SUGGESTIONS, |limit| {
            usize::try_from(limit).map_or(MAX_SUGGESTIONS, |limit| limit.min(MAX_SUGGESTIONS))
        });
        Ok(RoomMentions {
            mentions: suggest(store, &team, &bots, &projects, &params.query, limit)?,
        })
    })
    .await
}

/// `team.room_resolve`.
pub(super) async fn resolve_line(
    ctx: RequestCtx,
    params: TeamRoomResolveParams,
) -> Result<RoomMentions, ErrorBody> {
    let workspaces = shared(&ctx);
    blocking(&ctx, move |store, _agents| {
        let team = selectors::team(store, &params.team)?;
        let bots = store.list_bots(&team.id).map_err(store_error)?;
        let projects = Projects::of(store, &workspaces, &team)?;
        Ok(RoomMentions {
            mentions: resolve(store, &team, &bots, &projects, &params.text)?.list,
        })
    })
    .await
}

/// What a line's mentions name: each in the order written, and the first team and project, which steer the line.
#[derive(Default)]
pub(super) struct Mentions {
    pub(super) list: Vec<RoomMention>,
    pub(super) team: Option<TeamRow>,
    pub(super) project: Option<TeamProjectRow>,
}

impl Mentions {
    /// What a task or a reply keeps of the mentions: the project and each file, or `None` without either.
    pub(super) fn notes(&self) -> Option<String> {
        let lines: Vec<String> = self
            .list
            .iter()
            .filter_map(|mention| match mention.kind {
                RoomMentionKind::Project => Some(format!("- project {}", mention.label)),
                RoomMentionKind::File => Some(format!(
                    "- file {} (in {})",
                    mention.label,
                    mention.project.as_deref().unwrap_or_default()
                )),
                _ => None,
            })
            .collect();
        (!lines.is_empty()).then(|| format!("Mentioned in the room:\n{}", lines.join("\n")))
    }
}

/// The team's projects with their code folders, in team order; a project with no folder has none.
pub(super) struct Projects {
    rows: Vec<(TeamProjectRow, Option<PathBuf>)>,
}

impl Projects {
    pub(super) fn of(
        store: &Store,
        workspaces: &Workspaces,
        team: &TeamRow,
    ) -> Result<Self, ErrorBody> {
        let rows = store
            .list_team_projects(&team.id)
            .map_err(store_error)?
            .into_iter()
            .map(|row| {
                let folder = row
                    .source_project_id
                    .as_deref()
                    .and_then(|id| workspaces.show_project(id).ok())
                    .map(|project| PathBuf::from(project.path));
                (row, folder)
            })
            .collect();
        Ok(Self { rows })
    }

    /// The folders a file mention is looked for in: the mentioned project's, else every project's in team order.
    fn roots<'a>(
        &'a self,
        mentioned: Option<&'a TeamProjectRow>,
    ) -> impl Iterator<Item = (&'a TeamProjectRow, &'a Path)> {
        self.rows.iter().filter_map(move |(row, folder)| {
            let folder = folder.as_deref()?;
            mentioned
                .is_none_or(|project| project.id == row.id)
                .then_some((row, folder))
        })
    }
}

/// One mention found in a line: its kind and the text after its prefix, up to the next space.
struct Found<'a> {
    at: usize,
    kind: RoomMentionKind,
    rest: &'a str,
}

/// Each `@` at the start of a word, with the kind its prefix names (a plain `@` names a bot).
fn scan(text: &str) -> Vec<Found<'_>> {
    let mut found = Vec::new();
    let mut previous: Option<char> = None;
    for (at, ch) in text.char_indices() {
        if ch == '@' && previous.is_none_or(|before| !before.is_alphanumeric()) {
            let after = &text[at + 1..];
            let (kind, rest) = [
                (TEAM_PREFIX, RoomMentionKind::Team),
                (PROJECT_PREFIX, RoomMentionKind::Project),
                (FILE_PREFIX, RoomMentionKind::File),
            ]
            .into_iter()
            .find_map(|(prefix, kind)| {
                strip_prefix_ignoring_case(after, prefix).map(|rest| (kind, rest))
            })
            .unwrap_or((RoomMentionKind::Bot, after));
            found.push(Found { at, kind, rest });
        }
        previous = Some(ch);
    }
    found
}

fn strip_prefix_ignoring_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

/// The longest of `names` that `rest` starts with, in any letter case, followed by no letter or digit.
fn longest_name<'a, T>(rest: &str, names: impl Iterator<Item = (&'a str, T)>) -> Option<T> {
    let lower = rest.to_lowercase();
    names
        .filter(|(name, _)| {
            let name = name.to_lowercase();
            lower.starts_with(&name)
                && lower[name.len()..]
                    .chars()
                    .next()
                    .is_none_or(|next| !next.is_alphanumeric())
        })
        .max_by_key(|(name, _)| name.chars().count())
        .map(|(_, value)| value)
}

/// The written mention up to the next space, as a refusal names it.
fn written(text: &str, at: usize) -> String {
    text[at..]
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_end_matches([',', ';', ':', ')', '!', '?', '.'])
        .to_owned()
}

fn unknown(mention: &str, kind: &str, why: &str) -> ErrorBody {
    let mut error = ErrorBody::new(
        codes::TEAM_ROOM_MENTION_UNKNOWN,
        format!("{mention} names no {kind} {why}"),
    );
    error.data = Some(serde_json::json!({ "mention": mention, "kind": kind }));
    error
}

fn outside(mention: &str) -> ErrorBody {
    let mut error = ErrorBody::new(
        codes::TEAM_ROOM_FILE_OUTSIDE,
        format!("{mention} leaves its project's folder; name a file inside it"),
    );
    error.data = Some(serde_json::json!({ "mention": mention, "kind": "file" }));
    error
}

/// The mentions of `text` in `team`'s room. Bots are the team's; teams are the other teams.
pub(super) fn resolve(
    store: &Store,
    team: &TeamRow,
    bots: &[BotRow],
    projects: &Projects,
    text: &str,
) -> Result<Mentions, ErrorBody> {
    let teams = store.list_teams().map_err(store_error)?;
    let found = scan(text);
    let mut mentions = Mentions::default();
    // Projects first, so a file mention looks in the project the line names wherever it is written.
    for item in found
        .iter()
        .filter(|item| item.kind == RoomMentionKind::Project)
    {
        let row = longest_name(
            item.rest,
            projects
                .rows
                .iter()
                .map(|(row, _)| (row.name.as_str(), row)),
        )
        .ok_or_else(|| unknown(&written(text, item.at), "project", "of this team"))?;
        mentions.project.get_or_insert_with(|| row.clone());
    }
    for item in &found {
        let mention = match item.kind {
            RoomMentionKind::Bot => {
                let Some(bot) =
                    longest_name(item.rest, bots.iter().map(|bot| (bot.name.as_str(), bot)))
                else {
                    continue;
                };
                bot_mention(bot)
            }
            RoomMentionKind::Team => {
                let other = longest_name(
                    item.rest,
                    teams
                        .iter()
                        .filter(|other| other.id != team.id)
                        .map(|other| (other.name.as_str(), other)),
                )
                .ok_or_else(|| unknown(&written(text, item.at), "team", "this room can send to"))?;
                mentions.team.get_or_insert_with(|| other.clone());
                team_mention(other)
            }
            RoomMentionKind::Project => {
                let row = longest_name(
                    item.rest,
                    projects
                        .rows
                        .iter()
                        .map(|(row, _)| (row.name.as_str(), row)),
                )
                .ok_or_else(|| unknown(&written(text, item.at), "project", "of this team"))?;
                project_mention(row)
            }
            RoomMentionKind::File | RoomMentionKind::Unknown => {
                file_mention(projects, mentions.project.as_ref(), &written(text, item.at))?
            }
        };
        mentions.list.push(mention);
    }
    Ok(mentions)
}

fn bot_mention(bot: &BotRow) -> RoomMention {
    RoomMention {
        kind: RoomMentionKind::Bot,
        id: bot.id.clone(),
        label: bot.name.clone(),
        insert: format!("@{}", bot.name),
        project: None,
    }
}

fn team_mention(team: &TeamRow) -> RoomMention {
    RoomMention {
        kind: RoomMentionKind::Team,
        id: team.id.clone(),
        label: team.name.clone(),
        insert: format!("@{TEAM_PREFIX}{}", team.name),
        project: None,
    }
}

fn project_mention(row: &TeamProjectRow) -> RoomMention {
    RoomMention {
        kind: RoomMentionKind::Project,
        id: row.id.clone(),
        label: row.name.clone(),
        insert: format!("@{PROJECT_PREFIX}{}", row.name),
        project: None,
    }
}

/// The file `written` (`@file:path`) names: the first project folder, among those it is looked for in, that holds it.
fn file_mention(
    projects: &Projects,
    mentioned: Option<&TeamProjectRow>,
    written: &str,
) -> Result<RoomMention, ErrorBody> {
    let path = written.get(1 + FILE_PREFIX.len()..).unwrap_or_default();
    let relative = inside(path).ok_or_else(|| outside(written))?;
    if relative.as_os_str().is_empty() {
        return Err(unknown(written, "file", "in this team's projects"));
    }
    for (row, root) in projects.roots(mentioned) {
        let candidate = root.join(&relative);
        if std::fs::symlink_metadata(&candidate).is_err() {
            continue;
        }
        if !stays_inside(root, &candidate) {
            return Err(outside(written));
        }
        let shown = slashed(&relative);
        return Ok(RoomMention {
            kind: RoomMentionKind::File,
            id: shown.clone(),
            label: shown.clone(),
            insert: format!("@{FILE_PREFIX}{shown}"),
            project: Some(row.name.clone()),
        });
    }
    Err(unknown(written, "file", "in this team's projects"))
}

/// `path` as a relative path of normal parts; `None` when it is absolute or climbs with `..`.
fn inside(path: &str) -> Option<PathBuf> {
    let mut relative = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    // A path some host reads as absolute (`\\x`, `C:x`) never passes on another host either.
    (!path.starts_with(['/', '\\']) && !path.contains(':')).then_some(relative)
}

/// Whether `candidate`, with every link followed, is still inside `root`.
fn stays_inside(root: &Path, candidate: &Path) -> bool {
    match (root.canonicalize(), candidate.canonicalize()) {
        (Ok(root), Ok(candidate)) => candidate.starts_with(root),
        _ => false,
    }
}

fn slashed(path: &Path) -> String {
    path.components()
        .filter_map(|part| match part {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// What `query` (the text after an `@`) can name, at most `limit`: the team's bots, the other teams, the team's
/// projects and the file prefix, or with `file:` the files and folders of one folder of the team's projects.
pub(super) fn suggest(
    store: &Store,
    team: &TeamRow,
    bots: &[BotRow],
    projects: &Projects,
    query: &str,
    limit: usize,
) -> Result<Vec<RoomMention>, ErrorBody> {
    if let Some(path) = strip_prefix_ignoring_case(query, FILE_PREFIX) {
        return files(projects, path, limit);
    }
    let (only, wanted) = [
        (TEAM_PREFIX, RoomMentionKind::Team),
        (PROJECT_PREFIX, RoomMentionKind::Project),
    ]
    .into_iter()
    .find_map(|(prefix, kind)| {
        strip_prefix_ignoring_case(query, prefix).map(|rest| (Some(kind), rest))
    })
    .unwrap_or((None, query));
    let wanted = wanted.to_lowercase();
    let matches = |name: &str| name.to_lowercase().contains(&wanted);
    let shown = |kind: RoomMentionKind| only.is_none_or(|only| only == kind);
    let mut list = Vec::new();
    if shown(RoomMentionKind::Bot) {
        list.extend(
            bots.iter()
                .filter(|bot| matches(&bot.name))
                .map(bot_mention),
        );
    }
    if shown(RoomMentionKind::Team) {
        let teams = store.list_teams().map_err(store_error)?;
        list.extend(
            teams
                .iter()
                .filter(|other| other.id != team.id && matches(&other.name))
                .map(team_mention),
        );
    }
    if shown(RoomMentionKind::Project) {
        list.extend(
            projects
                .rows
                .iter()
                .filter(|(row, _)| matches(&row.name))
                .map(|(row, _)| project_mention(row)),
        );
    }
    if only.is_none() && FILE_PREFIX.starts_with(wanted.as_str()) {
        list.push(RoomMention {
            kind: RoomMentionKind::File,
            id: String::new(),
            label: FILE_PREFIX.to_owned(),
            insert: format!("@{FILE_PREFIX}"),
            project: None,
        });
    }
    list.truncate(limit);
    Ok(list)
}

/// The entries of the folder `path` names (up to its last `/`) whose name starts with the rest, in each project with a
/// folder: folders end with `/`. Hidden entries are left out.
fn files(projects: &Projects, path: &str, limit: usize) -> Result<Vec<RoomMention>, ErrorBody> {
    let written = format!("@{FILE_PREFIX}{path}");
    let (folder, start) = path.rsplit_once('/').unwrap_or(("", path));
    let relative = inside(folder).ok_or_else(|| outside(&written))?;
    let start = start.to_lowercase();
    let mut list = Vec::new();
    for (row, root) in projects.roots(None) {
        let dir = root.join(&relative);
        if !stays_inside(root, &dir) {
            if dir.exists() {
                return Err(outside(&written));
            }
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut named: Vec<(String, bool)> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let is_dir = entry.file_type().ok()?.is_dir();
                (!name.starts_with('.') && name.to_lowercase().starts_with(&start))
                    .then_some((name, is_dir))
            })
            .collect();
        named.sort();
        for (name, is_dir) in named {
            let mut shown = slashed(&relative.join(&name));
            if is_dir {
                shown.push('/');
            }
            list.push(RoomMention {
                kind: RoomMentionKind::File,
                id: shown.clone(),
                label: shown.clone(),
                insert: format!("@{FILE_PREFIX}{shown}"),
                project: Some(row.name.clone()),
            });
        }
    }
    list.truncate(limit);
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_that_climbs_or_is_absolute_is_not_inside() {
        assert_eq!(
            inside("src/lib.rs"),
            Some(PathBuf::from("src").join("lib.rs"))
        );
        assert_eq!(inside("./src"), Some(PathBuf::from("src")));
        for path in [
            "../x",
            "src/../../x",
            "/etc/hosts",
            "\\\\host\\share",
            "C:x",
            "C:/x",
        ] {
            assert_eq!(inside(path), None, "{path}");
        }
    }

    #[test]
    fn a_mention_starts_a_word_and_its_prefix_reads_in_any_case() {
        let kinds: Vec<RoomMentionKind> = scan("@a mail me@b (@TEAM:x) @file:y @Project:z")
            .into_iter()
            .map(|found| found.kind)
            .collect();
        assert_eq!(
            kinds,
            [
                RoomMentionKind::Bot,
                RoomMentionKind::Team,
                RoomMentionKind::File,
                RoomMentionKind::Project
            ]
        );
        assert_eq!(written("see @file:src/lib.rs, then", 4), "@file:src/lib.rs");
    }
}
