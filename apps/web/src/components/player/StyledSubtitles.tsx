import { useEffect, useRef } from "react";
import type { SubtitleTrack } from "@nuvora/core";

interface StyledSubtitlesProps {
  video: HTMLVideoElement;
  track: SubtitleTrack;
  content: string;
  fonts: string[];
  onReady: (track: SubtitleTrack) => void;
  onFailed: (track: SubtitleTrack) => void;
}

/** A renderer that hasn't drawn by now is treated as failed, and the track falls back to plain text. */
const READY_TIMEOUT_MS = 15_000;

/**
 * An ASS track drawn by libass on a canvas over the picture, signs and
 * karaoke intact. The canvas is made here rather than by React because the
 * renderer moves it to its worker (`transferControlToOffscreen`), which a
 * canvas allows only once.
 */
export function StyledSubtitles({ video, track, content, fonts, onReady, onFailed }: StyledSubtitlesProps) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const canvas = document.createElement("canvas");
    host.current?.append(canvas);
    let destroy = () => {};
    let settled = false;
    const settle = (ok: boolean) => {
      if (settled) return;
      settled = true;
      if (ok) onReady(track);
      else onFailed(track);
    };
    const timer = window.setTimeout(() => settle(false), READY_TIMEOUT_MS);
    import("../../lib/styledRenderer")
      .then(({ createStyledRenderer }) => {
        if (settled) return;
        const renderer = createStyledRenderer(video, canvas, content, fonts);
        destroy = renderer.destroy;
        return renderer.ready.then(() => settle(true));
      })
      .catch((error: unknown) => {
        console.warn("Styled subtitles failed; showing the track as plain text.", error);
        settle(false);
      });
    return () => {
      settled = true;
      window.clearTimeout(timer);
      destroy();
      canvas.remove();
    };
  }, [video, track, content, fonts, onReady, onFailed]);
  return <div ref={host} className="subs-styled" aria-hidden />;
}
