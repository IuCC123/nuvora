import { useEffect, useRef, useState } from "react";
import { hasOwnTitle, UP_NEXT_SECONDS, upNextCountdown, type EpisodeRef } from "@nuvora/core";
import { Artwork } from "../Artwork";
import { PlayIcon } from "../Icon";
import "../../styles/forms.css";
import "../../styles/up-next.css";

const RING = 2 * Math.PI * 22;

/**
 * The Next episode card: it slides in over the last ten seconds of an episode
 * and counts down on playback time, so pausing holds the count and seeking back
 * hides it again. At the end the next episode plays, unless the viewer cancelled.
 * It never takes focus; Tab reaches Play now and Cancel.
 */
export function UpNext({
  video,
  next,
  controlsShown,
  onPlay,
}: {
  video: HTMLVideoElement | null;
  next: EpisodeRef;
  /** The card rises above the control bar while it shows. */
  controlsShown: boolean;
  onPlay: (next: EpisodeRef) => void;
}) {
  const [left, setLeft] = useState<number | null>(null);
  const [exact, setExact] = useState(UP_NEXT_SECONDS);
  const [cancelled, setCancelled] = useState(false);
  const play = useRef(onPlay);
  play.current = onPlay;
  const cancelledRef = useRef(cancelled);
  cancelledRef.current = cancelled;

  useEffect(() => {
    if (!video) return;
    const update = () => {
      const count = upNextCountdown(video.currentTime, video.duration);
      setLeft(count);
      setExact(Math.max(0, video.duration - video.currentTime));
      // Seeking back out of the last seconds arms the card again.
      if (count === null) setCancelled(false);
    };
    const ended = () => !cancelledRef.current && play.current(next);
    video.addEventListener("timeupdate", update);
    video.addEventListener("seeked", update);
    video.addEventListener("ended", ended);
    return () => {
      video.removeEventListener("timeupdate", update);
      video.removeEventListener("seeked", update);
      video.removeEventListener("ended", ended);
    };
  }, [video, next]);

  if (left === null || cancelled) return null;
  const name = hasOwnTitle(next) ? next.title : null;
  return (
    <section className="up-next" data-raised={controlsShown} aria-labelledby="up-next-title">
      <span className="sr-only" role="status">
        Next episode, {next.label}
        {name ? `, ${name}` : ""}, plays in {UP_NEXT_SECONDS} seconds.
      </span>
      <div className="up-next-thumb" aria-hidden>
        {next.thumb ? <Artwork image={next.thumb} size="small" alt="" fill /> : <span className="up-next-thumb-blank" />}
        <svg className="up-next-ring" viewBox="0 0 52 52">
          <circle cx="26" cy="26" r="22" className="up-next-ring-track" />
          <circle cx="26" cy="26" r="22" className="up-next-ring-fill" strokeDasharray={RING} strokeDashoffset={RING * (1 - Math.min(1, exact / UP_NEXT_SECONDS))} />
        </svg>
        <span className="up-next-count">{left}</span>
      </div>
      <div className="up-next-body">
        <h2 id="up-next-title">Next episode</h2>
        <p className="up-next-episode">
          <span className="up-next-label">{next.label}</span>
          {name && <span className="up-next-name">{name}</span>}
        </p>
        <div className="up-next-actions">
          <button type="button" className="btn btn-primary btn-small" onClick={() => onPlay(next)}>
            <PlayIcon size={15} />
            Play now
          </button>
          <button type="button" className="btn btn-quiet btn-small" onClick={() => setCancelled(true)}>
            Cancel
          </button>
        </div>
      </div>
    </section>
  );
}
