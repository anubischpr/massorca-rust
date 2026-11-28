//! `massorca team delegate` (decision 0128): a bot's agent run hands a piece of work to a teammate. The run's session and
//! token come from its environment, so the call is refused before connecting when it is not an agent run.

use clap::Args;
use massorca_protocol::codes;
use massorca_protocol::methods::teams::{TeamDelegateParams, TeamDelegated};

use crate::agent_hook::{ENV_SESSION, ENV_TOKEN};
use crate::connect::connect;
use crate::error::{CliError, exit};
use crate::output::Output;
use crate::target::Target;

/// Hands a task to a teammate from inside a bot's run.
#[derive(Debug, Args)]
pub struct DelegateArgs {
    /// The teammate that takes the task: id or name within the team. Absent: picked by its job and skills.
    #[arg(long, value_name = "BOT")]
    pub bot: Option<String>,
    /// What the teammate does. Its first line is the title.
    #[arg(long, value_name = "TEXT")]
    pub text: String,
    /// The team the task is filed in: id or name. Absent: the caller's own team.
    #[arg(long, value_name = "TEAM")]
    pub team: Option<String>,
    /// The teammate only reads (research, review, explanation): it works in the parent's workspace, not its own.
    #[arg(long)]
    pub reads: bool,
}

/// Runs `team delegate`. Outside an agent run it refuses with `team.delegate_not_a_bot` and connects to nothing.
pub async fn run(
    target: &Target,
    autostart: bool,
    args: DelegateArgs,
    output: Output,
) -> Result<(), CliError> {
    let (Some(session), Some(agent_token)) = (env_value(ENV_SESSION), env_value(ENV_TOKEN)) else {
        return Err(CliError::new(
            codes::TEAM_DELEGATE_NOT_A_BOT,
            "team delegate runs only inside a bot's agent run; a person hands work to a bot from the Conference room",
            exit::ERROR,
        ));
    };
    let request_id = request_id_for(
        &session,
        &args.text,
        args.bot.as_deref(),
        args.team.as_deref(),
        args.reads,
    );
    let client = connect(target, autostart).await?;
    let delegated = client
        .team_delegate(&TeamDelegateParams {
            bot: args.bot,
            text: args.text,
            team: args.team,
            read_only: args.reads,
            session: Some(session),
            agent_token: Some(agent_token),
            request_id: Some(request_id),
        })
        .await?;
    output.success(&delegated, &lines(&delegated));
    Ok(())
}

/// The key of one delegation: the same run, task, bot, team and read flag give the same key, so a retried call returns the
/// child the first call made (decision 0128). FNV-1a, so it is the same in every build.
fn request_id_for(
    session: &str,
    text: &str,
    bot: Option<&str>,
    team: Option<&str>,
    reads: bool,
) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let reads = if reads { "reads" } else { "writes" };
    for part in [session, text, bot.unwrap_or(""), team.unwrap_or(""), reads] {
        for byte in part.bytes().chain(std::iter::once(0)) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("cli-{hash:016x}")
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

/// The reply a delegation prints: the child, where it starts, and whether competence picked its bot.
fn lines(delegated: &TeamDelegated) -> Vec<String> {
    let mut lines = vec![format!(
        "delegated task {} ({}){}",
        delegated.task.id,
        delegated.task.title,
        if delegated.picked {
            ", bot picked by competence"
        } else {
            ""
        }
    )];
    if let Some(error) = &delegated.task.error {
        lines.push(format!("the child did not start: {error}"));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::request_id_for;

    #[test]
    fn a_retried_delegation_keeps_its_key_and_a_different_one_gets_another() {
        let key = request_id_for("s1", "Parse", Some("mara"), None, false);
        assert_eq!(
            key,
            request_id_for("s1", "Parse", Some("mara"), None, false)
        );
        assert_ne!(
            key,
            request_id_for("s2", "Parse", Some("mara"), None, false)
        );
        assert_ne!(key, request_id_for("s1", "Lex", Some("mara"), None, false));
        assert_ne!(key, request_id_for("s1", "Parse", Some("jun"), None, false));
        assert_ne!(key, request_id_for("s1", "Parse", Some("mara"), None, true));
    }
}
