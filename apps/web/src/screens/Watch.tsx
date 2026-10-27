import { useCallback, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useCanGoBack, useNavigate, useParams, useRouter } from "@tanstack/react-router";
import { ApiError, audioLanguage, formatBytes, type SubtitleTrack, type TorrentStream, type Version } from "@nuvora/core";
import type { NoticeText } from "../components/Notice";
import { playbackMode } from "../components/player/PlaybackMode";
import { playerFailed } from "../lib/playState";
import { useDocumentTitle } from "../lib/useDocumentTitle";
import { client, queries, servedClient } from "../nuvora";
import { PlayerView, sourceFor } from "./Player";

const POLL_STARTING_MS = 1000;
/** Once it plays, asking is a heartbeat: a stream nobody asks about for two minutes stops. */
const POLL_PLAYING_MS = 10_000;
const NO_TRACKS: SubtitleTrack[] = [];
const nothing = () => {};

/**
 * A release playing while it downloads: the player, with what it waits for
 * until the torrent's details and the start of the video are in. Back stops
 * the stream and deletes what it fetched.
 */
export function Watch() {
  const { id } = useParams({ from: "/watch/$id" });
  const navigate = useNavigate();
  const router = useRouter();
  const canGoBack = useCanGoBack();
  const status = useQuery({
    ...queries.stream(id),
    retry: false,
    refetchInterval: (q) => (q.state.error || q.state.data?.state === "failed" ? false : q.state.data?.state === "ready" ? POLL_PLAYING_MS : POLL_STARTING_MS),
  });
  const stream = status.data;
  // The server's converted stream, for a file this browser was told it could play as it is but couldn't.
  const [served, setServed] = useState<Version["playback"] | null>(null);
  const [failed, setFailed] = useState(false);
  const name = stream?.movie || stream?.title || undefined;
  useDocumentTitle(name ?? "Starting");

  const back = useCallback(() => {
    void client.admin.stopStream(id).catch(nothing);
    if (canGoBack) router.history.back();
    else void navigate({ to: "/downloads", replace: true });
  }, [id, canGoBack, router, navigate]);

  const fallBack = useCallback(() => {
    if (stream?.playback?.kind !== "direct_play" || served) return setFailed(true);
    servedClient.admin.stream(id).then(
      (s) => (s.playback?.kind === "hls" ? setServed(s.playback) : setFailed(true)),
      () => setFailed(true),
    );
  }, [id, stream, served]);

  const notice = noticeFor(status.error, stream, failed);
  const playback = notice ? undefined : (served ?? stream?.playback ?? undefined);
  const source = sourceFor(playback);
  return (
    <PlayerView
      key={served ? "server" : "file"}
      catalog={null}
      title={name}
      year={stream?.year}
      episode={undefined}
      onNext={nothing}
      source={source}
      mode={playback ? playbackMode(playback, source?.kind === "rewrap") : undefined}
      startAt={undefined}
      subtitleTracks={NO_TRACKS}
      audioLanguage={stream?.media ? audioLanguage(stream.media.audio) : null}
      subtitleScope="stream"
      notice={notice}
      waiting={waitingFor(stream)}
      streamNote={downloadNote(stream)}
      onBack={back}
      onFailed={fallBack}
      onPosition={nothing}
    />
  );
}

function noticeFor(error: unknown, stream: TorrentStream | undefined, failed: boolean): NoticeText | undefined {
  if (error instanceof ApiError && error.status === 404) {
    return { problem: ["This stream has ended."], hint: "A stream stops a couple of minutes after nobody watches it. Go back and press Play again." };
  }
  if (error) return { problem: ["Can't reach your Nuvora server."], hint: "Check that it's running, then go back and try again." };
  if (stream?.state === "failed") return { problem: [stream.error ?? "This release won't play."], hint: "Go back and try another release." };
  if (stream?.playback?.kind === "needs_transcode") return { problem: ["This video can't play here.", ...stream.playback.reasons], hint: "Go back and try another release." };
  if (failed) return playerFailed;
  return undefined;
}

function waitingFor(stream: TorrentStream | undefined): string | null {
  if (!stream || stream.state === "starting") return "Getting the torrent's details";
  if (stream.state === "probing") return "Reading the start of the video";
  return null;
}

function downloadNote(stream: TorrentStream | undefined): string | null {
  if (!stream || stream.state === "failed") return null;
  if (stream.peers === 0) return "Looking for peers";
  const parts = [`${stream.peers} ${stream.peers === 1 ? "peer" : "peers"}`, `${formatBytes(stream.down_bps)}/s`];
  if (stream.done_bytes != null && stream.size_bytes) parts.push(`${Math.floor((stream.done_bytes / stream.size_bytes) * 100)}% downloaded`);
  return parts.join(" · ");
}
