import React, { useEffect, useState } from 'react';
import { ScrollView, StyleSheet, Text, View } from 'react-native';
import { describeError, initials, type PosterCard as Card, type SearchResponse } from '@nuvora/core';
import { open, useNav } from '../nav';
import { useSignedIn } from '../state';
import { colors, gutter, type } from '../theme';
import { Artwork, Field, FocusGuide, PosterCard } from '../ui';
import { Shell } from './Shell';

/** Type with the remote; results follow as you type. */
export function Search() {
  const { client } = useSignedIn();
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<SearchResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const q = query.trim();
    if (!q) {
      setResults(null);
      return;
    }
    let current = true;
    const timer = setTimeout(() => {
      client.search(q).then(
        (found) => {
          if (!current) return;
          setResults(found);
          setError(null);
        },
        (e: unknown) => current && setError(describeError(e)),
      );
    }, 300);
    return () => {
      current = false;
      clearTimeout(timer);
    };
  }, [client, query]);

  const empty = results && !results.titles.length && !results.episodes.length && !results.people.length;
  return (
    <Shell section="search">
      <View style={styles.top}>
        <Field label="Search" value={query} onChangeText={setQuery} placeholder="Movies, shows, episodes, people" preferred style={styles.field} />
      </View>
      {error ? (
        <Message text={error} />
      ) : !query.trim() ? (
        <Message text="Find anything by its title, an episode by its name, or an actor or voice actor." />
      ) : empty ? (
        <Message text={`Nothing matches “${query.trim()}”.`} />
      ) : results ? (
        <Results found={results} />
      ) : null}
    </Shell>
  );
}

function Results({ found }: { found: SearchResponse }) {
  const nav = useNav();
  const episodes: Card[] = found.episodes.map((e) => ({
    id: e.id,
    kind: 'episode',
    title: e.show_title,
    poster: e.poster ?? null,
    subtitle: `S${e.season} · E${e.episode} · ${e.title}`,
  })) as Card[];
  return (
    <FocusGuide autoFocus style={styles.results}>
      <ScrollView contentContainerStyle={styles.resultsContent}>
        {found.titles.length > 0 && (
          <Section title="Movies & shows">
            {found.titles.map((card) => (
              <PosterCard key={`t${card.kind}${card.id}`} card={card} onPress={() => open(nav, card)} />
            ))}
          </Section>
        )}
        {episodes.length > 0 && (
          <Section title="Episodes">
            {episodes.map((card) => (
              <PosterCard key={`e${card.id}`} card={card} onPress={() => nav.push({ name: 'player', itemId: card.id })} />
            ))}
          </Section>
        )}
        {found.people.length > 0 && <Text style={styles.heading}>People</Text>}
        {found.people.map((person, n) => (
          <View key={`p${n}`} style={styles.person}>
            <View style={styles.personLine}>
              <View style={styles.face}>{person.image ? <Artwork image={person.image} style={StyleSheet.absoluteFill} /> : <Text style={styles.faceText}>{initials(person.name)}</Text>}</View>
              <Text style={styles.personName}>{person.name}</Text>
            </View>
            <View style={styles.grid}>
              {person.roles.map((role) => (
                <PosterCard
                  key={`p${n}-${role.card.kind}${role.card.id}`}
                  card={role.card}
                  subtitle={role.character ? `as ${role.character}` : undefined}
                  onPress={() => open(nav, role.card)}
                />
              ))}
            </View>
          </View>
        ))}
      </ScrollView>
    </FocusGuide>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <View style={styles.section}>
      <Text style={styles.heading}>{title}</Text>
      <View style={styles.grid}>{children}</View>
    </View>
  );
}

function Message({ text }: { text: string }) {
  return <Text style={styles.message}>{text}</Text>;
}

const styles = StyleSheet.create({
  top: { paddingHorizontal: gutter, paddingTop: 36 },
  field: { width: '60%', borderRadius: 28 },
  message: { ...type.bodyLarge, color: colors.secondary, marginLeft: gutter + 4, marginTop: 28 },
  results: { flex: 1 },
  resultsContent: { paddingHorizontal: gutter, paddingTop: 28, paddingBottom: 48 },
  section: { marginBottom: 26 },
  heading: { ...type.titleLarge, color: colors.text, marginBottom: 14 },
  grid: { flexDirection: 'row', flexWrap: 'wrap', columnGap: 22, rowGap: 26 },
  person: { marginBottom: 26 },
  personLine: { flexDirection: 'row', alignItems: 'center', marginBottom: 14 },
  face: { width: 56, height: 56, borderRadius: 28, overflow: 'hidden', alignItems: 'center', justifyContent: 'center', backgroundColor: colors.raised, marginRight: 16 },
  faceText: { ...type.labelLarge, color: colors.secondary },
  personName: { ...type.titleMedium, color: colors.text },
});
