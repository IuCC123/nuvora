import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useQuery } from "@tanstack/react-query";
import { Link, useNavigate, useParams, useSearch } from "@tanstack/react-router";
import {
  ApiError,
  formatClock,
  formatRuntime,
  hasOwnTitle,
  initialSeason,
  plural,
  showCounts,
  showPlay,
  type Episode,
  type SeasonSummary,
  type ShowDetail,
} from "@nuvora/core";
import { queries, queryClient } from "../nuvora";
import { Artwork } from "../components/Artwork";
import { CheckIcon, ChevronDownIcon, PlayIcon } from "../components/Icon";
import { Menu, MenuRadio } from "../components/Menu";
import { PosterMarks } from "../components/PosterCard";
import { CardArt } from "../components/CardArt";
import { CastSection, DetailFacts, Overview } from "../components/Details";
import { titleCardVars } from "../components/TitleCard";
import { cachedCard } from "../lib/cachedCard";
import { hasInteracted } from "../lib/interacted";
import { useDocumentTitle } from "../lib/useDocumentTitle";
import { useWatchedToggle } from "../lib/useWatchedToggle";
import { ErrorState, MissingShow } from "./ErrorState";
import "../styles/home.css";
import "../styles/movie.css";
import "../styles/show.css";

/** More seasons than this and the tabs become a menu, so the row never wraps or scrolls. */
const MAX_SEASON_TABS = 6;

/**
 * A show: its ambient wash, poster and the one button that continues it, then
 * its seasons and the open season's episodes. The open season is in the URL
 * (`?season=2`), so Back and a shared link land on it.
 */
export function ShowScreen() {
  const { id } = useParams({ from: "/shell/show/$id" });
  const search = useSearch({ from: "/shell/show/$id" });
  const item = useQuery(queries.item(id));
  const card = cachedCard(id, "show");
  const show = item.data?.kind === "show" ? item.data : undefined;
  useDocumentTitle(show?.title ?? card?.title);

  if (item.error || (item.data && !show)) {
    return item.data || (item.error instanceof ApiError && item.error.status === 404) ? (
      <MissingShow />
    ) : (
      <ErrorState title="Can't reach your Nuvora server" detail="Check that it's running, then try again." retry={() => void item.refetch()} />
    );
  }
  const head = show ?? card;
  const seasonIndex = show ? (show.seasons.some((s) => s.index === search.season) ? search.season : initialSeason(show)) : undefined;
  const season = show?.seasons.find((s) => s.index === seasonIndex);
  return (
    <div className="show" style={titleCardVars(head?.title ?? "")}>
      <div className="show-ambient" aria-hidden>
        {show?.backdrop ? (
          <div className="detail-backdrop">
            <Artwork image={show.backdrop} size="large" alt="" fill eager />
          </div>
        ) : (
          <div className="detail-wash" />
        )}
        <div className="detail-scrim" />
      </div>
      {head && (
        <div className="detail-content show-head">
          <div className="detail-poster">
            <CardArt image={head.poster ?? show?.backdrop} title={head.title} year={show ? null : card?.year} large eager />
            {show && <PosterMarks progress={null} watched={show.watched_count === show.episode_count && show.episode_count > 0} />}
          </div>
          <div className="detail-info">
            <h1>{head.title}</h1>
            <div className="detail-meta">
              {show ? (
                <>
                  {show.year_range && (
                    <>
                      <span>{show.year_range}</span>
                      <span className="detail-sep" aria-hidden>
                        ·
                      </span>
                    </>
                  )}
                  <span>{showCounts(show)}</span>
                  {show.watched_count > 0 && show.watched_count < show.episode_count && (
                    <span className="show-progress-note">{show.watched_count.toLocaleString()} watched</span>
                  )}
                </>
              ) : (
                <span className="show-meta-pending" />
              )}
            </div>
            <DetailFacts details={show?.details} />
            <Overview text={show?.overview} tagline={show?.details.tagline} />
            {show ? <ShowActions show={show} /> : <div className="detail-actions" />}
          </div>
        </div>
      )}
      {show && season && <Seasons show={show} season={season} />}
      <CastSection details={show?.details} />
    </div>
  );
}

/** Episode ids of this show's seasons already in cache, so a whole-show mark flips the open list at once. */
function cachedEpisodeIds(show: ShowDetail): number[] {
  return show.seasons.flatMap((s) => queryClient.getQueryData<Episode[]>(queries.episodes(s.id).queryKey)?.map((e) => e.id) ?? []);
}

function ShowActions({ show }: { show: ShowDetail }) {
  const firstSeason = show.seasons.find((s) => s.index > 0) ?? show.seasons[0];
  // With every episode watched there is no Next Up; Play starts the show over from its first episode.
  const first = useQuery({ ...queries.episodes(firstSeason?.id ?? 0), enabled: !show.next && !!firstSeason });
  const firstEpisode = first.data?.[0];
  const play = showPlay(show, firstEpisode ? { id: firstEpisode.id, label: firstEpisode.label } : null);
  const button = useRef<HTMLAnchorElement>(null);
  const allWatched = show.episode_count > 0 && show.watched_count === show.episode_count;
  const toggle = useWatchedToggle(show.id, { episodes: cachedEpisodeIds(show) });
  useEffect(() => {
    if (hasInteracted() && document.activeElement === document.body) button.current?.focus({ preventScroll: true });
  }, []);
  return (
    <div className="detail-actions">
      {play ? (
        <Link ref={button} to="/episode/$id/play" params={{ id: play.episode.id }} search={{}} className="btn btn-primary">
          <PlayIcon />
          {play.kind === "resume" ? "Resume" : play.kind === "next" ? "Next episode" : "Play"}
          <span className="btn-aside">
            {play.episode.label}
            {play.kind === "resume" && ` from ${formatClock(play.positionMs)}`}
          </span>
        </Link>
      ) : (
        <span className="btn btn-primary btn-placeholder" aria-hidden />
      )}
      <button type="button" className="btn btn-secondary btn-watched" data-watched={allWatched} onClick={() => toggle.mutate(!allWatched)}>
        <CheckIcon size={18} />
        {allWatched ? "Mark show unwatched" : "Mark show watched"}
      </button>
    </div>
  );
}

function Seasons({ show, season }: { show: ShowDetail; season: SeasonSummary }) {
  const navigate = useNavigate();
  const episodes = useQuery(queries.episodes(season.id));
  const choose = (index: number) =>
    void navigate({ to: "/show/$id", params: { id: show.id }, search: { season: index }, replace: true, resetScroll: false });
  const prefetch = (s: SeasonSummary) => void queryClient.prefetchQuery(queries.episodes(s.id));
  const seasonWatched = season.episode_count > 0 && season.watched_count === season.episode_count;
  const toggle = useWatchedToggle(season.id, { showId: show.id, episodes: episodes.data?.map((e) => e.id) });
  const { sentinel, stuck } = useStuck();

  return (
    <section className="seasons" aria-label="Episodes">
      <div ref={sentinel} className="seasons-sentinel" aria-hidden />
      <div className="seasons-bar" data-stuck={stuck}>
        {show.seasons.length > MAX_SEASON_TABS ? (
          <Menu
            label="Seasons"
            triggerClassName="tool-btn season-menu-btn"
            trigger={
              <>
                <span>{season.title}</span>
                <ChevronDownIcon />
              </>
            }
          >
            {show.seasons.map((s) => (
              <MenuRadio key={s.id} checked={s.id === season.id} onSelect={() => choose(s.index)} aside={plural(s.episode_count, "episode")}>
                {s.title}
              </MenuRadio>
            ))}
          </Menu>
        ) : (
          <SeasonTabs show={show} season={season} onChoose={choose} onIntent={prefetch} />
        )}
        <p className="seasons-count">
          {plural(season.episode_count, "episode")}
          {season.watched_count > 0 && !seasonWatched && <span> · {season.watched_count} watched</span>}
        </p>
        <button type="button" className="btn btn-quiet btn-small seasons-mark" data-watched={seasonWatched} onClick={() => toggle.mutate(!seasonWatched)}>
          <CheckIcon size={16} />
          {seasonWatched ? `Mark ${season.index === 0 ? "specials" : "season"} unwatched` : `Mark ${season.index === 0 ? "specials" : "season"} watched`}
        </button>
      </div>
      <div id="season-panel" role="tabpanel" aria-label={season.title} className="season-panel">
        {episodes.isError ? (
          <ErrorState title="Couldn't load these episodes" detail="Check that your Nuvora server is running, then try again." retry={() => void episodes.refetch()} />
        ) : (
          <EpisodeList show={show} episodes={episodes.data} placeholderCount={Math.min(season.episode_count, 8)} />
        )}
      </div>
    </section>
  );
}

/** Tabs follow focus: Left and Right move to the neighbouring season and open it. */
function SeasonTabs({ show, season, onChoose, onIntent }: { show: ShowDetail; season: SeasonSummary; onChoose: (index: number) => void; onIntent: (s: SeasonSummary) => void }) {
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const at = show.seasons.findIndex((s) => s.id === season.id);
    const to = e.key === "ArrowRight" ? at + 1 : e.key === "ArrowLeft" ? at - 1 : e.key === "Home" ? 0 : e.key === "End" ? show.seasons.length - 1 : null;
    const next = to === null ? undefined : show.seasons[(to + show.seasons.length) % show.seasons.length];
    if (!next) return;
    e.preventDefault();
    onChoose(next.index);
    e.currentTarget.querySelector<HTMLElement>(`[data-season="${next.index}"]`)?.focus();
  };
  return (
    <div role="tablist" aria-label="Seasons" className="season-tabs" onKeyDown={onKeyDown}>
      {show.seasons.map((s) => {
        const selected = s.id === season.id;
        return (
          <button
            key={s.id}
            type="button"
            role="tab"
            data-season={s.index}
            aria-selected={selected}
            aria-controls="season-panel"
            tabIndex={selected ? 0 : -1}
            className="season-tab"
            onClick={() => onChoose(s.index)}
            onPointerEnter={() => onIntent(s)}
            onFocus={() => onIntent(s)}
          >
            {s.title}
            {s.episode_count > 0 && s.watched_count === s.episode_count && <CheckIcon size={13} strokeWidth={3} className="season-tab-check" aria-label="watched" />}
          </button>
        );
      })}
    </div>
  );
}

function EpisodeList({ show, episodes, placeholderCount }: { show: ShowDetail; episodes: Episode[] | undefined; placeholderCount: number }) {
  if (!episodes) {
    return (
      <ol className="episodes" aria-busy="true" aria-label="Loading episodes">
        {Array.from({ length: placeholderCount }, (_, i) => (
          <li key={i} className="episode episode-pending" aria-hidden>
            <span className="episode-thumb" />
            <span className="episode-text">
              <span className="pending-line" />
              <span className="pending-line pending-line-short" />
            </span>
          </li>
        ))}
      </ol>
    );
  }
  if (!episodes.length) return <p className="episodes-empty">No episodes in this season have files right now.</p>;
  return (
    <ol className="episodes" onKeyDown={moveFocus}>
      {episodes.map((e) => (
        <EpisodeRow key={e.id} show={show} episode={e} upNext={show.next?.id === e.id} />
      ))}
    </ol>
  );
}

function EpisodeRow({ show, episode, upNext }: { show: ShowDetail; episode: Episode; upNext: boolean }) {
  const toggle = useWatchedToggle(episode.id, { showId: show.id });
  const { watch } = episode;
  const runtime = episode.runtime_ms ?? episode.versions[0]?.media?.duration_ms ?? null;
  const progress = watch.position_ms != null && runtime ? Math.min(1, watch.position_ms / runtime) : null;
  const left = progress != null && runtime ? runtime - watch.position_ms! : null;
  const own = hasOwnTitle(episode);
  const facts = [own ? `Episode ${episode.index}` : null, runtime ? formatRuntime(runtime) : null, left ? `${formatRuntime(left)} left` : null].filter(Boolean);
  const state = progress != null ? `${Math.max(1, Math.round(progress * 100))}% watched` : watch.watched ? "Watched" : null;
  return (
    <li className="episode" data-watched={watch.watched && progress == null}>
      <Link
        to="/episode/$id/play"
        params={{ id: episode.id }}
        search={{}}
        className="episode-link"
        onFocus={() => void queryClient.prefetchQuery(queries.item(episode.id))}
        onPointerEnter={() => void queryClient.prefetchQuery(queries.item(episode.id))}
      >
        <span className="episode-thumb" aria-hidden>
          {episode.thumb ? (
            <Artwork image={episode.thumb} size="small" alt="" fill />
          ) : (
            <span className="episode-blank">
              <span>{episode.index}</span>
            </span>
          )}
          <PosterMarks progress={progress} watched={false} />
          <span className="poster-hover">
            <span className="poster-hover-play">
              <PlayIcon size={20} />
            </span>
          </span>
        </span>
        <span className="episode-text">
          <span className="episode-title">
            {episode.title}
            {upNext && <span className="episode-tag">Up next</span>}
          </span>
          <span className="episode-facts">{facts.join(" · ")}</span>
          {state && <span className="sr-only">, {state}</span>}
        </span>
      </Link>
      <button
        type="button"
        className="episode-mark"
        aria-pressed={watch.watched}
        aria-label={`Watched: ${episode.label}`}
        title={watch.watched ? "Mark as unwatched" : "Mark as watched"}
        onClick={() => toggle.mutate(!watch.watched)}
      >
        <CheckIcon size={16} strokeWidth={2.6} />
      </button>
    </li>
  );
}

/** Whether the sticky bar after the sentinel has reached the top bar, so it only paints a background once it covers the list. */
function useStuck() {
  const [stuck, setStuck] = useState(false);
  const [node, sentinel] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (!node) return;
    const top = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--nv-layout-top-bar")) || 56;
    const observer = new IntersectionObserver(([entry]) => setStuck(!!entry && !entry.isIntersecting && entry.boundingClientRect.top < top + 1), {
      rootMargin: `-${top + 1}px 0px 0px 0px`,
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, [node]);
  return { sentinel, stuck };
}

/** Up and Down walk the episode links, the way Left and Right walk a poster row. */
function moveFocus(e: KeyboardEvent<HTMLOListElement>) {
  if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
  const links = [...e.currentTarget.querySelectorAll<HTMLAnchorElement>("a.episode-link")];
  const at = links.indexOf(document.activeElement as HTMLAnchorElement);
  const next = links[at + (e.key === "ArrowDown" ? 1 : -1)];
  if (at < 0 || !next) return;
  e.preventDefault();
  next.focus();
}

