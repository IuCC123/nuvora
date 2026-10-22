import { useCallback, useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate, useParams, useSearch } from "@tanstack/react-router";
import { client, queries } from "../nuvora";
import { ChevronLeftIcon } from "../components/Icon";
import { Notice } from "../components/Notice";
import { playerFailed, playState } from "../lib/playState";
import { useDocumentTitle } from "../lib/useDocumentTitle";
import "../styles/player.css";

/** The Back control hides after this long without pointer movement while the video plays. */
const IDLE_MS = 2500;

/**
 * Full-window `<video>` with native controls (Step 1). The URL comes from the
 * cached item's playback decision; the server does Range, so seeking is native.
 * Escape or Back returns to the detail screen. A file the browser rejects
 * despite the server's decision shows the same notice as a server refusal.
 */
export function Player() {
  const { id } = useParams({ from: "/movie/$id/play" });
  const { file } = useSearch({ from: "/movie/$id/play" });
  const navigate = useNavigate();
  const { data: movie } = useQuery(queries.item(id));
  const [failed, setFailed] = useState(false);
  const idle = useIdle(IDLE_MS);
  useDocumentTitle(movie?.title);

  const back = useCallback(() => void navigate({ to: "/movie/$id", params: { id } }), [navigate, id]);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && !document.fullscreenElement && back();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [back]);

  const version = movie?.versions.find((v) => v.file_id === file);
  const decided = version ? playState([version]) : undefined;
  const notice = failed ? playerFailed : decided?.kind === "blocked" ? decided.notice : undefined;
  const src = version?.playback.kind === "direct_play" ? client.resolve(version.playback.url) : undefined;

  return (
    <div className="player" data-idle={idle && !notice}>
      {src && !failed && (
        <video
          className="player-video"
          src={src}
          autoPlay
          controls
          preload="auto"
          aria-label={movie?.title}
          onError={() => setFailed(true)}
        />
      )}
      <button type="button" className="player-back" onClick={back}>
        <ChevronLeftIcon />
        Back
      </button>
      {notice && (
        <div className="player-notice">
          <h1>{movie?.title}</h1>
          <Notice text={notice} />
        </div>
      )}
    </div>
  );
}

/** True after `ms` without pointer or key activity. */
function useIdle(ms: number) {
  const [idle, setIdle] = useState(false);
  const timer = useRef<number>(undefined);
  useEffect(() => {
    const wake = () => {
      setIdle(false);
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => setIdle(true), ms);
    };
    wake();
    const events = ["pointermove", "pointerdown", "keydown"] as const;
    for (const e of events) window.addEventListener(e, wake);
    return () => {
      window.clearTimeout(timer.current);
      for (const e of events) window.removeEventListener(e, wake);
    };
  }, [ms]);
  return idle;
}
