//! Creatures across the network (docs/netcode.md, build step 5; D51): creatures live only on
//! the host, which thinks and walks them; a joined game shows them. Each creature is a thing
//! the host tells of (`terra_walker::net::Host::put`): its kind, where it is, the way it
//! faces and what it is doing, by which a joined game plays the clip itself. A joined game
//! puts none out of its own and thinks for none.
//!
//! A joined player's blow is asked of the host ([`STRIKE`]), which lands it by the same
//! rule as its own player's, within reach and no harder than a blow can be (D51). A
//! creature's blow on a guest is ordered where the guest stands ([`HURT`]), and the guest's
//! own game takes it, less its leather, guard or roll, as vitals are still each game's own.

use crate::combat::{Blow, Struck, strike};
use crate::creatures::{self, Bitten, Creature, Kinds, State};
use crate::species::{Bestiary, Part};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use terra_net::client::Event as Heard;
use terra_net::proto::{EntityId, Kind};
use terra_survival::skills::{Did, Skill};
use terra_walker::net::link::Link;
use terra_walker::net::{Guest, Host, tell};
use terra_walker::{IsLocalPlayer, Player};
use terra_world::planet::Planet;
use terra_world::tick::Sim;

/// A creature, among the kinds of thing a server tells of.
pub const CREATURE: Kind = Kind(2);
/// The first byte of a joined player's blow as it is asked of the host, and of a creature's
/// blow on a guest as the host orders it.
pub const STRIKE: u8 = 18;
pub const HURT: u8 = 19;
/// A guest's blow lands only on a creature within this of them, and takes no more than
/// this: what a script or a changed game asks beyond them is refused (D51).
const REACH_M: f32 = 12.0;
const MOST_DAMAGE: f32 = 400.0;

/// What is shown of a creature: its kind, what it is at, which blow, where and facing what.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Shown {
    pub species: u8,
    pub act: Act,
    pub blow: u8,
    pub at: Vec3,
    pub facing: Vec3,
}

/// What a creature is at, as far as a game showing it needs: which clip it plays.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Act {
    Rests = 0,
    Feeds,
    Walks,
    Runs,
    Strikes,
    Lies,
    Flies,
}

impl Act {
    const ALL: [Act; 7] = [
        Act::Rests,
        Act::Feeds,
        Act::Walks,
        Act::Runs,
        Act::Strikes,
        Act::Lies,
        Act::Flies,
    ];

    pub fn of(state: State) -> Act {
        match state {
            State::Idle(_) | State::Perch { .. } | State::Roost { .. } => Act::Rests,
            State::Graze(_) => Act::Feeds,
            State::Wander(_) | State::Return => Act::Walks,
            State::Flee(_) | State::Chase | State::Climb { .. } => Act::Runs,
            State::Attack(_) => Act::Strikes,
            State::Dead(_) => Act::Lies,
            State::Fly { .. } => Act::Flies,
        }
    }

    /// The state a shown creature is given for it, at a place: enough to play its clip.
    fn state(self, at: Vec3) -> State {
        match self {
            Act::Rests => State::Idle(1.0),
            Act::Feeds => State::Graze(1.0),
            Act::Walks => State::Return,
            Act::Runs => State::Chase,
            Act::Strikes => State::Attack(0.0),
            Act::Lies => State::Dead(0.0),
            Act::Flies => State::Fly {
                from: at,
                to: at,
                along: 0.0,
            },
        }
    }
}

impl Shown {
    const BYTES: usize = 18;

    pub fn encoded(&self) -> [u8; Self::BYTES] {
        let mut bytes = [0; Self::BYTES];
        bytes[..3].copy_from_slice(&[self.species, self.act as u8, self.blow]);
        for (axis, value) in self.at.to_array().into_iter().enumerate() {
            bytes[3 + axis * 4..7 + axis * 4].copy_from_slice(&value.to_le_bytes());
        }
        for (axis, value) in self.facing.to_array().into_iter().enumerate() {
            bytes[15 + axis] = (value.clamp(-1.0, 1.0) * 127.0).round() as i8 as u8;
        }
        bytes
    }

    /// What some bytes show, if they are a creature's: nonsense shows nothing.
    pub fn decoded(bytes: &[u8]) -> Option<Shown> {
        if bytes.len() != Self::BYTES {
            return None;
        }
        let number = |from: usize| f32::from_le_bytes(bytes[from..from + 4].try_into().unwrap());
        let at = Vec3::new(number(3), number(7), number(11));
        let facing = Vec3::new(
            f32::from(bytes[15] as i8),
            f32::from(bytes[16] as i8),
            f32::from(bytes[17] as i8),
        );
        Some(Shown {
            species: bytes[0],
            act: *Act::ALL.get(bytes[1] as usize)?,
            blow: bytes[2],
            at: at.is_finite().then_some(at)?,
            facing: facing.try_normalize()?,
        })
    }
}

fn part_of(code: u8) -> Part {
    match code {
        0 => Part::Head,
        2 => Part::Legs,
        _ => Part::Body,
    }
}

/// A joined player's blow on a creature the host shows, as it is asked of the host.
pub fn asked(creature: u32, damage: f32, part: Part) -> Vec<u8> {
    let part = match part {
        Part::Head => 0,
        Part::Body => 1,
        Part::Legs => 2,
    };
    let mut bytes = vec![STRIKE];
    bytes.extend(creature.to_le_bytes());
    bytes.extend(damage.to_le_bytes());
    bytes.push(part);
    bytes
}

/// On the host: each creature told of, by the id it is told as.
#[derive(Resource, Default)]
struct ToldAs(HashMap<Entity, EntityId>);

/// On a joined game: a creature the host shows, by the id the host tells it as.
#[derive(Component)]
pub struct Remote(pub EntityId);

pub fn plugin(app: &mut App) {
    app.init_resource::<ToldAs>().add_systems(
        FixedUpdate,
        (
            (struck_by_guests, tell_creatures, hurt_guests)
                .chain()
                .before(tell)
                .run_if(resource_exists::<Host>),
            (show, hurt_here)
                .after(terra_walker::net::link::show)
                .run_if(resource_exists::<Link>),
        )
            .in_set(Sim::End),
    );
}

/// On the host, before the tick's telling: every creature is told of as it is now, a new
/// one put and one gone taken away.
fn tell_creatures(
    mut host: ResMut<Host>,
    mut told: ResMut<ToldAs>,
    creatures: Query<(Entity, &Transform, &Creature)>,
) {
    if host.is_added() {
        told.0.clear();
    }
    for (entity, at, creature) in &creatures {
        let place = at.translation;
        let id = *told
            .0
            .entry(entity)
            .or_insert_with(|| host.put(CREATURE, place));
        let shown = Shown {
            species: creature.species.index(),
            act: Act::of(creature.state),
            blow: creature.blow,
            at: place,
            facing: at.forward().as_vec3(),
        };
        host.tell_of(id, place, &shown.encoded());
    }
    told.0.retain(|entity, id| {
        let stays = creatures.contains(*entity);
        if !stays {
            host.gone(*id);
        }
        stays
    });
}

/// On the host: a guest's blow, asked for, lands on the creature it names by the rule the
/// host's own player's does, if the guest is within reach of it.
#[allow(clippy::too_many_arguments)]
fn struck_by_guests(
    mut commands: Commands,
    mut host: ResMut<Host>,
    told: Res<ToldAs>,
    bestiary: Res<Bestiary>,
    guests: Query<&Transform, With<Guest>>,
    mut creatures: Query<(&Transform, &mut Creature)>,
    (mut did, mut struck, mut said): (
        MessageWriter<Did>,
        MessageWriter<Struck>,
        MessageWriter<terra_survival::told::Told>,
    ),
) {
    let asks = std::mem::take(&mut host.asked);
    for (peer, bytes) in asks {
        let Some((&STRIKE, rest)) = bytes.split_first() else {
            // Another layer's.
            host.asked.push((peer, bytes));
            continue;
        };
        let (Some(by), Ok::<[u8; 9], _>(rest)) = (host.guest(peer), rest.try_into()) else {
            continue;
        };
        let id = EntityId(u32::from_le_bytes(rest[..4].try_into().unwrap()));
        let damage = f32::from_le_bytes(rest[4..8].try_into().unwrap());
        let Some(entity) = told
            .0
            .iter()
            .find_map(|(entity, told)| (*told == id).then_some(*entity))
        else {
            continue;
        };
        let (Ok(from), Ok((at, mut creature))) = (guests.get(by), creatures.get_mut(entity)) else {
            continue;
        };
        let within = from.translation.distance(at.translation) <= REACH_M;
        if !within || !(0.0..=MOST_DAMAGE).contains(&damage) || creature.health <= 0.0 {
            continue;
        }
        let blow = Blow {
            damage,
            at: at.translation,
            part: part_of(rest[8]),
            skill: Skill::Fighting,
        };
        strike(
            &mut commands,
            &bestiary,
            by,
            entity,
            &mut creature,
            blow,
            &mut did,
            &mut struck,
            &mut said,
        );
    }
}

/// On the host: a creature's blow on a guest is ordered where the guest stands, for the
/// guest's own game to take.
fn hurt_guests(
    mut host: ResMut<Host>,
    mut bitten: MessageReader<Bitten>,
    guests: Query<&Transform, With<Guest>>,
) {
    for bite in bitten.read() {
        let (Ok(at), Some(id)) = (guests.get(bite.player), host.id(bite.player)) else {
            continue;
        };
        let mut bytes = vec![HURT];
        bytes.extend(id.0.to_le_bytes());
        bytes.extend(bite.damage.to_le_bytes());
        host.order(at.translation, &bytes);
    }
}

/// On a joined game, at the tick's end: every creature the host tells of is shown, where and
/// as it is; one newly told of is made, one no longer told of is gone, and any creature of
/// this game's own from before it joined is let go.
#[allow(clippy::too_many_arguments)]
fn show(
    mut commands: Commands,
    link: Res<Link>,
    (kinds, bestiary, planet): (Option<Res<Kinds>>, Res<Bestiary>, Res<Planet>),
    mut shown: Local<HashMap<EntityId, Entity>>,
    mut creatures: Query<(Entity, &mut Transform, &mut Creature, Option<&Remote>)>,
) {
    let Some(kinds) = kinds else { return };
    for (entity, _, _, remote) in &creatures {
        if remote.is_none() {
            commands.entity(entity).try_despawn();
        }
    }
    let told = link.shown_of(CREATURE);
    for (id, from, to, share) in &told {
        let (Some(from), Some(to)) = (Shown::decoded(from), Shown::decoded(to)) else {
            continue;
        };
        let at = from.at.lerp(to.at, *share);
        let Some(species) = bestiary.all().nth(to.species as usize) else {
            continue;
        };
        let known = shown
            .get(id)
            .and_then(|entity| creatures.get_mut(*entity).ok());
        let Some((entity, mut transform, mut creature, _)) = known else {
            // Made where it is; shown as it is from the next tick.
            if let Some(entity) =
                creatures::spawn(&mut commands, (&kinds, &bestiary), &planet, species, at)
            {
                commands.entity(entity).insert(Remote(*id));
                shown.insert(*id, entity);
            }
            continue;
        };
        creature.remote = Some(id.0);
        let scale = transform.scale;
        *transform = Transform::from_translation(at)
            .looking_to(to.facing, at.normalize())
            .with_scale(scale);
        // What it is at, when that changes: a blow begun again is played again.
        if Act::of(creature.state) != to.act || creature.blow != to.blow {
            creature.blow = to.blow;
            if to.act == Act::Lies {
                creatures::fallen(&mut commands, &bestiary, entity, &mut creature);
            } else {
                creature.state = to.act.state(at);
            }
        }
    }
    shown.retain(|id, entity| {
        let stays = told.iter().any(|(told, ..)| told == id);
        if !stays {
            commands.entity(*entity).try_despawn();
        }
        stays
    });
}

/// On a joined game: a creature's blow the host ordered on this game's own player is taken
/// here, as one dealt here would be.
fn hurt_here(
    mut link: ResMut<Link>,
    player: Query<Entity, (With<Player>, IsLocalPlayer)>,
    mut bitten: MessageWriter<Bitten>,
) {
    let (blows, rest): (Vec<Heard>, Vec<Heard>) =
        std::mem::take(&mut link.events).into_iter().partition(
            |event| matches!(event, Heard::Op { bytes, .. } if bytes.first() == Some(&HURT)),
        );
    link.events = rest;
    let (Some(mine), Ok(player)) = (link.mine(), player.single()) else {
        return;
    };
    for blow in blows {
        let Heard::Op { bytes, .. } = blow else {
            continue;
        };
        let Ok::<[u8; 8], _>(rest) = bytes[1..].try_into() else {
            continue;
        };
        if u32::from_le_bytes(rest[..4].try_into().unwrap()) != mine.0 {
            continue;
        }
        let damage = f32::from_le_bytes(rest[4..].try_into().unwrap());
        if damage.is_finite() && damage > 0.0 {
            bitten.write(Bitten { player, damage });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_shown_of_a_creature_crosses_whole_and_nonsense_shows_nothing() {
        let shown = Shown {
            species: 3,
            act: Act::Strikes,
            blow: 1,
            at: Vec3::new(120.5, 30_012.25, -77.125),
            facing: Vec3::new(0.6, 0.0, -0.8),
        };
        let back = Shown::decoded(&shown.encoded()).unwrap();
        assert_eq!(
            (back.species, back.act, back.blow, back.at),
            (3, Act::Strikes, 1, shown.at)
        );
        assert!(back.facing.distance(shown.facing) < 0.02);
        assert_eq!(Shown::decoded(&[0; 5]), None);
        let mut nonsense = shown.encoded();
        nonsense[1] = 99;
        assert_eq!(Shown::decoded(&nonsense), None);
    }

    /// Every state has an act, and the act's state plays the clip the state's would.
    #[test]
    fn an_act_names_the_clip_a_state_plays() {
        let at = Vec3::Y;
        for state in [
            State::Idle(0.3),
            State::Graze(1.0),
            State::Wander(Vec3::X),
            State::Return,
            State::Flee(2.0),
            State::Chase,
            State::Attack(0.7),
            State::Dead(4.0),
        ] {
            let act = Act::of(state);
            assert_eq!(Act::of(act.state(at)), act, "{state:?}");
        }
    }

    #[test]
    fn a_blow_asked_for_names_its_creature_its_damage_and_where_it_lands() {
        let bytes = asked(7, 12.5, Part::Head);
        assert_eq!(bytes[0], STRIKE);
        assert_eq!(u32::from_le_bytes(bytes[1..5].try_into().unwrap()), 7);
        assert_eq!(f32::from_le_bytes(bytes[5..9].try_into().unwrap()), 12.5);
        assert_eq!(part_of(bytes[9]), Part::Head);
    }
}
