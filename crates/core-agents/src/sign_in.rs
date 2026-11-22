//! Sign-in to an account (decision 0059): the account's empty home, a service-owned terminal that runs the kind's login
//! with the home variable set, and the decision when that terminal ends: keep the account with what its check found, or
//! delete it. The same decision is made at startup for a sign-in an earlier service left open.

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

use crate::account_check::{account_strip_names, sign_in_env, state_name};
use crate::accounts::{DEFAULT_LABEL, ORIGIN_SIGN_IN, valid_label};
use crate::credentials;
use crate::sessions::{AgentError, Agents};

/// How often a sign-in terminal's exit is looked for.
const SIGN_IN_POLL: Duration = Duration::from_millis(200);
/// The scope a sign-in terminal is listed under: no workspace owns it.
pub const SIGN_IN_SCOPE: &str = "agent-accounts";

impl Agents {
    /// Starts a sign-in (decision 0059): an empty account home, and a service-owned terminal that runs the kind's login
    /// with the home variable set. The account is pending until the terminal ends; a pending account cannot be removed or
    /// activated. When the terminal ends, see [`Agents::decide_sign_in`].
    pub fn accounts_add(
        &self,
        kind: &str,
        label: Option<String>,
    ) -> Result<AccountsAdded, AgentError> {
        let label = label.unwrap_or_else(|| DEFAULT_LABEL.to_owned());
        valid_label(&label)?;
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
            label,
            created_at: now_ms(),
            identity: None,
            state: state_name(AccountState::NeedsSignIn).to_owned(),
            origin: ORIGIN_SIGN_IN.to_owned(),
        };
        let home = {
            let gate = credentials::lock_homes();
            self.migrate_legacy(&gate, kind)?;
            let home = credentials::create_account_home(self.profile_dir(), kind, &account)?;
            if let Err(error) = self.store().insert_pending_sign_in(&row) {
                let _ = std::fs::remove_dir_all(&home);
                return Err(error.into());
            }
            // Pending from the row's creation, under the same gate hold, so no removal can see the row without it.
            self.account_busy()
                .pending
                .insert(account.clone(), String::new());
            home
        };
        let env = sign_in_env(
            &credential_decl.home_env,
            &home,
            &account_strip_names(&decl),
        );
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
                self.account_busy().pending.remove(&account);
                self.discard_account(kind, &account);
                return Err(error.into());
            }
        };
        self.account_busy()
            .pending
            .insert(account.clone(), terminal.id.clone());
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

    /// Runs when a sign-in terminal ends: the account is no longer pending, and is then decided.
    fn finish_sign_in(&self, kind: &str, account: &str, terminal: &str) {
        wait_for_exit(self.terminals(), terminal);
        self.account_busy().pending.remove(account);
        self.decide_sign_in(kind, account);
    }

    /// Keeps a sign-in account or deletes it (decision 0059). With a check that reports signed in, the account is kept as
    /// signed in, with the check's own identity. A check that reports anything else deletes it. A check that did not finish
    /// (`unknown`), or a kind with no check, keeps it only if its declared files exist, as `unknown`. A row that is already
    /// gone is left alone, with no event.
    pub(crate) fn decide_sign_in(&self, kind: &str, account: &str) {
        // Only a sign-in the service made is decided, so no other row can be discarded from here (decision 0059).
        match self.store().get_agent_account(account) {
            Ok(Some(row)) if row.kind == kind && row.origin == ORIGIN_SIGN_IN => {}
            _ => return,
        }
        let files =
            credentials::account_files_present(self.profile_dir(), kind, account).unwrap_or(false);
        let verdict = match self.startable_kind(kind) {
            Ok((decl, program)) => self.check_account_home(&decl, &program, account, None),
            Err(_) => None,
        };
        let (keep, state, identity) = match verdict {
            Some(found) if found.state == AccountState::SignedIn => {
                (true, AccountState::SignedIn, found.identity)
            }
            Some(found) if found.state != AccountState::Unknown => (false, found.state, None),
            _ => (files, AccountState::Unknown, None),
        };
        if !keep {
            self.discard_account(kind, account);
            return;
        }
        let gate = credentials::lock_homes();
        match self
            .store()
            .set_agent_account_check(account, state_name(state), identity.as_deref())
        {
            Ok(true) => {
                // Decided: the account is kept, so it is no longer an open sign-in (decision 0059).
                if let Err(error) = self.store().set_agent_account_pending(account, false) {
                    tracing::warn!(kind, code = %error, "a decided sign-in could not be cleared");
                }
            }
            Ok(false) => return,
            Err(error) => {
                tracing::warn!(kind, code = %error, "a sign-in could not be recorded");
                return;
            }
        }
        self.make_active_if_none(kind, account);
        drop(gate);
        self.notify_account(kind, account, state);
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
        format!(
            "agent kind {kind} has no sign-in in MassOrca: sign in from the vendor's CLI, then import the login"
        ),
    )
}
