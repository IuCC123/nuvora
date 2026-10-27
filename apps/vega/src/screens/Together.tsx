import React, { useState } from 'react';
import { ScrollView, StyleSheet, Text, View } from 'react-native';
import { ApiError, describeError, partyPosition, type NuvoraClient, type PartyInvite, type PartyPerson, type PartyView } from '@nuvora/core';
import { Icon } from '../icons';
import { useLive, type LiveLink } from '../live';
import { useLoad } from '../load';
import { useNav, type Nav } from '../nav';
import { useSignedIn } from '../state';
import { colors, gutter, radius, type } from '../theme';
import { Avatar, Button, FocusGuide, Focusable, Gradient } from '../ui';
import { Shell } from './Shell';

/** Opens the player on what the party is playing, at the party's position; replaces a player that's open. */
export function followParty(nav: Nav, link: LiveLink, party: PartyView, playerOnTop: boolean) {
  const playback = party.playback;
  if (!playback) return;
  const route = { name: 'player' as const, itemId: playback.item_id, fileId: playback.file_id, startMs: Math.max(0, Math.round(partyPosition(playback, link.serverNow()) * 1000)) };
  if (playerOnTop) nav.replace(route);
  else nav.push(route);
}

/** Joins a party; then to what it's watching, or to its page when nothing is on yet. Null, or what went wrong. */
export async function join(nav: Nav, client: NuvoraClient, link: LiveLink, invite: PartyInvite, playerOnTop: boolean): Promise<string | null> {
  try {
    const party = await client.party.join(invite.party_id);
    link.setParty(party);
    link.setInvites(link.getSnapshot().invites.filter((i) => i.party_id !== invite.party_id));
    if (party.playback) followParty(nav, link, party, playerOnTop);
    else nav.section({ name: 'together' });
    return null;
  } catch (e) {
    link.setInvites(link.getSnapshot().invites.filter((i) => i.party_id !== invite.party_id));
    return e instanceof ApiError && e.status === 404 ? 'That invitation has ended.' : describeError(e);
  }
}

export async function decline(client: NuvoraClient, link: LiveLink, invite: PartyInvite) {
  await client.party.decline(invite.party_id).catch(() => undefined);
  link.setInvites(link.getSnapshot().invites.filter((i) => i.party_id !== invite.party_id));
}

/** Watch Together: invitations to answer, who's in the group and what it's watching, and who else can be invited. */
export function Together() {
  const { client, live, session } = useSignedIn();
  const nav = useNav();
  const snapshot = useLive(live);
  const party = snapshot.party;
  const [people] = useLoad('people', () => client.party.people());
  const [problem, setProblem] = useState<string | null>(null);
  const me = session.user.id;

  const act = (work: () => Promise<unknown>) => {
    setProblem(null);
    work().catch((e: unknown) => setProblem(describeError(e)));
  };

  const inGroup = new Set([...(party?.members.map((m) => m.id) ?? []), ...(party?.invited.map((p) => p.id) ?? [])]);
  const invitable = people.kind === 'ready' ? people.value.filter((p) => p.id !== me && !inGroup.has(p.id)) : [];
  const firstInvites = snapshot.invites.length > 0;

  return (
    <Shell section="together">
      <View style={styles.page}>
        <View style={styles.headingRow}>
          <Text style={styles.heading}>Watch together</Text>
          {!snapshot.connected && <Text style={styles.reconnecting}>Reconnecting…</Text>}
        </View>
        <Text style={styles.lead}>
          {party
            ? 'Press Play on any movie or episode and it starts for everyone in the group.'
            : 'Invite someone, then press Play on anything. It starts for everyone at the same moment, and pausing or skipping stays in sync.'}
        </Text>
        {problem && <Text style={styles.problem}>{problem}</Text>}
        <View style={styles.columns}>
          <FocusGuide autoFocus style={styles.column}>
            <ScrollView contentContainerStyle={styles.columnList}>
              {firstInvites && <Text style={styles.subheading}>Invitations</Text>}
              {snapshot.invites.map((invite, n) => (
                <InviteCard
                  key={`${invite.party_id}:${invite.sent_at_ms}`}
                  invite={invite}
                  preferred={n === 0}
                  onJoin={() => act(async () => setProblem(await join(nav, client, live, invite, false)))}
                  onDecline={() => act(() => decline(client, live, invite))}
                />
              ))}
              {firstInvites && <View style={styles.gap18} />}
              {party && (
                <>
                  <NowPlaying party={party} preferred={!firstInvites} onWatch={() => followParty(nav, live, party, false)} />
                  <Text style={styles.subheading}>In this group</Text>
                  {party.members.map((member) => (
                    <PersonLine
                      key={`m${member.id}`}
                      name={member.display_name}
                      you={member.id === me}
                      status={member.id === party.host_id ? 'Host' : member.online ? 'Here' : 'Away'}
                    />
                  ))}
                  {party.invited.map((person) => (
                    <PersonButton key={`i${person.id}`} person={person} action="Invited · cancel" on onPress={() => act(async () => live.setParty(await client.party.cancelInvite(person.id)))} />
                  ))}
                  <View style={styles.leave}>
                    <Button
                      label="Leave group"
                      icon="signOut"
                      preferred={!firstInvites && !party.playback && party.invited.length === 0}
                      onPress={() =>
                        act(async () => {
                          await client.party.leave();
                          live.setParty(null);
                        })
                      }
                    />
                  </View>
                </>
              )}
            </ScrollView>
          </FocusGuide>
          <FocusGuide autoFocus style={styles.column}>
            <ScrollView contentContainerStyle={styles.columnList}>
              <Text style={styles.subheading}>{party ? 'Invite more' : 'Invite'}</Text>
              {people.kind === 'loading' && <Text style={styles.hint}>Loading…</Text>}
              {people.kind === 'failed' && <Text style={styles.hint}>Couldn't load who's on this server.</Text>}
              {people.kind === 'ready' && invitable.length === 0 && (
                <Text style={styles.hint}>
                  {people.value.some((p) => p.id !== me) ? "Everyone's in already." : 'No one else has an account yet. An admin can add people in Settings › Users, in a browser.'}
                </Text>
              )}
              {invitable.map((person, n) => (
                <PersonButton
                  key={person.id}
                  person={person}
                  action="Invite"
                  preferred={!party && !firstInvites && n === 0}
                  onPress={() => act(async () => live.setParty(await client.party.invite(person.id)))}
                />
              ))}
            </ScrollView>
          </FocusGuide>
        </View>
      </View>
    </Shell>
  );
}

function PersonLine({ name, you, status }: { name: string; you: boolean; status: string }) {
  const away = status === 'Away';
  return (
    <View style={styles.personLine}>
      <Avatar name={name} dim={away} />
      <Text style={styles.personName} numberOfLines={1}>
        {you ? `${name} (you)` : name}
      </Text>
      <Text style={[styles.personStatus, away && { color: colors.muted }]}>{status}</Text>
    </View>
  );
}

function PersonButton({ person, action, on, preferred, onPress }: { person: PartyPerson; action: string; on?: boolean; preferred?: boolean; onPress: () => void }) {
  return (
    <Focusable onPress={onPress} preferred={preferred} scale={1.02} style={(focused) => [styles.personButton, focused && { backgroundColor: colors.text }]}>
      {(focused) => {
        const color = focused ? colors.canvas : colors.text;
        return (
          <>
            <Avatar name={person.display_name} />
            <Text style={[styles.personName, { color }]} numberOfLines={1}>
              {person.display_name}
            </Text>
            {on && <Icon name="check" size={18} color={color} />}
            <Text style={[styles.personAction, { color }]}>{action}</Text>
          </>
        );
      }}
    </Focusable>
  );
}

/** What the group is watching, with a way back to it. */
function NowPlaying({ party, preferred, onWatch }: { party: PartyView; preferred: boolean; onWatch: () => void }) {
  const playback = party.playback;
  return (
    <View style={styles.card}>
      {!playback ? (
        <>
          <Text style={styles.kicker}>NOTHING PLAYING YET</Text>
          <Text style={styles.cardText}>Press Play on any movie or episode and everyone starts together.</Text>
        </>
      ) : (
        <>
          <Text style={styles.kicker}>{playback.playing ? 'PLAYING NOW' : 'PAUSED'}</Text>
          <Text style={styles.cardTitle} numberOfLines={1}>
            {playback.title}
          </Text>
          {playback.subtitle ? <Text style={styles.cardSub}>{playback.subtitle}</Text> : null}
          <View style={styles.cardButtons}>
            <Button label="Watch" icon="play" primary preferred={preferred} onPress={onWatch} />
          </View>
        </>
      )}
    </View>
  );
}

function InviteCard({ invite, preferred, onJoin, onDecline }: { invite: PartyInvite; preferred: boolean; onJoin: () => void; onDecline: () => void }) {
  const others = invite.members.filter((m) => m.id !== invite.from.id).map((m) => m.display_name);
  return (
    <View style={[styles.card, styles.inviteCard]}>
      <Avatar name={invite.from.display_name} size={52} />
      <View style={styles.inviteBody}>
        <Text style={styles.cardTitleSmall}>
          {invite.from.display_name} wants to watch together{others.length ? ` with ${others.join(', ')}` : ''}
        </Text>
        <View style={styles.cardButtons}>
          <Button label="Join" icon="users" primary preferred={preferred} onPress={onJoin} />
          <Button label="Not now" onPress={onDecline} />
        </View>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  page: { flex: 1, paddingHorizontal: gutter, paddingTop: 36 },
  headingRow: { flexDirection: 'row', alignItems: 'center' },
  heading: { ...type.headlineMedium, color: colors.text },
  reconnecting: { ...type.titleSmall, color: colors.muted, marginLeft: 16 },
  lead: { ...type.bodyLarge, color: colors.secondary, marginTop: 8, maxWidth: 820 },
  problem: { ...type.titleSmall, color: colors.danger, marginTop: 8 },
  columns: { flex: 1, flexDirection: 'row', gap: 56, marginTop: 28 },
  column: { flex: 1 },
  columnList: { gap: 10, paddingBottom: 48 },
  subheading: { ...type.titleLarge, color: colors.text, marginBottom: 4 },
  hint: { ...type.bodyMedium, color: colors.muted },
  gap18: { height: 18 },
  leave: { flexDirection: 'row', marginTop: 14 },
  personLine: { flexDirection: 'row', alignItems: 'center', paddingHorizontal: 16, paddingVertical: 6, gap: 16 },
  personName: { ...type.titleMedium, color: colors.text, flex: 1 },
  personStatus: { ...type.labelLarge, color: colors.secondary },
  personButton: { flexDirection: 'row', alignItems: 'center', gap: 16, paddingHorizontal: 16, paddingVertical: 10, borderRadius: radius.lg + 2, backgroundColor: colors.surface },
  personAction: { ...type.labelLarge },
  card: { borderRadius: radius.xl, backgroundColor: colors.surface, padding: 20, marginBottom: 18 },
  kicker: { ...type.labelSmall, color: colors.accent, marginBottom: 6 },
  cardText: { ...type.bodyLarge, color: colors.secondary },
  cardTitle: { ...type.titleLarge, color: colors.text },
  cardTitleSmall: { ...type.titleMedium, color: colors.text },
  cardSub: { ...type.bodyMedium, color: colors.secondary },
  cardButtons: { flexDirection: 'row', gap: 12, marginTop: 14 },
  inviteCard: { flexDirection: 'row', alignItems: 'center', padding: 18, marginBottom: 0 },
  inviteBody: { flex: 1, marginLeft: 18 },
});

/**
 * A new invitation, wherever the person is in the app: a card in the corner
 * that takes focus, so OK joins. Back leaves it for later, on the Watch
 * together page.
 */
export function InviteDialog({ invite, playerOnTop, onClose }: { invite: PartyInvite; playerOnTop: boolean; onClose: () => void }) {
  const { client, live } = useSignedIn();
  const nav = useNav();
  const [problem, setProblem] = useState<string | null>(null);
  const others = invite.members.filter((m) => m.id !== invite.from.id).map((m) => m.display_name);
  return (
    <View style={dialog.screen}>
      <Gradient direction="right" stops={[[0.4, '#000', 0], [1, '#000', 0.6]]} />
      <FocusGuide autoFocus trapFocusUp trapFocusDown trapFocusLeft trapFocusRight style={dialog.card}>
        <View style={dialog.head}>
          <Avatar name={invite.from.display_name} size={52} />
          <View style={dialog.headText}>
            <Text style={dialog.kicker}>WATCH TOGETHER</Text>
            <Text style={dialog.title}>
              {invite.from.display_name} invited you{others.length ? `, with ${others.join(', ')}` : ''}
            </Text>
          </View>
        </View>
        {problem && <Text style={dialog.problem}>{problem}</Text>}
        <View style={dialog.buttons}>
          <Button
            label="Join"
            icon="users"
            primary
            preferred
            onPress={() =>
              void join(nav, client, live, invite, playerOnTop).then((failed) => {
                if (failed) setProblem(failed);
                else onClose();
              })
            }
          />
          <Button label="Not now" onPress={() => void decline(client, live, invite).then(onClose)} />
          <Button label="Later" onPress={onClose} />
        </View>
      </FocusGuide>
    </View>
  );
}

const dialog = StyleSheet.create({
  screen: { ...StyleSheet.absoluteFillObject, alignItems: 'flex-end' },
  card: { margin: 40, minWidth: 420, maxWidth: 560, borderRadius: 16, borderWidth: 1, borderColor: colors.hairline, backgroundColor: colors.surface, padding: 24 },
  head: { flexDirection: 'row', alignItems: 'center' },
  headText: { marginLeft: 18, flexShrink: 1 },
  kicker: { ...type.labelSmall, color: colors.accent, marginBottom: 4 },
  title: { ...type.titleMedium, color: colors.text },
  problem: { ...type.bodyMedium, color: colors.danger, marginTop: 10 },
  buttons: { flexDirection: 'row', gap: 12, marginTop: 20 },
});
