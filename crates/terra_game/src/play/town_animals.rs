//! A town's animals drawn (docs/demos.md, a living town): each kind that has a model
//! (`animals` in `assets/data/townsfolk.ron`) stood where `terra_town::Animals` has it,
//! playing its walk, run or rest. A kind with no model yet is kept by the town's life but
//! not drawn.

use super::creatures::Tame;
use super::tick::{Sim, Smoothed};
use super::town::{Live, Market};
use crate::planet::Planet;
use bevy::{
    platform::collections::HashMap,
    prelude::*,
    world_serialization::{WorldAsset, WorldAssetRoot, WorldInstanceReady},
};
use serde::Deserialize;
use std::time::Duration;
use terra_town::animals::Doing;

const LOOKS: &str = "townsfolk.ron";
const BLEND: Duration = Duration::from_millis(250);

/// How a kind of animal is drawn: its model, how much it is scaled, and its clips.
#[derive(Deserialize, Clone, Debug)]
struct Drawn {
    model: String,
    #[serde(default = "one")]
    scale: f32,
    idle: String,
    walk: String,
    #[serde(default)]
    run: Option<String>,
    /// Played while it rests, if not its idle: pecking, grazing, lying.
    #[serde(default)]
    rest: Option<String>,
    /// Its walk's own pace, metres a second, so its feet keep to the ground.
    pace: f32,
}

fn one() -> f32 {
    1.0
}

#[derive(Deserialize, Default)]
struct Looks {
    #[serde(default)]
    animals: std::collections::BTreeMap<String, Drawn>,
}

/// One of the town's animals: which, in the town's life.
#[derive(Component)]
#[require(Smoothed, Tame)]
pub struct Beast {
    pub index: usize,
}

/// The kinds being drawn, their models and clips once loaded.
#[derive(Resource, Default)]
struct Kinds {
    kinds: HashMap<String, (Drawn, Handle<Gltf>)>,
    ready: HashMap<String, (Handle<WorldAsset>, Handle<AnimationGraph>, Clips)>,
    spawned: bool,
}

#[derive(Clone, Copy)]
struct Clips {
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    run: AnimationNodeIndex,
    rest: AnimationNodeIndex,
}

#[derive(Component)]
struct Plays {
    beast: Entity,
    clips: Clips,
    pace: f32,
    playing: Option<AnimationNodeIndex>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Kinds>()
        .add_systems(
            Update,
            (load, prepare, spawn, animate)
                .chain()
                .run_if(resource_exists::<Market>),
        )
        .add_systems(
            FixedUpdate,
            place
                .run_if(resource_exists::<Market>)
                .in_set(Sim::World)
                .after(Live),
        );
}

/// Each kind's model, for the kinds the town keeps that have one.
fn load(
    assets: Res<AssetServer>,
    market: Res<Market>,
    mut kinds: ResMut<Kinds>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    *done = true;
    let looks: Looks = std::fs::read_to_string(terra_world::data::path(LOOKS))
        .ok()
        .and_then(|text| {
            ron::from_str(&text)
                .map_err(|why| error!("{LOOKS}: {why}"))
                .ok()
        })
        .unwrap_or_default();
    for name in market.animals.kinds.keys() {
        match looks.animals.get(name) {
            Some(drawn) if terra_world::data::assets().join(&drawn.model).exists() => {
                let model = assets.load(drawn.model.clone());
                kinds.kinds.insert(name.clone(), (drawn.clone(), model));
            }
            _ => info!("Town animals: no model for {name} yet; kept, not drawn."),
        }
    }
}

fn prepare(
    gltfs: Res<Assets<Gltf>>,
    assets: Res<AssetServer>,
    mut kinds: ResMut<Kinds>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    let waiting: Vec<String> = kinds
        .kinds
        .keys()
        .filter(|name| !kinds.ready.contains_key(*name))
        .cloned()
        .collect();
    for name in waiting {
        let (drawn, model) = kinds.kinds[&name].clone();
        if !assets.is_loaded_with_dependencies(&model) {
            continue;
        }
        let Some(gltf) = gltfs.get(&model) else {
            continue;
        };
        let Some(scene) = gltf.default_scene.clone() else {
            continue;
        };
        let clip = |name: &str| gltf.named_animations.get(name).cloned();
        let (Some(idle), Some(walk)) = (clip(&drawn.idle), clip(&drawn.walk)) else {
            warn!("Town animals: {name}'s model lacks its idle or walk");
            kinds.kinds.remove(&name);
            continue;
        };
        let run = drawn
            .run
            .as_deref()
            .and_then(clip)
            .unwrap_or_else(|| walk.clone());
        let rest = drawn
            .rest
            .as_deref()
            .and_then(clip)
            .unwrap_or_else(|| idle.clone());
        let (graph, nodes) = AnimationGraph::from_clips([idle, walk, run, rest]);
        kinds.ready.insert(
            name,
            (
                scene,
                graphs.add(graph),
                Clips {
                    idle: nodes[0],
                    walk: nodes[1],
                    run: nodes[2],
                    rest: nodes[3],
                },
            ),
        );
    }
}

/// Every animal of a drawn kind as a body, once its model is ready.
fn spawn(mut commands: Commands, market: Res<Market>, mut kinds: ResMut<Kinds>) {
    if kinds.spawned || kinds.kinds.is_empty() || kinds.ready.len() < kinds.kinds.len() {
        return;
    }
    kinds.spawned = true;
    let mut count = 0;
    for (index, animal) in market.animals.animals.iter().enumerate() {
        let Some((scene, graph, clips)) = kinds.ready.get(&animal.kind).cloned() else {
            continue;
        };
        let (drawn, _) = &kinds.kinds[&animal.kind];
        let pace = drawn.pace;
        let beast = commands
            .spawn((
                Name::new(format!("{} {index}", animal.kind)),
                Beast { index },
                Transform::from_translation(market.middle),
                Visibility::Hidden,
            ))
            .id();
        commands
            .spawn((
                WorldAssetRoot(scene),
                Transform::from_scale(Vec3::splat(drawn.scale)),
                ChildOf(beast),
            ))
            .observe(
                move |ready: On<WorldInstanceReady>,
                      mut commands: Commands,
                      children: Query<&Children>,
                      players: Query<(), With<AnimationPlayer>>| {
                    for child in children.iter_descendants(ready.entity) {
                        if players.contains(child) {
                            commands.entity(child).insert((
                                AnimationGraphHandle(graph.clone()),
                                AnimationTransitions::new(),
                                Plays {
                                    beast,
                                    clips,
                                    pace,
                                    playing: None,
                                },
                            ));
                        }
                    }
                },
            );
        count += 1;
    }
    info!("Town animals: {count} drawn.");
}

/// Each animal where the town's life has it, on the ground, facing its way; not drawn far
/// from every viewer.
fn place(
    planet: Res<Planet>,
    market: Res<Market>,
    mut beasts: Query<(&Beast, &mut Transform, &mut Visibility)>,
    rising: Option<Res<super::rising::Rising>>,
) {
    for (beast, mut at, mut shown) in &mut beasts {
        let Some(animal) = market.animals.animals.get(beast.index) else {
            continue;
        };
        // In a settlement rising, a yard's animals come with its house, and none in a
        // time-lapse without its people (D381).
        let wanted = if rising
            .as_ref()
            .is_some_and(|r| !r.shows(false, super::rising::owner(&market.life.town, animal.at)))
        {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *shown != wanted {
            *shown = wanted;
        }
        let feet = market.ground(&planet, animal.at);
        let up = feet.normalize();
        let facing = market
            .way(animal.facing)
            .reject_from(up)
            .normalize_or(market.east);
        *at = Transform::from_translation(feet).looking_to(facing, up);
    }
}

fn animate(
    market: Res<Market>,
    beasts: Query<&Beast>,
    mut players: Query<(&mut Plays, &mut AnimationPlayer, &mut AnimationTransitions)>,
    rising: Option<Res<super::rising::Rising>>,
) {
    for (mut plays, mut player, mut transitions) in &mut players {
        let Ok(beast) = beasts.get(plays.beast) else {
            continue;
        };
        let Some(animal) = market.animals.animals.get(beast.index) else {
            continue;
        };
        let clips = plays.clips;
        let (wanted, rate) = match animal.doing {
            Doing::Moving { run: true } if animal.speed > 0.05 => (clips.run, 1.0),
            Doing::Moving { .. } if animal.speed > 0.02 => (
                clips.walk,
                (animal.speed / plays.pace.max(0.05)).clamp(0.3, 2.0),
            ),
            Doing::Moving { .. } => (clips.idle, 1.0),
            Doing::Resting { .. } => (clips.rest, 1.0),
        };
        if plays.playing != Some(wanted) {
            plays.playing = Some(wanted);
            transitions.play(&mut player, wanted, BLEND).repeat();
        }
        if let Some(active) = player.animation_mut(wanted) {
            active.set_speed(rate * rising.as_ref().map_or(1.0, |r| r.lapse.pace));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_town_animal_is_tame_so_a_film_sees_it_with_the_wild_ones() {
        let mut world = World::new();
        let beast = world.spawn(Beast { index: 0 }).id();
        assert!(world.entity(beast).contains::<Tame>());
    }
}
