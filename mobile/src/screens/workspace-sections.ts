import type { AgentSessionSummary, WorkspaceSummary } from '../binding/types';
import { projectHeading } from '../copy/workspaces';
import { aggregateDisplay } from '../state/agent-display';
import { glyphViewOf, type GlyphView } from './agent-display-view';

export interface WorkspaceRow {
  readonly id: string;
  readonly name: string;
  /** A plain folder, not a git worktree (screen M-WS-05). */
  readonly noGit: boolean;
  readonly agent: GlyphView | undefined;
}

export interface WorkspaceSection {
  readonly key: string;
  readonly title: string;
  readonly data: readonly WorkspaceRow[];
}

/** Workspaces grouped by project, in the order the host lists them; each row carries its aggregate agent state. */
export function workspaceSections(
  workspaces: readonly WorkspaceSummary[],
  sessions: readonly AgentSessionSummary[],
  linkLost: boolean,
  age: string | undefined,
): readonly WorkspaceSection[] {
  const groups = new Map<string, WorkspaceRow[]>();
  for (const workspace of workspaces) {
    const own = sessions.filter((session) => session.workspaceId === workspace.id);
    const row: WorkspaceRow = {
      id: workspace.id,
      name: workspace.name,
      noGit: workspace.kind === 'folder',
      agent: glyphViewOf(aggregateDisplay(own, linkLost), age),
    };
    groups.set(workspace.projectId, [...(groups.get(workspace.projectId) ?? []), row]);
  }
  return Array.from(groups, ([projectId, data]) => ({ key: projectId, title: projectHeading(projectId), data }));
}
