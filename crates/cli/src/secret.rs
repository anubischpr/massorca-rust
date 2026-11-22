//! `massorca secret ...`: the profile's secret vault (decision 0071). Values go in through `set` only, and nothing
//! here ever prints one: the service never returns it.

use clap::{Args, Subcommand};
use massorca_client::Client;
use massorca_protocol::methods::secrets::{
    SecretDecline, SecretEntry, SecretLink, SecretListParams, SecretName, SecretRemove,
    SecretRequest, SecretRequestParams, SecretRequestsParams, SecretSet,
};

use crate::agent_hook::{ENV_SESSION as ENV_AGENT_SESSION, ENV_TOKEN as ENV_AGENT_TOKEN};
use crate::connect::connect;
use crate::error::CliError;
use crate::output::Output;
use crate::secret_input;
use crate::target::Target;

#[derive(Debug, Subcommand)]
pub enum SecretCommand {
    /// Shows where the vault key is kept, whether it opens, and how many secrets there are.
    Status,
    /// Lists secret names by scope, owned and linked. Never values.
    List(ListArgs),
    /// Stores a secret, or rotates one with `--replace`. The value comes from stdin or a hidden prompt.
    Set(SetArgs),
    /// Uses the secret of the same name that another scope owns.
    Link(LinkArgs),
    /// Removes a link; the owner's secret stays.
    Unlink(NameArgs),
    /// Removes an owned secret. Refuses one that other scopes link to unless `--include-links`.
    Remove(RemoveArgs),
    /// Asks for a secret by name, with a reason the owner sees.
    Request(RequestArgs),
    /// Lists open requests for secrets, or every request with `--all`.
    Requests(RequestsArgs),
    /// Declines a pending request.
    Decline(DeclineArgs),
    /// Forgets the vault key while no secret exists, so the next `set` creates a fresh one.
    ResetKey,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Only this scope: `global`, `project:<id>` or `team:<id>`.
    #[arg(long)]
    pub scope: Option<String>,
}

/// A name in a scope.
#[derive(Debug, Args)]
pub struct NameArgs {
    /// `global`, `project:<id>` or `team:<id>`.
    pub scope: String,
    /// Variable name: letters, digits and `_`, not starting with a digit.
    pub name: String,
}

#[derive(Debug, Args)]
pub struct SetArgs {
    #[command(flatten)]
    pub target: NameArgs,
    /// Rotates an existing secret of this name.
    #[arg(long)]
    pub replace: bool,
}

#[derive(Debug, Args)]
pub struct LinkArgs {
    #[command(flatten)]
    pub target: NameArgs,
    /// The scope that owns the secret.
    #[arg(long = "from", value_name = "SCOPE")]
    pub from: String,
}

#[derive(Debug, Args)]
pub struct RemoveArgs {
    #[command(flatten)]
    pub target: NameArgs,
    /// Also removes every scope's link to it.
    #[arg(long)]
    pub include_links: bool,
    /// Removes it even while the vault is locked.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct RequestArgs {
    #[command(flatten)]
    pub target: NameArgs,
    /// Why the secret is needed; plain text, at most 200 characters.
    #[arg(long)]
    pub reason: String,
    /// The agent session asking, if any.
    #[arg(long)]
    pub session: Option<String>,
}

#[derive(Debug, Args)]
pub struct RequestsArgs {
    /// Also lists resolved requests.
    #[arg(long)]
    pub all: bool,
}

#[derive(Debug, Args)]
pub struct DeclineArgs {
    /// The request id.
    pub request: String,
}

/// Runs one `secret` subcommand against the service `target` names.
pub async fn execute(
    target: &Target,
    autostart: bool,
    command: SecretCommand,
    output: Output,
) -> Result<(), CliError> {
    if let SecretCommand::Set(args) = command {
        // Read before connecting, so an empty value never reaches or starts the service.
        let value = secret_input::read_value(&args.target.scope, &args.target.name)?;
        let client = connect(target, autostart).await?;
        return set(&client, args, value, output).await;
    }
    let client = connect(target, autostart).await?;
    run(&client, command, output).await
}

/// Every subcommand but `set`, which reads its value before it connects.
async fn run(client: &Client, command: SecretCommand, output: Output) -> Result<(), CliError> {
    match command {
        SecretCommand::Status => status(client, output).await,
        SecretCommand::List(args) => list(client, args, output).await,
        SecretCommand::Link(args) => link(client, args, output).await,
        SecretCommand::Unlink(args) => unlink(client, args, output).await,
        SecretCommand::Remove(args) => remove(client, args, output).await,
        SecretCommand::Request(args) => request(client, args, output).await,
        SecretCommand::Requests(args) => requests(client, args, output).await,
        SecretCommand::Decline(args) => decline(client, args, output).await,
        SecretCommand::ResetKey => reset_key(client, output).await,
        SecretCommand::Set(args) => {
            let value = secret_input::read_value(&args.target.scope, &args.target.name)?;
            set(client, args, value, output).await
        }
    }
}

async fn status(client: &Client, output: Output) -> Result<(), CliError> {
    let status = client.secret_status().await?;
    let mut lines = vec![
        format!("protection: {}", status.protection),
        format!("state: {}", status.state),
        format!("secrets: {}", status.count),
    ];
    for clash in &status.name_clashes {
        lines.push(format!(
            "names that differ only by case, refused at agent start until one is renamed: {clash}"
        ));
    }
    if status.protection == "file" {
        lines.push(
            "the vault key is stored on disk next to the data: anyone who can read the profile directory can read the secrets"
                .to_owned(),
        );
    }
    output.success(&status, &lines);
    Ok(())
}

async fn reset_key(client: &Client, output: Output) -> Result<(), CliError> {
    let status = client.secret_reset_key().await?;
    output.success(
        &status,
        &["the vault key was reset; the next secret creates a fresh one".to_owned()],
    );
    Ok(())
}

async fn list(client: &Client, args: ListArgs, output: Output) -> Result<(), CliError> {
    let list = client
        .secret_list(&SecretListParams { scope: args.scope })
        .await?;
    let lines: Vec<String> = list.secrets.iter().map(entry_line).collect();
    output.success(&list, &lines);
    Ok(())
}

async fn set(
    client: &Client,
    args: SetArgs,
    value: String,
    output: Output,
) -> Result<(), CliError> {
    let params = SecretSet {
        scope: args.target.scope,
        name: args.target.name,
        value,
        replace: args.replace,
    };
    let entry = client.secret_set(&params).await?;
    drop(params);
    let verb = if args.replace { "rotated" } else { "stored" };
    output.success(&entry, &[format!("{verb} {} {}", entry.scope, entry.name)]);
    Ok(())
}

async fn link(client: &Client, args: LinkArgs, output: Output) -> Result<(), CliError> {
    let entry = client
        .secret_link(&SecretLink {
            scope: args.target.scope,
            name: args.target.name,
            from_scope: args.from,
        })
        .await?;
    output.success(&entry, &[format!("linked {}", entry_line(&entry))]);
    Ok(())
}

async fn unlink(client: &Client, args: NameArgs, output: Output) -> Result<(), CliError> {
    let removed = client
        .secret_unlink(&SecretName {
            scope: args.scope,
            name: args.name,
        })
        .await?;
    output.success(
        &removed,
        &[format!("unlinked {} {}", removed.scope, removed.name)],
    );
    Ok(())
}

async fn remove(client: &Client, args: RemoveArgs, output: Output) -> Result<(), CliError> {
    let removed = client
        .secret_remove(&SecretRemove {
            scope: args.target.scope,
            name: args.target.name,
            include_links: args.include_links,
            force: args.force,
        })
        .await?;
    let mut line = format!("removed {} {}", removed.scope, removed.name);
    if !removed.links_removed.is_empty() {
        line.push_str(&format!(
            " and its links in {}",
            removed.links_removed.join(", ")
        ));
    }
    output.success(&removed, &[line]);
    Ok(())
}

async fn request(client: &Client, args: RequestArgs, output: Output) -> Result<(), CliError> {
    let request = client
        .secret_request(&agent_request(SecretRequestParams {
            scope: args.target.scope,
            name: args.target.name,
            reason: args.reason,
            session: args.session,
            agent_token: None,
        }))
        .await?;
    output.success(&request, &[request_line(&request)]);
    Ok(())
}

async fn requests(client: &Client, args: RequestsArgs, output: Output) -> Result<(), CliError> {
    let requests = client
        .secret_requests(&SecretRequestsParams { all: args.all })
        .await?;
    let lines: Vec<String> = requests.requests.iter().map(request_line).collect();
    output.success(&requests, &lines);
    Ok(())
}

async fn decline(client: &Client, args: DeclineArgs, output: Output) -> Result<(), CliError> {
    let request = client
        .secret_decline(&SecretDecline {
            request: args.request,
        })
        .await?;
    output.success(&request, &[request_line(&request)]);
    Ok(())
}

/// Inside an agent run, the run's session and token, so the request counts against the agent's own quota. A
/// `--session` naming another session is sent as given, without the token.
fn agent_request(mut params: SecretRequestParams) -> SecretRequestParams {
    let (Ok(session), Ok(token)) = (
        std::env::var(ENV_AGENT_SESSION),
        std::env::var(ENV_AGENT_TOKEN),
    ) else {
        return params;
    };
    if params
        .session
        .as_deref()
        .is_none_or(|given| given == session)
    {
        params.session = Some(session);
        params.agent_token = Some(token);
    }
    params
}

/// `<scope> <name>`, then where a link points or which scopes link to an owned secret.
fn entry_line(entry: &SecretEntry) -> String {
    let base = format!("{} {}", entry.scope, entry.name);
    if entry.linked {
        format!("{base} (linked from {})", entry.owner_scope)
    } else if entry.linked_scopes.is_empty() {
        base
    } else {
        format!("{base} (linked by {})", entry.linked_scopes.join(", "))
    }
}

fn request_line(request: &SecretRequest) -> String {
    format!(
        "{} {} {} {}: {}",
        request.id, request.state, request.scope, request.name, request.reason
    )
}
