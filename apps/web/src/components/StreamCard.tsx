import { useMutation, useQueryClient } from "@tanstack/react-query";
import { contentLabel, describeError, formatClock, queryKeys, type WatchedStream } from "@nuvora/core";
import { client } from "../nuvora";
import { useOpenStream } from "../lib/useOpenStream";
import { CardArt } from "./CardArt";
import { CloseIcon, PlayIcon, SpinIcon } from "./Icon";
import { PosterMarks } from "./PosterCard";

/**
 * A stream stopped partway, in Continue Watching: it starts the torrent
 * again and plays on from where it stopped. It has no artwork, so its title
 * card stands in, marked as a stream so it isn't taken for a library title.
 */
export function StreamCardView({ stream }: { stream: WatchedStream }) {
  const qc = useQueryClient();
  const open = useOpenStream();
  const forget = useMutation({
    mutationFn: () => client.admin.forgetStream(stream.link),
    onSettled: () => qc.invalidateQueries({ queryKey: queryKeys.watchedStreams }),
  });
  const movie = stream.content.kind === "movie";
  const second = movie ? (stream.year != null ? String(stream.year) : " ") : contentLabel(stream.content);
  return (
    <div className="stream-poster">
      <button
        type="button"
        className="poster-card"
        onClick={() => open.mutate({ link: stream.link, title: stream.title })}
        disabled={open.isPending}
        aria-busy={open.isPending}
        aria-label={`Play ${stream.movie}${movie ? "" : `, ${second}`} from ${formatClock(stream.position_ms)}, streamed`}
      >
        <span className="poster-frame">
          <CardArt image={null} title={stream.movie} year={movie ? stream.year : null} />
          <PosterMarks progress={stream.duration_ms > 0 ? stream.position_ms / stream.duration_ms : null} watched={false} />
          <span className="poster-stream" aria-hidden>
            Stream
          </span>
          <span className="poster-hover" aria-hidden>
            <span className="poster-hover-play">{open.isPending ? <SpinIcon size={22} className="poster-spin" /> : <PlayIcon size={22} />}</span>
          </span>
        </span>
        <span className="poster-caption">
          <span className="poster-title">{stream.movie}</span>
          <span className="poster-year">{second}</span>
        </span>
      </button>
      <button type="button" className="poster-remove" onClick={() => forget.mutate()} disabled={forget.isPending} aria-label={`Remove ${stream.movie} from Continue Watching`}>
        <CloseIcon size={14} />
      </button>
      {open.isError && (
        <p className="poster-error" role="alert">
          {describeError(open.error)}
        </p>
      )}
    </div>
  );
}
