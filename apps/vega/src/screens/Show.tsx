import React, { useEffect, useState } from 'react';
import { FlatList, StyleSheet, Text, View } from 'react-native';
import { formatClock, formatRuntime, type Episode, type ItemDetail } from '@nuvora/core';
import { useLoad } from '../load';
import { useNav } from '../nav';
import { useSignedIn } from '../state';
import { colors, radius, type } from '../theme';
import { Artwork, Button, Chip, ErrorState, FocusGuide, Focusable, Loading, ProgressBar, WatchedMark } from '../ui';
import { CastRow, DETAIL_INSET, DetailLayout } from './Detail';

type ShowItem = Extract<ItemDetail, { kind: 'show' }>;

export function Show({ id }: { id: number }) {
  const { client } = useSignedIn();
  const nav = useNav();
  const [item, reload] = useLoad(id, () => client.item(id), { refresh: true });
  if (item.kind === 'loading') return <Loading />;
  if (item.kind === 'failed') return <ErrorState title="Can't open this show" detail={item.error} action="Back" onAction={nav.pop} />;
  if (item.value.kind !== 'show') return <ErrorState title="This isn't a show" detail="It may have been changed on the server." action="Back" onAction={nav.pop} />;
  return <ShowPage show={item.value} reload={reload} />;
}

function ShowPage({ show, reload }: { show: ShowItem; reload: () => void }) {
  const { client } = useSignedIn();
  const nav = useNav();
  const next = show.next;
  const [seasonId, setSeasonId] = useState(next?.season_id ?? show.seasons.find((s) => s.index > 0)?.id ?? show.seasons[0]?.id ?? 0);
  // Bumped after marking something watched, so the episodes load again.
  const [changes, setChanges] = useState(0);
  const allWatched = show.episode_count > 0 && show.watched_count >= show.episode_count;
  const mark = (id: number, watched: boolean) => {
    void (watched ? client.markWatched(id) : client.markUnwatched(id))
      .catch(() => undefined)
      .then(() => {
        setChanges((n) => n + 1);
        reload();
      });
  };
  const seasons = show.seasons.filter((s) => s.index > 0).length;
  const meta = [
    show.year_range ?? null,
    `${seasons} season${seasons === 1 ? '' : 's'}`,
    `${show.watched_count} of ${show.episode_count} watched`,
    show.details.rating != null ? `★ ${show.details.rating.toFixed(1)}` : null,
  ].filter((m): m is string => !!m);
  const season = show.seasons.find((s) => s.id === seasonId);
  const seasonDone = !!season && season.episode_count > 0 && season.watched_count >= season.episode_count;

  return (
    <DetailLayout
      backdrop={show.backdrop}
      poster={show.poster}
      title={show.title}
      year={null}
      meta={meta}
      tagline={show.details.tagline}
      genres={show.details.genres}
      overview={show.overview}
      actions={
        <>
          {next && (
            <Button
              label={next.position_ms != null ? `Resume ${next.label} from ${formatClock(next.position_ms)}` : `Play ${next.label}`}
              icon="play"
              primary
              preferred
              onPress={() => nav.push({ name: 'player', itemId: next.id })}
            />
          )}
          <Button label={allWatched ? 'Mark unwatched' : 'Mark watched'} icon="check" preferred={!next} onPress={() => mark(show.id, !allWatched)} />
          {!next && <Text style={styles.allDone}>You've watched every episode.</Text>}
        </>
      }>
      <FocusGuide autoFocus style={styles.seasons}>
        {show.seasons.length > 1 &&
          show.seasons.map((s) => {
            const done = s.episode_count > 0 && s.watched_count >= s.episode_count;
            return <Chip key={s.id} label={s.title + (done ? '  ✓' : '')} selected={s.id === seasonId} onPress={() => setSeasonId(s.id)} />;
          })}
        {season && (
          <Chip
            label={seasonDone ? `Mark ${season.title} unwatched` : `Mark ${season.title} watched`}
            selected={false}
            onPress={() => mark(season.id, !seasonDone)}
            style={show.seasons.length > 1 ? styles.markSeason : undefined}
          />
        )}
      </FocusGuide>
      {seasonId !== 0 && <Episodes seasonId={seasonId} changes={changes} onToggleWatched={(e) => mark(e.id, !e.watch.watched)} />}
      <CastRow cast={show.details.cast} />
    </DetailLayout>
  );
}

function Episodes({ seasonId, changes, onToggleWatched }: { seasonId: number; changes: number; onToggleWatched: (episode: Episode) => void }) {
  const { client } = useSignedIn();
  const nav = useNav();
  const [episodes, reload] = useLoad(seasonId, () => client.episodes(seasonId), { refresh: true });
  const [focused, setFocused] = useState<Episode | null>(null);
  useEffect(() => {
    if (changes > 0) reload();
  }, [changes, reload]);
  if (episodes.kind !== 'ready') return null;
  const list = episodes.value;
  return (
    <View>
      {/* Above the row, so the page shows it whenever an episode has focus. */}
      <EpisodeInfo episode={list.find((e) => e.id === focused?.id) ?? null} />
      <FocusGuide autoFocus>
        <FlatList
          horizontal
          data={list}
          keyExtractor={(e) => String(e.id)}
          showsHorizontalScrollIndicator={false}
          contentContainerStyle={styles.episodes}
          initialNumToRender={4}
          maxToRenderPerBatch={4}
          windowSize={5}
          renderItem={({ item }) => (
            <EpisodeCard episode={item} onPress={() => nav.push({ name: 'player', itemId: item.id })} onLongPress={() => onToggleWatched(item)} onFocus={() => setFocused(item)} />
          )}
        />
      </FocusGuide>
    </View>
  );
}

/** The focused episode: its overview, and how to mark it. Fixed height, so the page doesn't jump. */
function EpisodeInfo({ episode }: { episode: Episode | null }) {
  const { client } = useSignedIn();
  const [overview, setOverview] = useState<{ id: number; text: string | null } | null>(null);
  useEffect(() => {
    if (!episode) return;
    let current = true;
    const timer = setTimeout(() => {
      client.item(episode.id).then(
        (detail) => current && setOverview({ id: episode.id, text: detail.kind === 'episode' ? (detail.overview ?? null) : null }),
        () => undefined,
      );
    }, 350);
    return () => {
      current = false;
      clearTimeout(timer);
    };
  }, [client, episode]);
  return (
    <View style={styles.info}>
      {!episode ? (
        <Text style={styles.infoHeading}>Episodes</Text>
      ) : (
        <>
          <Text style={styles.infoTitle} numberOfLines={1}>
            {episode.label} · {episode.title}
          </Text>
          <Text style={styles.infoOverview} numberOfLines={3}>
            {overview?.id === episode.id ? (overview.text ?? '') : ''}
          </Text>
          <View style={styles.spacer} />
          <Text style={styles.infoHint}>{episode.watch.watched ? 'Watched · hold OK to mark unwatched' : 'Hold OK to mark watched'}</Text>
        </>
      )}
    </View>
  );
}

function EpisodeCard({ episode, onPress, onLongPress, onFocus }: { episode: Episode; onPress: () => void; onLongPress: () => void; onFocus: () => void }) {
  const position = episode.watch.position_ms;
  const runtime = episode.runtime_ms;
  return (
    <View style={styles.card}>
      <Focusable onPress={onPress} onLongPress={onLongPress} onFocus={onFocus} scale={1.06} style={styles.thumb}>
        {(focused) => (
          <>
            <Artwork image={episode.thumb} style={StyleSheet.absoluteFill} />
            {episode.watch.watched && position == null && <WatchedMark size={24} style={styles.watched} />}
            {position != null && runtime ? <ProgressBar fraction={position / runtime} style={styles.progress} /> : null}
            {focused && <View style={styles.ring} />}
          </>
        )}
      </Focusable>
      <Text style={styles.cardLabel}>{episode.label}</Text>
      <Text style={styles.cardTitle} numberOfLines={1}>
        {episode.title}
      </Text>
      {runtime ? <Text style={styles.cardRuntime}>{formatRuntime(runtime)}</Text> : null}
    </View>
  );
}

const styles = StyleSheet.create({
  allDone: { ...type.titleSmall, color: colors.secondary },
  seasons: { flexDirection: 'row', flexWrap: 'wrap', gap: 10, paddingHorizontal: DETAIL_INSET },
  markSeason: { marginLeft: 14 },
  info: { paddingHorizontal: DETAIL_INSET, height: 112, marginBottom: 10 },
  infoHeading: { ...type.titleLarge, color: colors.text },
  infoTitle: { ...type.titleMedium, color: colors.text },
  infoOverview: { ...type.bodyMedium, color: colors.secondary, marginTop: 4 },
  infoHint: { ...type.labelMedium, color: colors.muted },
  spacer: { flex: 1 },
  episodes: { paddingHorizontal: DETAIL_INSET, paddingVertical: 10, gap: 22 },
  card: { width: 300 },
  thumb: { width: 300, aspectRatio: 16 / 9, borderRadius: radius.md, overflow: 'hidden', backgroundColor: colors.surface },
  ring: { ...StyleSheet.absoluteFillObject, borderRadius: radius.md, borderWidth: 3, borderColor: colors.accent },
  watched: { position: 'absolute', top: 8, right: 8 },
  progress: { position: 'absolute', left: 0, right: 0, bottom: 0 },
  cardLabel: { ...type.labelMedium, color: colors.muted, marginTop: 10 },
  cardTitle: { ...type.titleSmall, color: colors.text },
  cardRuntime: { ...type.bodySmall, color: colors.muted },
});
