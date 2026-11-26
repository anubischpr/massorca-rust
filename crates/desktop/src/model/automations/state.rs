//! The Automations page's state: the list the service last reported, the editor's target, the delete question, the
//! notice of the last refused change (decision 0123) and the open run table (decision 0125). The editor's fields live
//! with the page that draws them.

use massorca_protocol::methods::automations::{Automation, AutomationRun};

use super::runs::RunHistory;

/// What the open editor changes: a new automation, or the one with this id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Editor {
    New,
    Edit(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AutomationsState {
    /// The automations as the service last reported them, or why it could not; `None` until the first read answers.
    list: Option<Result<Vec<Automation>, String>>,
    editor: Option<Editor>,
    /// The automation the person is asked to remove.
    delete: Option<String>,
    /// Why the last change was refused, shown above the list until the next change.
    notice: Option<String>,
    /// The run table open under the list.
    history: Option<RunHistory>,
}

impl AutomationsState {
    /// The list as the service reported it, in creation order; empty while unread or unreadable.
    pub fn automations(&self) -> &[Automation] {
        match &self.list {
            Some(Ok(automations)) => automations,
            _ => &[],
        }
    }

    pub fn find(&self, id: &str) -> Option<&Automation> {
        self.automations()
            .iter()
            .find(|automation| automation.id == id)
    }

    /// Why the list could not be read, when it could not.
    pub fn list_error(&self) -> Option<&str> {
        match &self.list {
            Some(Err(message)) => Some(message),
            _ => None,
        }
    }

    /// Whether the service has answered the first read, successfully or not.
    pub fn is_read(&self) -> bool {
        self.list.is_some()
    }

    /// Replaces the list with the service's answer.
    pub fn set_list(&mut self, list: Result<Vec<Automation>, String>) {
        self.list = Some(list);
    }

    /// Puts a changed or created automation in the list: replaced where it was, appended when it is new.
    pub fn put(&mut self, automation: Automation) {
        let list = self.list.get_or_insert_with(|| Ok(Vec::new()));
        if let Ok(automations) = list {
            match automations
                .iter()
                .position(|known| known.id == automation.id)
            {
                Some(index) => automations[index] = automation,
                None => automations.push(automation),
            }
        }
    }

    /// Drops an automation the service removed.
    pub fn drop_automation(&mut self, id: &str) {
        if let Some(Ok(automations)) = &mut self.list {
            automations.retain(|automation| automation.id != id);
        }
    }

    pub fn editor(&self) -> Option<&Editor> {
        self.editor.as_ref()
    }

    /// Opens the editor on `editor`, leaving the last notice out of the way of the new attempt.
    pub fn open_editor(&mut self, editor: Editor) {
        self.editor = Some(editor);
        self.notice = None;
    }

    pub fn close_editor(&mut self) {
        self.editor = None;
    }

    pub fn ask_delete(&mut self, id: String) {
        self.delete = Some(id);
    }

    pub fn delete_target(&self) -> Option<&str> {
        self.delete.as_deref()
    }

    /// Takes the automation the person confirmed removing, closing the question.
    pub fn take_delete(&mut self) -> Option<String> {
        self.delete.take()
    }

    pub fn cancel_delete(&mut self) {
        self.delete = None;
    }

    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    pub fn set_notice(&mut self, notice: Option<String>) {
        self.notice = notice;
    }

    pub fn history(&self) -> Option<&RunHistory> {
        self.history.as_ref()
    }

    /// Opens the run table of `automation`, unread until its runs answer.
    pub fn open_history(&mut self, automation: String) {
        self.history = Some(RunHistory::new(automation));
    }

    pub fn close_history(&mut self) {
        self.history = None;
    }

    /// The runs read for `automation`; an answer for a table no longer open is dropped.
    pub fn set_runs(&mut self, automation: &str, runs: Result<Vec<AutomationRun>, String>) {
        if let Some(history) = self.history_of(automation) {
            history.set_runs(runs);
        }
    }

    /// A run the service just answered with, shown when its automation's table is open.
    pub fn put_run(&mut self, run: AutomationRun) {
        let automation = run.automation_id.clone();
        if let Some(history) = self.history_of(&automation) {
            history.put(run);
        }
    }

    pub fn history_of(&mut self, automation: &str) -> Option<&mut RunHistory> {
        self.history
            .as_mut()
            .filter(|history| history.automation() == automation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use massorca_protocol::methods::automations::AutomationCadence;

    fn automation(id: &str, name: &str) -> Automation {
        Automation {
            id: id.to_owned(),
            project_id: Some("p1".to_owned()),
            project_name: Some("app".to_owned()),
            name: name.to_owned(),
            prompt: "Summarise".to_owned(),
            agent: "claude".to_owned(),
            cadence: AutomationCadence::Daily,
            time: "09:00".to_owned(),
            weekday: None,
            cron: None,
            zone: None,
            zone_used: None,
            grace_minutes: 720,
            enabled: true,
            next_run_at_ms: None,
            last_run_at_ms: None,
            created_at_ms: 0,
            updated_at_ms: 0,
        }
    }

    #[test]
    fn nothing_is_listed_until_the_first_read_answers() {
        let state = AutomationsState::default();
        assert!(!state.is_read());
        assert!(state.automations().is_empty());
        assert_eq!(state.list_error(), None);
    }

    #[test]
    fn a_put_replaces_in_place_and_a_new_automation_is_appended() {
        let mut state = AutomationsState::default();
        state.set_list(Ok(vec![
            automation("a", "first"),
            automation("b", "second"),
        ]));
        state.put(automation("a", "renamed"));
        state.put(automation("c", "third"));
        let names: Vec<&str> = state
            .automations()
            .iter()
            .map(|automation| automation.name.as_str())
            .collect();
        assert_eq!(names, ["renamed", "second", "third"]);
    }

    #[test]
    fn a_removed_automation_leaves_the_list_by_id() {
        let mut state = AutomationsState::default();
        state.set_list(Ok(vec![
            automation("a", "first"),
            automation("b", "second"),
        ]));
        state.drop_automation("a");
        assert_eq!(state.automations().len(), 1);
        assert!(state.find("a").is_none());
        assert_eq!(state.find("b").map(|a| a.name.as_str()), Some("second"));
    }

    #[test]
    fn a_failed_read_says_why_and_lists_nothing() {
        let mut state = AutomationsState::default();
        state.set_list(Err("The service is not connected.".to_owned()));
        assert!(state.is_read());
        assert_eq!(state.list_error(), Some("The service is not connected."));
        assert!(state.automations().is_empty());
    }

    #[test]
    fn the_delete_question_names_one_automation_until_it_is_taken_or_cancelled() {
        let mut state = AutomationsState::default();
        state.ask_delete("a".to_owned());
        assert_eq!(state.delete_target(), Some("a"));
        assert_eq!(state.take_delete().as_deref(), Some("a"));
        assert_eq!(state.delete_target(), None);
        state.ask_delete("b".to_owned());
        state.cancel_delete();
        assert_eq!(state.delete_target(), None);
    }

    #[test]
    fn runs_read_for_a_table_no_longer_open_are_dropped() {
        let mut state = AutomationsState::default();
        state.open_history("a".to_owned());
        state.set_runs("b", Ok(Vec::new()));
        assert!(!state.history().is_some_and(RunHistory::is_read));
        state.set_runs("a", Err("The service is not connected.".to_owned()));
        assert_eq!(
            state.history().and_then(RunHistory::error),
            Some("The service is not connected.")
        );
        state.close_history();
        assert!(state.history().is_none());
    }

    #[test]
    fn opening_the_editor_clears_the_last_notice() {
        let mut state = AutomationsState::default();
        state.set_notice(Some("Give the automation a name.".to_owned()));
        state.open_editor(Editor::New);
        assert_eq!(state.editor(), Some(&Editor::New));
        assert_eq!(state.notice(), None);
        state.close_editor();
        assert_eq!(state.editor(), None);
    }
}
