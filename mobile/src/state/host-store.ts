import { asyncIterate } from '../binding/watch-iterator';
import { FfiException, type AgentEvent, type FfiError, type Host, type MassOrcaBinding, type Watch } from '../binding/types';
import { applyAgentEvent, INITIAL_AGENT_WATCH, type AgentWatchState } from './agent-watch-state';
import { appendLogLine, errorLogText, linkLogText, type LogLine } from './connection-log';

export type HostConnection =
  | { readonly status: 'connecting' }
  | { readonly status: 'connected'; readonly host: Host }
  | { readonly status: 'failed'; readonly error: FfiError };

/** What the screens know about one paired host. Immutable: every change replaces the record. */
export interface HostRecord {
  readonly name: string;
  readonly connection: HostConnection;
  /** The agent watch of the current connection; after a failure it stays as the last known state. */
  readonly agents: AgentWatchState | undefined;
  readonly log: readonly LogLine[];
}

/**
 * One connection and one agent watch per paired host, plus the connection log, for the whole app.
 * Screens read it through useSyncExternalStore; the binding is only reached through its own seams.
 */
export class HostStore {
  private records: ReadonlyMap<string, HostRecord> = new Map();
  private readonly listeners = new Set<() => void>();
  private readonly generations = new Map<string, number>();
  private readonly watches = new Map<string, Watch<AgentEvent>>();

  constructor(
    private readonly binding: MassOrcaBinding,
    private readonly now: () => number = () => Date.now(),
  ) {}

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  readonly getRecord = (name: string): HostRecord | undefined => this.records.get(name);

  /** Connects, unless a connection is being made or is open already. */
  ensure(name: string): Promise<void> {
    const status = this.records.get(name)?.connection.status;
    return status === 'connecting' || status === 'connected' ? Promise.resolve() : this.retry(name);
  }

  /** Connects again, closing the agent watch of the previous connection. Resolves once the attempt is decided. */
  retry(name: string): Promise<void> {
    const generation = this.nextGeneration(name);
    this.closeWatch(name);
    this.patch(name, { connection: { status: 'connecting' } }, `Connecting to ${name}`);
    return this.open(name, generation);
  }

  dispose(): void {
    for (const name of [...this.generations.keys()]) this.nextGeneration(name);
    for (const name of [...this.watches.keys()]) this.closeWatch(name);
    this.listeners.clear();
  }

  private async open(name: string, generation: number): Promise<void> {
    let host: Host;
    try {
      host = await this.binding.connect(name);
    } catch (error) {
      this.fail(name, generation, toFfiError(error));
      return;
    }
    if (!this.isCurrent(name, generation)) return;
    this.patch(name, { connection: { status: 'connected', host } }, `Connected to ${name}`);
    this.follow(name, host, generation);
  }

  private async follow(name: string, host: Host, generation: number): Promise<void> {
    try {
      const watch = host.watchAgents();
      this.watches.set(name, watch);
      for await (const event of asyncIterate(watch)) {
        if (!this.isCurrent(name, generation)) return;
        this.applyEvent(name, event);
      }
    } catch (error) {
      this.fail(name, generation, toFfiError(error));
    }
  }

  private applyEvent(name: string, event: AgentEvent): void {
    const previous = this.records.get(name)?.agents ?? INITIAL_AGENT_WATCH;
    const agents = applyAgentEvent(previous, event, this.now());
    this.patch(name, { agents }, event.tag === 'Link' ? linkLogText(event.link) : undefined);
  }

  private fail(name: string, generation: number, error: FfiError): void {
    if (!this.isCurrent(name, generation)) return;
    this.patch(name, { connection: { status: 'failed', error } }, errorLogText(error));
  }

  private patch(
    name: string,
    changes: Partial<Pick<HostRecord, 'connection' | 'agents'>>,
    logText?: string,
  ): void {
    const base = this.records.get(name) ?? { name, connection: { status: 'connecting' as const }, agents: undefined, log: [] };
    const log = logText === undefined ? base.log : appendLogLine(base.log, { atMs: this.now(), text: logText });
    const next = new Map(this.records);
    next.set(name, { ...base, ...changes, log });
    this.records = next;
    for (const listener of [...this.listeners]) listener();
  }

  private nextGeneration(name: string): number {
    const generation = (this.generations.get(name) ?? 0) + 1;
    this.generations.set(name, generation);
    return generation;
  }

  private isCurrent(name: string, generation: number): boolean {
    return this.generations.get(name) === generation;
  }

  private closeWatch(name: string): void {
    const watch = this.watches.get(name);
    this.watches.delete(name);
    watch?.close();
  }
}

function toFfiError(error: unknown): FfiError {
  if (error instanceof FfiException) return error.error;
  return { tag: 'Internal', message: error instanceof Error ? error.message : String(error) };
}
