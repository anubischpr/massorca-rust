import type * as Rust from '../generated/massorca_client_ffi';

import { createNativeBinding } from './native';
import type {
  RustAgentEvent,
  RustGitEvent,
  RustHost,
  RustMassOrca,
  RustModule,
  RustTerminalEvent,
  RustTerminalView,
  RustWatch,
} from './native-shapes';
import { FfiException, type AgentEvent, type GitEvent, type Host, type MassOrcaBinding, type TerminalEvent } from './types';

const offerRecord: Rust.OfferSummary = {
  server: 'vps.example.ts.net',
  addresses: ['100.64.0.10:7443'],
  expiresAtMs: 1_700_000_000_000n,
  scopes: ['sync'],
};

const hostRecord: Rust.HostSummary = {
  name: 'home-pc',
  server: 'vps.example.ts.net',
  addresses: ['100.64.0.10:7443'],
  pin: 'sha256:abc',
  deviceName: 'phone',
  scopes: ['sync'],
  pairedAtMs: 42n,
};

const workspaceRecord: Rust.WorkspaceSummary = {
  id: 'ws-1',
  projectId: 'p-1',
  name: 'massorca',
  path: '/work/massorca',
  kind: 'worktree',
  state: 'ready',
  missing: false,
};

const terminalRecord: Rust.TerminalSummary = {
  id: 'term-1',
  workspaceId: 'ws-1',
  state: 'live',
  title: 'build',
  cols: 120,
  rows: 40,
  shell: 'zsh',
  cwd: '/work/massorca',
  createdAtMs: 9n,
  holdsSecrets: false,
};

const exitedRecord: Rust.TerminalSummary = {
  id: 'term-2',
  workspaceId: 'ws-1',
  state: 'exited',
  title: undefined,
  cols: 80,
  rows: 24,
  shell: 'bash',
  cwd: '/work/massorca',
  createdAtMs: 3n,
  holdsSecrets: true,
};

const gitStatusRecord: Rust.GitStatusSummary = {
  branch: 'main',
  ahead: 2n,
  behind: undefined,
  operation: undefined,
  files: [
    {
      path: 'a.ts',
      originalPath: undefined,
      staged: undefined,
      unstaged: 'modified',
      untracked: false,
      conflict: undefined,
    },
  ],
  truncated: false,
};

const session: Rust.AgentSessionSummary = {
  id: 's-1',
  kind: 'claude',
  workspaceId: 'ws-1',
  title: 'Fix login',
  activityState: 'working',
  executionState: 'running',
  updatedAtMs: 7n,
};

const agentSnapshot: RustAgentEvent = {
  tag: 'Item',
  inner: { item: { tag: 'Snapshot', inner: { sessions: [session] } } },
};
const agentChanged: RustAgentEvent = {
  tag: 'Item',
  inner: { item: { tag: 'Changed', inner: { session: { ...session, executionState: 'exited' } } } },
};
const unverifiable: RustAgentEvent = {
  tag: 'Link',
  inner: { link: { tag: 'Unverifiable', inner: { attempt: 1, retryInMs: 2000n, reason: 'connection closed' } } },
};
const agentLive: RustAgentEvent = { tag: 'Link', inner: { link: { tag: 'Live' } } };

const gitItem: Rust.GitItem = { ready: true, kinds: ['worktree'], degraded: false, reason: undefined };
const gitChanged: RustGitEvent = { tag: 'Item', inner: { item: gitItem } };
const gitLive: RustGitEvent = { tag: 'Link', inner: { link: { tag: 'Live' } } };

const store: Rust.HostStore = {
  readHost: () => undefined,
  writeHost: () => undefined,
  removeHost: () => false,
  listHosts: () => [],
  seal: (plain) => plain,
  unseal: (sealed) => sealed,
};

/** An error shaped the way the generated module throws an FfiError: a tag, and the case's fields in `inner`. */
function generated(tag: string, inner: object): Error {
  return Object.assign(new Error(tag), { tag, inner });
}

/** Fails the next call of a method with the given error, once. */
class Failures {
  private readonly queued = new Map<string, unknown>();

  fail(method: string, error: unknown): void {
    this.queued.set(method, error);
  }

  check(method: string): void {
    if (!this.queued.has(method)) return;
    const error = this.queued.get(method);
    this.queued.delete(method);
    throw error;
  }
}

/** A watch that yields its script in order; an error in the script ends it. Closing it ends it too. */
class ScriptedWatch<E> implements RustWatch<E> {
  closed = false;
  private readonly script: (E | Error)[];

  constructor(script: readonly (E | Error)[]) {
    this.script = [...script];
  }

  async next(): Promise<E | undefined> {
    if (this.closed) return undefined;
    const step = this.script.shift();
    if (step === undefined) return undefined;
    if (step instanceof Error) throw step;
    return step;
  }

  close(): void {
    this.closed = true;
  }
}

/** A terminal view that yields its script in order; an error in the script ends it, and so does closing it. */
class ScriptedTerminal implements RustTerminalView {
  closed = false;
  readonly inputs: ArrayBuffer[] = [];
  readonly sizes: [number, number][] = [];
  private readonly script: (RustTerminalEvent | Error)[];

  constructor(script: readonly (RustTerminalEvent | Error)[]) {
    this.script = [...script];
  }

  async next(): Promise<RustTerminalEvent | undefined> {
    if (this.closed) return undefined;
    const step = this.script.shift();
    if (step === undefined) return undefined;
    if (step instanceof Error) throw step;
    return step;
  }

  async input(bytes: ArrayBuffer): Promise<void> {
    this.inputs.push(bytes);
  }

  async resize(cols: number, rows: number): Promise<void> {
    this.sizes.push([cols, rows]);
  }

  close(): void {
    this.closed = true;
  }
}

class FakeHost implements RustHost {
  readonly workspaceArgs: (string | undefined)[] = [];
  readonly terminalArgs: (string | undefined)[] = [];
  readonly gitStatusArgs: string[] = [];
  readonly gitWatchArgs: string[] = [];
  readonly agentWatchArgs: (string | undefined)[] = [];
  readonly gitWatches: ScriptedWatch<RustGitEvent>[] = [];
  readonly agentWatches: ScriptedWatch<RustAgentEvent>[] = [];
  readonly attachArgs: [string, number, number][] = [];
  readonly terminalViews: ScriptedTerminal[] = [];
  readonly opened: ScriptedTerminal[] = [];
  whoamiRecord: Rust.WhoAmI = { deviceId: 'device-1', deviceName: 'phone', scopes: ['sync'] };
  private readonly failures: Failures;

  constructor(failures: Failures) {
    this.failures = failures;
  }

  name(): string {
    return hostRecord.name;
  }

  async whoami(): Promise<Rust.WhoAmI> {
    this.failures.check('whoami');
    return this.whoamiRecord;
  }

  protocol(): Rust.ProtocolInfo {
    this.failures.check('protocol');
    return { protocol: 16, serverName: 'vps', serverVersion: '0.9.0', capabilities: ['terminal'] };
  }

  async workspaces(project: string | undefined): Promise<Rust.WorkspaceSummary[]> {
    this.failures.check('workspaces');
    this.workspaceArgs.push(project);
    return [workspaceRecord];
  }

  async terminals(workspace: string | undefined): Promise<Rust.TerminalSummary[]> {
    this.failures.check('terminals');
    this.terminalArgs.push(workspace);
    return [terminalRecord, exitedRecord];
  }

  async gitStatus(workspace: string): Promise<Rust.GitStatusSummary> {
    this.failures.check('gitStatus');
    this.gitStatusArgs.push(workspace);
    return gitStatusRecord;
  }

  watchGit(workspace: string): RustWatch<RustGitEvent> {
    this.failures.check('watchGit');
    this.gitWatchArgs.push(workspace);
    return this.gitWatches.shift() ?? new ScriptedWatch<RustGitEvent>([]);
  }

  watchAgents(workspace: string | undefined): RustWatch<RustAgentEvent> {
    this.failures.check('watchAgents');
    this.agentWatchArgs.push(workspace);
    return this.agentWatches.shift() ?? new ScriptedWatch<RustAgentEvent>([]);
  }

  async attachTerminal(terminal: string, cols: number, rows: number): Promise<RustTerminalView> {
    this.failures.check('attachTerminal');
    this.attachArgs.push([terminal, cols, rows]);
    const view = this.terminalViews.shift() ?? new ScriptedTerminal([]);
    this.opened.push(view);
    return view;
  }
}

class FakeClient implements RustMassOrca {
  readonly host: FakeHost;
  readonly forgotten: string[] = [];
  readonly constructed: [string, string][] = [];
  private readonly failures: Failures;

  constructor(failures: Failures, deviceName: string, appVersion: string) {
    this.failures = failures;
    this.host = new FakeHost(failures);
    this.constructed.push([deviceName, appVersion]);
  }

  hosts(): Rust.HostSummary[] {
    this.failures.check('hosts');
    return [hostRecord];
  }

  async pair(_offer: string, hostName: string): Promise<Rust.HostSummary> {
    this.failures.check('pair');
    return { ...hostRecord, name: hostName };
  }

  forget(hostName: string): void {
    this.failures.check('forget');
    this.forgotten.push(hostName);
  }

  async connect(_hostName: string): Promise<RustHost> {
    this.failures.check('connect');
    return this.host;
  }
}

interface Harness {
  readonly binding: MassOrcaBinding;
  readonly client: FakeClient;
  readonly failures: Failures;
}

/** A binding over a fake module; the fake client is the one the binding creates. */
function harness(): Harness {
  const failures = new Failures();
  const clients: FakeClient[] = [];
  class RecordingClient extends FakeClient {
    constructor(_store: Rust.HostStore, deviceName: string, appVersion: string) {
      super(failures, deviceName, appVersion);
      clients.push(this);
    }
  }
  const rust: RustModule = {
    parseOffer: (text) => {
      failures.check('parseOffer');
      if (!text.startsWith('massorca://')) {
        throw generated('InvalidOffer', { message: 'Not a valid pairing code, or it was already used.' });
      }
      return offerRecord;
    },
    MassOrca: RecordingClient,
  };
  const binding = createNativeBinding(rust, { store, deviceName: 'phone', appVersion: '1.2.3' });
  const client = clients[0];
  if (client === undefined) throw new Error('the binding did not create its client');
  return { binding, client, failures };
}

/** The exception a synchronous call throws; fails the test when it throws nothing or something else. */
function exceptionOf(call: () => unknown): FfiException {
  try {
    call();
  } catch (error) {
    if (error instanceof FfiException) return error;
    throw error;
  }
  throw new Error('expected the call to throw');
}

async function drain<E>(next: () => Promise<E | null>): Promise<E[]> {
  const events: E[] = [];
  for (let event = await next(); event !== null; event = await next()) events.push(event);
  return events;
}

describe('createNativeBinding', () => {
  it('creates the client with the device name and app version it is given', () => {
    const { client } = harness();

    expect(client.constructed).toEqual([['phone', '1.2.3']]);
  });

  it('parses a pairing code into the summary the app shows', () => {
    const { binding } = harness();

    expect(binding.parseOffer('massorca://pair')).toEqual({
      server: 'vps.example.ts.net',
      addresses: ['100.64.0.10:7443'],
      expiresAtMs: 1_700_000_000_000n,
      scopes: ['sync'],
    });
  });

  it('reports a code it cannot parse as an invalid offer', () => {
    const { binding } = harness();

    expect(exceptionOf(() => binding.parseOffer('nope')).error).toEqual({
      tag: 'InvalidOffer',
      message: 'Not a valid pairing code, or it was already used.',
    });
  });

  it('lists the paired hosts and forgets one by name', () => {
    const { binding, client } = harness();

    expect(binding.hosts()).toEqual([hostRecord]);
    binding.forget('home-pc');
    expect(client.forgotten).toEqual(['home-pc']);
  });

  it('pairs with an offer and returns the host it stored', async () => {
    const { binding } = harness();

    await expect(binding.pair('massorca://pair', 'office')).resolves.toEqual({ ...hostRecord, name: 'office' });
  });

  it('reads the name, who the phone is, the protocol and the workspaces of a connected host', async () => {
    const { binding, client } = harness();
    const host: Host = await binding.connect('home-pc');

    expect(host.name()).toBe('home-pc');
    expect(await host.whoami()).toEqual({ deviceId: 'device-1', deviceName: 'phone', scopes: ['sync'] });
    expect(host.protocol()).toEqual({
      protocol: 16,
      serverName: 'vps',
      serverVersion: '0.9.0',
      capabilities: ['terminal'],
    });
    expect(await host.workspaces('p-1')).toEqual([{ ...workspaceRecord, branch: undefined }]);
    expect(client.host.workspaceArgs).toEqual(['p-1']);
  });

  it('keeps absent whoami fields as undefined', async () => {
    const { binding, client } = harness();
    client.host.whoamiRecord = { scopes: [] };
    const host = await binding.connect('home-pc');

    expect(await host.whoami()).toEqual({ deviceId: undefined, deviceName: undefined, scopes: [] });
  });

  it('reads the git status of a workspace, with its counts as bigints', async () => {
    const { binding, client } = harness();
    const host = await binding.connect('home-pc');

    expect(await host.gitStatus('ws-1')).toEqual({
      branch: 'main',
      ahead: 2n,
      behind: undefined,
      operation: undefined,
      files: [
        {
          path: 'a.ts',
          originalPath: undefined,
          staged: undefined,
          unstaged: 'modified',
          untracked: false,
          conflict: undefined,
        },
      ],
      truncated: false,
    });
    expect(client.host.gitStatusArgs).toEqual(['ws-1']);
  });

  it('lists the terminals of one workspace, or of every workspace, with their state and title', async () => {
    const { binding, client } = harness();
    const host = await binding.connect('home-pc');

    expect(await host.terminals('ws-1')).toEqual([
      {
        id: 'term-1',
        workspaceId: 'ws-1',
        state: 'live',
        title: 'build',
        cols: 120,
        rows: 40,
        shell: 'zsh',
        cwd: '/work/massorca',
        createdAtMs: 9n,
        holdsSecrets: false,
      },
      {
        id: 'term-2',
        workspaceId: 'ws-1',
        state: 'exited',
        title: undefined,
        cols: 80,
        rows: 24,
        shell: 'bash',
        cwd: '/work/massorca',
        createdAtMs: 3n,
        holdsSecrets: true,
      },
    ]);
    await host.terminals();
    expect(client.host.terminalArgs).toEqual(['ws-1', undefined]);
  });

  it('reports a failed terminal list as the app error', async () => {
    const { binding, failures } = harness();
    failures.fail('terminals', generated('Unreachable', { message: 'home-pc is off' }));
    const host = await binding.connect('home-pc');

    await expect(host.terminals('ws-1')).rejects.toMatchObject({
      error: { tag: 'Unreachable', message: 'home-pc is off' },
    });
  });

  it('turns every agent event into the app shape and ends the watch with null', async () => {
    const { binding, client } = harness();
    const host = await binding.connect('home-pc');
    client.host.agentWatches.push(new ScriptedWatch([agentSnapshot, agentChanged, unverifiable, agentLive]));

    const watch = host.watchAgents();
    const events = await drain<AgentEvent>(() => watch.next());

    expect(events).toEqual([
      { tag: 'Item', item: { tag: 'Snapshot', sessions: [session] } },
      { tag: 'Item', item: { tag: 'Changed', session: { ...session, executionState: 'exited' } } },
      { tag: 'Link', link: { tag: 'Unverifiable', attempt: 1, retryInMs: 2000n, reason: 'connection closed' } },
      { tag: 'Link', link: { tag: 'Live' } },
    ]);
    expect(client.host.agentWatchArgs).toEqual([undefined]);
  });

  it('follows the git state of one workspace and turns its events into the app shape', async () => {
    const { binding, client } = harness();
    const host = await binding.connect('home-pc');
    client.host.gitWatches.push(new ScriptedWatch([gitChanged, gitLive]));

    const watch = host.watchGit('ws-1');
    const events = await drain<GitEvent>(() => watch.next());

    expect(events).toEqual([
      { tag: 'Item', item: { ready: true, kinds: ['worktree'], degraded: false, reason: undefined } },
      { tag: 'Link', link: { tag: 'Live' } },
    ]);
    expect(client.host.gitWatchArgs).toEqual(['ws-1']);
  });

  it('closes the watch it wraps when the app closes it', async () => {
    const { binding, client } = harness();
    const host = await binding.connect('home-pc');
    const scripted = new ScriptedWatch<RustAgentEvent>([agentSnapshot]);
    client.host.agentWatches.push(scripted);

    const watch = host.watchAgents();
    await watch.next();
    watch.close();

    expect(scripted.closed).toBe(true);
    await expect(watch.next()).resolves.toBeNull();
  });

  it('reports a failure of a watch as an FfiException', async () => {
    const { binding, client } = harness();
    const host = await binding.connect('home-pc');
    client.host.gitWatches.push(new ScriptedWatch<RustGitEvent>([generated('Unreachable', { message: 'gone' })]));

    await expect(host.watchGit('ws-1').next()).rejects.toBeInstanceOf(FfiException);
  });

  it('reports a failed sync call as the app error', () => {
    const { binding, failures } = harness();
    failures.fail('hosts', generated('Storage', { message: 'The store is locked.' }));

    expect(exceptionOf(() => binding.hosts()).error).toEqual({ tag: 'Storage', message: 'The store is locked.' });
  });

  it('reports a failed async call as the app error', async () => {
    const { binding, failures } = harness();
    failures.fail('connect', generated('HostNotFound', { message: 'no such host' }));

    await expect(binding.connect('missing')).rejects.toMatchObject({
      error: { tag: 'HostNotFound', message: 'no such host' },
    });
  });
});

function bytesOf(text: string): ArrayBuffer {
  return new TextEncoder().encode(text).buffer;
}

/** An event with its bytes read as text, so the expectations stay readable. */
function readable(event: TerminalEvent): unknown {
  if (event.tag === 'Output' || event.tag === 'Snapshot') {
    return { ...event, bytes: new TextDecoder().decode(event.bytes) };
  }
  return event;
}

describe('terminal attach', () => {
  it('attaches at the size it is given and turns each step into the app shape', async () => {
    const { binding, client } = harness();
    client.host.terminalViews.push(
      new ScriptedTerminal([
        { tag: 'Snapshot', inner: { offset: 0n, cols: 80, rows: 24, bytes: bytesOf('$ ') } },
        { tag: 'Output', inner: { offset: 2n, bytes: bytesOf('ls\r\n') } },
        { tag: 'Resized', inner: { cols: 100, rows: 30 } },
        { tag: 'Link', inner: { link: { tag: 'Unverifiable', inner: { attempt: 1, retryInMs: 2000n, reason: 'gone' } } } },
        { tag: 'Exited', inner: { reason: 'exited', code: 0n, signal: undefined } },
      ]),
    );
    const host = await binding.connect('home-pc');

    const view = await host.attachTerminal('term-1', 80, 24);
    const events = await drain(() => view.next());

    expect(client.host.attachArgs).toEqual([['term-1', 80, 24]]);
    expect(events.map(readable)).toEqual([
      { tag: 'Snapshot', offset: 0n, cols: 80, rows: 24, bytes: '$ ' },
      { tag: 'Output', offset: 2n, bytes: 'ls\r\n' },
      { tag: 'Resized', cols: 100, rows: 30 },
      { tag: 'Link', link: { tag: 'Unverifiable', attempt: 1, retryInMs: 2000n, reason: 'gone' } },
      { tag: 'Exited', reason: 'exited', code: 0, signal: undefined },
    ]);
  });

  it('reads an exit by signal, with no exit code', async () => {
    const { binding, client } = harness();
    client.host.terminalViews.push(
      new ScriptedTerminal([{ tag: 'Exited', inner: { reason: 'killed', code: undefined, signal: 'SIGKILL' } }]),
    );
    const view = await (await binding.connect('home-pc')).attachTerminal('term-1', 80, 24);

    expect(await view.next()).toEqual({ tag: 'Exited', reason: 'killed', code: undefined, signal: 'SIGKILL' });
  });

  it('sends input and resizes to the view, and closing it detaches it', async () => {
    const { binding, client } = harness();
    const view = await (await binding.connect('home-pc')).attachTerminal('term-1', 80, 24);
    const [terminal] = client.host.opened;

    await view.input(bytesOf('a'));
    await view.resize(100, 30);
    view.close();

    expect(terminal?.inputs.map((bytes) => new TextDecoder().decode(bytes))).toEqual(['a']);
    expect(terminal?.sizes).toEqual([[100, 30]]);
    expect(terminal?.closed).toBe(true);
    expect(await view.next()).toBeNull();
  });

  it('reports a refused attach as the app error', async () => {
    const { binding, failures } = harness();
    failures.fail('attachTerminal', generated('Remote', { code: 'terminal.not_found', message: 'No such terminal.' }));
    const host = await binding.connect('home-pc');

    await expect(host.attachTerminal('gone', 80, 24)).rejects.toMatchObject({
      error: { tag: 'Remote', code: 'terminal.not_found', message: 'No such terminal.' },
    });
  });

  it('reports a failure of the view as an FfiException', async () => {
    const { binding, client } = harness();
    client.host.terminalViews.push(new ScriptedTerminal([generated('Unreachable', { message: 'gone' })]));
    const view = await (await binding.connect('home-pc')).attachTerminal('term-1', 80, 24);

    await expect(view.next()).rejects.toBeInstanceOf(FfiException);
  });
});
