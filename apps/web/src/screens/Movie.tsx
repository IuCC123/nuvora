import { Fragment, useEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { Link, useParams } from "@tanstack/react-router";
import {
  ApiError,
  fileFacts,
  formatClock,
  formatRuntime,
  versionChips,
  type Image,
  type MovieDetail,
  type Version,
} from "@nuvora/core";
import { queries } from "../nuvora";
import { cachedCard } from "../lib/cachedCard";
import { Artwork } from "../components/Artwork";
import { CheckIcon, PlayIcon, RestartIcon } from "../components/Icon";
import { Notice } from "../components/Notice";
import { TitleCard, titleCardVars } from "../components/TitleCard";
import { hasInteracted } from "../lib/interacted";
import { languageName } from "../lib/languageName";
import { playState } from "../lib/playState";
import { useDocumentTitle } from "../lib/useDocumentTitle";
import { useWatchedToggle } from "../lib/useWatchedToggle";
import { PosterMarks } from "../components/PosterCard";
import { ErrorState, MissingMovie } from "./ErrorState";
import "../styles/movie.css";

/** What the detail page can show before the item arrives: the home card's title, year and poster. */
interface Head {
  title: string;
  year?: number | null | undefined;
  poster?: Image | null | undefined;
  backdrop?: Image | null | undefined;
}

/**
 * Movie detail. First frame: title, year and poster (or title card) from the
 * home card already in cache, over the card's ambient wash. Then the item
 * response fills backdrop, runtime, chips, Play and the File facts.
 */
export function MovieScreen() {
  const { id } = useParams({ from: "/shell/movie/$id" });
  const item = useQuery(queries.item(id));
  const card = cachedCard(id, "movie");
  const movie = item.data?.kind === "movie" ? item.data : undefined;
  useDocumentTitle(movie?.title ?? card?.title);

  if (item.error) {
    return item.error instanceof ApiError && item.error.status === 404 ? (
      <MissingMovie />
    ) : (
      <ErrorState title="Can't reach your Nuvora server" detail="Check that it's running, then try again." retry={() => void item.refetch()} />
    );
  }
  const head: Head | undefined = movie ?? card;
  return (
    <div className="detail" style={titleCardVars(head?.title ?? "")}>
      {head?.backdrop ? (
        <div className="detail-backdrop">
          <Artwork image={head.backdrop} size="large" alt="" fill eager />
        </div>
      ) : (
        <div className="detail-wash" />
      )}
      <div className="detail-scrim" />
      {head && (
        <div className="detail-content">
          <div className="detail-poster">
            {head.poster ? <Artwork image={head.poster} size="large" alt="" eager /> : <TitleCard title={head.title} year={head.year} large />}
            {movie && <PosterMarks progress={progressOf(movie)} watched={movie.watch.watched} />}
          </div>
          <div className="detail-info">
            <h1>{head.title}</h1>
            <Meta head={head} movie={movie} />
            {movie?.overview && <p className="detail-overview">{movie.overview}</p>}
            {movie ? <Actions movie={movie} /> : <div className="detail-actions" />}
            {movie?.versions[0] && <Facts version={movie.versions[0]} />}
          </div>
        </div>
      )}
    </div>
  );
}

function Meta({ head, movie }: { head: Head; movie: MovieDetail | undefined }) {
  const runtime = movie?.runtime_ms ?? movie?.versions[0]?.media?.duration_ms;
  const parts = [head.year, runtime ? formatRuntime(runtime) : null].filter((p) => p != null);
  const chips = movie?.versions[0] ? versionChips(movie.versions[0]) : [];
  return (
    <div className="detail-meta">
      {parts.map((part, i) => (
        <Fragment key={i}>
          {i > 0 && (
            <span className="detail-sep" aria-hidden>
              ·
            </span>
          )}
          <span>{part}</span>
        </Fragment>
      ))}
      {chips.length > 0 && (
        <ul className="chips" role="list" aria-label="Technical details">
          {chips.map((chip) => (
            <li key={chip} className="chip">
              {chip}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** How far into the movie its progress is, 0 to 1, from the resume point and the version's length. */
function progressOf(movie: MovieDetail): number | null {
  const { position_ms, file_id } = movie.watch;
  const duration = movie.versions.find((v) => v.file_id === file_id)?.media?.duration_ms ?? movie.runtime_ms;
  return position_ms != null && duration ? Math.min(1, position_ms / duration) : null;
}

function Actions({ movie }: { movie: MovieDetail }) {
  const state = playState(movie.versions);
  const play = useRef<HTMLAnchorElement>(null);
  // The poster that opened this page is gone, so focus would fall to <body>; give it to Play instead.
  // A page someone loaded directly has no opener, so focus stays put rather than ringing Play unasked.
  useEffect(() => {
    if (hasInteracted() && document.activeElement === document.body) play.current?.focus({ preventScroll: true });
  }, []);
  if (state.kind === "play") {
    const { position_ms, file_id } = movie.watch;
    // Resume the version the position belongs to while it can still play here; otherwise the default one.
    const resumeFile = movie.versions.some((v) => v.file_id === file_id && v.playback.kind === "direct_play") ? file_id! : state.fileId;
    return (
      <div className="detail-actions">
        {position_ms != null ? (
          <>
            <Link ref={play} to="/movie/$id/play" params={{ id: movie.id }} search={{ file: resumeFile, t: Math.floor(position_ms / 1000) }} className="btn btn-primary">
              <PlayIcon />
              Resume
              <span className="btn-aside">from {formatClock(position_ms)}</span>
            </Link>
            <Link to="/movie/$id/play" params={{ id: movie.id }} search={{ file: state.fileId }} className="btn btn-secondary">
              <RestartIcon />
              Start over
            </Link>
          </>
        ) : (
          <Link ref={play} to="/movie/$id/play" params={{ id: movie.id }} search={{ file: state.fileId }} className="btn btn-primary">
            <PlayIcon />
            Play
          </Link>
        )}
        <WatchedToggle movie={movie} />
      </div>
    );
  }
  return (
    <>
      <div className="detail-actions">
        <button type="button" className="btn btn-primary" disabled aria-describedby="play-notice">
          <PlayIcon />
          Play
        </button>
        <WatchedToggle movie={movie} />
      </div>
      <Notice id="play-notice" text={state.notice} />
    </>
  );
}

function WatchedToggle({ movie }: { movie: MovieDetail }) {
  const toggle = useWatchedToggle(movie.id);
  const watched = movie.watch.watched;
  return (
    <button type="button" className="btn btn-secondary btn-watched" data-watched={watched} onClick={() => toggle.mutate(!watched)}>
      <CheckIcon size={18} />
      {watched ? "Mark as unwatched" : "Mark as watched"}
    </button>
  );
}

function Facts({ version }: { version: Version }) {
  return (
    <section className="facts" aria-labelledby="facts-title">
      <h2 id="facts-title">File</h2>
      <dl>
        {fileFacts(version, languageName).map((fact) => (
          <div key={fact.label} className="fact">
            <dt>{fact.label}</dt>
            <dd>
              {fact.value}
              {fact.detail && <span className="fact-detail"> · {fact.detail}</span>}
            </dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
