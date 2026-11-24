export const PROJECTS_TITLE = 'Projects';
export const LOADING_WORKSPACES = 'Loading workspaces';
export const NO_WORKSPACES_TITLE = 'No workspaces yet';
export const NO_WORKSPACES_LINE = 'A workspace is a git worktree or a folder where an agent works.';
export const NO_GIT_BADGE = 'No Git';
export const ON_HOST = (host: string): string => `On ${host}`;
export const workspacesLoadFailed = (host: string): string => `Could not load workspaces from ${host}`;

/** The project name is not in the binding yet, so the heading shows the start of the project id. */
export const projectHeading = (projectId: string): string => `Project ${projectId.slice(0, 8)}`;
