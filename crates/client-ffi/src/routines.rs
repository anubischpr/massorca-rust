//! The teams of a host, their bots, and the routines of each team with their runs (AUTO-28, screen D-AG-25 on the phone).
//! Every call is one typed call of `massorca-client`; the schedule, the next run and the runs stay in the service.

use massorca_protocol::methods::teams::{
    Bot, Routine, RoutineRef, RoutineRun, RoutineRunsParams, Team, TeamRef,
};

use crate::error::FfiError;
use crate::massorca::{Host, run_on};
use crate::wire::word;

/// A team of the host, as the app lists it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TeamSummary {
    pub id: String,
    pub name: String,
}

impl From<Team> for TeamSummary {
    fn from(team: Team) -> Self {
        Self {
            id: team.id,
            name: team.name,
        }
    }
}

/// A bot of a team: the one that does the work of its routines.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BotSummary {
    pub id: String,
    pub team_id: String,
    pub name: String,
    /// Whether this is the team's lead bot.
    pub lead: bool,
}

impl From<Bot> for BotSummary {
    fn from(bot: Bot) -> Self {
        Self {
            id: bot.id,
            team_id: bot.team_id,
            name: bot.name,
            lead: bot.lead,
        }
    }
}

/// A routine of a team: a schedule that starts its bot on a task each time it is due.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoutineSummary {
    pub id: String,
    pub team_id: String,
    /// The bot the runs' tasks are for; absent once that bot is removed.
    pub bot_id: Option<String>,
    pub name: String,
    /// `daily`, `weekly`, `monthly` or `once`; `unknown` for a cadence this build does not know.
    pub cadence: String,
    /// The time of day, `HH:MM`, 24-hour.
    pub time: String,
    /// The day of a weekly routine, `monday` to `sunday`; `unknown` for a day this build does not know.
    pub weekday: Option<String>,
    /// The day of the month, 1 to 28, of a monthly routine.
    pub month_day: Option<u8>,
    /// The date, `YYYY-MM-DD`, of a once routine.
    pub run_date: Option<String>,
    /// An IANA zone name; absent means the host's zone.
    pub zone: Option<String>,
    /// A paused routine has no next run until it is resumed.
    pub paused: bool,
    /// The next run in UTC epoch milliseconds; absent while paused or once a one-time routine has run.
    pub next_run_at_ms: Option<i64>,
    pub created_at_ms: i64,
}

impl From<Routine> for RoutineSummary {
    fn from(routine: Routine) -> Self {
        Self {
            id: routine.id,
            team_id: routine.team_id,
            bot_id: routine.bot_id,
            name: routine.name,
            cadence: word(routine.cadence),
            time: routine.time,
            weekday: routine.weekday.map(word),
            month_day: routine.month_day,
            run_date: routine.run_date,
            zone: routine.zone,
            paused: routine.paused,
            next_run_at_ms: routine.next_run_at_ms,
            created_at_ms: routine.created_at_ms,
        }
    }
}

/// One run of a routine: an occurrence it claimed, or a run made now.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoutineRunSummary {
    pub id: String,
    pub routine_id: String,
    /// The occurrence the run stands for, in UTC epoch milliseconds.
    pub scheduled_for_ms: i64,
    /// The task the run made, when it made one.
    pub task_id: Option<String>,
    /// Why the run or its task did not start, as a sentence.
    pub error: Option<String>,
    /// The protocol code of `error`, such as `bot.not_found`.
    pub error_code: Option<String>,
    pub created_at_ms: i64,
}

impl From<RoutineRun> for RoutineRunSummary {
    fn from(run: RoutineRun) -> Self {
        Self {
            id: run.id,
            routine_id: run.routine_id,
            scheduled_for_ms: run.scheduled_for_ms,
            task_id: run.task_id,
            error: run.error,
            error_code: run.error_code,
            created_at_ms: run.created_at_ms,
        }
    }
}

#[uniffi::export]
impl Host {
    /// The teams of the host, by name.
    pub async fn teams(&self) -> Result<Vec<TeamSummary>, FfiError> {
        let client = self.client.clone();
        let name = self.name.clone();
        run_on(&self.runtime, async move {
            let list = client
                .team_list()
                .await
                .map_err(|error| FfiError::from_client(error, &name))?;
            Ok(list.teams.into_iter().map(TeamSummary::from).collect())
        })
        .await
    }

    /// The bots of `team` (its id or exact name), in the order the host lists them.
    pub async fn team_bots(&self, team: String) -> Result<Vec<BotSummary>, FfiError> {
        let client = self.client.clone();
        let name = self.name.clone();
        run_on(&self.runtime, async move {
            let list = client
                .team_bot_list(&TeamRef { team })
                .await
                .map_err(|error| FfiError::from_client(error, &name))?;
            Ok(list.bots.into_iter().map(BotSummary::from).collect())
        })
        .await
    }

    /// The routines of `team` (its id or exact name), in creation order.
    pub async fn team_routines(&self, team: String) -> Result<Vec<RoutineSummary>, FfiError> {
        let client = self.client.clone();
        let name = self.name.clone();
        run_on(&self.runtime, async move {
            let list = client
                .team_routine_list(&TeamRef { team })
                .await
                .map_err(|error| FfiError::from_client(error, &name))?;
            Ok(list
                .routines
                .into_iter()
                .map(RoutineSummary::from)
                .collect())
        })
        .await
    }

    /// The newest `limit` runs of a routine, newest first; the host's default when `limit` is absent.
    pub async fn routine_runs(
        &self,
        team: String,
        routine: String,
        limit: Option<u32>,
    ) -> Result<Vec<RoutineRunSummary>, FfiError> {
        let client = self.client.clone();
        let name = self.name.clone();
        let params = RoutineRunsParams {
            team,
            routine,
            limit: limit.map(|count| usize::try_from(count).unwrap_or(usize::MAX)),
        };
        run_on(&self.runtime, async move {
            let list = client
                .team_routine_runs(&params)
                .await
                .map_err(|error| FfiError::from_client(error, &name))?;
            Ok(list.runs.into_iter().map(RoutineRunSummary::from).collect())
        })
        .await
    }

    /// Pauses a routine of `team`: it has no next run until it is resumed.
    pub async fn pause_routine(
        &self,
        team: String,
        routine: String,
    ) -> Result<RoutineSummary, FfiError> {
        let client = self.client.clone();
        let name = self.name.clone();
        run_on(&self.runtime, async move {
            client
                .team_routine_pause(&RoutineRef { team, routine })
                .await
                .map(RoutineSummary::from)
                .map_err(|error| FfiError::from_client(error, &name))
        })
        .await
    }

    /// Resumes a paused routine of `team` from now; runs missed while it was paused are not made up.
    pub async fn resume_routine(
        &self,
        team: String,
        routine: String,
    ) -> Result<RoutineSummary, FfiError> {
        let client = self.client.clone();
        let name = self.name.clone();
        run_on(&self.runtime, async move {
            client
                .team_routine_resume(&RoutineRef { team, routine })
                .await
                .map(RoutineSummary::from)
                .map_err(|error| FfiError::from_client(error, &name))
        })
        .await
    }

    /// Makes one run of a routine of `team` now. It does not move the routine's schedule.
    pub async fn run_routine(
        &self,
        team: String,
        routine: String,
    ) -> Result<RoutineRunSummary, FfiError> {
        let client = self.client.clone();
        let name = self.name.clone();
        run_on(&self.runtime, async move {
            client
                .team_routine_run(&RoutineRef { team, routine })
                .await
                .map(RoutineRunSummary::from)
                .map_err(|error| FfiError::from_client(error, &name))
        })
        .await
    }
}
