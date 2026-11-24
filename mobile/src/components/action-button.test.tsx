import { fireEvent, render, screen } from '@testing-library/react-native';

import { ActionButton } from './action-button';

describe('ActionButton', () => {
  it('calls its handler when pressed', async () => {
    const onPress = jest.fn();
    await render(<ActionButton label="Connect" onPress={onPress} variant="primary" />);

    await fireEvent.press(screen.getByRole('button', { name: 'Connect' }));

    expect(onPress).toHaveBeenCalledTimes(1);
  });

  it('does not respond while disabled or busy, and reports busy', async () => {
    const onPress = jest.fn();
    await render(
      <>
        <ActionButton label="Pair" onPress={onPress} disabled />
        <ActionButton label="Saving" onPress={onPress} busy />
      </>,
    );

    await fireEvent.press(screen.getByRole('button', { name: 'Pair' }));
    await fireEvent.press(screen.getByRole('button', { name: 'Saving' }));

    expect(onPress).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'Saving' }).props.accessibilityState).toMatchObject({ busy: true });
  });
});
