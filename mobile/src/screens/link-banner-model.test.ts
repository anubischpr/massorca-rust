import { RESTORED_BANNER_MS } from './connection-banner-model';
import { linkBannerModel } from './link-banner-model';
import type { LinkTracker } from '../state/link-tracker';

const NOW = 1_000_000;
const FIVE_MIN = 5 * 60_000;

function tracker(overrides: Partial<LinkTracker>): LinkTracker {
  return { link: { tag: 'Live' }, lastContactMs: undefined, restoredAtMs: undefined, ...overrides };
}

const lost = { tag: 'Unverifiable' as const, attempt: 1, retryInMs: 2000n, reason: 'connection closed' };

describe('linkBannerModel', () => {
  it('shows the last known state with its age while the link is lost, and never says stopped', () => {
    const banner = linkBannerModel('home-pc', tracker({ link: lost, lastContactMs: NOW - FIVE_MIN }), NOW);

    expect(banner).toEqual({
      tone: 'unreachable',
      message:
        "home-pc can't be reached. Showing the last known state from 5 min ago. Work that was running there may still be running.",
      actions: ['details'],
    });
    expect(banner?.message).not.toMatch(/stopped|exited/i);
  });

  it('says it is reconnecting, with the attempt and the seconds to the next one, from the second attempt on', () => {
    const banner = linkBannerModel(
      'home-pc',
      tracker({ link: { ...lost, attempt: 2, retryInMs: 12_000n } }),
      NOW,
    );

    expect(banner).toEqual({
      tone: 'warning',
      message: 'Reconnecting to home-pc, attempt 2. Retrying in 12 s.',
      actions: ['details'],
    });
  });

  it('says it is reconnected for a few seconds after the link comes back', () => {
    const returned = tracker({ restoredAtMs: NOW });

    expect(linkBannerModel('home-pc', returned, NOW + RESTORED_BANNER_MS - 1)).toEqual({
      tone: 'success',
      message: 'Reconnected to home-pc.',
      actions: [],
    });
    expect(linkBannerModel('home-pc', returned, NOW + RESTORED_BANNER_MS)).toBeUndefined();
  });

  it('shows nothing while the link is live and has not just come back', () => {
    expect(linkBannerModel('home-pc', tracker({ lastContactMs: NOW }), NOW)).toBeUndefined();
  });
});
