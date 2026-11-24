import { fireEvent, render, screen } from '@testing-library/react-native';
import { useState } from 'react';

import { SegmentedControl } from './segmented-control';
import { TextField } from './text-field';

describe('SegmentedControl', () => {
  function Harness() {
    const [value, setValue] = useState<'scan' | 'paste'>('scan');
    return (
      <SegmentedControl
        accessibilityLabel="Pairing mode"
        options={[
          { value: 'scan', label: 'Scan' },
          { value: 'paste', label: 'Paste' },
        ]}
        value={value}
        onChange={setValue}
      />
    );
  }

  it('marks the chosen segment as selected and switches on press', async () => {
    await render(<Harness />);

    expect(screen.getByRole('tab', { name: 'Scan' }).props.accessibilityState).toMatchObject({ selected: true });

    await fireEvent.press(screen.getByRole('tab', { name: 'Paste' }));

    expect(screen.getByRole('tab', { name: 'Paste' }).props.accessibilityState).toMatchObject({ selected: true });
    expect(screen.getByRole('tab', { name: 'Scan' }).props.accessibilityState).toMatchObject({ selected: false });
  });
});

describe('TextField', () => {
  it('reports each change and shows the error in place of the helper line', async () => {
    const onChangeText = jest.fn();
    await render(
      <TextField label="Pairing code" value="" onChangeText={onChangeText} helper="Paste it here" error="Not a valid code" />,
    );

    await fireEvent.changeText(screen.getByLabelText('Pairing code'), 'fake-offer:x');

    expect(onChangeText).toHaveBeenCalledWith('fake-offer:x');
    expect(screen.getByText('Not a valid code')).toBeTruthy();
    expect(screen.queryByText('Paste it here')).toBeNull();
  });
});
