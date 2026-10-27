import React, { useEffect, useMemo, useRef, useState } from 'react';
import { BackHandler, StyleSheet, Text, View } from 'react-native';
import { useTVEventHandler } from '@amazon-devices/react-native-kepler';
import { KeplerVideoSurfaceView } from '@amazon-devices/react-native-w3cmedia';
import { createProgressReporter, episodeHeading, upNextCountdown, type AudioTrack, type EpisodeRef, type ItemDetail, type PreviewInfo, type Version } from '@nuvora/core';
import { useLoad } from '../load';
import { useNav } from '../nav';
import { UNDECODABLE, useVideo } from '../player/engine';
import { AudioMenu, Controls, Countdown, Note, ScrubBar, scrubStep, SubtitleMenu, UpNext } from '../player/Overlays';
import { usePartyPlayer } from '../player/party';
import { SubtitleText } from '../player/SubtitleText';
import { useSubtitles } from '../player/subtitles';
import { useSignedIn } from '../state';
import { saveSubtitleStyle, subtitleStyle, type SubtitleStyle } from '../storage';
import { colors, type } from '../theme';
import { ErrorState } from '../ui';
import { playable as canPlay } from './Detail';

const SKIP_MS = 10_000;
const HIDE_AFTER_MS = 5_000;
/** A scrub lands this long after the last press. */
const SCRUB_SETTLE_MS = 800;
const ASK_PREVIEWS_AGAIN_MS = 15_000;
const KEY_DOWN = 0;

/** What the player plays: one movie or episode, one version of it. */
interface Playable {
  itemId: number;
  title: string;
  subtitle: string | null;
  version: Version;
  /** The server's playlist for this TV. */
  url: string;
  /** Where the viewer stopped in this version, if anywhere. */
  resumeMs: number | null;
  next: EpisodeRef | null;
  /** Where a subtitle choice carries over: `"show:12"` or `"movie:4"`. */
  scope: string;
}

function resolve(item: ItemDetail, fileId: number | undefined): Playable | null {
  if (item.kind !== 'movie' && item.kind !== 'episode') return null;
  const ok = (v: Version) => canPlay(v.playback) && v.playback.kind === 'hls';
  const version = item.versions.find((v) => v.file_id === fileId && ok(v)) ?? item.versions.find((v) => v.file_id === item.watch.file_id && ok(v)) ?? item.versions.find(ok);
  if (!version || version.playback.kind !== 'hls') return null;
  const [title, ...rest] = item.kind === 'episode' ? episodeHeading(item) : [item.title];
  return {
    itemId: item.id,
    title: title ?? '',
    subtitle: rest.filter(Boolean).join(' · ') || null,
    version,
    url: version.playback.url,
    resumeMs: item.watch.file_id === version.file_id ? (item.watch.position_ms ?? null) : null,
    next: item.kind === 'episode' ? (item.next ?? null) : null,
    scope: item.kind === 'episode' ? `show:${item.show_id}` : `movie:${item.id}`,
  };
}

/** `fileId`: the version (left out, the one being resumed, else the best); `startMs`: where (left out, where the person stopped). */
export function Player({ itemId, fileId, startMs }: { itemId: number; fileId?: number; startMs?: number }) {
  const { client } = useSignedIn();
  const nav = useNav();
  // A new client (the TV turned out not to decode a codec it claimed) asks again.
  const key = useMemo(() => ({ client, itemId }), [client, itemId]);
  const [item] = useLoad(key, () => client.item(itemId));
  if (item.kind === 'loading') return <Black />;
  if (item.kind === 'failed') return <ErrorState title="Can't play this" detail={item.error} action="Back" onAction={nav.pop} />;
  const playable = resolve(item.value, fileId);
  if (!playable) return <ErrorState title="Nothing to play" detail="This title has no file this TV can play." action="Back" onAction={nav.pop} />;
  return <PlayerView key={playable.url} playable={playable} startMs={startMs ?? playable.resumeMs ?? 0} />;
}

function Black() {
  return (
    <View style={styles.black}>
      <View style={styles.center}>
        <Text style={styles.loading}>Loading…</Text>
      </View>
    </View>
  );
}

/** 128 random bits as hex: one conversion session on the server per stream. */
function sessionId(): string {
  let id = '';
  for (let i = 0; i < 32; i++) id += '0123456789abcdef'[Math.floor(Math.random() * 16)];
  return id;
}

/** The same plan with audio stream `track` converted to AAC (the server converts any audio it's asked for). */
function withAudio(url: string, track: AudioTrack, aacChannels: number): string {
  const plan = url.match(/\/hls\/([^/]+)\//)?.[1];
  if (!plan) return url;
  const video = plan.split('.')[0];
  const channels = Math.min(Math.max(track.channels, 1), Math.min(Math.max(aacChannels, 1), 6));
  return url.replace(`/hls/${plan}/`, `/hls/${video}.a${track.index}-aac${channels}/`);
}

type Menu = 'none' | 'subtitles' | 'audio';

function PlayerView({ playable, startMs }: { playable: Playable; startMs: number }) {
  const { client, session, live, device, onlyH264 } = useSignedIn();
  const nav = useNav();
  const version = playable.version;
  const media = version.media;
  const audioTracks = media?.audio ?? [];
  const planAudio = Number(playable.url.match(/\/hls\/[^/.]+\.a(\d+)/)?.[1] ?? NaN);
  // Another audio track: the stream starts again with it, where it was.
  const [audio, setAudio] = useState<{ track: AudioTrack; atMs: number } | null>(null);
  const streamStart = audio?.atMs ?? startMs;
  const planUrl = audio ? withAudio(playable.url, audio.track, device.audio.find((a) => a.codec === 'aac')?.max_channels ?? 2) : playable.url;
  const url = useMemo(
    () => `${session.server.url}${planUrl}?session=${sessionId()}${streamStart > 0 ? `&start=${Math.floor(streamStart / 1000)}` : ''}`,
    [session.server.url, planUrl, streamStart],
  );
  const [error, setError] = useState<string | null>(null);
  const copiesVideo = version.playback.kind === 'hls' && !version.playback.converts_video;
  const [inParty] = useState(() => live.getSnapshot().connected && live.getSnapshot().party !== null);

  const engine = useVideo({
    url,
    token: session.token,
    startMs: streamStart,
    // In a party, the party's clock starts the video, everyone at once.
    autoplay: !inParty,
    onError: (code, message) => {
      // The TV couldn't decode the file's own picture: ask for H.264 instead (Player loads again).
      if (copiesVideo && code != null && UNDECODABLE.includes(code) && onlyH264()) return;
      setError(
        code === null
          ? "Couldn't start the video. Check that the server is still running."
          : code === 2
            ? 'Lost the connection to the server.'
            : UNDECODABLE.includes(code)
              ? "This TV couldn't decode this file."
              : `Playback stopped (error ${code}${message ? `: ${message}` : ''}).`,
      );
    },
    onPlayState: (playing) => party.heard(playing, (engine.video.current?.currentTime ?? 0) * 1000),
    onSeeked: () => reporter.current?.seeked(),
  });
  const { state, video } = engine;
  const party = usePartyPlayer(live, video, state, session.user.id, playable.itemId, version.file_id, startMs);

  // Progress for the server, as the web and Fire TV apps report it.
  const reporter = useRef<ReturnType<typeof createProgressReporter> | null>(null);
  useEffect(() => {
    const r = createProgressReporter({
      fileId: version.file_id,
      read: () => {
        const v = video.current;
        return v && v.duration > 0 && Number.isFinite(v.duration) ? { positionMs: Math.round(v.currentTime * 1000), durationMs: Math.round(v.duration * 1000) } : null;
      },
      send: (report) => client.reportProgress(playable.itemId, report),
      timers: { setTimeout, clearTimeout },
    });
    reporter.current = r;
    return () => void r.leave();
  }, [client, playable.itemId, version.file_id, video]);
  useEffect(() => {
    if (state.playing) reporter.current?.playing();
    else reporter.current?.paused();
  }, [state.playing]);

  const subtitles = useSubtitles(version.file_id, media?.subtitles ?? [], audioTracks, playable.scope);
  const [look, setLook] = useState<SubtitleStyle>(subtitleStyle);

  const [controls, setControls] = useState(true);
  const [touched, setTouched] = useState(0);
  const [menu, setMenu] = useState<Menu>('none');
  const show = () => {
    setControls(true);
    setTouched((n) => n + 1);
  };
  // Controls fade while playing; they stay while paused, buffering or a menu is open.
  useEffect(() => {
    if (!state.playing || menu !== 'none') return;
    const timer = setTimeout(() => setControls(false), HIDE_AFTER_MS);
    return () => clearTimeout(timer);
  }, [touched, state.playing, menu]);

  // Seek previews, once the server has made them (asking hurries them along).
  const [previews, setPreviews] = useState<PreviewInfo | null>(null);
  useEffect(() => {
    let alive = true;
    let timer: ReturnType<typeof setTimeout>;
    const ask = () =>
      client.previews(version.file_id).then(
        (info) => {
          if (!alive) return;
          if (info) setPreviews(info);
          else timer = setTimeout(ask, ASK_PREVIEWS_AGAIN_MS);
        },
        () => {
          if (alive) timer = setTimeout(ask, ASK_PREVIEWS_AGAIN_MS);
        },
      );
    void ask();
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [client, version.file_id]);

  const durationMs = state.durationMs || (media?.duration_ms ?? 0);
  const seekTo = (target: number) => {
    const to = Math.min(Math.max(0, target), Math.max(0, durationMs));
    if (party.active) party.seek(to);
    else if (video.current) video.current.currentTime = to / 1000;
    show();
  };
  const seekBy = (delta: number) => seekTo(state.positionMs + delta);
  const toggle = () => {
    const v = video.current;
    if (!v) return;
    const playing = !v.paused && !state.ended;
    if (party.active) {
      if (playing) party.pause(v.currentTime * 1000);
      else party.play(state.ended ? 0 : v.currentTime * 1000);
    } else if (playing) {
      console.warn('NVDBG toggle pause'); v.pause();
    } else {
      if (state.ended) v.currentTime = 0;
      void v.play();
    }
    show();
  };

  // Left and right with the controls hidden move a target; the video jumps there once the presses stop.
  const [scrub, setScrub] = useState<{ targetMs: number; presses: number } | null>(null);
  const commitScrub = () => {
    if (scrub) seekTo(scrub.targetMs);
    setScrub(null);
  };
  useEffect(() => {
    if (!scrub) return;
    const timer = setTimeout(commitScrub, SCRUB_SETTLE_MS);
    return () => clearTimeout(timer);
    // Each press restarts the wait.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scrub?.presses]);
  const scrubBy = (direction: 1 | -1) =>
    setScrub((s) => {
      const presses = (s?.presses ?? 0) + 1;
      const from = s?.targetMs ?? state.positionMs;
      return { targetMs: Math.min(Math.max(0, from + direction * scrubStep(presses)), durationMs), presses };
    });

  const next = playable.next;
  const playNext = () => {
    if (next) nav.replace({ name: 'player', itemId: next.id, startMs: 0 });
  };
  const upNext = next && !error ? upNextCountdown(state.positionMs / 1000, durationMs / 1000) : null;
  const [upNextDismissed, setUpNextDismissed] = useState(false);
  const showUpNext = upNext !== null && !upNextDismissed;
  useEffect(() => {
    if (showUpNext && upNext === 0) playNext();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showUpNext, upNext]);

  useTVEventHandler((event) => {
    console.warn(`NVDBG key ${event.eventType}/${event.eventKeyAction} menu=${menu} controls=${controls}`);
    if (event.eventKeyAction !== KEY_DOWN || error || menu !== 'none' || showUpNext) return;
    switch (event.eventType) {
      // OK is `select` on a remote, `enter` or `kpenter` from a keyboard.
      case 'select':
      case 'enter':
      case 'kpenter':
        if (scrub) commitScrub();
        else if (!controls) toggle();
        else setTouched((n) => n + 1);
        break;
      case 'left':
      case 'right':
        if (!controls) scrubBy(event.eventType === 'right' ? 1 : -1);
        else setTouched((n) => n + 1);
        break;
      case 'up':
      case 'down':
        if (!controls) show();
        else setTouched((n) => n + 1);
        break;
      case 'back':
        break;
      // Play/pause, rewind and fast-forward go to Vega's media controls, which drive the player.
      default:
        show();
    }
  });

  // Back: out of a scrub, a menu, the controls, then the player.
  const back = useRef<() => boolean>(() => false);
  back.current = () => {
    if (scrub) {
      setScrub(null);
      return true;
    }
    if (menu !== 'none') {
      setMenu('none');
      show();
      return true;
    }
    if (controls && state.playing && !error) {
      setControls(false);
      return true;
    }
    return false;
  };
  useEffect(() => {
    const sub = BackHandler.addEventListener('hardwareBackPress', () => back.current());
    return () => sub.remove();
  }, []);

  return (
    <View style={styles.black}>
      <KeplerVideoSurfaceView style={StyleSheet.absoluteFill} onSurfaceViewCreated={engine.surface.onSurfaceViewCreated} onSurfaceViewDestroyed={engine.surface.onSurfaceViewDestroyed} />
      <SubtitleText lines={subtitles.linesAt(state.positionMs)} look={look} height={540} lift={controls && !scrub ? 160 : 0} />
      {state.buffering && !error && party.countdown === null && (
        <View style={styles.center} pointerEvents="none">
          <Text style={styles.loading}>Loading…</Text>
        </View>
      )}
      {party.countdown !== null && !error && <Countdown seconds={party.countdown} />}
      {scrub && !error && <ScrubBar targetMs={scrub.targetMs} durationMs={durationMs} previews={previews} />}
      {controls && !error && !scrub && !showUpNext && (
        <Controls
          title={playable.title}
          subtitle={playable.subtitle}
          playing={state.playing}
          positionMs={state.positionMs}
          durationMs={durationMs}
          together={party.active ? party.others.map((m) => m.display_name) : []}
          hasSubtitles={subtitles.tracks.length > 0}
          hasAudioChoice={audioTracks.length > 1}
          onToggle={toggle}
          onBack={() => seekBy(-SKIP_MS)}
          onForward={() => seekBy(SKIP_MS)}
          onSubtitles={() => setMenu('subtitles')}
          onAudio={() => (console.warn('NVDBG onAudio'), setMenu('audio'))}
          onNext={next ? playNext : null}
          onAnyKey={() => setTouched((n) => n + 1)}
        />
      )}
      {party.note && <Note text={party.note} />}
      {menu === 'subtitles' && (
        <SubtitleMenu
          tracks={subtitles.tracks}
          selected={subtitles.selected}
          look={look}
          onPick={(track) => {
            subtitles.select(track);
            setMenu('none');
            show();
          }}
          onLook={(chosen) => {
            setLook(chosen);
            saveSubtitleStyle(chosen);
          }}
        />
      )}
      {menu === 'audio' && (
        <AudioMenu
          tracks={audioTracks}
          selected={audio?.track.index ?? (Number.isNaN(planAudio) ? null : planAudio)}
          onPick={(track) => {
            setMenu('none');
            show();
            if (track.index !== (audio?.track.index ?? planAudio)) setAudio({ track, atMs: state.positionMs });
          }}
        />
      )}
      {showUpNext && next && (
        <UpNext
          label={next.label}
          title={next.title}
          seconds={upNext ?? 0}
          onPlay={playNext}
          onCancel={() => {
            setUpNextDismissed(true);
            nav.pop();
          }}
        />
      )}
      {error && (
        <View style={styles.error}>
          <ErrorState title="Playback stopped" detail={error} action="Back" onAction={nav.pop} />
        </View>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  black: { flex: 1, backgroundColor: '#000' },
  center: { ...StyleSheet.absoluteFillObject, alignItems: 'center', justifyContent: 'center' },
  loading: { ...type.titleLarge, color: colors.secondary },
  error: { ...StyleSheet.absoluteFillObject, backgroundColor: 'rgba(15,16,18,0.92)' },
});
