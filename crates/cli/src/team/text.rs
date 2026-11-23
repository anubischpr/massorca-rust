//! The text lines of the `team` commands. JSON output is the protocol document itself.

use massorca_protocol::methods::teams::{
    Board, Bot, MemorySource, Team, TeamCharacter, TeamMemoryLine, TeamProject,
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
        format!("workspaces: {}", team.workspaces),
        format!("id: {}", team.id),
    ]
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

/// One project per line: name, goal, id.
pub fn project_table(projects: &[TeamProject]) -> Vec<String> {
    projects
        .iter()
        .map(|project| format!("{}	{}	{}", project.name, project.goal, project.id))
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
