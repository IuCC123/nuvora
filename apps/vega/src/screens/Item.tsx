import React, { useCallback, useEffect, useState, type ReactNode } from 'react';
import { FlatList, ScrollView, StyleSheet, Text, View } from 'react-native';
import {
  describeError,
  formatClock,
  formatRuntime,
  hasOwnTitle,
  initialSeason,
  showCounts,
  showPlay,
  type Episode,
  type Image,
  type ItemDetail,
  type ShowDetail,
} from '@nuvora/core';
import { useNav } from '../nav';
import { useSignedIn } from '../state';
import { colors, gutter, radius, type } from '../theme';
import { Artwork, Button, Focusable, Loading, Problem, TitleCard } from '../ui';

/** Loads item `id` as `kind`; reloads each time the screen opens (back from the player, the resume point has moved). */
function useItem<K extends 'movie' | 'show'>(id: number, kind: K) {
  const { client } = useSignedIn();
  const [item, setItem] = useState<Extract<ItemDetail, { kind: K }> | null>(null);
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(() => {
    setError(null);
    client.item(id).then(
      (detail) => (detail.kind === kind ? setItem(detail as Extract<ItemDetail, { kind: K }>) : setError("That isn't on the server any more.")),
      (e: unknown) => setError(describeError(e)),
    );
  }, [client, id, kind]);
  useEffect(load, [load]);
  return { item, error, load };
}

/** The backdrop dimmed behind the page, poster on the left, details on the right. */
function DetailLayout(props: { backdrop: Image | null | undefined; poster: Image | null | undefined; title: string; year: number | null; children: ReactNode }) {
  return (
    <View style={styles.screen}>
      <Artwork image={props.backdrop} style={StyleSheet.absoluteFill} />
      <View style={styles.dim} />
      <View style={styles.top}>
        <View style={styles.poster}>{props.poster ? <Artwork image={props.poster} style={styles.fill} size="small" /> : <TitleCard title={props.title} year={props.year} />}</View>
        <View style={styles.info}>{props.children}</View>
      </View>
    </View>
  );
}

export function Movie({ id }: { id: number }) {
  const nav = useNav();
  const { item: movie, error, load } = useItem(id, 'movie');
  if (error) return <Problem message={error} onRetry={load} />;
  if (!movie) return <Loading />;
  const position = movie.watch.position_ms ?? 0;
  const facts = [movie.year, movie.runtime_ms ? formatRuntime(movie.runtime_ms) : null, movie.versions[0]?.summary].filter(Boolean).join(' · ');
  const play = (startMs?: number) => nav.push({ name: 'player', itemId: movie.id, ...(startMs !== undefined && { startMs }) });
  return (
    <DetailLayout backdrop={movie.backdrop} poster={movie.poster} title={movie.title} year={movie.year ?? null}>
      <Text style={styles.title} numberOfLines={2}>
        {movie.title}
      </Text>
      <Text style={styles.facts}>{facts}</Text>
      {movie.overview && (
        <Text style={styles.overview} numberOfLines={5}>
          {movie.overview}
        </Text>
      )}
      <View style={styles.actions}>
        {position > 0 ? (
          <>
            <Button label={`Resume from ${formatClock(position)}`} onPress={() => play()} primary preferred />
            <Button label="From the start" onPress={() => play(0)} />
          </>
        ) : (
          <Button label="Play" onPress={() => play(0)} primary preferred />
        )}
      </View>
      {movie.watch.watched && <Text style={styles.note}>Watched</Text>}
    </DetailLayout>
  );
}

export function Show({ id }: { id: number }) {
  const { client } = useSignedIn();
  const nav = useNav();
  const { item: show, error, load } = useItem(id, 'show');
  const [season, setSeason] = useState<number | null>(null);
  const [episodes, setEpisodes] = useState<Episode[] | null>(null);

  const seasonIndex = show ? (season ?? initialSeason(show) ?? null) : null;
  const seasonId = show?.seasons.find((s) => s.index === seasonIndex)?.id;
  useEffect(() => {
    if (seasonId === undefined) return;
    setEpisodes(null);
    client.episodes(seasonId).then(setEpisodes, () => setEpisodes([]));
  }, [client, seasonId]);

  if (error) return <Problem message={error} onRetry={load} />;
  if (!show) return <Loading />;
  const first = episodes?.[0] ? { id: episodes[0].id, label: episodes[0].label } : null;
  const next = showPlay(show, first);
  const playLabel = next && { start: `Play ${next.episode.label}`, resume: `Resume ${next.episode.label}`, next: `Play ${next.episode.label}`, rewatch: `Play ${next.episode.label} again` }[next.kind];

  return (
    <DetailLayout backdrop={show.backdrop} poster={show.poster} title={show.title} year={null}>
      <ScrollView>
        <Text style={styles.title} numberOfLines={2}>
          {show.title}
        </Text>
        <Text style={styles.facts}>{[show.year_range, showCounts(show)].filter(Boolean).join(' · ')}</Text>
        {show.overview && (
          <Text style={styles.overview} numberOfLines={3}>
            {show.overview}
          </Text>
        )}
        {next && playLabel && (
          <View style={styles.actions}>
            <Button label={playLabel} onPress={() => nav.push({ name: 'player', itemId: next.episode.id })} primary preferred />
          </View>
        )}
        <Seasons show={show} selected={seasonIndex} onSelect={setSeason} />
        {episodes === null ? null : (
          <FlatList
            style={styles.episodes}
            data={episodes}
            keyExtractor={(e) => String(e.id)}
            renderItem={({ item }) => <EpisodeRow episode={item} onPress={() => nav.push({ name: 'player', itemId: item.id })} />}
          />
        )}
      </ScrollView>
    </DetailLayout>
  );
}

function Seasons({ show, selected, onSelect }: { show: ShowDetail; selected: number | null; onSelect: (index: number) => void }) {
  if (show.seasons.length < 2) return null;
  return (
    <View style={styles.seasons}>
      {show.seasons.map((s) => (
        <Focusable key={s.id} onPress={() => onSelect(s.index)} style={(focused) => [styles.chip, s.index === selected && styles.chipOn, focused && styles.chipFocused]}>
          {(focused) => <Text style={[styles.chipLabel, s.index === selected && styles.chipLabelOn, focused && styles.chipLabelFocused]}>{s.title}</Text>}
        </Focusable>
      ))}
    </View>
  );
}

function EpisodeRow({ episode, onPress }: { episode: Episode; onPress: () => void }) {
  const position = episode.watch.position_ms ?? 0;
  const runtime = episode.runtime_ms ?? 0;
  const line = [episode.label, hasOwnTitle(episode) ? episode.title : null].filter(Boolean).join(' · ');
  const state = episode.watch.watched ? 'Watched' : position > 0 ? `${formatClock(position)} watched` : runtime ? formatRuntime(runtime) : '';
  return (
    <Focusable onPress={onPress} style={(focused) => [styles.episode, focused && styles.episodeFocused]}>
      {(focused) => (
        <>
          <Text style={[styles.episodeTitle, focused && styles.episodeTitleOn]} numberOfLines={1}>
            {line}
          </Text>
          <Text style={[styles.episodeState, focused && styles.episodeStateOn]}>{state}</Text>
        </>
      )}
    </Focusable>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, backgroundColor: colors.canvas },
  fill: { width: '100%', height: '100%' },
  dim: { ...StyleSheet.absoluteFillObject, backgroundColor: 'rgba(15,16,18,0.78)' },
  top: { flex: 1, flexDirection: 'row', paddingHorizontal: gutter, paddingTop: 44, paddingBottom: 20 },
  poster: { width: 160, height: 240, borderRadius: radius.md, overflow: 'hidden', marginRight: 32, backgroundColor: colors.placeholder },
  info: { flex: 1 },
  title: { ...type.headline, color: colors.text },
  facts: { ...type.bodyLarge, color: colors.secondary, marginTop: 6 },
  overview: { ...type.body, color: colors.text, marginTop: 12, maxWidth: 600 },
  actions: { flexDirection: 'row', gap: 12, marginTop: 18 },
  note: { ...type.small, color: colors.secondary, marginTop: 10 },
  seasons: { flexDirection: 'row', flexWrap: 'wrap', gap: 8, marginTop: 22, marginBottom: 6 },
  episodes: { marginTop: 16 },
  chip: { paddingHorizontal: 14, height: 32, borderRadius: radius.pill, justifyContent: 'center', backgroundColor: colors.surface },
  chipOn: { backgroundColor: colors.raised },
  chipFocused: { backgroundColor: colors.text },
  chipLabel: { ...type.body, fontWeight: '600', color: colors.secondary },
  chipLabelOn: { color: colors.text },
  chipLabelFocused: { color: colors.canvas },
  episode: { flexDirection: 'row', alignItems: 'center', paddingHorizontal: 14, height: 42, borderRadius: radius.md, marginTop: 6, backgroundColor: colors.surface },
  episodeFocused: { backgroundColor: colors.text },
  episodeTitle: { ...type.body, flex: 1, color: colors.text },
  episodeTitleOn: { color: colors.canvas },
  episodeState: { ...type.small, color: colors.secondary, marginLeft: 12 },
  episodeStateOn: { color: colors.canvas },
});
