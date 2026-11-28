//! Accounts of a kind (decision 0059): several credential homes per kind, one active per kind, per-project choices,
//! sign-in through a service-owned terminal, import, checks, sign-out and removal. The files of an account follow the
//! rules of 0056 (see [`crate::credentials`]); this module decides which account a run uses and what its state is.
//!
//! Lock order, everywhere: the homes gate ([`credentials::lock_homes`]), then the account busy set ([`AccountBusy`]),
//! then the status lock of live runs. A sign-out or a check runs outside the gate, so a hung program cannot hold it.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::MutexGuard;
use std::thread;

use massorca_core_store::{AgentAccountRow, StoreError, new_id, now_ms};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    AccountOrigin, AccountState, AccountSummary, AccountsChanged, AccountsImported,
    AccountsRemoved, CredentialsImported, CredentialsRemoved, CredentialsShown, ImportResult,
    PackageState,
};

use crate::account_check::{
    CheckOutcome, MAX_TEXT_CHARS, Verdict, account_strip_names, normalized_identity, outcome,
    parse_state, run_program, state_name, unknown,
};
use crate::credentials::{self, ImportFrom};
use crate::kind::{KindDecl, valid_id};
use crate::packages::{BuiltInPackages, FIRST_PARTY_PREFIX, KindPackages};
use crate::paths;
use crate::sessions::{AgentError, Agents, Change};
use crate::sign_in::{SignInEnd, SignInOutcome, sign_in_outcome};
use crate::state_file;

/// The label of an account when the person gives none.
pub(crate) const DEFAULT_LABEL: &str = "account";
/// The label of an account made from 0056's single home, or adopted from a home that has no row.
const MIGRATED_LABEL: &str = "default";
/// An account whose sign-in the service ran: removing it runs the kind's sign-out (decision 0059).
pub(crate) const ORIGIN_SIGN_IN: &str = "sign_in";
/// An account whose files were imported: removing it only deletes them, the login stays the person's.
pub(crate) const ORIGIN_IMPORT: &str = "import";
/// An account made from 0056's home, or adopted at startup: removing it only deletes its files.
pub(crate) const ORIGIN_MIGRATED: &str = "migrated";

/// Which account a run uses, and its home (decision 0059). Both are absent when the kind has no account, and the agent
/// then uses its usual home, as in 0056.
#[derive(Debug, Default)]
pub struct AccountUse {
    pub home: Option<PathBuf>,
    pub account: Option<String>,
}

/// The accounts a service call is working on, under the busy lock: sign-ins still open (account to its terminal), accounts
/// being decided (empty: taken before a terminal exists, while a decision runs, or for an import), and accounts being
/// removed.
#[derive(Debug, Default)]
pub(crate) struct AccountBusy {
    pub(crate) pending: HashMap<String, String>,
    pub(crate) removing: HashSet<String>,
}

/// What a decision did with a new account its check reports signed in (decisions 0059 and 0098).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Kept {
    /// Kept as a new account, or nothing else to report.
    New,
    /// The login was already this account: it now has the new files, and the new account is gone.
    Updated(String),
    /// The login is already this account, which is busy; the new account is deleted and that account is unchanged.
    Duplicate(String),
    /// The login is already this account, whose files could not be replaced; the new account is deleted.
    NotUpdated { existing: String, message: String },
}

/// Marks an account as being removed until dropped, so no run starts on it and no other change touches it.
pub(crate) struct RemovalMark {
    agents: Agents,
    account: String,
}

impl Drop for RemovalMark {
    fn drop(&mut self) {
        self.agents.account_busy().removing.remove(&self.account);
    }
}

/// Marks an account as being decided until dropped (decision 0059): no check, activation, removal or other decision of it
/// runs meanwhile. The mark is dropped once the outcome is stored, or the account is signed out and deleted.
pub(crate) struct DecisionMark {
    agents: Agents,
    account: String,
}

impl Drop for DecisionMark {
    fn drop(&mut self) {
        self.agents.account_busy().pending.remove(&self.account);
    }
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

fn busy(account: &str, why: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_BUSY,
        format!("account {account} is {why}; wait for it, or stop the live session that uses it"),
    )
}

/// An account another call holds: a sign-in still open (`signing in`), or one being decided (`being decided`).
fn pending_busy(account: &str, mark: &str) -> AgentError {
    busy(
        account,
        if mark.is_empty() {
            "being decided"
        } else {
            "signing in"
        },
    )
}

/// An account whose sign-in no check has decided yet: it cannot be activated until a check decides it.
fn undecided(account: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_BUSY,
        format!("account {account} has a sign-in that is not decided yet; check it, or remove it"),
    )
}

/// Repairs the state file of an account home before a launch (decision 0099), under the homes gate. A state file that
/// cannot be repaired refuses the launch, as the check reports the account as needing a sign-in: the CLI would open its
/// first-run setup instead.
pub(crate) fn repair_state(decl: &KindDecl, home: &Path) -> Result<(), AgentError> {
    let Some(rule) = decl
        .credentials
        .as_ref()
        .and_then(|credentials| credentials.state.as_ref())
    else {
        return Ok(());
    };
    state_file::ensure(home, rule).map(|_| ()).map_err(|reason| {
        AgentError::new(
            codes::AGENT_CREDENTIALS_INVALID,
            format!(
                "the account's {} for agent kind {} cannot be used ({reason}); delete that file in the account's folder, or remove the account and add it again",
                rule.file,
                decl.id
            ),
        )
    })
}

fn already_has_home(kind: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_CREDENTIALS_EXIST,
        format!("agent kind {kind} already has an active account; import with replace to swap it"),
    )
}

/// A label is at most [`MAX_TEXT_CHARS`] characters with no control character; anything else is an invalid request.
pub(crate) fn valid_label(label: &str) -> Result<(), AgentError> {
    if label.chars().count() <= MAX_TEXT_CHARS && !label.chars().any(char::is_control) {
        return Ok(());
    }
    Err(AgentError::new(
        codes::REQUEST_INVALID_PARAMS,
        format!(
            "an account label is at most {MAX_TEXT_CHARS} characters and holds no control character"
        ),
    ))
}

/// The protocol's origin for a stored origin name; a name this build does not know reads as `unknown`.
pub(crate) fn origin_of(stored: &str) -> AccountOrigin {
    match stored {
        ORIGIN_SIGN_IN => AccountOrigin::SignIn,
        ORIGIN_IMPORT => AccountOrigin::Import,
        ORIGIN_MIGRATED => AccountOrigin::Migrated,
        _ => AccountOrigin::Unknown,
    }
}

/// The state a refused sign-in or import is announced with: the state its check found, or `needs_sign_in` when no check
/// judged it, since no login is left to keep.
pub(crate) fn refused_state(verdict: Option<&Verdict>) -> AccountState {
    match verdict {
        Some(found) if found.state != AccountState::Unknown => found.state,
        _ => AccountState::NeedsSignIn,
    }
}

impl Agents {
    /// The accounts of a kind, oldest first, with the active one and the projects that chose each. A read: it never moves
    /// a credential home (that happens at startup, and at a start or resume).
    pub fn accounts_list(&self, kind: &str) -> Result<Vec<AccountSummary>, AgentError> {
        if !valid_id(kind) {
            return Err(kind_not_found(kind));
        }
        let active = self.store().get_active_agent_account(kind)?;
        self.store()
            .list_agent_accounts(kind)?
            .iter()
            .map(|row| self.summary(row, active.as_deref()))
            .collect()
    }

    /// Imports the kind's declared files from `from` into a new account (decision 0059), then decides it with its check. The
    /// account stays undecided until the check keeps it, so a refused import never counts as the kind's history. The result
    /// is `imported`; `failed` when the check reports the login is not signed in (the new account is then deleted, without a
    /// sign-out: the login is the person's); or `not_found` when a required file is missing, with a reason naming each one and
    /// where it was looked for. Without `from`, each declared file is read from where the kind's CLI keeps it (decision
    /// 0097).
    pub fn accounts_import(
        &self,
        kind: &str,
        from: Option<&str>,
        label: Option<String>,
    ) -> Result<AccountsImported, AgentError> {
        let label = label.unwrap_or_else(|| DEFAULT_LABEL.to_owned());
        valid_label(&label)?;
        self.require_enabled_kind(kind)?;
        let from = match from {
            Some(folder) => ImportFrom::Folder(Path::new(folder)),
            None => ImportFrom::Detected,
        };
        let (account, imported, mark) =
            match self.import_new_account(kind, from, &label, ORIGIN_IMPORT) {
                Ok(added) => added,
                Err(error) if error.code == codes::AGENT_CREDENTIALS_UNSUPPORTED => {
                    return Err(error);
                }
                Err(error) if all_missing(&error) => {
                    return Ok(AccountsImported {
                        result: ImportResult::NotFound,
                        account: None,
                        reason: Some(error.message),
                        updated: None,
                    });
                }
                Err(error) => return Ok(failed_import(error.message)),
            };
        let checked = self.check_imported(kind, &account);
        let timed_out = checked.as_ref().is_some_and(CheckOutcome::timed_out);
        let verdict = checked.map(CheckOutcome::verdict);
        let files = credentials::account_files_present(
            self.profile_dir(),
            self.kind_packages(),
            kind,
            &account,
        )
        .unwrap_or(false);
        let refused = refused_state(verdict.as_ref());
        match sign_in_outcome(verdict.as_ref(), files, SignInEnd::TerminalEnded) {
            SignInOutcome::Discard => {
                self.discard_marked(kind, &account, refused);
                return Ok(failed_import(
                    "the check reports the login is not signed in".to_owned(),
                ));
            }
            outcome => match self.store_outcome(kind, &account, outcome, mark, None, refused) {
                // A check stopped at its time limit kept the account as `unknown`; a retry decides it later.
                Kept::New if timed_out => self.schedule_check_retry(kind, &account),
                Kept::New => {}
                Kept::Updated(existing) => {
                    let row = self.account_of(kind, &existing)?;
                    let summary = self.summary(
                        &row,
                        self.store().get_active_agent_account(kind)?.as_deref(),
                    )?;
                    return Ok(AccountsImported {
                        result: ImportResult::Imported,
                        account: Some(summary),
                        reason: None,
                        updated: Some(existing),
                    });
                }
                Kept::Duplicate(existing) => {
                    return Ok(failed_import(format!(
                        "this login is already account {existing} of {kind}, which is busy; try again once it is free"
                    )));
                }
                Kept::NotUpdated { existing, message } => {
                    return Ok(failed_import(format!(
                        "this login is already account {existing} of {kind}, whose files could not be replaced: {message}"
                    )));
                }
            },
        }
        let row = self.account_of(kind, &account)?;
        let summary = self.summary(
            &row,
            self.store().get_active_agent_account(kind)?.as_deref(),
        )?;
        // A state file built without its source says so (decision 0099); the import still succeeded.
        let notes: Vec<String> = imported
            .files
            .into_iter()
            .filter_map(|file| file.reason)
            .collect();
        Ok(AccountsImported {
            result: ImportResult::Imported,
            account: Some(summary),
            reason: (!notes.is_empty()).then(|| notes.join("; ")),
            updated: None,
        })
    }

    /// Makes an account the kind's active one, or the project's choice for `project` (decision 0059). An account that is
    /// being removed, or whose sign-in is still open, being decided, or not decided yet, is refused with `agent.busy`; one
    /// that never signed in is refused. The homes gate is held from the checks to the write, so a removal cannot delete the
    /// row in between.
    pub fn accounts_activate(
        &self,
        kind: &str,
        account: &str,
        project: Option<&str>,
    ) -> Result<AccountSummary, AgentError> {
        self.require_enabled_kind(kind)?;
        let summary = self.activate_under_gate(kind, account, project)?;
        // Announced once, after the gate is released: the account made active, or chosen by the project (decision 0059).
        self.notify_account(kind, account, summary.state);
        Ok(summary)
    }

    /// The body of [`Agents::accounts_activate`], run under the homes gate so its checks and its write cannot be split.
    fn activate_under_gate(
        &self,
        kind: &str,
        account: &str,
        project: Option<&str>,
    ) -> Result<AccountSummary, AgentError> {
        let _gate = credentials::lock_homes();
        let row = self.account_of(kind, account)?;
        {
            let busy_set = self.account_busy();
            if busy_set.removing.contains(account) {
                return Err(busy(account, "being removed"));
            }
            if let Some(mark) = busy_set.pending.get(account) {
                return Err(pending_busy(account, mark));
            }
        }
        // A sign-in or import an earlier service left undecided is marked only in the store, so the store decides this too.
        if self.store_pending(account)? {
            return Err(undecided(account));
        }
        if parse_state(&row.state) == AccountState::NeedsSignIn {
            return Err(AgentError::new(
                codes::AGENT_CREDENTIALS_INVALID,
                format!("account {account} has not signed in; sign it in before it is activated"),
            ));
        }
        match project {
            None => {
                self.store().set_active_agent_account(kind, account)?;
                self.forget_listings(kind, account);
            }
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

    /// Deletes an account (decision 0059), in three steps: under the gate it refuses a busy account (a live run, a sign-in
    /// still open, or a removal already running) and marks it as being removed; outside the gate the kind's sign-out runs,
    /// only for a sign-in the service made (an imported login is the person's and is never revoked here); under the gate
    /// again the home and the row are deleted. A failed sign-out does not keep the home. Nothing becomes active afterwards.
    pub fn accounts_remove(
        &self,
        kind: &str,
        account: &str,
    ) -> Result<AccountsRemoved, AgentError> {
        let row = self.account_of(kind, account)?;
        let mark = {
            let _gate = credentials::lock_homes();
            self.begin_removal(account)?
        };
        self.cancel_check_retry(account);
        if row.origin == ORIGIN_SIGN_IN
            && let Ok((decl, program)) = self.runnable_kind(kind)
        {
            self.sign_out(&decl, &program, kind, account);
        }
        {
            let _gate = credentials::lock_homes();
            credentials::remove_account_locked(self.profile_dir(), kind, account)?;
            self.store().delete_agent_account(account)?;
        }
        drop(mark);
        self.forget_listings(kind, account);
        // Announced once, with the state the account had when it was removed (decision 0059).
        self.notify_account(kind, account, parse_state(&row.state));
        Ok(AccountsRemoved {
            kind: kind.to_owned(),
            account: account.to_owned(),
            removed: true,
        })
    }

    /// Marks `account` as being removed, under the gate. Refused while it is busy. The mark lasts until the returned guard
    /// is dropped.
    fn begin_removal(&self, account: &str) -> Result<RemovalMark, AgentError> {
        {
            let mut busy_set = self.account_busy();
            if busy_set.removing.contains(account) {
                return Err(busy(account, "already being removed"));
            }
            if let Some(mark) = busy_set.pending.get(account) {
                return Err(pending_busy(account, mark));
            }
            if self.used_by_live_run(account) {
                return Err(busy(account, "in use by a live session"));
            }
            busy_set.removing.insert(account.to_owned());
        }
        Ok(RemovalMark {
            agents: self.clone(),
            account: account.to_owned(),
        })
    }

    /// Whether a live run that has not exited uses `account`. A removal and a sign-in again both refuse it: a login rewrites
    /// the files the running agent reads (decision 0059).
    fn used_by_live_run(&self, account: &str) -> bool {
        self.live_runs()
            .values()
            .any(|run| run.account.as_deref() == Some(account))
    }

    /// Marks an existing account as signing in again (decision 0059), and returns its summary and its home for the login.
    /// Under the homes gate, with the account busy set held across the checks and the mark, so no other call takes the
    /// account in between. Refused with `agent.busy` while it is being removed, signing in, or used by a live run, and with
    /// `agent.busy` while its sign-in or import is undecided in the store. A home that is gone is refused, since the login
    /// has nowhere to write. The mark is in memory only, so the store's row is not touched (see
    /// [`Agents::accounts_sign_in`]).
    pub(crate) fn begin_sign_in_again(
        &self,
        kind: &str,
        account: &str,
    ) -> Result<(AccountSummary, PathBuf), AgentError> {
        let _gate = credentials::lock_homes();
        let row = self.account_of(kind, account)?;
        let mut busy_set = self.account_busy();
        if busy_set.removing.contains(account) {
            return Err(busy(account, "being removed"));
        }
        if let Some(mark) = busy_set.pending.get(account) {
            return Err(pending_busy(account, mark));
        }
        if self.used_by_live_run(account) {
            return Err(busy(account, "in use by a live session"));
        }
        if self.store_pending(account)? {
            return Err(undecided(account));
        }
        let home = credentials::usable_account_home(self.profile_dir(), kind, account)?
            .ok_or_else(|| {
                AgentError::new(
                    codes::AGENT_CREDENTIALS_INVALID,
                    format!(
                        "account {account} of agent kind {kind} has no credential home; remove it and add the account again"
                    ),
                )
            })?;
        let active = self.store().get_active_agent_account(kind)?;
        let summary = self.summary(&row, active.as_deref())?;
        busy_set.pending.insert(account.to_owned(), String::new());
        Ok((summary, home))
    }

    /// Runs the kind's check on an account now, or on the active one when `account` is absent (decision 0059). An account
    /// whose sign-in is open, or being decided, is busy. An account whose sign-in or import is still undecided in the store
    /// is decided by the check, as a sign-in's end decides it; any other account keeps what its check finds. A check
    /// stopped at its time limit decides nothing: the account keeps its last known state and the check is retried in the
    /// background (decision 0059, amended 2026-11-27). This check supersedes a retry already waiting, once it is let in: a
    /// check refused as busy leaves a running retry, and the verdict it brings, alone.
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
        let (summary, timed_out) = self.check_once(kind, &id, None)?;
        if timed_out {
            self.schedule_check_retry(kind, &id);
        }
        Ok(summary)
    }

    /// One retry of a timed-out check of `account` (decision 0059, amended 2026-11-27), applied as an explicit check applies
    /// its result. `Ok(true)` when it timed out again. Refused when the retry was superseded or cancelled, or the account
    /// is gone, busy or being removed.
    pub(crate) fn check_for_retry(
        &self,
        kind: &str,
        account: &str,
        generation: u64,
    ) -> Result<bool, AgentError> {
        self.check_once(kind, account, Some(generation))
            .map(|(_, timed_out)| timed_out)
    }

    /// The body of a check of `id`, explicit or a retry of generation `retry`: its summary afterwards, and whether the check
    /// timed out. A timeout stores nothing. A retry stores its result only while it is still the retry wanted and the
    /// account is not being removed. An explicit check cancels a retry only once its busy checks let it in.
    fn check_once(
        &self,
        kind: &str,
        id: &str,
        retry: Option<u64>,
    ) -> Result<(AccountSummary, bool), AgentError> {
        let row = self.account_of(kind, id)?;
        if let Some(mark) = self.account_busy().pending.get(id) {
            return Err(pending_busy(id, mark));
        }
        if self.store_pending(id)? {
            return self.check_undecided(kind, id, retry);
        }
        if retry.is_none() {
            self.cancel_check_retry(id);
        }
        let (decl, program) = self.runnable_kind(kind)?;
        let checked = self.check_verdict(&decl, &program, id, row.identity.as_deref());
        let timed_out = checked.timed_out();
        if !timed_out {
            let state = checked.verdict();
            let found = {
                let _gate = credentials::lock_homes();
                self.retry_still_wanted(id, retry)?;
                self.store().set_agent_account_check(
                    id,
                    state_name(state.state),
                    state.identity.as_deref(),
                )?
            };
            if found && parse_state(&row.state) != state.state {
                self.notify_account(kind, id, state.state);
            }
        }
        let row = self.account_of(kind, id)?;
        let summary = self.summary(
            &row,
            self.store().get_active_agent_account(kind)?.as_deref(),
        )?;
        Ok((summary, timed_out))
    }

    /// Refuses a retry that is no longer wanted: superseded or cancelled, or of an account being removed. An explicit check
    /// (`retry` absent) is always wanted.
    fn retry_still_wanted(&self, account: &str, retry: Option<u64>) -> Result<(), AgentError> {
        let Some(generation) = retry else {
            return Ok(());
        };
        if self.account_busy().removing.contains(account) {
            return Err(busy(account, "being removed"));
        }
        if !self.retry_is_current(account, generation) {
            return Err(busy(account, "checked again"));
        }
        Ok(())
    }

    /// The outcome of an account's check as a check command reports it: `unknown` when the kind declares no check, and
    /// `needs_sign_in` when the check cannot run on the account's home.
    pub(crate) fn check_verdict(
        &self,
        decl: &KindDecl,
        program: &Path,
        account: &str,
        recorded: Option<&str>,
    ) -> CheckOutcome {
        let declares_check = decl
            .accounts
            .as_ref()
            .and_then(|accounts| accounts.check.as_ref())
            .is_some();
        if !declares_check {
            return CheckOutcome::Decided(unknown());
        }
        self.check_account_home(decl, program, account, recorded)
            .unwrap_or(CheckOutcome::Decided(Verdict {
                state: AccountState::NeedsSignIn,
                identity: None,
            }))
    }

    /// Checks an account whose sign-in or import is still undecided in the store (decision 0059). The check decides it as a
    /// sign-in's end does: signed in keeps it; a check that cannot tell keeps it as `unknown` while its files exist; anything
    /// else signs a sign-in out and deletes it (an import is only deleted). A check stopped at its time limit decides
    /// nothing, so the account stays undecided (decision 0059, amended 2026-11-27). The mark is held meanwhile, so no other
    /// call decides the account too.
    fn check_undecided(
        &self,
        kind: &str,
        account: &str,
        retry: Option<u64>,
    ) -> Result<(AccountSummary, bool), AgentError> {
        let mark = self.mark_for_decision(account)?;
        // The row may have been decided between the first look and the mark: then it is an ordinary account again.
        if !self.store_pending(account)? {
            drop(mark);
            return self.check_once(kind, account, retry);
        }
        // Let in: a retry running for the account would hold the mark, so only a waiting one is cancelled.
        if retry.is_none() {
            self.cancel_check_retry(account);
        }
        let row = self.account_of(kind, account)?;
        let (decl, program) = self.runnable_kind(kind)?;
        let checked = self.check_verdict(&decl, &program, account, row.identity.as_deref());
        if checked.timed_out() {
            drop(mark);
            let active = self.store().get_active_agent_account(kind)?;
            return Ok((self.summary(&row, active.as_deref())?, true));
        }
        if let Err(error) = self.retry_still_wanted(account, retry) {
            drop(mark);
            return Err(error);
        }
        let verdict = checked.verdict();
        let files = credentials::account_files_present(
            self.profile_dir(),
            self.kind_packages(),
            kind,
            account,
        )
        .unwrap_or(false);
        let outcome = sign_in_outcome(Some(&verdict), files, SignInEnd::TerminalEnded);
        let refused = refused_state(Some(&verdict));
        let sign_out = (row.origin == ORIGIN_SIGN_IN).then_some((decl, program));
        self.store_outcome(kind, account, outcome, mark, sign_out, refused);
        match self.store().get_agent_account(account)? {
            Some(row) if row.kind == kind => {
                let active = self.store().get_active_agent_account(kind)?;
                Ok((self.summary(&row, active.as_deref())?, false))
            }
            _ => Err(AgentError::new(
                codes::AGENT_ACCOUNT_NOT_FOUND,
                format!(
                    "account {account} of agent kind {kind} was signed out and deleted: its sign-in was not kept"
                ),
            )),
        }
    }

    /// Marks `account` as being decided, unless another call holds it: a sign-in still open, another decision, or a removal.
    /// Refused with `agent.busy` meanwhile.
    pub(crate) fn mark_for_decision(&self, account: &str) -> Result<DecisionMark, AgentError> {
        {
            let mut busy_set = self.account_busy();
            if busy_set.removing.contains(account) {
                return Err(busy(account, "being removed"));
            }
            if let Some(mark) = busy_set.pending.get(account) {
                return Err(pending_busy(account, mark));
            }
            busy_set.pending.insert(account.to_owned(), String::new());
        }
        Ok(DecisionMark {
            agents: self.clone(),
            account: account.to_owned(),
        })
    }

    /// Marks `account` without a check: for a row the caller has just created, or for a sign-in whose terminal has ended,
    /// which replaces the open mark (the terminal's id) by the decision's.
    pub(crate) fn mark_now(&self, account: &str) -> DecisionMark {
        self.account_busy()
            .pending
            .insert(account.to_owned(), String::new());
        DecisionMark {
            agents: self.clone(),
            account: account.to_owned(),
        }
    }

    /// Whether the store still marks the account's sign-in or import as undecided (`sign_in_pending`).
    pub(crate) fn store_pending(&self, account: &str) -> Result<bool, AgentError> {
        Ok(self
            .store()
            .list_pending_sign_ins()?
            .iter()
            .any(|row| row.id == account))
    }

    /// Stores the outcome of a decision, then drops its mark (decision 0059). A refused sign-in is signed out first, when
    /// `sign_out` names its kind's program; an import is never signed out. Each store write is made under the homes gate.
    /// `refused` is the state a refusal is announced with.
    pub(crate) fn store_outcome(
        &self,
        kind: &str,
        account: &str,
        outcome: SignInOutcome,
        mark: DecisionMark,
        sign_out: Option<(KindDecl, PathBuf)>,
        refused: AccountState,
    ) -> Kept {
        match outcome {
            SignInOutcome::Discard => {
                if let Some((decl, program)) = &sign_out {
                    self.sign_out(decl, program, kind, account);
                }
                self.discard_marked(kind, account, refused);
                self.forget_listings(kind, account);
            }
            SignInOutcome::Undecided => {
                self.leave_undecided(kind, account);
                self.forget_listings(kind, account);
            }
            SignInOutcome::Unknown => {
                self.keep_marked(kind, account, AccountState::Unknown, None);
                self.forget_listings(kind, account);
            }
            SignInOutcome::SignedIn { identity } => {
                let kept = self.keep_signed_in(kind, account, identity);
                // An import that updated an existing account changed that account's files (decision 0098).
                match &kept {
                    Kept::New => self.forget_listings(kind, account),
                    Kept::Updated(existing) => self.forget_listings(kind, existing),
                    Kept::Duplicate(existing) => self.forget_listings(kind, existing),
                    Kept::NotUpdated { existing, .. } => self.forget_listings(kind, existing),
                }
                drop(mark);
                return kept;
            }
        }
        drop(mark);
        Kept::New
    }

    /// The other account of `kind` already signed in as `identity`, if any (ACC-03). Identities are compared trimmed and
    /// without case, as pv96 compares a duplicate add's email. A new account that matches updates that account (decision
    /// 0098), and is never signed out.
    pub(crate) fn duplicate_of(
        &self,
        kind: &str,
        account: &str,
        identity: &str,
    ) -> Result<Option<String>, AgentError> {
        let wanted = normalized_identity(identity);
        if wanted.is_empty() {
            return Ok(None);
        }
        Ok(self
            .store()
            .list_agent_accounts(kind)?
            .into_iter()
            .find(|row| {
                row.id != account
                    && row
                        .identity
                        .as_deref()
                        .is_some_and(|known| normalized_identity(known) == wanted)
            })
            .map(|row| row.id))
    }

    /// Keeps a new account its check reports signed in, while the caller holds its mark (decision 0059). When another
    /// account of the kind already has its identity (ACC-03), that account is updated instead (decision 0098): the new
    /// account's declared credential files overwrite those files in its home (nothing else there changes), its state
    /// and identity are refreshed, and its id, label, origin, active mark and
    /// project choices stay; the new account's row is deleted. An existing account that is busy (a live run, a removal, a
    /// sign-in or a decision of its own, or a sign-in still undecided in the store) is left alone and the new account is
    /// deleted, as before 0098. Neither is ever signed out: the vendor sign-out would end the session both share. The
    /// comparison and the writes happen under one hold of the homes gate, so two decisions of the same login cannot both
    /// keep it.
    fn keep_signed_in(&self, kind: &str, account: &str, identity: Option<String>) -> Kept {
        let gate = credentials::lock_homes();
        let duplicate = match identity.as_deref() {
            Some(identity) => self
                .duplicate_of(kind, account, identity)
                .unwrap_or_else(|error| {
                    tracing::warn!(
                        kind,
                        code = error.code,
                        "the accounts could not be compared"
                    );
                    None
                }),
            None => None,
        };
        if let Some(existing) = duplicate {
            let busy =
                self.account_is_busy(&existing) || self.store_pending(&existing).unwrap_or(true);
            let kept = if busy {
                Kept::Duplicate(existing.clone())
            } else {
                match self.update_existing(kind, account, &existing, identity.as_deref()) {
                    Ok(()) => Kept::Updated(existing.clone()),
                    Err(error) => {
                        tracing::warn!(kind, code = error.code, "an account could not be updated");
                        Kept::NotUpdated {
                            existing: existing.clone(),
                            message: error.message,
                        }
                    }
                }
            };
            drop(gate);
            if matches!(kept, Kept::Updated(_)) {
                // The new account is gone and the existing one has its files: both are announced once.
                self.notify_account(kind, account, AccountState::SignedIn);
                self.notify_account(kind, &existing, AccountState::SignedIn);
            } else {
                tracing::warn!(
                    kind,
                    existing = %existing,
                    "a new account of a login that is already an account is discarded"
                );
                self.discard_marked(kind, account, AccountState::Conflict);
            }
            return kept;
        }
        match self.keep_decided(kind, account, AccountState::SignedIn, identity.as_deref()) {
            Ok(true) => {}
            Ok(false) => return Kept::New,
            Err(error) => {
                tracing::warn!(kind, code = %error, "a sign-in could not be recorded");
                return Kept::New;
            }
        }
        self.make_active_if_none(kind, account);
        drop(gate);
        self.notify_account(kind, account, AccountState::SignedIn);
        Kept::New
    }

    /// Gives `existing` the declared credential files of the new account `account`, leaving every other file of its home
    /// as it is, records its check (decision 0098), then deletes the new account's home and row. The caller holds the homes gate and the new account's mark, and has
    /// checked that `existing` is not busy. A failed swap leaves `existing` as it was.
    fn update_existing(
        &self,
        kind: &str,
        account: &str,
        existing: &str,
        identity: Option<&str>,
    ) -> Result<(), AgentError> {
        credentials::update_account_files_locked(
            self.profile_dir(),
            self.kind_packages(),
            kind,
            account,
            existing,
        )?;
        self.store().set_agent_account_check(
            existing,
            state_name(AccountState::SignedIn),
            identity,
        )?;
        if let Err(error) = self.store().delete_agent_account(account) {
            tracing::warn!(kind, code = %error, "an updating account's row could not be deleted");
        }
        tracing::info!(
            kind,
            "a second import or sign-in of an account's login updated it"
        );
        Ok(())
    }

    /// Keeps an account whose mark the caller holds: its check's state and identity are stored, its sign-in mark is cleared
    /// (so the kind's history records it), and it is made active when its kind has no active account (decision 0059).
    fn keep_marked(
        &self,
        kind: &str,
        account: &str,
        state: AccountState,
        identity: Option<String>,
    ) {
        let gate = credentials::lock_homes();
        match self.keep_decided(kind, account, state, identity.as_deref()) {
            Ok(true) => {}
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

    /// Deletes an account a decision has refused, while the caller holds its mark: its home and its row, under the gate. The
    /// refusal is announced once, with `last_state`: the state its check found.
    fn discard_marked(&self, kind: &str, account: &str, last_state: AccountState) {
        let deleted = {
            let _gate = credentials::lock_homes();
            if let Err(error) =
                credentials::remove_account_locked(self.profile_dir(), kind, account)
            {
                tracing::warn!(
                    kind,
                    code = error.code,
                    "a discarded account's home could not be deleted"
                );
            }
            let row = self.store().delete_agent_account(account);
            if row.is_err() {
                tracing::warn!(kind, "a discarded account's row could not be deleted");
            }
            row.is_ok()
        };
        if deleted {
            self.notify_account(kind, account, last_state);
        }
    }

    /// Leaves an account the startup sweep cannot decide: its state is `unknown` and its mark is not kept, so it stays pending
    /// until a check decides it (decision 0059).
    fn leave_undecided(&self, kind: &str, account: &str) {
        let gate = credentials::lock_homes();
        match self
            .store()
            .set_agent_account_check(account, state_name(AccountState::Unknown), None)
        {
            Ok(true) => {}
            Ok(false) => return,
            Err(error) => {
                tracing::warn!(kind, code = %error, "a sign-in could not be recorded");
                return;
            }
        }
        drop(gate);
        self.notify_account(kind, account, AccountState::Unknown);
    }

    /// The account a run of `kind` in `project` uses, and its home (decision 0059): the project's choice, else the active
    /// account, else none. The home is taken only when it is a plain directory (0056). The caller holds `lock_homes`, so
    /// the home cannot be swapped before the run is registered. An account being removed is refused.
    pub(crate) fn resolve_home(
        &self,
        gate: &MutexGuard<'_, ()>,
        decl: &KindDecl,
        project: &str,
    ) -> Result<AccountUse, AgentError> {
        let kind = decl.id.as_str();
        self.migrate_legacy(gate, kind)?;
        let chosen = match self.store().get_agent_account_choice(kind, project)? {
            Some(account) => Some(account),
            None => self.store().get_active_agent_account(kind)?,
        };
        let Some(account) = chosen else {
            // A kind that had an account, and has none now, is not sent back to the vendor's usual home: its login may be
            // the person's own (decision 0059). Only a kind that never had an account keeps the usual home.
            if self.store().kind_has_had_account(kind)? {
                return Err(AgentError::new(
                    codes::AGENT_CREDENTIALS_INVALID,
                    format!("the active account for {kind} was removed; choose or add an account"),
                ));
            }
            return Ok(AccountUse::default());
        };
        if self.account_busy().removing.contains(&account) {
            return Err(busy(&account, "being removed"));
        }
        let home = credentials::usable_account_home(self.profile_dir(), kind, &account)?
            .ok_or_else(|| {
                AgentError::new(
                    codes::AGENT_CREDENTIALS_INVALID,
                    format!(
                        "the account of agent kind {kind} has no credential home; sign in again or remove it"
                    ),
                )
            })?;
        repair_state(decl, &home)?;
        Ok(AccountUse {
            home: Some(home),
            account: Some(account),
        })
    }

    /// Whether the kind's active account has a usable home (decision 0059), for the kinds listing. A read: no migration.
    pub(crate) fn has_usable_account(&self, kind: &str) -> bool {
        let Ok(Some(account)) = self.store().get_active_agent_account(kind) else {
            return false;
        };
        let _gate = credentials::lock_homes();
        matches!(
            credentials::usable_account_home(self.profile_dir(), kind, &account),
            Ok(Some(_))
        )
    }

    /// The cleanup of a package removed with its agent homes (decision 0098): for each kind the package provided, deletes
    /// every account of the kind (its home under `<profile>/agents/accounts/<kind>/` and its row, which takes its active
    /// mark and project choices with it), any account directory left without a row, the hidden folders an interrupted
    /// import or replace left there, the kind's account folder itself, and the kind's 0056 home with its hidden siblings.
    /// Nothing is signed out, so an account a MassOrca sign-in made keeps its login valid at the vendor. Left alone: a kind
    /// another installed package still provides; a kind the built-in catalogue declares, when the package is not a
    /// first-party one (reported); and an account busy with a sign-in, a decision, a removal or a live run (reported).
    /// Links are refused as by every removal. Every kind is tried; what was left is returned as one error, with the code
    /// of its first problem.
    pub fn remove_package_accounts(
        &self,
        package_id: &str,
        kinds: &[String],
    ) -> Result<(), AgentError> {
        let still_provided: HashSet<String> = self
            .kind_packages()
            .kinds()
            .into_iter()
            // An unknown state is a lockfile that cannot be read: the kind may still be installed, so it is kept.
            .filter(|kind| {
                kind.package_id != package_id
                    && matches!(
                        kind.state,
                        PackageState::Enabled | PackageState::Disabled | PackageState::Unknown
                    )
            })
            .map(|kind| kind.kind_id)
            .collect();
        let built_in: HashSet<String> = BuiltInPackages
            .kinds()
            .into_iter()
            .map(|kind| kind.kind_id)
            .collect();
        let first_party = package_id.starts_with(FIRST_PARTY_PREFIX);
        let mut problems: Vec<AgentError> = Vec::new();
        for kind in kinds {
            if !valid_id(kind) {
                continue;
            }
            if !first_party && built_in.contains(kind) {
                problems.push(AgentError::new(
                    codes::AGENT_HOOK_REFUSED,
                    format!(
                        "the accounts of {kind} were left: it is a built-in agent kind, which package {package_id} does not own"
                    ),
                ));
                continue;
            }
            if still_provided.contains(kind) {
                continue;
            }
            let removed = {
                let _gate = credentials::lock_homes();
                self.remove_kind_accounts_locked(kind, &mut problems)
            };
            for (account, state) in removed {
                self.notify_account(kind, &account, state);
            }
            self.model_cache().invalidate_kind(kind);
        }
        let Some(first) = problems.first() else {
            return Ok(());
        };
        let messages: Vec<&str> = problems
            .iter()
            .map(|problem| problem.message.as_str())
            .collect();
        Err(AgentError::new(
            first.code,
            format!(
                "some accounts of package {package_id} were not deleted: {}",
                messages.join("; ")
            ),
        ))
    }

    /// The body of [`Agents::remove_package_accounts`] for one kind, under the homes gate. Returns the deleted accounts with
    /// their last state, to be announced once the gate is released; adds what it could not delete to `problems`.
    fn remove_kind_accounts_locked(
        &self,
        kind: &str,
        problems: &mut Vec<AgentError>,
    ) -> Vec<(String, AccountState)> {
        let mut removed = Vec::new();
        let named = |error: AgentError, what: String| {
            AgentError::new(error.code, format!("{what}: {}", error.message))
        };
        let rows = match self.store().list_agent_accounts(kind) {
            Ok(rows) => rows,
            Err(error) => {
                problems.push(named(error.into(), format!("the accounts of {kind}")));
                return removed;
            }
        };
        let mut with_row = HashSet::new();
        for row in rows {
            with_row.insert(row.id.clone());
            if self.account_is_busy(&row.id) {
                problems.push(busy(&row.id, "busy"));
                continue;
            }
            if let Err(error) =
                credentials::remove_account_locked(self.profile_dir(), kind, &row.id)
            {
                problems.push(named(error, format!("account {} of {kind}", row.id)));
                continue;
            }
            match self.store().delete_agent_account(&row.id) {
                Ok(_) => {
                    self.cancel_check_retry(&row.id);
                    removed.push((row.id.clone(), parse_state(&row.state)));
                }
                Err(error) => {
                    problems.push(named(error.into(), format!("account {} of {kind}", row.id)));
                }
            }
        }
        for account in self.account_dirs(kind) {
            if with_row.contains(&account) || self.account_is_busy(&account) {
                continue;
            }
            if let Err(error) =
                credentials::remove_account_locked(self.profile_dir(), kind, &account)
            {
                problems.push(named(
                    error,
                    format!("account directory {account} of {kind}"),
                ));
            }
        }
        if let Err(error) = credentials::remove_locked(self.profile_dir(), kind) {
            problems.push(named(error, format!("the credential home of {kind}")));
        }
        problems.extend(credentials::remove_kind_leftovers_locked(
            self.profile_dir(),
            kind,
        ));
        removed
    }

    /// `agent.credentials.import` (deprecated, 0056): imports into a new account that becomes the active one. The active
    /// account is refused unless `replace` is set, and is then removed once the new one is in place. This import checks no
    /// login, so the new account is kept as it was imported: decided, as `unknown`.
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
        let (account, imported, mark) = self.import_new_account(
            kind,
            ImportFrom::Folder(Path::new(from)),
            DEFAULT_LABEL,
            ORIGIN_IMPORT,
        )?;
        if let Some(old) = active
            && let Err(error) = self.accounts_remove(kind, &old)
        {
            self.discard_marked(kind, &account, AccountState::Unknown);
            return Err(error);
        }
        self.keep_marked(kind, &account, AccountState::Unknown, None);
        drop(mark);
        self.store().set_active_agent_account(kind, &account)?;
        self.forget_listings(kind, &account);
        Ok(imported)
    }

    /// `agent.credentials.show` (deprecated, 0056): the kind's active account's files, or the empty shape when it has none.
    /// A read: no migration.
    pub fn credentials_show(&self, kind: &str) -> Result<CredentialsShown, AgentError> {
        match self.store().get_active_agent_account(kind)? {
            Some(account) => {
                credentials::show_account(self.profile_dir(), self.kind_packages(), kind, &account)
            }
            None => credentials::show(self.profile_dir(), self.kind_packages(), kind),
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

    /// The service's start-up pass over credential homes (decision 0059), run once before requests are served:
    ///
    /// - a `migrated` row whose home is gone is deleted (a crash between recording the row and moving the home);
    /// - 0056's single home of each kind is moved into an account;
    /// - an account directory with no row is adopted as `migrated`, active only if its kind has no active account;
    /// - each sign-in or import left undecided by an earlier service is decided by [`Agents::decide_sign_in`], on a thread
    ///   of its own; a check that cannot tell leaves a sign-in pending.
    pub fn startup_sweep(&self) {
        let profile = self.profile_dir().to_path_buf();
        // A crash during a replace may have left a home aside: it is put back, or deleted once its swap finished (0098).
        {
            let _gate = credentials::lock_homes();
            credentials::restore_replaced_leftovers_locked(&profile);
        }
        match self.store().list_all_agent_accounts() {
            Ok(rows) => {
                for row in rows.iter().filter(|row| row.origin == ORIGIN_MIGRATED) {
                    let gone = matches!(
                        credentials::usable_account_home(&profile, &row.kind, &row.id),
                        Ok(None)
                    );
                    // Only a crash leftover: the 0056 home is still there to be moved again. A migrated home lost by hand
                    // keeps its row, and the kind refuses to start until its account is removed or replaced.
                    let legacy_present =
                        matches!(credentials::usable_home(&profile, &row.kind), Ok(Some(_)));
                    if gone && legacy_present {
                        tracing::warn!(kind = %row.kind, "a migrated account has no home; its row is deleted");
                        if self.store().delete_agent_account(&row.id).is_ok() {
                            self.notify_account(&row.kind, &row.id, parse_state(&row.state));
                        }
                    }
                }
            }
            Err(error) => {
                tracing::error!(code = %error, "the account rows could not be read at startup")
            }
        }
        if let Ok(homes) = fs::read_dir(paths::agents_dir(&profile).join("homes")) {
            for kind in homes
                .flatten()
                .filter_map(|entry| entry.file_name().into_string().ok())
            {
                let gate = credentials::lock_homes();
                if let Err(error) = self.migrate_legacy(&gate, &kind) {
                    tracing::error!(kind = %kind, code = error.code, "the credential home could not be moved");
                }
            }
        }
        if let Ok(kinds) = fs::read_dir(paths::agents_dir(&profile).join("accounts")) {
            for kind in kinds
                .flatten()
                .filter_map(|entry| entry.file_name().into_string().ok())
            {
                for account in self.account_dirs(&kind) {
                    if matches!(self.store().get_agent_account(&account), Ok(None)) {
                        self.adopt_orphan(&kind, &account);
                    }
                }
            }
        }
        // Only an undecided row is decided here. A `needs_sign_in` state alone can be an imported or migrated account whose
        // check failed, and such a login is never discarded at startup (decision 0059).
        let open: Vec<(String, String, String)> = match self.store().list_pending_sign_ins() {
            Ok(rows) => rows
                .into_iter()
                .map(|row| (row.kind, row.id, row.origin))
                .collect(),
            Err(error) => {
                tracing::error!(code = %error, "the undecided sign-ins could not be read at startup");
                Vec::new()
            }
        };
        if !open.is_empty() {
            let agents = self.clone();
            thread::spawn(move || {
                for (kind, account, origin) in open {
                    // A sign-in the sweep cannot decide stays pending; an import is decided as its import would be, so it
                    // never stays pending (decision 0059).
                    let end = if origin == ORIGIN_SIGN_IN {
                        SignInEnd::StartupSweep
                    } else {
                        SignInEnd::TerminalEnded
                    };
                    agents.decide_sign_in(&kind, &account, end);
                }
            });
        }
    }

    /// The account directories of `kind` that are plain directories named by a canonical account id.
    fn account_dirs(&self, kind: &str) -> Vec<String> {
        let Ok(entries) = fs::read_dir(paths::accounts_dir(self.profile_dir(), kind)) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| paths::valid_account_id(name))
            .collect()
    }

    /// Records a directory that has no row as a `migrated` account, active only when the kind has no active account.
    fn adopt_orphan(&self, kind: &str, account: &str) {
        let _gate = credentials::lock_homes();
        let row = AgentAccountRow {
            id: account.to_owned(),
            kind: kind.to_owned(),
            label: MIGRATED_LABEL.to_owned(),
            created_at: now_ms(),
            identity: None,
            state: state_name(AccountState::Migrated).to_owned(),
            origin: ORIGIN_MIGRATED.to_owned(),
        };
        if self.store().insert_agent_account(&row).is_ok() {
            tracing::info!(kind, "adopted an account directory that had no row");
            self.make_active_if_none(kind, account);
            self.notify_account(kind, account, AccountState::Migrated);
        }
    }

    /// Moves 0056's single home into an account named `default`, as the kind's active account when it has none (decision
    /// 0059). The row is recorded first, so a crash after the move leaves a row the sweep can see; a move that fails
    /// deletes the row again. Does nothing when there is no legacy home.
    pub(crate) fn migrate_legacy(
        &self,
        _gate: &MutexGuard<'_, ()>,
        kind: &str,
    ) -> Result<(), AgentError> {
        if !valid_id(kind) || credentials::usable_home(self.profile_dir(), kind)?.is_none() {
            return Ok(());
        }
        let account = new_id();
        let had_active = self.store().get_active_agent_account(kind)?.is_some();
        self.store().insert_agent_account(&AgentAccountRow {
            id: account.clone(),
            kind: kind.to_owned(),
            label: MIGRATED_LABEL.to_owned(),
            created_at: now_ms(),
            identity: None,
            state: state_name(AccountState::Migrated).to_owned(),
            origin: ORIGIN_MIGRATED.to_owned(),
        })?;
        // The active mark is recorded before the move, so a crash after the move leaves the kind on this account, never on
        // the usual home. A move that fails takes the row, and with it the mark; the kind's history record stays, so the
        // kind refuses to start until an account is activated, and the next start tries the move again.
        if !had_active {
            self.store().set_active_agent_account(kind, &account)?;
            self.forget_listings(kind, &account);
        }
        if let Err(error) = credentials::adopt_legacy_home(self.profile_dir(), kind, &account) {
            let _ = self.store().delete_agent_account(&account);
            return Err(error);
        }
        tracing::info!(kind, "made the credential home an account");
        self.notify_account(kind, &account, AccountState::Migrated);
        Ok(())
    }

    /// Runs [`Agents::migrate_legacy`] under the gate, for a write that needs the home in place first.
    fn migrate_now(&self, kind: &str) -> Result<(), AgentError> {
        let gate = credentials::lock_homes();
        self.migrate_legacy(&gate, kind)
    }

    /// The row of an account of `kind`; an account of another kind, or none, is `agent.account_not_found`.
    pub(crate) fn account_of(
        &self,
        kind: &str,
        account: &str,
    ) -> Result<AgentAccountRow, AgentError> {
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
            origin: origin_of(&row.origin),
        })
    }

    /// Creates a new account, copies the declared files into it, and records its row undecided: the sign-in mark keeps the
    /// account out of the kind's history until a decision keeps it (decision 0059). Returns its id, the copy's record, and the
    /// mark the caller holds until it decides the account. The gate is taken by the copy itself, which is not reentrant, so
    /// the caller must not hold it.
    pub(crate) fn import_new_account(
        &self,
        kind: &str,
        from: ImportFrom<'_>,
        label: &str,
        origin: &str,
    ) -> Result<(String, CredentialsImported, DecisionMark), AgentError> {
        self.migrate_now(kind)?;
        let account = new_id();
        // Marked before the copy and the row exist, so no check of the account can take the decision first.
        let mark = self.mark_now(&account);
        let imported = credentials::import_account(
            self.profile_dir(),
            self.kind_packages(),
            kind,
            &account,
            from,
        )?;
        {
            let _gate = credentials::lock_homes();
            let row = AgentAccountRow {
                id: account.clone(),
                kind: kind.to_owned(),
                label: label.to_owned(),
                created_at: now_ms(),
                identity: None,
                state: state_name(AccountState::Unknown).to_owned(),
                origin: origin.to_owned(),
            };
            if let Err(error) = self.store().insert_pending_sign_in(&row) {
                let _ = credentials::remove_account_locked(self.profile_dir(), kind, &account);
                return Err(error.into());
            }
        }
        Ok((account, imported, mark))
    }

    /// The check of an import: `None` when the kind declares no check or cannot run one. It records nothing: the import's
    /// decision does.
    fn check_imported(&self, kind: &str, account: &str) -> Option<CheckOutcome> {
        let (decl, program) = self.runnable_kind(kind).ok()?;
        self.check_account_home(&decl, &program, account, None)
    }

    /// Runs the kind's check on an account's home and settles the result against the recorded identity. `None` when the
    /// kind declares no check, or the account has no home. The check runs outside the gate; one stopped at its time limit
    /// is [`CheckOutcome::TimedOut`].
    pub(crate) fn check_account_home(
        &self,
        decl: &KindDecl,
        program: &Path,
        account: &str,
        recorded: Option<&str>,
    ) -> Option<CheckOutcome> {
        let check = decl.accounts.as_ref()?.check.as_ref()?;
        let credential_decl = decl.credentials.as_ref()?;
        let home = {
            let _gate = credentials::lock_homes();
            let home = credentials::usable_account_home(self.profile_dir(), &decl.id, account)
                .ok()
                .flatten()?;
            // The check sees the home the session will: its state file repaired first (decision 0099). One that cannot
            // be repaired would open the CLI's first-run setup, so the account needs a new sign-in or import.
            if let Some(rule) = &credential_decl.state
                && let Err(reason) = state_file::ensure(&home, rule)
            {
                tracing::warn!(kind = %decl.id, file = %rule.file, %reason, "the account's state file cannot be repaired");
                // Only a lasting reason means the account needs a new sign-in; a passing one says nothing about it.
                let state = if state_file::lasting(&reason, rule) {
                    AccountState::NeedsSignIn
                } else {
                    AccountState::Unknown
                };
                return Some(CheckOutcome::Decided(Verdict {
                    state,
                    identity: None,
                }));
            }
            home
        };
        let run = self.run_account_check(
            program,
            &check.args,
            &home,
            &credential_decl.home_env,
            &account_strip_names(decl),
        );
        Some(outcome(check, &run, recorded))
    }

    /// Runs the kind's sign-out on an account's home, when declared and the home exists. It runs outside the gate. Its
    /// failure is only logged.
    pub(crate) fn sign_out(&self, decl: &KindDecl, program: &Path, kind: &str, account: &str) {
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
            &account_strip_names(decl),
        );
        if run.exit != Some(0) {
            tracing::warn!(
                kind,
                "the account's sign-out did not finish cleanly; its home is deleted anyway"
            );
        }
    }

    /// Deletes an account that is not kept: its home and its row. Under the gate, and only when nothing is using it, being
    /// removed, or signing in. A refusal is logged and the account stays.
    pub(crate) fn discard_account(&self, kind: &str, account: &str) {
        let _gate = credentials::lock_homes();
        if self.account_is_busy(account) {
            tracing::warn!(kind, "an account to discard is busy; it is kept");
            return;
        }
        if let Err(error) = credentials::remove_account_locked(self.profile_dir(), kind, account) {
            tracing::warn!(
                kind,
                code = error.code,
                "a discarded account's home could not be deleted"
            );
        }
        if self.store().delete_agent_account(account).is_err() {
            tracing::warn!(kind, "a discarded account's row could not be deleted");
        }
        self.forget_listings(kind, account);
    }

    /// Whether `account` is being removed, signing in, or used by a live run.
    fn account_is_busy(&self, account: &str) -> bool {
        let busy_set = self.account_busy();
        busy_set.removing.contains(account)
            || busy_set.pending.contains_key(account)
            || self.used_by_live_run(account)
    }

    pub(crate) fn make_active_if_none(&self, kind: &str, account: &str) {
        if matches!(self.store().get_active_agent_account(kind), Ok(None)) {
            let _ = self.store().set_active_agent_account(kind, account);
            self.forget_listings(kind, account);
        }
    }

    /// Records what a check found for an account, and clears its sign-in mark: a kept account is decided, so it is no longer
    /// an open sign-in, and the kind's history records it (decision 0059). Every decision that keeps an account uses it. The
    /// caller holds the homes gate. Returns whether the account exists.
    pub(crate) fn keep_decided(
        &self,
        kind: &str,
        account: &str,
        state: AccountState,
        identity: Option<&str>,
    ) -> Result<bool, StoreError> {
        let found = self
            .store()
            .set_agent_account_check(account, state_name(state), identity)?;
        if found && let Err(error) = self.store().set_agent_account_pending(account, false) {
            tracing::warn!(kind, code = %error, "a decided sign-in could not be cleared");
        }
        Ok(found)
    }

    pub(crate) fn notify_account(&self, kind: &str, account: &str, state: AccountState) {
        self.notify(Change::Account(AccountsChanged {
            kind: kind.to_owned(),
            account: account.to_owned(),
            state,
        }));
    }
}

/// An import that kept no account, with its reason.
fn failed_import(reason: String) -> AccountsImported {
    AccountsImported {
        result: ImportResult::Failed,
        account: None,
        reason: Some(reason),
        updated: None,
    }
}

/// Whether an import failed only because required files are missing: every reason is `missing`.
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

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::sessions::tests::service;
    use std::sync::{Arc, Mutex};

    fn account_row(state: &str, origin: &str) -> AgentAccountRow {
        AgentAccountRow {
            id: new_id(),
            kind: "fake".to_owned(),
            label: "work".to_owned(),
            created_at: now_ms(),
            identity: None,
            state: state.to_owned(),
            origin: origin.to_owned(),
        }
    }

    #[test]
    fn a_third_party_package_never_cleans_a_built_in_kind() {
        let (agents, _, profile) = service();
        let mut account = account_row(state_name(AccountState::SignedIn), ORIGIN_IMPORT);
        account.kind = "claude".to_owned();
        agents.store().insert_agent_account(&account).unwrap();
        let home = {
            let _gate = credentials::lock_homes();
            credentials::create_account_home(profile.path(), "claude", &account.id).unwrap()
        };

        let refused = agents
            .remove_package_accounts("acme.kit", &["claude".to_owned()])
            .expect_err("the built-in kind is skipped and reported");

        assert_eq!(refused.code, codes::AGENT_HOOK_REFUSED);
        assert!(refused.message.contains("claude"), "{}", refused.message);
        assert!(home.is_dir(), "the account's home stays");
        assert_eq!(
            agents.store().list_agent_accounts("claude").unwrap().len(),
            1
        );
    }

    #[test]
    fn an_account_that_is_not_pending_is_activated() {
        let (agents, _, _profile) = service();
        let account = account_row(state_name(AccountState::SignedIn), ORIGIN_IMPORT);
        agents.store().insert_agent_account(&account).unwrap();

        let summary = agents
            .accounts_activate("fake", &account.id, None)
            .expect("a signed-in account is activated");
        assert!(summary.active);
        assert_eq!(
            agents.store().get_active_agent_account("fake").unwrap(),
            Some(account.id.clone())
        );
    }

    #[test]
    fn a_sign_in_still_open_in_the_store_refuses_activation_with_nothing_in_memory() {
        let (agents, _, _profile) = service();
        // A sign-in an earlier service left undecided: its state is not `needs_sign_in`, so only its mark refuses it.
        let open = account_row("unknown", ORIGIN_SIGN_IN);
        agents.store().insert_pending_sign_in(&open).unwrap();

        match agents.accounts_activate("fake", &open.id, None) {
            Ok(_) => panic!("an undecided sign-in was activated"),
            Err(error) => assert_eq!(error.code, codes::AGENT_BUSY, "{}", error.message),
        }
        assert_eq!(
            agents.store().get_active_agent_account("fake").unwrap(),
            None,
            "nothing became active"
        );
    }

    #[test]
    fn keeping_an_account_as_checked_clears_its_sign_in_mark() {
        let (agents, _, _profile) = service();
        let open = account_row(state_name(AccountState::NeedsSignIn), ORIGIN_SIGN_IN);
        agents.store().insert_pending_sign_in(&open).unwrap();

        assert!(
            agents
                .keep_decided("fake", &open.id, AccountState::Unknown, None)
                .unwrap()
        );
        assert!(
            agents.store().list_pending_sign_ins().unwrap().is_empty(),
            "a kept account is no longer an open sign-in"
        );
    }

    #[test]
    fn a_check_of_an_account_being_decided_is_refused_as_busy() {
        let (agents, _, _profile) = service();
        // A sign-in the startup sweep is deciding right now: its mark is in the busy set, and the store still has it pending.
        let open = account_row("unknown", ORIGIN_SIGN_IN);
        agents.store().insert_pending_sign_in(&open).unwrap();
        agents
            .account_busy()
            .pending
            .insert(open.id.clone(), String::new());

        match agents.accounts_check("fake", Some(&open.id)) {
            Ok(_) => panic!("a check ran on an account that is being decided"),
            Err(error) => assert_eq!(error.code, codes::AGENT_BUSY, "{}", error.message),
        }
    }

    #[test]
    fn a_new_import_is_not_history_until_its_check_keeps_it() {
        let (agents, _, profile) = service();
        let kinds = paths::kinds_dir(profile.path());
        fs::create_dir_all(&kinds).expect("create the kinds directory");
        let kind = serde_json::json!({
            "id": "fake",
            "name": "Fake",
            "program": ["fake-agent"],
            "credentials": { "home_env": "FAKE_AGENT_HOME", "files": ["token.json"] }
        });
        fs::write(kinds.join("fake.json"), kind.to_string()).expect("write the fake kind");
        let source = tempfile::tempdir().expect("a source directory");
        fs::write(source.path().join("token.json"), "{}").expect("write a token");

        let (account, _, _mark) = agents
            .import_new_account(
                "fake",
                ImportFrom::Folder(source.path()),
                "work",
                ORIGIN_IMPORT,
            )
            .expect("the import copies the declared file");
        assert!(
            !agents.store().kind_has_had_account("fake").unwrap(),
            "an import its check has not kept is not history"
        );
        agents
            .keep_decided("fake", &account, AccountState::SignedIn, None)
            .unwrap();
        assert!(
            agents.store().kind_has_had_account("fake").unwrap(),
            "a kept import is history"
        );
    }

    /// Collects the changes a service announces. It is set before the call under test, since a service takes one listener.
    pub(crate) fn collect_changes(agents: &Agents) -> Arc<Mutex<Vec<Change>>> {
        let changes = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&changes);
        agents.set_listener(Box::new(move |change| sink.lock().unwrap().push(change)));
        changes
    }

    /// The account changes collected so far, in order.
    pub(crate) fn account_events(changes: &Mutex<Vec<Change>>) -> Vec<AccountsChanged> {
        changes
            .lock()
            .unwrap()
            .iter()
            .filter_map(|change| match change {
                Change::Account(event) => Some(event.clone()),
                _ => None,
            })
            .collect()
    }

    /// Declares kind `fake` in the profile, running `program`, with `accounts` as its declaration when there is one.
    fn declare_kind(profile: &Path, program: &str, accounts: Option<serde_json::Value>) {
        let kinds = paths::kinds_dir(profile);
        fs::create_dir_all(&kinds).expect("create the kinds directory");
        let mut kind = serde_json::json!({
            "id": "fake",
            "name": "Fake",
            "program": [program],
            "credentials": { "home_env": "FAKE_AGENT_HOME", "files": ["token.json"] }
        });
        if let Some(accounts) = accounts {
            kind["accounts"] = accounts;
        }
        fs::write(kinds.join("fake.json"), kind.to_string()).expect("write the fake kind");
    }

    /// A declared check, which [`refuse_checks`] makes refuse every login.
    fn refusing_check() -> serde_json::Value {
        serde_json::json!({ "check": { "args": ["--no-such-option-for-this-check"] } })
    }

    /// Makes every check of the service finish at once with a non-zero exit, as the stand-in for its program. Running a real
    /// program here would be at the mercy of a loaded machine: a run past the time limit is no verdict, so the account
    /// would be kept as `unknown` instead of refused (decision 0059, amended 2026-11-27).
    fn refuse_checks(agents: &Agents) {
        agents.set_check_settings(crate::check_retry::CheckSettings {
            runner: Some(Arc::new(|_program: &Path, _args: &[String]| {
                crate::account_check::CheckRun {
                    run: crate::account_check::RunOutput {
                        exit: Some(1),
                        output: String::new(),
                    },
                    timed_out: false,
                }
            })),
            ..Default::default()
        });
    }

    /// A source directory holding the declared credential file.
    pub(crate) fn source_with_token() -> tempfile::TempDir {
        let source = tempfile::tempdir().expect("a source directory");
        fs::write(source.path().join("token.json"), "{}").expect("write a token");
        source
    }

    #[test]
    fn a_kept_import_announces_its_account_once() {
        let (agents, _, profile) = service();
        declare_kind(profile.path(), "fake-agent", None);
        let changes = collect_changes(&agents);
        let source = source_with_token();

        let imported = agents
            .accounts_import("fake", source.path().to_str(), None)
            .expect("the import runs");
        let account = imported.account.expect("the import keeps its account");
        let events = account_events(&changes);
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(events[0].kind, "fake");
        assert_eq!(events[0].account, account.id);
        assert_eq!(
            events[0].state,
            AccountState::Unknown,
            "a kind with no check keeps it as unknown"
        );
    }

    #[test]
    fn a_refused_import_announces_its_last_known_state_once_and_keeps_nothing() {
        let (agents, _, profile) = service();
        let program = std::env::current_exe().expect("the test binary");
        declare_kind(
            profile.path(),
            program.to_str().expect("a UTF-8 path"),
            Some(refusing_check()),
        );
        refuse_checks(&agents);
        let changes = collect_changes(&agents);
        let source = source_with_token();

        let imported = agents
            .accounts_import("fake", source.path().to_str(), None)
            .expect("the import runs");
        assert_eq!(imported.result, ImportResult::Failed, "{imported:?}");
        assert!(
            agents.accounts_list("fake").expect("a list").is_empty(),
            "a refused import is deleted"
        );
        let events = account_events(&changes);
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(events[0].kind, "fake");
        assert_eq!(
            events[0].state,
            AccountState::NeedsSignIn,
            "the state its check found"
        );
    }

    #[test]
    fn an_activation_announces_the_account_once() {
        let (agents, _, _profile) = service();
        let account = account_row(state_name(AccountState::SignedIn), ORIGIN_IMPORT);
        agents.store().insert_agent_account(&account).unwrap();
        let changes = collect_changes(&agents);

        agents
            .accounts_activate("fake", &account.id, None)
            .expect("the activation runs");

        let events = account_events(&changes);
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(events[0].kind, "fake");
        assert_eq!(events[0].account, account.id);
        assert_eq!(events[0].state, AccountState::SignedIn);
    }

    #[test]
    fn a_removal_announces_the_removed_account_once_with_its_last_known_state() {
        let (agents, _, profile) = service();
        declare_kind(profile.path(), "fake-agent", None);
        let source = source_with_token();
        let imported = agents
            .accounts_import("fake", source.path().to_str(), None)
            .expect("the import runs");
        let account = imported.account.expect("the import keeps its account");
        let changes = collect_changes(&agents);

        agents
            .accounts_remove("fake", &account.id)
            .expect("the removal runs");

        let events = account_events(&changes);
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(events[0].kind, "fake");
        assert_eq!(events[0].account, account.id);
        assert_eq!(
            events[0].state,
            AccountState::Unknown,
            "the state it had when it was removed"
        );
    }

    #[test]
    fn a_sign_in_decided_as_kept_announces_it_once() {
        let (agents, _, profile) = service();
        declare_kind(profile.path(), "fake-agent", None);
        let account = account_row(state_name(AccountState::NeedsSignIn), ORIGIN_SIGN_IN);
        agents.store().insert_pending_sign_in(&account).unwrap();
        // The login's file is in the home and no check is declared, so the sign-in is kept as unknown.
        let home = paths::account_home(profile.path(), "fake", &account.id);
        fs::create_dir_all(&home).expect("create the home");
        fs::write(home.join("token.json"), "{}").expect("write a token");
        let changes = collect_changes(&agents);

        agents.decide_sign_in("fake", &account.id, SignInEnd::TerminalEnded);

        let events = account_events(&changes);
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(events[0].account, account.id);
        assert_eq!(events[0].state, AccountState::Unknown);
    }

    #[test]
    fn a_sign_in_decided_as_refused_announces_it_once_and_is_deleted() {
        let (agents, _, _profile) = service();
        // No home and no login file: the sign-in is refused, and it is announced with the state it was added with.
        let account = account_row(state_name(AccountState::NeedsSignIn), ORIGIN_SIGN_IN);
        agents.store().insert_pending_sign_in(&account).unwrap();
        let changes = collect_changes(&agents);

        agents.decide_sign_in("fake", &account.id, SignInEnd::TerminalEnded);

        assert!(
            agents
                .store()
                .get_agent_account(&account.id)
                .unwrap()
                .is_none(),
            "a refused sign-in is deleted"
        );
        let events = account_events(&changes);
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(events[0].account, account.id);
        assert_eq!(events[0].state, AccountState::NeedsSignIn);
    }

    #[test]
    fn a_check_that_changes_an_accounts_state_announces_it_once() {
        let (agents, _, profile) = service();
        let program = std::env::current_exe().expect("the test binary");
        declare_kind(
            profile.path(),
            program.to_str().expect("a UTF-8 path"),
            Some(refusing_check()),
        );
        refuse_checks(&agents);
        let account = account_row(state_name(AccountState::SignedIn), ORIGIN_IMPORT);
        agents.store().insert_agent_account(&account).unwrap();
        let changes = collect_changes(&agents);

        let checked = agents
            .accounts_check("fake", Some(&account.id))
            .expect("the check runs");
        assert_eq!(checked.state, AccountState::NeedsSignIn);
        let events = account_events(&changes);
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(events[0].state, AccountState::NeedsSignIn);
    }
}
