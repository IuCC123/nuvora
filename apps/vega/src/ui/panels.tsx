import React, { type ReactNode } from 'react';
import { ScrollView, StyleSheet, Text, View, type StyleProp, type ViewStyle } from 'react-native';
import { Icon } from '../icons';
import { colors, radius, type } from '../theme';
import { Button } from './controls';
import { FocusGuide, Focusable } from './focus';

export function Loading() {
  return (
    <View style={styles.center}>
      <Text style={styles.loading}>Loading…</Text>
    </View>
  );
}

/** What went wrong, and the one thing that helps. */
export function ErrorState({ title, detail, action, onAction }: { title: string; detail: string; action?: string; onAction?: () => void }) {
  return (
    <View style={styles.center}>
      <View style={styles.error}>
        <Text style={styles.errorTitle}>{title}</Text>
        <Text style={styles.errorDetail}>{detail}</Text>
        {action && onAction && <Button label={action} onPress={onAction} primary preferred style={styles.errorAction} />}
      </View>
    </View>
  );
}

/**
 * A panel down the right edge over a dimmed screen, for picking one of a list.
 * Focus stays inside until it closes (Back).
 */
export function SidePanel({ title, width = 460, children }: { title: string; width?: number; children: ReactNode }) {
  return (
    <View style={styles.scrim}>
      <FocusGuide autoFocus trapFocusLeft trapFocusRight trapFocusUp trapFocusDown style={[styles.side, { width }]}>
        <Text style={styles.sideTitle}>{title}</Text>
        <ScrollView contentContainerStyle={styles.sideList}>{children}</ScrollView>
      </FocusGuide>
    </View>
  );
}

/** One choice in a side panel: a check when chosen, white when focused. */
export function ListRow({ label, detail, selected, onPress, preferred, disabled }: { label: string; detail?: string | null; selected: boolean; onPress: () => void; preferred?: boolean; disabled?: boolean }) {
  return (
    <Focusable onPress={onPress} preferred={preferred} disabled={disabled} style={(focused) => [styles.row, focused && styles.rowFocused]}>
      {(focused) => {
        const color = focused ? colors.canvas : disabled ? colors.muted : selected ? colors.accent : colors.text;
        return (
          <>
            <View style={styles.rowCheck}>{selected && <Icon name="check" size={18} color={color} />}</View>
            <View style={styles.rowText}>
              <Text style={[styles.rowLabel, { color }]} numberOfLines={1}>
                {label}
              </Text>
              {detail ? (
                <Text style={[styles.rowDetail, { color: focused ? colors.canvas : colors.muted }]} numberOfLines={1}>
                  {detail}
                </Text>
              ) : null}
            </View>
          </>
        );
      }}
    </Focusable>
  );
}

/** A rounded block of related settings. */
export function Panel({ children, style }: { children: ReactNode; style?: StyleProp<ViewStyle> }) {
  return <View style={[styles.panel, style]}>{children}</View>;
}

/** A panel's small capitals heading. */
export function Label({ text }: { text: string }) {
  return <Text style={styles.label}>{text}</Text>;
}

const styles = StyleSheet.create({
  center: { flex: 1, alignItems: 'center', justifyContent: 'center' },
  loading: { ...type.bodyLarge, color: colors.muted },
  error: { width: 560, alignItems: 'center' },
  errorTitle: { ...type.headlineSmall, color: colors.text, textAlign: 'center' },
  errorDetail: { ...type.bodyLarge, color: colors.secondary, textAlign: 'center', marginTop: 10 },
  errorAction: { marginTop: 24 },
  scrim: { ...StyleSheet.absoluteFillObject, backgroundColor: 'rgba(0,0,0,0.5)', alignItems: 'flex-end' },
  side: { height: '100%', backgroundColor: colors.surface, padding: 28 },
  sideTitle: { ...type.headlineSmall, color: colors.text, marginBottom: 18 },
  sideList: { gap: 6, paddingBottom: 12 },
  row: { flexDirection: 'row', alignItems: 'center', paddingHorizontal: 16, paddingVertical: 12, borderRadius: radius.lg },
  rowFocused: { backgroundColor: colors.text },
  rowCheck: { width: 28 },
  rowText: { flex: 1 },
  rowLabel: { ...type.titleSmall },
  rowDetail: { ...type.bodySmall },
  panel: { width: 860, borderRadius: radius.xl, backgroundColor: colors.surface, padding: 24, gap: 4 },
  label: { ...type.labelSmall, color: colors.muted, marginBottom: 4 },
});
