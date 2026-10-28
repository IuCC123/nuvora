import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useCanGoBack, useNavigate, useParams, useRouter } from "@tanstack/react-router";
import {
  ApiError,
  describeError,
  fileFacts,
  formatRuntime,
  versionChips,
  type Image,
  type MovieDetail,
  type Version,
  queryKeys,
} from "@nuvora/core";
import { client, queries } from "../nuvora";
import { cachedCard } from "../lib/cachedCard";
import { CheckIcon, PlayIcon, RestartIcon, TrashIcon } from "../components/Icon";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { Notice } from "../components/Notice";
import { CardArt } from "../components/CardArt";
import { aboutRows, CastSection, DetailHero, HeroMeta, InfoSection, Overview, Starring, type InfoRow } from "../components/Details";
import { titleCardVars } from "../components/TitleCard";
import { hasInteracted } from "../lib/interacted";
import { languageName } from "../lib/languageName";
import { playable, playState } from "../lib/playState";
import { preloadPlayer } from "../components/player/preload";
import { useDocumentTitle } from "../lib/useDocumentTitle";
import { useUser } from "../lib/session";
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
 * Movie detail. First frame: the title and year from the home card already in
 * cache, over the card's ambient wash. Then the item response fills the
 * backdrop, facts, Play, the cast and the Information block.
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
  // A movie without a backdrop shows its poster (or title card) on the wash; the card alone waits for the item.
  const poster = movie && !movie.backdrop && (
    <>
      <CardArt image={movie.poster} title={movie.title} year={movie.year} large eager />
      <PosterMarks progress={progressOf(movie)} watched={movie.watch.watched} />
    </>
  );
  return (
    <div className="detail" style={titleCardVars(head?.title ?? "")}>
      <DetailHero title={head?.title} backdrop={head?.backdrop} poster={poster}>
        {head && <Meta head={head} movie={movie} />}
        {movie ? <Actions movie={movie} /> : <div className="hero-actions" />}
        <Overview text={movie?.overview} tagline={movie?.details.tagline} />
        <Starring details={movie?.details} />
      </DetailHero>
      <CastSection details={movie?.details} />
      {movie && <InfoSection details={movie.details} columns={infoColumns(movie)} />}
    </div>
  );
}

function Meta({ head, movie }: { head: Head; movie: MovieDetail | undefined }) {
  const runtime = movie?.runtime_ms ?? movie?.versions[0]?.media?.duration_ms;
  const genres = movie?.details.genres.slice(0, 2).join(", ");
  const parts = [genres, head.year, runtime ? formatRuntime(runtime) : null].filter((p) => p != null && p !== "");
  return <HeroMeta parts={parts} rating={movie?.details.rating} chips={movie?.versions[0] ? versionChips(movie.versions[0]) : []} />;
}

/** Where the resume point is, and how long the version it belongs to runs. */
function resumePoint(movie: MovieDetail): { position: number; duration: number | null } | null {
  const { position_ms, file_id } = movie.watch;
  if (position_ms == null) return null;
  const duration = movie.versions.find((v) => v.file_id === file_id)?.media?.duration_ms ?? movie.runtime_ms ?? null;
  return { position: position_ms, duration };
}

/** How far into the movie its progress is, 0 to 1. */
function progressOf(movie: MovieDetail): number | null {
  const at = resumePoint(movie);
  return at?.duration ? Math.min(1, at.position / at.duration) : null;
}

function Actions({ movie }: { movie: MovieDetail }) {
  const state = playState(movie.versions);
  useEffect(() => preloadPlayer(movie.versions), [movie.versions]);
  const play = useRef<HTMLAnchorElement>(null);
  // The poster that opened this page is gone, so focus would fall to <body>; give it to Play instead.
  // A page someone loaded directly has no opener, so focus stays put rather than ringing Play unasked.
  useEffect(() => {
    if (hasInteracted() && document.activeElement === document.body) play.current?.focus({ preventScroll: true });
  }, []);
  if (state.kind === "play") {
    const at = resumePoint(movie);
    // Resume the version the position belongs to while it can still play here; otherwise the default one.
    const { file_id } = movie.watch;
    const resumeFile = movie.versions.some((v) => v.file_id === file_id && playable(v)) ? file_id! : state.fileId;
    const progress = progressOf(movie);
    return (
      <div className="hero-actions">
        {at ? (
          <>
            <Link ref={play} to="/movie/$id/play" params={{ id: movie.id }} search={{ file: resumeFile, t: Math.floor(at.position / 1000) }} className="btn btn-primary btn-hero">
              <PlayIcon />
              Resume
              {progress != null && (
                <span className="btn-progress" aria-hidden>
                  <span style={{ transform: `scaleX(${Math.max(0.04, progress)})` }} />
                </span>
              )}
              {at.duration != null && <span className="btn-aside">{formatRuntime(Math.max(0, at.duration - at.position))} left</span>}
            </Link>
            <Link to="/movie/$id/play" params={{ id: movie.id }} search={{ file: state.fileId }} className="hero-round" aria-label="Start over" title="Start over">
              <RestartIcon size={20} />
            </Link>
          </>
        ) : (
          <Link ref={play} to="/movie/$id/play" params={{ id: movie.id }} search={{ file: state.fileId }} className="btn btn-primary btn-hero">
            <PlayIcon />
            Play
          </Link>
        )}
        <WatchedToggle movie={movie} />
        <DeleteMovie movie={movie} />
      </div>
    );
  }
  return (
    <>
      <div className="hero-actions">
        <button type="button" className="btn btn-primary btn-hero" disabled aria-describedby="play-notice">
          <PlayIcon />
          Play
        </button>
        <WatchedToggle movie={movie} />
        <DeleteMovie movie={movie} />
      </div>
      <Notice id="play-notice" text={state.notice} />
    </>
  );
}

function WatchedToggle({ movie }: { movie: MovieDetail }) {
  const toggle = useWatchedToggle(movie.id);
  const watched = movie.watch.watched;
  return (
    <button
      type="button"
      className="hero-round"
      aria-pressed={watched}
      aria-label="Watched"
      title={watched ? "Mark as unwatched" : "Mark as watched"}
      onClick={() => toggle.mutate(!watched)}
    >
      <CheckIcon size={20} strokeWidth={2.4} />
    </button>
  );
}

/** For admins: deletes the movie from the server's disk, once they confirm. */
function DeleteMovie({ movie }: { movie: MovieDetail }) {
  const admin = useUser()?.role === "admin";
  const [confirming, setConfirming] = useState(false);
  const qc = useQueryClient();
  const router = useRouter();
  const navigate = useNavigate();
  const canGoBack = useCanGoBack();
  const remove = useMutation({
    mutationFn: () => client.admin.deleteMovie(movie.id),
    onSuccess: () => {
      for (const queryKey of [queryKeys.home, queryKeys.rows, queryKeys.library, queryKeys.libraries, ["search"]]) {
        void qc.invalidateQueries({ queryKey });
      }
      qc.removeQueries({ queryKey: queryKeys.item(movie.id) });
      if (canGoBack) router.history.back();
      else void navigate({ to: "/", replace: true });
    },
  });
  if (!admin) return null;
  const files = movie.versions.length;
  return (
    <>
      <button type="button" className="hero-round" aria-label="Delete movie" title="Delete movie" onClick={() => setConfirming(true)}>
        <TrashIcon size={20} />
      </button>
      {confirming && (
        <ConfirmDialog
          title={`Delete ${movie.title}?`}
          confirmLabel="Delete movie"
          pending={remove.isPending}
          error={remove.error ? describeError(remove.error) : null}
          onConfirm={() => remove.mutate()}
          onCancel={() => {
            remove.reset();
            setConfirming(false);
          }}
        >
          <p>
            {files > 1 ? `Its ${files} files are` : "Its file is"} deleted from your server, with its folder when nothing else of yours is in it. This
            can't be undone.
          </p>
        </ConfirmDialog>
      )}
    </>
  );
}

/** Information: what the movie is, the languages of its file, and the file itself. */
function infoColumns(movie: MovieDetail) {
  const version: Version | undefined = movie.versions[0];
  const facts = version ? fileFacts(version, languageName) : [];
  const rows = (labels: string[]): InfoRow[] => facts.filter((f) => labels.includes(f.label));
  const about: InfoRow[] = [
    ...(movie.year ? [{ label: "Released", value: movie.year }] : []),
    ...(movie.runtime_ms ? [{ label: "Runtime", value: formatRuntime(movie.runtime_ms) }] : []),
    ...aboutRows(movie.details, movie.title),
  ];
  return [
    { title: "About", rows: about },
    { title: "Languages", rows: rows(["Audio", "Subtitles"]) },
    { title: "File", rows: rows(["Video", "Length", "Location"]) },
  ];
}
