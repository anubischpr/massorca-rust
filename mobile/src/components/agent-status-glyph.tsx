import { StyleSheet, Text, View } from 'react-native';

import { usePalette } from '../theme/palette';
import type { ColorTokenName } from '../theme/tokens.generated';
import { GLYPH_SIZE, type GlyphSize } from './dimensions';

export type GlyphState = 'running' | 'waiting' | 'idle' | 'done' | 'error' | 'unreachable';

const GLYPH_COLOR: Record<GlyphState, ColorTokenName> = {
  running: 'agent.running',
  waiting: 'agent.waiting',
  idle: 'agent.idle',
  done: 'agent.done',
  error: 'agent.error',
  unreachable: 'agent.unreachable',
};

/** One shape per state, so colour is never the only cue (AgentStatus spec). The word is the accessible name. */
export function AgentStatusGlyph({
  state,
  word,
  size = GLYPH_SIZE.medium,
}: {
  readonly state: GlyphState;
  readonly word: string;
  readonly size?: GlyphSize;
}) {
  const palette = usePalette();
  const color = palette[GLYPH_COLOR[state]];
  const stroke = size <= GLYPH_SIZE.small ? 1.5 : 2;
  const box = { width: size, height: size, borderRadius: size / 2 };

  return (
    <View accessible accessibilityRole="image" accessibilityLabel={word} style={[styles.box, { width: size, height: size }]}>
      {state === 'running' && (
        <View style={[styles.fill, box, styles.arc, { borderWidth: stroke, borderColor: color }]} />
      )}
      {state === 'idle' && <View style={[styles.fill, box, { borderWidth: stroke, borderColor: color }]} />}
      {state === 'unreachable' && (
        <>
          <View style={[styles.fill, box, styles.dashed, { borderWidth: stroke, borderColor: color }]} />
          <View style={[styles.slash, { width: size * 0.9, backgroundColor: color }]} />
        </>
      )}
      {state === 'waiting' && (
        <View style={[styles.fill, box, { backgroundColor: color }]}>
          <Text style={[styles.mark, { color: palette['status.warning.on'], fontSize: size * 0.75 }]}>?</Text>
        </View>
      )}
      {state === 'done' && (
        <View style={[styles.fill, box, { backgroundColor: color }]}>
          <Text style={[styles.mark, { color: palette['status.success.on'], fontSize: size * 0.75 }]}>✓</Text>
        </View>
      )}
      {state === 'error' && (
        <View style={[styles.triangle, { borderLeftWidth: size / 2, borderRightWidth: size / 2, borderBottomWidth: size, borderBottomColor: color }]} />
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  box: {
    alignItems: 'center',
    justifyContent: 'center',
  },
  fill: {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
  },
  arc: {
    borderTopColor: 'transparent',
  },
  dashed: {
    borderStyle: 'dashed',
  },
  mark: {
    flex: 1,
    textAlign: 'center',
    textAlignVertical: 'center',
    fontWeight: '700',
  },
  slash: {
    position: 'absolute',
    height: 2,
    transform: [{ rotate: '-45deg' }],
  },
  triangle: {
    width: 0,
    height: 0,
    borderLeftColor: 'transparent',
    borderRightColor: 'transparent',
  },
});
