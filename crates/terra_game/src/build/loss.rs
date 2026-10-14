//! Loss and repair in the game (D193, D305; `terra_build::loss`): the player's base's debris
//! drawn where it lies, falling there from where it was when it is new; gathered with the
//! interact key, what it gives back going to the pack; and, until attacks come (D90), F9
//! harming the cube the hammer is aimed at by a quarter of its kind's health, as the remote's
//! `build.damage` does. Mending is the hammer's (`play::tool`), its damage drawn as its
//! sections are (`terra_build::sections`).

use super::act::Building;
use super::draw::{self, Uvs};
use super::{Build, BuildLook, Built, Site};
use crate::play::interact::{CanInteract, Interactable, Interacted};
use crate::play::{Builder, LocalPlayer, Player};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
use std::collections::BTreeMap;
use std::time::Instant;
use terra_build::loss::Pile;
use terra_build::ops::Op;
use terra_build::{CELL_M, Kind, Material};
use terra_world::tick::Sim;

/// How fast debris falls, in metres a second a second.
const GRAVITY: f32 = 9.8;

pub fn plugin(app: &mut App) {
    app.init_resource::<Piles>()
        .add_systems(Update, (show, fall, harm_aimed))
        .add_systems(FixedUpdate, gather.in_set(Sim::World));
}

/// A pile of debris drawn: the cube it lies in, and what it is.
#[derive(Component)]
pub struct Debris {
    pub at: [i32; 3],
}

/// Debris still falling to where it lies: how far above it, and how fast.
#[derive(Component)]
struct Falling {
    height: f32,
    speed: f32,
    rest: Vec3,
}

/// The piles drawn of each build, by its root.
#[derive(Resource, Default)]
struct Piles(BTreeMap<Entity, Drawn>);

/// A build's piles drawn: the revision they were drawn for, by the cube each lies in its
/// entity and what was drawn, and those being made on a worker.
#[derive(Default)]
struct Drawn {
    revision: Option<u64>,
    piles: BTreeMap<[i32; 3], (Entity, Pile)>,
    /// How much was in a pile drawn before that is being drawn again, so only more falling
    /// onto it makes it fall.
    was: BTreeMap<[i32; 3], u32>,
    making: Option<(Task<Vec<Made>>, Instant)>,
}

/// A pile's meshes made on a worker, about its middle.
struct Made {
    at: [i32; 3],
    pile: Pile,
    centre: Vec3,
    meshes: Vec<((Kind, Material), Mesh)>,
}

/// Draws every build's debris anew where it has changed: a pile gone is taken away at once,
/// a new or grown one made on a worker, as sections are (`layout`), and put on screen
/// falling from where the highest of it was. Only the player's base's is gathered.
fn show(
    mut commands: Commands,
    site: Res<Site>,
    builds: Query<&Built>,
    look: Res<BuildLook>,
    mut piles: ResMut<Piles>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(kits) = look.kits.clone() else {
        return;
    };
    let all = std::iter::once((&site.build, true)).chain(builds.iter().map(|b| (&b.0, false)));
    for (build, gathered) in all {
        let drawn = piles.0.entry(build.root).or_default();
        if let Some((mut task, began)) = drawn.making.take() {
            let Some(made) = block_on(poll_once(&mut task)) else {
                drawn.making = Some((task, began));
                continue;
            };
            let count = made.len();
            for each in made {
                put(
                    &mut commands,
                    build,
                    gathered,
                    &look,
                    drawn,
                    each,
                    &mut meshes,
                );
            }
            info!(
                "Build: debris drawn: {count} piles, {:.1} ms from noticed to on screen",
                began.elapsed().as_secs_f32() * 1000.0
            );
        }
        if drawn.revision == Some(build.revision) {
            continue;
        }
        drawn.revision = Some(build.revision);
        let now: BTreeMap<[i32; 3], Pile> = build
            .base
            .debris()
            .map(|(at, pile)| (at.to_array(), pile.clone()))
            .collect();
        let gone: Vec<[i32; 3]> = drawn
            .piles
            .iter()
            .filter(|(at, (_, pile))| now.get(*at) != Some(pile))
            .map(|(at, _)| *at)
            .collect();
        for at in gone {
            if let Some((entity, pile)) = drawn.piles.remove(&at) {
                drawn.was.insert(at, pile.cubes() + pile.props);
                commands.entity(entity).despawn();
            }
        }
        let new: Vec<([i32; 3], Pile)> = now
            .into_iter()
            .filter(|(at, _)| !drawn.piles.contains_key(at))
            .collect();
        if new.is_empty() {
            continue;
        }
        let kits = kits.clone();
        let task = AsyncComputeTaskPool::get().spawn(async move {
            new.into_iter()
                .map(|(at, pile)| make(at, pile, &kits))
                .collect()
        });
        drawn.making = Some((task, Instant::now()));
    }
}

/// A pile's meshes, about its middle on what it lies on.
fn make(at: [i32; 3], pile: Pile, kits: &terra_build::kit::Kits) -> Made {
    let place = IVec3::from_array(at);
    let centre = Vec3::new(
        (place.x as f32 + 0.5) * CELL_M,
        pile.floor_cm as f32 / 100.0,
        (place.z as f32 + 0.5) * CELL_M,
    );
    let meshes = terra_build::debris::draw(place, &pile, kits)
        .into_iter()
        .filter_map(|(key, surface)| Some((key, draw::mesh(&surface, centre, Uvs::Mapped)?)))
        .collect();
    Made {
        at,
        pile,
        centre,
        meshes,
    }
}

/// Puts a pile made on a worker on screen, if it is still what lies there: new, or more
/// fallen onto it, falling from where the highest of it was.
fn put(
    commands: &mut Commands,
    build: &Build,
    gathered: bool,
    look: &BuildLook,
    drawn: &mut Drawn,
    made: Made,
    meshes: &mut Assets<Mesh>,
) {
    let place = IVec3::from_array(made.at);
    if build.base.pile(place) != Some(&made.pile) || drawn.piles.contains_key(&made.at) {
        // Changed again since: drawn again when it is noticed.
        drawn.revision = None;
        return;
    }
    let pile = made.pile;
    let was = drawn.was.remove(&made.at).unwrap_or(0);
    let height = if pile.cubes() + pile.props > was {
        ((pile.from as f32 + 0.5) * CELL_M - made.centre.y).max(0.0)
    } else {
        0.0
    };
    let root = commands
        .spawn((
            Name::new("Debris"),
            Debris { at: made.at },
            Transform::from_translation(made.centre + Vec3::Y * height),
            Visibility::default(),
            ChildOf(build.root),
        ))
        .id();
    if gathered {
        commands
            .entity(root)
            .insert(CanInteract(Interactable::Debris));
    }
    if height > 0.0 {
        commands.entity(root).insert(Falling {
            height,
            speed: 0.0,
            rest: made.centre,
        });
    }
    for ((kind, material), mesh) in made.meshes {
        let Some(handle) = look
            .materials
            .get(kind, material)
            .or_else(|| look.materials.get(Kind::PLAIN, material))
        else {
            continue;
        };
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(handle.clone()),
            Transform::default(),
            ChildOf(root),
        ));
    }
    drawn.piles.insert(made.at, (root, pile));
}

/// Debris falls to where it lies.
fn fall(
    mut commands: Commands,
    time: Res<Time>,
    mut falling: Query<(Entity, &mut Falling, &mut Transform)>,
) {
    for (entity, mut fall, mut at) in &mut falling {
        fall.speed += GRAVITY * time.delta_secs();
        fall.height = (fall.height - fall.speed * time.delta_secs()).max(0.0);
        at.translation = fall.rest + Vec3::Y * fall.height;
        if fall.height <= 0.0 {
            commands.entity(entity).remove::<Falling>();
        }
    }
}

/// The interact key at a pile gathers it: what it gives back goes to the pack, what does not
/// fit drops at the player's feet.
fn gather(
    mut interacted: MessageReader<Interacted>,
    piles: Query<&Debris>,
    mut building: Building,
) {
    for used in interacted.read() {
        let Some(entity) = used.on(Interactable::Debris) else {
            continue;
        };
        let Ok(debris) = piles.get(entity) else {
            continue;
        };
        let op = Op::Gather { at: debris.at };
        if let Ok(done) = building.act(Some(used.player), "gather the rubble", &[op]) {
            let said = if done.gave.is_empty() {
                "Nothing worth keeping".to_string()
            } else {
                format!("Gathered {}", done.gave)
            };
            building.tell(used.player, said);
        }
    }
}

/// F9 harms the cube the hammer is aimed at by a quarter of its kind's health, as an attack
/// will (D90): a debug key, until attacks come.
fn harm_aimed(
    keys: Res<ButtonInput<KeyCode>>,
    builders: Query<&Builder, (With<Player>, With<LocalPlayer>)>,
    mut building: Building,
) {
    if !keys.just_pressed(KeyCode::F9) {
        return;
    }
    let Some(at) = builders.iter().find_map(Builder::aimed) else {
        return;
    };
    let Some(cube) = building.site.base.cube(at) else {
        return;
    };
    let health = building.rules.rules.toughness(cube.kind).health;
    let op = Op::Damage {
        cubes: vec![at.to_array()],
        amount: health.div_ceil(4),
    };
    let _ = building.act(None, "damage", &[op]);
}
