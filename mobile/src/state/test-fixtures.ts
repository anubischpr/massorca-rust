// Sample data for tests. Placeholders only: the repository is public, so no real address, user or secret appears here.
import { FakeMassOrca, type FakeOptions } from '../binding/fake';
import {
  FfiException,
  type AgentEvent,
  type AgentSessionSummary,
  type GitEvent,
  type GitStatusSummary,
  type Host,
  type HostSummary,
  type ProtocolInfo,
  type TerminalSummary,
  type TerminalView,
  type WhoAmI,
  type Watch,
  type WorkspaceSummary,
} from '../binding/types';

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

export function terminalSummary(overrides: Partial<TerminalSummary> & { readonly id: string }): TerminalSummary {
  return {
    workspaceId: 'ws-1',
    state: 'live',
    title: undefined,
    cols: 120,
    rows: 40,
    shell: 'zsh',
    cwd: '/repos/ws-1',
    createdAtMs: 1n,
    holdsSecrets: false,
    ...overrides,
  };
}

/** A fake whose connect refuses with a protocol mismatch for the first `refusals` attempts, then connects. */
export class ProtocolRefusingFake extends FakeMassOrca {
  private refusals: number;

  constructor(
    private readonly range: { readonly min?: number; readonly max?: number },
    refusals: number = Number.POSITIVE_INFINITY,
    options: FakeOptions = { hosts: [hostSummary()] },
  ) {
    super(options);
    this.refusals = refusals;
  }

  override async connect(hostName: string): Promise<Host> {
    if (this.refusals > 0) {
      this.refusals -= 1;
      throw new FfiException({
        tag: 'ProtocolMismatch',
        message: `${hostName} refused the connection.`,
        supportedMin: this.range.min,
        supportedMax: this.range.max,
      });
    }
    return super.connect(hostName);
  }
}

/** A host whose workspace list never arrives, for the loading state. */
class StalledHost implements Host {
  constructor(private readonly inner: Host) {}

  name(): string {
    return this.inner.name();
  }

  whoami(): Promise<WhoAmI> {
    return this.inner.whoami();
  }

  protocol(): ProtocolInfo {
    return this.inner.protocol();
  }

  workspaces(): Promise<WorkspaceSummary[]> {
    return new Promise<WorkspaceSummary[]>(() => undefined);
  }

  terminals(workspaceId?: string): Promise<TerminalSummary[]> {
    return this.inner.terminals(workspaceId);
  }

  gitStatus(workspaceId: string): Promise<GitStatusSummary> {
    return this.inner.gitStatus(workspaceId);
  }

  watchGit(workspaceId: string): Watch<GitEvent> {
    return this.inner.watchGit(workspaceId);
  }

  watchAgents(workspaceId?: string): Watch<AgentEvent> {
    return this.inner.watchAgents(workspaceId);
  }

  attachTerminal(terminal: string, cols: number, rows: number): Promise<TerminalView> {
    return this.inner.attachTerminal(terminal, cols, rows);
  }
}

export class StalledWorkspacesFake extends FakeMassOrca {
  override async connect(hostName: string): Promise<Host> {
    return new StalledHost(await super.connect(hostName));
  }
}

/** A host whose git status never arrives, for the loading state of the changes. */
class StalledStatusHost implements Host {
  constructor(private readonly inner: Host) {}

  name(): string {
    return this.inner.name();
  }

  whoami(): Promise<WhoAmI> {
    return this.inner.whoami();
  }

  protocol(): ProtocolInfo {
    return this.inner.protocol();
  }

  workspaces(project?: string): Promise<WorkspaceSummary[]> {
    return this.inner.workspaces(project);
  }

  terminals(workspaceId?: string): Promise<TerminalSummary[]> {
    return this.inner.terminals(workspaceId);
  }

  gitStatus(): Promise<GitStatusSummary> {
    return new Promise<GitStatusSummary>(() => undefined);
  }

  watchGit(workspaceId: string): Watch<GitEvent> {
    return this.inner.watchGit(workspaceId);
  }

  watchAgents(workspaceId?: string): Watch<AgentEvent> {
    return this.inner.watchAgents(workspaceId);
  }

  attachTerminal(terminal: string, cols: number, rows: number): Promise<TerminalView> {
    return this.inner.attachTerminal(terminal, cols, rows);
  }
}

export class StalledGitStatusFake extends FakeMassOrca {
  override async connect(hostName: string): Promise<Host> {
    return new StalledStatusHost(await super.connect(hostName));
  }
}

/** A host whose terminal list never arrives, for the loading state of the Terminal view. */
class StalledTerminalsHost implements Host {
  constructor(private readonly inner: Host) {}

  name(): string {
    return this.inner.name();
  }

  whoami(): Promise<WhoAmI> {
    return this.inner.whoami();
  }

  protocol(): ProtocolInfo {
    return this.inner.protocol();
  }

  workspaces(project?: string): Promise<WorkspaceSummary[]> {
    return this.inner.workspaces(project);
  }

  terminals(): Promise<TerminalSummary[]> {
    return new Promise<TerminalSummary[]>(() => undefined);
  }

  gitStatus(workspaceId: string): Promise<GitStatusSummary> {
    return this.inner.gitStatus(workspaceId);
  }

  watchGit(workspaceId: string): Watch<GitEvent> {
    return this.inner.watchGit(workspaceId);
  }

  watchAgents(workspaceId?: string): Watch<AgentEvent> {
    return this.inner.watchAgents(workspaceId);
  }

  attachTerminal(terminal: string, cols: number, rows: number): Promise<TerminalView> {
    return this.inner.attachTerminal(terminal, cols, rows);
  }
}

export class StalledTerminalsFake extends FakeMassOrca {
  override async connect(hostName: string): Promise<Host> {
    return new StalledTerminalsHost(await super.connect(hostName));
  }
}
