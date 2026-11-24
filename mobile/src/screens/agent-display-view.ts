import type { GlyphState } from '../components/agent-status-glyph';
import { AGENT_WORDS, lastKnownLine } from '../copy/agent';
import type { AgentDisplay } from '../state/agent-display';

/** What a row or the status pill shows for one agent display. */
export interface GlyphView {
  readonly glyph: GlyphState;
  readonly word: string;
  readonly dimmed: boolean;
}

/** The live word, or without contact the last known state with its age (never "stopped"). */
export function glyphViewOf(display: AgentDisplay | undefined, age: string | undefined): GlyphView | undefined {
  if (display === undefined) return undefined;
  if (display.tag === 'live') return { glyph: display.glyph, word: display.word, dimmed: false };
  const lastWord = display.lastKnown === undefined ? undefined : AGENT_WORDS[display.lastKnown];
  return { glyph: 'unreachable', word: lastKnownLine(lastWord, age), dimmed: true };
}
