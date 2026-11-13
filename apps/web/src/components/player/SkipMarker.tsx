import { useEffect, useState } from "react";
import type { MediaMarker } from "@nuvora/core";
import { SkipNextIcon } from "../Icon";

const LABEL = { intro: "Skip Intro", recap: "Skip Recap" } as const;

/**
 * "Skip Intro" or "Skip Recap" while the playhead is inside one the file
 * marks; it jumps to the marker's end. It shows whether or not the controls
 * do, rises above them when they show, and S presses it.
 */
export function SkipMarker({ video, markers, controlsShown }: { video: HTMLVideoElement | null; markers: readonly MediaMarker[]; controlsShown: boolean }) {
  const [current, setCurrent] = useState<MediaMarker | null>(null);
  useEffect(() => {
    if (!video) return;
    const skippable = markers.filter((m): m is MediaMarker & { kind: "intro" | "recap" } => m.kind !== "credits");
    if (!skippable.length) return;
    // Hidden in the last second, so a click never lands on a marker that just ended.
    const update = () => {
      const ms = video.currentTime * 1000;
      setCurrent(skippable.find((m) => ms >= m.start_ms && ms < m.end_ms - 1000) ?? null);
    };
    update();
    video.addEventListener("timeupdate", update);
    video.addEventListener("seeked", update);
    return () => {
      video.removeEventListener("timeupdate", update);
      video.removeEventListener("seeked", update);
    };
  }, [video, markers]);

  const skip = () => {
    if (video && current) video.currentTime = current.end_ms / 1000;
  };
  useEffect(() => {
    if (!current) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key.toLowerCase() !== "s" || e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.target instanceof HTMLElement && /^(INPUT|TEXTAREA|SELECT)$/.test(e.target.tagName)) return;
      e.preventDefault();
      skip();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  if (!current || current.kind === "credits") return null;
  return (
    <button type="button" className="skip-marker glass-thick" data-raised={controlsShown} onClick={skip} onMouseDown={(e) => e.preventDefault()}>
      {LABEL[current.kind]}
      <SkipNextIcon size={18} />
      <kbd aria-hidden>S</kbd>
    </button>
  );
}
