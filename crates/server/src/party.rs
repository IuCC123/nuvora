//! Watch Together: small groups of users who play the same thing at the
//! same moment. In memory only; a party lives as long as someone is in it.
//!
//! A user is in at most one party. Anyone in it can invite others, start a
//! movie or episode, play, pause and seek; every change goes to every
//! member's live connections (`/api/live`). Playback is one shared clock:
//! "at server time `at_ms` the position is `position_ms`, playing or
//! paused". A start or resume is set slightly in the future, so everyone's
//! player gets the message before the moment it describes and they begin
//! together. Clients correct their own drift against that clock.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use utoipa::ToSchema;

/// Lead before a new movie or episode starts for everyone: time to open the player.
const START_LEAD_MS: i64 = 2_500;
/// Lead before a play or seek takes effect: time for the message to arrive.
const RESUME_LEAD_MS: i64 = 600;
/// A member with no live connection this long is taken out of the party.
const OFFLINE_GRACE: Duration = Duration::from_secs(300);
/// An invitation nobody answers lapses after this long.
const INVITE_TTL_MS: i64 = 30 * 60 * 1000;

/// What a signed-in user sees: their party, if any, and invitations to others.
#[derive(Serialize, ToSchema, Clone, Debug, PartialEq)]
pub struct PartyStatus {
    pub party: Option<PartyView>,
    pub invites: Vec<PartyInvite>,
}

#[derive(Serialize, ToSchema, Clone, Debug, PartialEq)]
pub struct PartyView {
    pub id: String,
    pub host_id: i64,
    /// In joining order.
    pub members: Vec<PartyMember>,
    /// Invited and not answered yet.
    pub invited: Vec<PartyPerson>,
    /// `None` until someone starts something.
    pub playback: Option<PartyPlayback>,
}

#[derive(Serialize, ToSchema, Clone, Debug, PartialEq)]
pub struct PartyMember {
    pub id: i64,
    pub display_name: String,
    /// Has the app open right now.
    pub online: bool,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug, PartialEq)]
pub struct PartyPerson {
    pub id: i64,
    pub display_name: String,
}

#[derive(Serialize, ToSchema, Clone, Debug, PartialEq)]
pub struct PartyInvite {
    pub party_id: String,
    pub from: PartyPerson,
    /// Everyone already in the party, inviter included.
    pub members: Vec<PartyPerson>,
    /// Unix ms.
    pub sent_at_ms: i64,
}

/// The party's shared clock: at server time `at_ms` the position is
/// `position_ms`; while `playing` it advances in real time from there.
/// `at_ms` can be a moment ahead, when everyone starts together.
#[derive(Serialize, ToSchema, Clone, Debug, PartialEq)]
pub struct PartyPlayback {
    pub item_id: i64,
    pub file_id: i64,
    pub kind: PartyMediaKind,
    /// The movie's or the show's title.
    pub title: String,
    /// For an episode: `"S2 · E3"`.
    pub subtitle: Option<String>,
    pub playing: bool,
    pub position_ms: u64,
    pub at_ms: i64,
    /// Goes up with every change, so a client applies each one once.
    pub seq: u64,
    /// Who made the change; `None` when the server did.
    pub by: Option<PartyPerson>,
    pub action: PartyAction,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PartyMediaKind {
    Movie,
    Episode,
}

#[derive(Serialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PartyAction {
    Start,
    Play,
    Pause,
    Seek,
}

/// What the server sends on `/api/live`, as JSON text frames.
#[derive(Serialize, ToSchema, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LiveEvent {
    /// First frame of every connection.
    Hello {
        server_time_ms: i64,
        status: PartyStatus,
    },
    /// The answer to a `ping`, for measuring clock offset.
    Pong {
        client_time_ms: f64,
        server_time_ms: i64,
    },
    /// The party you are in changed (members, invitations or playback).
    Party { party: PartyView },
    /// You are no longer in `party_id`.
    PartyLeft { party_id: String },
    /// Your pending invitations changed; the whole list.
    Invites { invites: Vec<PartyInvite> },
}

/// What a client sends on `/api/live`, as JSON text frames.
#[derive(Deserialize, ToSchema, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LiveCommand {
    Ping {
        client_time_ms: f64,
    },
    /// Play this for the whole party, from `position_ms`.
    Start {
        item_id: i64,
        file_id: i64,
        position_ms: u64,
    },
    Play {
        position_ms: u64,
    },
    Pause {
        position_ms: u64,
    },
    Seek {
        position_ms: u64,
    },
}

/// A movie or episode resolved for [`Parties::start`].
pub struct Media {
    pub item_id: i64,
    pub file_id: i64,
    pub kind: PartyMediaKind,
    pub title: String,
    pub subtitle: Option<String>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum PartyError {
    #[error("You can't invite yourself")]
    SelfInvite,
    #[error("That invitation has lapsed")]
    NoInvite,
    #[error("You're not watching together with anyone")]
    NotInParty,
}

pub struct Parties {
    state: Mutex<State>,
    conns: Mutex<HashMap<i64, Vec<Conn>>>,
    next: AtomicU64,
}

#[derive(Default)]
struct State {
    parties: HashMap<String, Party>,
    member_of: HashMap<i64, String>,
}

struct Party {
    id: String,
    host: i64,
    members: Vec<Member>,
    invited: Vec<Invite>,
    playback: Option<PartyPlayback>,
    seq: u64,
}

struct Member {
    id: i64,
    name: String,
    offline_since: Option<Instant>,
}

struct Invite {
    to: PartyPerson,
    from: PartyPerson,
    at_ms: i64,
}

/// One live connection: its id and where its frames go.
type Conn = (u64, mpsc::UnboundedSender<String>);

/// Who to tell what, worked out under the lock and sent after it.
type Outbox = Vec<(i64, LiveEvent)>;

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

impl Default for Parties {
    fn default() -> Self {
        Parties {
            state: Mutex::new(State::default()),
            conns: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
        }
    }
}

impl Parties {
    /// Registers a live connection; its frames arrive on the receiver.
    pub fn connect(&self, user: i64) -> (u64, mpsc::UnboundedReceiver<String>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.conns
            .lock()
            .unwrap()
            .entry(user)
            .or_default()
            .push((id, tx));
        let out = self.set_online(user, true);
        self.deliver(out);
        (id, rx)
    }

    pub fn disconnect(&self, user: i64, conn: u64) {
        let gone = {
            let mut conns = self.conns.lock().unwrap();
            let list = conns.entry(user).or_default();
            list.retain(|(id, _)| *id != conn);
            let gone = list.is_empty();
            if gone {
                conns.remove(&user);
            }
            gone
        };
        if gone {
            let out = self.set_online(user, false);
            self.deliver(out);
        }
    }

    pub fn status(&self, user: i64) -> PartyStatus {
        let state = self.state.lock().unwrap();
        PartyStatus {
            party: state.party_of(user).map(Party::view),
            invites: state.invites_for(user),
        }
    }

    /// Invites `to`, opening a party for `me` first if I'm not in one.
    pub fn invite(&self, me: PartyPerson, to: PartyPerson) -> Result<PartyView, PartyError> {
        if me.id == to.id {
            return Err(PartyError::SelfInvite);
        }
        let mut out = Outbox::new();
        let view = {
            let mut state = self.state.lock().unwrap();
            let id = match state.member_of.get(&me.id) {
                Some(id) => id.clone(),
                None => state.open(&me),
            };
            let party = state
                .parties
                .get_mut(&id)
                .expect("member_of points at a party");
            if !party.members.iter().any(|m| m.id == to.id) {
                party.invited.retain(|i| i.to.id != to.id);
                party.invited.push(Invite {
                    to: to.clone(),
                    from: me,
                    at_ms: now_ms(),
                });
            }
            let view = party.view();
            out.extend(party.broadcast());
            out.push((
                to.id,
                LiveEvent::Invites {
                    invites: state.invites_for(to.id),
                },
            ));
            view
        };
        self.deliver(out);
        Ok(view)
    }

    pub fn cancel_invite(&self, me: i64, to: i64) -> Result<PartyView, PartyError> {
        let mut out = Outbox::new();
        let view = {
            let mut state = self.state.lock().unwrap();
            let id = state
                .member_of
                .get(&me)
                .cloned()
                .ok_or(PartyError::NotInParty)?;
            let party = state
                .parties
                .get_mut(&id)
                .expect("member_of points at a party");
            party.invited.retain(|i| i.to.id != to);
            let view = party.view();
            out.extend(party.broadcast());
            out.push((
                to,
                LiveEvent::Invites {
                    invites: state.invites_for(to),
                },
            ));
            view
        };
        self.deliver(out);
        Ok(view)
    }

    /// Joins a party I'm invited to (leaving my current one).
    pub fn join(&self, me: PartyPerson, party_id: &str) -> Result<PartyView, PartyError> {
        let mut out = Outbox::new();
        let view = {
            let mut state = self.state.lock().unwrap();
            if state.member_of.get(&me.id).is_some_and(|p| p == party_id) {
                return Ok(state.parties[party_id].view());
            }
            let invited = state
                .parties
                .get(party_id)
                .is_some_and(|p| p.invited.iter().any(|i| i.to.id == me.id));
            if !invited {
                return Err(PartyError::NoInvite);
            }
            out.extend(state.leave(me.id));
            let party = state.parties.get_mut(party_id).expect("checked above");
            party.invited.retain(|i| i.to.id != me.id);
            party.members.push(Member {
                id: me.id,
                name: me.display_name.clone(),
                offline_since: None,
            });
            state.member_of.insert(me.id, party_id.to_owned());
            let party = &state.parties[party_id];
            let view = party.view();
            out.extend(party.broadcast());
            out.push((
                me.id,
                LiveEvent::Invites {
                    invites: state.invites_for(me.id),
                },
            ));
            view
        };
        self.deliver(out);
        Ok(view)
    }

    pub fn decline(&self, me: i64, party_id: &str) {
        let mut out = Outbox::new();
        {
            let mut state = self.state.lock().unwrap();
            if let Some(party) = state.parties.get_mut(party_id) {
                party.invited.retain(|i| i.to.id != me);
                out.extend(party.broadcast());
            }
            out.push((
                me,
                LiveEvent::Invites {
                    invites: state.invites_for(me),
                },
            ));
        }
        self.deliver(out);
    }

    pub fn leave(&self, me: i64) {
        let out = self.state.lock().unwrap().leave(me);
        self.deliver(out);
    }

    /// Starts `media` for everyone in my party, a moment from now.
    pub fn start(&self, me: PartyPerson, media: Media, position_ms: u64) -> Result<(), PartyError> {
        self.change(me.id, |party, seq| {
            party.playback = Some(PartyPlayback {
                item_id: media.item_id,
                file_id: media.file_id,
                kind: media.kind,
                title: media.title,
                subtitle: media.subtitle,
                playing: true,
                position_ms,
                at_ms: now_ms() + START_LEAD_MS,
                seq,
                by: Some(me),
                action: PartyAction::Start,
            });
        })
    }

    /// Play, pause or seek the party's current media.
    pub fn control(
        &self,
        me: PartyPerson,
        action: PartyAction,
        position_ms: u64,
    ) -> Result<(), PartyError> {
        self.change(me.id, |party, seq| {
            let Some(playback) = party.playback.as_mut() else {
                return;
            };
            let playing = match action {
                PartyAction::Play => true,
                PartyAction::Pause => false,
                PartyAction::Seek | PartyAction::Start => playback.playing,
            };
            playback.playing = playing;
            playback.position_ms = position_ms;
            // Paused, the moment is now; playing, a moment ahead so everyone resumes together.
            playback.at_ms = now_ms() + if playing { RESUME_LEAD_MS } else { 0 };
            playback.seq = seq;
            playback.by = Some(me);
            playback.action = action;
        })
    }

    fn change(&self, me: i64, apply: impl FnOnce(&mut Party, u64)) -> Result<(), PartyError> {
        let out = {
            let mut state = self.state.lock().unwrap();
            let id = state
                .member_of
                .get(&me)
                .cloned()
                .ok_or(PartyError::NotInParty)?;
            let party = state
                .parties
                .get_mut(&id)
                .expect("member_of points at a party");
            party.seq += 1;
            let seq = party.seq;
            apply(party, seq);
            party.broadcast()
        };
        self.deliver(out);
        Ok(())
    }

    /// Takes out members offline too long and invitations nobody answered.
    pub fn tidy(&self) {
        let out = {
            let mut state = self.state.lock().unwrap();
            let mut out = Outbox::new();
            let gone: Vec<i64> = state
                .parties
                .values()
                .flat_map(|p| &p.members)
                .filter(|m| m.offline_since.is_some_and(|t| t.elapsed() > OFFLINE_GRACE))
                .map(|m| m.id)
                .collect();
            for user in gone {
                out.extend(state.leave(user));
            }
            let cutoff = now_ms() - INVITE_TTL_MS;
            let mut lapsed = Vec::new();
            for party in state.parties.values_mut() {
                let before = party.invited.len();
                party.invited.retain(|i| {
                    let keep = i.at_ms >= cutoff;
                    if !keep {
                        lapsed.push(i.to.id);
                    }
                    keep
                });
                if party.invited.len() != before {
                    out.extend(party.broadcast());
                }
            }
            for user in lapsed {
                out.push((
                    user,
                    LiveEvent::Invites {
                        invites: state.invites_for(user),
                    },
                ));
            }
            out
        };
        self.deliver(out);
    }

    fn set_online(&self, user: i64, online: bool) -> Outbox {
        let mut state = self.state.lock().unwrap();
        let Some(id) = state.member_of.get(&user).cloned() else {
            return Outbox::new();
        };
        let party = state
            .parties
            .get_mut(&id)
            .expect("member_of points at a party");
        let member = party
            .members
            .iter_mut()
            .find(|m| m.id == user)
            .expect("member of its party");
        let was_online = member.offline_since.is_none();
        if was_online == online {
            return Outbox::new();
        }
        member.offline_since = (!online).then(Instant::now);
        party.broadcast()
    }

    /// Sends one frame to every live connection of `user`.
    pub fn send(&self, user: i64, event: &LiveEvent) {
        let Ok(text) = serde_json::to_string(event) else {
            return;
        };
        if let Some(conns) = self.conns.lock().unwrap().get(&user) {
            for (_, tx) in conns {
                let _ = tx.send(text.clone());
            }
        }
    }

    fn deliver(&self, out: Outbox) {
        for (user, event) in out {
            self.send(user, &event);
        }
    }
}

impl State {
    fn party_of(&self, user: i64) -> Option<&Party> {
        self.member_of
            .get(&user)
            .and_then(|id| self.parties.get(id))
    }

    fn invites_for(&self, user: i64) -> Vec<PartyInvite> {
        let mut invites: Vec<PartyInvite> = self
            .parties
            .values()
            .filter_map(|p| {
                let invite = p.invited.iter().find(|i| i.to.id == user)?;
                Some(PartyInvite {
                    party_id: p.id.clone(),
                    from: invite.from.clone(),
                    members: p
                        .members
                        .iter()
                        .map(|m| PartyPerson {
                            id: m.id,
                            display_name: m.name.clone(),
                        })
                        .collect(),
                    sent_at_ms: invite.at_ms,
                })
            })
            .collect();
        invites.sort_by_key(|i| i.sent_at_ms);
        invites
    }

    /// A new party with `host` as its only member.
    fn open(&mut self, host: &PartyPerson) -> String {
        let id = party_id(host.id);
        self.parties.insert(
            id.clone(),
            Party {
                id: id.clone(),
                host: host.id,
                members: vec![Member {
                    id: host.id,
                    name: host.display_name.clone(),
                    offline_since: None,
                }],
                invited: Vec::new(),
                playback: None,
                seq: 0,
            },
        );
        self.member_of.insert(host.id, id.clone());
        id
    }

    /// Takes `user` out of their party. The next member becomes host; a
    /// party left empty ends, and its invitations with it.
    fn leave(&mut self, user: i64) -> Outbox {
        let Some(id) = self.member_of.remove(&user) else {
            return Outbox::new();
        };
        let mut out = vec![(
            user,
            LiveEvent::PartyLeft {
                party_id: id.clone(),
            },
        )];
        let party = self
            .parties
            .get_mut(&id)
            .expect("member_of points at a party");
        party.members.retain(|m| m.id != user);
        if let Some(first) = party.members.first()
            && party.host == user
        {
            party.host = first.id;
        }
        if party.members.is_empty() {
            let party = self.parties.remove(&id).expect("just looked up");
            for invite in party.invited {
                out.push((
                    invite.to.id,
                    LiveEvent::Invites {
                        invites: self.invites_for(invite.to.id),
                    },
                ));
            }
        } else {
            out.extend(party.broadcast());
        }
        out
    }
}

impl Party {
    fn view(&self) -> PartyView {
        PartyView {
            id: self.id.clone(),
            host_id: self.host,
            members: self
                .members
                .iter()
                .map(|m| PartyMember {
                    id: m.id,
                    display_name: m.name.clone(),
                    online: m.offline_since.is_none(),
                })
                .collect(),
            invited: self.invited.iter().map(|i| i.to.clone()).collect(),
            playback: self.playback.clone(),
        }
    }

    /// The current view, to every member.
    fn broadcast(&self) -> Outbox {
        let view = self.view();
        self.members
            .iter()
            .map(|m| {
                (
                    m.id,
                    LiveEvent::Party {
                        party: view.clone(),
                    },
                )
            })
            .collect()
    }
}

/// Short, unguessable enough for an in-memory room: a hash of the host and the time.
fn party_id(host: i64) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&host.to_le_bytes());
    hasher.update(
        &SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_le_bytes(),
    );
    hasher.finalize().to_hex()[..12].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person(id: i64, name: &str) -> PartyPerson {
        PartyPerson {
            id,
            display_name: name.to_owned(),
        }
    }

    fn frames(rx: &mut mpsc::UnboundedReceiver<String>) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        while let Ok(text) = rx.try_recv() {
            out.push(serde_json::from_str(&text).unwrap());
        }
        out
    }

    fn movie() -> Media {
        Media {
            item_id: 7,
            file_id: 9,
            kind: PartyMediaKind::Movie,
            title: "Sintel".into(),
            subtitle: None,
        }
    }

    #[test]
    fn invite_join_play_and_leave() {
        let parties = Parties::default();
        let (ana, bo) = (person(1, "Ana"), person(2, "Bo"));
        let (_, mut ana_rx) = parties.connect(1);
        let (bo_conn, mut bo_rx) = parties.connect(2);

        assert_eq!(
            parties.invite(ana.clone(), ana.clone()),
            Err(PartyError::SelfInvite)
        );
        let party = parties.invite(ana.clone(), bo.clone()).unwrap();
        assert_eq!(party.members.len(), 1);
        assert_eq!(party.invited, std::slice::from_ref(&bo));
        let invites = frames(&mut bo_rx);
        assert_eq!(invites.last().unwrap()["type"], "invites");
        assert_eq!(
            invites.last().unwrap()["invites"][0]["from"]["display_name"],
            "Ana"
        );

        assert_eq!(
            parties.join(person(3, "Cy"), &party.id),
            Err(PartyError::NoInvite)
        );
        let joined = parties.join(bo.clone(), &party.id).unwrap();
        assert_eq!(
            joined.members.iter().map(|m| m.id).collect::<Vec<_>>(),
            [1, 2]
        );
        assert!(joined.invited.is_empty());
        assert!(parties.status(2).invites.is_empty());
        frames(&mut ana_rx);
        frames(&mut bo_rx);

        parties.start(bo.clone(), movie(), 0).unwrap();
        let started = frames(&mut ana_rx);
        let playback = &started.last().unwrap()["party"]["playback"];
        assert_eq!(playback["action"], "start");
        assert_eq!(playback["by"]["id"], 2);
        assert!(
            playback["at_ms"].as_i64().unwrap() > now_ms(),
            "starts a moment ahead"
        );

        parties
            .control(ana.clone(), PartyAction::Pause, 61_000)
            .unwrap();
        let paused = parties.status(2).party.unwrap().playback.unwrap();
        assert!(!paused.playing);
        assert_eq!(paused.position_ms, 61_000);
        assert_eq!(paused.seq, 2);

        // Going offline keeps Bo in, shown as away.
        parties.disconnect(2, bo_conn);
        assert!(!parties.status(1).party.unwrap().members[1].online);

        // The host leaves: Bo hosts what's left.
        parties.leave(1);
        assert!(
            frames(&mut ana_rx)
                .iter()
                .any(|f| f["type"] == "party_left")
        );
        let left = parties.status(2).party.unwrap();
        assert_eq!(left.host_id, 2);
        assert_eq!(left.members.len(), 1);
        parties.leave(2);
        assert!(parties.status(2).party.is_none());
        assert_eq!(
            parties.control(bo, PartyAction::Play, 0),
            Err(PartyError::NotInParty)
        );
    }

    #[test]
    fn an_ended_party_takes_its_invitations_with_it() {
        let parties = Parties::default();
        let (_, mut cy_rx) = parties.connect(3);
        parties.invite(person(1, "Ana"), person(3, "Cy")).unwrap();
        assert_eq!(parties.status(3).invites.len(), 1);
        parties.leave(1);
        assert!(parties.status(3).invites.is_empty());
        let last = frames(&mut cy_rx).pop().unwrap();
        assert_eq!(last["type"], "invites");
        assert_eq!(last["invites"], serde_json::json!([]));
    }
}
