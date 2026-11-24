import { BANNER } from '../copy/connection';
import type { LinkTracker } from '../state/link-tracker';
import { formatAge } from '../state/relative-age';
import { RESTORED_BANNER_MS, retrySeconds, type BannerModel } from './connection-banner-model';

/**
 * The X-CN-01 notice for one watch or view of a host: the last known state with its age while the link is lost, the
 * retry while it reconnects, and "Reconnected" for a while after it returns. Undefined while the link is live.
 */
export function linkBannerModel(host: string, tracker: LinkTracker, nowMs: number): BannerModel | undefined {
  const { link, lastContactMs, restoredAtMs } = tracker;
  if (link.tag === 'Unverifiable') {
    if (link.attempt > 1) {
      return {
        tone: 'warning',
        message: BANNER.reconnecting(host, link.attempt, retrySeconds(link.retryInMs)),
        actions: ['details'],
      };
    }
    const age = lastContactMs === undefined ? undefined : formatAge(nowMs - lastContactMs);
    return { tone: 'unreachable', message: BANNER.unreachable(host, age), actions: ['details'] };
  }
  if (restoredAtMs !== undefined && nowMs - restoredAtMs < RESTORED_BANNER_MS) {
    return { tone: 'success', message: BANNER.restored(host), actions: [] };
  }
  return undefined;
}
