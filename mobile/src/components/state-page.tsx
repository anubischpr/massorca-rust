import { ScrollView, StyleSheet, Text, View } from 'react-native';

import { usePalette } from '../theme/palette';
import { spacingTokens, typeTokens } from '../theme/tokens.generated';
import { ActionButton, type ButtonVariant } from './action-button';

export interface PageAction {
  readonly label: string;
  readonly onPress: () => void;
  readonly variant?: ButtonVariant;
  readonly busy?: boolean;
}

/** A left-aligned page for an empty or failed state; its actions are pinned under the content. */
export function StatePage({
  title,
  line,
  detail,
  actions = [],
}: {
  readonly title: string;
  readonly line?: string;
  readonly detail?: string;
  readonly actions?: readonly PageAction[];
}) {
  const palette = usePalette();
  return (
    <View style={[styles.page, { backgroundColor: palette['surface.canvas'] }]}>
      <ScrollView contentContainerStyle={styles.content}>
        <Text accessibilityRole="header" style={[styles.title, { color: palette['text.primary'] }]}>
          {title}
        </Text>
        {line !== undefined && <Text style={[styles.line, { color: palette['text.secondary'] }]}>{line}</Text>}
        {detail !== undefined && <Text style={[styles.detail, { color: palette['text.tertiary'] }]}>{detail}</Text>}
      </ScrollView>
      {actions.length > 0 && (
        <View style={styles.actions}>
          {actions.map((action) => (
            <ActionButton
              key={action.label}
              label={action.label}
              onPress={action.onPress}
              variant={action.variant ?? 'secondary'}
              busy={action.busy}
            />
          ))}
        </View>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  page: {
    flex: 1,
    padding: spacingTokens['space.4'],
  },
  content: {
    gap: spacingTokens['space.3'],
    paddingBottom: spacingTokens['space.4'],
  },
  title: {
    fontSize: typeTokens['a-heading'].size,
    lineHeight: typeTokens['a-heading'].lineHeight,
    fontWeight: '600',
  },
  line: {
    fontSize: typeTokens['a-body'].size,
    lineHeight: typeTokens['a-body'].lineHeight,
  },
  detail: {
    fontSize: typeTokens['a-mono'].size,
    lineHeight: typeTokens['a-mono'].lineHeight,
    fontFamily: 'monospace',
  },
  actions: {
    gap: spacingTokens['space.2'],
    paddingTop: spacingTokens['space.2'],
  },
});
