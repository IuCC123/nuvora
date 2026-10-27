import { useEffect, useRef, useState, type FormEvent } from "react";
import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useSearch } from "@tanstack/react-router";
import {
  contentLabel,
  describeError,
  formatBytes,
  formatDuration,
  formatLastSeen,
  languageLabels,
  queryKeys,
  releaseLanguage,
  type Download,
  type DownloadState,
  type ReleaseResult,
  type ReleaseSource,
} from "@nuvora/core";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { AlertIcon, CheckIcon, DownloadIcon, PlayIcon, SearchIcon } from "../components/Icon";
import { client, queries } from "../nuvora";
import { useDocumentTitle } from "../lib/useDocumentTitle";
import { LoadError } from "./settings/SettingsPage";
import "../styles/forms.css";
import "../styles/settings.css";
import "../styles/downloads.css";

/** Polled fast while something moves, slowly otherwise (another admin may start one). */
const BUSY_POLL_MS = 1000;
const IDLE_POLL_MS = 10_000;
const BUSY: readonly DownloadState[] = ["starting", "checking", "downloading", "moving"];

type Quality = "any" | 2160 | 1080 | 720;
const QUALITIES: readonly { value: Quality; label: string }[] = [
  { value: "any", label: "Any quality" },
  { value: 2160, label: "4K" },
  { value: 1080, label: "1080p" },
  { value: 720, label: "720p" },
];

type Kind = "all" | "movies" | "shows";
const KINDS: readonly { value: Kind; label: string }[] = [
  { value: "all", label: "All" },
  { value: "movies", label: "Movies" },
  { value: "shows", label: "Shows" },
];

/** Remembered on this device: whether releases only in other languages are hidden. */
const ENGLISH_ONLY_KEY = "nuvora.downloads.englishOnly";

/**
 * Admins find a movie on their indexers and download it into a library
 * without leaving Nuvora: search results on the left, the downloads on the
 * right, each moving into the library by itself when it finishes.
 */
export function Downloads() {
  const { q = "" } = useSearch({ from: "/shell/downloads" });
  const query = q.trim();
  useDocumentTitle(query ? `“${query}” · Downloads` : "Downloads");
  const settings = useQuery(queries.downloadSettings());
  const downloads = useQuery({
    ...queries.downloads(),
    refetchInterval: (q) => (q.state.data?.some((d) => BUSY.includes(d.state)) ? BUSY_POLL_MS : IDLE_POLL_MS),
  });
  useScanWhenDone(downloads.data);

  return (
    <div className="downloads-page">
      <header className="downloads-head">
        <div>
          <h1 tabIndex={-1}>Downloads</h1>
          <p>Find a movie or show on your indexers. Play it right away while it downloads, or download it into your library.</p>
        </div>
        <Link to="/settings/downloads" className="btn btn-secondary btn-small">
          Indexers
        </Link>
      </header>
      {settings.data?.unavailable && (
        <div className="downloads-alert" role="alert">
          <AlertIcon />
          <p>This server can't download right now: {settings.data.unavailable}. Restart it to try again.</p>
        </div>
      )}
      <SearchForm initial={query} />
      <div className="downloads-layout">
        <section className="downloads-results" aria-label="Search results">
          <Results query={query} hasIndexers={(settings.data?.indexers.length ?? 1) > 0} downloads={downloads.data ?? []} />
        </section>
        <aside className="downloads-queue" aria-labelledby="queue-heading">
          <h2 id="queue-heading">
            Your downloads {downloads.data && downloads.data.length > 0 && <span className="downloads-count">{downloads.data.length}</span>}
          </h2>
          {downloads.isPending ? (
            <div className="queue-skeleton" aria-busy="true" aria-label="Loading downloads" />
          ) : downloads.isError && !downloads.data ? (
            <LoadError error={downloads.error} retry={() => void downloads.refetch()} />
          ) : downloads.data.length === 0 ? (
            <p className="queue-empty">Nothing yet. Pick a release, or paste a magnet link into the search box.</p>
          ) : (
            <ul role="list" className="queue-list">
              {downloads.data.map((d) => (
                <DownloadRow key={d.id} download={d} />
              ))}
            </ul>
          )}
        </aside>
      </div>
    </div>
  );
}

/** A finished download starts a scan on the server; asking for its status lets the top bar follow it and refresh Home after. */
function useScanWhenDone(downloads: Download[] | undefined) {
  const qc = useQueryClient();
  const done = useRef<Set<number> | null>(null);
  useEffect(() => {
    if (!downloads) return;
    const now = new Set(downloads.filter((d) => d.state === "done").map((d) => d.id));
    const fresh = done.current !== null && [...now].some((id) => !done.current!.has(id));
    done.current = now;
    if (fresh) {
      void qc.invalidateQueries({ queryKey: queryKeys.scan });
      void qc.invalidateQueries({ queryKey: queryKeys.libraries });
    }
  }, [downloads, qc]);
}

const isLink = (text: string) => text.startsWith("magnet:?") || /^https?:\/\/\S+$/.test(text);

/** Starts playing a release while it downloads, then opens its player. */
function useOpenStream() {
  const navigate = useNavigate();
  return useMutation({
    mutationFn: client.admin.openStream,
    onSuccess: (stream) => void navigate({ to: "/watch/$id", params: { id: stream.id } }),
  });
}

/** Searches the indexers, or plays or downloads a pasted magnet or `.torrent` link straight away. */
function SearchForm({ initial }: { initial: string }) {
  const navigate = useNavigate({ from: "/downloads" });
  const qc = useQueryClient();
  const [text, setText] = useState(initial);
  useEffect(() => setText(initial), [initial]);
  const play = useOpenStream();
  const add = useMutation({
    mutationFn: client.admin.addDownload,
    onSuccess: () => {
      setText("");
      void qc.invalidateQueries({ queryKey: queryKeys.downloads });
    },
  });
  const value = text.trim();
  const link = isLink(value);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!value) return;
    if (link) add.mutate({ link: value });
    else void navigate({ search: { q: value } });
  };

  return (
    <form className="downloads-search" role="search" onSubmit={submit}>
      <div className="downloads-search-field">
        <SearchIcon size={20} />
        <input
          className="input"
          type="search"
          aria-label="Movie name, or a magnet link"
          placeholder="Search for a movie, or paste a magnet link"
          value={text}
          autoFocus={!initial}
          spellCheck={false}
          onChange={(e) => {
            add.reset();
            play.reset();
            setText(e.currentTarget.value);
          }}
        />
      </div>
      {link && (
        <button type="button" className="btn btn-secondary" onClick={() => play.mutate({ link: value })} disabled={play.isPending} aria-busy={play.isPending}>
          <PlayIcon size={16} />
          Play
        </button>
      )}
      <button type="submit" className="btn btn-primary" disabled={!value || add.isPending} aria-busy={add.isPending}>
        {link ? (
          <>
            <DownloadIcon size={18} />
            Download link
          </>
        ) : (
          "Search"
        )}
      </button>
      {(add.isError || play.isError) && (
        <p className="downloads-search-error" role="alert">
          <AlertIcon size={14} />
          {describeError(add.error ?? play.error)}
        </p>
      )}
    </form>
  );
}

function Results({ query, hasIndexers, downloads }: { query: string; hasIndexers: boolean; downloads: Download[] }) {
  const [quality, setQuality] = useState<Quality>("any");
  const [kind, setKind] = useState<Kind>("all");
  const [englishOnly, setEnglishOnly] = useState(() => localStorage.getItem(ENGLISH_ONLY_KEY) !== "false");
  const chooseEnglishOnly = (on: boolean) => {
    setEnglishOnly(on);
    localStorage.setItem(ENGLISH_ONLY_KEY, String(on));
  };
  const results = useQuery({ ...queries.releases(query), enabled: query !== "" && hasIndexers, placeholderData: keepPreviousData });

  if (!hasIndexers) {
    return (
      <div className="settings-empty">
        <h2>Connect an indexer first</h2>
        <p>Nuvora searches the Torznab feeds of Jackett or Prowlarr. Add one, and its results show up here.</p>
        <Link to="/settings/downloads" className="btn btn-primary">
          Add an indexer
        </Link>
      </div>
    );
  }
  if (!query) {
    return (
      <div className="results-hint">
        <SearchIcon size={28} />
        <h2>What do you want to watch?</h2>
        <p>Search by name. Add a year for a movie, like “Heat 1995”, or a season for a show, like “Severance S02”.</p>
      </div>
    );
  }
  if (results.isPending) return <ResultSkeleton />;
  if (results.isError && !results.data) return <LoadError error={results.error} retry={() => void results.refetch()} />;

  const { releases, failures } = results.data;
  const fits = (r: ReleaseResult) =>
    (quality === "any" || r.resolution === quality) && (kind === "all" || (kind === "shows") === (r.content.kind !== "movie"));
  const matching = releases.filter(fits);
  const shown = englishOnly ? matching.filter((r) => releaseLanguage(r) !== "other") : matching;
  const hidden = matching.length - shown.length;
  const added = new Set(downloads.map((d) => d.title));
  return (
    <div aria-busy={results.isFetching}>
      <div className="results-head">
        <h2>
          {releases.length === 0 ? "Nothing found" : `${shown.length} ${shown.length === 1 ? "release" : "releases"}`} for <q>{query}</q>
        </h2>
        {releases.length > 0 && (
          <div className="results-filters">
            <Segmented label="Type" options={KINDS} value={kind} onChange={setKind} />
            <Segmented label="Quality" options={QUALITIES} value={quality} onChange={setQuality} />
            <label className="english-only">
              <input type="checkbox" role="switch" className="switch" checked={englishOnly} onChange={(e) => chooseEnglishOnly(e.currentTarget.checked)} />
              English only
            </label>
          </div>
        )}
      </div>
      {hidden > 0 && (
        <p className="results-hidden">
          {hidden} {hidden === 1 ? "release is" : "releases are"} only in other languages.{" "}
          <button type="button" className="link-button" onClick={() => chooseEnglishOnly(false)}>
            Show {hidden === 1 ? "it" : "them"}
          </button>
        </p>
      )}
      {failures.map((f) => (
        <p key={f.indexer} className="results-failure">
          <AlertIcon size={16} />
          {f.indexer} didn't answer. {f.message}.
        </p>
      ))}
      {releases.length === 0 ? (
        <p className="results-none">Try fewer words, or check the spelling. Indexers match the release name, not the title Nuvora shows.</p>
      ) : shown.length === 0 ? (
        <p className="results-none">None match these filters. Change them above.</p>
      ) : (
        <ul role="list" className="release-list">
          {shown.map((release) => (
            <ReleaseRow key={release.link} release={release} added={added.has(release.title)} />
          ))}
        </ul>
      )}
    </div>
  );
}

function Segmented<T extends string | number>({
  label,
  options,
  value,
  onChange,
}: {
  label: string;
  options: readonly { value: T; label: string }[];
  value: T;
  onChange: (value: T) => void;
}) {
  return (
    <div className="quality-filter" role="radiogroup" aria-label={label}>
      {options.map((option) => (
        <button key={option.value} type="button" role="radio" aria-checked={value === option.value} onClick={() => onChange(option.value)}>
          {option.label}
        </button>
      ))}
    </div>
  );
}

const SOURCE_LABEL: Record<ReleaseSource, string> = {
  remux: "Remux",
  bluray: "Blu-ray",
  web: "Web",
  hdtv: "TV",
  dvd: "DVD",
  cam: "Cam",
};

function ReleaseRow({ release, added }: { release: ReleaseResult; added: boolean }) {
  const qc = useQueryClient();
  const play = useOpenStream();
  const add = useMutation({
    mutationFn: () => client.admin.addDownload({ link: release.link, title: release.title }),
    onSuccess: () => void qc.invalidateQueries({ queryKey: queryKeys.downloads }),
  });
  const tags = [
    release.resolution && (release.resolution === 2160 ? "4K" : `${release.resolution}p`),
    release.hdr === "dolby_vision" ? "Dolby Vision" : release.hdr === "hdr" ? "HDR" : null,
    release.codec === "hevc" ? "HEVC" : release.codec === "h264" ? "H.264" : release.codec === "av1" ? "AV1" : null,
  ].filter((t): t is string => !!t);
  const seeders = release.seeders;
  const language = releaseLanguage(release);
  const languages = languageLabels(release);
  const age = release.published ? formatLastSeen(release.published * 1000, Date.now()) : null;
  return (
    <li className="release">
      <div className="release-main">
        <p className="release-movie">
          {release.movie}
          {release.year && <span className="release-year">{release.year}</span>}
          <span className={release.content.kind === "movie" ? "release-content" : "release-content is-show"}>{contentLabel(release.content)}</span>
        </p>
        <p className="release-name" title={release.title}>
          {release.title}
        </p>
        {languages.length > 0 && (
          <ul role="list" className={language === "other" ? "release-languages is-other" : "release-languages"} aria-label="Language">
            {languages.map((l) => (
              <li key={l}>{l}</li>
            ))}
          </ul>
        )}
        <div className="release-meta">
          {(tags.length > 0 || release.source) && (
            <ul role="list" className="release-tags" aria-label="Quality">
              {tags.map((t) => (
                <li key={t}>{t}</li>
              ))}
              {release.source && <li className={release.source === "cam" ? "is-cam" : undefined}>{SOURCE_LABEL[release.source]}</li>}
            </ul>
          )}
          <p className="release-facts">
            {release.size_bytes && <span>{formatBytes(release.size_bytes)}</span>}
            {seeders != null && (
              <span className={seeders === 0 ? "seeders-none" : seeders < 5 ? "seeders-few" : "seeders-many"}>
                {seeders === 0 ? "No seeders" : `${seeders} ${seeders === 1 ? "seeder" : "seeders"}`}
              </span>
            )}
            {age && <span>Added {/^\d/.test(age) ? age : age.toLowerCase()}</span>}
            <span>
              {release.details_url ? (
                <a href={release.details_url} target="_blank" rel="noreferrer noopener">
                  {release.indexer}
                </a>
              ) : (
                release.indexer
              )}
            </span>
          </p>
        </div>
      </div>
      <div className="release-action">
        <div className="release-buttons">
          <button
            type="button"
            className="btn btn-secondary btn-small"
            onClick={() => play.mutate({ link: release.link, title: release.title })}
            disabled={play.isPending}
            aria-busy={play.isPending}
            aria-label={`Play ${release.title} while it downloads`}
          >
            <PlayIcon size={14} />
            Play
          </button>
          {added || add.isSuccess ? (
            <span className="release-added">
              <CheckIcon size={16} />
              Added
            </span>
          ) : (
            <button
              type="button"
              className="btn btn-primary btn-small"
              onClick={() => add.mutate()}
              disabled={add.isPending}
              aria-busy={add.isPending}
              aria-label={`Download ${release.title}`}
            >
              <DownloadIcon size={16} />
              Download
            </button>
          )}
        </div>
        {(add.isError || play.isError) && (
          <p className="release-error" role="alert">
            {describeError(add.error ?? play.error)}
          </p>
        )}
      </div>
    </li>
  );
}

function ResultSkeleton() {
  return (
    <ul role="list" className="release-list" aria-busy="true" aria-label="Searching your indexers">
      {Array.from({ length: 5 }, (_, i) => (
        <li key={i} className="release release-skeleton">
          <span />
          <span />
        </li>
      ))}
    </ul>
  );
}

const STATE_LABEL: Record<DownloadState, string> = {
  starting: "Starting",
  checking: "Checking",
  downloading: "Downloading",
  paused: "Paused",
  moving: "Moving",
  done: "In your library",
  failed: "Stopped",
};

function DownloadRow({ download: d }: { download: Download }) {
  const qc = useQueryClient();
  const [confirming, setConfirming] = useState(false);
  const refresh = () => qc.invalidateQueries({ queryKey: queryKeys.downloads });
  const pause = useMutation({ mutationFn: () => client.admin.pauseDownload(d.id), onSettled: refresh });
  const resume = useMutation({ mutationFn: () => client.admin.resumeDownload(d.id), onSettled: refresh });
  const remove = useMutation({
    mutationFn: () => client.admin.removeDownload(d.id),
    onSuccess: () => setConfirming(false),
    onSettled: refresh,
  });
  const error = pause.error ?? resume.error ?? (confirming ? null : remove.error);
  const percent = d.total_bytes ? Math.min(100, ((d.done_bytes ?? 0) / d.total_bytes) * 100) : null;
  const running = d.state === "starting" || d.state === "checking" || d.state === "downloading";

  return (
    <li className={`queue-item is-${d.state}`}>
      <div className="queue-head">
        <div className="queue-name">
          <p className="queue-title">
            {d.movie}
            {d.year && <span className="release-year">{d.year}</span>}
          </p>
          <p className="queue-release" title={d.title}>
            {d.title}
          </p>
        </div>
        <span className={`queue-state is-${d.state}`}>
          {d.state === "done" && <CheckIcon size={14} />}
          {STATE_LABEL[d.state]}
        </span>
      </div>
      {percent !== null && d.state !== "done" && (
        <div
          className="queue-bar"
          role="progressbar"
          aria-label={`${d.title}: downloaded`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round(percent)}
        >
          <span style={{ width: `${percent}%` }} />
        </div>
      )}
      <p className="queue-detail">{detail(d, percent)}</p>
      <div className="queue-actions">
        {running && (
          <button type="button" className="btn btn-secondary btn-small" onClick={() => pause.mutate()} disabled={pause.isPending}>
            Pause
          </button>
        )}
        {(d.state === "paused" || d.state === "failed") && (
          <button type="button" className="btn btn-secondary btn-small" onClick={() => resume.mutate()} disabled={resume.isPending}>
            {d.state === "failed" ? "Try again" : "Resume"}
          </button>
        )}
        {d.state === "done" && (
          <Link to="/library/$id" params={{ id: d.library_id }} className="btn btn-secondary btn-small">
            Open library
          </Link>
        )}
        {d.state !== "moving" &&
          (d.state === "done" ? (
            <button type="button" className="btn btn-quiet btn-small" onClick={() => remove.mutate()} disabled={remove.isPending}>
              Clear
            </button>
          ) : (
            <button type="button" className="btn btn-quiet btn-quiet-danger btn-small" onClick={() => setConfirming(true)}>
              {d.state === "failed" ? "Remove" : "Cancel"}
            </button>
          ))}
      </div>
      {error && (
        <p className="queue-error" role="alert">
          {describeError(error)}
        </p>
      )}
      {confirming && (
        <ConfirmDialog
          title={d.state === "failed" ? "Remove this download?" : "Cancel this download?"}
          confirmLabel={d.state === "failed" ? "Remove" : "Cancel download"}
          pending={remove.isPending}
          error={remove.error ? describeError(remove.error) : null}
          onConfirm={() => remove.mutate()}
          onCancel={() => {
            remove.reset();
            setConfirming(false);
          }}
        >
          <p>What it has fetched so far is deleted. You can search for it again later.</p>
        </ConfirmDialog>
      )}
    </li>
  );
}

function detail(d: Download, percent: number | null): string {
  const amount = d.total_bytes ? `${formatBytes(d.done_bytes ?? 0)} of ${formatBytes(d.total_bytes)}` : null;
  switch (d.state) {
    case "starting":
      return "Getting the torrent's details";
    case "checking":
      return `Checking what's already here${percent !== null ? `, ${percent.toFixed(0)}%` : ""}`;
    case "downloading": {
      if (d.peers === 0) return [amount, "Looking for peers"].filter(Boolean).join(" · ");
      const parts = [amount, `${formatBytes(d.down_bps)}/s`, `${d.peers} ${d.peers === 1 ? "peer" : "peers"}`];
      if (d.eta_s != null) parts.push(`${formatDuration(d.eta_s * 1000)} left`);
      return parts.filter(Boolean).join(" · ");
    }
    case "paused":
      return amount ? `Paused at ${amount}` : "Paused before it started";
    case "moving":
      return "Moving it into your library";
    case "done":
      return d.path ?? "In your library";
    case "failed":
      return d.error ?? "It stopped";
  }
}
