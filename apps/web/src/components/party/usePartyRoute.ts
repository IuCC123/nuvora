import { useCallback } from "react";
import { useNavigate } from "@tanstack/react-router";
import { partyPosition, type PartyPlayback } from "@nuvora/core";
import { partyLink } from "../../lib/party";

/** Opens the player on what the party is playing, at the party's position. */
export function usePartyRoute() {
  const navigate = useNavigate();
  return useCallback(
    (playback: PartyPlayback) => {
      const t = Math.floor(partyPosition(playback, partyLink.serverNow()));
      const search = { file: playback.file_id, t: t > 0 ? t : undefined };
      if (playback.kind === "movie") void navigate({ to: "/movie/$id/play", params: { id: playback.item_id }, search });
      else void navigate({ to: "/episode/$id/play", params: { id: playback.item_id }, search });
    },
    [navigate],
  );
}
