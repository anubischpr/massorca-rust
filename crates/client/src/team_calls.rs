//! Typed helpers over the `teams/1` methods: teams, their bots, projects and memory, the Board, and the tasks on it
//! (decision 0072).

use massorca_protocol::methods::tasks::{
    self, Task, TaskCreateParams, TaskList, TaskListParams, TaskRef, TaskUpdateParams,
};
use massorca_protocol::methods::teams::{
    self, Board, BoardParams, Bot, BotAddParams, BotList, BotRemoveParams, BotUpdateParams, Team,
    TeamCreateParams, TeamList, TeamMemoryAddParams, TeamMemoryLine, TeamMemoryList,
    TeamMemoryListParams, TeamMemoryRemoveParams, TeamProject, TeamProjectAddParams,
    TeamProjectList, TeamProjectRemoveParams, TeamProjectUpdateParams, TeamRef, TeamRemoveParams,
    TeamUpdateParams,
};

use crate::client::Client;
use crate::error::ClientError;

impl Client {
    /// `team.create`: creates a team.
    pub async fn team_create(&self, params: &TeamCreateParams) -> Result<Team, ClientError> {
        self.call(teams::CREATE, params).await
    }

    /// `team.list`: every team, ordered by name.
    pub async fn team_list(&self) -> Result<TeamList, ClientError> {
        self.call(teams::LIST, &()).await
    }

    /// `team.show`: one team, selected by id or name.
    pub async fn team_show(&self, params: &TeamRef) -> Result<Team, ClientError> {
        self.call(teams::SHOW, params).await
    }

    /// `team.update`: changes the fields given.
    pub async fn team_update(&self, params: &TeamUpdateParams) -> Result<Team, ClientError> {
        self.call(teams::UPDATE, params).await
    }

    /// `team.rm`: removes a team with its bots, projects and tasks; refused while a task runs unless forced.
    pub async fn team_remove(&self, params: &TeamRemoveParams) -> Result<Team, ClientError> {
        self.call(teams::REMOVE, params).await
    }

    /// `team.bot_list`: the bots of a team, in creation order.
    pub async fn team_bot_list(&self, params: &TeamRef) -> Result<BotList, ClientError> {
        self.call(teams::BOT_LIST, params).await
    }

    /// `team.bot_add`: adds a bot to a team.
    pub async fn team_bot_add(&self, params: &BotAddParams) -> Result<Bot, ClientError> {
        self.call(teams::BOT_ADD, params).await
    }

    /// `team.bot_update`: changes the fields of a bot that are given.
    pub async fn team_bot_update(&self, params: &BotUpdateParams) -> Result<Bot, ClientError> {
        self.call(teams::BOT_UPDATE, params).await
    }

    /// `team.bot_rm`: removes a bot; refused while it owns open tasks unless they are unassigned.
    pub async fn team_bot_remove(&self, params: &BotRemoveParams) -> Result<Bot, ClientError> {
        self.call(teams::BOT_REMOVE, params).await
    }

    /// `team.project_list`: the projects of a team, in creation order.
    pub async fn team_project_list(
        &self,
        params: &TeamRef,
    ) -> Result<TeamProjectList, ClientError> {
        self.call(teams::PROJECT_LIST, params).await
    }

    /// `team.project_add`: adds a project to a team.
    pub async fn team_project_add(
        &self,
        params: &TeamProjectAddParams,
    ) -> Result<TeamProject, ClientError> {
        self.call(teams::PROJECT_ADD, params).await
    }

    /// `team.project_update`: changes the fields of a project that are given.
    pub async fn team_project_update(
        &self,
        params: &TeamProjectUpdateParams,
    ) -> Result<TeamProject, ClientError> {
        self.call(teams::PROJECT_UPDATE, params).await
    }

    /// `team.project_rm`: removes a project; its tasks stay on the Board without it.
    pub async fn team_project_remove(
        &self,
        params: &TeamProjectRemoveParams,
    ) -> Result<TeamProject, ClientError> {
        self.call(teams::PROJECT_REMOVE, params).await
    }

    /// `team.board`: a team's Board, or one project's, in four columns.
    pub async fn team_board(&self, params: &BoardParams) -> Result<Board, ClientError> {
        self.call(teams::BOARD, params).await
    }

    /// `team.memory_list`: a team's memory lines, or one bot's, newest first.
    pub async fn team_memory_list(
        &self,
        params: &TeamMemoryListParams,
    ) -> Result<TeamMemoryList, ClientError> {
        self.call(teams::MEMORY_LIST, params).await
    }

    /// `team.memory_add`: adds a line to a team's memory, or to one bot's.
    pub async fn team_memory_add(
        &self,
        params: &TeamMemoryAddParams,
    ) -> Result<TeamMemoryLine, ClientError> {
        self.call(teams::MEMORY_ADD, params).await
    }

    /// `team.memory_rm`: removes a memory line of a team; the line is returned.
    pub async fn team_memory_remove(
        &self,
        params: &TeamMemoryRemoveParams,
    ) -> Result<TeamMemoryLine, ClientError> {
        self.call(teams::MEMORY_REMOVE, params).await
    }

    /// `task.create`: creates a task on a team's Board.
    pub async fn task_create(&self, params: &TaskCreateParams) -> Result<Task, ClientError> {
        self.call(tasks::CREATE, params).await
    }

    /// `task.list`: the tasks that match the filters given, oldest first.
    pub async fn task_list(&self, params: &TaskListParams) -> Result<TaskList, ClientError> {
        self.call(tasks::LIST, params).await
    }

    /// `task.show`: one task, selected by id.
    pub async fn task_show(&self, params: &TaskRef) -> Result<Task, ClientError> {
        self.call(tasks::SHOW, params).await
    }

    /// `task.update`: changes the fields of a task that are given.
    pub async fn task_update(&self, params: &TaskUpdateParams) -> Result<Task, ClientError> {
        self.call(tasks::UPDATE, params).await
    }

    /// `task.done`: marks a task done by hand; refused while its agent session runs.
    pub async fn task_done(&self, params: &TaskRef) -> Result<Task, ClientError> {
        self.call(tasks::DONE, params).await
    }

    /// `task.reopen`: clears a task's outcome and error; refused while its agent session runs.
    pub async fn task_reopen(&self, params: &TaskRef) -> Result<Task, ClientError> {
        self.call(tasks::REOPEN, params).await
    }

    /// `task.rm`: removes a task; refused while its agent session runs.
    pub async fn task_remove(&self, params: &TaskRef) -> Result<Task, ClientError> {
        self.call(tasks::REMOVE, params).await
    }
}
