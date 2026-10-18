//! Creatures (game design 001, creatures.md): wildlife with routines, and monsters.
//!
//! What each kind is like is data, in `assets/data/creatures.ron` (`species`, D136); its
//! temper decides how it behaves. Shy ones, such as deer, graze, wander near home and run
//! from anyone who comes close or hurts them; dead, they lie where they fell, and F with
//! the right tool in your pack butchers one for what its kind gives. Hostile ones, such as
//! spiders, wait at home, chase whoever comes near, bite within reach, and give up and go
//! home if led too far.
//!
//! Behaviour is a few small states per creature, run in full for every one; at this
//! scale that is cheap. Detail by distance comes when there are many (creatures.md).

mod gait;
mod mind;

use super::Player;
use super::bodies::Body;
use super::flora::ClearAround;
use super::interact::{CanInteract, Interactable, Interacted};
use super::inventory::Belongings;
use super::kinds::CutSkill;
use super::lod::Detail;
use super::skills::{Did, ONCE};
pub use super::species::Species;
use super::species::{Bestiary, Lives, Moves, Refuge};
use super::tick::{Sim, Smoothed};
use super::told::Told;
use crate::loading::Loading;
use crate::planet::Planet;
use crate::start::Start;
use bevy::{
    prelude::*,
    world_serialization::{WorldAsset, WorldInstanceReady},
};
pub use gait::Pace;
use mind::think;
use std::time::Duration;
use terra_gen::{Rng, Seed};
use terra_world::flora::Trunk;

const BLEND: Duration = Duration::from_millis(200);
/// A creature casts a shadow from so wide a body, in metres of its radius.
const CASTS_SHADOW_FROM_M: f32 = 0.2;
/// How much bigger or smaller one animal is than another of its kind, a share either way.
const SIZES: f32 = 0.09;
/// How fast one that does not fly from place to place flies when it takes wing to flee.
const TAKES_WING_MPS: f32 = 11.0;

/// Two numbers from 0 to 1 that are an animal's own, from where it was put out: its size,
/// the beat its idle and grazing keep, its coat (`coats`).
pub(super) fn luck(home: Vec3) -> Vec2 {
    let bits = |v: f32| (v * 16.0).round() as i64 as u64;
    let h = terra_gen::hash::splitmix64(
        bits(home.x)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(bits(home.y).wrapping_mul(0xC2B2_AE3D_27D4_EB4F))
            .wrapping_add(bits(home.z).wrapping_mul(0x1656_67B1_9E37_79F9)),
    );
    Vec2::new(
        (h & 0xFFFF) as f32 / 65535.0,
        ((h >> 16) & 0xFFFF) as f32 / 65535.0,
    )
}

/// What a creature is doing.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Idle(f32),
    Graze(f32),
    Wander(Vec3),
    Flee(f32),
    Chase,
    /// Biting, and how far through the bite.
    Attack(f32),
    Return,
    /// Dead, and for how long.
    Dead(f32),
    /// On the wing from one place to another, so far along the way (0 to 1).
    Fly {
        from: Vec3,
        to: Vec3,
        along: f32,
    },
    /// To a trunk and up it to so high, or down it to the ground.
    Climb {
        trunk: Trunk,
        height: f32,
    },
    /// Holding still up a trunk, for at least so long more.
    Perch {
        trunk: Trunk,
        left: f32,
    },
    /// Sitting up in a crown, a flier, for so long more.
    Roost {
        at: Vec3,
        left: f32,
    },
}

/// Something every shy creature runs from when it comes within so many metres, as from a
/// player it is shy of: a dragon on the wing, a fire (D382, D377). A shy creature startled by
/// it flees as its kind does, a bird taking wing away from it.
#[derive(Component, Clone, Copy, Debug)]
pub struct Frightful {
    pub within_m: f32,
}

#[derive(Component)]
#[require(Smoothed, Detail)]
pub struct Creature {
    pub species: Species,
    pub health: f32,
    pub state: State,
    home: Vec3,
    facing: Vec3,
    /// Hurt lately, which sends wildlife running.
    pub hurt: bool,
    /// The player it is chasing, biting or running from, if any.
    quarry: Option<Entity>,
    /// How high up a trunk it is, and which way from the trunk it clings.
    aloft: f32,
    side: Vec3,
    /// The way it runs while it flees, chosen when it was startled (`bolt`); none when it was
    /// hurt with nobody about, and runs the way it faces.
    bolt: Vec3,
    /// Why it is on the move, which sets how fast (`gait`).
    pace: Pace,
}

impl Creature {
    /// Whether it has not noticed anyone: resting, grazing or wandering, and unhurt, rather
    /// than fleeing, chasing or fighting. A blow it does not see coming lands harder (D119).
    pub fn unaware(&self) -> bool {
        !self.hurt
            && matches!(
                self.state,
                State::Idle(_) | State::Graze(_) | State::Wander(_)
            )
    }

    pub fn alive(&self) -> bool {
        !matches!(self.state, State::Dead(_))
    }

    /// Where it was placed, which stays the same for as long as it lives and is the same
    /// in every run of the same world, so it tells one creature from another.
    pub fn home(&self) -> Vec3 {
        self.home
    }

    /// Sitting up off the ground at a point for so long, as a bird put out on a roof's ridge.
    pub fn roost(&mut self, at: Vec3, left: f32) {
        self.state = State::Roost { at, left };
    }

    /// Going a way for so long, calmly, as one already put up by something and crossing a
    /// player's way (wildlife's crossings), until they come near.
    pub fn dash(&mut self, way: Vec3, for_s: f32) {
        (self.bolt, self.pace) = (way, Pace::Calm);
        self.facing = way;
        self.state = State::Flee(for_s);
    }

    /// On the wing from one place to another, calmly.
    pub fn fly(&mut self, from: Vec3, to: Vec3) {
        self.pace = Pace::Calm;
        self.state = State::Fly {
            from,
            to,
            along: 0.0,
        };
    }
}

#[cfg(test)]
impl Creature {
    /// One of a kind at its home, resting and unhurt, for tests.
    pub fn resting(species: Species, health: f32, home: Vec3) -> Self {
        Self {
            species,
            health,
            state: State::Idle(1.0),
            home,
            facing: Vec3::X,
            hurt: false,
            quarry: None,
            aloft: 0.0,
            side: Vec3::X,
            bolt: Vec3::ZERO,
            pace: Pace::Startled,
        }
    }
}

/// Sent when a creature lands a blow on a player.
#[derive(Message, Clone, Copy)]
pub struct Bitten {
    pub player: Entity,
    pub damage: f32,
}

/// The animations of each species, ready to play.
#[derive(Resource)]
pub(super) struct Kinds {
    models: Vec<(Species, Handle<Gltf>)>,
    ready: Vec<(Species, Handle<WorldAsset>, Handle<AnimationGraph>, Clips)>,
}

impl Kinds {
    /// Whether every kind's model is in and its clips ready to play.
    pub(super) fn ready(&self) -> bool {
        !self.ready.is_empty()
    }
}

#[derive(Clone, Copy)]
struct Clips {
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    run: AnimationNodeIndex,
    eat: AnimationNodeIndex,
    attack: AnimationNodeIndex,
    death: AnimationNodeIndex,
    fly: AnimationNodeIndex,
}

/// On a creature's animation player: whose it is, and what it plays.
#[derive(Component)]
struct Animates {
    creature: Entity,
    clips: Clips,
    playing: Option<AnimationNodeIndex>,
}

#[derive(Resource)]
struct Dice(Rng);

/// Where magic pools and the spiders live (D103).
#[derive(Resource)]
pub struct Lair(pub Vec3);

/// Asks for a creature to form at a place: where magic pools makes monsters (D103).
#[derive(Message, Clone, Copy)]
pub struct Form(pub Species, pub Vec3);

pub fn plugin(app: &mut App) {
    app.add_message::<Bitten>()
        .add_message::<Form>()
        .insert_resource(Dice(Seed(31).derive("creatures").rng()))
        .add_systems(Startup, load)
        .add_systems(
            Update,
            (
                prepare.run_if(not(resource_exists::<Populated>)),
                animate,
                form.run_if(resource_exists::<Populated>),
            ),
        )
        .add_systems(
            FixedUpdate,
            ((think, walk).chain(), butcher).in_set(Sim::World),
        );
}

#[derive(Resource)]
struct Populated;

fn load(
    mut commands: Commands,
    assets: Res<AssetServer>,
    bestiary: Res<Bestiary>,
    mut loading: ResMut<Loading>,
) {
    commands.insert_resource(Kinds {
        models: bestiary
            .all()
            .map(|species| {
                let model = assets.load(&bestiary.get(species).model.0);
                loading.wait_for(&model);
                (species, model)
            })
            .collect(),
        ready: Vec::new(),
    });
}

/// Once the models are in, builds their animation graphs and puts the creatures out.
#[allow(clippy::too_many_arguments)]
fn prepare(
    mut commands: Commands,
    assets: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    start: Res<Start>,
    planet: Res<Planet>,
    mut clear: MessageWriter<ClearAround>,
    mut kinds: ResMut<Kinds>,
    bestiary: Res<Bestiary>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut dice: ResMut<Dice>,
    options: Res<crate::shell::Options>,
) {
    if kinds
        .models
        .iter()
        .any(|(_, model)| !assets.is_loaded_with_dependencies(model))
    {
        return;
    }
    let mut ready = Vec::new();
    for (species, model) in &kinds.models {
        let Some(gltf) = gltfs.get(model) else { return };
        let Some(scene) = gltf.default_scene.clone() else {
            error!("{} has no scene.", bestiary.name(*species));
            return;
        };
        let about = &bestiary.get(*species).clips;
        let clip = |name: &str| gltf.named_animations.get(name).cloned();
        let names = [
            &about.idle,
            &about.walk,
            &about.run,
            about.eat.as_ref().unwrap_or(&about.idle),
            &about.attack,
            &about.death,
            about.fly.as_ref().unwrap_or(&about.run),
        ];
        let Some(clips) = names
            .iter()
            .map(|name| clip(name))
            .collect::<Option<Vec<_>>>()
        else {
            error!("{} is missing an animation.", bestiary.name(*species));
            return;
        };
        let (graph, nodes) = AnimationGraph::from_clips(clips);
        ready.push((
            *species,
            scene,
            graphs.add(graph),
            Clips {
                idle: nodes[0],
                walk: nodes[1],
                run: nodes[2],
                eat: nodes[3],
                attack: nodes[4],
                death: nodes[5],
                fly: nodes[6],
            },
        ));
    }
    kinds.ready = ready;
    // The yard, the flat test level, has no herds, and so no lair and no village beside it;
    // a forest grown on it has its wildlife all the same (`wildlife`).
    if options.yard() {
        commands.insert_resource(Populated);
        return;
    }

    // Herds in the meadow, and those that keep a lair in a hollow further off, each as
    // many as its kind says.
    let on_ground = |east: f32, south: f32| planet.ground_at(start.at(east, south));
    let dry = |point: Vec3| point.length() > planet.sea_radius_m() + 0.6;
    let lair = [(70.0, -60.0), (-80.0, -70.0), (90.0, 20.0), (-90.0, 10.0)]
        .into_iter()
        .map(|(east, south)| on_ground(east, south))
        .find(|&point| dry(point));
    // Where they like it first, then the nearest dry ground on rings further out, so a
    // world with water at all four still has a herd, away from the lair.
    let rings = (3..=9).flat_map(|ring| {
        (0..16).map(move |bearing| {
            let (radius, angle) = (
                10.0 * ring as f32,
                bearing as f32 * std::f32::consts::TAU / 16.0,
            );
            (radius * libm::sinf(angle), radius * libm::cosf(angle))
        })
    });
    let herd = [(-45.0, 30.0), (-55.0, -20.0), (40.0, 45.0), (-30.0, 60.0)]
        .into_iter()
        .chain(rings)
        .map(|(east, south)| on_ground(east, south))
        .find(|&point| dry(point) && lair.is_none_or(|lair| point.distance(lair) > 40.0));
    let local = |point: Vec3| start.local(point);
    if let Some(home) = herd {
        // They graze in the open, as the village says.
        clear.write(ClearAround(home, 16.0));
    }
    if let Some(lair) = lair {
        commands.insert_resource(Lair(lair));
    }
    for species in bestiary.all() {
        let name = bestiary.name(species);
        match (bestiary.get(species).lives, herd, lair) {
            (Lives::Herd(count), Some(home), _) => {
                let at = local(home);
                info!(
                    "{name} graze {:.0} m east and {:.0} m south of the start.",
                    at.x, at.z
                );
                for _ in 0..count {
                    let up = home.normalize();
                    let side = up.any_orthonormal_vector();
                    let offset =
                        Quat::from_axis_angle(up, dice.0.range(0.0, std::f32::consts::TAU))
                            * side
                            * dice.0.range(1.0, 7.0);
                    spawn(
                        &mut commands,
                        (&kinds, &bestiary),
                        &planet,
                        species,
                        home + offset,
                    );
                }
            }
            (Lives::Lair(count), _, Some(lair)) => {
                let at = local(lair);
                info!(
                    "{name} nest {:.0} m east and {:.0} m south of the start.",
                    at.x, at.z
                );
                // One at the middle, the rest three metres out, a quarter turn apart.
                let ways = [Vec3::ZERO, Vec3::X, Vec3::Z, Vec3::NEG_X, Vec3::NEG_Z];
                for index in 0..count as usize {
                    let out = ways[index % ways.len()] * 3.0 * (1 + index / ways.len()) as f32;
                    let up = lair.normalize();
                    spawn(
                        &mut commands,
                        (&kinds, &bestiary),
                        &planet,
                        species,
                        lair + out.reject_from(up),
                    );
                }
            }
            _ => {}
        }
    }
    commands.insert_resource(Populated);
}

/// Forms the creatures asked for.
fn form(
    mut commands: Commands,
    kinds: Res<Kinds>,
    bestiary: Res<Bestiary>,
    planet: Res<Planet>,
    mut asked: MessageReader<Form>,
) {
    for &Form(species, at) in asked.read() {
        spawn(&mut commands, (&kinds, &bestiary), &planet, species, at);
    }
}

/// Puts out a creature of a kind on the ground at a place, once its kind's model is ready.
pub(super) fn spawn(
    commands: &mut Commands,
    (kinds, bestiary): (&Kinds, &Bestiary),
    planet: &Planet,
    species: Species,
    at: Vec3,
) -> Option<Entity> {
    let (_, scene, graph, clips) = kinds.ready.iter().find(|(each, ..)| *each == species)?;
    let about = bestiary.get(species);
    // Drawn as far as something its size is seen: a songbird some tens of metres, a heron
    // across a lake (VisibilityRange, Bevy's distance culling, on each of its meshes).
    let seen_m = about.seen_m();
    // A small animal is a few of the sun's shadow texels across, so it shadowed itself whole
    // and was lit by the blue sky alone: a grey pigeon or heron in the sun read cobalt (the
    // review of 2026-10-16). Only those big enough to resolve cast a shadow.
    let casts = about.body.radius >= CASTS_SHADOW_FROM_M;
    let feet = planet.ground_at(at);
    let up = feet.normalize();
    let facing = up.any_orthonormal_vector();
    let creature = commands
        .spawn((
            Name::new(about.name.clone()),
            Creature {
                species,
                health: about.health,
                state: State::Idle(1.0),
                home: feet,
                facing,
                hurt: false,
                quarry: None,
                aloft: 0.0,
                side: Vec3::X,
                bolt: Vec3::ZERO,
                pace: Pace::Startled,
            },
            bestiary.body(species),
            Transform::from_translation(feet).looking_to(facing, up),
            Visibility::default(),
        ))
        .id();
    let (graph, clips) = (graph.clone(), *clips);
    // No two of a kind quite the same size.
    let size = 1.0 + (luck(feet).y - 0.5) * 2.0 * SIZES;
    commands
        .spawn((
            WorldAssetRoot(scene.clone()),
            // The models face +Z; things here face -Z.
            Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI))
                .with_scale(Vec3::splat(about.model.1 * size)),
            ChildOf(creature),
        ))
        .observe(
            move |ready: On<WorldInstanceReady>,
                  mut commands: Commands,
                  children: Query<&Children>,
                  players: Query<(), With<AnimationPlayer>>,
                  meshes: Query<(), With<Mesh3d>>| {
                for mesh in children
                    .iter_descendants(ready.entity)
                    .filter(|&child| meshes.contains(child))
                {
                    let mut mesh = commands.entity(mesh);
                    mesh.insert(bevy::camera::visibility::VisibilityRange {
                        start_margin: 0.0..0.0,
                        end_margin: seen_m..seen_m + 4.0,
                        use_aabb: false,
                    });
                    if !casts {
                        mesh.insert(bevy::light::NotShadowCaster);
                    }
                }
                let Some(animator) = children
                    .iter_descendants(ready.entity)
                    .find(|&child| players.contains(child))
                else {
                    return;
                };
                commands.entity(animator).insert((
                    AnimationGraphHandle(graph.clone()),
                    AnimationTransitions::new(),
                    Animates {
                        creature,
                        clips,
                        playing: None,
                    },
                ));
            },
        );
    Some(creature)
}

/// How far a startled climber looks for a trunk, and how fast it goes up one.
const CLIMB_REACH_M: f32 = 14.0;
const CLIMB_MPS: f32 = 2.2;
/// How far from a trunk's bark a climber's middle is.
const CLINGS_M: f32 = 0.06;

/// Moves each creature by what it is doing: over the ground, afloat, on the wing or up a
/// trunk.
fn walk(
    time: Res<Time>,
    bestiary: Res<Bestiary>,
    planet: Res<Planet>,
    players: Query<&Transform, With<Player>>,
    mut creatures: Query<(&mut Creature, &mut Transform, &Detail), Without<Player>>,
) {
    let dt = time.delta_secs().min(0.1);
    for (mut creature, mut at, detail) in &mut creatures {
        if !detail.moves() {
            continue;
        }
        let here = at.translation;
        let up = here.normalize();
        let about = bestiary.get(creature.species);
        let toward = |point: Vec3| (point - here).reject_from(up).normalize_or_zero();
        let scale = at.scale;
        match creature.state {
            State::Fly { from, to, along } => {
                let speed = creature.pace.flies_mps(match about.moves {
                    Moves::Flies(mps) => mps,
                    // A swimmer that takes to the air when startled, as a duck does.
                    _ if matches!(about.refuge, Refuge::Flies(..)) => TAKES_WING_MPS,
                    _ => about.run_mps,
                });
                let length = (to - from).length().max(0.5);
                let along = (along + speed * dt / length).min(1.0);
                // Up off the ground and down onto it again, as high over the way as a tenth of
                // its length and never lower than the ground.
                let s = along;
                let lift = (length * 0.12).clamp(1.5, 14.0) * libm::sinf(std::f32::consts::PI * s);
                let line = from.lerp(to, s);
                let ground = planet.ground_at(line);
                let high = (line.length() + lift)
                    .max(ground.length() + 0.4 * libm::sinf(std::f32::consts::PI * s));
                let point = line.normalize() * high;
                let ahead = (to - from)
                    .reject_from(point.normalize())
                    .normalize_or(creature.facing);
                creature.facing = ahead;
                creature.state = State::Fly { from, to, along };
                *at = Transform::from_translation(point)
                    .looking_to(ahead, point.normalize())
                    .with_scale(scale);
                continue;
            }
            State::Climb { trunk, height } => {
                let foot_up = trunk.foot.normalize();
                let off = (here - trunk.foot).reject_from(foot_up);
                let at_trunk = off.length() <= trunk.radius_m + CLINGS_M + 0.25;
                if creature.aloft <= 0.0 && !at_trunk && height > 0.0 {
                    // Across the ground to its foot first.
                    let way = toward(trunk.foot);
                    creature.facing = way;
                    let next = planet.ground_at(here + way * about.run_mps * dt);
                    *at = Transform::from_translation(next)
                        .looking_to(way, next.normalize())
                        .with_scale(scale);
                    continue;
                }
                if creature.aloft <= 0.0 && height > 0.0 {
                    creature.side = off.normalize_or(-creature.facing);
                }
                let climb = CLIMB_MPS * dt;
                creature.aloft = if height > creature.aloft {
                    (creature.aloft + climb).min(height)
                } else {
                    (creature.aloft - climb).max(0.0)
                };
                let going_up = height > creature.aloft || height > 0.0;
                cling(&mut at, &creature, trunk, going_up);
                if creature.aloft <= 0.0 && height <= 0.0 {
                    // Down: on the ground again, facing away from the trunk.
                    let ground =
                        planet.ground_at(trunk.foot + creature.side * (trunk.radius_m + 0.3));
                    creature.facing = creature.side;
                    *at = Transform::from_translation(ground)
                        .looking_to(creature.side, ground.normalize())
                        .with_scale(scale);
                }
                continue;
            }
            State::Perch { trunk, .. } => {
                cling(&mut at, &creature, trunk, true);
                continue;
            }
            State::Roost { at: sits, .. } => {
                if at.translation != sits {
                    *at = Transform::from_translation(sits)
                        .looking_to(creature.facing, sits.normalize())
                        .with_scale(scale);
                }
                continue;
            }
            _ => {}
        }
        let quarry = creature
            .quarry
            .and_then(|player| players.get(player).ok())
            .map(|at| toward(at.translation));
        let (direction, speed) = match (creature.state, quarry) {
            (State::Wander(to), _) => (toward(to), about.walk_mps),
            (State::Return, _) => (toward(creature.home), about.walk_mps),
            // The way it broke when startled; hurt from afar with nobody near, the way it
            // faces.
            (State::Flee(_), quarry) => (
                if creature.bolt != Vec3::ZERO {
                    creature.bolt.reject_from(up).normalize_or(creature.facing)
                } else {
                    quarry.map_or(creature.facing, |toward| -toward)
                },
                creature.pace.runs_mps(about),
            ),
            (State::Chase, Some(toward)) => (toward, about.run_mps),
            (State::Attack(_), Some(toward)) => (toward, 0.0),
            _ => (Vec3::ZERO, 0.0),
        };
        if direction != Vec3::ZERO {
            let turn = 1.0 - libm::expf(-dt * 6.0);
            creature.facing = creature
                .facing
                .lerp(direction, turn)
                .reject_from(up)
                .normalize_or(direction);
        }
        let mut next = here + direction * speed * dt;
        // Each keeps to its own: walkers out of water deeper than they wade, swimmers in it.
        let depth = |point: Vec3| planet.water_radius_m(point) - planet.ground_at(point).length();
        let swims = matches!(about.moves, Moves::Swims(_));
        let kept = if swims {
            depth(next) > 0.3
        } else if about.wades_m > 0.0 {
            depth(next) < about.wades_m
        } else {
            depth(next) < -0.2
        };
        if !kept {
            next = here;
            if let State::Wander(_) = creature.state {
                creature.state = State::Idle(1.0);
            }
        }
        let place = match about.moves {
            // Afloat: on the water's surface, its feet under it.
            Moves::Swims(feet_m) if depth(next) > 0.0 => {
                next.normalize() * (planet.water_radius_m(next) - feet_m)
            }
            _ => planet.ground_at(next),
        };
        creature.aloft = 0.0;
        *at = Transform::from_translation(place)
            .looking_to(creature.facing, place.normalize())
            .with_scale(scale);
    }
}

/// A climber on a trunk at its height, belly to the bark, head up or down it.
fn cling(at: &mut Transform, creature: &Creature, trunk: Trunk, head_up: bool) {
    let up = trunk.foot.normalize();
    let side = creature
        .side
        .reject_from(up)
        .normalize_or(up.any_orthonormal_vector());
    let point = trunk.foot + up * creature.aloft + side * (trunk.radius_m + CLINGS_M);
    let along = if head_up { up } else { -up };
    *at = Transform::from_translation(point)
        .looking_to(along, side)
        .with_scale(at.scale);
}

/// Plays the animation that fits what each creature is doing, its walk and run as fast as it
/// goes over the clip's own pace, so its feet keep to the ground.
fn animate(
    bestiary: Res<Bestiary>,
    creatures: Query<&Creature>,
    mut animators: Query<(
        &mut Animates,
        &mut AnimationPlayer,
        &mut AnimationTransitions,
    )>,
) {
    for (mut animates, mut player, mut transitions) in &mut animators {
        let Ok(creature) = creatures.get(animates.creature) else {
            continue;
        };
        let about = bestiary.get(creature.species);
        let clips = animates.clips;
        // Each keeps its own beat at rest, and picks up its clips where it will, so a group
        // does not idle and graze in step.
        let own = luck(creature.home());
        let beat = 0.85 + own.y * 0.3;
        let (wanted, speed) = match creature.state {
            State::Idle(_) | State::Perch { .. } | State::Roost { .. } => (clips.idle, beat),
            State::Graze(_) => (clips.eat, beat),
            State::Wander(_) | State::Return => {
                (clips.walk, gait::on_foot(about.walk_mps, about).1)
            }
            State::Flee(_) | State::Chase => {
                let fleeing = matches!(creature.state, State::Flee(_));
                let speed = if fleeing {
                    creature.pace.runs_mps(about)
                } else {
                    about.run_mps
                };
                let (walks, rate) = gait::on_foot(speed, about);
                (if walks { clips.walk } else { clips.run }, rate)
            }
            State::Climb { .. } => (clips.run, 1.0),
            State::Fly { .. } => (clips.fly, 1.0),
            State::Attack(_) => (clips.attack, 1.0),
            State::Dead(_) => (clips.death, 1.0),
        };
        if animates.playing != Some(wanted) {
            animates.playing = Some(wanted);
            let playing = transitions.play(&mut player, wanted, BLEND);
            if !matches!(creature.state, State::Dead(_) | State::Attack(_)) {
                // Its loop wraps a seek past its end.
                playing.repeat().seek_to(own.x * 6.0);
            }
        }
        if let Some(active) = player.animation_mut(wanted) {
            active.set_speed(speed);
        }
    }
}

/// Marks the dead: one whose kind leaves a carcass can be butchered; anything lies a
/// while, then is gone.
pub fn fallen(
    commands: &mut Commands,
    bestiary: &Bestiary,
    entity: Entity,
    creature: &mut Creature,
) {
    creature.state = State::Dead(0.0);
    // The dead are walked over and round, not into.
    commands.entity(entity).remove::<Body>();
    if bestiary.carcass(creature.species).is_some() {
        commands
            .entity(entity)
            .insert(CanInteract(Interactable::Carcass));
    }
}

/// F with something carried that cuts it, such as a knife, butchers a carcass for what its
/// kind gives.
fn butcher(
    mut commands: Commands,
    mut interacted: MessageReader<Interacted>,
    mut belongings: Belongings,
    mut told: MessageWriter<Told>,
    mut did: MessageWriter<Did>,
    bestiary: Res<Bestiary>,
    bodies: Query<(Entity, &Creature)>,
) {
    for used in interacted.read() {
        let Some(entity) = used.on(Interactable::Carcass) else {
            continue;
        };
        let (Ok((_, creature)), Some(mut inventory)) =
            (bodies.get(entity), belongings.of(used.player))
        else {
            continue;
        };
        let Some(carcass) = bestiary.carcass(creature.species) else {
            continue;
        };
        let items = inventory.items();
        let cuts = |kind| {
            items
                .tool(kind)
                .is_some_and(|tool| tool.cuts(carcass.needs).is_some())
        };
        let carries = inventory
            .backpack()
            .iter()
            .any(|placed| cuts(placed.item.kind));
        if !carries {
            // Say what would do, such as a knife.
            let needed = items
                .kinds()
                .find(|&kind| cuts(kind))
                .map_or("tool", |kind| items.name(kind))
                .to_lowercase();
            told.write(Told::new(
                used.player,
                format!("You need a {needed} to butcher it"),
            ));
            continue;
        }
        let given: Vec<String> = carcass
            .gives
            .iter()
            .map(|&(kind, count)| {
                let given = inventory.give(kind, count);
                format!("{given} {}", inventory.items().name(kind))
            })
            .collect();
        info!("Butchered it for {}.", given.join(", "));
        told.write(Told::new(used.player, "Butchered it"));
        did.write(Did(used.player, carcass.needs.skill(), ONCE));
        commands.entity(entity).despawn();
    }
    // Bodies left long enough are gone.
    for (entity, creature) in &bodies {
        if let State::Dead(lying) = creature.state
            && lying > bestiary.get(creature.species).lies_s
        {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::gait::bolts_within;
    use super::mind::{ROOSTS_OVER_M, SENSE_M, nearest};
    use super::*;
    use crate::play::species::Temper;
    use crate::play::tick::Ticks;
    use crate::play::{Downed, test_player};
    use crate::spatial::SpatialHash;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn a_creature_about_its_day_is_unaware_and_one_that_has_noticed_is_not() {
        let mut creature = Creature {
            species: Bestiary::read(&Default::default()).named("Deer").unwrap(),
            health: 10.0,
            state: State::Graze(1.0),
            home: Vec3::ZERO,
            facing: Vec3::X,
            hurt: false,
            quarry: None,
            aloft: 0.0,
            side: Vec3::X,
            bolt: Vec3::ZERO,
            pace: Pace::Startled,
        };
        assert!(creature.unaware());
        creature.state = State::Flee(2.0);
        assert!(!creature.unaware());
        creature.state = State::Idle(1.0);
        creature.hurt = true;
        assert!(!creature.unaware());
    }

    fn spider(world: &mut World, at: Vec3) -> Entity {
        world
            .spawn((
                Creature {
                    species: world.resource::<Bestiary>().named("Spider").unwrap(),
                    health: 10.0,
                    state: State::Idle(1.0),
                    home: at,
                    facing: Vec3::X,
                    hurt: false,
                    quarry: None,
                    aloft: 0.0,
                    side: Vec3::X,
                    bolt: Vec3::ZERO,
                    pace: Pace::Startled,
                },
                Transform::from_translation(at),
            ))
            .id()
    }

    fn world() -> World {
        let mut world = World::new();
        world.init_resource::<Time>();
        world.init_resource::<Messages<Bitten>>();
        world.insert_resource(Dice(Seed(1).derive("test").rng()));
        world.insert_resource(Bestiary::read(&Default::default()));
        world.init_resource::<Ticks>();
        // What a creature asks of the world round it: the ground, its trees.
        let planet = Planet::smooth(terra_core::PlanetSpec {
            seed: Seed(1),
            radius_m: 2000.0,
        });
        world.insert_resource(Start::choose(&planet, true));
        world.insert_resource(planet);
        world.init_resource::<terra_world::flora::Trunks>();
        world.init_resource::<terra_world::flora::Obstacles>();
        world.init_resource::<super::super::wildlife::Warmth>();
        world.init_resource::<super::super::wildlife::Perches>();
        world.init_resource::<super::super::wildlife::Thickets>();
        world.init_resource::<terra_world::daylight::Clock>();
        world
    }

    #[test]
    fn a_spider_goes_for_the_nearest_player_who_is_up() {
        let mut world = world();
        let ground = Vec3::Y * 2000.0;
        let near = test_player(&mut world, ground + Vec3::X * 5.0);
        let far = test_player(&mut world, ground + Vec3::X * 10.0);
        let spider = spider(&mut world, ground);
        world.run_system_once(think).unwrap();
        let chased = world.get::<Creature>(spider).unwrap();
        assert_eq!(chased.quarry, Some(near));
        assert_eq!(chased.state, State::Chase);
        // Down, the nearer one is passed over for the other.
        world.entity_mut(near).insert(Downed { lying_s: 0.0 });
        world.run_system_once(think).unwrap();
        assert_eq!(world.get::<Creature>(spider).unwrap().quarry, Some(far));
    }

    #[test]
    fn a_bite_lands_on_the_player_bitten() {
        let mut world = world();
        let ground = Vec3::Y * 2000.0;
        let _far = test_player(&mut world, ground + Vec3::X * 20.0);
        let near = test_player(&mut world, ground + Vec3::X * 1.5);
        let spider = spider(&mut world, ground);
        let Temper::Hostile { bite_at_s, .. } = temper(&world, spider) else {
            panic!("a spider is hostile");
        };
        // Just short of where the bite lands, which the next moment passes.
        world.get_mut::<Creature>(spider).unwrap().state = State::Attack(bite_at_s - 0.01);
        world
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(20));
        world.run_system_once(think).unwrap();
        let bites: Vec<_> = world
            .resource_mut::<Messages<Bitten>>()
            .drain()
            .map(|bite| bite.player)
            .collect();
        assert_eq!(bites, [near]);
    }

    #[test]
    fn nobody_beyond_its_senses_is_noticed() {
        let mut awake = SpatialHash::default();
        let here = Vec3::Y * 2000.0;
        awake.insert(here + Vec3::X * (SENSE_M + 1.0), Entity::PLACEHOLDER);
        assert_eq!(nearest(&awake, here), None);
    }

    fn temper(world: &World, creature: Entity) -> Temper {
        let species = world.get::<Creature>(creature).unwrap().species;
        world.resource::<Bestiary>().get(species).temper
    }

    /// A creature of the first kind that is shy, or hostile, at home at a place.
    fn one(world: &mut World, hostile: bool, home: Vec3) -> Entity {
        let bestiary = world.resource::<Bestiary>();
        let species = bestiary
            .all()
            .find(|&species| bestiary.hostile(species) == hostile)
            .unwrap();
        world
            .spawn((
                Creature::resting(species, 10.0, home),
                Transform::from_translation(home),
            ))
            .id()
    }

    fn state(world: &World, creature: Entity) -> State {
        world.get::<Creature>(creature).unwrap().state
    }

    #[test]
    fn a_shy_creature_runs_from_anyone_near() {
        let ground = Vec3::Y * 2000.0;
        let (mut world, mut far_world) = (world(), world());
        let near = one(&mut world, false, ground);
        let far = one(&mut far_world, false, ground);
        let Temper::Shy { flees_within_m, .. } = temper(&world, near) else {
            unreachable!()
        };
        test_player(&mut world, ground + Vec3::X * flees_within_m * 0.5);
        test_player(&mut far_world, ground + Vec3::X * (flees_within_m + 1.0));
        world.run_system_once(think).unwrap();
        far_world.run_system_once(think).unwrap();
        assert!(matches!(state(&world, near), State::Flee(_)));
        assert!(!matches!(state(&far_world, far), State::Flee(_)));
    }

    #[test]
    fn a_shy_creature_hurt_with_nobody_near_runs_and_then_settles() {
        let mut world = world();
        let deer = one(&mut world, false, Vec3::Y * 2000.0);
        world.get_mut::<Creature>(deer).unwrap().hurt = true;
        world.run_system_once(think).unwrap();
        assert!(matches!(state(&world, deer), State::Flee(left) if left > 0.0));
        assert!(!world.get::<Creature>(deer).unwrap().hurt);
        // Once it has run its course, it goes back to its day.
        world.get_mut::<Creature>(deer).unwrap().state = State::Flee(0.0);
        world.run_system_once(think).unwrap();
        assert!(world.get::<Creature>(deer).unwrap().unaware());
    }

    #[test]
    fn a_hostile_creature_led_too_far_from_home_goes_back() {
        let home = Vec3::Y * 2000.0;
        let mut world = world();
        let spider = one(&mut world, true, home);
        let Temper::Hostile {
            notices_m,
            leash_m,
            attacks_within_m,
            ..
        } = temper(&world, spider)
        else {
            unreachable!()
        };
        // Someone it would chase, near enough to notice and too far to bite.
        let quarry = Vec3::X * (notices_m + attacks_within_m) * 0.5;
        let player = test_player(&mut world, home + quarry);
        world.get_mut::<Creature>(spider).unwrap().state = State::Chase;
        world.run_system_once(think).unwrap();
        assert_eq!(
            state(&world, spider),
            State::Chase,
            "at home it gives chase"
        );
        let beyond = home + Vec3::NEG_Z * (leash_m + 1.0);
        world.get_mut::<Transform>(spider).unwrap().translation = beyond;
        world.get_mut::<Transform>(player).unwrap().translation = beyond + quarry;
        world.run_system_once(think).unwrap();
        assert_eq!(state(&world, spider), State::Return);
        // It keeps going home with nobody about, until it is there.
        world.despawn(player);
        world.run_system_once(think).unwrap();
        assert_eq!(state(&world, spider), State::Return);
        world.get_mut::<Transform>(spider).unwrap().translation = home;
        world.run_system_once(think).unwrap();
        assert_ne!(state(&world, spider), State::Return);
    }

    #[test]
    fn a_hostile_creature_goes_for_whoever_hurt_it_from_beyond_its_notice() {
        let home = Vec3::Y * 2000.0;
        let mut world = world();
        let spider = one(&mut world, true, home);
        let Temper::Hostile { notices_m, .. } = temper(&world, spider) else {
            unreachable!()
        };
        assert!(notices_m < SENSE_M);
        test_player(&mut world, home + Vec3::X * (notices_m + SENSE_M) * 0.5);
        world.run_system_once(think).unwrap();
        assert_ne!(
            state(&world, spider),
            State::Chase,
            "unhurt, it has not noticed"
        );
        world.get_mut::<Creature>(spider).unwrap().hurt = true;
        world.run_system_once(think).unwrap();
        assert_eq!(state(&world, spider), State::Chase);
    }

    /// One of a kind, named, resting at home.
    fn kind(world: &mut World, name: &str, home: Vec3) -> Entity {
        let species = world.resource::<Bestiary>().named(name).unwrap();
        world
            .spawn((
                Creature::resting(species, 2.0, home),
                Transform::from_translation(home),
            ))
            .id()
    }

    #[test]
    fn a_climber_startled_goes_up_the_nearest_trunk_and_down_once_nobody_is_near() {
        let mut world = world();
        let ground = Vec3::Y * 2000.0;
        let near = Trunk {
            foot: ground + Vec3::X * 3.0,
            radius_m: 0.3,
            height_m: 20.0,
            solid: false,
        };
        let far = Trunk {
            foot: ground + Vec3::NEG_X * 9.0,
            ..near
        };
        let mut trunks = world.resource_mut::<terra_world::flora::Trunks>();
        trunks.add(near);
        trunks.add(far);
        let squirrel = kind(&mut world, "Red squirrel", ground);
        let player = test_player(&mut world, ground + Vec3::Z * 3.0);
        world.run_system_once(think).unwrap();
        let State::Climb { trunk, height } = state(&world, squirrel) else {
            panic!("it should climb, not {:?}", state(&world, squirrel));
        };
        assert_eq!(trunk, near);
        assert!(height > 1.0 && height < near.height_m);
        // Up there, it waits while anyone is near, and comes down once they have gone.
        world.get_mut::<Creature>(squirrel).unwrap().aloft = height;
        world.run_system_once(think).unwrap();
        assert!(matches!(state(&world, squirrel), State::Perch { .. }));
        world.get_mut::<Creature>(squirrel).unwrap().state = State::Perch { trunk, left: 0.0 };
        world.run_system_once(think).unwrap();
        assert!(matches!(state(&world, squirrel), State::Perch { left, .. } if left > 0.0));
        world.despawn(player);
        world.get_mut::<Creature>(squirrel).unwrap().state = State::Perch { trunk, left: 0.0 };
        world.run_system_once(think).unwrap();
        assert_eq!(state(&world, squirrel), State::Climb { trunk, height: 0.0 });
    }

    #[test]
    fn a_bird_startled_takes_wing_away_from_whoever_startled_it() {
        let mut world = world();
        let ground = Vec3::Y * 2000.0;
        let bird = kind(&mut world, "Chaffinch", ground);
        test_player(&mut world, ground + Vec3::X * 2.0);
        world.run_system_once(think).unwrap();
        let State::Fly { from, to, .. } = state(&world, bird) else {
            panic!("it should fly, not {:?}", state(&world, bird));
        };
        assert_eq!(from, ground);
        let species = world.get::<Creature>(bird).unwrap().species;
        let Refuge::Flies(near, far) = world.resource::<Bestiary>().get(species).refuge else {
            panic!("a chaffinch flies off");
        };
        let across = (to - from).reject_from(Vec3::Y).length();
        assert!(across >= near * 0.9 && across <= far * 1.1, "{across}");
        // Away: farther from the player than it was.
        assert!(to.x < from.x, "{to} from {from}");
    }

    #[test]
    fn a_heron_someone_passes_walks_off_and_takes_wing_only_once_they_come_close() {
        let mut world = world();
        let ground = Vec3::Y * 2000.0;
        let heron = kind(&mut world, "Grey heron", ground);
        let species = world.get::<Creature>(heron).unwrap().species;
        let Temper::Shy { flees_within_m, .. } = world.resource::<Bestiary>().get(species).temper
        else {
            panic!("a heron is shy");
        };
        let player = test_player(&mut world, ground + Vec3::X * flees_within_m * 0.7);
        world.run_system_once(think).unwrap();
        assert!(
            matches!(state(&world, heron), State::Flee(_)),
            "{:?}",
            state(&world, heron)
        );
        let creature = world.get::<Creature>(heron).unwrap();
        assert_eq!(creature.pace, Pace::Wary);
        assert!(creature.bolt.x < 0.0, "not away: {}", creature.bolt);
        world.get_mut::<Transform>(player).unwrap().translation =
            ground + Vec3::X * bolts_within(flees_within_m) * 0.5;
        world.run_system_once(think).unwrap();
        assert!(
            matches!(state(&world, heron), State::Fly { .. }),
            "{:?}",
            state(&world, heron)
        );
    }

    #[test]
    fn a_bird_takes_wing_away_from_something_frightful_and_not_from_one_far_off() {
        let mut world = world();
        let ground = Vec3::Y * 2000.0;
        let bird = kind(&mut world, "Chaffinch", ground);
        let calm = kind(&mut world, "Chaffinch", ground + Vec3::Z * 500.0);
        world.spawn((
            Frightful { within_m: 30.0 },
            GlobalTransform::from_translation(ground + Vec3::X * 20.0),
        ));
        world.run_system_once(think).unwrap();
        let State::Fly { from, to, .. } = state(&world, bird) else {
            panic!(
                "it should fly from the fright, not {:?}",
                state(&world, bird)
            );
        };
        assert!(to.x < from.x, "{to} from {from}: not away from it");
        assert!(!matches!(
            state(&world, calm),
            State::Fly { .. } | State::Flee(_)
        ));
    }

    #[test]
    fn a_bird_on_the_wing_flies_its_way_and_lands_there() {
        let mut world = world();
        let ground = Vec3::Y * 2000.0;
        let bird = kind(&mut world, "Chaffinch", ground);
        let to = ground + Vec3::Z * 12.0;
        world.get_mut::<Creature>(bird).unwrap().state = State::Fly {
            from: ground,
            to,
            along: 0.0,
        };
        let mut highest: f32 = 0.0;
        for _ in 0..200 {
            world
                .resource_mut::<Time>()
                .advance_by(Duration::from_millis(16));
            world.run_system_once(walk).unwrap();
            world.run_system_once(think).unwrap();
            let at = world.get::<Transform>(bird).unwrap().translation;
            highest = highest.max(at.length() - 2000.0);
            if !matches!(state(&world, bird), State::Fly { .. }) {
                break;
            }
        }
        assert!(highest > 1.0, "it rose off the ground: {highest}");
        let at = world.get::<Transform>(bird).unwrap().translation;
        assert!(at.distance(to) < 0.5, "landed at {at}, not {to}");
        assert!(world.get::<Creature>(bird).unwrap().home().distance(to) < 0.1);
    }

    #[test]
    fn a_bird_flown_into_a_crown_sits_there_and_later_leaves_it() {
        let mut world = world();
        let ground = Vec3::Y * 2000.0;
        let bird = kind(&mut world, "Chaffinch", ground);
        let up_there = ground + Vec3::Y * 12.0 + Vec3::X * 5.0;
        world.get_mut::<Creature>(bird).unwrap().state = State::Fly {
            from: ground,
            to: up_there,
            along: 1.0,
        };
        world.run_system_once(think).unwrap();
        let State::Roost { at, left } = state(&world, bird) else {
            panic!("it should sit in the crown, not {:?}", state(&world, bird));
        };
        assert_eq!(at, up_there);
        assert!(left > 0.0);
        // While it sits it stays where it is, off the ground.
        world.run_system_once(walk).unwrap();
        let sits = world.get::<Transform>(bird).unwrap().translation;
        assert!(sits.distance(up_there) < 1e-3, "{sits}");
        // Its home is the ground under it, where it comes down to.
        let home = world.get::<Creature>(bird).unwrap().home();
        assert!((home.length() - 2000.0).abs() < 0.1, "{home}");
        world.get_mut::<Creature>(bird).unwrap().state = State::Roost {
            at: up_there,
            left: 0.0,
        };
        world.run_system_once(think).unwrap();
        assert!(
            matches!(state(&world, bird), State::Fly { from, .. } if from == up_there),
            "{:?}",
            state(&world, bird)
        );
    }

    #[test]
    fn a_startled_bird_among_trees_may_go_up_into_a_crown_away_from_the_player() {
        let mut world = world();
        let ground = Vec3::Y * 2000.0;
        for i in 0..8 {
            let trunk = Trunk {
                foot: ground + Vec3::new(-6.0 - i as f32 * 2.0, 0.0, (i % 3) as f32 * 3.0 - 3.0),
                radius_m: 0.3,
                height_m: 22.0,
                solid: false,
            };
            world
                .resource_mut::<terra_world::flora::Trunks>()
                .add(trunk);
        }
        let mut crowns = 0;
        for seed in 0..20 {
            world.insert_resource(Dice(Seed(seed).derive("test").rng()));
            let bird = kind(&mut world, "Chaffinch", ground);
            let player = test_player(&mut world, ground + Vec3::X * 2.0);
            world.run_system_once(think).unwrap();
            let State::Fly { to, .. } = state(&world, bird) else {
                panic!("it should fly, not {:?}", state(&world, bird));
            };
            assert!(to.x < ground.x, "away from the player: {to}");
            if to.length() - 2000.0 > ROOSTS_OVER_M {
                crowns += 1;
                assert!(
                    to.length() - 2000.0 > 22.0 * 0.45,
                    "up among the branches: {to}"
                );
            }
            world.despawn(bird);
            world.despawn(player);
        }
        assert!(crowns > 3 && crowns < 17, "{crowns} of 20 went up a tree");
    }
}
