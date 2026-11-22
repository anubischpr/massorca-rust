//! Accounts of a kind (decision 0059): several credential homes per kind, one active per kind, per-project choices,
//! sign-in through a service-owned terminal, import, checks, sign-out and removal. The files of an account follow the
//! rules of 0056 (see [`crate::credentials`]); this module decides which account a run uses and what its state is.

use std::path::{Path, PathBuf};
use std::sync::MutexGuard;

use massorca_core_store::{AgentAccountRow, new_id, now_ms};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    AccountState, AccountSummary, AccountsChanged, AccountsImported, AccountsRemoved,
    CredentialsImported, CredentialsRemoved, CredentialsShown, ImportResult,
};

use crate::account_check::{
    Verdict, parse_state, run_program, settle, state_name, unknown, verdict,
};
use crate::credentials;
use crate::kind::{KindDecl, valid_id};
use crate::sessions::{AgentError, Agents, Change};
/// The label of an account when the person gives none.
pub(crate) const DEFAULT_LABEL: &str = "account";
/// The label of the account made from 0056's single home.
const MIGRATED_LABEL: &str = "default";

/// Which account a run uses, and its home (decision 0059). Both are absent when the kind has no account, and the agent
/// then uses its usual home, as in 0056.
#[derive(Debug, Default)]
pub struct AccountUse {
    pub home: Option<PathBuf>,
    pub account: Option<String>,
}

fn kind_not_found(kind: &str) -> AgentError {
    AgentError::new(codes::AGENT_KIND_NOT_FOUND, format!("no agent kind {kind}"))
}

fn account_not_found(kind: &str, account: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_ACCOUNT_NOT_FOUND,
        format!("no account {account} of agent kind {kind}"),
    )
}

fn busy(account: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_BUSY,
        format!("a live session uses account {account}; stop or forget it first"),
    )
}

fn already_has_home(kind: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_CREDENTIALS_EXIST,
        format!("agent kind {kind} already has an active account; import with replace to swap it"),
    )
}

impl Agents {
    /// The accounts of a kind, oldest first, with the active one and the projects that chose each.
    pub fn accounts_list(&self, kind: &str) -> Result<Vec<AccountSummary>, AgentError> {
        if !valid_id(kind) {
            return Err(kind_not_found(kind));
        }
        self.migrate_now(kind)?;
        let active = self.store().get_active_agent_account(kind)?;
        self.store()
            .list_agent_accounts(kind)?
            .iter()
            .map(|row| self.summary(row, active.as_deref()))
            .collect()
    }

    /// Imports the kind's declared files from `from` into a new account (decision 0059), then checks it. The result is
    /// `imported`; `failed` when the check reports the login is not signed in (the new account is then deleted); or
    /// `not_found` when none of the declared files is in `from`.
    pub fn accounts_import(
        &self,
        kind: &str,
        from: &str,
        label: Option<String>,
    ) -> Result<AccountsImported, AgentError> {
        let label = label.unwrap_or_else(|| DEFAULT_LABEL.to_owned());
        let (account, _) = match self.import_new_account(kind, Path::new(from), &label) {
            Ok(added) => added,
            Err(error) if error.code == codes::AGENT_CREDENTIALS_UNSUPPORTED => return Err(error),
            Err(error) if all_missing(&error) => {
                return Ok(AccountsImported {
                    result: ImportResult::NotFound,
                    account: None,
                    reason: Some("none of the declared files is in the source".to_owned()),
                });
            }
            Err(error) => {
                return Ok(AccountsImported {
                    result: ImportResult::Failed,
                    account: None,
                    reason: Some(error.message),
                });
            }
        };
        if let Some(AccountState::Expired | AccountState::NeedsSignIn) =
            self.check_imported(kind, &account)
        {
            self.discard_account(kind, &account);
            return Ok(AccountsImported {
                result: ImportResult::Failed,
                account: None,
                reason: Some("the check reports the login is not signed in".to_owned()),
            });
        }
        self.make_active_if_none(kind, &account);
        let row = self.account_of(kind, &account)?;
        let summary = self.summary(
            &row,
            self.store().get_active_agent_account(kind)?.as_deref(),
        )?;
        Ok(AccountsImported {
            result: ImportResult::Imported,
            account: Some(summary),
            reason: None,
        })
    }

    /// Makes an account the kind's active one, or the project's choice for `project` (decision 0059).
    pub fn accounts_activate(
        &self,
        kind: &str,
        account: &str,
        project: Option<&str>,
    ) -> Result<AccountSummary, AgentError> {
        let row = self.account_of(kind, account)?;
        match project {
            None => self.store().set_active_agent_account(kind, account)?,
            Some(project) => {
                if self.store().get_project(project)?.is_none() {
                    return Err(AgentError::new(
                        codes::PROJECT_NOT_FOUND,
                        format!("no project {project}"),
                    ));
                }
                self.store()
                    .set_agent_account_choice(kind, project, account)?;
            }
        }
        let active = self.store().get_active_agent_account(kind)?;
        self.summary(&row, active.as_deref())
    }

    /// Deletes an account (decision 0059). Refused with `agent.busy` while a live run uses it. The kind's sign-out runs
    /// first when declared; a failed sign-out does not keep the home. Nothing becomes active afterwards.
    pub fn accounts_remove(
        &self,
        kind: &str,
        account: &str,
    ) -> Result<AccountsRemoved, AgentError> {
        self.account_of(kind, account)?;
        let gate = credentials::lock_homes();
        if self
            .live_runs()
            .values()
            .any(|run| run.account.as_deref() == Some(account))
        {
            return Err(busy(account));
        }
        if let Ok((decl, program)) = self.startable_kind(kind) {
            self.sign_out(&decl, &program, kind, account);
        }
        credentials::remove_account_locked(self.profile_dir(), kind, account)?;
        self.store().delete_agent_account(account)?;
        drop(gate);
        Ok(AccountsRemoved {
            kind: kind.to_owned(),
            account: account.to_owned(),
            removed: true,
        })
    }

    /// Runs the kind's check on an account now, or on the active one when `account` is absent (decision 0059).
    pub fn accounts_check(
        &self,
        kind: &str,
        account: Option<&str>,
    ) -> Result<AccountSummary, AgentError> {
        let id = match account {
            Some(account) => account.to_owned(),
            None => self
                .store()
                .get_active_agent_account(kind)?
                .ok_or_else(|| account_not_found(kind, "active"))?,
        };
        let row = self.account_of(kind, &id)?;
        let (decl, program) = self.startable_kind(kind)?;
        let state = if decl
            .accounts
            .as_ref()
            .and_then(|accounts| accounts.check.as_ref())
            .is_none()
        {
            unknown()
        } else {
            self.check_account_home(&decl, &program, &id, row.identity.as_deref())
                .unwrap_or(Verdict {
                    state: AccountState::NeedsSignIn,
                    identity: None,
                })
        };
        self.store().set_agent_account_check(
            &id,
            state_name(state.state),
            state.identity.as_deref(),
        )?;
        if parse_state(&row.state) != state.state {
            self.notify_account(kind, &id, state.state);
        }
        let row = self.account_of(kind, &id)?;
        self.summary(
            &row,
            self.store().get_active_agent_account(kind)?.as_deref(),
        )
    }

    /// The account a run of `kind` in `project` uses, and its home (decision 0059): the project's choice, else the active
    /// account, else none. The home is taken only when it is a plain directory (0056). The caller holds `lock_homes`, so
    /// the home cannot be swapped before the run is registered.
    pub(crate) fn resolve_home(
        &self,
        gate: &MutexGuard<'_, ()>,
        kind: &str,
        project: &str,
    ) -> Result<AccountUse, AgentError> {
        self.migrate_legacy(gate, kind)?;
        let chosen = match self.store().get_agent_account_choice(kind, project)? {
            Some(account) => Some(account),
            None => self.store().get_active_agent_account(kind)?,
        };
        let Some(account) = chosen else {
            return Ok(AccountUse::default());
        };
        let home = credentials::usable_account_home(self.profile_dir(), kind, &account)?.ok_or_else(|| {
            AgentError::new(
                codes::AGENT_CREDENTIALS_INVALID,
                format!("the account of agent kind {kind} has no credential home; sign in again or remove it"),
            )
        })?;
        Ok(AccountUse {
            home: Some(home),
            account: Some(account),
        })
    }

    /// Whether the kind's active account has a usable home (decision 0059), for the kinds listing.
    pub(crate) fn has_usable_account(&self, kind: &str) -> bool {
        if self.migrate_now(kind).is_err() {
            return false;
        }
        let Ok(Some(account)) = self.store().get_active_agent_account(kind) else {
            return false;
        };
        let _gate = credentials::lock_homes();
        matches!(
            credentials::usable_account_home(self.profile_dir(), kind, &account),
            Ok(Some(_))
        )
    }

    /// `agent.credentials.import` (deprecated, 0056): imports into a new account that becomes the active one. The active
    /// account is refused unless `replace` is set, and is then deleted once the new one is in place.
    pub fn credentials_import(
        &self,
        kind: &str,
        from: &str,
        replace: bool,
    ) -> Result<CredentialsImported, AgentError> {
        self.migrate_now(kind)?;
        let active = self.store().get_active_agent_account(kind)?;
        if active.is_some() && !replace {
            return Err(already_has_home(kind));
        }
        let (account, imported) = self.import_new_account(kind, Path::new(from), DEFAULT_LABEL)?;
        if let Some(old) = active
            && let Err(error) = self.accounts_remove(kind, &old)
        {
            self.discard_account(kind, &account);
            return Err(error);
        }
        self.store().set_active_agent_account(kind, &account)?;
        Ok(imported)
    }

    /// `agent.credentials.show` (deprecated, 0056): the kind's active account's files, or the empty shape when it has none.
    pub fn credentials_show(&self, kind: &str) -> Result<CredentialsShown, AgentError> {
        self.migrate_now(kind)?;
        match self.store().get_active_agent_account(kind)? {
            Some(account) => credentials::show_account(self.profile_dir(), kind, &account),
            None => credentials::show(self.profile_dir(), kind),
        }
    }

    /// `agent.credentials.remove` (deprecated, 0056): removes the kind's active account.
    pub fn credentials_remove(&self, kind: &str) -> Result<CredentialsRemoved, AgentError> {
        self.migrate_now(kind)?;
        let removed = match self.store().get_active_agent_account(kind)? {
            Some(account) => self.accounts_remove(kind, &account)?.removed,
            None => false,
        };
        Ok(CredentialsRemoved {
            kind: kind.to_owned(),
            removed,
        })
    }

    fn migrate_now(&self, kind: &str) -> Result<(), AgentError> {
        let gate = credentials::lock_homes();
        self.migrate_legacy(&gate, kind)
    }

    /// Moves 0056's single home into an account named `default`, as the kind's active account when it has none (decision
    /// 0059). A same-volume rename under the gate; nothing happens when there is no legacy home.
    pub(crate) fn migrate_legacy(
        &self,
        _gate: &MutexGuard<'_, ()>,
        kind: &str,
    ) -> Result<(), AgentError> {
        if !valid_id(kind) || credentials::usable_home(self.profile_dir(), kind)?.is_none() {
            return Ok(());
        }
        let account = new_id();
        credentials::adopt_legacy_home(self.profile_dir(), kind, &account)?;
        let had_active = self.store().get_active_agent_account(kind)?.is_some();
        self.store().insert_agent_account(&AgentAccountRow {
            id: account.clone(),
            kind: kind.to_owned(),
            label: MIGRATED_LABEL.to_owned(),
            created_at: now_ms(),
            identity: None,
            state: state_name(AccountState::Migrated).to_owned(),
        })?;
        if !had_active {
            self.store().set_active_agent_account(kind, &account)?;
        }
        tracing::info!(kind, "made the credential home an account");
        Ok(())
    }

    /// The row of an account of `kind`; an account of another kind, or none, is `agent.account_not_found`.
    fn account_of(&self, kind: &str, account: &str) -> Result<AgentAccountRow, AgentError> {
        self.store()
            .get_agent_account(account)?
            .filter(|row| row.kind == kind)
            .ok_or_else(|| account_not_found(kind, account))
    }

    pub(crate) fn summary(
        &self,
        row: &AgentAccountRow,
        active: Option<&str>,
    ) -> Result<AccountSummary, AgentError> {
        Ok(AccountSummary {
            id: row.id.clone(),
            label: row.label.clone(),
            state: parse_state(&row.state),
            identity: row.identity.clone(),
            active: active == Some(row.id.as_str()),
            projects: self.store().projects_using_agent_account(&row.id)?,
        })
    }

    /// Creates a new account and copies the declared files into it, under the gate. Returns its id and the copy's record.
    fn import_new_account(
        &self,
        kind: &str,
        from: &Path,
        label: &str,
    ) -> Result<(String, CredentialsImported), AgentError> {
        // Not held across the import: `import_account` takes the gate itself (it is not reentrant).
        self.migrate_now(kind)?;
        let account = new_id();
        let imported = credentials::import_account(self.profile_dir(), kind, &account, from)?;
        self.store().insert_agent_account(&AgentAccountRow {
            id: account.clone(),
            kind: kind.to_owned(),
            label: label.to_owned(),
            created_at: now_ms(),
            identity: None,
            state: state_name(AccountState::Unknown).to_owned(),
        })?;
        Ok((account, imported))
    }

    /// Checks an imported account and records what it finds. `None` when the kind declares no check or cannot run one.
    fn check_imported(&self, kind: &str, account: &str) -> Option<AccountState> {
        let (decl, program) = self.startable_kind(kind).ok()?;
        let found = self.check_account_home(&decl, &program, account, None)?;
        let _ = self.store().set_agent_account_check(
            account,
            state_name(found.state),
            found.identity.as_deref(),
        );
        Some(found.state)
    }

    /// Runs the kind's check on an account's home and settles the result against the recorded identity. `None` when the
    /// kind declares no check, or the account has no home.
    pub(crate) fn check_account_home(
        &self,
        decl: &KindDecl,
        program: &Path,
        account: &str,
        recorded: Option<&str>,
    ) -> Option<Verdict> {
        let check = decl.accounts.as_ref()?.check.as_ref()?;
        let credential_decl = decl.credentials.as_ref()?;
        let home = {
            let _gate = credentials::lock_homes();
            credentials::usable_account_home(self.profile_dir(), &decl.id, account)
                .ok()
                .flatten()?
        };
        let run = run_program(
            program,
            &check.args,
            &home,
            &credential_decl.home_env,
            &credential_decl.strip_env,
        );
        Some(settle(verdict(check, run.exit, &run.output), recorded))
    }

    /// Runs the kind's sign-out on an account's home, when declared and the home exists. Its failure is only logged.
    fn sign_out(&self, decl: &KindDecl, program: &Path, kind: &str, account: &str) {
        let Some(logout) = decl
            .accounts
            .as_ref()
            .and_then(|accounts| accounts.logout.clone())
        else {
            return;
        };
        let Some(credential_decl) = decl.credentials.as_ref() else {
            return;
        };
        let Ok(Some(home)) = credentials::usable_account_home(self.profile_dir(), kind, account)
        else {
            return;
        };
        let run = run_program(
            program,
            &logout,
            &home,
            &credential_decl.home_env,
            &credential_decl.strip_env,
        );
        if run.exit != Some(0) {
            tracing::warn!(
                kind,
                "the account's sign-out did not finish cleanly; its home is deleted anyway"
            );
        }
    }

    /// Deletes an account that is not kept: its home and its row. A failure is logged, never returned.
    pub(crate) fn discard_account(&self, kind: &str, account: &str) {
        if let Err(error) = credentials::remove_account(self.profile_dir(), kind, account) {
            tracing::warn!(
                kind,
                code = error.code,
                "a discarded account's home could not be deleted"
            );
        }
        if self.store().delete_agent_account(account).is_err() {
            tracing::warn!(kind, "a discarded account's row could not be deleted");
        }
    }

    pub(crate) fn make_active_if_none(&self, kind: &str, account: &str) {
        if matches!(self.store().get_active_agent_account(kind), Ok(None)) {
            let _ = self.store().set_active_agent_account(kind, account);
        }
    }

    pub(crate) fn notify_account(&self, kind: &str, account: &str, state: AccountState) {
        self.notify(Change::Account(AccountsChanged {
            kind: kind.to_owned(),
            account: account.to_owned(),
            state,
        }));
    }
}

/// Whether an import failed only because none of the declared files is in the source: every reason is `missing`.
fn all_missing(error: &AgentError) -> bool {
    error
        .data
        .as_ref()
        .and_then(|data| data.get("files"))
        .and_then(|files| files.as_array())
        .is_some_and(|files| {
            !files.is_empty() && files.iter().all(|file| file["reason"] == "missing")
        })
}
