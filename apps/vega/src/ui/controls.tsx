import React, { forwardRef, useState } from 'react';
import { StyleSheet, Text, TextInput, View, type StyleProp, type ViewStyle } from 'react-native';
import { Icon, type IconName } from '../icons';
import { colors, radius, type } from '../theme';
import { Focusable } from './focus';

interface ButtonProps {
  label: string;
  onPress: () => void;
  icon?: IconName;
  /** The amber one. */
  primary?: boolean;
  preferred?: boolean;
  disabled?: boolean;
  onFocus?: () => void;
  style?: StyleProp<ViewStyle>;
}

/** A pill button, as on the Fire TV app: amber when primary, white when focused. */
export const Button = forwardRef<View, ButtonProps>(function Button({ label, onPress, icon, primary, preferred, disabled, onFocus, style }, ref) {
  return (
    <Focusable
      ref={ref}
      onPress={onPress}
      onFocus={onFocus}
      preferred={preferred}
      disabled={disabled}
      scale={1.1}
      style={(focused) => [styles.button, primary ? (focused ? styles.primaryFocused : styles.primary) : focused && styles.buttonFocused, disabled && styles.disabled, style]}>
      {(focused) => {
        const color = primary ? colors.accentText : focused ? colors.canvas : colors.text;
        return (
          <>
            {icon && <Icon name={icon} size={20} color={color} />}
            <Text style={[styles.buttonLabel, { color }]} numberOfLines={1}>
              {label}
            </Text>
          </>
        );
      }}
    </Focusable>
  );
});

/** A choice among a few: amber when chosen. */
export function Chip({ label, selected, onPress, preferred, style }: { label: string; selected: boolean; onPress: () => void; preferred?: boolean; style?: StyleProp<ViewStyle> }) {
  return (
    <Focusable
      onPress={onPress}
      preferred={preferred}
      scale={1.1}
      style={(focused) => [styles.chip, focused && { backgroundColor: selected ? colors.accent : colors.text }, style]}>
      {(focused) => (
        <Text style={[styles.chipLabel, { color: focused ? (selected ? colors.accentText : colors.canvas) : selected ? colors.accent : colors.secondary }]} numberOfLines={1}>
          {label}
        </Text>
      )}
    </Focusable>
  );
}

/** One of `options` (value and label), as a row of chips. */
export function OptionChips<T extends string>({ options, selected, onSelect }: { options: readonly (readonly [T, string])[]; selected: T; onSelect: (value: T) => void }) {
  return (
    <View style={styles.chips}>
      {options.map(([value, label]) => (
        <Chip key={value} label={label} selected={value === selected} onPress={() => onSelect(value)} />
      ))}
    </View>
  );
}

interface FieldProps {
  label: string;
  value: string;
  onChangeText: (text: string) => void;
  onSubmit?: () => void;
  placeholder?: string;
  secure?: boolean;
  preferred?: boolean;
  onFocus?: () => void;
  style?: StyleProp<ViewStyle>;
}

/** An outlined text field with its label on the border, as on the Fire TV app. */
export const Field = forwardRef<TextInput, FieldProps>(function Field(props, ref) {
  const [focused, setFocused] = useState(false);
  return (
    <View style={[styles.field, focused && styles.fieldFocused, props.style]}>
      <TextInput
        ref={ref}
        value={props.value}
        onChangeText={props.onChangeText}
        onSubmitEditing={props.onSubmit}
        placeholder={props.placeholder}
        placeholderTextColor={colors.muted}
        secureTextEntry={props.secure}
        autoCapitalize="none"
        autoCorrect={false}
        hasTVPreferredFocus={props.preferred}
        onFocus={() => {
          setFocused(true);
          props.onFocus?.();
        }}
        onBlur={() => setFocused(false)}
        style={styles.input}
      />
      <Text style={[styles.fieldLabel, focused && { color: colors.accent }]}>{props.label}</Text>
    </View>
  );
});

const styles = StyleSheet.create({
  button: { flexDirection: 'row', alignItems: 'center', gap: 10, height: 44, paddingHorizontal: 24, borderRadius: radius.pill, backgroundColor: colors.raised },
  buttonFocused: { backgroundColor: colors.text },
  primary: { backgroundColor: colors.accent },
  primaryFocused: { backgroundColor: colors.accentFocused },
  disabled: { opacity: 0.5 },
  buttonLabel: { ...type.labelLarge },
  chip: { height: 36, paddingHorizontal: 16, borderRadius: radius.md, justifyContent: 'center', backgroundColor: colors.raised },
  chipLabel: { ...type.labelLarge },
  chips: { flexDirection: 'row', gap: 10 },
  field: { height: 56, borderRadius: radius.lg, borderWidth: 1, borderColor: colors.hairline, backgroundColor: colors.surface, justifyContent: 'center' },
  fieldFocused: { borderWidth: 2, borderColor: colors.accent },
  fieldLabel: { position: 'absolute', left: 12, top: -9, paddingHorizontal: 4, ...type.labelMedium, color: colors.secondary, backgroundColor: colors.canvas },
  input: { ...type.bodyLarge, color: colors.text, paddingHorizontal: 16, paddingVertical: 0, height: '100%', textAlignVertical: 'center' },
});
