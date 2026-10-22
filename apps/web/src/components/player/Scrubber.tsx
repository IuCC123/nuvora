import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent } from "react";
import { formatClock } from "@nuvora/core";

interface ScrubberProps {
  video: HTMLVideoElement | null;
  /** Seconds; `NaN` until metadata loads. */
  duration: number;
  buffered: readonly (readonly [number, number])[];
  onSeek: (seconds: number) => void;
  onDragChange: (dragging: boolean) => void;
}

const PAGE_SECONDS = 60;

/**
 * The timeline. The played bar follows the video every frame (a CSS variable
 * set outside React), so it moves smoothly even on a 20 s clip. Hover shows the
 * time under the pointer; dragging seeks live and lands exactly on release.
 * Left and Right reach the player's own 10 s skip; Home, End and Page keys are handled here.
 */
export function Scrubber({ video, duration, buffered, onSeek, onDragChange }: ScrubberProps) {
  const root = useRef<HTMLDivElement>(null);
  const [hover, setHover] = useState<number | null>(null);
  const [drag, setDrag] = useState<number | null>(null);
  const known = Number.isFinite(duration) && duration > 0;

  useEffect(() => {
    if (drag !== null) {
      root.current?.style.setProperty("--played", String(drag));
      return;
    }
    let frame = 0;
    const follow = () => {
      if (video && known) root.current?.style.setProperty("--played", String(Math.min(1, video.currentTime / duration)));
      frame = requestAnimationFrame(follow);
    };
    follow();
    return () => cancelAnimationFrame(frame);
  }, [video, duration, known, drag]);

  const fractionAt = (clientX: number) => {
    const rect = root.current!.getBoundingClientRect();
    return Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
  };

  function onPointerDown(e: PointerEvent<HTMLDivElement>) {
    if (!known || e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    const f = fractionAt(e.clientX);
    setDrag(f);
    onDragChange(true);
    onSeek(f * duration);
  }

  function onPointerMove(e: PointerEvent<HTMLDivElement>) {
    const f = fractionAt(e.clientX);
    if (e.pointerType === "mouse") setHover(f);
    if (drag === null) return;
    setDrag(f);
    onSeek(f * duration);
  }

  function endDrag(e: PointerEvent<HTMLDivElement>) {
    if (drag === null) return;
    const f = fractionAt(e.clientX);
    onSeek(f * duration);
    setDrag(null);
    onDragChange(false);
    if (e.pointerType !== "mouse") setHover(null);
  }

  function onKeyDown(e: KeyboardEvent<HTMLDivElement>) {
    if (!known || !video) return;
    const to = { Home: 0, End: duration, PageUp: video.currentTime + PAGE_SECONDS, PageDown: video.currentTime - PAGE_SECONDS }[e.key];
    if (to === undefined) return;
    e.preventDefault();
    onSeek(Math.min(duration, Math.max(0, to)));
  }

  const tip = drag ?? hover;
  const now = video?.currentTime ?? 0;
  return (
    <div
      ref={root}
      className="scrubber"
      role="slider"
      tabIndex={0}
      aria-label="Seek"
      aria-valuemin={0}
      aria-valuemax={known ? Math.round(duration) : 0}
      aria-valuenow={Math.round(now)}
      aria-valuetext={known ? `${formatClock(now * 1000)} of ${formatClock(duration * 1000)}` : "Loading"}
      aria-disabled={!known}
      data-dragging={drag !== null}
      onMouseDown={(e) => e.preventDefault()}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      onPointerLeave={() => setHover(null)}
      onKeyDown={onKeyDown}
    >
      <div className="scrubber-track">
        {known &&
          buffered.map(([start, end]) => (
            <span key={start} className="scrubber-buffered" style={{ left: `${(start / duration) * 100}%`, width: `${((end - start) / duration) * 100}%` }} />
          ))}
        {hover !== null && <span className="scrubber-preview" style={{ width: `${hover * 100}%` }} />}
        <span className="scrubber-played" />
      </div>
      <span className="scrubber-thumb" />
      {tip !== null && known && (
        <span className="scrubber-tip" style={{ "--at": tip } as CSSProperties} aria-hidden>
          {formatClock(tip * duration * 1000)}
        </span>
      )}
    </div>
  );
}
