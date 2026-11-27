import type { ReactNode } from 'react';
import { Modal, Pressable, StyleSheet, Text, View } from 'react-native';

import { usePalette } from '../theme/palette';
import { radiusTokens, spacingTokens, typeTokens } from '../theme/tokens.generated';

/**
 * The Android sheet (X-DLG-02): a scrim, a card with rounded top corners and a handle, and a pinned footer. Back and a
 * scrim tap close it; focus stays inside it while it is open.
 */
export function BottomSheet({
  visible,
  title,
  onClose,
  children,
  footer,
}: {
  readonly visible: boolean;
  readonly title: string;
  readonly onClose: () => void;
  readonly children: ReactNode;
  readonly footer?: ReactNode;
}) {
  const palette = usePalette();
  return (
    <Modal transparent visible={visible} animationType="slide" onRequestClose={onClose} statusBarTranslucent>
      <View style={styles.root}>
        <Pressable accessible={false} onPress={onClose} style={[StyleSheet.absoluteFill, { backgroundColor: palette.scrim }]} />
        <View accessibilityViewIsModal style={[styles.sheet, { backgroundColor: palette['surface.overlay'] }]}>
          <View style={[styles.handle, { backgroundColor: palette['border.strong'] }]} />
          <Text accessibilityRole="header" style={[styles.title, { color: palette['text.primary'] }]}>
            {title}
          </Text>
          <View style={styles.body}>{children}</View>
          {footer !== undefined && <View style={styles.footer}>{footer}</View>}
        </View>
      </View>
    </Modal>
  );
}

const styles = StyleSheet.create({
  root: {
    flex: 1,
    justifyContent: 'flex-end',
  },
  sheet: {
    maxHeight: '90%',
    borderTopLeftRadius: radiusTokens['radius.2xl'],
    borderTopRightRadius: radiusTokens['radius.2xl'],
    paddingHorizontal: spacingTokens['space.4'],
    paddingBottom: spacingTokens['space.6'],
    gap: spacingTokens['space.3'],
    elevation: 6,
  },
  handle: {
    alignSelf: 'center',
    width: 32,
    height: 4,
    borderRadius: radiusTokens['radius.full'],
    marginTop: spacingTokens['space.2'],
  },
  title: {
    fontSize: typeTokens['a-title'].size,
    lineHeight: typeTokens['a-title'].lineHeight,
    fontWeight: '600',
  },
  body: {
    gap: spacingTokens['space.2'],
  },
  footer: {
    gap: spacingTokens['space.2'],
    paddingTop: spacingTokens['space.2'],
  },
});
