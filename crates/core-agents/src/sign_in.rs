//! Sign-in to an account (decision 0059): the account's empty home, a service-owned terminal that runs the kind's login
//! with the home variable set, and the decision when that terminal ends (keep the account, or delete it).

use std::collections::BTreeMap;
use std::thread;
use std::time::Duration;

use massorca_core_store::{AgentAccountRow, new_id, now_ms};
use massorca_core_terminal::pty::PtySize;
use massorca_core_terminal::{
    CreateSpec, ShellChoice, TerminalManager, TerminalState as CoreState,
};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{AccountState, AccountsAdded};
use massorca_protocol::methods::terminals::{DEFAULT_COLS, DEFAULT_ROWS};

use crate::account_check::state_name;
use crate::accounts::DEFAULT_LABEL;
use crate::credentials;
use crate::sessions::{AgentError, Agents};

/// How often a sign-in terminal's exit is looked for.
const SIGN_IN_POLL: Duration = Duration::from_millis(200);
/// The scope a sign-in terminal is listed under: no workspace owns it.
pub const SIGN_IN_SCOPE: &str = "agent-accounts";

impl Agents {
    /// Starts a sign-in (decision 0059): an empty account home, and a service-owned terminal that runs the kind's login
    /// with the home variable set. When that terminal ends, the account is kept if its declared files exist or its check
    /// reports signed in, and deleted otherwise.
    pub fn accounts_add(
        &self,
        kind: &str,
        label: Option<String>,
    ) -> Result<AccountsAdded, AgentError> {
        let (decl, program) = self.startable_kind(kind)?;
        let credential_decl = decl
            .credentials
            .clone()
            .filter(|_| decl.accounts_supported())
            .ok_or_else(|| unsupported(kind))?;
        let login = decl
            .accounts
            .as_ref()
            .map(|accounts| accounts.login.clone())
            .unwrap_or_default();
        let account = new_id();
        let row = AgentAccountRow {
            id: account.clone(),
            kind: kind.to_owned(),
            label: label.unwrap_or_else(|| DEFAULT_LABEL.to_owned()),
            created_at: now_ms(),
            identity: None,
            state: state_name(AccountState::NeedsSignIn).to_owned(),
        };
        let home = {
            let gate = credentials::lock_homes();
            self.migrate_legacy(&gate, kind)?;
            let home = credentials::create_account_home(self.profile_dir(), kind, &account)?;
            if let Err(error) = self.store().insert_agent_account(&row) {
                let _ = std::fs::remove_dir_all(&home);
                return Err(error.into());
            }
            home
        };
        let mut env = BTreeMap::new();
        env.insert(credential_decl.home_env.clone(), home.display().to_string());
        // The terminal can only add variables (docs/architecture/agents.md), so a stripped one is set to empty.
        for name in &credential_decl.strip_env {
            env.entry(name.clone()).or_default();
        }
        let spec = CreateSpec {
            workspace_id: SIGN_IN_SCOPE.to_owned(),
            cwd: home,
            shell: Some(ShellChoice {
                program: program.to_string_lossy().into_owned(),
                args: login,
            }),
            env,
            size: PtySize {
                cols: DEFAULT_COLS,
                rows: DEFAULT_ROWS,
            },
            title: Some(format!("{} sign-in", decl.name)),
            request_id: None,
        };
        let terminal = match self.terminals().create(spec) {
            Ok(terminal) => terminal,
            Err(error) => {
                self.discard_account(kind, &account);
                return Err(error.into());
            }
        };
        let summary = self.summary(
            &row,
            self.store().get_active_agent_account(kind)?.as_deref(),
        )?;
        self.notify_account(kind, &account, AccountState::NeedsSignIn);
        let agents = self.clone();
        let (kind_owned, watched, terminal_id) =
            (kind.to_owned(), account.clone(), terminal.id.clone());
        thread::spawn(move || agents.finish_sign_in(&kind_owned, &watched, &terminal_id));
        Ok(AccountsAdded {
            account: summary,
            terminal_id: terminal.id,
        })
    }

    /// Runs when a sign-in terminal ends: keeps the account signed in, or deletes it (decision 0059).
    fn finish_sign_in(&self, kind: &str, account: &str, terminal: &str) {
        wait_for_exit(self.terminals(), terminal);
        let checked = match self.startable_kind(kind) {
            Ok((decl, program)) => self.check_account_home(&decl, &program, account, None),
            Err(_) => None,
        };
        let by_check = checked
            .as_ref()
            .is_some_and(|found| found.state == AccountState::SignedIn);
        let files =
            credentials::account_files_present(self.profile_dir(), kind, account).unwrap_or(false);
        if !by_check && !files {
            self.discard_account(kind, account);
            return;
        }
        let identity = checked.and_then(|found| found.identity);
        if let Err(error) = self.store().set_agent_account_check(
            account,
            state_name(AccountState::SignedIn),
            identity.as_deref(),
        ) {
            tracing::warn!(kind, code = %error, "a sign-in could not be recorded");
            return;
        }
        self.make_active_if_none(kind, account);
        self.notify_account(kind, account, AccountState::SignedIn);
    }
}

/// Waits until the terminal has exited, or can no longer be found (it was closed).
fn wait_for_exit(terminals: &TerminalManager, terminal: &str) {
    loop {
        match terminals.get(terminal) {
            Ok(info) if !matches!(info.state, CoreState::Exited) => thread::sleep(SIGN_IN_POLL),
            _ => return,
        }
    }
}

fn unsupported(kind: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_CREDENTIALS_UNSUPPORTED,
        format!("agent kind {kind} declares no sign-in, so it has no accounts to add"),
    )
}
