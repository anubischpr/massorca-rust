// An in-memory stand-in for the binding, scripted by tests: hosts, pair results, failures by error code, the
// events each watch and terminal view delivers, and the git status a host reports. It has no network and no
// storage; it never talks to a real host.

import { errorFromCode } from './ffi-error';
import {
  FfiException,
  type AgentEvent,
  type BotSummary,
  type FfiError,
  type GitEvent,
  type GitStatusSummary,
  type Host,
  type HostSummary,
  type MassOrcaBinding,
  type OfferSummary,
  type ProtocolInfo,
  type RoutineRunSummary,
  type RoutineSummary,
  type TeamSummary,
  type TerminalEvent,
  type TerminalSummary,
  type TerminalView,
  type WhoAmI,
  type Watch,
  type WorkspaceSummary,
} from './types';

/** The calls the fake records and can fail, by name. */
export type FakeCall =
  | 'parseOffer'
  | 'hosts'
  | 'pair'
  | 'forget'
  | 'connect'
  | 'whoami'
  | 'protocol'
  | 'workspaces'
  | 'teams'
  | 'teamBots'
  | 'teamRoutines'
  | 'routineRuns'
  | 'pauseRoutine'
  | 'resumeRoutine'
  | 'runRoutine'
  | 'terminals'
  | 'gitStatus'
  | 'watchAgents'
  | 'watchGit'
  | 'attachTerminal';

/** One attach as the app asked for it. */
export interface FakeAttach {
  readonly terminal: string;
  readonly cols: number;
  readonly rows: number;
}

/** The only offer text the fake accepts. Anything else is an invalid offer, as the real parser refuses it. */
export const FAKE_OFFER_PREFIX = 'fake-offer:';

export interface FakeOptions {
  readonly hosts?: readonly HostSummary[];
  readonly protocol?: ProtocolInfo;
  readonly whoami?: WhoAmI;
  readonly workspaces?: readonly WorkspaceSummary[];
  readonly terminals?: readonly TerminalSummary[];
  readonly teams?: readonly TeamSummary[];
  readonly bots?: readonly BotSummary[];
  readonly routines?: readonly RoutineSummary[];
  readonly routineRuns?: readonly RoutineRunSummary[];
}

interface FakeState {
  readonly calls: FakeCall[];
  readonly hosts: HostSummary[];
  readonly failures: Map<FakeCall, FfiError[]>;
  readonly pairResults: (HostSummary | FfiError)[];
  readonly agentWatches: AgentEvent[][];
  readonly gitWatches: GitEvent[][];
  readonly terminalScripts: TerminalEvent[][];
  readonly attaches: FakeAttach[];
  readonly terminalViews: FakeTerminalView[];
  readonly protocol: ProtocolInfo;
  readonly whoami: WhoAmI;
  readonly workspaces: readonly WorkspaceSummary[];
  readonly terminals: readonly TerminalSummary[];
  readonly teams: readonly TeamSummary[];
  readonly bots: readonly BotSummary[];
  /** Replaced as routines are paused, resumed and read; the order is the creation order. */
  readonly routines: RoutineSummary[];
  /** Newest first, as the host lists them. */
  readonly routineRuns: RoutineRunSummary[];
  /** Calls whose next run never answers, for the loading state. */
  readonly stalls: Set<FakeCall>;
  gitStatus: GitStatusSummary;
}

/** The next run the fake gives a resumed routine: a fixed instant in 2100, so tests do not depend on the clock. */
const FAKE_NEXT_RUN_MS = 4102444800000n;
const FAKE_CREATED_MS = 1n;
const FAKE_RUN_DEFAULT_LIMIT = 50;

const DEFAULT_PROTOCOL: ProtocolInfo = {
  protocol: 16,
  serverName: 'fake-host',
  serverVersion: '0.0.0-fake',
  capabilities: [],
};

const DEFAULT_WHOAMI: WhoAmI = { deviceId: 'fake-device', deviceName: 'fake-phone', scopes: [] };

const EMPTY_GIT_STATUS: GitStatusSummary = {
  branch: undefined,
  ahead: undefined,
  behind: undefined,
  operation: undefined,
  files: [],
  truncated: false,
};

/** Records the call, then fails it when a failure was scripted for it. */
function enter(state: FakeState, call: FakeCall): void {
  state.calls.push(call);
  const failure = state.failures.get(call)?.shift();
  if (failure !== undefined) throw new FfiException(failure);
}

function isFfiError(result: HostSummary | FfiError): result is FfiError {
  return 'tag' in result;
}

function readOffer(text: string): OfferSummary {
  if (!text.startsWith(FAKE_OFFER_PREFIX)) {
    throw new FfiException({ tag: 'InvalidOffer', message: 'Not a valid pairing code.' });
  }
  return {
    server: 'vps.example.ts.net',
    addresses: ['100.64.0.10:7443'],
    expiresAtMs: 4102444800000n,
    scopes: ['workspaces:read'],
  };
}

/** The team `selector` names, by id or exact name, as the host answers it. */
function teamOf(state: FakeState, selector: string): TeamSummary {
  const team = state.teams.find((candidate) => candidate.id === selector || candidate.name === selector);
  if (team === undefined) {
    throw new FfiException({ tag: 'Remote', code: 'team.not_found', message: `No team named ${selector}.` });
  }
  return team;
}

/** The routine `selector` names within `team`, by id or exact name. */
function routineOf(state: FakeState, team: TeamSummary, selector: string): RoutineSummary {
  const routine = state.routines.find(
    (candidate) => candidate.teamId === team.id && (candidate.id === selector || candidate.name === selector),
  );
  if (routine === undefined) {
    throw new FfiException({
      tag: 'Remote',
      code: 'team.routine_not_found',
      message: `Team ${team.name} has no routine ${selector}.`,
    });
  }
  return routine;
}

/** Pauses or resumes a routine, as the host does: a paused routine has no next run, a resumed one has one. */
function setPaused(state: FakeState, team: string, routine: string, paused: boolean): RoutineSummary {
  const current = routineOf(state, teamOf(state, team), routine);
  const next: RoutineSummary = { ...current, paused, nextRunAtMs: paused ? undefined : FAKE_NEXT_RUN_MS };
  state.routines.splice(state.routines.indexOf(current), 1, next);
  return next;
}

/** A pull-based watch over a scripted list; it ends (returns null) when the list runs out or it is closed. */
class FakeWatch<E> implements Watch<E> {
  private closed = false;
  private readonly events: E[];

  constructor(events: readonly E[]) {
    this.events = [...events];
  }

  async next(): Promise<E | null> {
    if (this.closed) return null;
    return this.events.shift() ?? null;
  }

  close(): void {
    this.closed = true;
    this.events.length = 0;
  }
}

/**
 * A terminal view the test drives: it delivers its script and then the events the test pushes, and it stays open
 * until closed or until its Exited event is read (the real view ends after that event).
 */
export class FakeTerminalView implements TerminalView {
  readonly inputs: ArrayBuffer[] = [];
  readonly resizes: [number, number][] = [];
  closed = false;
  private readonly queue: TerminalEvent[];
  private waiting: ((event: TerminalEvent | null) => void) | undefined;
  private over = false;

  constructor(events: readonly TerminalEvent[]) {
    this.queue = [...events];
  }

  /** Delivers `event` to the read waiting now, or queues it behind the events not read yet. */
  push(event: TerminalEvent): void {
    if (this.closed || this.over) return;
    const resolve = this.waiting;
    if (resolve === undefined) {
      this.queue.push(event);
      return;
    }
    this.waiting = undefined;
    resolve(this.take(event));
  }

  async next(): Promise<TerminalEvent | null> {
    if (this.closed || this.over) return null;
    const event = this.queue.shift();
    if (event !== undefined) return this.take(event);
    return new Promise<TerminalEvent | null>((resolve) => {
      this.waiting = resolve;
    });
  }

  async input(bytes: ArrayBuffer): Promise<void> {
    if (this.closed) throw new FfiException({ tag: 'Internal', message: 'This terminal view is closed.' });
    this.inputs.push(bytes);
  }

  async resize(cols: number, rows: number): Promise<void> {
    if (this.closed) throw new FfiException({ tag: 'Internal', message: 'This terminal view is closed.' });
    this.resizes.push([cols, rows]);
  }

  close(): void {
    this.closed = true;
    const resolve = this.waiting;
    this.waiting = undefined;
    resolve?.(null);
  }

  private take(event: TerminalEvent): TerminalEvent {
    if (event.tag === 'Exited') this.over = true;
    return event;
  }
}

class FakeHost implements Host {
  constructor(
    private readonly hostName: string,
    private readonly state: FakeState,
  ) {}

  name(): string {
    return this.hostName;
  }

  async whoami(): Promise<WhoAmI> {
    enter(this.state, 'whoami');
    return this.state.whoami;
  }

  protocol(): ProtocolInfo {
    enter(this.state, 'protocol');
    return this.state.protocol;
  }

  async workspaces(project?: string): Promise<WorkspaceSummary[]> {
    enter(this.state, 'workspaces');
    return this.state.workspaces.filter((w) => project === undefined || w.projectId === project);
  }

  async teams(): Promise<TeamSummary[]> {
    enter(this.state, 'teams');
    if (this.state.stalls.delete('teams')) return new Promise<TeamSummary[]>(() => undefined);
    return [...this.state.teams];
  }

  async teamBots(team: string): Promise<BotSummary[]> {
    enter(this.state, 'teamBots');
    const id = teamOf(this.state, team).id;
    return this.state.bots.filter((bot) => bot.teamId === id);
  }

  async teamRoutines(team: string): Promise<RoutineSummary[]> {
    enter(this.state, 'teamRoutines');
    const id = teamOf(this.state, team).id;
    return this.state.routines.filter((routine) => routine.teamId === id);
  }

  async routineRuns(team: string, routine: string, limit?: number): Promise<RoutineRunSummary[]> {
    enter(this.state, 'routineRuns');
    const id = routineOf(this.state, teamOf(this.state, team), routine).id;
    return this.state.routineRuns
      .filter((run) => run.routineId === id)
      .slice(0, limit ?? FAKE_RUN_DEFAULT_LIMIT);
  }

  async pauseRoutine(team: string, routine: string): Promise<RoutineSummary> {
    enter(this.state, 'pauseRoutine');
    return setPaused(this.state, team, routine, true);
  }

  async resumeRoutine(team: string, routine: string): Promise<RoutineSummary> {
    enter(this.state, 'resumeRoutine');
    return setPaused(this.state, team, routine, false);
  }

  async runRoutine(team: string, routine: string): Promise<RoutineRunSummary> {
    enter(this.state, 'runRoutine');
    const id = routineOf(this.state, teamOf(this.state, team), routine).id;
    const sequence = this.state.routineRuns.length + 1;
    const run: RoutineRunSummary = {
      id: `fake-run-${sequence}`,
      routineId: id,
      scheduledForMs: FAKE_CREATED_MS,
      taskId: `fake-task-${sequence}`,
      error: undefined,
      errorCode: undefined,
      createdAtMs: FAKE_CREATED_MS,
    };
    this.state.routineRuns.unshift(run);
    return run;
  }

  async terminals(workspace?: string): Promise<TerminalSummary[]> {
    enter(this.state, 'terminals');
    return this.state.terminals.filter((t) => workspace === undefined || t.workspaceId === workspace);
  }

  async gitStatus(_workspace: string): Promise<GitStatusSummary> {
    enter(this.state, 'gitStatus');
    return this.state.gitStatus;
  }

  watchGit(_workspace: string): Watch<GitEvent> {
    enter(this.state, 'watchGit');
    return new FakeWatch(this.state.gitWatches.shift() ?? []);
  }

  watchAgents(_workspace?: string): Watch<AgentEvent> {
    enter(this.state, 'watchAgents');
    return new FakeWatch(this.state.agentWatches.shift() ?? []);
  }

  async attachTerminal(terminal: string, cols: number, rows: number): Promise<TerminalView> {
    enter(this.state, 'attachTerminal');
    this.state.attaches.push({ terminal, cols, rows });
    const view = new FakeTerminalView(this.state.terminalScripts.shift() ?? []);
    this.state.terminalViews.push(view);
    return view;
  }
}

export class FakeMassOrca implements MassOrcaBinding {
  private readonly state: FakeState;

  constructor(options: FakeOptions = {}) {
    this.state = {
      calls: [],
      hosts: [...(options.hosts ?? [])],
      failures: new Map(),
      pairResults: [],
      agentWatches: [],
      gitWatches: [],
      terminalScripts: [],
      attaches: [],
      terminalViews: [],
      protocol: options.protocol ?? DEFAULT_PROTOCOL,
      whoami: options.whoami ?? DEFAULT_WHOAMI,
      workspaces: options.workspaces ?? [],
      terminals: options.terminals ?? [],
      teams: options.teams ?? [],
      bots: options.bots ?? [],
      routines: [...(options.routines ?? [])],
      routineRuns: [...(options.routineRuns ?? [])],
      stalls: new Set(),
      gitStatus: EMPTY_GIT_STATUS,
    };
  }

  /** Every call made so far, in order. */
  get calls(): readonly FakeCall[] {
    return this.state.calls;
  }

  /** Every attach the app asked for, in order. */
  get attaches(): readonly FakeAttach[] {
    return this.state.attaches;
  }

  /** Every terminal view opened so far, in order. */
  get terminals(): readonly FakeTerminalView[] {
    return this.state.terminalViews;
  }

  /** The events of the next terminal view: it delivers them in order, then stays open until it is closed. */
  scriptTerminal(events: readonly TerminalEvent[]): void {
    this.state.terminalScripts.push([...events]);
  }

  /** The next `teams` call never answers, so the list stays in its loading state. */
  stallNextTeams(): void {
    this.state.stalls.add('teams');
  }

  /** The git status that every read of a workspace returns from now on. */
  setGitStatus(status: GitStatusSummary): void {
    this.state.gitStatus = status;
  }

  /** The next call of `call` fails with the case that `code` names (see errorFromCode); later calls succeed. */
  failNext(call: FakeCall, code: string, message: string = `fake ${code}`): void {
    const queue = this.state.failures.get(call) ?? [];
    queue.push(errorFromCode(code, message));
    this.state.failures.set(call, queue);
  }

  /** The next `pair` returns this host, or fails with this error. Without a script, `pair` succeeds. */
  queuePair(result: HostSummary | FfiError): void {
    this.state.pairResults.push(result);
  }

  /** The events of the next agent watch, in order; after the last one the watch ends. */
  scriptAgentWatch(events: readonly AgentEvent[]): void {
    this.state.agentWatches.push([...events]);
  }

  /** The events of the next git watch, in order; after the last one the watch ends. */
  scriptGitWatch(events: readonly GitEvent[]): void {
    this.state.gitWatches.push([...events]);
  }

  parseOffer(text: string): OfferSummary {
    enter(this.state, 'parseOffer');
    return readOffer(text);
  }

  hosts(): HostSummary[] {
    enter(this.state, 'hosts');
    return [...this.state.hosts];
  }

  async pair(offer: string, hostName: string): Promise<HostSummary> {
    enter(this.state, 'pair');
    const scripted = this.state.pairResults.shift();
    if (scripted !== undefined && isFfiError(scripted)) throw new FfiException(scripted);
    if (scripted !== undefined) {
      this.state.hosts.push(scripted);
      return scripted;
    }
    const summary = readOffer(offer);
    if (hostName.trim() === '') {
      throw new FfiException({ tag: 'InvalidHostName', message: 'Give the host a name.' });
    }
    if (this.state.hosts.some((h) => h.name === hostName)) {
      throw new FfiException({ tag: 'HostExists', message: `A host named ${hostName} is already paired.` });
    }
    const host: HostSummary = {
      name: hostName,
      server: summary.server,
      addresses: summary.addresses,
      pin: 'sha256:fake',
      deviceName: 'fake-phone',
      scopes: summary.scopes,
      pairedAtMs: 0n,
    };
    this.state.hosts.push(host);
    return host;
  }

  forget(hostName: string): void {
    enter(this.state, 'forget');
    const index = this.state.hosts.findIndex((h) => h.name === hostName);
    if (index === -1) throw new FfiException({ tag: 'HostNotFound', message: `No host named ${hostName}.` });
    this.state.hosts.splice(index, 1);
  }

  async connect(hostName: string): Promise<Host> {
    enter(this.state, 'connect');
    if (!this.state.hosts.some((h) => h.name === hostName)) {
      throw new FfiException({ tag: 'HostNotFound', message: `No host named ${hostName}.` });
    }
    return new FakeHost(hostName, this.state);
  }
}
