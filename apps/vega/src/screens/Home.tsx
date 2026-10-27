import React, { useState } from 'react';
import { Image, ScrollView, StyleSheet, Text, View } from 'react-native';
import { placeholderDataUrl, type HomeResponse, type HomeRow, type PosterCard } from '@nuvora/core';
import { useLoad } from '../load';
import { open, useNav } from '../nav';
import { useSignedIn } from '../state';
import { colors, gutter, type } from '../theme';
import { cardKey, ErrorState, Gradient, Loading, PosterRow } from '../ui';
import { Shell } from './Shell';

/** Continue Watching, Recently Added and each library, as rows of posters. */
export function Home() {
  const { client } = useSignedIn();
  const [home, reload] = useLoad('home', () => client.home(), { refresh: true });
  return (
    <Shell section="home">
      {home.kind === 'loading' ? (
        <Loading />
      ) : home.kind === 'failed' ? (
        <ErrorState title="Can't load Home" detail={home.error} action="Try again" onAction={reload} />
      ) : (
        <Rows home={home.value} />
      )}
    </Shell>
  );
}

const rowKey = (row: HomeRow) => `${row.kind}:${row.library_id ?? ''}:${row.title}`;

function Rows({ home }: { home: HomeResponse }) {
  const nav = useNav();
  const rows = home.rows.filter((row) => row.items.length > 0);
  const [focused, setFocused] = useState<PosterCard | null>(rows[0]?.items[0] ?? null);
  if (!rows.length) {
    return <ErrorState title="Nothing here yet" detail="Add a library on the server (Settings › Libraries in a browser), and it shows up here after a scan." />;
  }
  const first = rows[0]!;
  return (
    <View style={styles.screen}>
      <Backdrop card={focused} />
      <View style={styles.info}>
        <Text style={styles.title} numberOfLines={1}>
          {focused?.title ?? ''}
        </Text>
        <Text style={styles.second} numberOfLines={1}>
          {focused?.subtitle ?? (focused?.year != null ? String(focused.year) : '')}
        </Text>
      </View>
      <ScrollView style={styles.rows} contentContainerStyle={styles.rowsContent}>
        {rows.map((row) => (
          <PosterRow
            key={rowKey(row)}
            title={row.title}
            cards={row.items}
            preferredKey={row === first ? cardKey(first.items[0]!) : null}
            onOpen={(card) => open(nav, card)}
            onFocus={setFocused}
            onSeeAll={row.library_id != null ? () => nav.section({ name: 'library', id: row.library_id!, title: row.title }) : undefined}
            total={row.total}
            style={styles.row}
          />
        ))}
      </ScrollView>
    </View>
  );
}

/**
 * A soft wash of the focused poster's colors behind the top of the screen:
 * its ThumbHash, stretched, is already a perfect blur.
 */
function Backdrop({ card }: { card: PosterCard | null }) {
  const poster = card?.poster;
  return (
    <View style={StyleSheet.absoluteFill} pointerEvents="none">
      {poster?.placeholder ? <Image source={{ uri: placeholderDataUrl(poster) }} style={styles.wash} resizeMode="cover" /> : null}
      <Gradient direction="right" stops={[[0.25, colors.canvas, 1], [0.75, colors.canvas, 0]]} />
      <Gradient direction="down" stops={[[0.2, colors.canvas, 0], [0.62, colors.canvas, 1]]} />
    </View>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1 },
  wash: { position: 'absolute', top: 0, right: 0, width: '70%', height: '75%', opacity: 0.55 },
  info: { paddingHorizontal: gutter, paddingTop: 36, height: 36 + 118 },
  title: { ...type.display, color: colors.text },
  second: { ...type.titleMedium, color: colors.secondary, marginTop: 6 },
  rows: { flex: 1 },
  rowsContent: { paddingBottom: 48 },
  row: { marginBottom: 18 },
});
