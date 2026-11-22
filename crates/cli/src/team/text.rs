//! The text lines of the `team` commands. JSON output is the protocol document itself.

use massorca_protocol::methods::teams::{Board, Bot, Team, TeamCharacter, TeamProject};

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
