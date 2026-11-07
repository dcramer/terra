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

pub mod link;
pub mod remote;

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
    vec![part(Audience::Public), part(Audience::Owner)]
}

/// What a player means to do and where they look, on a tick: what a client sends up.
#[derive(Clone, Debug, PartialEq)]
pub struct Steering {
    pub intent: Intent,
    pub view: View,
}

fn write_vec3(vector: Vec3, to: &mut Writer) {
    vector.to_array().write(to);
}

/// A vector, refused where it is no number: it is a stranger's.
fn read_vec3(from: &mut Reader) -> Result<Vec3, Malformed> {
    let vector = Vec3::from_array(<[f32; 3]>::read(from)?);
    if vector.is_finite() {
        Ok(vector)
    } else {
        Err(Malformed::Range)
    }
}

impl Wire for Steering {
    fn write(&self, to: &mut Writer) {
        self.intent.write(to);
        write_vec3(self.view.forward, to);
        self.view.pitch.write(to);
    }
    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        let intent = Intent::read(from)?;
        let forward = read_vec3(from)?;
        let pitch = f32::read(from)?;
        if !pitch.is_finite() {
            return Err(Malformed::Range);
        }
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
            arriving: Vec::new(),
            events: Vec::new(),
            asked: Vec::new(),
        }
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
    .add_systems(Update, link::pace.run_if(joined));
}

/// Before the walk: the server takes what arrived. A client let in gets a body where guests
/// start; one gone loses it; each guest's intent and view for this tick are written, the one
/// it sent for the tick or, that lost, its last.
pub fn hear(
    mut host: ResMut<Host>,
    mut commands: Commands,
    mut guests: Query<(&mut Intent, &mut View), With<Guest>>,
) {
    let host = &mut *host;
    let mut events = std::mem::take(&mut host.events);
    host.wire.advance(TICK);
    host.server.poll(&mut host.wire, &mut events);
    for event in events.drain(..) {
        match event {
            Event::Hello { peer, .. } => {
                let up = host.start.normalize_or(Vec3::Y);
                let forward = up.any_orthonormal_vector();
                let body = (
                    Player,
                    Guest(peer),
                    Transform::from_translation(host.start),
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
                if let Some(entity) = host.guests.remove(&peer) {
                    commands.entity(entity).despawn();
                }
            }
            Event::Asked { peer, bytes } => host.asked.push((peer, bytes)),
        }
    }
    host.events = events;
    for (&peer, &entity) in &host.guests {
        let Ok((mut intent, mut view)) = guests.get_mut(entity) else {
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
    host.server.step(&mut host.wire);
}

#[cfg(test)]
#[path = "net_tests.rs"]
mod tests;
