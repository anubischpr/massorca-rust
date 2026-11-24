import { Pressable, StyleSheet, Text, View } from 'react-native';

import type { HostSummary } from '../binding/types';
import { ActionButton } from '../components/action-button';
import { StateBadge } from '../components/state-badge';
import { useHostRecord } from '../state/host-store-context';
import { useNow } from '../state/use-now';
import { usePalette } from '../theme/palette';
import { radiusTokens, spacingTokens, typeTokens } from '../theme/tokens.generated';
import { hostCardModel, type HostCardTap } from './host-card-model';

/** One paired host (M-HM-02): its name, state badge, the reason or last known state, and the attention line. */
export function HostCard({
  host,
  onTap,
  onAction,
}: {
  readonly host: HostSummary;
  readonly onTap: (tap: HostCardTap) => void;
  readonly onAction: (kind: 'retry' | 'pair') => void;
}) {
  const record = useHostRecord(host.name);
  const now = useNow(60_000);
  const model = hostCardModel(host, record, now);
  const palette = usePalette();
  const nameColour = model.dimmed ? palette['text.tertiary'] : palette['text.primary'];

  return (
    <View style={[styles.card, { backgroundColor: palette['surface.raised'], borderColor: palette['border.default'] }]}>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={`${host.name}, ${model.badge}`}
        onPress={() => onTap(model.tap)}
        style={styles.tapArea}
      >
        <View style={styles.header}>
          <View style={[styles.plate, { backgroundColor: palette['surface.sunken'] }]}>
            <Text style={[styles.plateText, { color: palette['text.secondary'] }]}>{host.name.slice(0, 1).toUpperCase()}</Text>
          </View>
          <View style={styles.titles}>
            <Text style={[styles.name, { color: nameColour }]} numberOfLines={1}>
              {host.name}
            </Text>
            <Text style={[styles.address, { color: palette['text.secondary'] }]} numberOfLines={1}>
              {host.addresses[0] ?? host.server}
            </Text>
          </View>
          <StateBadge label={model.badge} tone={model.tone} />
        </View>
        {model.attention !== undefined && (
          <Text style={[styles.line, { color: palette['text.secondary'] }]}>{model.attention}</Text>
        )}
        {model.body !== undefined && (
          <Text style={[styles.line, { color: model.dimmed ? palette['text.tertiary'] : palette['text.secondary'] }]}>
            {model.body}
          </Text>
        )}
      </Pressable>
      {model.action !== undefined && (
        <ActionButton label={model.action.label} variant="ghost" onPress={() => onAction(model.action?.kind ?? 'retry')} />
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  card: {
    borderWidth: 1,
    borderRadius: radiusTokens['radius.lg'],
    padding: spacingTokens['space.3'],
    gap: spacingTokens['space.2'],
  },
  tapArea: {
    gap: spacingTokens['space.2'],
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: spacingTokens['space.3'],
  },
  plate: {
    width: 40,
    height: 40,
    borderRadius: radiusTokens['radius.md'],
    alignItems: 'center',
    justifyContent: 'center',
  },
  plateText: {
    fontSize: typeTokens['a-label'].size,
    lineHeight: typeTokens['a-label'].lineHeight,
    fontWeight: '600',
  },
  titles: {
    flex: 1,
  },
  name: {
    fontSize: typeTokens['a-title-sm'].size,
    lineHeight: typeTokens['a-title-sm'].lineHeight,
    fontWeight: '600',
  },
  address: {
    fontSize: typeTokens['a-caption'].size,
    lineHeight: typeTokens['a-caption'].lineHeight,
  },
  line: {
    fontSize: typeTokens['a-body-sm'].size,
    lineHeight: typeTokens['a-body-sm'].lineHeight,
  },
});
