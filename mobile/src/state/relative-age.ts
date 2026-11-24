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
