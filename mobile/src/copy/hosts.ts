export const HOME_EMPTY_TITLE = 'Connect your desktop';
export const HOME_EMPTY_LINE = 'Follow your agents from the phone, answer their questions and approve their work.';

export const CONNECT_STEPS: readonly { readonly title: string; readonly line: string }[] = [
  { title: 'Open MassOrca on your computer', line: 'In the sidebar, choose MassOrca Mobile, which shows a QR code.' },
  { title: 'Scan the code', line: 'Or paste the pairing code that the computer shows.' },
  { title: 'You are connected', line: 'Your workspaces and agents appear here.' },
];

export const PAIR_DESKTOP = 'Pair desktop';
export const PASTE_A_CODE = 'Paste a code';
export const HOSTS_HEADING = 'Hosts';
export const PAIR_ACTION = 'Pair';

/** Host card badges (screen spec 5.2). */
export const HOST_BADGE = {
  connected: 'Connected',
  connecting: 'Connecting',
  unreachable: 'Unreachable',
  signIn: 'Sign in',
  updateNeeded: 'Update needed',
  check: 'Check',
} as const;

export const CANT_REACH_LINE = (host: string): string => `${host} can't be reached.`;

/** "1 session needs you, 2 working"; undefined when no session is known. */
export function attentionSummary(needsYou: number, working: number): string | undefined {
  const parts: string[] = [];
  if (needsYou > 0) parts.push(needsYou === 1 ? '1 session needs you' : `${needsYou} sessions need you`);
  if (working > 0) parts.push(`${working} working`);
  return parts.length > 0 ? parts.join(', ') : undefined;
}

export function lastKnownHostLine(age: string, summary: string | undefined): string {
  return summary === undefined
    ? `Last known state, ${age}. It may still be running.`
    : `Last known state, ${age}: ${summary}. It may still be running.`;
}
