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

  gitStatus(workspaceId: string): Promise<GitStatusSummary> {
    return this.inner.gitStatus(workspaceId);
  }

  watchGit(workspaceId: string): Watch<GitEvent> {
    return this.inner.watchGit(workspaceId);
  }

  watchAgents(workspaceId?: string): Watch<AgentEvent> {
    return this.inner.watchAgents(workspaceId);
  }
}

export class StalledWorkspacesFake extends FakeMassOrca {
  override async connect(hostName: string): Promise<Host> {
    return new StalledHost(await super.connect(hostName));
  }
}
