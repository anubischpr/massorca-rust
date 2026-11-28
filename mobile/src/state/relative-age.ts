const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;

/** The age a last known state is shown with: "a moment ago", "5 min ago", "3 h ago", "2 d ago". */
export function formatAge(elapsedMs: number): string {
  const elapsed = Math.max(0, elapsedMs);
  if (elapsed < MINUTE_MS) return 'a moment ago';
  if (elapsed < HOUR_MS) return `${Math.floor(elapsed / MINUTE_MS)} min ago`;
  if (elapsed < DAY_MS) return `${Math.floor(elapsed / HOUR_MS)} h ago`;
  return `${Math.floor(elapsed / DAY_MS)} d ago`;
}

/** The time until a run, read after "next": "in a moment", "in 5 min", "in 3 h", "in 1 day", "in 4 days". */
export function formatUntil(untilMs: number): string {
  const until = Math.max(0, untilMs);
  if (until < MINUTE_MS) return 'in a moment';
  if (until < HOUR_MS) return `in ${Math.floor(until / MINUTE_MS)} min`;
  if (until < DAY_MS) return `in ${Math.floor(until / HOUR_MS)} h`;
  const days = Math.floor(until / DAY_MS);
  return days === 1 ? 'in 1 day' : `in ${days} days`;
}
