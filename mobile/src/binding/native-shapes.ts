// The shapes of the generated module that the adapter reads, written out so that tests can stand in a fake. index.ts
// checks the real module against them; the converters here turn them into the app's types (types.ts).

import type * as Rust from '../generated/massorca_client_ffi';

import type {
  AgentEvent,
  AgentItem,
  AgentSessionSummary,
  GitEvent,
  GitFileStatus,
  GitItem,
  GitStatusSummary,
  HostSummary,
  LinkState,
  OfferSummary,
  ProtocolInfo,
  TerminalEvent,
  WhoAmI,
  WorkspaceSummary,
} from './types';

/** A pull-based watch: `next` resolves to the next event, or to undefined once the watch is over. */
export interface RustWatch<E> {
  next(): Promise<E | undefined>;
  close(): void;
}

/** The generated enums: `tag` names the variant, and a variant with fields carries them in `inner`. */
export type RustLinkState =
  | { readonly tag: 'Live' }
  | {
      readonly tag: 'Unverifiable';
      readonly inner: { readonly attempt: number; readonly retryInMs: bigint; readonly reason: string };
    };

export type RustAgentItem =
  | { readonly tag: 'Snapshot'; readonly inner: { readonly sessions: readonly Rust.AgentSessionSummary[] } }
  | { readonly tag: 'Changed'; readonly inner: { readonly session: Rust.AgentSessionSummary } };

export type RustAgentEvent =
  | { readonly tag: 'Item'; readonly inner: { readonly item: RustAgentItem } }
  | { readonly tag: 'Link'; readonly inner: { readonly link: RustLinkState } };

export type RustGitEvent =
  | { readonly tag: 'Item'; readonly inner: { readonly item: Rust.GitItem } }
  | { readonly tag: 'Link'; readonly inner: { readonly link: RustLinkState } };

export interface RustTerminalView {
  next(): Promise<RustTerminalEvent | undefined>;
  input(bytes: ArrayBuffer): Promise<void>;
  resize(cols: number, rows: number): Promise<void>;
  close(): void;
}

/** The generated terminal events; a 64-bit exit code arrives as a bigint. */
export type RustTerminalEvent =
  | { readonly tag: 'Output'; readonly inner: { readonly offset: bigint; readonly bytes: ArrayBuffer } }
  | {
      readonly tag: 'Snapshot';
      readonly inner: { readonly offset: bigint; readonly cols: number; readonly rows: number; readonly bytes: ArrayBuffer };
    }
  | { readonly tag: 'Resized'; readonly inner: { readonly cols: number; readonly rows: number } }
  | {
      readonly tag: 'Exited';
      readonly inner: { readonly reason: string; readonly code?: bigint; readonly signal?: string };
    }
  | { readonly tag: 'Link'; readonly inner: { readonly link: RustLinkState } };

export type RustFfiError =
  | { readonly tag: 'Unreachable'; readonly inner: { readonly message: string } }
  | {
      readonly tag: 'IdentityMismatch';
      readonly inner: { readonly message: string; readonly expected: string; readonly actual?: string };
    }
  | { readonly tag: 'NotPaired'; readonly inner: { readonly message: string } }
  | {
      readonly tag: 'ProtocolMismatch';
      readonly inner: { readonly message: string; readonly supportedMin?: number; readonly supportedMax?: number };
    }
  | { readonly tag: 'InvalidOffer'; readonly inner: { readonly message: string } }
  | { readonly tag: 'HostExists'; readonly inner: { readonly message: string } }
  | { readonly tag: 'HostNotFound'; readonly inner: { readonly message: string } }
  | { readonly tag: 'InvalidHostName'; readonly inner: { readonly message: string } }
  | { readonly tag: 'Remote'; readonly inner: { readonly code: string; readonly message: string } }
  | { readonly tag: 'Storage'; readonly inner: { readonly message: string } }
  | { readonly tag: 'Internal'; readonly inner: { readonly message: string } };

export interface RustHost {
  name(): string;
  whoami(): Promise<Rust.WhoAmI>;
  protocol(): Rust.ProtocolInfo;
  workspaces(project: string | undefined): Promise<Rust.WorkspaceSummary[]>;
  gitStatus(workspace: string): Promise<Rust.GitStatusSummary>;
  watchGit(workspace: string): RustWatch<RustGitEvent>;
  watchAgents(workspace: string | undefined): RustWatch<RustAgentEvent>;
  attachTerminal(terminal: string, cols: number, rows: number): Promise<RustTerminalView>;
}

export interface RustMassOrca {
  hosts(): Rust.HostSummary[];
  pair(offer: string, hostName: string): Promise<Rust.HostSummary>;
  forget(hostName: string): void;
  connect(hostName: string): Promise<RustHost>;
}

export interface RustModule {
  parseOffer(text: string): Rust.OfferSummary;
  MassOrca: new (store: Rust.HostStore, deviceName: string, appVersion: string) => RustMassOrca;
}

export function hostSummary(raw: Rust.HostSummary): HostSummary {
  return {
    name: raw.name,
    server: raw.server,
    addresses: raw.addresses,
    pin: raw.pin,
    deviceName: raw.deviceName,
    scopes: raw.scopes,
    pairedAtMs: raw.pairedAtMs,
  };
}

export function offerSummary(raw: Rust.OfferSummary): OfferSummary {
  return { server: raw.server, addresses: raw.addresses, expiresAtMs: raw.expiresAtMs, scopes: raw.scopes };
}

export function whoAmI(raw: Rust.WhoAmI): WhoAmI {
  return { deviceId: raw.deviceId, deviceName: raw.deviceName, scopes: raw.scopes };
}

export function protocolInfo(raw: Rust.ProtocolInfo): ProtocolInfo {
  return {
    protocol: raw.protocol,
    serverName: raw.serverName,
    serverVersion: raw.serverVersion,
    capabilities: raw.capabilities,
  };
}

export function workspaceSummary(raw: Rust.WorkspaceSummary): WorkspaceSummary {
  return {
    id: raw.id,
    projectId: raw.projectId,
    name: raw.name,
    path: raw.path,
    kind: raw.kind,
    branch: raw.branch,
    state: raw.state,
    missing: raw.missing,
  };
}

export function gitStatusSummary(raw: Rust.GitStatusSummary): GitStatusSummary {
  return {
    branch: raw.branch,
    ahead: raw.ahead,
    behind: raw.behind,
    operation: raw.operation,
    files: raw.files.map(gitFileStatus),
    truncated: raw.truncated,
  };
}

function gitFileStatus(raw: Rust.GitFileStatus): GitFileStatus {
  return {
    path: raw.path,
    originalPath: raw.originalPath,
    staged: raw.staged,
    unstaged: raw.unstaged,
    untracked: raw.untracked,
    conflict: raw.conflict,
  };
}

function gitItem(raw: Rust.GitItem): GitItem {
  return { ready: raw.ready, kinds: raw.kinds, degraded: raw.degraded, reason: raw.reason };
}

function agentSession(raw: Rust.AgentSessionSummary): AgentSessionSummary {
  return {
    id: raw.id,
    kind: raw.kind,
    workspaceId: raw.workspaceId,
    title: raw.title,
    activityState: raw.activityState,
    executionState: raw.executionState,
    updatedAtMs: raw.updatedAtMs,
  };
}

function linkState(raw: RustLinkState): LinkState {
  switch (raw.tag) {
    case 'Live':
      return { tag: 'Live' };
    case 'Unverifiable':
      return {
        tag: 'Unverifiable',
        attempt: raw.inner.attempt,
        retryInMs: raw.inner.retryInMs,
        reason: raw.inner.reason,
      };
  }
}

function agentItem(raw: RustAgentItem): AgentItem {
  switch (raw.tag) {
    case 'Snapshot':
      return { tag: 'Snapshot', sessions: raw.inner.sessions.map(agentSession) };
    case 'Changed':
      return { tag: 'Changed', session: agentSession(raw.inner.session) };
  }
}

export function agentEvent(raw: RustAgentEvent): AgentEvent {
  switch (raw.tag) {
    case 'Item':
      return { tag: 'Item', item: agentItem(raw.inner.item) };
    case 'Link':
      return { tag: 'Link', link: linkState(raw.inner.link) };
  }
}

export function gitEvent(raw: RustGitEvent): GitEvent {
  switch (raw.tag) {
    case 'Item':
      return { tag: 'Item', item: gitItem(raw.inner.item) };
    case 'Link':
      return { tag: 'Link', link: linkState(raw.inner.link) };
  }
}

export function terminalEvent(raw: RustTerminalEvent): TerminalEvent {
  switch (raw.tag) {
    case 'Output':
      return { tag: 'Output', offset: raw.inner.offset, bytes: raw.inner.bytes };
    case 'Snapshot':
      return {
        tag: 'Snapshot',
        offset: raw.inner.offset,
        cols: raw.inner.cols,
        rows: raw.inner.rows,
        bytes: raw.inner.bytes,
      };
    case 'Resized':
      return { tag: 'Resized', cols: raw.inner.cols, rows: raw.inner.rows };
    case 'Exited':
      return {
        tag: 'Exited',
        reason: raw.inner.reason,
        code: raw.inner.code === undefined ? undefined : Number(raw.inner.code),
        signal: raw.inner.signal,
      };
    case 'Link':
      return { tag: 'Link', link: linkState(raw.inner.link) };
  }
}
