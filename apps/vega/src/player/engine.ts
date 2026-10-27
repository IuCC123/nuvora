import { useEffect, useRef, useState } from 'react';
import { VideoPlayer } from '@amazon-devices/react-native-w3cmedia';
import { streamHls, type Stream } from './shaka';

export interface VideoState {
  positionMs: number;
  durationMs: number;
  playing: boolean;
  buffering: boolean;
  ended: boolean;
}

/** MEDIA_ERR_DECODE and MEDIA_ERR_SRC_NOT_SUPPORTED: the TV couldn't decode the stream. */
export const UNDECODABLE = [3, 4];

export function mediaError(video: VideoPlayer) {
  return (video as unknown as { error?: { code?: number; message?: string } | null }).error ?? null;
}

interface Options {
  /** The playlist, with its session and start already in the query. */
  url: string;
  token: string;
  startMs: number;
  /** Starts playing once loaded; in a party the party's clock does that. */
  autoplay: boolean;
  /** A media error (`code` is the W3C code) or a stream that wouldn't load (`code` null). */
  onError(code: number | null, message: string | null): void;
  /** Play or pause happened, by anyone (the app, or Vega's media controls). */
  onPlayState?(playing: boolean): void;
  onSeeked?(): void;
}

export interface Engine {
  state: VideoState;
  video: React.MutableRefObject<VideoPlayer | null>;
  surface: { onSurfaceViewCreated(handle: string): void; onSurfaceViewDestroyed(handle: string): void };
}

/**
 * One video: Vega's W3C media player with Shaka streaming `url` into it, on
 * the surface the screen gives it. The surface and the stream come up in
 * either order; whichever is last starts playback.
 */
export function useVideo(options: Options): Engine {
  const { url, token, startMs } = options;
  const video = useRef<VideoPlayer | null>(null);
  const surface = useRef<string | null>(null);
  const loaded = useRef(false);
  const [state, setState] = useState<VideoState>({ positionMs: startMs, durationMs: 0, playing: false, buffering: true, ended: false });
  // The latest callbacks, without restarting the video when they change.
  const latest = useRef(options);
  latest.current = options;

  useEffect(() => {
    const v = new VideoPlayer();
    video.current = v;
    // Amazon's Shaka build looks for the player here.
    (globalThis as { gmedia?: VideoPlayer | null }).gmedia = v;
    let stream: Stream | null = null;
    let closed = false;
    const update = () =>
      setState((s) => ({ ...s, positionMs: v.currentTime * 1000, durationMs: v.duration > 0 && Number.isFinite(v.duration) ? v.duration * 1000 : s.durationMs }));
    const listeners: [string, () => void][] = [
      ['timeupdate', update],
      ['durationchange', update],
      ['playing', () => setState((s) => ({ ...s, playing: true, buffering: false, ended: false }))],
      ['play', () => latest.current.onPlayState?.(true)],
      ['pause', () => (setState((s) => ({ ...s, playing: false })), latest.current.onPlayState?.(false))],
      ['waiting', () => setState((s) => ({ ...s, buffering: true }))],
      ['canplay', () => setState((s) => ({ ...s, buffering: false }))],
      ['seeked', () => (update(), latest.current.onSeeked?.())],
      ['ended', () => setState((s) => ({ ...s, playing: false, ended: true }))],
      [
        'error',
        () => {
          const error = mediaError(v);
          latest.current.onError(error?.code ?? null, error?.message ?? null);
        },
      ],
    ];

    const start = () => {
      if (!surface.current) return;
      v.setSurfaceHandle(surface.current);
      if (latest.current.autoplay) void v.play();
    };

    (async () => {
      await v.initialize();
      if (closed) return;
      v.autoplay = false;
      for (const [event, listener] of listeners) v.addEventListener(event, listener);
      stream = await streamHls(v, url, token, startMs / 1000);
      if (closed) return;
      loaded.current = true;
      start();
    })().catch(() => {
      if (!closed) latest.current.onError(null, null);
    });

    return () => {
      closed = true;
      loaded.current = false;
      for (const [event, listener] of listeners) v.removeEventListener(event, listener);
      v.pause();
      void (async () => {
        await stream?.stop();
        await v.deinitialize().catch(() => undefined);
      })();
      if (video.current === v) video.current = null;
      (globalThis as { gmedia?: VideoPlayer | null }).gmedia = null;
    };
    // One player per stream: a new url (another audio track) is a new player.
  }, [url, token, startMs]);

  return {
    state,
    video,
    surface: {
      onSurfaceViewCreated: (handle) => {
        surface.current = handle;
        const v = video.current;
        if (loaded.current && v) {
          v.setSurfaceHandle(handle);
          if (latest.current.autoplay) void v.play();
        }
      },
      onSurfaceViewDestroyed: (handle) => {
        surface.current = null;
        video.current?.clearSurfaceHandle(handle);
      },
    },
  };
}
