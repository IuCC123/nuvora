import React, { forwardRef, useState, type ReactNode } from 'react';
import { ActivityIndicator, Image, Pressable, StyleSheet, Text, TextInput, View, type StyleProp, type ViewStyle } from 'react-native';
import { titleCardLook, type Image as ArtworkImage, type PosterCard } from '@nuvora/core';
import { colors, gutter, radius, type } from './theme';
import { imageSource } from './server';
import { useSignedIn } from './state';

interface FocusableProps {
  onPress?: () => void;
  onFocus?: () => void;
  preferred?: boolean;
  style?: StyleProp<ViewStyle> | ((focused: boolean) => StyleProp<ViewStyle>);
  children: (focused: boolean) => ReactNode;
}

/** Anything the remote can land on and press. */
export function Focusable({ onPress, onFocus, preferred, style, children }: FocusableProps) {
  const [focused, setFocused] = useState(false);
  return (
    <Pressable
      onPress={onPress}
      onFocus={() => {
        setFocused(true);
        onFocus?.();
      }}
      onBlur={() => setFocused(false)}
      hasTVPreferredFocus={preferred}
      style={typeof style === 'function' ? style(focused) : style}>
      {children(focused)}
    </Pressable>
  );
}

export function Button({ label, onPress, primary, preferred }: { label: string; onPress: () => void; primary?: boolean; preferred?: boolean }) {
  return (
    <Focusable onPress={onPress} preferred={preferred} style={(focused) => [styles.button, primary && styles.primary, focused && (primary ? styles.primaryFocused : styles.buttonFocused)]}>
      {(focused) => <Text style={[styles.buttonLabel, (primary || focused) && styles.buttonLabelOn]}>{label}</Text>}
    </Focusable>
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
}

export const Field = forwardRef<TextInput, FieldProps>(function Field(props, ref) {
  const [focused, setFocused] = useState(false);
  return (
    <View style={styles.field}>
      <Text style={styles.fieldLabel}>{props.label}</Text>
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
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
        style={[styles.input, focused && styles.inputFocused]}
      />
    </View>
  );
});

const POSTER_WIDTH = 118;

/** A poster, or a title card in the title's palette when there is no artwork. */
export function Poster({ card, onPress, onFocus, preferred }: { card: PosterCard; onPress: () => void; onFocus?: () => void; preferred?: boolean }) {
  const { session } = useSignedIn();
  return (
    <Focusable onPress={onPress} onFocus={onFocus} preferred={preferred} style={styles.posterSlot}>
      {(focused) => (
        <View style={[styles.poster, focused && styles.posterFocused]}>
          {card.poster ? (
            <Image source={imageSource(session.server.url, session.token, card.poster)} style={styles.posterImage} />
          ) : (
            <TitleCard title={card.title} year={card.year ?? null} />
          )}
          {card.progress != null && card.progress > 0 && (
            <View style={styles.progressTrack}>
              <View style={[styles.progressFill, { width: `${Math.round(card.progress * 100)}%` }]} />
            </View>
          )}
        </View>
      )}
    </Focusable>
  );
}

export function TitleCard({ title, year }: { title: string; year: number | null }) {
  const look = titleCardLook(title);
  return (
    <View style={[styles.titleCard, { backgroundColor: look.palette.top }]}>
      <View style={[styles.titleCardFoot, { backgroundColor: look.palette.bottom }]} />
      <Text style={styles.titleCardText} numberOfLines={3}>
        {title.toUpperCase()}
      </Text>
      {year != null && <Text style={styles.titleCardYear}>{year}</Text>}
    </View>
  );
}

export function Artwork({ image, style, size = 'large' }: { image: ArtworkImage | null | undefined; style: StyleProp<ViewStyle>; size?: 'small' | 'large' }) {
  const { session } = useSignedIn();
  if (!image) return null;
  return <Image source={imageSource(session.server.url, session.token, image, size)} style={style as never} resizeMode="cover" />;
}

export function Loading() {
  return (
    <View style={styles.center}>
      <ActivityIndicator size="large" color={colors.accent} />
    </View>
  );
}

export function Problem({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return (
    <View style={styles.center}>
      <Text style={[type.title, { color: colors.text }]}>Something went wrong</Text>
      <Text style={[type.body, styles.problemText]}>{message}</Text>
      {onRetry && <Button label="Try again" onPress={onRetry} primary preferred />}
    </View>
  );
}

const styles = StyleSheet.create({
  button: { paddingHorizontal: 20, height: 40, borderRadius: radius.pill, justifyContent: 'center', backgroundColor: colors.raised },
  buttonFocused: { backgroundColor: colors.text },
  primary: { backgroundColor: colors.accent },
  primaryFocused: { backgroundColor: '#f5b52a', transform: [{ scale: 1.05 }] },
  buttonLabel: { ...type.bodyLarge, fontWeight: '600', color: colors.text },
  buttonLabelOn: { color: colors.canvas },
  field: { marginBottom: 14 },
  fieldLabel: { ...type.small, color: colors.secondary, marginBottom: 6 },
  input: { ...type.bodyLarge, color: colors.text, height: 44, paddingHorizontal: 14, borderRadius: radius.md, backgroundColor: colors.surface, borderWidth: 2, borderColor: colors.hairline },
  inputFocused: { borderColor: colors.accent },
  posterSlot: { marginRight: 14 },
  poster: { width: POSTER_WIDTH, height: POSTER_WIDTH * 1.5, borderRadius: radius.sm, overflow: 'hidden', backgroundColor: colors.placeholder, borderWidth: 2, borderColor: 'transparent' },
  posterFocused: { borderColor: colors.accent, transform: [{ scale: 1.08 }] },
  posterImage: { width: '100%', height: '100%' },
  progressTrack: { position: 'absolute', left: 0, right: 0, bottom: 0, height: 3, backgroundColor: 'rgba(0,0,0,0.6)' },
  progressFill: { height: '100%', backgroundColor: colors.accent },
  titleCard: { flex: 1, justifyContent: 'flex-end', padding: 9 },
  titleCardFoot: { position: 'absolute', left: 0, right: 0, bottom: 0, height: '55%' },
  titleCardText: { fontSize: 17, lineHeight: 18, fontWeight: '800', color: '#f7f2ea' },
  titleCardYear: { fontSize: 9, color: 'rgba(247,242,234,0.7)', marginTop: 2 },
  center: { flex: 1, alignItems: 'center', justifyContent: 'center', padding: gutter },
  problemText: { color: colors.secondary, marginTop: 6, marginBottom: 18, textAlign: 'center', maxWidth: 520 },
});
