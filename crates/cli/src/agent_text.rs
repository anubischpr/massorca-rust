//! Human-readable lines for agent kinds and sessions.

use massorca_protocol::methods::agents::AgentKind;

use crate::workspace_text;

/// The kinds as a table, then one line per problem of a kind that has any.
pub fn kinds_lines(kinds: &[AgentKind]) -> Vec<String> {
    if kinds.is_empty() {
        return vec!["no agent kinds".to_owned()];
    }
    let rows = kinds
        .iter()
        .map(|kind| {
            vec![
                kind.id.clone(),
                kind.name.clone(),
                kind.owner.clone(),
                installed_text(kind),
            ]
        })
        .collect();
    let mut lines = workspace_text::table(&["ID", "NAME", "OWNER", "INSTALLED"], rows);
    for kind in kinds {
        for problem in &kind.problems {
            lines.push(format!("problem: {}: {problem}", kind.id));
        }
    }
    lines
}

/// The resolved program, `invalid` for a declaration with problems, or `no`.
fn installed_text(kind: &AgentKind) -> String {
    match (&kind.program_path, kind.problems.is_empty()) {
        (Some(path), _) if kind.installed => path.clone(),
        (_, false) => "invalid".to_owned(),
        _ => "no".to_owned(),
    }
}
