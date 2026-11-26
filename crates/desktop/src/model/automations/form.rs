//! The fields the automation editor holds, checked before they are sent (decision 0123). The service checks again; these
//! checks only name the field a person must change.

use massorca_protocol::methods::automations::{
    Automation, AutomationCadence, AutomationCreateParams, AutomationDestination,
    AutomationDestinationKind, AutomationUpdateParams, AutomationWeekday,
};

/// The editor field a refusal names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Project,
    Name,
    Prompt,
    Agent,
    Time,
    Cron,
    Destination,
}

/// A field that cannot be sent as it stands, and what to change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormError {
    pub field: Field,
    pub message: String,
}

/// The cadences the editor offers, in the order it lists them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cadence {
    Hourly,
    #[default]
    Daily,
    Weekdays,
    Weekly,
    Cron,
}

impl Cadence {
    pub const ALL: [Cadence; 5] = [
        Cadence::Hourly,
        Cadence::Daily,
        Cadence::Weekdays,
        Cadence::Weekly,
        Cadence::Cron,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Cadence::Hourly => "Hourly",
            Cadence::Daily => "Daily",
            Cadence::Weekdays => "Weekdays",
            Cadence::Weekly => "Weekly",
            Cadence::Cron => "Cron expression",
        }
    }

    pub fn to_protocol(self) -> AutomationCadence {
        match self {
            Cadence::Hourly => AutomationCadence::Hourly,
            Cadence::Daily => AutomationCadence::Daily,
            Cadence::Weekdays => AutomationCadence::Weekdays,
            Cadence::Weekly => AutomationCadence::Weekly,
            Cadence::Cron => AutomationCadence::Cron,
        }
    }

    /// The choice an automation's cadence shows as; a cadence this build does not know shows as daily.
    pub fn of(cadence: AutomationCadence) -> Self {
        match cadence {
            AutomationCadence::Hourly => Cadence::Hourly,
            AutomationCadence::Weekdays => Cadence::Weekdays,
            AutomationCadence::Weekly => Cadence::Weekly,
            AutomationCadence::Cron => Cadence::Cron,
            AutomationCadence::Daily | AutomationCadence::Unknown => Cadence::Daily,
        }
    }
}

/// The days a weekly automation can run, Monday first.
pub const WEEKDAYS: [AutomationWeekday; 7] = [
    AutomationWeekday::Monday,
    AutomationWeekday::Tuesday,
    AutomationWeekday::Wednesday,
    AutomationWeekday::Thursday,
    AutomationWeekday::Friday,
    AutomationWeekday::Saturday,
    AutomationWeekday::Sunday,
];

/// The name a weekday shows as in the editor and the summaries.
pub fn weekday_label(day: AutomationWeekday) -> &'static str {
    match day {
        AutomationWeekday::Monday => "Monday",
        AutomationWeekday::Tuesday => "Tuesday",
        AutomationWeekday::Wednesday => "Wednesday",
        AutomationWeekday::Thursday => "Thursday",
        AutomationWeekday::Friday => "Friday",
        AutomationWeekday::Saturday => "Saturday",
        AutomationWeekday::Sunday => "Sunday",
        AutomationWeekday::Unknown => "an unknown day",
    }
}

/// Where the runs work, as the editor holds it (decision 0127). The workspace may be an id, a name or a path; the service
/// resolves it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FormDestination {
    /// The project's own place: one worktree kept for the automation.
    #[default]
    ProjectDefault,
    /// A new worktree for each run, from `base_branch` when it is not blank.
    NewWorktree { base_branch: String },
    /// One existing workspace; every run goes there.
    Workspace { workspace: String },
}

impl FormDestination {
    /// The choices the editor offers, in the order it lists them.
    pub const LABELS: [&'static str; 3] = [
        "The project's own place",
        "A new worktree per run",
        "An existing workspace",
    ];

    /// The position of this destination's choice in [`Self::LABELS`].
    pub fn index(&self) -> usize {
        match self {
            Self::ProjectDefault => 0,
            Self::NewWorktree { .. } => 1,
            Self::Workspace { .. } => 2,
        }
    }

    /// The destination a choice position and the two text fields mean. Positions past the choices are the default.
    pub fn from_choice(index: Option<usize>, base_branch: String, workspace: String) -> Self {
        match index {
            Some(1) => Self::NewWorktree { base_branch },
            Some(2) => Self::Workspace { workspace },
            _ => Self::ProjectDefault,
        }
    }

    /// The destination an automation shows as.
    pub fn of(destination: &AutomationDestination) -> Self {
        match destination.kind {
            AutomationDestinationKind::ProjectWorktreePerRun => Self::NewWorktree {
                base_branch: destination.base_branch.clone().unwrap_or_default(),
            },
            AutomationDestinationKind::ExistingWorkspace => Self::Workspace {
                workspace: destination.workspace.clone().unwrap_or_default(),
            },
            AutomationDestinationKind::ProjectDefault | AutomationDestinationKind::Unknown => {
                Self::ProjectDefault
            }
        }
    }

    /// The destination the request carries. The editor always sends one, so an edit can return to the default.
    fn to_protocol(&self) -> Result<AutomationDestination, FormError> {
        match self {
            Self::ProjectDefault => Ok(AutomationDestination::default()),
            Self::NewWorktree { base_branch } => Ok(AutomationDestination {
                kind: AutomationDestinationKind::ProjectWorktreePerRun,
                base_branch: non_blank(base_branch),
                ..AutomationDestination::default()
            }),
            Self::Workspace { workspace } => {
                let workspace = non_blank(workspace).ok_or_else(|| {
                    error(
                        Field::Destination,
                        "Name the workspace the runs go to: its id, name or path.",
                    )
                })?;
                Ok(AutomationDestination {
                    kind: AutomationDestinationKind::ExistingWorkspace,
                    workspace: Some(workspace),
                    ..AutomationDestination::default()
                })
            }
        }
    }
}

/// What the editor holds. The project is required to create an automation; an edit may keep the one it has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutomationForm {
    /// The project the runs work in, as the service selects it: its id, its name or its path.
    pub project: String,
    pub name: String,
    pub prompt: String,
    pub agent: String,
    pub cadence: Cadence,
    /// Wall-clock `HH:MM`; unused by a cron cadence.
    pub time: String,
    pub weekday: AutomationWeekday,
    pub cron: String,
    /// An IANA zone name; blank means the host's zone.
    pub zone: String,
    pub destination: FormDestination,
}

impl AutomationForm {
    /// A new automation's starting values: daily at 09:00 for Claude, with no project chosen yet.
    pub fn starting() -> Self {
        Self {
            project: String::new(),
            name: String::new(),
            prompt: String::new(),
            agent: "claude".to_owned(),
            cadence: Cadence::Daily,
            time: "09:00".to_owned(),
            weekday: AutomationWeekday::Monday,
            cron: String::new(),
            zone: String::new(),
            destination: FormDestination::default(),
        }
    }

    /// The values an edit starts from: the automation's own fields.
    pub fn of(automation: &Automation) -> Self {
        Self {
            project: automation
                .project_name
                .clone()
                .or_else(|| automation.project_id.clone())
                .unwrap_or_default(),
            name: automation.name.clone(),
            prompt: automation.prompt.clone(),
            agent: automation.agent.clone(),
            cadence: Cadence::of(automation.cadence),
            time: automation.time.clone(),
            weekday: automation.weekday.unwrap_or(AutomationWeekday::Monday),
            cron: automation.cron.clone().unwrap_or_default(),
            zone: automation.zone.clone().unwrap_or_default(),
            destination: FormDestination::of(&automation.destination),
        }
    }

    /// The request that creates the automation, when every field is usable.
    pub fn create_params(
        &self,
        creation_key: Option<String>,
    ) -> Result<AutomationCreateParams, FormError> {
        let checked = self.checked(true)?;
        Ok(AutomationCreateParams {
            project: checked.project,
            name: checked.name,
            prompt: checked.prompt,
            agent: checked.agent,
            cadence: self.cadence.to_protocol(),
            time: checked.time,
            weekday: self.weekday_for_cadence(),
            cron: checked.cron,
            zone: Some(checked.zone).filter(|zone| !zone.is_empty()),
            grace_minutes: None,
            creation_key,
            destination: Some(checked.destination),
        })
    }

    /// The request that changes the automation `id` to these values. It carries every field, so the service keeps
    /// nothing the editor no longer shows.
    pub fn update_params(&self, id: &str) -> Result<AutomationUpdateParams, FormError> {
        let checked = self.checked(false)?;
        Ok(AutomationUpdateParams {
            automation: id.to_owned(),
            project: checked.project_on_edit,
            name: Some(checked.name),
            prompt: Some(checked.prompt),
            agent: Some(checked.agent),
            cadence: Some(self.cadence.to_protocol()),
            time: Some(checked.time),
            weekday: self.weekday_for_cadence(),
            cron: checked.cron,
            zone: Some(checked.zone),
            grace_minutes: None,
            destination: Some(checked.destination),
        })
    }

    /// The weekday the request carries: only a weekly automation has one.
    fn weekday_for_cadence(&self) -> Option<AutomationWeekday> {
        (self.cadence == Cadence::Weekly).then_some(self.weekday)
    }

    fn checked(&self, creating: bool) -> Result<Checked, FormError> {
        let name = non_blank(&self.name)
            .ok_or_else(|| error(Field::Name, "Give the automation a name."))?;
        let prompt = non_blank(&self.prompt)
            .ok_or_else(|| error(Field::Prompt, "Write the prompt the agent starts with."))?;
        let agent =
            non_blank(&self.agent).ok_or_else(|| error(Field::Agent, "Choose an agent."))?;
        let project = non_blank(&self.project);
        if creating && project.is_none() {
            return Err(error(
                Field::Project,
                "Choose the project the runs work in.",
            ));
        }
        let project_on_edit = project.clone();
        let project = project.unwrap_or_default();
        let destination = self.destination.to_protocol()?;
        let (time, cron) = match self.cadence {
            Cadence::Cron => {
                let cron = non_blank(&self.cron).ok_or_else(|| {
                    error(Field::Cron, "Write the five fields, such as 0 9 * * 1-5.")
                })?;
                (String::new(), Some(cron))
            }
            _ => {
                if !valid_time(self.time.trim()) {
                    return Err(error(Field::Time, "Use a 24-hour time such as 09:00."));
                }
                (self.time.trim().to_owned(), None)
            }
        };
        Ok(Checked {
            project,
            project_on_edit,
            name,
            prompt,
            agent,
            time,
            cron,
            zone: self.zone.trim().to_owned(),
            destination,
        })
    }
}

/// The values after the checks: text trimmed, and the cadence's own field only.
struct Checked {
    project: String,
    project_on_edit: Option<String>,
    name: String,
    prompt: String,
    agent: String,
    time: String,
    cron: Option<String>,
    zone: String,
    destination: AutomationDestination,
}

fn error(field: Field, message: &str) -> FormError {
    FormError {
        field,
        message: message.to_owned(),
    }
}

fn non_blank(text: &str) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// `HH:MM` on the 24-hour clock.
fn valid_time(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 5
        && bytes[2] == b':'
        && bytes[0].is_ascii_digit()
        && bytes[1].is_ascii_digit()
        && bytes[3].is_ascii_digit()
        && bytes[4].is_ascii_digit()
        && text[..2].parse::<u8>().is_ok_and(|hour| hour <= 23)
        && text[3..].parse::<u8>().is_ok_and(|minute| minute <= 59)
}

#[cfg(test)]
mod tests {
    use super::*;
    use massorca_protocol::methods::automations::AutomationDestinationKind;

    fn filled() -> AutomationForm {
        AutomationForm {
            project: "app".to_owned(),
            name: "Morning PRs".to_owned(),
            prompt: "Summarise the open pull requests".to_owned(),
            ..AutomationForm::starting()
        }
    }

    #[test]
    fn a_new_automation_needs_a_name_a_prompt_an_agent_and_a_project() {
        let form = AutomationForm {
            name: "  ".to_owned(),
            ..filled()
        };
        assert_eq!(form.create_params(None).unwrap_err().field, Field::Name);

        let form = AutomationForm {
            prompt: String::new(),
            ..filled()
        };
        assert_eq!(form.create_params(None).unwrap_err().field, Field::Prompt);

        let form = AutomationForm {
            agent: String::new(),
            ..filled()
        };
        assert_eq!(form.create_params(None).unwrap_err().field, Field::Agent);

        let form = AutomationForm {
            project: "  ".to_owned(),
            ..filled()
        };
        assert_eq!(form.create_params(None).unwrap_err().field, Field::Project);
    }

    #[test]
    fn a_time_must_be_a_24_hour_hh_mm() {
        for bad in ["9:00", "24:00", "12:60", "12-30", "noon", ""] {
            let form = AutomationForm {
                time: bad.to_owned(),
                ..filled()
            };
            assert_eq!(
                form.create_params(None).unwrap_err().field,
                Field::Time,
                "{bad}"
            );
        }
        let form = AutomationForm {
            time: " 08:05 ".to_owned(),
            ..filled()
        };
        let params = form.create_params(None).expect("valid");
        assert_eq!(params.time, "08:05");
    }

    #[test]
    fn a_cron_needs_its_expression_and_a_cron_time_is_not_checked() {
        let form = AutomationForm {
            cadence: Cadence::Cron,
            cron: " ".to_owned(),
            time: "bad".to_owned(),
            ..filled()
        };
        assert_eq!(form.create_params(None).unwrap_err().field, Field::Cron);

        let form = AutomationForm {
            cadence: Cadence::Cron,
            cron: " 0 9 * * 1-5 ".to_owned(),
            time: "bad".to_owned(),
            ..filled()
        };
        let params = form.create_params(None).expect("valid");
        assert_eq!(params.cron.as_deref(), Some("0 9 * * 1-5"));
        assert_eq!(params.time, "");
    }

    #[test]
    fn only_a_weekly_automation_sends_its_weekday_and_a_blank_zone_is_the_hosts() {
        let weekly = AutomationForm {
            cadence: Cadence::Weekly,
            weekday: AutomationWeekday::Friday,
            ..filled()
        };
        assert_eq!(
            weekly.create_params(None).unwrap().weekday,
            Some(AutomationWeekday::Friday)
        );
        assert_eq!(filled().create_params(None).unwrap().weekday, None);
        assert_eq!(filled().create_params(None).unwrap().zone, None);
    }

    #[test]
    fn an_edit_sends_every_field_and_may_keep_a_project_the_automation_lost() {
        let form = AutomationForm {
            project: String::new(),
            ..filled()
        };
        let params = form
            .update_params("automation-1")
            .expect("an edit keeps no project");
        assert_eq!(params.automation, "automation-1");
        assert_eq!(params.project, None);
        let moved = filled().update_params("automation-1").expect("valid");
        assert_eq!(moved.project.as_deref(), Some("app"));
        assert_eq!(params.name.as_deref(), Some("Morning PRs"));
        assert_eq!(params.zone.as_deref(), Some(""));
    }

    #[test]
    fn the_destination_choice_is_sent_with_the_request() {
        let default = filled().create_params(None).expect("valid");
        assert_eq!(
            default.destination.map(|destination| destination.kind),
            Some(AutomationDestinationKind::ProjectDefault)
        );

        let per_run = AutomationForm {
            destination: FormDestination::NewWorktree {
                base_branch: " main ".to_owned(),
            },
            ..filled()
        };
        let destination = per_run
            .create_params(None)
            .expect("valid")
            .destination
            .expect("sent");
        assert_eq!(
            destination.kind,
            AutomationDestinationKind::ProjectWorktreePerRun
        );
        assert_eq!(destination.base_branch.as_deref(), Some("main"));
    }

    #[test]
    fn an_existing_workspace_needs_a_name_and_an_edit_always_sends_its_destination() {
        let blank = AutomationForm {
            destination: FormDestination::Workspace {
                workspace: "  ".to_owned(),
            },
            ..filled()
        };
        assert_eq!(
            blank.create_params(None).unwrap_err().field,
            Field::Destination
        );

        let named = AutomationForm {
            destination: FormDestination::Workspace {
                workspace: " feature-x ".to_owned(),
            },
            ..filled()
        };
        let edit = named.update_params("automation-1").expect("valid");
        let destination = edit.destination.expect("an edit sends it");
        assert_eq!(
            destination.kind,
            AutomationDestinationKind::ExistingWorkspace
        );
        assert_eq!(destination.workspace.as_deref(), Some("feature-x"));
    }

    #[test]
    fn the_choice_positions_map_to_the_destinations_they_name() {
        assert_eq!(
            FormDestination::from_choice(Some(1), "main".to_owned(), String::new()),
            FormDestination::NewWorktree {
                base_branch: "main".to_owned()
            }
        );
        assert_eq!(
            FormDestination::from_choice(Some(2), String::new(), "ws".to_owned()),
            FormDestination::Workspace {
                workspace: "ws".to_owned()
            }
        );
        assert_eq!(
            FormDestination::from_choice(None, String::new(), String::new()),
            FormDestination::ProjectDefault
        );
        assert_eq!(
            FormDestination::Workspace {
                workspace: String::new()
            }
            .index(),
            2
        );
    }
}
