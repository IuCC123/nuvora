import { useEffect, useState } from "react";
import { activeCues, type Cue } from "@nuvora/core";
import type { SubtitleStyle } from "../../lib/subtitleMemory";

interface PlainSubtitlesProps {
  video: HTMLVideoElement;
  cues: readonly Cue[];
  style: SubtitleStyle;
}

/**
 * Nuvora's own cue overlay, so subtitles look the same in every browser. It
 * reads the video's clock every frame while playing and on every seek, so a
 * cue lands on its frame at any playback rate.
 */
export function PlainSubtitles({ video, cues, style }: PlainSubtitlesProps) {
  const showing = useShowingCues(video, cues);
  const top = showing.filter((c) => c.placement === "top");
  const bottom = showing.filter((c) => c.placement === "bottom");
  return (
    <div className="subs-plain" data-size={style.size} data-background={style.background} aria-hidden>
      {[top, bottom].map((group, i) => (
        <div key={i} className="subs-plain-group" data-placement={i === 0 ? "top" : "bottom"}>
          {group.map((cue) => (
            <p key={`${cue.start}-${cue.end}-${cues.indexOf(cue)}`} className="subs-cue">
              {cue.lines.map((line, n) => (
                <span key={n} className="subs-line">
                  {line.map((run, r) => (
                    <span key={r} data-i={run.italic || undefined} data-b={run.bold || undefined} data-u={run.underline || undefined}>
                      {run.text}
                    </span>
                  ))}
                </span>
              ))}
            </p>
          ))}
        </div>
      ))}
    </div>
  );
}

function useShowingCues(video: HTMLVideoElement, cues: readonly Cue[]): Cue[] {
  const [showing, setShowing] = useState<Cue[]>(() => activeCues(cues, video.currentTime));
  useEffect(() => {
    let frame = 0;
    const update = () => {
      const next = activeCues(cues, video.currentTime);
      setShowing((prev) => (prev.length === next.length && prev.every((c, i) => c === next[i]) ? prev : next));
    };
    const loop = () => {
      update();
      frame = requestAnimationFrame(loop);
    };
    const start = () => {
      cancelAnimationFrame(frame);
      if (!video.paused) loop();
      else update();
    };
    start();
    const events = ["play", "pause", "seeking", "seeked", "ratechange", "emptied"] as const;
    for (const e of events) video.addEventListener(e, start);
    return () => {
      cancelAnimationFrame(frame);
      for (const e of events) video.removeEventListener(e, start);
    };
  }, [video, cues]);
  return showing;
}
