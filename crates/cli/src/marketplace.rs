//! `massorca marketplace services ...` (SKL-35): the hosted services of a host. A token comes from stdin, a hidden
//! prompt or the environment variable `--token-env` names, never an argument; nothing here prints one.

use clap::{Args, Subcommand};
use massorca_protocol::methods::hosted_services::{
    ServiceConnect, ServiceList, ServiceName, ServiceState, ServiceStatus, ServiceStatusParams,
};

use crate::connect::connect;
use crate::error::{CliError, exit};
use crate::output::Output;
use crate::secret_input;
use crate::target::Target;

#[derive(Debug, Subcommand)]
pub enum MarketplaceCommand {
    /// Lists, connects and disconnects GitLab, Bitbucket, Azure DevOps and Gitea; shows Linear and Jira.
    Services {
        #[command(subcommand)]
        command: ServicesCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ServicesCommand {
    /// Lists each service and its connection state on the host.
    List,
    /// Connects a service with a token read from stdin, a hidden prompt, or `--token-env`.
    Connect(ConnectArgs),
    /// Deletes what MassOrca stored for a service on the host.
    Disconnect {
        /// gitlab, bitbucket, azure-devops or gitea.
        service: String,
    },
}

#[derive(Debug, Args)]
pub struct ConnectArgs {
    /// gitlab, bitbucket, azure-devops or gitea.
    pub service: String,
    /// The server address of a self-hosted service, such as https://gitlab.example.com.
    #[arg(long, value_name = "URL")]
    pub base_url: Option<String>,
    /// Bitbucket only: the account email, for an Atlassian API token.
    #[arg(long)]
    pub email: Option<String>,
    /// Reads the token from this environment variable instead of stdin.
    #[arg(long, value_name = "NAME")]
    pub token_env: Option<String>,
}

/// Runs one `marketplace` subcommand against the service `target` names.
pub async fn execute(
    target: &Target,
    autostart: bool,
    command: MarketplaceCommand,
    output: Output,
) -> Result<(), CliError> {
    let MarketplaceCommand::Services { command } = command;
    // Read before connecting, so a missing token fails without starting the service.
    let token = match &command {
        ServicesCommand::Connect(args) => Some(read_token(args)?),
        _ => None,
    };
    let client = connect(target, autostart).await?;
    let row = match command {
        ServicesCommand::List => {
            let list = client
                .hosted_service_status(&ServiceStatusParams::default())
                .await?;
            let lines: Vec<String> = list.services.iter().map(line).collect();
            output.success(&list, &lines);
            return Ok(());
        }
        ServicesCommand::Connect(args) => {
            let params = ServiceConnect {
                service: args.service,
                token: token.unwrap_or_default(),
                base_url: args.base_url,
                email: args.email,
            };
            client.hosted_service_connect(&params).await?
        }
        ServicesCommand::Disconnect { service } => {
            client
                .hosted_service_disconnect(&ServiceName { service })
                .await?
        }
    };
    let lines = vec![line(&row)];
    output.success(
        &ServiceList {
            services: vec![row],
        },
        &lines,
    );
    Ok(())
}

fn read_token(args: &ConnectArgs) -> Result<String, CliError> {
    let token = match &args.token_env {
        Some(name) => std::env::var(name).unwrap_or_default(),
        None => secret_input::read_hidden_or_stdin(&format!("Token for {}: ", args.service))?,
    };
    let token = token.trim().to_owned();
    if token.is_empty() {
        return Err(CliError::new(
            "marketplace.empty_token",
            "no token was given; pipe it on stdin or name a variable with --token-env",
            exit::ERROR,
        ));
    }
    Ok(token)
}

fn line(row: &ServiceStatus) -> String {
    let state = match row.state {
        ServiceState::Connected => "connected",
        ServiceState::NotConnected => "not connected",
        ServiceState::Unknown => "unknown",
    };
    let account = row
        .account
        .as_ref()
        .map_or_else(String::new, |account| format!(" as {account}"));
    let detail = row
        .detail
        .as_ref()
        .or(row.base_url.as_ref())
        .map_or_else(String::new, |detail| format!(" ({detail})"));
    format!("{}: {state}{account}{detail}", row.name)
}
