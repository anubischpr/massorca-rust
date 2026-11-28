import { StyleSheet, Switch, Text, View } from 'react-native';

import { TOUCH_TARGET } from '../components/dimensions';
import { usePalette } from '../theme/palette';
import { spacingTokens, typeTokens } from '../theme/tokens.generated';
import type { RoutineRowText } from './routines-model';

/**
 * One routine (section 11.1): its name, the schedule with its bot and state, the newest run, and a switch that pauses
 * and resumes it. The switch is on while the routine runs; a failed run's sentence is shown in the danger colour.
 */
export function RoutineRowView({
  text,
  busy,
  onPausedChange,
}: {
  readonly text: RoutineRowText;
  readonly busy: boolean;
  readonly onPausedChange: (paused: boolean) => void;
}) {
  const palette = usePalette();
  return (
    <View style={styles.row}>
      <View style={styles.texts}>
        <Text numberOfLines={1} style={[styles.title, { color: palette['text.primary'] }]}>
          {text.title}
        </Text>
        <Text style={[styles.body, { color: palette['text.secondary'] }]}>{text.summary}</Text>
        <Text style={[styles.caption, { color: palette['text.tertiary'] }]}>{text.lastRun}</Text>
        {text.lastRunError !== undefined && (
          <Text style={[styles.caption, { color: palette['status.danger.fg'] }]}>{text.lastRunError}</Text>
        )}
      </View>
      <Switch
        accessibilityLabel={`${text.title}, ${text.paused ? 'paused' : 'running'}`}
        value={!text.paused}
        disabled={busy}
        onValueChange={(running) => onPausedChange(!running)}
        trackColor={{ false: palette['border.strong'], true: palette['accent.default'] }}
        thumbColor={palette['surface.raised']}
      />
    </View>
  );
}

const styles = StyleSheet.create({
  row: {
    minHeight: TOUCH_TARGET + spacingTokens['space.2'],
    flexDirection: 'row',
    alignItems: 'center',
    gap: spacingTokens['space.3'],
    paddingHorizontal: spacingTokens['space.4'],
    paddingVertical: spacingTokens['space.2'],
  },
  texts: {
    flex: 1,
    gap: spacingTokens['space.half'],
  },
  title: {
    fontSize: typeTokens['a-body'].size,
    lineHeight: typeTokens['a-body'].lineHeight,
    fontWeight: '600',
  },
  body: {
    fontSize: typeTokens['a-body-sm'].size,
    lineHeight: typeTokens['a-body-sm'].lineHeight,
  },
  caption: {
    fontSize: typeTokens['a-caption'].size,
    lineHeight: typeTokens['a-caption'].lineHeight,
    fontVariant: ['tabular-nums'],
  },
});
