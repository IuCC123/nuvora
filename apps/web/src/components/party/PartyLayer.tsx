import { useEffect, useRef, useState } from "react";
import { useRouterState } from "@tanstack/react-router";
import { partyLink, useParty } from "../../lib/party";
import { useUser } from "../../lib/session";
import { CloseIcon } from "../Icon";
import { usePartyRoute } from "./usePartyRoute";
import { InviteRow } from "./WatchTogether";
import "../../styles/party.css";

/**
 * Watch Together across the whole app, under every screen: keeps the live
 * connection open while someone is signed in, shows new invitations as
 * cards in the corner, and takes everyone in a party to the player when one
 * of them starts something.
 */
export function PartyLayer() {
  const user = useUser();
  const id = user?.id;
  useEffect(() => {
    if (id === undefined) return;
    partyLink.start();
    return () => partyLink.stop();
  }, [id]);
  useFollowStarts(id);
  return id === undefined ? null : <InviteCards />;
}

/** Someone in the party started a movie or episode: open the player on it, unless already there. */
function useFollowStarts(me: number | undefined) {
  const go = usePartyRoute();
  const followed = useRef<string | null>(null);
  useEffect(
    () =>
      partyLink.onEvent((event) => {
        if (me === undefined || event.type !== "party") return;
        const playback = event.party.playback;
        if (!playback || playback.action !== "start") return;
        const key = `${event.party.id}:${playback.seq}`;
        if (followed.current === key) return;
        followed.current = key;
        const here = new URL(window.location.href);
        const onIt =
          here.pathname === `/${playback.kind}/${playback.item_id}/play` && Number(here.searchParams.get("file")) === playback.file_id;
        if (!onIt) go(playback);
      }),
    [me, go],
  );
}

/** New invitations, newest on top, until answered or dismissed for now. */
function InviteCards() {
  const { invites } = useParty();
  const [dismissed, setDismissed] = useState<ReadonlySet<string>>(new Set());
  // Sign-in and first-run setup stay quiet.
  const path = useRouterState({ select: (s) => s.location.pathname });
  const shown = invites.filter((i) => !dismissed.has(`${i.party_id}:${i.sent_at_ms}`)).slice(-3).reverse();
  if (shown.length === 0 || path.startsWith("/signin") || path.startsWith("/welcome")) return null;
  return (
    <div className="together-cards" role="region" aria-label="Watch together invitations">
      {shown.map((invite) => (
        <div key={`${invite.party_id}:${invite.sent_at_ms}`} className="together-card" role="alert">
          <button
            type="button"
            className="icon-btn together-card-close"
            aria-label="Dismiss for now"
            onClick={() => setDismissed((d) => new Set(d).add(`${invite.party_id}:${invite.sent_at_ms}`))}
          >
            <CloseIcon size={16} />
          </button>
          <InviteRow invite={invite} />
        </div>
      ))}
    </div>
  );
}
