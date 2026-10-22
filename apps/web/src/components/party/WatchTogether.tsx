import { useEffect, useId, useRef, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useNavigate, useRouterState } from "@tanstack/react-router";
import { initials, type PartyInvite, type PartyPerson, type PartyView } from "@nuvora/core";
import { client } from "../../nuvora";
import { partyLink, useParty } from "../../lib/party";
import { useUser } from "../../lib/session";
import { CloseIcon, PlayIcon, UsersIcon } from "../Icon";
import { usePartyRoute } from "./usePartyRoute";
import "../../styles/party.css";

/**
 * Watch Together in the top bar, next to the account button: who's in the
 * group, invitations both ways, and who else can be invited. In a group, the
 * button shows its members; with an invitation waiting, a count.
 */
export function WatchTogether() {
  const me = useUser();
  const { party, invites, connected } = useParty();
  const [open, setOpen] = useState(false);
  const button = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const panelId = useId();

  useEffect(() => {
    if (!open) return;
    const outside = (e: PointerEvent) => {
      const target = e.target as Node;
      if (!panel.current?.contains(target) && !button.current?.contains(target)) setOpen(false);
    };
    const escape = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      setOpen(false);
      button.current?.focus();
    };
    document.addEventListener("pointerdown", outside);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("pointerdown", outside);
      document.removeEventListener("keydown", escape);
    };
  }, [open]);

  if (!me) return null;
  const others = party?.members.filter((m) => m.id !== me.id) ?? [];
  const label = party
    ? `Watching together with ${others.map((m) => m.display_name).join(", ") || "no one yet"}`
    : invites.length
      ? `Watch together: ${invites.length} invitation${invites.length === 1 ? "" : "s"}`
      : "Watch together";

  return (
    <div className="together">
      <button
        ref={button}
        type="button"
        className={party ? "together-button together-button-on" : "together-button"}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls={open ? panelId : undefined}
        aria-label={label}
        title={label}
        onClick={() => setOpen((o) => !o)}
      >
        {party && others.length > 0 ? (
          <span className="together-faces" aria-hidden>
            {others.slice(0, 3).map((m) => (
              <span key={m.id} className="together-face" data-away={!m.online}>
                {initials(m.display_name)}
              </span>
            ))}
          </span>
        ) : (
          <UsersIcon size={20} />
        )}
        {party && <span className="together-label">Together</span>}
        {invites.length > 0 && <span className="together-badge">{invites.length}</span>}
      </button>
      {open && (
        <div ref={panel} id={panelId} className="together-panel" role="dialog" aria-label="Watch together">
          <Panel me={me.id} party={party} invites={invites} connected={connected} onClose={() => setOpen(false)} />
        </div>
      )}
    </div>
  );
}

function Panel({ me, party, invites, connected, onClose }: { me: number; party: PartyView | null; invites: PartyInvite[]; connected: boolean; onClose: () => void }) {
  const people = useQuery({ queryKey: ["party", "people"], queryFn: ({ signal }) => client.party.people(signal), staleTime: 60_000 });
  const invite = useMutation({ mutationFn: (id: number) => client.party.invite(id), onSuccess: (view) => partyLink.setParty(view) });
  const cancel = useMutation({ mutationFn: (id: number) => client.party.cancelInvite(id), onSuccess: (view) => partyLink.setParty(view) });
  const leave = useMutation({ mutationFn: () => client.party.leave(), onSuccess: () => partyLink.setParty(null) });
  const inGroup = new Set([...(party?.members.map((m) => m.id) ?? []), ...(party?.invited.map((p) => p.id) ?? [])]);
  const invitable = (people.data ?? []).filter((p) => !inGroup.has(p.id));

  return (
    <>
      <div className="together-head">
        <h2>Watch together</h2>
        {!connected && <span className="together-offline">Reconnecting…</span>}
      </div>

      {invites.length > 0 && (
        <section className="together-section" aria-label="Invitations">
          {invites.map((i) => (
            <InviteRow key={i.party_id} invite={i} onDone={onClose} />
          ))}
        </section>
      )}

      {party ? (
        <>
          <NowPlaying party={party} onGo={onClose} />
          <section className="together-section" aria-labelledby="together-members">
            <h3 id="together-members">In this group</h3>
            <ul className="together-list">
              {party.members.map((m) => (
                <li key={m.id} className="together-person">
                  <span className="avatar" data-away={!m.online} aria-hidden>
                    {initials(m.display_name)}
                  </span>
                  <span className="together-name">
                    {m.display_name}
                    {m.id === me && <span className="together-note"> (you)</span>}
                  </span>
                  <span className="together-status">{m.id === party.host_id ? "Host" : m.online ? "Here" : "Away"}</span>
                </li>
              ))}
              {party.invited.map((p) => (
                <li key={p.id} className="together-person together-person-invited">
                  <span className="avatar" aria-hidden>
                    {initials(p.display_name)}
                  </span>
                  <span className="together-name">{p.display_name}</span>
                  <span className="together-status">Invited</span>
                  <button type="button" className="icon-btn together-x" aria-label={`Cancel ${p.display_name}'s invitation`} onClick={() => cancel.mutate(p.id)}>
                    <CloseIcon size={16} />
                  </button>
                </li>
              ))}
            </ul>
          </section>
        </>
      ) : (
        <p className="together-intro">
          Invite someone, then press Play on anything. It starts for everyone at the same moment, and pausing or skipping stays in sync.
        </p>
      )}

      <section className="together-section" aria-labelledby="together-invite">
        <h3 id="together-invite">{party ? "Invite more" : "Invite"}</h3>
        {people.isPending ? (
          <p className="together-hint">Loading…</p>
        ) : people.isError ? (
          <p className="together-hint">Couldn't load who's on this server.</p>
        ) : invitable.length === 0 ? (
          <p className="together-hint">{people.data?.length ? "Everyone's in already." : "No one else has an account yet. An admin can add people in Settings › Users."}</p>
        ) : (
          <ul className="together-list">
            {invitable.map((p) => (
              <PersonRow key={p.id} person={p} pending={invite.isPending && invite.variables === p.id} onInvite={() => invite.mutate(p.id)} />
            ))}
          </ul>
        )}
        {invite.isError && (
          <p className="together-error" role="alert">
            Couldn't send the invitation. Try again.
          </p>
        )}
      </section>

      {party && (
        <div className="together-foot">
          <button type="button" className="btn btn-secondary together-leave" onClick={() => leave.mutate()} disabled={leave.isPending}>
            Leave group
          </button>
        </div>
      )}
    </>
  );
}

function PersonRow({ person, pending, onInvite }: { person: PartyPerson; pending: boolean; onInvite: () => void }) {
  return (
    <li className="together-person">
      <span className="avatar" aria-hidden>
        {initials(person.display_name)}
      </span>
      <span className="together-name">{person.display_name}</span>
      <button type="button" className="btn btn-secondary together-invite" onClick={onInvite} disabled={pending} aria-busy={pending}>
        Invite
      </button>
    </li>
  );
}

/** What the group is watching, with a way back to it from anywhere else. */
function NowPlaying({ party, onGo }: { party: PartyView; onGo: () => void }) {
  const playback = party.playback;
  const go = usePartyRoute();
  const path = useRouterState({ select: (s) => s.location.pathname });
  if (!playback) {
    return <p className="together-intro">Nothing's playing yet. Press Play on any movie or episode and everyone starts together.</p>;
  }
  const watching = path === `/${playback.kind}/${playback.item_id}/play`;
  return (
    <div className="together-now">
      <div className="together-now-text">
        <span className="together-now-label">{playback.playing ? "Playing now" : "Paused"}</span>
        <span className="together-now-title">
          {playback.title}
          {playback.subtitle && <span className="together-note"> · {playback.subtitle}</span>}
        </span>
      </div>
      {!watching && (
        <button
          type="button"
          className="btn btn-primary together-join-now"
          onClick={() => {
            go(playback);
            onGo();
          }}
        >
          <PlayIcon size={16} />
          Watch
        </button>
      )}
    </div>
  );
}

/** An invitation in the panel: join (and go to what's playing), or decline. */
export function InviteRow({ invite, onDone }: { invite: PartyInvite; onDone?: () => void }) {
  const accept = useJoin();
  const decline = useMutation({
    mutationFn: () => client.party.decline(invite.party_id),
    onSuccess: () => partyLink.setInvites(partyLink.getSnapshot().invites.filter((i) => i.party_id !== invite.party_id)),
  });
  const with_ = invite.members.filter((m) => m.id !== invite.from.id).map((m) => m.display_name);
  return (
    <div className="together-invitation">
      <span className="avatar" aria-hidden>
        {initials(invite.from.display_name)}
      </span>
      <div className="together-invitation-text">
        <p>
          <strong>{invite.from.display_name}</strong> wants to watch together
          {with_.length > 0 && <> with {with_.join(", ")}</>}
        </p>
        <div className="together-invitation-actions">
          <button
            type="button"
            className="btn btn-primary"
            disabled={accept.isPending}
            onClick={() => accept.mutate(invite.party_id, { onSuccess: () => onDone?.() })}
          >
            Join
          </button>
          <button type="button" className="btn btn-secondary" disabled={decline.isPending} onClick={() => decline.mutate()}>
            Not now
          </button>
        </div>
        {accept.isError && (
          <p className="together-error" role="alert">
            That invitation has ended.
          </p>
        )}
      </div>
    </div>
  );
}

/** Joins a party; if it's already watching something, opens the player there. */
export function useJoin() {
  const go = usePartyRoute();
  const navigate = useNavigate();
  return useMutation({
    mutationFn: (partyId: string) => client.party.join(partyId),
    onSuccess: (view, partyId) => {
      partyLink.setParty(view);
      partyLink.setInvites(partyLink.getSnapshot().invites.filter((i) => i.party_id !== partyId));
      if (view.playback) go(view.playback);
      else void navigate({ to: "/" });
    },
  });
}
