import React, { useEffect, useRef, useState } from 'react';
import { StyleSheet, Text, View } from 'react-native';
import { useTVEventHandler } from '@amazon-devices/react-native-kepler';
import { KeplerVideoSurfaceView, VideoPlayer } from '@amazon-devices/react-native-w3cmedia';
import { createProgressReporter, describeError, episodeHeading, formatClock } from '@nuvora/core';
import { useNav } from '../nav';
import { streamHls, type Stream } from '../player/shaka';
import { useSignedIn } from '../state';
import { colors, gutter, radius, type } from '../theme';
import { Loading, Problem } from '../ui';

const SKIP_MS = 10_000;
const HIDE_AFTER_MS = 4_000;
const KEY_DOWN = 0;

/** What the player needs about the movie or episode. */
interface Playable {
  fileId: number;
  url: string;
  title: string;
  subtitle: string | null;
  startMs: number;
  /** The stream carries the file's own picture, which this TV said it decodes. */
  copiesVideo: boolean;
}

/** `startMs`: where to start; left out, where the person stopped. */
export function Player({ itemId, startMs }: { itemId: number; startMs?: number }) {
  const { client, session, onlyH264 } = useSignedIn();
  const nav = useNav();
  const [playable, setPlayable] = useState<Playable | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Runs again with a new client when the TV turns out not to decode a codec it claimed.
  useEffect(() => {
    setPlayable(null);
    client.item(itemId).then(
      (item) => {
        if (item.kind !== 'movie' && item.kind !== 'episode') return setError("That isn't something to play.");
        const version = item.versions[0];
        if (!version) return setError("This has no file on the server any more.");
        const playback = version.playback;
        if (playback.kind !== 'hls') {
          return setError(playback.kind === 'unavailable' ? playback.reason : "The server can't convert this video for this TV.");
        }
        const [title, ...rest] = item.kind === 'episode' ? episodeHeading(item) : [item.title, item.year ? String(item.year) : ''];
        setPlayable({
          fileId: version.file_id,
          url: session.server.url + playback.url,
          title: title!,
          subtitle: rest.filter(Boolean).join(' · ') || null,
          startMs: startMs ?? (item.watch.file_id === version.file_id ? item.watch.position_ms ?? 0 : 0),
          copiesVideo: !playback.converts_video,
        });
      },
      (e: unknown) => setError(describeError(e)),
    );
  }, [client, itemId, session.server.url, startMs]);

  if (error) return <Problem message={error} />;
  if (!playable) return <Loading />;
  return <Video key={playable.url} itemId={itemId} playable={playable} onEnded={() => nav.pop()} onError={setError} onUndecodable={onlyH264} />;
}

/** What the player says went wrong, and the server's plan for the stream: enough to tell a codec problem from a network one. */
function describeMediaError(video: VideoPlayer, url: string): string {
  const plan = url.match(/\/hls\/([^/]+)\//)?.[1] ?? 'direct';
  const error = mediaError(video);
  return [plan, error?.code != null ? `error ${error.code}` : null, error?.message || null].filter(Boolean).join(', ');
}

function mediaError(video: VideoPlayer) {
  return (video as unknown as { error?: { code?: number; message?: string } | null }).error;
}

/** MEDIA_ERR_DECODE and MEDIA_ERR_SRC_NOT_SUPPORTED: the TV couldn't decode the stream. */
const UNDECODABLE = [3, 4];

interface VideoState {
  positionMs: number;
  durationMs: number;
  playing: boolean;
  buffering: boolean;
}

interface VideoProps {
  itemId: number;
  playable: Playable;
  onEnded: () => void;
  onError: (message: string) => void;
  /** The TV couldn't decode the file's own picture; true when a converted stream is on its way. */
  onUndecodable: () => boolean;
}

function Video({ itemId, playable, onEnded, onError, onUndecodable }: VideoProps) {
  const { client, session } = useSignedIn();
  const video = useRef<VideoPlayer | null>(null);
  // The surface and the stream come up in either order; whichever is last starts playback.
  const surface = useRef<string | null>(null);
  const loaded = useRef(false);
  const [state, setState] = useState<VideoState>({ positionMs: playable.startMs, durationMs: 0, playing: false, buffering: true });
  const [controls, setControls] = useState(true);
  const [touched, setTouched] = useState(0);
  // The latest client and callbacks, without restarting the video when they change.
  const api = useRef(client);
  const ended = useRef(onEnded);
  const failed = useRef(onError);
  const undecodable = useRef(onUndecodable);
  api.current = client;
  ended.current = onEnded;
  failed.current = onError;
  undecodable.current = onUndecodable;

  useEffect(() => {
    const v = new VideoPlayer();
    video.current = v;
    (globalThis as { gmedia?: VideoPlayer | null }).gmedia = v;
    let stream: Stream | null = null;
    let closed = false;
    const reporter = createProgressReporter({
      fileId: playable.fileId,
      read: () => (v.duration > 0 ? { positionMs: Math.round(v.currentTime * 1000), durationMs: Math.round(v.duration * 1000) } : null),
      send: (report) => api.current.reportProgress(itemId, report),
      timers: { setTimeout, clearTimeout },
    });
    const update = () =>
      setState((s) => ({ ...s, positionMs: v.currentTime * 1000, durationMs: v.duration > 0 ? v.duration * 1000 : s.durationMs }));
    const listeners: [string, () => void][] = [
      ['timeupdate', update],
      ['playing', () => (reporter.playing(), setState((s) => ({ ...s, playing: true, buffering: false })))],
      ['pause', () => (reporter.paused(), setState((s) => ({ ...s, playing: false })))],
      ['waiting', () => setState((s) => ({ ...s, buffering: true }))],
      ['seeked', () => (reporter.seeked(), update())],
      ['ended', () => ended.current()],
      [
        'error',
        () => {
          const code = mediaError(v)?.code;
          if (playable.copiesVideo && code != null && UNDECODABLE.includes(code) && undecodable.current()) return;
          failed.current(`The video stopped playing (${describeMediaError(v, playable.url)}).`);
        },
      ],
    ];

    (async () => {
      await v.initialize();
      if (closed) return;
      v.autoplay = false;
      for (const [event, listener] of listeners) v.addEventListener(event, listener);
      stream = await streamHls(v, playable.url, session.token, playable.startMs / 1000);
      if (closed) return;
      loaded.current = true;
      if (surface.current) {
        v.setSurfaceHandle(surface.current);
        void v.play();
      }
    })().catch(() => {
      if (!closed) failed.current("Couldn't start the video. Check that the server is still running.");
    });

    return () => {
      closed = true;
      void reporter.leave();
      for (const [event, listener] of listeners) v.removeEventListener(event, listener);
      v.pause();
      void (async () => {
        await stream?.stop();
        await v.deinitialize().catch(() => undefined);
      })();
      (globalThis as { gmedia?: VideoPlayer | null }).gmedia = null;
    };
  }, [itemId, playable, session.token]);

  // Controls fade while playing; any key brings them back.
  useEffect(() => {
    if (!state.playing) return;
    const timer = setTimeout(() => setControls(false), HIDE_AFTER_MS);
    return () => clearTimeout(timer);
  }, [touched, state.playing]);

  const seekBy = (deltaMs: number) => {
    const v = video.current;
    if (!v || !(v.duration > 0)) return;
    v.currentTime = Math.min(Math.max(0, v.currentTime + deltaMs / 1000), v.duration);
  };

  useTVEventHandler((event) => {
    if (event.eventKeyAction !== KEY_DOWN) return;
    const v = video.current;
    setControls(true);
    setTouched((n) => n + 1);
    // OK is `select` on a remote, `enter` or `kpenter` from a keyboard. Play/pause,
    // rewind and fast-forward go to Vega's media controls, which drive the player.
    switch (event.eventType) {
      case 'select':
      case 'enter':
      case 'kpenter':
        if (v) v.paused ? void v.play() : v.pause();
        break;
      case 'left':
        seekBy(-SKIP_MS);
        break;
      case 'right':
        seekBy(SKIP_MS);
        break;
    }
  });

  const fraction = state.durationMs > 0 ? Math.min(1, state.positionMs / state.durationMs) : 0;
  return (
    <View style={styles.screen}>
      <KeplerVideoSurfaceView
        style={StyleSheet.absoluteFill}
        onSurfaceViewCreated={(handle) => {
          surface.current = handle;
          if (loaded.current && video.current) {
            video.current.setSurfaceHandle(handle);
            void video.current.play();
          }
        }}
        onSurfaceViewDestroyed={(handle) => {
          surface.current = null;
          video.current?.clearSurfaceHandle(handle);
        }}
      />
      {state.buffering && (
        <View style={styles.center} pointerEvents="none">
          <Text style={styles.loading}>Loading…</Text>
        </View>
      )}
      {controls && (
        <View style={StyleSheet.absoluteFill} pointerEvents="none">
          <View style={styles.topScrim}>
            <Text style={styles.title} numberOfLines={1}>
              {playable.title}
            </Text>
            {playable.subtitle && <Text style={styles.subtitle}>{playable.subtitle}</Text>}
          </View>
          <View style={styles.bottomScrim}>
            {!state.playing && !state.buffering && <Text style={styles.paused}>Paused · press OK to play</Text>}
            <View style={styles.timeline}>
              <Text style={styles.time}>{formatClock(state.positionMs)}</Text>
              <View style={styles.track}>
                <View style={[styles.played, { width: `${fraction * 100}%` }]} />
              </View>
              <Text style={[styles.time, styles.remaining]}>−{formatClock(Math.max(0, state.durationMs - state.positionMs))}</Text>
            </View>
          </View>
        </View>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, backgroundColor: '#000' },
  center: { ...StyleSheet.absoluteFillObject, alignItems: 'center', justifyContent: 'center' },
  loading: { ...type.title, color: colors.secondary },
  topScrim: { paddingHorizontal: gutter, paddingTop: 28, paddingBottom: 30, backgroundColor: 'rgba(0,0,0,0.55)' },
  title: { ...type.heading, color: colors.text },
  subtitle: { ...type.bodyLarge, color: colors.secondary, marginTop: 2 },
  bottomScrim: { position: 'absolute', left: 0, right: 0, bottom: 0, paddingHorizontal: gutter, paddingTop: 22, paddingBottom: 30, backgroundColor: 'rgba(0,0,0,0.6)' },
  paused: { ...type.body, color: colors.text, marginBottom: 10 },
  timeline: { flexDirection: 'row', alignItems: 'center' },
  time: { ...type.small, color: colors.text, minWidth: 60 },
  remaining: { textAlign: 'right', color: colors.secondary },
  track: { flex: 1, height: 4, borderRadius: radius.pill, marginHorizontal: 12, backgroundColor: 'rgba(255,255,255,0.25)', overflow: 'hidden' },
  played: { height: '100%', backgroundColor: colors.accent },
});
