//! The walker layer across the network, a joining game's side (docs/netcode.md step 4; D651):
//! this machine's player is predicted by the world's own walk and corrected by replay, and
//! every other player is a [`Puppet`] put where the server last said, a little in the past.
//!
//! - [`listen`], before the walk: what the server sent; its answer for this player's body
//!   checked against what was predicted for that tick and, where it differs, the body set to
//!   the answer and the ticks since walked again; then this tick's intent sent.
//! - [`record`], after the walk: what the walk left of the player, kept until answered.
//! - [`show`], at the tick's end: the others, between their last two words.
//! - [`pace`], each frame: time run a little fast or slow, so ticks come more or less often
//!   and the clock's margin holds; a tick's length is the server's, always.
//!
//! What is replayed is the body and what the walk keeps of the player ([`Walked`]); a fall or
//! a drowning told during a replay is told again, which the server's word on harm will settle
//! when vitals cross (step 5).

use super::{OWN, SHOWN, Steering};
use crate::intent::Intent;
use crate::movement::{Movers, walk};
use crate::{IsLocalPlayer, Motion, Player, Stamina, Stance, View};
use bevy::prelude::*;
use std::collections::HashMap;
use terra_motion::mover::net::{Own, Shown};
use terra_motion::mover::{Body, Ground};
use terra_net::client::{Client, Event as Heard, SHOWN_SENDS_BACK};
use terra_net::clock::Clock;
use terra_net::identity::Identity;
use terra_net::predict::History;
use terra_net::proto::{EntityId, Reason};
use terra_net::transport::ClientTransport;
use terra_net::wire::Wire;

/// What the walk leaves of a player after a tick: all a replay puts back.
#[derive(Clone)]
struct Walked {
    body: Body,
    stance: Stance,
    stamina: Option<Stamina>,
    motion: Motion,
    /// The server's answer this state is to be set to, when it differed.
    answer: Option<Own>,
}

/// Another player, as the server last said. It is a [`Player`], so it is given a figure as
/// any player is, moved by its [`Motion`]; it has no [`View`], so the walk, which moves only
/// what looks somewhere, leaves it where the server put it.
#[derive(Component, Clone, Copy, Debug)]
pub struct Puppet {
    pub id: EntityId,
    pub shown: Shown,
}

/// Why this game's link to a hosted world ended, in words for the player.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct Ended(pub String);

/// What a player is told when the host's world is not the one this game made.
/// Said when a connection ends before the server ever welcomed this game.
const NO_ANSWER: &str =
    "No answer from that address. Check it, and that the host's game is hosting.";
const OTHER_WORLD: &str = "That world is made from another seed or size than this game's. Start this game on the host's seed, then join again.";

/// How often the server sends state, ticks (its `send_every`): others are shown
/// [`SHOWN_SENDS_BACK`] of them in the past.
const SEND_EVERY: u32 = 2;
/// The most events kept for the layers above before the oldest are dropped unread.
const MOST_EVENTS_KEPT: usize = 4096;

/// This game's connection to the server of the world it plays in: present while joined.
#[derive(Resource)]
pub struct Link {
    client: Client,
    wire: Box<dyn ClientTransport + Send + Sync>,
    clock: Clock,
    history: History<Steering, Walked>,
    /// The newest tick the server has answered for this player's body, and whether the body
    /// has been taken up from the server at all.
    answered: u32,
    begun: bool,
    /// The tick the walk is on, and what was sent for it.
    tick: Option<u32>,
    steering: Option<Steering>,
    puppets: HashMap<EntityId, Entity>,
    heard: Vec<Heard>,
    /// The world this game has made (`net::realm_of`), which the host's must be; empty for
    /// any. And why the link is over, once it is.
    realm: Vec<u8>,
    over: Option<String>,
    /// The realm the server welcomed this game to, once it has.
    welcomed: Option<Vec<u8>>,
    /// The link was kept waiting, the world's clock held (a title screen, a load): the next
    /// tick starts the clock and the body again from the server's newest word.
    waited: bool,
    /// Who is in the world, as the server has said.
    players: Vec<String>,
    /// What the server said that is the layers' above to act on (operations, events, the
    /// welcome, the end): drained by them each tick.
    pub events: Vec<Heard>,
}

impl Link {
    /// Joins over a transport, as an identity.
    pub fn join(
        mut wire: Box<dyn ClientTransport + Send + Sync>,
        version: u64,
        identity: &Identity,
        name: &str,
        realm: Vec<u8>,
    ) -> Link {
        let mut client = Client::new();
        client.hello(&mut wire, version, identity, name);
        Link {
            client,
            wire,
            clock: Clock::default(),
            history: History::default(),
            answered: 0,
            begun: false,
            tick: None,
            steering: None,
            puppets: HashMap::new(),
            heard: Vec::new(),
            realm,
            over: None,
            welcomed: None,
            waited: false,
            players: Vec::new(),
            events: Vec::new(),
        }
    }

    /// Says something to everyone in the world.
    pub fn say(&mut self, text: &str) {
        self.client.say(&mut self.wire, text);
    }

    /// Asks the host for an operation: it is made when the host's order of it comes.
    pub fn ask(&mut self, bytes: &[u8]) {
        self.client.ask(&mut self.wire, bytes);
    }

    /// The names of the others in the world.
    pub fn players(&self) -> &[String] {
        &self.players
    }

    /// Leaves: the host sees this player go at once.
    pub fn leave(&mut self) {
        self.wire.close();
        self.wire.flush();
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    /// The tick the walk is on, once welcomed.
    pub fn tick(&self) -> Option<u32> {
        self.tick
    }

    /// How many of the server's answers differed from what was predicted.
    pub fn corrections(&self) -> u64 {
        self.history.corrections
    }

    /// How fast the fixed tick should run, as a share of 64 a second.
    pub fn rate(&self) -> f64 {
        self.clock.rate()
    }

    /// The realm the server welcomed this game to (`net::realm_of`'s bytes), once it has:
    /// all a game needs to make the land it joined.
    pub fn realm(&self) -> Option<&[u8]> {
        self.welcomed.as_deref()
    }

    /// Takes what the server sent, so much time having passed on the wire.
    fn hear(&mut self, passed: std::time::Duration) {
        let mut heard = std::mem::take(&mut self.heard);
        self.wire.advance(passed);
        self.client.poll(&mut self.wire, &mut heard);
        for event in heard.drain(..) {
            match &event {
                Heard::Welcome { tick, realm, .. } => {
                    if !self.realm.is_empty() && *realm != self.realm {
                        self.over = Some(OTHER_WORLD.to_string());
                    }
                    self.welcomed = Some(realm.clone());
                    self.clock.start(*tick);
                    self.history.clear();
                    (self.answered, self.begun) = (0, false);
                }
                // Lost before any welcome: nobody was there to answer.
                Heard::Closed(Reason::Lost) if self.welcomed.is_none() => {
                    self.over = Some(NO_ANSWER.to_string());
                }
                Heard::Closed(reason) => self.over = Some(reason.message().to_string()),
                Heard::Said { name, text } => info!("Chat: {name}: {text}"),
                Heard::Roster { name, here: true } => self.players.push(name.clone()),
                Heard::Roster { name, here: false } => {
                    if let Some(at) = self.players.iter().position(|player| player == name) {
                        self.players.remove(at);
                    }
                }
                Heard::Entered(_) | Heard::Gone(_) => continue,
                _ => {}
            }
            self.events.push(event);
        }
        self.heard = heard;
        if self.events.len() > MOST_EVENTS_KEPT {
            self.events.drain(..MOST_EVENTS_KEPT / 2);
        }
    }

    /// Keeps the link while the world's clock is held: the server is heard and answered (its
    /// packets acknowledged, the connection kept), and no tick is walked or sent.
    fn wait(&mut self, passed: std::time::Duration) {
        self.hear(passed);
        self.wire.flush();
        self.waited = true;
    }

    fn listen(&mut self, world: &mut World) {
        self.hear(super::TICK);
        if std::mem::take(&mut self.waited)
            && let Some(stamp) = self.client.stamp()
        {
            // Ticks passed on the server that this game did not walk: its clock and its
            // body begin again from the server's newest word, as at the welcome.
            self.clock.start(stamp.tick);
            self.history.clear();
            (self.answered, self.begun) = (0, false);
        }
        if self
            .client
            .stamp()
            .is_some_and(|stamp| self.clock.hear(stamp))
        {
            self.history.clear();
        }
        let mut locals = world.query_filtered::<Entity, IsLocalPlayer>();
        let Some(player) = locals.iter(world).next() else {
            return;
        };
        self.answer(world, player);
        self.tick = self.clock.advance();
        self.steering = None;
        if let (Some(tick), Some(intent), Some(view)) = (
            self.tick,
            world.get::<Intent>(player),
            world.get::<View>(player),
        ) {
            // As the wire carries it: the walk here must be given what the host's is.
            let steering = Steering {
                intent: intent.clone(),
                view: *view,
            }
            .wired();
            if let Some(mut intent) = world.get_mut::<Intent>(player) {
                *intent = steering.intent.clone();
            }
            if let Some(mut view) = world.get_mut::<View>(player) {
                *view = steering.view;
            }
            self.client.intent(tick, steering.encoded());
            self.steering = Some(steering);
        }
    }

    /// Takes the server's newest answer for this player's body.
    fn answer(&mut self, world: &mut World, player: Entity) {
        let mine = self.client.entities().find(|(_, entity)| entity.mine);
        let Some(sample) = mine
            .and_then(|(_, entity)| entity.part(OWN))
            .map(|samples| &samples.newer)
        else {
            return;
        };
        if sample.tick <= self.answered {
            return;
        }
        let (tick, Ok(own)) = (sample.tick, Own::decoded(&sample.bytes)) else {
            return;
        };
        self.answered = tick;
        if !self.begun {
            // The first word of its body: where the server put it, taken as it is.
            self.begun = true;
            if let Some(mut motion) = world.get_mut::<Motion>(player) {
                *motion = Motion::facing(own.facing);
            }
            set_to(world, player, &own);
            return;
        }
        let replayed = self.history.reconcile(
            tick,
            &own,
            |walked, own| walked.body.agrees(own),
            |walked, own| walked.answer = Some(*own),
            |before, steering| {
                put(world, player, before);
                if let Some(mut intent) = world.get_mut::<Intent>(player) {
                    *intent = steering.intent.clone();
                }
                if let Some(mut view) = world.get_mut::<View>(player) {
                    *view = steering.view;
                }
                // The walk of a tick again, as the schedule runs it.
                let _ = world.run_system_cached(walk);
                taken(world, player).unwrap_or_else(|| before.clone())
            },
        );
        if let Some(walked) = replayed {
            put(world, player, &walked);
        }
    }
}

/// Puts a player where the server's answer says: its place now, and the body's correction
/// for the walk to make, which alone can ask the ground.
fn set_to(world: &mut World, player: Entity, own: &Own) {
    if let Some(mut transform) = world.get_mut::<Transform>(player) {
        transform.translation = own.position.as_vec3();
    }
    world.resource_mut::<Movers>().correct(player, *own);
}

/// Puts back what a walk left of a player.
fn put(world: &mut World, player: Entity, walked: &Walked) {
    if let Some(mut transform) = world.get_mut::<Transform>(player) {
        transform.translation = walked.body.position.as_vec3();
    }
    if let Some(mut stance) = world.get_mut::<Stance>(player) {
        *stance = walked.stance.clone();
    }
    if let (Some(mut stamina), Some(kept)) = (world.get_mut::<Stamina>(player), walked.stamina) {
        *stamina = kept;
    }
    if let Some(mut motion) = world.get_mut::<Motion>(player) {
        *motion = walked.motion.clone();
    }
    world
        .resource_mut::<Movers>()
        .restore(player, walked.body.clone());
    if let Some(own) = &walked.answer {
        set_to(world, player, own);
    }
}

/// What the walk has left of a player.
fn taken(world: &World, player: Entity) -> Option<Walked> {
    Some(Walked {
        body: world.resource::<Movers>().body(player)?.clone(),
        stance: world.get::<Stance>(player)?.clone(),
        stamina: world.get::<Stamina>(player).copied(),
        motion: world.get::<Motion>(player)?.clone(),
        answer: None,
    })
}

/// Before the walk: see [`Link`]'s module.
pub fn listen(world: &mut World) {
    let Some(mut link) = world.remove_resource::<Link>() else {
        return;
    };
    link.listen(world);
    keep(world, link);
}

/// While the world's clock is held (a title screen, a load), each frame: the link is kept,
/// by the time that truly passed.
pub fn wait(world: &mut World) {
    let passed = world.resource::<Time<Real>>().delta();
    let Some(mut link) = world.remove_resource::<Link>() else {
        return;
    };
    link.wait(passed);
    keep(world, link);
}

/// Whether the world's clock is held, so no tick comes to keep the link.
pub fn held(time: Res<Time<Virtual>>) -> bool {
    time.is_paused()
}

/// Puts the link back, unless it is over, by the server's word or because its world is not
/// this game's: then it goes, its puppets with it, and why is kept for whoever asks.
fn keep(world: &mut World, mut link: Link) {
    if let Some(why) = link.over.take() {
        link.wire.close();
        for puppet in link.puppets.values() {
            // Gone already if the world they stood in was left.
            if let Ok(puppet) = world.get_entity_mut(*puppet) {
                puppet.despawn();
            }
        }
        warn!("Left the hosted world: {why}");
        world.insert_resource(Ended(why));
        return;
    }
    world.insert_resource(link);
}

/// After the walk: what it left of this machine's player is kept for the server's answer, and
/// the tick's packet goes.
pub fn record(world: &mut World) {
    let Some(mut link) = world.remove_resource::<Link>() else {
        return;
    };
    let mut locals = world.query_filtered::<Entity, IsLocalPlayer>();
    let player = locals.iter(world).next();
    let walked = player.and_then(|player| taken(world, player));
    if let (true, Some(tick), Some(steering), Some(walked)) =
        (link.begun, link.tick, link.steering.take(), walked)
    {
        link.history.record(tick, steering, walked);
    }
    let Link { client, wire, .. } = &mut link;
    client.send(wire);
    world.insert_resource(link);
}

/// At the tick's end: every other player the server tells of is a puppet, put between its
/// last two words, two sends back, and held at the newest: never run on past what was said.
pub fn show(
    mut link: ResMut<Link>,
    mut commands: Commands,
    mut puppets: Query<(&mut Transform, &mut Puppet, &mut Motion)>,
) {
    let Link {
        client,
        puppets: held,
        ..
    } = &mut *link;
    let Some(newest) = client.stamp().map(|stamp| stamp.tick) else {
        return;
    };
    let at = f64::from(newest.saturating_sub(SHOWN_SENDS_BACK * SEND_EVERY));
    for (id, entity) in client.entities().filter(|(_, entity)| !entity.mine) {
        let Some((from, to, share)) = entity.part(SHOWN).map(|samples| samples.around(at)) else {
            continue;
        };
        let (Ok(from), Ok(to)) = (Shown::decoded(&from.bytes), Shown::decoded(&to.bytes)) else {
            continue;
        };
        let shown = from.between(&to, share);
        let place = shown.feet.as_vec3();
        match held
            .get(&id)
            .and_then(|puppet| puppets.get_mut(*puppet).ok())
        {
            Some((mut transform, mut puppet, mut motion)) => {
                transform.translation = place;
                puppet.shown = shown;
                moving(&mut motion, &shown);
            }
            None => {
                let mut motion = Motion::facing(shown.facing);
                moving(&mut motion, &shown);
                let stance = Stance::default();
                // Its figure is its child, drawn only under something visible.
                let seen = Visibility::default();
                // And dressed only once it has an outfit: its body's look whole, for now.
                let worn = crate::character::Outfit(Vec::new());
                let at = Transform::from_translation(place);
                let puppet = (Player, Puppet { id, shown }, at, seen, worn, motion, stance);
                held.insert(id, commands.spawn(puppet).id());
            }
        }
    }
    held.retain(|id, puppet| {
        let stays = client.entity(*id).is_some();
        if !stays {
            commands.entity(*puppet).despawn();
        }
        stays
    });
}

/// A puppet's motion as the figure reads it, from what the server said of its body.
fn moving(motion: &mut Motion, shown: &Shown) {
    let up = shown.feet.as_vec3().normalize_or(Vec3::Y);
    motion.velocity = shown.velocity.reject_from(up);
    motion.rising = shown.velocity.dot(up);
    motion.on_ground = shown.ground == Ground::Grounded;
    motion.swimming = shown.ground == Ground::Swimming;
    motion.facing = shown.facing;
}

/// Each frame: time runs at the clock's rate, at most 2% from the server's (D651), so ticks
/// come that much more or less often. A tick's own length is never changed: the walk of a
/// tick must be the server's walk of it, to the bit. (Found in the game: a tick 2% long was
/// corrected on nearly every answer.)
pub fn pace(link: Res<Link>, mut time: ResMut<Time<Virtual>>) {
    time.set_relative_speed_f64(link.rate());
}
