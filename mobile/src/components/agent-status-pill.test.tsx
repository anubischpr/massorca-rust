import { render, screen } from '@testing-library/react-native';

import { AgentStatusPill } from './agent-status-pill';

describe('AgentStatusPill', () => {
  it('shows the live state word with its glyph', async () => {
    await render(<AgentStatusPill glyph="waiting" word="Needs you" />);

    expect(screen.getByText('Needs you')).toBeTruthy();
    expect(screen.getByRole('image', { name: 'Needs you' })).toBeTruthy();
  });

  it('keeps the last known state with its age when contact is lost, never stopped', async () => {
    await render(<AgentStatusPill glyph="unreachable" word="Last known: Working, 5 min ago" />);

    expect(screen.getByText('Last known: Working, 5 min ago')).toBeTruthy();
    expect(screen.queryByText(/stopped|exited/i)).toBeNull();
  });
});
