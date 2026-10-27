import React, { useState } from 'react';
import { BackHandler, View } from 'react-native';
import { formatClock, formatRuntime, type ItemDetail, type MovieDetail } from '@nuvora/core';
import { useLoad } from '../load';
import { useNav } from '../nav';
import { useSignedIn } from '../state';
import { Button, ErrorState, Loading } from '../ui';
import { CastRow, DetailLayout, NotPlayable, notPlayableReasons, playable, VersionFacts, VersionPicker } from './Detail';

export function Movie({ id }: { id: number }) {
  const { client } = useSignedIn();
  const nav = useNav();
  const [item, reload] = useLoad(id, () => client.item(id), { refresh: true });
  if (item.kind === 'loading') return <Loading />;
  if (item.kind === 'failed') return <ErrorState title="Can't open this movie" detail={item.error} action="Back" onAction={nav.pop} />;
  if (item.value.kind !== 'movie') return <ErrorState title="This isn't a movie" detail="It may have been changed on the server." action="Back" onAction={nav.pop} />;
  return <MoviePage movie={item.value} reload={reload} />;
}

function MoviePage({ movie, reload }: { movie: Extract<ItemDetail, { kind: 'movie' }> & MovieDetail; reload: () => void }) {
  const { client } = useSignedIn();
  const nav = useNav();
  // The version being resumed, else the first this TV can play.
  const preferred =
    movie.versions.find((v) => v.file_id === movie.watch.file_id && playable(v.playback)) ?? movie.versions.find((v) => playable(v.playback)) ?? movie.versions[0];
  const [chosenId, setChosenId] = useState(preferred?.file_id ?? 0);
  const version = movie.versions.find((v) => v.file_id === chosenId) ?? preferred;
  const [picking, setPicking] = useState(false);
  const resumeAt = version && movie.watch.file_id === version.file_id && playable(version.playback) ? (movie.watch.position_ms ?? null) : null;

  // Back closes the picker first.
  React.useEffect(() => {
    if (!picking) return;
    const sub = BackHandler.addEventListener('hardwareBackPress', () => {
      setPicking(false);
      return true;
    });
    return () => sub.remove();
  }, [picking]);

  const meta = [
    movie.year != null ? String(movie.year) : null,
    movie.runtime_ms ? formatRuntime(movie.runtime_ms) : null,
    movie.details.rating != null ? `★ ${movie.details.rating.toFixed(1)}` : null,
    version?.summary || null,
  ].filter((m): m is string => !!m);
  const play = (startMs?: number) => version && nav.push({ name: 'player', itemId: movie.id, fileId: version.file_id, ...(startMs !== undefined && { startMs }) });
  const toggleWatched = () => {
    void (movie.watch.watched ? client.markUnwatched(movie.id) : client.markWatched(movie.id)).catch(() => undefined).then(reload);
  };

  return (
    <View style={{ flex: 1 }}>
      <DetailLayout
        backdrop={movie.backdrop}
        poster={movie.poster}
        title={movie.title}
        year={movie.year}
        meta={meta}
        tagline={movie.details.tagline}
        genres={movie.details.genres}
        overview={movie.overview}
        actions={
          <>
            {!version || !playable(version.playback) ? (
              <NotPlayable reasons={notPlayableReasons(version?.playback)} />
            ) : resumeAt != null ? (
              <>
                <Button label={`Resume from ${formatClock(resumeAt)}`} icon="play" primary preferred onPress={() => play(resumeAt)} />
                <Button label="From the start" icon="restart" onPress={() => play(0)} />
              </>
            ) : (
              <Button label="Play" icon="play" primary preferred onPress={() => play(0)} />
            )}
            <Button label={movie.watch.watched ? 'Mark unwatched' : 'Mark watched'} icon="check" onPress={toggleWatched} />
            {movie.versions.length > 1 && <Button label={`${movie.versions.length} versions`} icon="layers" onPress={() => setPicking(true)} />}
          </>
        }>
        {version && <VersionFacts version={version} />}
        <CastRow cast={movie.details.cast} />
      </DetailLayout>
      {picking && (
        <VersionPicker
          versions={movie.versions}
          chosen={version?.file_id ?? 0}
          onPick={(v) => {
            setChosenId(v.file_id);
            setPicking(false);
          }}
        />
      )}
    </View>
  );
}
