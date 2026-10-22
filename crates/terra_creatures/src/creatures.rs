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

mod appear;
mod gait;
mod mind;
mod steer;
mod wake;

pub use crate::species::Species;
use crate::species::{Bestiary, Lives, Moves, Refuge};
pub use appear::{Shown, leave};
use bevy::{
    prelude::*,
    world_serialization::{WorldAsset, WorldInstanceReady},
};
pub use gait::{Pace, bolts_within, keeps_to};
use mind::think;
use std::time::Duration;
use terra_gen::{Rng, Seed};
use terra_survival::interact::{CanInteract, Interactable, Interacted};
use terra_survival::inventory::Belongings;
use terra_survival::kinds::CutSkill;
use terra_survival::skills::{Did, ONCE};
use terra_survival::told::Told;
use terra_walker::Player;
use terra_walker::bodies::Body;
use terra_world::flora::ClearAround;
use terra_world::flora::{Obstacles, Trunk, Trunks};
use terra_world::loading::Loading;
use terra_world::lod::Detail;
use terra_world::planet::Planet;
use terra_world::start::Start;
use terra_world::tick::{Sim, Smoothed};

const BLEND: Duration = Duration::from_millis(200);
/// A creature casts a shadow from so wide a body, in metres of its radius.
const CASTS_SHADOW_FROM_M: f32 = 0.2;
/// How much bigger or smaller one animal is than another of its kind, a share either way.
const SIZES: f32 = 0.09;
/// How fast one that does not fly from place to place flies when it takes wing to flee.
const TAKES_WING_MPS: f32 = 11.0;

/// Two numbers from 0 to 1 that are an animal's own, from where it was put out: its size,
/// the beat its idle and grazing keep, its coat (`coats`).
pub fn luck(home: Vec3) -> Vec2 {
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

/// An animal kept by people, as a town's are: no wild `Creature`, minded by its keepers'
/// days, but an animal all the same to what watches for animals (a film's `sighted`).
#[derive(Component, Default)]
pub struct Tame;

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

    /// Going a way for so long at a pace, as one already put up by something and crossing a
    /// player's way calmly (wildlife's crossings) until they come near, or one a film asks to
    /// see fleeing, startled.
    pub fn dash(&mut self, way: Vec3, for_s: f32, pace: Pace) {
        (self.bolt, self.pace) = (way, pace);
        self.facing = way;
        self.state = State::Flee(for_s);
    }

    /// Walking to a point, unhurried, and about its day there, as one a film asks to see
    /// grazing comes into sight.
    pub fn walk_to(&mut self, to: Vec3) {
        self.state = State::Wander(to);
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
pub struct Kinds {
    models: Vec<(Species, Handle<Gltf>)>,
    ready: Vec<(Species, Handle<WorldAsset>, Handle<AnimationGraph>, Clips)>,
}

impl Kinds {
    /// Whether every kind's model is in and its clips ready to play.
    pub fn ready(&self) -> bool {
        !self.ready.is_empty()
    }
}

#[derive(Clone, Copy)]
struct Clips {
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    /// Its trot, or its run for a kind that does not trot.
    trot: AnimationNodeIndex,
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
    // The kinds of creature, read with the other registries (D136).
    terra_survival::registries::read_with_the_rest::<Bestiary>(app);
    app.add_message::<Bitten>()
        .add_message::<Form>()
        .insert_resource(Dice(Seed(31).derive("creatures").rng()))
        .add_plugins((appear::plugin, wake::plugin))
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
    options: Res<terra_world::shell::Options>,
) {
    if kinds
        .models
        .iter()
        .any(|(_, model)| !assets.is_loaded_with_dependencies(model))
    {
        return;
    }
    let Some(ready) = animated(&kinds, &gltfs, &bestiary, &mut graphs) else {
        return;
    };
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
        .find(|&point| dry(point))
        // A demo's or film's place has none (D421): play keeps it.
        .filter(|_| !options.no_lair);
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

/// Each kind's scene, its animation graph and the clips in it, or nothing if a model
/// lacks its scene or a clip its kind asks for.
fn animated(
    kinds: &Kinds,
    gltfs: &Assets<Gltf>,
    bestiary: &Bestiary,
    graphs: &mut Assets<AnimationGraph>,
) -> Option<Vec<(Species, Handle<WorldAsset>, Handle<AnimationGraph>, Clips)>> {
    let mut ready = Vec::new();
    for (species, model) in &kinds.models {
        let gltf = gltfs.get(model)?;
        let Some(scene) = gltf.default_scene.clone() else {
            error!("{} has no scene.", bestiary.name(*species));
            return None;
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
            about.trot.as_ref().map_or(&about.run, |(trot, _)| trot),
        ];
        let Some(clips) = names
            .iter()
            .map(|name| clip(name))
            .collect::<Option<Vec<_>>>()
        else {
            error!("{} is missing an animation.", bestiary.name(*species));
            return None;
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
                trot: nodes[7],
            },
        ));
    }
    Some(ready)
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
pub fn spawn(
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
    // Each its own way: all facing one way, a fox met ahead sat square to the lens (lake
    // film r10).
    let bits = [feet.x.to_bits(), feet.y.to_bits(), feet.z.to_bits()];
    let turn = terra_gen::hash::unit(terra_gen::hash::of(&bits)) * std::f32::consts::TAU;
    let facing = Quat::from_axis_angle(up, turn) * up.any_orthonormal_vector();
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
            // Into sight over a moment, never whole in a frame.
            Shown::new(seen_m),
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
                  meshes: Query<(), With<Mesh3d>>,
                  mut shown: Query<&mut Shown>| {
                // Drawn as far into sight as it is: unseen until its fade begins.
                let range = shown
                    .get_mut(creature)
                    .map_or(appear::whole(seen_m), |mut shown| {
                        appear::meshes_in(&mut shown)
                    });
                for mesh in children
                    .iter_descendants(ready.entity)
                    .filter(|&child| meshes.contains(child))
                {
                    let mut mesh = commands.entity(mesh);
                    mesh.insert(range.clone());
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
    (trunks, obstacles): (Res<Trunks>, Res<Obstacles>),
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
                *at =
                    fly_on(&mut creature, about, &planet, (from, to, along), dt).with_scale(scale);
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
                    let next =
                        planet.ground_at(here + way * gait::keeps_to(about.run_mps, about) * dt);
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
            (State::Wander(to), _) => (toward(to), gait::keeps_to(about.walk_mps, about)),
            (State::Return, _) => (toward(creature.home), gait::keeps_to(about.walk_mps, about)),
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
            (State::Chase, Some(toward)) => (toward, gait::keeps_to(about.run_mps, about)),
            (State::Attack(_), Some(toward)) => (toward, 0.0),
            _ => (Vec3::ZERO, 0.0),
        };
        // Round the trunks, rocks and logs in its way, never through one.
        let solids = (&*trunks, &*obstacles);
        let (direction, speed) = steer::round(
            solids,
            here,
            about.body.radius,
            (creature.facing, direction, speed),
        );
        if direction != Vec3::ZERO {
            let turn = 1.0 - libm::expf(-dt * 6.0);
            creature.facing = creature
                .facing
                .lerp(direction, turn)
                .reject_from(up)
                .normalize_or(direction);
        }
        let mut next = steer::out_of(
            solids,
            here + direction * speed * dt,
            about.body.radius,
            speed,
        );
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

/// A flier on its way from one point to another, so far along: where it is a step on.
fn fly_on(
    creature: &mut Creature,
    about: &crate::species::SpeciesDef,
    planet: &Planet,
    (from, to, along): (Vec3, Vec3, f32),
    dt: f32,
) -> Transform {
    let speed = creature.pace.flies_mps(match about.moves {
        Moves::Flies(mps) => mps,
        // A swimmer that takes to the air when startled, as a duck does.
        _ if matches!(about.refuge, Refuge::Flies(..)) => TAKES_WING_MPS,
        _ => about.run_mps,
    });
    let length = (to - from).length().max(0.5);
    let along = (along + speed * dt / length).min(1.0);
    // Up off the ground and down onto it again, as high over the way as a tenth of its length
    // and never lower than the ground.
    let s = along;
    let lift = (length * 0.12).clamp(1.5, 14.0) * libm::sinf(std::f32::consts::PI * s);
    let line = from.lerp(to, s);
    let ground = planet.ground_at(line);
    let high =
        (line.length() + lift).max(ground.length() + 0.4 * libm::sinf(std::f32::consts::PI * s));
    let point = line.normalize() * high;
    let ahead = (to - from)
        .reject_from(point.normalize())
        .normalize_or(creature.facing);
    creature.facing = ahead;
    creature.state = State::Fly { from, to, along };
    Transform::from_translation(point).looking_to(ahead, point.normalize())
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
            State::Wander(_) | State::Return => (
                clips.walk,
                gait::on_foot(gait::keeps_to(about.walk_mps, about), about).1,
            ),
            State::Flee(_) | State::Chase => {
                let fleeing = matches!(creature.state, State::Flee(_));
                let speed = if fleeing {
                    creature.pace.runs_mps(about)
                } else {
                    gait::keeps_to(about.run_mps, about)
                };
                let (gait, rate) = gait::on_foot(speed, about);
                let clip = match gait {
                    gait::Gait::Walk => clips.walk,
                    gait::Gait::Trot => clips.trot,
                    gait::Gait::Run => clips.run,
                };
                (clip, rate)
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
        leave(&mut commands, entity);
    }
    // Bodies left long enough are gone.
    for (entity, creature) in &bodies {
        if let State::Dead(lying) = creature.state
            && lying > bestiary.get(creature.species).lies_s
        {
            leave(&mut commands, entity);
        }
    }
}

#[cfg(test)]
mod tests;
