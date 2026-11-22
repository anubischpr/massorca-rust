//! The text lines of the `task` commands. JSON output is the protocol document itself.

use massorca_protocol::methods::tasks::Task;

/// `created task parse the header (id) in column Open`, or the verb the command used.
pub fn task_line(task: &Task, verb: &str) -> String {
    format!(
        "{verb} task {} ({}) in column {}",
        task.title,
        task.id,
        task.column.label()
    )
}

/// One line per task: id, title, state word, and column.
pub fn task_table(tasks: &[Task]) -> Vec<String> {
    tasks
        .iter()
        .map(|task| {
            format!(
                "{}\t{}\t{}\t{}",
                task.id,
                task.title,
                task.state.label(),
                task.column.label()
            )
        })
        .collect()
}

pub fn task_details(task: &Task) -> Vec<String> {
    let mut lines = vec![
        format!("id: {}", task.id),
        format!("title: {}", task.title),
        format!("team: {}", task.team_id),
        format!("column: {}", task.column.label()),
        format!("state: {}", task.state.label()),
        format!("bot: {}", task.bot_id.as_deref().unwrap_or("none")),
        format!("project: {}", task.project_id.as_deref().unwrap_or("none")),
    ];
    if task.last_known {
        lines.push("state is the last one the agent reported".to_owned());
    }
    if let Some(error) = &task.error {
        lines.push(format!("error: {error}"));
    }
    if !task.body.is_empty() {
        lines.push(format!("body: {}", task.body));
    }
    lines
}
