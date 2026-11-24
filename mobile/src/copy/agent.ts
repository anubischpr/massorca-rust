/** Visible state words of agent sessions (screen spec section 1, status convention). */
export const AGENT_WORDS = {
  running: 'Working',
  waiting: 'Needs you',
  idle: 'Idle',
  done: 'Done',
  unreachable: 'Unreachable',
} as const;

/** The state a phone can no longer confirm: the last known state with its age, never "stopped". */
export function lastKnownLine(word: string | undefined, age: string | undefined): string {
  if (word !== undefined && age !== undefined) return `Last known: ${word}, ${age}`;
  if (age !== undefined) return `Last known state, ${age}`;
  if (word !== undefined) return `Last known: ${word}`;
  return AGENT_WORDS.unreachable;
}
