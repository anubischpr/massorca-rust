import { render, screen } from '@testing-library/react-native';

import { AgentStatusGlyph, type GlyphState } from './agent-status-glyph';

describe('AgentStatusGlyph', () => {
  const states: readonly [GlyphState, string][] = [
    ['running', 'Working'],
    ['waiting', 'Needs you'],
    ['idle', 'Idle'],
    ['done', 'Done'],
    ['error', 'Failed'],
    ['unreachable', 'Unreachable'],
  ];

  it.each(states)('names the %s state by its word, as an image', async (state, word) => {
    await render(<AgentStatusGlyph state={state} word={word} />);

    expect(screen.getByRole('image', { name: word })).toBeTruthy();
  });
});
