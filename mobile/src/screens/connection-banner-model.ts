import type { BannerTone } from '../components/connection-banner';
import { BANNER } from '../copy/connection';
import type { LinkState } from '../binding/types';
import type { HostRecord } from '../state/host-store';
import type { AgentWatchState } from '../state/agent-watch-state';
import { connectionProblemOf } from '../state/connection-problem';
import { formatAge } from '../state/relative-age';

export type BannerActionKind = 'retry' | 'retry-now' | 'details' | 'pair-again';

export interface BannerModel {
  readonly tone: BannerTone;
  readonly message: string;
  readonly actions: readonly BannerActionKind[];
}

/** The "Reconnected" notice stays this long after the link comes back. */
export const RESTORED_BANNER_MS = 5_000;

/** X-CN-01: the notice for a host's connection. Nothing is shown while the host is simply connected. */
export function connectionBannerModel(
  record: HostRecord | undefined,
  host: string,
  nowMs: number,
): BannerModel | undefined {
  const connection = record?.connection;
  if (connection === undefined) return undefined;
  if (connection.status === 'connecting') return { tone: 'info', message: BANNER.connecting(host), actions: [] };

  const agents = record?.agents;
  if (connection.status === 'failed') {
    const problem = connectionProblemOf(connection.error);
    switch (problem.kind) {
      case 'unreachable':
        return {
          tone: 'unreachable',
          message: BANNER.unreachable(host, lastKnownAge(agents, nowMs)),
          actions: ['retry', 'details'],
        };
      case 'sign-in':
        return { tone: 'warning', message: BANNER.signIn(host), actions: ['pair-again'] };
      case 'other':
        return { tone: 'danger', message: BANNER.failed(host, problem.message), actions: ['retry'] };
      default:
        return undefined;
    }
  }

  const link = agents?.link;
  if (link?.tag === 'Unverifiable') return lostBanner(link, host, agents, nowMs);
  if (agents?.restoredAtMs !== undefined && nowMs - agents.restoredAtMs < RESTORED_BANNER_MS) {
    return { tone: 'success', message: BANNER.restored(host), actions: [] };
  }
  return undefined;
}

function lostBanner(
  link: Extract<LinkState, { readonly tag: 'Unverifiable' }>,
  host: string,
  agents: AgentWatchState | undefined,
  nowMs: number,
): BannerModel {
  const actions: readonly BannerActionKind[] = ['retry-now', 'details'];
  if (link.attempt > 1) {
    return { tone: 'warning', message: BANNER.reconnecting(host, link.attempt, retrySeconds(link.retryInMs)), actions };
  }
  return { tone: 'unreachable', message: BANNER.unreachable(host, lastKnownAge(agents, nowMs)), actions };
}

function lastKnownAge(agents: AgentWatchState | undefined, nowMs: number): string | undefined {
  return agents?.lastContactMs === undefined ? undefined : formatAge(nowMs - agents.lastContactMs);
}

/** The whole seconds a retry waits, never under one. */
export function retrySeconds(retryInMs: bigint): number {
  return Math.max(1, Math.round(Number(retryInMs) / 1000));
}
