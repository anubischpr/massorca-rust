import type { BadgeTone } from '../components/state-badge';
import { PAIR_AGAIN, RETRY } from '../copy/connection';
import { attentionSummary, CANT_REACH_LINE, HOST_BADGE, lastKnownHostLine } from '../copy/hosts';
import type { HostSummary } from '../binding/types';
import type { HostRecord } from '../state/host-store';
import { attentionCounts } from '../state/agent-display';
import { connectionProblemOf } from '../state/connection-problem';
import { formatAge } from '../state/relative-age';

/** What a tap on the card does: open the projects, open the protocol gate, or pair again. */
export type HostCardTap = 'open' | 'protocol' | 'pair';

export interface HostCardModel {
  readonly badge: string;
  readonly tone: BadgeTone;
  readonly dimmed: boolean;
  /** The sentence under the badge: the reason, or the last known state with its age. */
  readonly body: string | undefined;
  /** "1 session needs you, 2 working", from the agent watch. */
  readonly attention: string | undefined;
  readonly tap: HostCardTap;
  readonly action: { readonly label: string; readonly kind: 'retry' | 'pair' } | undefined;
}

/** Maps a host's connection and agent watch to the card of screen spec 5.2. Loss of contact is never "stopped". */
export function hostCardModel(host: HostSummary, record: HostRecord | undefined, nowMs: number): HostCardModel {
  const connection = record?.connection;
  if (connection === undefined || connection.status === 'connecting') {
    return card({ badge: HOST_BADGE.connecting, tone: 'info' });
  }

  const agents = record?.agents;
  const linkLost = agents?.link.tag === 'Unverifiable';
  if (connection.status === 'connected' && !linkLost) {
    return card({ badge: HOST_BADGE.connected, tone: 'success', attention: attentionFor(record, false) });
  }

  if (connection.status === 'connected' || connectionProblemOf(connection.error).kind === 'unreachable') {
    const age = agents?.lastContactMs === undefined ? undefined : formatAge(nowMs - agents.lastContactMs);
    const body = age === undefined ? CANT_REACH_LINE(host.name) : lastKnownHostLine(age, attentionFor(record, true));
    return card({
      badge: HOST_BADGE.unreachable,
      tone: 'unreachable',
      dimmed: true,
      body,
      action: { label: RETRY, kind: 'retry' },
    });
  }

  const problem = connectionProblemOf(connection.error);
  switch (problem.kind) {
    case 'sign-in':
      return card({
        badge: HOST_BADGE.signIn,
        tone: 'warning',
        body: problem.message,
        tap: 'pair',
        action: { label: PAIR_AGAIN, kind: 'pair' },
      });
    case 'protocol':
      return card({ badge: HOST_BADGE.updateNeeded, tone: 'warning', tap: 'protocol' });
    default:
      return card({
        badge: HOST_BADGE.check,
        tone: 'warning',
        body: problem.kind === 'other' ? problem.message : undefined,
        action: { label: RETRY, kind: 'retry' },
      });
  }
}

function attentionFor(record: HostRecord | undefined, linkLost: boolean): string | undefined {
  const sessions = record?.agents?.sessions ?? [];
  const counts = attentionCounts(sessions, linkLost);
  return attentionSummary(counts.needsYou, counts.working);
}

function card(
  partial: Partial<HostCardModel> & Pick<HostCardModel, 'badge' | 'tone'>,
): HostCardModel {
  return {
    dimmed: false,
    body: undefined,
    attention: undefined,
    tap: 'open',
    action: undefined,
    ...partial,
  };
}
