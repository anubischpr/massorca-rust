// The app's view of crates/client-ffi (decision 0080, section 1). Records are interfaces, enums are tagged unions
// (`tag` names the Rust variant), objects are interfaces with methods, `Option<T>` is `T | undefined`, and 64-bit
// integers are `bigint`. native.ts adapts the generated module to these names; the screens code against them only.

/** A host this phone is paired with, as the store lists it. */
export interface HostSummary {
  readonly name: string;
  /** The server's own name. */
  readonly server: string;
  readonly addresses: readonly string[];
  /** `sha256:<base64url>` of the server key this phone trusts. */
  readonly pin: string;
  readonly deviceName: string;
  readonly scopes: readonly string[];
  readonly pairedAtMs: bigint;
}

/** What a pairing offer says, without its secret. */
export interface OfferSummary {
  readonly server: string;
  /** Addresses the phone will try, `host:port`. */
  readonly addresses: readonly string[];
  readonly expiresAtMs: bigint;
  readonly scopes: readonly string[];
}

export interface WhoAmI {
  readonly deviceId: string | undefined;
  readonly deviceName: string | undefined;
  readonly scopes: readonly string[];
}

export interface ProtocolInfo {
  readonly protocol: number;
  readonly serverName: string;
  readonly serverVersion: string;
  readonly capabilities: readonly string[];
}

export interface WorkspaceSummary {
  readonly id: string;
  readonly projectId: string;
  readonly name: string;
  readonly path: string;
  readonly kind: string;
  readonly branch: string | undefined;
  readonly state: string;
  readonly missing: boolean;
}

/** A terminal of a workspace, as the list shows it. `state` is `live`, `unverifiable`, `exited` or `unknown`. */
export interface TerminalSummary {
  readonly id: string;
  readonly workspaceId: string;
  readonly state: string;
  readonly title: string | undefined;
  readonly cols: number;
  readonly rows: number;
  /** The program the terminal runs. */
  readonly shell: string;
  readonly cwd: string;
  readonly createdAtMs: bigint;
  /** Attaching needs host-admin scope, because the program got vault secrets. */
  readonly holdsSecrets: boolean;
}

export interface GitFileStatus {
  readonly path: string;
  readonly originalPath: string | undefined;
  readonly staged: string | undefined;
  readonly unstaged: string | undefined;
  readonly untracked: boolean;
  readonly conflict: string | undefined;
}

export interface GitStatusSummary {
  readonly branch: string | undefined;
  readonly ahead: bigint | undefined;
  readonly behind: bigint | undefined;
  readonly operation: string | undefined;
  readonly files: readonly GitFileStatus[];
  readonly truncated: boolean;
}

export interface AgentSessionSummary {
  readonly id: string;
  readonly kind: string;
  readonly workspaceId: string;
  readonly title: string | undefined;
  readonly activityState: string | undefined;
  readonly executionState: string;
  readonly updatedAtMs: bigint;
}

/** A team of the host. Its bots, projects and routines are read through it. */
export interface TeamSummary {
  readonly id: string;
  readonly name: string;
}

/** A bot of a team: the one that does the work of its routines. */
export interface BotSummary {
  readonly id: string;
  readonly teamId: string;
  readonly name: string;
  readonly lead: boolean;
}

/**
 * A routine of a team: a schedule that starts its bot on a task each time it is due. `cadence` is `daily`, `weekly`,
 * `monthly`, `once` or `unknown`; `weekday` is the day of a weekly routine, `monthDay` of a monthly one.
 */
export interface RoutineSummary {
  readonly id: string;
  readonly teamId: string;
  /** Absent once the routine's bot is removed. */
  readonly botId: string | undefined;
  readonly name: string;
  readonly cadence: string;
  /** The time of day, `HH:MM`, 24-hour. */
  readonly time: string;
  readonly weekday: string | undefined;
  readonly monthDay: number | undefined;
  /** `YYYY-MM-DD`, the date of a once routine. */
  readonly runDate: string | undefined;
  /** An IANA zone name; absent means the host's zone. */
  readonly zone: string | undefined;
  readonly paused: boolean;
  /** Absent while paused, and once a one-time routine has run. */
  readonly nextRunAtMs: bigint | undefined;
  readonly createdAtMs: bigint;
}

/** One run of a routine: an occurrence it claimed, or a run made now. Its task is absent when none was made. */
export interface RoutineRunSummary {
  readonly id: string;
  readonly routineId: string;
  readonly scheduledForMs: bigint;
  readonly taskId: string | undefined;
  /** Why the run or its task did not start, as a sentence. */
  readonly error: string | undefined;
  /** The protocol code of `error`, such as `bot.not_found`. */
  readonly errorCode: string | undefined;
  readonly createdAtMs: bigint;
}

export type AgentItem =
  | { readonly tag: 'Snapshot'; readonly sessions: readonly AgentSessionSummary[] }
  | { readonly tag: 'Changed'; readonly session: AgentSessionSummary };

/** Whether what a watch reports can be trusted right now. Loss of contact is never "stopped". */
export type LinkState =
  | { readonly tag: 'Live' }
  | {
      readonly tag: 'Unverifiable';
      /** The attempt that runs next, from 1. */
      readonly attempt: number;
      readonly retryInMs: bigint;
      readonly reason: string;
    };

export type AgentEvent = { readonly tag: 'Item'; readonly item: AgentItem } | { readonly tag: 'Link'; readonly link: LinkState };

export interface GitItem {
  /** True on the first item, which says the watch is set up. */
  readonly ready: boolean;
  readonly kinds: readonly string[];
  readonly degraded: boolean;
  readonly reason: string | undefined;
}

export type GitEvent = { readonly tag: 'Item'; readonly item: GitItem } | { readonly tag: 'Link'; readonly link: LinkState };

/** A pull-based stream: `next` resolves to the next event, or `null` once the watch is over. */
export interface Watch<E> {
  next(): Promise<E | null>;
  close(): void;
}

/** One step of a terminal view. Output comes in order, each run at the byte offset it starts at. */
export type TerminalEvent =
  | { readonly tag: 'Output'; readonly offset: bigint; readonly bytes: ArrayBuffer }
  /** The screen as the host holds it, from `offset` on: it opens the view or replaces output the view missed. */
  | {
      readonly tag: 'Snapshot';
      readonly offset: bigint;
      readonly cols: number;
      readonly rows: number;
      readonly bytes: ArrayBuffer;
    }
  | { readonly tag: 'Resized'; readonly cols: number; readonly rows: number }
  /** The process ended. The view ends after this event; `reason` is `exited`, `killed` or `service_stopped`. */
  | {
      readonly tag: 'Exited';
      readonly reason: string;
      readonly code: number | undefined;
      readonly signal: string | undefined;
    }
  | { readonly tag: 'Link'; readonly link: LinkState };

/** A live view of one terminal. `next` resolves to `null` once the view is over; `close` detaches. */
export interface TerminalView {
  next(): Promise<TerminalEvent | null>;
  /** Types `bytes` into the terminal. Fails while the host is out of contact. */
  input(bytes: ArrayBuffer): Promise<void>;
  resize(cols: number, rows: number): Promise<void>;
  close(): void;
}

/** One connection to a paired host. */
export interface Host {
  name(): string;
  whoami(): Promise<WhoAmI>;
  protocol(): ProtocolInfo;
  workspaces(project?: string): Promise<WorkspaceSummary[]>;
  /** The teams of the host, by name. */
  teams(): Promise<TeamSummary[]>;
  /** The bots of `team` (its id or exact name). */
  teamBots(team: string): Promise<BotSummary[]>;
  /** The routines of `team` (its id or exact name), in creation order. */
  teamRoutines(team: string): Promise<RoutineSummary[]>;
  /** The newest `limit` runs of a routine, newest first; the host's default when `limit` is absent. */
  routineRuns(team: string, routine: string, limit?: number): Promise<RoutineRunSummary[]>;
  pauseRoutine(team: string, routine: string): Promise<RoutineSummary>;
  resumeRoutine(team: string, routine: string): Promise<RoutineSummary>;
  /** Makes one run of a routine now; the routine's schedule does not move. */
  runRoutine(team: string, routine: string): Promise<RoutineRunSummary>;
  /** The terminals of every workspace, or only those of `workspace` (its id), live and exited. */
  terminals(workspace?: string): Promise<TerminalSummary[]>;
  gitStatus(workspace: string): Promise<GitStatusSummary>;
  watchGit(workspace: string): Watch<GitEvent>;
  watchAgents(workspace?: string): Watch<AgentEvent>;
  /** Attaches to `terminal` at `cols` by `rows`: the first event is its screen. A refusal fails this call. */
  attachTerminal(terminal: string, cols: number, rows: number): Promise<TerminalView>;
}

/** The flat error of the binding. Every case carries a sentence the UI may show as is. */
export type FfiError =
  | { readonly tag: 'Unreachable'; readonly message: string }
  | { readonly tag: 'IdentityMismatch'; readonly message: string }
  | { readonly tag: 'NotPaired'; readonly message: string }
  | {
      readonly tag: 'ProtocolMismatch';
      readonly message: string;
      readonly supportedMin: number | undefined;
      readonly supportedMax: number | undefined;
    }
  | { readonly tag: 'InvalidOffer'; readonly message: string }
  | { readonly tag: 'HostExists'; readonly message: string }
  | { readonly tag: 'HostNotFound'; readonly message: string }
  | { readonly tag: 'InvalidHostName'; readonly message: string }
  | { readonly tag: 'Remote'; readonly code: string; readonly message: string }
  | { readonly tag: 'Storage'; readonly message: string }
  | { readonly tag: 'Internal'; readonly message: string };

/** What every failing call throws: the flat error, as an `Error` so stacks and `instanceof` work. */
export class FfiException extends Error {
  readonly error: FfiError;

  constructor(error: FfiError, options?: ErrorOptions) {
    super(error.message, options);
    this.name = 'FfiException';
    this.error = error;
  }
}

/** The module-level API: `MassOrca` and `parse_offer` as the app calls them. */
export interface MassOrcaBinding {
  parseOffer(text: string): OfferSummary;
  hosts(): HostSummary[];
  pair(offer: string, hostName: string): Promise<HostSummary>;
  forget(hostName: string): void;
  connect(hostName: string): Promise<Host>;
}
