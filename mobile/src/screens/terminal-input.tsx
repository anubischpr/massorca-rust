import { Pressable, StyleSheet, Text, TextInput, View } from 'react-native';

import { ActionButton } from '../components/action-button';
import { CONTROL_HEIGHT, TOUCH_TARGET } from '../components/dimensions';
import { KEY_ROW, SEND_TO_TERMINAL, TYPE_COMMAND } from '../copy/terminal';
import { usePalette } from '../theme/palette';
import { radiusTokens, spacingTokens, typeTokens } from '../theme/tokens.generated';

export type KeyName = (typeof KEY_ROW)[number]['key'];

/** The key row (M-TM-02): Esc, Tab, Ctrl and the arrows. Ctrl is sticky and shows when it is armed. */
export function TerminalKeyRow({
  ctrlArmed,
  disabled,
  onKey,
}: {
  readonly ctrlArmed: boolean;
  readonly disabled: boolean;
  readonly onKey: (key: KeyName) => void;
}) {
  const palette = usePalette();
  return (
    <View style={[styles.keyRow, { backgroundColor: palette['surface.frame'] }]} accessibilityRole="toolbar">
      {KEY_ROW.map((key) => {
        const armed = key.key === 'ctrl' && ctrlArmed;
        return (
          <Pressable
            key={key.key}
            accessibilityRole="button"
            accessibilityLabel={key.name}
            accessibilityState={{ disabled, selected: armed }}
            disabled={disabled}
            onPress={() => onKey(key.key)}
            style={[
              styles.key,
              { backgroundColor: armed ? palette['accent.subtle.frame'] : palette['surface.raised'] },
              disabled && styles.disabled,
            ]}
          >
            <Text style={[styles.keyLabel, { color: palette['text.primary'] }]}>{key.label}</Text>
          </Pressable>
        );
      })}
    </View>
  );
}

/** The input dock (M-TM-03): one mono line that is sent to the terminal with Enter. */
export function TerminalInputDock({
  value,
  disabled,
  onChangeText,
  onSend,
}: {
  readonly value: string;
  readonly disabled: boolean;
  readonly onChangeText: (text: string) => void;
  readonly onSend: () => void;
}) {
  const palette = usePalette();
  return (
    <View style={[styles.dock, { backgroundColor: palette['surface.frame'] }]}>
      <TextInput
        accessibilityLabel={TYPE_COMMAND}
        autoCapitalize="none"
        testID="terminal-input"
        autoCorrect={false}
        editable={!disabled}
        onChangeText={onChangeText}
        onSubmitEditing={onSend}
        placeholder={TYPE_COMMAND}
        placeholderTextColor={palette['text.tertiary']}
        returnKeyType="send"
        style={[
          styles.field,
          { backgroundColor: palette['surface.sunken'], borderColor: palette['border.default'], color: palette['text.primary'] },
        ]}
        value={value}
      />
      <ActionButton label={SEND_TO_TERMINAL} onPress={onSend} disabled={disabled} variant="primary" testID="terminal-send" />
    </View>
  );
}

const styles = StyleSheet.create({
  keyRow: {
    flexDirection: 'row',
    gap: spacingTokens['space.1'],
    paddingHorizontal: spacingTokens['space.2'],
    paddingVertical: spacingTokens['space.1'],
  },
  key: {
    flex: 1,
    minWidth: TOUCH_TARGET,
    minHeight: TOUCH_TARGET,
    height: CONTROL_HEIGHT,
    alignItems: 'center',
    justifyContent: 'center',
    borderRadius: radiusTokens['radius.sm'],
  },
  keyLabel: {
    fontFamily: 'monospace',
    fontSize: typeTokens['a-mono'].size,
    lineHeight: typeTokens['a-mono'].lineHeight,
    fontWeight: '500',
  },
  disabled: {
    opacity: 0.5,
  },
  dock: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: spacingTokens['space.2'],
    padding: spacingTokens['space.2'],
  },
  field: {
    flex: 1,
    minHeight: CONTROL_HEIGHT,
    borderWidth: 1,
    borderRadius: radiusTokens['radius.lg'],
    paddingHorizontal: spacingTokens['space.3'],
    fontFamily: 'monospace',
    fontSize: typeTokens['a-mono'].size,
  },
});
