import type { Version } from "@nuvora/core";
import type { NoticeText } from "../components/Notice";

/** What the Play button does for a movie: the server decided, the view only renders it. */
export type PlayState = { kind: "play"; fileId: number } | { kind: "blocked"; notice: NoticeText };

const CANT_PLAY = "This browser can't play this file yet.";
const CONVERT_HINT = "It will play once Nuvora can convert files on the fly.";

export const playerFailed: NoticeText = { problem: ["This browser couldn't play this file."], hint: CONVERT_HINT };

/** The link named a file the movie no longer has: replaced, or removed by a scan. */
export const fileGone: NoticeText = { problem: ["This file is no longer part of the movie."], hint: "Go back to the movie and play it from there." };

export function playState(versions: readonly Version[]): PlayState {
  const version = versions[0];
  if (!version) {
    return {
      kind: "blocked",
      notice: { problem: ["This movie's files are missing."], hint: "Reconnect the drive or folder it was on, then scan again." },
    };
  }
  const p = version.playback;
  switch (p.kind) {
    case "direct_play":
      return { kind: "play", fileId: version.file_id };
    case "needs_transcode":
      return { kind: "blocked", notice: { problem: p.reasons.length ? p.reasons.map(sentence) : [CANT_PLAY], hint: CONVERT_HINT } };
    case "unavailable":
      return { kind: "blocked", notice: { problem: ["Nuvora can't read this file.", sentence(p.reason)], hint: "Replace the file, then scan again." } };
    default:
      // A newer server added a playback kind this build does not know: treat it like needs_transcode.
      return { kind: "blocked", notice: { problem: [CANT_PLAY], hint: CONVERT_HINT } };
  }
}

function sentence(text: string): string {
  const t = text.trim();
  return /[.!?]$/.test(t) ? t : `${t}.`;
}
