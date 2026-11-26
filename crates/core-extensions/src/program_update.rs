//! Updating a host program with its vendor's update command (decision 0151). Only a program this package installed is
//! updated, and only with the command the person reviewed.

use std::fs;

use massorca_ext_package::manifest::agent::is_vendor_program;
use serde::Serialize;
use serde_json::json;

use crate::error::Error;
use crate::host::INSTALLER_TIMEOUT;
use crate::install_log_tail::last_log_lines_after;
use crate::run_log::{RunAction, log_len};
use crate::service::{Extensions, describe};
use crate::source::sha256_hex;

/// What updating one host program runs, and the fingerprint that accepts it. Nothing was changed to build it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProgramUpdateReview {
    pub id: String,
    pub program: String,
    pub binary: String,
    pub command: Vec<String>,
    pub verify: Option<Vec<String>>,
    pub consent: String,
}

/// An update that ran and verified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProgramUpdated {
    pub id: String,
    pub program: String,
    pub command: Vec<String>,
    pub exit_code: Option<i32>,
}

impl Extensions {
    /// The update of `program` in package `id` for this OS. Refused when this package did not install the program.
    pub fn review_program_update(
        &self,
        id: &str,
        program: &str,
    ) -> Result<ProgramUpdateReview, Error> {
        let entry = self
            .read_lockfile()?
            .get(id)
            .cloned()
            .ok_or_else(|| Error::NotInstalled(id.to_owned()))?;
        let refused = |reason: &str| Error::ProgramUpdateRefused {
            id: id.to_owned(),
            program: program.to_owned(),
            reason: reason.to_owned(),
        };
        let record = entry
            .cli
            .iter()
            .find(|cli| cli.agent == program)
            .ok_or_else(|| refused("the package names no such program"))?;
        if !record.installed_by_package {
            return Err(refused(
                "MassOrca did not install it; update it with the tool that installed it",
            ));
        }
        let manifest = self.installed_manifest(&entry)?;
        let cli = manifest
            .cli_of(program)
            .ok_or_else(|| refused("the package names no such program"))?;
        let command = cli
            .update
            .iter()
            .find(|step| Some(step.os) == self.os)
            .map(|step| step.command.clone())
            .ok_or_else(|| refused("the package has no update command for this system"))?;
        let consent = sha256_hex(
            json!([
                "massorca.program-update/1",
                id,
                entry.version,
                program,
                command,
                cli.verify
            ])
            .to_string()
            .as_bytes(),
        );
        Ok(ProgramUpdateReview {
            id: id.to_owned(),
            program: program.to_owned(),
            binary: cli.binary.clone(),
            command,
            verify: cli.verify.clone(),
            consent,
        })
    }

    /// Runs the reviewed update command, then the verify command. `consent` is the review's fingerprint.
    pub fn update_program(
        &self,
        id: &str,
        program: &str,
        consent: Option<&str>,
    ) -> Result<ProgramUpdated, Error> {
        let review = self.review_program_update(id, program)?;
        match consent {
            None => return Err(Error::ConsentRequired),
            Some(given) if given != review.consent => return Err(Error::ConsentMismatch),
            Some(_) => {}
        }
        let _busy = {
            let _changes = self.lock_changes();
            self.mark_busy(id, Vec::new())?
        };
        let log = self.layout.install_log(id);
        let offset = log_len(&log);
        let ran = self.run_update(&review, &log);
        let version = self
            .read_lockfile()?
            .get(id)
            .map(|entry| entry.version.clone());
        self.log_run(
            id,
            RunAction::Update,
            version.clone(),
            version,
            ran.is_ok(),
            last_log_lines_after(&log, offset),
        );
        ran
    }

    fn run_update(
        &self,
        review: &ProgramUpdateReview,
        log: &std::path::Path,
    ) -> Result<ProgramUpdated, Error> {
        let command = &review.command;
        let program_of = |argv: &[String]| argv.first().cloned().into_iter().collect::<Vec<_>>();
        let failed = |message: String| Error::InstallerFailed {
            agent: review.program.clone(),
            message,
            diagnosis: self.diagnosis(program_of(command), Some(log)),
        };
        if !is_vendor_program(command) {
            return Err(failed(
                "its program is not on the vendor allowlist".to_owned(),
            ));
        }
        let data = self.layout.data_dir(&review.id);
        fs::create_dir_all(&data).map_err(Error::io("cannot create", &data))?;
        let exit = self
            .runner
            .run(command, log, INSTALLER_TIMEOUT)
            .map_err(|error| failed(error.to_string()))?;
        if !exit.success() {
            return Err(failed(describe(exit)));
        }
        let unverified = |message: String, programs: Vec<String>| Error::VerifyFailed {
            agent: review.program.clone(),
            message,
            installer: Some(command.clone()),
            diagnosis: self.diagnosis(programs, Some(log)),
        };
        if let Some(verify) = &review.verify {
            let verified = self
                .runner
                .run(verify, log, INSTALLER_TIMEOUT)
                .map_err(|error| unverified(error.to_string(), program_of(verify)))?;
            if !verified.success() {
                return Err(unverified(describe(verified), program_of(verify)));
            }
        }
        if self.resolver.resolve(&review.binary).is_none() {
            return Err(unverified(
                format!("`{}` does not resolve", review.binary),
                vec![review.binary.clone()],
            ));
        }
        Ok(ProgramUpdated {
            id: review.id.clone(),
            program: review.program.clone(),
            command: command.clone(),
            exit_code: exit.code,
        })
    }
}
