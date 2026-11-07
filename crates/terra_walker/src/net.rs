//! The walker layer across the network, the host's side (docs/netcode.md step 4; D51, D651,
//! D652): a server over this world's players. The walk already moves every [`Player`] by its
//! [`Intent`], whoever wrote it; here a guest's is written from what its game sent, and every
//! player's body is told to those near it.
//!
//! - [`hear`], before the walk: who joined and left, and each guest's intent for this tick.
//!   A guest is a `Player` like any other, with a [`Guest`] naming its connection; when it
//!   leaves, its body leaves the world at once (D637).
//! - [`tell`], at the tick's end: every player's mover body as its parts ([`SHOWN`] to all,
//!   [`OWN`] to its player), then the server's sends.
//!
//! A game that joins another's world is [`link`]'s side.
//!
//! The server is `terra_net`'s and knows nothing of Bevy; this is the thin side. A client that
//! joins need not be a game at all (D73): the tests' is a few lines.

pub mod ground;
pub mod link;
pub mod remote;
pub mod session;
pub mod worn;

use crate::intent::Intent;
use crate::movement::{Movers, walk};
use crate::{Motion, Player, Stamina, Stance, View};
use bevy::prelude::*;
use std::collections::HashMap;
use terra_net::proto::{EntityId, Kind, RoomId};
use terra_net::server::{Audience, Config, Event, PartSpec, Server};
use terra_net::transport::{Peer, ServerTransport};
use terra_net::wire::{Malformed, Reader, Wire, Writer};
use terra_world::tick::Sim;

/// The walker's parts of an entity: a body as shown to all (`mover::net::Shown`), and as its
/// own player is told it (`mover::net::Own`).
pub const SHOWN: usize = 0;
pub const OWN: usize = 1;
/// A tick, for a transport that keeps time: the systems here run once a tick.
pub(crate) const TICK: std::time::Duration = std::time::Duration::from_micros(15_625);
/// The kind of a player's entity.
pub const PERSON: Kind = Kind(1);
/// The one room, until realms are instances a server holds several of (instances.md).
pub const ROOM: RoomId = RoomId(0);

/// What a room's welcome says of its world (D596): the planet's seed and size, which are all
/// a game needs to make the same land, and what a joining game must already have made.
pub fn realm_of(planet: &terra_world::planet::Planet) -> Vec<u8> {
    let mut realm = planet.seed().0.to_le_bytes().to_vec();
    realm.extend_from_slice(&planet.radius_m().to_bits().to_le_bytes());
    realm
}

/// The walker's parts, in the order of their numbers: the first of the game's schema.
pub fn parts() -> Vec<PartSpec> {
    let part = |audience| PartSpec {
        audience,
        weight: 1.0,
    };
    // What is worn changes rarely and matters less than where a body is.
    let worn = PartSpec {
        audience: Audience::Public,
        weight: 0.3,
    };
    vec![part(Audience::Public), part(Audience::Owner), worn]
}

/// What a player means to do and where they look, on a tick: what a client sends up.
#[derive(Clone, Debug, PartialEq)]
pub struct Steering {
    pub intent: Intent,
    pub view: View,
}

/// The bits of a direction's axis on the wire, and of a pitch: a step of a thirtieth of a
/// degree, which no eye or aim tells. Both ends act on the direction as it reads back
/// ([`Steering::wired`]), so the step costs prediction nothing.
pub(crate) const WAY_BITS: u32 = 12;
const MOST_PITCH: f32 = std::f32::consts::FRAC_PI_2;

pub(crate) fn write_way(way: Vec3, to: &mut Writer) {
    for along in way.to_array() {
        to.quantised(along, -1.0, 1.0, WAY_BITS);
    }
}

/// A direction read back to unit length; one along nothing (far short of a unit before it is
/// made one) is refused.
pub(crate) fn read_way(from: &mut Reader) -> Result<Vec3, Malformed> {
    let mut way = Vec3::ZERO;
    for axis in 0..3 {
        way[axis] = from.quantised(-1.0, 1.0, WAY_BITS)?;
    }
    (way.length_squared() > 0.25)
        .then(|| way.normalize())
        .ok_or(Malformed::Range)
}

impl Steering {
    /// The steering as it reads back from the wire: what both ends act on.
    pub fn wired(&self) -> Steering {
        Steering::decoded(&self.encoded()).unwrap_or_else(|_| self.clone())
    }
}

impl Wire for Steering {
    fn write(&self, to: &mut Writer) {
        self.intent.write(to);
        write_way(self.view.forward, to);
        to.quantised(self.view.pitch, -MOST_PITCH, MOST_PITCH, WAY_BITS);
    }
    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        let intent = Intent::read(from)?;
        let forward = read_way(from)?;
        let pitch = from.quantised(-MOST_PITCH, MOST_PITCH, WAY_BITS)?;
        Ok(Steering {
            intent,
            view: View { forward, pitch },
        })
    }
}

/// A player whose intent comes from a connection, not this machine's keys.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Guest(pub Peer);

/// This world's server: present while the world is hosted.
#[derive(Resource)]
pub struct Host {
    server: Server,
    wire: Box<dyn ServerTransport + Send + Sync>,
    /// Where a guest's body is first put, and what a client is told of the room (its realm
    /// and seed).
    start: Vec3,
    realm: Vec<u8>,
    guests: HashMap<Peer, Entity>,
    ids: HashMap<Entity, EntityId>,
    /// Who each guest is, and where those who left were standing: one who comes back is put
    /// there (D592, D637). Kept while the world is hosted; the save keeps it longer (Play).
    identities: HashMap<Peer, [u8; 32]>,
    names: HashMap<Peer, String>,
    left: HashMap<[u8; 32], Vec3>,
    /// Guests whose bodies the walk has not yet made: welcomed once it has.
    arriving: Vec<(Peer, Entity)>,
    events: Vec<Event>,
    /// Operations guests asked for, for the layers that own them to check and order.
    pub asked: Vec<(Peer, Vec<u8>)>,
}

impl Host {
    /// A server of a schema (the walker's [`parts`] first) on a transport, for a world whose
    /// guests start at a place.
    pub fn new(
        config: Config,
        parts: Vec<PartSpec>,
        wire: Box<dyn ServerTransport + Send + Sync>,
        start: Vec3,
        realm: Vec<u8>,
    ) -> Host {
        Host {
            server: Server::new(config, parts),
            wire,
            start,
            realm,
            guests: HashMap::new(),
            ids: HashMap::new(),
            identities: HashMap::new(),
            names: HashMap::new(),
            left: HashMap::new(),
            arriving: Vec::new(),
            events: Vec::new(),
            asked: Vec::new(),
        }
    }

    /// Names the host's own player, for the list of who is here and for what it says.
    pub fn named(mut self, name: &str) -> Host {
        self.server.host_name = Some(name.to_string());
        self
    }

    /// The host's own words, to everyone.
    pub fn say(&mut self, text: &str) {
        let name = self
            .server
            .host_name
            .clone()
            .unwrap_or_else(|| "host".to_string());
        info!("Chat: {name}: {text}");
        self.server.say(&mut self.wire, &name, text);
        self.wire.flush();
    }

    /// The names of everyone here: the host's own player and each guest.
    pub fn players(&self) -> Vec<String> {
        let host = self.server.host_name.iter().cloned();
        host.chain(self.server.names().map(str::to_string))
            .collect()
    }

    /// A part of a player's entity as it is now, once the entity is on the wire.
    pub fn set(&mut self, player: Entity, part: usize, value: &impl Wire) {
        if let Some(&id) = self.ids.get(&player) {
            self.server.set_as(id, part, value);
        }
    }

    /// Orders an operation at a place: told to every game near it, now and when it comes
    /// (`terra_net::server::Server::order`). The first byte says whose it is ([`ground::GROUND`]).
    pub fn order(&mut self, place: Vec3, bytes: &[u8]) {
        self.server.order(ROOM, place.as_dvec3().to_array(), bytes);
    }

    /// The host is leaving: every guest is told why, and the session is over (D51).
    pub fn close(&mut self) {
        self.server
            .close(&mut self.wire, terra_net::proto::Reason::HostLeft);
        self.wire.flush();
    }

    pub fn server(&self) -> &Server {
        &self.server
    }

    /// A connection's player.
    pub fn guest(&self, peer: Peer) -> Option<Entity> {
        self.guests.get(&peer).copied()
    }

    /// A player's entity on the wire, once its body has been told.
    pub fn id(&self, player: Entity) -> Option<EntityId> {
        self.ids.get(&player).copied()
    }
}

pub fn plugin(app: &mut App) {
    remote::plugin(app);
    let hosted = resource_exists::<Host>;
    let joined = resource_exists::<link::Link>;
    app.add_systems(
        FixedUpdate,
        (
            hear.in_set(Sim::Act).before(walk).run_if(hosted),
            tell.in_set(Sim::End).run_if(hosted),
            link::listen.in_set(Sim::Act).before(walk).run_if(joined),
            link::record.in_set(Sim::Act).after(walk).run_if(joined),
            link::show.in_set(Sim::End).run_if(joined),
        ),
    )
    .add_systems(
        Update,
        (
            link::pace.run_if(joined),
            link::wait.run_if(joined).run_if(link::held),
        ),
    )
    .add_systems(Last, farewell)
    .init_resource::<ground::Ground>()
    .init_resource::<worn::Said>()
    .add_systems(
        FixedUpdate,
        (
            worn::host.in_set(Sim::End).before(tell).run_if(hosted),
            worn::joined
                .in_set(Sim::End)
                .after(link::show)
                .run_if(joined),
            worn::unjoined.in_set(Sim::End).run_if(not(joined)),
        ),
    )
    .add_systems(
        FixedUpdate,
        (
            ground::host.in_set(Sim::End).before(tell).run_if(hosted),
            ground::joined.in_set(Sim::End).run_if(joined),
            ground::unjoined.in_set(Sim::End).run_if(not(joined)),
        ),
    );
}

/// The game is closing: a host's guests are told the host left, and a joined game leaves, so
/// neither is left waiting on silence.
fn farewell(
    mut exits: MessageReader<AppExit>,
    host: Option<ResMut<Host>>,
    link: Option<ResMut<link::Link>>,
) {
    if exits.read().next().is_none() {
        return;
    }
    if let Some(mut host) = host {
        host.close();
    }
    if let Some(mut link) = link {
        link.leave();
    }
}

/// Before the walk: the server takes what arrived. A client let in gets a body where guests
/// start; one gone loses it; each guest's intent and view for this tick are written, the one
/// it sent for the tick or, that lost, its last.
pub fn hear(
    mut host: ResMut<Host>,
    mut commands: Commands,
    mut guests: Query<(&mut Intent, &mut View, &Transform), With<Guest>>,
) {
    let host = &mut *host;
    let mut events = std::mem::take(&mut host.events);
    host.wire.advance(TICK);
    host.server.poll(&mut host.wire, &mut events);
    for event in events.drain(..) {
        match event {
            Event::Hello {
                peer,
                identity,
                name,
            } => {
                // Back where it left, if it has been here before (D592, D637).
                let place = host.left.remove(&identity).unwrap_or(host.start);
                host.identities.insert(peer, identity);
                info!("{name} joined the world.");
                host.names.insert(peer, name);
                let up = place.normalize_or(Vec3::Y);
                let forward = up.any_orthonormal_vector();
                let body = (
                    Player,
                    Guest(peer),
                    Transform::from_translation(place),
                    // Its figure is its child, drawn only under something visible.
                    Visibility::default(),
                    // And dressed only once it has an outfit: with none of its own it wears its
                    // body's look whole, until what a player wears crosses too (step 5).
                    crate::character::Outfit(Vec::new()),
                    Motion::facing(forward),
                    Stance::default(),
                    Stamina::default(),
                    View {
                        forward,
                        pitch: 0.0,
                    },
                    Intent::default(),
                );
                let entity = commands.spawn(body).id();
                host.guests.insert(peer, entity);
                host.arriving.push((peer, entity));
            }
            Event::Left { peer } => {
                let name = host.names.remove(&peer).unwrap_or_default();
                info!("{name} left the world.");
                let Some(entity) = host.guests.remove(&peer) else {
                    continue;
                };
                // Its body leaves at once (D637); where it stood is kept for its return.
                if let (Some(identity), Ok((_, _, transform))) =
                    (host.identities.remove(&peer), guests.get(entity))
                {
                    host.left.insert(identity, transform.translation);
                }
                commands.entity(entity).despawn();
            }
            Event::Asked { peer, bytes } => host.asked.push((peer, bytes)),
            Event::Said { peer, text } => {
                let name = host.names.get(&peer).map_or("", String::as_str);
                info!("Chat: {name}: {text}");
            }
        }
    }
    host.events = events;
    for (&peer, &entity) in &host.guests {
        let Ok((mut intent, mut view, _)) = guests.get_mut(entity) else {
            continue;
        };
        let sent = host.server.intent(peer).map(Steering::decoded);
        if let Some(Ok(steering)) = sent {
            *intent = steering.intent;
            *view = steering.view;
        }
    }
}

/// At the tick's end: every player's body as the mover has it is told, a guest whose body
/// now exists is welcomed as it, a player gone is no more on the wire, and the server sends.
pub fn tell(mut host: ResMut<Host>, movers: Res<Movers>, players: Query<Entity, With<Player>>) {
    let host = &mut *host;
    for player in &players {
        let Some(body) = movers.body(player) else {
            continue;
        };
        let place = body.position.to_array();
        let id = *host
            .ids
            .entry(player)
            .or_insert_with(|| host.server.spawn(ROOM, PERSON, place));
        host.server.place(id, place);
        host.server.set_as(id, SHOWN, &body.shown());
        host.server.set_as(id, OWN, &body.own());
    }
    let (server, wire, ids, realm) = (&mut host.server, &mut host.wire, &host.ids, &host.realm);
    host.arriving.retain(|(peer, entity)| {
        let Some(&id) = ids.get(entity) else {
            return true;
        };
        server.welcome(wire, *peer, ROOM, id, realm);
        false
    });
    host.ids.retain(|player, id| {
        let stays = players.contains(*player);
        if !stays {
            host.server.despawn(*id);
        }
        stays
    });
    // What was asked this tick and is no layer's is nobody's.
    host.asked.clear();
    host.server.step(&mut host.wire);
}

#[cfg(test)]
#[path = "net_tests.rs"]
mod tests;
