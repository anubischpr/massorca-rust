import { Text } from 'react-native';

import { ActionButton } from '../components/action-button';
import { BottomSheet } from '../components/bottom-sheet';
import { usePalette } from '../theme/palette';
import { typeTokens } from '../theme/tokens.generated';
import { INITIALIZE_GIT, NOT_NOW, NO_GIT_BODY, NO_GIT_GATE, NO_GIT_TITLE } from '../copy/folder-without-git';

/**
 * The sheet of a plain folder (M-WS-05), opened from its No Git badge. It says what the folder lacks. Initialize Git is
 * drawn but disabled: this build has no call that starts it, so the gate line says which MassOrca it needs.
 */
export function FolderWithoutGitSheet({
  visible,
  folder,
  onClose,
}: {
  readonly visible: boolean;
  readonly folder: string;
  readonly onClose: () => void;
}) {
  const palette = usePalette();
  return (
    <BottomSheet
      visible={visible}
      title={NO_GIT_TITLE}
      onClose={onClose}
      footer={
        <>
          {/* Disabled, so its press never runs: there is no call to start Git yet. */}
          <ActionButton label={INITIALIZE_GIT} variant="primary" disabled onPress={() => undefined} />
          <ActionButton label={NOT_NOW} variant="secondary" onPress={onClose} />
        </>
      }
    >
      <Text style={{ color: palette['text.secondary'], fontSize: typeTokens['a-body'].size, lineHeight: typeTokens['a-body'].lineHeight }}>
        {NO_GIT_BODY(folder)}
      </Text>
      <Text style={{ color: palette['text.tertiary'], fontSize: typeTokens['a-caption'].size, lineHeight: typeTokens['a-caption'].lineHeight }}>
        {NO_GIT_GATE}
      </Text>
    </BottomSheet>
  );
}
