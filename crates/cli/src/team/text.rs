//! The text lines of the `team` commands. JSON output is the protocol document itself.

use massorca_protocol::methods::teams::{
    Board, Bot, CodeSource, MemorySource, Routine, RoutineCadence, RoutineRun, RoutineWeekday,
    Team, TeamAccess, TeamCharacter, TeamMemoryLine, TeamProject,
};

/// `created team alpha (id)`, or the verb the command used.
pub fn team_line(team: &Team, verb: &str) -> String {
    format!("{verb} team {} ({})", team.name, team.id)
}

/// One line per team: name, shape, and id.
pub fn team_table(teams: &[Team]) -> Vec<String> {
    teams
        .iter()
        .map(|team| {
            format!(
                "{}\t{}\t{}",
                team.name,
                character_word(team.character),
                team.id
            )
        })
        .collect()
}

pub fn team_details(team: &Team) -> Vec<String> {
    vec![
        format!("name: {}", team.name),
        format!("purpose: {}", team.purpose),
        format!("character: {}", character_word(team.character)),
        format!("access: {}", access_word(team.access)),
        format!("workspaces: {}", team.workspaces),
        format!("id: {}", team.id),
    ]
}

fn access_word(access: TeamAccess) -> &'static str {
    match access {
        TeamAccess::Full => "full",
        TeamAccess::Default => "default",
        TeamAccess::Unknown => "unknown",
    }
}

/// The sentence a full-access team shows every time it is made or changed: its bots bypass permission prompts (0083).
pub fn access_notice(team: &Team) -> Option<String> {
    (team.access == TeamAccess::Full).then(|| {
        format!(
            "Full access: bots run without asking for permission. To have them ask, run: massorca team update {} --access default",
            team.name
        )
    })
}

pub fn bot_line(bot: &Bot, verb: &str) -> String {
    let lead = if bot.lead { " (lead)" } else { "" };
    format!(
        "{verb} bot {}{lead} in team {} ({})",
        bot.name, bot.team_id, bot.id
    )
}

/// One bot per line: name, agent kind, lead, job, id.
pub fn bot_table(bots: &[Bot]) -> Vec<String> {
    bots.iter()
        .map(|bot| {
            let lead = if bot.lead { "lead" } else { "-" };
            format!("{}	{}	{lead}	{}	{}", bot.name, bot.kind, bot.job, bot.id)
        })
        .collect()
}

/// One project per line: name, goal, id, its recorded code source or `-`, and the name of its linked Workspaces project
/// or `-`.
pub fn project_table(projects: &[TeamProject]) -> Vec<String> {
    projects
        .iter()
        .map(|project| {
            let source = project.source_project_name.as_deref().unwrap_or("-");
            let code = project.code_source.map_or("-", CodeSource::label);
            format!(
                "{}\t{}\t{}\t{code}\t{source}",
                project.name, project.goal, project.id
            )
        })
        .collect()
}

pub fn project_line(project: &TeamProject, verb: &str) -> String {
    format!(
        "{verb} project {} in team {} ({})",
        project.name, project.team_id, project.id
    )
}

/// The Board as text: each column with its count, then its cards, oldest first.
pub fn board_lines(board: &Board) -> Vec<String> {
    let mut lines = Vec::new();
    for column in &board.columns {
        lines.push(format!(
            "{} ({})",
            column.column.label(),
            column.cards.len()
        ));
        for card in &column.cards {
            let mut line = format!(
                "  {}\t{}\t{}\t{}",
                card.id,
                card.title,
                card.state.label(),
                card.bot.as_deref().unwrap_or("no bot")
            );
            if card.last_known {
                line.push_str(" (last known state)");
            }
            if let Some(error) = &card.error {
                line.push_str(&format!(" - {error}"));
            }
            lines.push(line);
        }
    }
    lines
}

/// One memory line per row: its scope (the bot, or `team`), its source, its text, and its id.
pub fn memory_table(lines: &[TeamMemoryLine]) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            format!(
                "{}\t{}\t{}\t{}",
                line.bot_id.as_deref().unwrap_or("team"),
                source_word(line.source),
                line.text,
                line.id
            )
        })
        .collect()
}

pub fn memory_line(line: &TeamMemoryLine, verb: &str) -> String {
    format!("{verb} memory line {} in team {}", line.id, line.team_id)
}

fn source_word(source: MemorySource) -> &'static str {
    match source {
        MemorySource::Task => "task",
        MemorySource::Person => "person",
        MemorySource::Unknown => "unknown",
    }
}

fn character_word(character: TeamCharacter) -> &'static str {
    match character {
        TeamCharacter::Triangle => "triangle",
        TeamCharacter::Square => "square",
        TeamCharacter::Circle => "circle",
        TeamCharacter::Diamond => "diamond",
        TeamCharacter::Hexagon => "hexagon",
        TeamCharacter::Star => "star",
        TeamCharacter::Unknown => "unknown",
    }
}

/// `added routine morning (id) in team alpha`, or the verb the command used.
pub fn routine_line(routine: &Routine, verb: &str) -> String {
    format!(
        "{verb} routine {} ({}) in team {}",
        routine.name, routine.id, routine.team_id
    )
}

/// One line per routine: name, schedule, state and id.
pub fn routine_table(routines: &[Routine]) -> Vec<String> {
    routines
        .iter()
        .map(|routine| {
            format!(
                "{}\t{}\t{}\t{}",
                routine.name,
                schedule_text(routine),
                state_text(routine),
                routine.id
            )
        })
        .collect()
}

/// One line per run, newest first: the occurrence it stands for, its task, its error with its code, and its id.
pub fn run_table(runs: &[RoutineRun]) -> Vec<String> {
    runs.iter()
        .map(|run| {
            format!(
                "{}\t{}\t{}\t{}",
                run.scheduled_for_ms,
                run.task_id.as_deref().unwrap_or("-"),
                run_error(run).unwrap_or_else(|| "-".to_owned()),
                run.id
            )
        })
        .collect()
}

/// `[bot.not_found] bot removed: ...`, or the message alone from a host that sends no code.
fn run_error(run: &RoutineRun) -> Option<String> {
    let message = run.error.as_deref()?;
    Some(match run.error_code.as_deref() {
        Some(code) => format!("[{code}] {message}"),
        None => message.to_owned(),
    })
}

pub fn run_line(run: &RoutineRun) -> String {
    let outcome = match (&run.task_id, run_error(run)) {
        (Some(task), None) => format!("task {task}"),
        (Some(task), Some(error)) => format!("task {task} did not start: {error}"),
        (None, Some(error)) => format!("not started: {error}"),
        (None, None) => "no task".to_owned(),
    };
    format!("run {} of routine {}: {outcome}", run.id, run.routine_id)
}

/// `daily 09:00 Europe/Madrid`, `weekly monday 09:00 host zone`, `monthly day 15 ...` or `once 2026-11-24 ...`.
fn schedule_text(routine: &Routine) -> String {
    let when = match routine.cadence {
        RoutineCadence::Daily => "daily".to_owned(),
        RoutineCadence::Weekly => format!("weekly {}", routine.weekday.map_or("?", weekday_word)),
        RoutineCadence::Monthly => format!(
            "monthly day {}",
            routine
                .month_day
                .map_or_else(|| "?".to_owned(), |day| day.to_string())
        ),
        RoutineCadence::Once => format!("once {}", routine.run_date.as_deref().unwrap_or("?")),
        RoutineCadence::Unknown => "unknown cadence".to_owned(),
    };
    let zone = match (&routine.zone, &routine.zone_used) {
        (Some(zone), _) => zone.clone(),
        (None, Some(used)) => format!("host zone ({used})"),
        (None, None) => "host zone".to_owned(),
    };
    format!("{when} {} {zone}", routine.time)
}

fn state_text(routine: &Routine) -> String {
    if routine.paused {
        return "paused".to_owned();
    }
    match routine.next_run_at_ms {
        Some(next) => format!("next run at {next} ms"),
        None => "finished".to_owned(),
    }
}

fn weekday_word(day: RoutineWeekday) -> &'static str {
    match day {
        RoutineWeekday::Monday => "monday",
        RoutineWeekday::Tuesday => "tuesday",
        RoutineWeekday::Wednesday => "wednesday",
        RoutineWeekday::Thursday => "thursday",
        RoutineWeekday::Friday => "friday",
        RoutineWeekday::Saturday => "saturday",
        RoutineWeekday::Sunday => "sunday",
        RoutineWeekday::Unknown => "unknown",
    }
}
