// The app's MassOrcaBinding (types.ts) over the client that uniffi-bindgen-react-native generates from crates/client-ffi.
// Every failure leaves as an FfiException; native-shapes.ts and native-errors.ts hold the generated shapes.

import type * as Rust from '../generated/massorca_client_ffi';

import { toException } from './native-errors';
import {
  agentEvent,
  gitEvent,
  gitStatusSummary,
  hostSummary,
  offerSummary,
  protocolInfo,
  terminalEvent,
  whoAmI,
  workspaceSummary,
  type RustAgentEvent,
  type RustGitEvent,
  type RustHost,
  type RustModule,
  type RustTerminalView,
  type RustWatch,
} from './native-shapes';
import type {
  AgentEvent,
  GitEvent,
  GitStatusSummary,
  Host,
  HostSummary,
  MassOrcaBinding,
  OfferSummary,
  ProtocolInfo,
  TerminalEvent,
  TerminalView,
  WhoAmI,
  Watch,
  WorkspaceSummary,
} from './types';

export interface NativeOptions {
  readonly store: Rust.HostStore;
  readonly deviceName: string;
  readonly appVersion: string;
}

/** The binding over `rust`. Creating it starts the Rust client, so the app creates one and keeps it. */
export function createNativeBinding(rust: RustModule, options: NativeOptions): MassOrcaBinding {
  const client = guard(() => new rust.MassOrca(options.store, options.deviceName, options.appVersion));
  return {
    parseOffer: (text): OfferSummary => guard(() => offerSummary(rust.parseOffer(text))),
    hosts: (): HostSummary[] => guard(() => client.hosts().map(hostSummary)),
    pair: (offer, hostName): Promise<HostSummary> =>
      guardAsync(async () => hostSummary(await client.pair(offer, hostName))),
    forget: (hostName): void => guard(() => client.forget(hostName)),
    connect: (hostName): Promise<Host> => guardAsync(async () => new NativeHost(await client.connect(hostName))),
  };
}

class NativeHost implements Host {
  private readonly host: RustHost;

  constructor(host: RustHost) {
    this.host = host;
  }

  name(): string {
    return this.host.name();
  }

  whoami(): Promise<WhoAmI> {
    return guardAsync(async () => whoAmI(await this.host.whoami()));
  }

  protocol(): ProtocolInfo {
    return guard(() => protocolInfo(this.host.protocol()));
  }

  workspaces(project?: string): Promise<WorkspaceSummary[]> {
    return guardAsync(async () => (await this.host.workspaces(project)).map(workspaceSummary));
  }

  gitStatus(workspace: string): Promise<GitStatusSummary> {
    return guardAsync(async () => gitStatusSummary(await this.host.gitStatus(workspace)));
  }

  watchGit(workspace: string): Watch<GitEvent> {
    return guard(() => new NativeWatch<RustGitEvent, GitEvent>(this.host.watchGit(workspace), gitEvent));
  }

  watchAgents(workspace?: string): Watch<AgentEvent> {
    return guard(() => new NativeWatch<RustAgentEvent, AgentEvent>(this.host.watchAgents(workspace), agentEvent));
  }

  attachTerminal(terminal: string, cols: number, rows: number): Promise<TerminalView> {
    return guardAsync(async () => new NativeTerminalView(await this.host.attachTerminal(terminal, cols, rows)));
  }
}

/** A generated terminal view as the app's view: `next` resolves to null once it is over, and failures are FfiException. */
class NativeTerminalView implements TerminalView {
  private readonly view: RustTerminalView;

  constructor(view: RustTerminalView) {
    this.view = view;
  }

  async next(): Promise<TerminalEvent | null> {
    try {
      const raw = await this.view.next();
      return raw === undefined ? null : terminalEvent(raw);
    } catch (error) {
      throw toException(error);
    }
  }

  input(bytes: ArrayBuffer): Promise<void> {
    return guardAsync(() => this.view.input(bytes));
  }

  resize(cols: number, rows: number): Promise<void> {
    return guardAsync(() => this.view.resize(cols, rows));
  }

  close(): void {
    guard(() => this.view.close());
  }
}

/** A generated watch as the app's pull-based watch: `null` once it is over, and its failures as FfiException. */
class NativeWatch<R, E> implements Watch<E> {
  private readonly watch: RustWatch<R>;
  private readonly convert: (raw: R) => E;

  constructor(watch: RustWatch<R>, convert: (raw: R) => E) {
    this.watch = watch;
    this.convert = convert;
  }

  async next(): Promise<E | null> {
    try {
      const raw = await this.watch.next();
      return raw === undefined ? null : this.convert(raw);
    } catch (error) {
      throw toException(error);
    }
  }

  close(): void {
    this.watch.close();
  }
}

function guard<T>(call: () => T): T {
  try {
    return call();
  } catch (error) {
    throw toException(error);
  }
}

async function guardAsync<T>(call: () => Promise<T>): Promise<T> {
  try {
    return await call();
  } catch (error) {
    throw toException(error);
  }
}
