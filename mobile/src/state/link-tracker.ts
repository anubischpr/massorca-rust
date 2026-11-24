import type { LinkState } from '../binding/types';

/** What a watch or a view knows about its host's link: the last state, when the host last answered, and the return. */
export interface LinkTracker {
  readonly link: LinkState;
  /** Phone clock: when the host last answered. Ages are measured from here, never from the host's clock. */
  readonly lastContactMs: number | undefined;
  /** Phone clock: when the link came back after a loss. */
  readonly restoredAtMs: number | undefined;
}

export const INITIAL_LINK: LinkTracker = { link: { tag: 'Live' }, lastContactMs: undefined, restoredAtMs: undefined };

/** Folds a link change in: a loss keeps the last contact, and a return records when it happened. */
export function trackLink(tracker: LinkTracker, link: LinkState, nowMs: number): LinkTracker {
  if (link.tag === 'Unverifiable') return { ...tracker, link };
  const wasLost = tracker.link.tag === 'Unverifiable';
  return { ...tracker, link, lastContactMs: nowMs, restoredAtMs: wasLost ? nowMs : tracker.restoredAtMs };
}

/** Any answer from the host is contact, so it moves the last contact on. */
export function touchContact<T extends LinkTracker>(tracker: T, nowMs: number): T {
  return { ...tracker, lastContactMs: nowMs };
}
