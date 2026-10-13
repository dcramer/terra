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

use super::bodies::Body;
use super::flora::ClearAround;
use super::interact::{CanInteract, Interactable, Interacted};
use super::inventory::Belongings;
use super::lod::Detail;
use super::skills::{Did, ONCE};
pub use super::species::Species;
use super::species::{Bestiary, Lives, Temper};
use super::tick::{Sim, Smoothed, Ticks};
use super::told::Told;
use super::{Downed, Player};
use crate::loading::Loading;
use crate::planet::Planet;
use crate::spatial::SpatialHash;
use crate::start::Start;
use bevy::{
    prelude::*,
    world_serialization::{WorldAsset, WorldInstanceReady},
};
use std::time::Duration;
use terra_gen::{Rng, Seed};

const BLEND: Duration = Duration::from_millis(200);
/// How far off a creature notices a player, or keeps track of one it chases.
const SENSE_M: f32 = 40.0;

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
}

/// Sent when a creature lands a blow on a player.
#[derive(Message, Clone, Copy)]
pub struct Bitten {
    pub player: Entity,
    pub damage: f32,
}

/// The animations of each species, ready to play.
#[derive(Resource)]
struct Kinds {
    models: Vec<(Species, Handle<Gltf>)>,
    ready: Vec<(Species, Handle<WorldAsset>, Handle<AnimationGraph>, Clips)>,
}

#[derive(Clone, Copy)]
struct Clips {
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    run: AnimationNodeIndex,
    eat: AnimationNodeIndex,
    attack: AnimationNodeIndex,
    death: AnimationNodeIndex,
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
                // The yard, the flat test level, has no creatures, and so no lair and
                // no village beside it.
                prepare
                    .run_if(not(resource_exists::<Populated>))
                    .run_if(crate::shell::in_the_world),
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
            },
        ));
    }
    kinds.ready = ready;

    // Herds in the meadow, and those that keep a lair in a hollow further off, each as
    // many as its kind says.
    let on_ground = |east: f32, south: f32| planet.ground_at(start.at(east, south));
    let dry = |point: Vec3| point.length() > planet.sea_radius_m() + 0.6;
    let herd = [(-45.0, 30.0), (-55.0, -20.0), (40.0, 45.0), (-30.0, 60.0)]
        .into_iter()
        .map(|(east, south)| on_ground(east, south))
        .find(|&point| dry(point));
    let lair = [(70.0, -60.0), (-80.0, -70.0), (90.0, 20.0), (-90.0, 10.0)]
        .into_iter()
        .map(|(east, south)| on_ground(east, south))
        .find(|&point| dry(point));
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

fn spawn(
    commands: &mut Commands,
    (kinds, bestiary): (&Kinds, &Bestiary),
    planet: &Planet,
    species: Species,
    at: Vec3,
) {
    let Some((_, scene, graph, clips)) = kinds.ready.iter().find(|(each, ..)| *each == species)
    else {
        return;
    };
    let about = bestiary.get(species);
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
            },
            bestiary.body(species),
            Transform::from_translation(feet).looking_to(facing, up),
            Visibility::default(),
        ))
        .id();
    let (graph, clips) = (graph.clone(), *clips);
    commands
        .spawn((
            WorldAssetRoot(scene.clone()),
            // The models face +Z; things here face -Z.
            Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI))
                .with_scale(Vec3::splat(about.model.1)),
            ChildOf(creature),
        ))
        .observe(
            move |ready: On<WorldInstanceReady>,
                  mut commands: Commands,
                  children: Query<&Children>,
                  players: Query<(), With<AnimationPlayer>>| {
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
}

/// Decides what each creature does next.
fn think(
    time: Res<Time>,
    ticks: Res<Ticks>,
    bestiary: Res<Bestiary>,
    players: Query<(Entity, &Transform), (With<Player>, Without<Downed>)>,
    mut creatures: Query<(Entity, &mut Creature, &Transform, &Detail), Without<Player>>,
    mut bitten: MessageWriter<Bitten>,
    mut dice: ResMut<Dice>,
    mut awake: Local<SpatialHash<Entity>>,
) {
    let tick = time.delta_secs().min(0.1);
    // Players filed by place, so each creature looks only at those near it (D127).
    awake.clear();
    awake.extend(players.iter().map(|(player, at)| (at.translation, player)));
    for (entity, mut creature, at, detail) in &mut creatures {
        // Far from everyone it thinks less often, over the ticks it waited (D145).
        let Some(dt) = detail.thinks(ticks.0, entity, tick) else {
            continue;
        };
        let here = at.translation;
        let (quarry, apart) = nearest(&awake, here).unzip();
        creature.quarry = quarry;
        let player_up = quarry.is_some();
        let apart = apart.unwrap_or(f32::INFINITY);
        let from_home = (here - creature.home).length();
        let about = bestiary.get(creature.species);
        let temper = about.temper;
        let next = match (temper, creature.state) {
            (_, State::Dead(lying)) => State::Dead(lying + dt),
            (Temper::Shy { .. }, State::Flee(left)) if left > 0.0 => State::Flee(left - dt),
            (
                Temper::Shy {
                    flees_within_m,
                    flees_s,
                },
                _,
            ) if creature.hurt || (apart < flees_within_m && player_up) => {
                creature.hurt = false;
                State::Flee(flees_s)
            }
            (
                Temper::Hostile {
                    bite,
                    bite_at_s,
                    bite_reach_m,
                    attack_s,
                    ..
                },
                State::Attack(through),
            ) => {
                let through = through + dt;
                // The bite lands partway through, if its quarry is still there.
                if through - dt < bite_at_s
                    && through >= bite_at_s
                    && apart < bite_reach_m
                    && let Some(player) = quarry
                {
                    bitten.write(Bitten {
                        player,
                        damage: bite,
                    });
                }
                if through > attack_s {
                    State::Chase
                } else {
                    State::Attack(through)
                }
            }
            (
                Temper::Hostile {
                    attacks_within_m, ..
                },
                _,
            ) if player_up && apart < attacks_within_m => State::Attack(0.0),
            (
                Temper::Hostile {
                    notices_m, leash_m, ..
                },
                _,
            ) if player_up && (apart < notices_m || creature.hurt) && from_home < leash_m => {
                State::Chase
            }
            (Temper::Hostile { .. }, State::Chase) => State::Return,
            (_, State::Return) if from_home > 2.0 => State::Return,
            (_, State::Idle(left)) if left > 0.0 => State::Idle(left - dt),
            (_, State::Graze(left)) if left > 0.0 => State::Graze(left - dt),
            (_, State::Wander(to)) if (to - here).length() > 1.0 => State::Wander(to),
            _ => {
                // Rest a while, graze if it can, or wander somewhere near home.
                let roll = dice.0.unit();
                if roll < 0.4 {
                    State::Idle(dice.0.range(2.0, 5.0))
                } else if roll < 0.75 && about.clips.eat.is_some() {
                    State::Graze(dice.0.range(3.0, 7.0))
                } else {
                    let up = creature.home.normalize();
                    let turn = Quat::from_axis_angle(up, dice.0.range(0.0, std::f32::consts::TAU));
                    State::Wander(
                        creature.home
                            + turn * up.any_orthonormal_vector() * dice.0.range(2.0, 12.0),
                    )
                }
            }
        };
        if bestiary.hostile(creature.species) && !matches!(next, State::Chase | State::Attack(_)) {
            creature.hurt = false;
        }
        creature.state = next;
    }
}

/// The nearest awake player a creature at a place can sense, and how far off across the
/// ground.
fn nearest(awake: &SpatialHash<Entity>, here: Vec3) -> Option<(Entity, f32)> {
    let up = here.normalize();
    awake
        .near(here, SENSE_M)
        .map(|&(at, player)| (player, (at - here).reject_from(up).length()))
        .filter(|&(_, apart)| apart <= SENSE_M)
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

/// Moves each creature over the ground by what it is doing.
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
        let quarry = creature
            .quarry
            .and_then(|player| players.get(player).ok())
            .map(|at| toward(at.translation));
        let (direction, speed) = match (creature.state, quarry) {
            (State::Wander(to), _) => (toward(to), about.walk_mps),
            (State::Return, _) => (toward(creature.home), about.walk_mps),
            // Hurt from afar with nobody near, it runs the way it faces.
            (State::Flee(_), quarry) => (
                quarry.map_or(creature.facing, |toward| -toward),
                about.run_mps,
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
        // Creatures keep out of the water.
        let ground = planet.ground_at(next);
        if ground.length() < planet.sea_radius_m() + 0.2 {
            next = here;
            if let State::Wander(_) = creature.state {
                creature.state = State::Idle(1.0);
            }
        }
        let ground = planet.ground_at(next);
        *at = Transform::from_translation(ground)
            .looking_to(creature.facing, ground.normalize())
            .with_scale(at.scale);
    }
}

/// Plays the animation that fits what each creature is doing.
fn animate(
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
        let clips = animates.clips;
        let wanted = match creature.state {
            State::Idle(_) => clips.idle,
            State::Graze(_) => clips.eat,
            State::Wander(_) | State::Return => clips.walk,
            State::Flee(_) | State::Chase => clips.run,
            State::Attack(_) => clips.attack,
            State::Dead(_) => clips.death,
        };
        if animates.playing == Some(wanted) {
            continue;
        }
        animates.playing = Some(wanted);
        let playing = transitions.play(&mut player, wanted, BLEND);
        if !matches!(creature.state, State::Dead(_) | State::Attack(_)) {
            playing.repeat();
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
    use super::*;
    use crate::play::test_player;
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
        world.get_mut::<Creature>(spider).unwrap().state = State::Attack(0.44);
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
}
