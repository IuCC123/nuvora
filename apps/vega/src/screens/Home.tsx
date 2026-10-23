import React, { useCallback, useEffect, useState } from 'react';
import { FlatList, ScrollView, StyleSheet, Text, View } from 'react-native';
import { describeError, type HomeResponse, type PosterCard } from '@nuvora/core';
import { useNav, type Nav } from '../nav';
import { useSignedIn } from '../state';
import { colors, gutter, type } from '../theme';
import { Button, Loading, Poster, Problem } from '../ui';

/** Where a poster goes: its page, or (an episode in Continue Watching) straight back into it. */
function open(nav: Nav, card: PosterCard) {
  if (card.kind === 'movie') nav.push({ name: 'movie', id: card.id });
  else if (card.kind === 'show') nav.push({ name: 'show', id: card.id });
  else if (card.kind === 'episode') nav.push({ name: 'player', itemId: card.id });
}

export function Home() {
  const { client, session, signOut } = useSignedIn();
  const nav = useNav();
  const [home, setHome] = useState<HomeResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [focused, setFocused] = useState<PosterCard | null>(null);

  const load = useCallback(() => {
    setError(null);
    client.home().then(setHome, (e: unknown) => setError(describeError(e)));
  }, [client]);
  useEffect(load, [load]);

  if (error) return <Problem message={error} onRetry={load} />;
  if (!home) return <Loading />;
  const rows = home.rows.filter((row) => row.items.length > 0);

  return (
    <View style={styles.screen}>
      <View style={styles.header}>
        <View style={styles.hero}>
          <Text style={styles.heroTitle} numberOfLines={1}>
            {focused?.title ?? `Welcome, ${session.user.display_name}`}
          </Text>
          <Text style={styles.heroLine} numberOfLines={1}>
            {focused ? [focused.subtitle ?? focused.year, focused.kind === 'show' && focused.unwatched_count ? `${focused.unwatched_count} unwatched` : null].filter(Boolean).join(' · ') : session.server.name}
          </Text>
        </View>
        <Button label="Sign out" onPress={signOut} />
      </View>
      {rows.length === 0 ? (
        <Text style={styles.empty}>Nothing to watch yet. Add movies or shows to a library on your server, then scan it.</Text>
      ) : (
        <ScrollView contentContainerStyle={styles.rows}>
          {rows.map((row, r) => (
            <View key={row.id ?? `${row.kind}-${row.library_id ?? r}`} style={styles.row}>
              <Text style={styles.rowTitle}>{row.title}</Text>
              <FlatList
                horizontal
                data={row.items}
                keyExtractor={(card) => `${card.kind}-${card.id}`}
                showsHorizontalScrollIndicator={false}
                contentContainerStyle={styles.rowItems}
                renderItem={({ item, index }) => (
                  <Poster card={item} preferred={r === 0 && index === 0} onFocus={() => setFocused(item)} onPress={() => open(nav, item)} />
                )}
              />
            </View>
          ))}
        </ScrollView>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, backgroundColor: colors.canvas },
  header: { flexDirection: 'row', alignItems: 'flex-start', paddingHorizontal: gutter, paddingTop: 30, paddingBottom: 4 },
  hero: { flex: 1, marginRight: 24 },
  heroTitle: { ...type.headline, color: colors.text },
  heroLine: { ...type.bodyLarge, color: colors.secondary, marginTop: 2 },
  rows: { paddingBottom: 40 },
  row: { marginTop: 14 },
  rowTitle: { ...type.title, fontSize: 18, color: colors.text, marginLeft: gutter, marginBottom: 6 },
  rowItems: { paddingHorizontal: gutter, paddingVertical: 10 },
  empty: { ...type.bodyLarge, color: colors.secondary, marginHorizontal: gutter, marginTop: 24 },
});
