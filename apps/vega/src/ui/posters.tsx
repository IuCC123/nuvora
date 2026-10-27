import React from 'react';
import { FlatList, StyleSheet, Text, View, type StyleProp, type ViewStyle } from 'react-native';
import type { PosterCard as Card } from '@nuvora/core';
import { Icon } from '../icons';
import { colors, gutter, POSTER_WIDTH, radius, type } from '../theme';
import { Artwork, Badge, ProgressBar, TitleCard, WatchedMark } from './art';
import { FocusGuide, Focusable } from './focus';

/** A stable key for a card, for lists and for putting focus back on it. */
export const cardKey = (card: Card) => `${card.kind}:${card.id}`;

interface PosterProps {
  card: Card;
  onPress: () => void;
  onFocus?: () => void;
  preferred?: boolean;
  width?: number;
  /** Replaces the line under the title (`as Viole`). */
  subtitle?: string | null;
  style?: StyleProp<ViewStyle>;
}

/**
 * A poster in a row or grid: grows with an amber ring when focused, with
 * progress, watched and new-episode marks, and its title underneath.
 */
export function PosterCard({ card, onPress, onFocus, preferred, width = POSTER_WIDTH, subtitle, style }: PosterProps) {
  const second = subtitle !== undefined ? subtitle : (card.subtitle ?? (card.year != null ? String(card.year) : null));
  return (
    <View style={[{ width }, style]}>
      <Focusable onPress={onPress} onFocus={onFocus} preferred={preferred} scale={1.08} style={[styles.poster, { width, height: width * 1.5 }]}>
        {(focused) => (
          <>
            {card.poster ? <Artwork image={card.poster} style={StyleSheet.absoluteFill} /> : <TitleCard title={card.title} year={card.year} />}
            {!!card.unwatched_count && card.unwatched_count > 0 && !card.watched && <Badge text={String(card.unwatched_count)} style={styles.corner} />}
            {card.watched && card.progress == null && <WatchedMark style={styles.corner} />}
            {card.progress != null && <ProgressBar fraction={card.progress} style={styles.bottom} />}
            {focused && <View style={styles.ring} pointerEvents="none" />}
          </>
        )}
      </Focusable>
      <Text style={styles.title} numberOfLines={1}>
        {card.title}
      </Text>
      {second ? (
        <Text style={styles.second} numberOfLines={1}>
          {second}
        </Text>
      ) : null}
    </View>
  );
}

interface RowProps {
  title: string;
  cards: readonly Card[];
  onOpen: (card: Card) => void;
  onFocus?: (card: Card) => void;
  /** The card to focus when the row appears (by key), else none. */
  preferredKey?: string | null;
  /** A last card that opens the whole list: `total` titles. */
  onSeeAll?: () => void;
  total?: number;
  style?: StyleProp<ViewStyle>;
}

/** A titled row of posters; coming back to it lands on the card focus left. */
export function PosterRow({ title, cards, onOpen, onFocus, preferredKey, onSeeAll, total, style }: RowProps) {
  const items: (Card | 'see-all')[] = onSeeAll ? [...cards, 'see-all'] : [...cards];
  return (
    <View style={style}>
      <Text style={styles.rowTitle}>{title}</Text>
      <FocusGuide autoFocus>
        <FlatList
          horizontal
          data={items}
          keyExtractor={(item) => (item === 'see-all' ? 'see-all' : cardKey(item))}
          showsHorizontalScrollIndicator={false}
          contentContainerStyle={styles.rowItems}
          initialNumToRender={9}
          maxToRenderPerBatch={9}
          windowSize={5}
          renderItem={({ item }) =>
            item === 'see-all' ? (
              <SeeAll total={total ?? 0} onPress={onSeeAll!} />
            ) : (
              <PosterCard card={item} preferred={preferredKey === cardKey(item)} onPress={() => onOpen(item)} onFocus={() => onFocus?.(item)} />
            )
          }
        />
      </FocusGuide>
    </View>
  );
}

/** The end of a row: the library behind it. */
function SeeAll({ total, onPress }: { total: number; onPress: () => void }) {
  return (
    <View style={{ width: POSTER_WIDTH }}>
      <Focusable onPress={onPress} scale={1.08} style={(focused) => [styles.poster, styles.seeAll, { height: POSTER_WIDTH * 1.5 }, focused && { backgroundColor: colors.raised }]}>
        {(focused) => (
          <>
            <View style={styles.seeAllIcon}>
              <Icon name="arrowRight" size={22} color={colors.text} />
            </View>
            <Text style={styles.seeAllLabel}>See all</Text>
            {total > 0 && <Text style={styles.second}>{total}</Text>}
            {focused && <View style={styles.ring} pointerEvents="none" />}
          </>
        )}
      </Focusable>
    </View>
  );
}

const styles = StyleSheet.create({
  poster: { borderRadius: radius.sm, overflow: 'hidden', backgroundColor: colors.surface },
  ring: { ...StyleSheet.absoluteFillObject, borderRadius: radius.sm, borderWidth: 3, borderColor: colors.accent },
  corner: { position: 'absolute', top: 6, right: 6 },
  bottom: { position: 'absolute', left: 0, right: 0, bottom: 0 },
  title: { ...type.titleSmall, color: colors.text, marginTop: 8 },
  second: { ...type.bodySmall, color: colors.muted },
  rowTitle: { ...type.titleMedium, color: colors.text, marginLeft: gutter, marginBottom: 8 },
  rowItems: { paddingHorizontal: gutter, paddingVertical: 10, gap: 18 },
  seeAll: { width: POSTER_WIDTH, alignItems: 'center', justifyContent: 'center' },
  seeAllIcon: { width: 44, height: 44, borderRadius: 22, alignItems: 'center', justifyContent: 'center', backgroundColor: colors.hairline, marginBottom: 12 },
  seeAllLabel: { ...type.titleSmall, color: colors.text },
});
