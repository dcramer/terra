//! How a player moves over the planet: walking, sprinting, rolling, jumping and floating,
//! moved by the mover every body walks (D53, `terra_motion::mover`, docs/controller.md). The
//! walk turns what the player means into the mover's intent, gathers what stops them near
//! them (`mover_world`: the ground's field, their base, props, trees and rocks, other bodies,
//! the sea), ticks the mover, and shows where it put them.
//!
//! What it gathers each tick it gathers into lists it keeps, so a tick makes none (D53's cost
//! row, `cost.body`); props are filed by place when one appears, moves, hides or goes, and
//! asked near the body, as flora's index is.

use crate::controls::Action;
use crate::intent::Intent;
use crate::mover_world::AtBase;
use crate::stepping::Stepping;
use crate::{Downed, Motion, Player, Stamina, Stance, View, bodies, mover_world};
use bevy::math::DVec3;
use bevy::prelude::*;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use terra_bases::{self as build, Site};
use terra_build_layout::solids::Slab;
use terra_build_layout::walk::Walking;
use terra_motion::mover::{self, Block, Bodies, Capsule, Capsules, Ground, Kind, Obstacles, Sdf};
use terra_world::flora;
use terra_world::planet::{HeightMemo, Planet};
use terra_world::spatial::SpatialHash;
use terra_world::tick::Sim;

/// A man's jog and sprint, as his kind has them (`assets/data/movers.ron`, tested).
pub const JOG_SPEED: f32 = 4.3;
pub const SPRINT_SPEED: f32 = 7.4;
const GRAVITY: f32 = 16.0;
/// What a jump costs, spent when the mover jumps, not on the press (its buffer and coyote
/// time decide).
const JUMP_STAMINA: f32 = 8.0;
/// A roll: what it costs, how long it lasts and how fast it carries you.
const DODGE_STAMINA: f32 = 18.0;
const DODGE_S: f32 = 0.45;
const DODGE_SPEED: f32 = 8.5;
/// A guard's pace, as a share of the jog.
const GUARD_PACE: f32 = 0.5;
/// How far round a body what could stop it this tick is gathered, past what it covers.
const REACH_M: f32 = 3.0;
/// Where the IDs of the other bodies (creatures, people), as the mover sees them, begin: the
/// players' are counted from 0.
const OTHERS: u32 = 1 << 31;
/// How many of the land's lattice points the players' memo keeps, a player, before it forgets
/// them: a body at a jog asks about a few new ones a second (the mover's bench's own).
const MEMO_MOST: usize = 1024;

pub fn plugin(app: &mut App) {
    app.init_resource::<Movers>()
        .add_message::<Fell>()
        .add_message::<Drowned>()
        .add_systems(FixedUpdate, walk.in_set(Sim::Act));
    crate::stepping::plugin(app);
}

/// A body landed from a fall that hurt it (D476): the share of its health the landing took,
/// by the speed it met the floor at (`mover::Body::fall`, its kind's numbers); 1 or more is
/// fatal. What it costs is the gameplay's to take (`terra_creatures::combat`).
#[derive(Message, Clone, Copy, Debug)]
pub struct Fell {
    pub body: Entity,
    pub harm: f32,
}

/// A body out of breath under water drowning (D74, D478): the share of its health this pulse
/// took (`mover::Body::drown`, a pulse a second, its kind's `drown_s`); 1 or more in all is
/// fatal. What it costs is the gameplay's to take (`terra_creatures::combat`).
#[derive(Message, Clone, Copy, Debug)]
pub struct Drowned {
    pub body: Entity,
    pub harm: f32,
}

/// The players as the mover keeps them, each by an ID it is given when first walked, and what
/// the walk keeps from tick to tick: the planet's land as last asked, the props by place, and
/// its lists.
#[derive(Resource)]
pub struct Movers {
    bodies: Bodies,
    ids: HashMap<Entity, u32>,
    next: u32,
    kind: Kind,
    /// The land's lattice points the players' bodies were asked about, for the next tick's
    /// asks nearby (`Planet::distance_in`).
    memo: HeightMemo,
    props: Props,
    lists: Lists,
}

impl Default for Movers {
    fn default() -> Self {
        Movers {
            bodies: Bodies::default(),
            ids: HashMap::new(),
            next: 0,
            kind: man(),
            memo: HeightMemo::default(),
            props: Props::default(),
            lists: Lists::default(),
        }
    }
}

impl Movers {
    /// A player's body as the mover has it.
    pub fn body(&self, player: Entity) -> Option<&mover::Body> {
        self.bodies.get(*self.ids.get(&player)?)
    }

    /// The players' kind (a man's, D136).
    pub fn kind(&self) -> &Kind {
        &self.kind
    }

    /// The boxes the last tick gathered round the players (their base's cubes, props), as the
    /// mover met them: what a foot stands on besides the ground.
    pub fn blocks(&self) -> &[Block] {
        &self.lists.blocks
    }
}

/// The props (`bodies::Solid`) filed by where they stand, and how far the widest reaches from
/// its foot, so those near a body are found without looking at all of them.
#[derive(Default)]
struct Props {
    filed: SpatialHash<Entity>,
    widest: f32,
}

impl Props {
    fn file<'a>(
        &mut self,
        solids: impl Iterator<Item = (Entity, &'a bodies::Solid, &'a Transform)>,
    ) {
        self.filed.clear();
        self.widest = 0.0;
        for (entity, solid, transform) in solids {
            self.filed.insert(transform.translation, entity);
            self.widest = self.widest.max(solid.half.y + solid.half.length());
        }
    }
}

/// The lists a tick fills and the next empties, kept for their room.
#[derive(Default)]
struct Lists {
    places: Vec<(Vec3, f32)>,
    blocks: Vec<Block>,
    slabs: Vec<Slab>,
    trees: Vec<&'static terra_core::Solid>,
    others: Vec<(u32, Capsule, Vec3)>,
    intents: Vec<(u32, mover::Intent)>,
    wishes: Vec<(Entity, Vec3)>,
    seen: HashSet<Entity>,
}

/// An emptied list of borrows as one of borrows that live as long as another, keeping its room:
/// what the walk borrows a tick (flora's trees) goes in a list it keeps between ticks. Rust
/// collects a list's `into_iter().map(..)` into the same room when the items are the same size.
fn recycle<'b, T: ?Sized>(mut list: Vec<&T>) -> Vec<&'b T> {
    list.clear();
    list.into_iter().map(|_| unreachable!("emptied")).collect()
}

/// A man's kind (D136): the player is one.
fn man() -> Kind {
    Kind::parse(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/data/movers.ron"
        )),
        "Man",
    )
    .expect("movers.ron has a man")
}

/// Which way a body turns to face: as a layer above steers it, the view while it works,
/// or the way it goes.
fn toward(stance: &Stance, view: Vec3, wish: Vec3, up: Vec3) -> Vec3 {
    if let Some(face) = stance.steer.face {
        face.reject_from(up).normalize_or_zero()
    } else if stance.since_acting < 0.8 {
        view
    } else {
        wish
    }
}

/// What a player means this tick, as the mover's intent: the way they go, sprinting, a jump
/// they could pay for, their pace (a guard's or a move's; never the ground's, D149) and, on
/// the ground, a roll's or a move's drive. Starts a roll they ask for and can pay for, and
/// runs down the stance's timers.
struct Asked<'a> {
    intent: &'a Intent,
    view: &'a View,
    up: Vec3,
    on_ground: bool,
    /// Ctrl and the wheel change the hammer's material while a part is in hand (D232).
    part_in_hand: bool,
}

fn ask(
    asked: Asked,
    stance: &mut Stance,
    mut stamina: Option<&mut Stamina>,
    kind: &Kind,
    dt: f32,
) -> mover::Intent {
    let Asked { intent, view, .. } = asked;
    let right = view.forward.cross(asked.up);
    let held = |action| f32::from(intent.held(action));
    let wish = (view.forward * (held(Action::Forward) - held(Action::Back))
        + right * (held(Action::Right) - held(Action::Left)))
    .normalize_or_zero();
    stance.attacking = (stance.attacking - dt).max(0.0);
    stance.flinch = (stance.flinch - dt).max(0.0);
    stance.dodging = (stance.dodging - dt).max(0.0);
    let steer = stance.steer;
    if stance.dodging == 0.0
        && !steer.no_roll
        && asked.on_ground
        && intent.pressed(Action::Dodge)
        && !asked.part_in_hand
        && stamina
            .as_mut()
            .is_none_or(|body| body.spend(DODGE_STAMINA))
    {
        stance.dodging = DODGE_S;
        stance.dodge_direction = if wish == Vec3::ZERO {
            -view.forward
        } else {
            wish
        };
    }
    let rolling = stance.dodging > 0.0;
    let can = stamina.as_ref().is_none_or(|body| body.can_sprint());
    let (sprint, pace) = match steer.pace {
        Some(pace) => (false, pace / kind.jog),
        None if stance.guarding => (false, GUARD_PACE),
        None => (intent.held(Action::Sprint) && can, 1.0),
    };
    let drive = if !asked.on_ground {
        None
    } else if rolling {
        Some(stance.dodge_direction * DODGE_SPEED)
    } else {
        steer.drive
    };
    mover::Intent {
        wish: if rolling {
            stance.dodge_direction
        } else {
            wish
        },
        sprint,
        jump: intent.pressed(Action::Jump) && can,
        scale: pace,
        drive,
        // Swimming (D478): forward looking down dives, jump held rises.
        pitch: if intent.held(Action::Forward) && !intent.held(Action::Back) {
            view.pitch
        } else {
            0.0
        },
        rise: intent.held(Action::Jump),
    }
}

/// Where what can stop the players is found: the props by place, the build they are at, the
/// trees and rocks in flora's index, the other bodies.
struct Sources<'w, 's, 'a> {
    props: &'a Props,
    solids: &'a Query<
        'w,
        's,
        (
            Entity,
            &'static bodies::Solid,
            &'static Transform,
            &'static Visibility,
        ),
        Without<Player>,
    >,
    site: Option<&'a Site>,
    builds: &'a Query<'w, 's, &'static build::Built>,
    flora: &'a flora::Obstacles,
    others: &'a Query<'w, 's, (&'static bodies::Body, &'static Transform), Without<Player>>,
}

/// What can stop the players this tick, gathered near each into the walk's lists: the props
/// and the base they are at as boxes, the trees and rocks borrowed from flora, the other bodies
/// as still capsules (a person's only its core, D479).
fn gather<'a>(
    sources: &Sources<'_, '_, 'a>,
    lists: &mut Lists,
    trees: &mut Vec<&'a terra_core::Solid>,
    kind: &Kind,
) {
    let Lists {
        places,
        blocks,
        slabs,
        others,
        ..
    } = lists;
    blocks.clear();
    others.clear();
    trees.clear();
    for &(at, reach) in places.iter() {
        let near = sources.props.filed.near(at, reach + sources.props.widest);
        let props = near.filter_map(|(_, entity)| sources.solids.get(*entity).ok());
        let props = props.map(|(_, solid, transform, shown)| (solid, transform, shown));
        mover_world::props(props, at, reach, blocks);
        let built = sources.builds.iter().map(|built| &built.0);
        if let Some(base) = at_base(at, sources.site, built) {
            let at_base = AtBase {
                base: base.walkable(),
                frame: &base.frame,
                kits: base.kits.as_deref(),
            };
            mover_world::base(at_base, at, reach, slabs, blocks);
        }
        mover_world::flora(sources.flora, at, reach, trees);
    }
    let within = |transform: &Transform| {
        places
            .iter()
            .any(|(at, reach)| transform.translation.distance(*at) < reach + 2.0)
    };
    let close = sources
        .others
        .iter()
        .filter(|(_, transform)| within(transform));
    let close = close
        .enumerate()
        .map(|(i, (body, transform))| (OTHERS + i as u32, body, transform));
    mover_world::others(close, kind, others);
}

/// The build a body is at decides what blocks the way: the player's own, or someone else's,
/// such as a house in the village. Builds stand well apart, so it is the one the body is at
/// or nearest. With no building in the game, there is none.
fn at_base<'a>(
    at: Vec3,
    site: Option<&'a Site>,
    mut builds: impl Iterator<Item = &'a build::Build>,
) -> Option<&'a build::Build> {
    builds
        .find(|built| built.covers(at, 3.0))
        .or(site.map(|site| &site.build))
}

type Walked<'a> = (
    Entity,
    &'a mut Transform,
    &'a mut Motion,
    &'a mut Stance,
    Option<&'a mut Stamina>,
    &'a View,
    &'a Intent,
    Has<Downed>,
    Option<&'a build::hammer::Hammer>,
    Option<&'a mut Stepping>,
);

/// Props that appeared, moved or were shown or hidden since the walk last ran.
type PropsChanged = (
    With<bodies::Solid>,
    Without<Player>,
    Or<(Changed<Transform>, Changed<Visibility>)>,
);

/// Moves every player by what they mean to do, through the mover: held up by the ground and
/// their base, stopped by walls, props, trees, bodies and roofs, floated by deep water. A body
/// with no stamina, as in the walker on its own, never tires.
#[allow(clippy::too_many_arguments)]
fn walk(
    time: Res<Time>,
    planet: Res<Planet>,
    site: Option<Res<Site>>,
    builds: Query<&'static build::Built>,
    flora: Res<flora::Obstacles>,
    solids: Query<
        (
            Entity,
            &'static bodies::Solid,
            &'static Transform,
            &'static Visibility,
        ),
        Without<Player>,
    >,
    moved: Query<(), PropsChanged>,
    mut gone: RemovedComponents<bodies::Solid>,
    others: Query<(&'static bodies::Body, &'static Transform), Without<Player>>,
    mut movers: ResMut<Movers>,
    mut commands: Commands,
    mut players: Query<Walked, With<Player>>,
    mut fell: MessageWriter<Fell>,
    mut drowned: MessageWriter<Drowned>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let movers = &mut *movers;
    let gone = gone.read().count() > 0;
    if gone || !moved.is_empty() {
        let filed = solids
            .iter()
            .map(|(entity, solid, transform, _)| (entity, solid, transform));
        movers.props.file(filed);
    }
    let lists = &mut movers.lists;
    lists.places.clear();
    lists
        .places
        .extend(players.iter().map(|(_, transform, motion, ..)| {
            let speed = motion.velocity.length() + motion.rising.abs();
            (transform.translation, REACH_M + speed * dt)
        }));
    let kind = movers.kind;
    let site = site.as_deref();
    let sources = Sources {
        props: &movers.props,
        solids: &solids,
        site,
        builds: &builds,
        flora: &flora,
        others: &others,
    };
    let mut trees = recycle(std::mem::take(&mut lists.trees));
    gather(&sources, lists, &mut trees, &kind);
    movers
        .memo
        .keep_within(MEMO_MOST * movers.lists.places.len().max(1));
    let memo = RefCell::new(std::mem::take(&mut movers.memo));
    {
        let field = Sdf {
            distance: |at: DVec3| planet.distance_in(at, &mut memo.borrow_mut()),
            steepest: planet.steepest(),
        };
        let water = |at: Vec3| mover_world::water(&planet, at);
        let blocks = Obstacles {
            obstacles: &movers.lists.blocks,
        };
        let trees = Obstacles { obstacles: &trees };
        let people = Capsules {
            bodies: &movers.lists.others,
            skip: u32::MAX,
            near: None,
            gathered: None,
        };
        let rest = (&trees, &people);
        let things = (&blocks, &rest);
        let ground = (&field, &things);
        let world = mover::World {
            ground: &ground,
            water: &water,
            gravity: GRAVITY,
            centre: Vec3::ZERO,
        };
        movers.lists.intents.clear();
        movers.lists.wishes.clear();
        movers.lists.seen.clear();
        for (entity, transform, motion, mut stance, stamina, view, intent, downed, hammer, _) in
            &mut players
        {
            movers.lists.seen.insert(entity);
            let feet = transform.translation;
            let id = *movers.ids.entry(entity).or_insert_with(|| {
                movers.next += 1;
                movers
                    .bodies
                    .add(movers.next - 1, kind, feet, motion.facing, &world);
                movers.next - 1
            });
            let body = movers.bodies.get(id).expect("each player's body");
            // Moved outside the walk (a respawn, a warp, a nudge): it is put where it was moved
            // to, still if it went far or lies where it fell.
            let moved = body.feet.distance(feet);
            if moved > 1.0e-4 || (downed && body.velocity != Vec3::ZERO) {
                let velocity = if moved > 1.0 || downed {
                    Vec3::ZERO
                } else {
                    body.velocity
                };
                movers.bodies.place(id, feet, velocity, &world);
            }
            // Lying where you fell, you do not move.
            if downed {
                continue;
            }
            let asked = Asked {
                intent,
                view,
                up: feet.normalize(),
                on_ground: motion.on_ground,
                part_in_hand: hammer.is_some_and(|hammer| hammer.part_in_hand),
            };
            let asked = ask(asked, &mut stance, stamina.map(Mut::into_inner), &kind, dt);
            movers.lists.wishes.push((entity, asked.wish));
            movers.lists.intents.push((id, asked));
        }
        movers.bodies.tick(&world, &movers.lists.intents, dt);
    }
    movers.memo = memo.into_inner();
    movers.lists.trees = recycle(trees);
    let Movers {
        bodies, ids, lists, ..
    } = &mut *movers;
    ids.retain(|entity, id| {
        let here = lists.seen.contains(entity);
        if !here {
            bodies.remove(*id);
        }
        here
    });
    for (entity, mut transform, mut motion, mut stance, stamina, view, _, downed, .., stepping) in
        &mut players
    {
        let Some(body) = movers.body(entity) else {
            continue;
        };
        if downed {
            motion.velocity = Vec3::ZERO;
            motion.rising = 0.0;
            continue;
        }
        if body.fall > 0.0 {
            fell.write(Fell {
                body: entity,
                harm: body.fall,
            });
        }
        if body.drown > 0.0 {
            drowned.write(Drowned {
                body: entity,
                harm: body.drown,
            });
        }
        if body.jumped
            && let Some(mut stamina) = stamina
        {
            stamina.spend(JUMP_STAMINA);
        }
        match stepping {
            Some(mut stepping) => stepping.step(body.stepped),
            None => {
                commands.entity(entity).insert(Stepping::default());
            }
        }
        let site_here = at_base(body.feet, site, builds.iter().map(|built| &built.0));
        indoors(&mut motion, site_here, body.feet);
        let wish = movers
            .lists
            .wishes
            .iter()
            .find(|(of, _)| *of == entity)
            .map_or(Vec3::ZERO, |(_, wish)| *wish);
        stance.since_acting += dt;
        shown(&mut motion, &stance, view, wish, body, dt);
        *transform = Transform::from_translation(body.feet).looking_to(motion.facing, body.up);
    }
}

/// Whether the player is under their base's roof, said when it changes.
fn indoors(motion: &mut Motion, site: Option<&build::Build>, feet: Vec3) {
    let indoors = site.is_some_and(|site| {
        let local = site.frame.compute_affine().inverse().transform_point3(feet);
        site.walkable().ceiling(local).is_some()
    });
    if indoors != motion.indoors {
        motion.indoors = indoors;
        info!(
            "Player is now {}.",
            if indoors {
                "inside the base"
            } else {
                "outside"
            }
        );
    }
}

/// What the rest of the game reads of a player's motion, from their body: their speed over
/// the ground and away from the centre, whether they stand, how long they have been in the
/// air (sliding down a steep face or floating is not falling), and the way the figure faces:
/// as it goes, or as it works.
fn shown(
    motion: &mut Motion,
    stance: &Stance,
    view: &View,
    wish: Vec3,
    body: &mover::Body,
    dt: f32,
) {
    let up = body.up;
    motion.velocity = body.velocity.reject_from(up);
    motion.rising = body.velocity.dot(up);
    motion.on_ground = body.ground == Ground::Grounded;
    motion.airborne = match body.ground {
        Ground::Airborne => motion.airborne + dt,
        _ => 0.0,
    };
    let toward = toward(stance, view.forward, wish, up);
    let turn = 1.0 - libm::expf(-dt * 12.0);
    // Turning right round has no shorter way to go, so it is given one.
    let about_face = toward != Vec3::ZERO && motion.facing.dot(toward) < -0.98;
    let from = if about_face {
        motion.facing + motion.facing.cross(up) * 0.3
    } else {
        motion.facing
    };
    motion.facing = from
        .lerp(toward, if toward == Vec3::ZERO { 0.0 } else { turn })
        .reject_from(up)
        .normalize_or(view.forward);
}

#[cfg(test)]
#[path = "movement_mover.rs"]
mod mover_tests;

#[cfg(test)]
#[path = "movement_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "movement_bench.rs"]
mod bench;
