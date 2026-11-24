import { fireEvent, render, screen } from '@testing-library/react-native';

import { ConnectionBanner } from './connection-banner';

describe('ConnectionBanner', () => {
  it('announces its message politely and runs the action it offers', async () => {
    const onRetry = jest.fn();
    await render(
      <ConnectionBanner
        tone="unreachable"
        message="home-pc can't be reached."
        actions={[{ label: 'Retry now', onPress: onRetry }]}
      />,
    );

    const message = screen.getByText("home-pc can't be reached.");
    expect(message.parent?.props.accessibilityLiveRegion).toBe('polite');

    await fireEvent.press(screen.getByRole('button', { name: 'Retry now' }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });
});
