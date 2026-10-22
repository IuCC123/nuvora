import type React from "react";
import { useQuery } from "@tanstack/react-query";
import { Link, useParams } from "@tanstack/react-router";
import { findCard, type MovieDetail, type PosterCard, type Version } from "@nuvora/core";
import { queries, queryClient } from "../nuvora";
import { Artwork } from "../components/Artwork";

/**
 * Movie detail. First frame: title, year and poster placeholder from the home
 * card already in cache. Then the item response fills backdrop, runtime,
 * technical info, and one Play button per playable version.
 */
export function MovieScreen() {
  const { id } = useParams({ from: "/movie/$id" });
  const item = useQuery(queries.item(id));
  const card = findCard(queryClient.getQueryData(queries.home().queryKey), id);
  if (item.data?.kind === "movie") return <MovieView movie={item.data} />;
  return <MovieSkeleton card={card} failed={item.isError} />;
}

function MovieView({ movie }: { movie: MovieDetail }) {
  const primary = movie.versions[0];
  return (
    <article aria-labelledby="movie-title">
      {movie.backdrop && <Artwork image={movie.backdrop} size="large" alt="" />}
      {movie.poster && <Artwork image={movie.poster} size="large" alt={`${movie.title} poster`} />}
      <h1 id="movie-title">{movie.title}</h1>
      {/* TODO: year · runtime (h m) · primary.summary; overview; tracks table from primary.media. */}
      {primary ? <PlayButton movieId={movie.id} version={primary} /> : <p>All files for this movie are missing.</p>}
    </article>
  );
}

/** Play when the server said direct play on this device; otherwise say why, never a broken player. */
function PlayButton({ movieId, version }: { movieId: number; version: Version }) {
  const p = version.playback;
  switch (p.kind) {
    case "direct_play":
      return (
        <Link to="/movie/$id/play" params={{ id: movieId }} search={{ file: version.file_id }} autoFocus>
          Play
        </Link>
      );
    case "needs_transcode":
      return <p role="status">Needs transcoding, coming in Step 3: {p.reasons.join("; ")}</p>;
    case "unavailable":
      return <p role="status">This file can't be read: {p.reason}</p>;
    default:
      // A newer server added a playback kind this build does not know.
      return <p role="status">Needs transcoding, coming in Step 3.</p>;
  }
}

function MovieSkeleton({ card, failed }: { card: PosterCard | undefined; failed: boolean }): React.ReactNode {
  void card;
  void failed;
  throw new Error("not implemented: card title/poster placeholder, or 'Movie not found' on 404");
}
