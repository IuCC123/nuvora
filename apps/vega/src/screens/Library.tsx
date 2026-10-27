import React, { useCallback, useEffect, useRef, useState } from 'react';
import { FlatList, StyleSheet, Text, View } from 'react-native';
import { describeError, type LibrarySort, type PosterCard as Card, type SortOrder } from '@nuvora/core';
import { open, useNav } from '../nav';
import { useSignedIn } from '../state';
import { colors, gutter, POSTER_WIDTH, type } from '../theme';
import { cardKey, Chip, ErrorState, FocusGuide, Loading, PosterCard } from '../ui';
import { Shell } from './Shell';

const SORTS: readonly { key: string; label: string; sort: LibrarySort; order: SortOrder }[] = [
  { key: 'title', label: 'A–Z', sort: 'title', order: 'asc' },
  { key: 'added', label: 'Recently added', sort: 'added', order: 'desc' },
  { key: 'year', label: 'Newest', sort: 'year', order: 'desc' },
];

const PAGE = 60;
const GAP = 22;
/** The grid's width: the screen less the side menu and both margins. */
const GRID_WIDTH = 960 - 80 - gutter * 2;
const COLUMNS = Math.floor((GRID_WIDTH + GAP) / (POSTER_WIDTH + GAP));
const CELL = Math.floor((GRID_WIDTH - GAP * (COLUMNS - 1)) / COLUMNS);

interface Pages {
  cards: Card[];
  next: string | null;
  total: number | null;
  error: string | null;
}

/** Every movie or show of one library, as a grid that loads as you scroll. */
export function Library({ id, title }: { id: number; title: string }) {
  const { client } = useSignedIn();
  const nav = useNav();
  const [sort, setSort] = useState(SORTS[0]!);
  const [unwatched, setUnwatched] = useState(false);
  const [pages, setPages] = useState<Pages>({ cards: [], next: null, total: null, error: null });
  const loading = useRef(false);
  const view = `${sort.key}|${unwatched}`;
  const viewRef = useRef(view);
  viewRef.current = view;

  const more = useCallback(
    async (fresh: boolean) => {
      if (loading.current && !fresh) return;
      loading.current = true;
      const asked = viewRef.current;
      try {
        const after = fresh ? null : pages.next;
        if (!fresh && pages.total !== null && after === null) return;
        const page = await client.libraryItems(id, { sort: sort.sort, order: sort.order, filter: unwatched ? 'unwatched' : 'all' }, after, PAGE);
        if (asked !== viewRef.current) return;
        setPages((p) => ({ cards: fresh ? page.items : [...p.cards, ...page.items], next: page.next ?? null, total: page.total, error: null }));
      } catch (e) {
        if (asked === viewRef.current) setPages((p) => ({ ...p, error: describeError(e) }));
      } finally {
        loading.current = false;
      }
    },
    [client, id, sort, unwatched, pages.next, pages.total],
  );

  useEffect(() => {
    setPages({ cards: [], next: null, total: null, error: null });
    void more(true);
    // A new view starts over; `more` itself changes with every page.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id, view]);

  const { cards, total, error } = pages;
  return (
    <Shell section={`library:${id}`}>
      <View style={styles.header}>
        <Text style={styles.title}>{title}</Text>
        {total != null && <Text style={styles.count}>{total}</Text>}
        <View style={styles.spacer} />
        <FocusGuide autoFocus style={styles.chips}>
          {SORTS.map((s) => (
            <Chip key={s.key} label={s.label} selected={s.key === sort.key} onPress={() => setSort(s)} />
          ))}
          <View style={styles.chipGap} />
          <Chip label="Unwatched" selected={unwatched} onPress={() => setUnwatched((u) => !u)} />
        </FocusGuide>
      </View>
      {error && cards.length === 0 ? (
        <ErrorState title={`Can't load ${title}`} detail={error} />
      ) : cards.length === 0 && total === null ? (
        <Loading />
      ) : cards.length === 0 ? (
        <ErrorState title="Nothing here" detail={unwatched ? `You've watched everything in ${title}.` : `${title} is empty.`} />
      ) : (
        <FocusGuide autoFocus style={styles.gridWrap}>
          <FlatList
            key={view}
            data={cards}
            numColumns={COLUMNS}
            keyExtractor={cardKey}
            contentContainerStyle={styles.grid}
            columnWrapperStyle={styles.gridRow}
            initialNumToRender={COLUMNS * 3}
            maxToRenderPerBatch={COLUMNS * 2}
            windowSize={7}
            onEndReached={() => void more(false)}
            onEndReachedThreshold={1.5}
            renderItem={({ item, index }) => <PosterCard card={item} width={CELL} preferred={index === 0} onPress={() => open(nav, item)} />}
          />
        </FocusGuide>
      )}
    </Shell>
  );
}

const styles = StyleSheet.create({
  header: { flexDirection: 'row', alignItems: 'center', paddingHorizontal: gutter, paddingTop: 36 },
  title: { ...type.headlineMedium, color: colors.text },
  count: { ...type.titleMedium, color: colors.muted, marginLeft: 14 },
  spacer: { flex: 1 },
  chips: { flexDirection: 'row', gap: 10 },
  chipGap: { width: 12 },
  gridWrap: { flex: 1 },
  grid: { paddingHorizontal: gutter, paddingTop: 24, paddingBottom: 48 },
  gridRow: { gap: GAP, marginBottom: 26 },
});
