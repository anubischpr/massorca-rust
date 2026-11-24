//! Where the Bot Teams data comes from. The sample source holds the team the screen specs show
//! (team Desarrollo, bots Mara, Bruno and Iris); `--sample` selects it the way it selects the
//! sample workspaces. It stands in for the service until Bot Teams exists there.

use super::state::TeamsState;
use super::types::{
    AddressId, Bot, BotId, Cadence, Column, FileId, HostChoice, MemoryEntry, MemoryId, MemoryTab,
    Project, ProjectId, Routine, RoutineId, RunOutcome, Task, TaskId, Team, TeamFile, TeamId,
    WebAddress, Weekday,
};
use crate::model::shell::{Agent, HostReach};
use massorca_protocol::methods::teams::{TeamAccess, TeamCharacter};

/// The host the sample team runs on, named as the shell names it.
const VPS: &str = "VPS";

/// VPS is unreachable for 5 minutes, as the shell snapshot shows it (data.rs).
const VPS_REACH: HostReach = HostReach::Unreachable { minutes: 5 };

/// Fills the Bot Teams state. Each source is chosen where the shell starts.
pub trait TeamsSource {
    fn teams(&self) -> TeamsState;
}

/// The sample team Desarrollo and its bots, as the screen specs show them (screens 4).
#[derive(Clone, Copy, Debug, Default)]
pub struct SampleTeamsSource;

/// No Bot Teams service is connected: the pages say so and nothing can be created.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnavailableTeams;

impl TeamsSource for SampleTeamsSource {
    fn teams(&self) -> TeamsState {
        TeamsState::connected(sample_hosts(), vec![desarrollo()])
    }
}

impl TeamsSource for UnavailableTeams {
    fn teams(&self) -> TeamsState {
        TeamsState::unavailable()
    }
}

fn sample_hosts() -> Vec<HostChoice> {
    vec![
        HostChoice {
            name: "This PC".into(),
            reach: HostReach::Connected,
        },
        HostChoice {
            name: VPS.into(),
            reach: VPS_REACH,
        },
    ]
}

const MARA: BotId = BotId(1);
const BRUNO: BotId = BotId(2);
const IRIS: BotId = BotId(3);

fn bot(id: BotId, name: &str, role: &str, agent: Agent) -> Bot {
    Bot {
        id,
        name: name.into(),
        role: role.into(),
        agent: Some(agent),
        kind_unavailable: None,
    }
}

fn task(id: u32, title: &str, column: Column, bot: Option<BotId>, time: Option<&str>) -> Task {
    Task {
        id: TaskId(id),
        title: title.into(),
        column,
        bot,
        time: time.map(Into::into),
        reach: HostReach::Connected,
        note: None,
    }
}

fn routine(
    id: u32,
    name: &str,
    instruction: &str,
    cadence: Cadence,
    time: &str,
    bot: BotId,
) -> Routine {
    Routine {
        id: RoutineId(id),
        name: name.into(),
        instruction: instruction.into(),
        cadence,
        time: time.into(),
        bot: Some(bot),
        host: VPS.into(),
        enabled: true,
        last_run: None,
    }
}

/// Team Desarrollo: six cards on its Board, one project, three routines, one web address,
/// two memory entries and three files.
fn desarrollo() -> Team {
    let mut nightly = task(
        4,
        "Nightly summary to the team",
        Column::Running,
        Some(BRUNO),
        None,
    );
    nightly.reach = VPS_REACH;
    let mut weekly_check = routine(
        1,
        "Check staging sign-up errors",
        "Check the staging sign-up log for failed confirmation emails and invalid tokens. Post a short summary in the Conference room, with the count for each cause.",
        Cadence::Weekly(Weekday::Monday),
        "09:00",
        BRUNO,
    );
    weekly_check.last_run = Some(RunOutcome::Done);
    let mut pulls = routine(
        2,
        "Summarise open pull requests",
        "List the open pull requests and what each one waits for.",
        Cadence::Weekly(Weekday::Tuesday),
        "09:00",
        MARA,
    );
    pulls.last_run = Some(RunOutcome::Done);
    let mut summary = routine(
        3,
        "Post the weekly summary to the Board",
        "Post the week's Board summary in the Conference room.",
        Cadence::Weekly(Weekday::Friday),
        "17:00",
        MARA,
    );
    summary.enabled = false;
    let mut magic = task(
        2,
        "Magic link sign-in for oppm",
        Column::Running,
        Some(MARA),
        Some("09:42"),
    );
    magic.note = Some("Two bots delegated".into());
    let mut approve = task(
        5,
        "Approve the magic link email template",
        Column::Waiting,
        Some(IRIS),
        None,
    );
    approve.note = Some("Needs your reply in the Conference room".into());
    Team {
        id: TeamId(1),
        name: "Desarrollo".into(),
        character: TeamCharacter::Triangle,
        access: TeamAccess::Full,
        purpose: "Sign-in and copy for oppm.".into(),
        host: VPS.into(),
        reach: VPS_REACH,
        bots: vec![
            bot(
                MARA,
                "Mara",
                "Leads the team. Splits a composite requirement by competence.",
                Agent::Claude,
            ),
            bot(BRUNO, "Bruno", "API and server work.", Agent::Codex),
            bot(IRIS, "Iris", "Copy and product text.", Agent::Antigravity),
        ],
        lead: Some(MARA),
        workspaces: false,
        tasks: vec![
            task(1, "Write release notes for oppm", Column::Open, None, None),
            magic,
            task(
                3,
                "Login page copy and error states",
                Column::Running,
                Some(IRIS),
                Some("10:05"),
            ),
            nightly,
            approve,
            task(
                6,
                "Weekly digest posted to the Board",
                Column::Done,
                Some(MARA),
                Some("08:00"),
            ),
        ],
        projects: vec![Project {
            id: ProjectId(1),
            name: "Desarrollo onboarding".into(),
            brief: "How a new bot picks up a task from the Board, and where it reports back."
                .into(),
            link: None,
            link_saved: true,
            tasks: vec![task(
                7,
                "Draft the first-run checklist",
                Column::Open,
                None,
                None,
            )],
        }],
        routines: vec![weekly_check, pulls, summary],
        addresses: vec![WebAddress {
            id: AddressId(1),
            name: "Release hook".into(),
            host: VPS.into(),
            created: "11 Nov 2026".into(),
            token: "sample-token".into(),
        }],
        memory: vec![
            MemoryEntry {
                id: MemoryId(1),
                tab: MemoryTab::Team,
                text: "Oppm sign-in uses the Supabase auth endpoints.".into(),
                author: Some(MARA),
                date: "11 Nov".into(),
            },
            MemoryEntry {
                id: MemoryId(2),
                tab: MemoryTab::Bot,
                text: "Staging mail goes through the sandbox SMTP host.".into(),
                author: Some(BRUNO),
                date: "10 Nov".into(),
            },
        ],
        files: vec![
            TeamFile {
                id: FileId(1),
                path: "notes/release-notes.md".into(),
                linked_project: None,
            },
            TeamFile {
                id: FileId(2),
                path: "copy/login-errors.md".into(),
                linked_project: None,
            },
            TeamFile {
                id: FileId(3),
                path: "auth-endpoints.md".into(),
                linked_project: Some("oppm".into()),
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{DataSource, SampleDataSource};

    #[test]
    fn the_sample_team_has_the_bots_and_board_the_specs_name() {
        let state = SampleTeamsSource.teams();
        assert!(state.is_connected());
        let team = &state.teams()[0];
        assert_eq!(team.name, "Desarrollo");
        assert_eq!(team.character, TeamCharacter::Triangle);
        assert_eq!(team.lead_name(), Some("Mara"));
        let names: Vec<&str> = team.bots.iter().map(|bot| bot.name.as_str()).collect();
        assert_eq!(names, ["Mara", "Bruno", "Iris"]);
        let count = |column| {
            team.tasks
                .iter()
                .filter(|task| task.column == column)
                .count()
        };
        let per_column = Column::ALL.map(count);
        assert_eq!(per_column, [1, 3, 1, 1]);
    }

    #[test]
    fn the_unavailable_source_is_not_connected_and_holds_no_teams() {
        let state = UnavailableTeams.teams();
        assert!(!state.is_connected());
        assert!(state.teams().is_empty());
    }

    #[test]
    fn new_ids_start_above_every_id_in_the_sample() {
        let mut state = SampleTeamsSource.teams();
        let created = state
            .create_team(super::super::state::NewTeam {
                name: "Ops".into(),
                purpose: String::new(),
                host: 0,
                workspaces: false,
            })
            .unwrap();
        assert!(
            created.0 > 7,
            "ids in the sample reach 7 and new ones follow"
        );
    }

    #[test]
    fn the_sample_vps_is_unreachable_as_the_shell_snapshot_shows_it() {
        let snapshot = SampleDataSource.snapshot();
        let shell_vps = snapshot
            .projects
            .iter()
            .flat_map(|project| &project.hosts)
            .find(|host| host.name == VPS)
            .expect("the shell snapshot has a VPS host");
        let state = SampleTeamsSource.teams();
        let host = state.hosts().iter().find(|host| host.name == VPS).unwrap();
        let team = &state.teams()[0];
        assert_eq!(host.reach, shell_vps.reach);
        assert_eq!(team.host, VPS);
        assert_eq!(team.reach, shell_vps.reach);
    }
}
