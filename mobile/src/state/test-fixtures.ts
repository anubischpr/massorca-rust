// Sample data for tests. Placeholders only: the repository is public, so no real address, user or secret appears here.
import type { AgentSessionSummary, HostSummary, WorkspaceSummary } from '../binding/types';

export function hostSummary(name: string = 'home-pc'): HostSummary {
  return {
    name,
    server: 'vps.example.ts.net',
    addresses: ['100.64.0.10:6768'],
    pin: 'sha256:fake',
    deviceName: 'fake-phone',
    scopes: [],
    pairedAtMs: 1n,
  };
}

export function agentSession(overrides: Partial<AgentSessionSummary> & { readonly id: string }): AgentSessionSummary {
  return {
    kind: 'claude',
    workspaceId: 'workspace-1',
    title: undefined,
    activityState: 'idle',
    executionState: 'live',
    updatedAtMs: 1n,
    ...overrides,
  };
}

export function workspace(overrides: Partial<WorkspaceSummary> & { readonly id: string }): WorkspaceSummary {
  return {
    projectId: 'a1b2c3d4-0000-7000-8000-000000000001',
    name: overrides.id,
    path: `/repos/${overrides.id}`,
    kind: 'worktree',
    branch: 'feat/example',
    state: 'ready',
    missing: false,
    ...overrides,
  };
}
