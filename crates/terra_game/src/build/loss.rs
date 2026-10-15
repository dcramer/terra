//! Loss and repair in the game (D193, D305; `terra_build::loss`): the player's base's debris
//! drawn where it lies, falling there from where it was when it is new; gathered with the
//! interact key, what it gives back going to the pack; and, until attacks come (D90), F9
//! harming the cube the hammer is aimed at by a quarter of its kind's health, as the remote's
//! `build.damage` does. Mending is the hammer's (`play::tool`), its damage drawn as its
//! sections are (`terra_build::sections`).

use super::act::Building;
use super::arrays::{Arrays, BuildMaterial, Layers};
use super::draw;
use super::{Build, BuildLook, Built, Site};
use crate::play::interact::{CanInteract, Interactable, Interacted};
use crate::play::{Builder, LocalPlayer, Player};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
use std::collections::{BTreeMap, BTreeSet};
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

/// Debris still falling to where it lies: how far above it, and how fast; how far it fell
/// from, and how it was turned there, tumbling square as it lands.
#[derive(Component)]
struct Falling {
    height: f32,
    speed: f32,
    rest: Vec3,
    from: f32,
    turned: Quat,
}

/// Where a pile starts falling, as a share of the highest it fell from, and how it is turned
/// there, by where it lies: the piles of a wall or a tower brought down rain from all through
/// the height it stood, each tumbling, rather than dropping as one sheet from its top.
fn scattered(at: [i32; 3]) -> (f32, Quat) {
    let hash = (at[0] as u32).wrapping_mul(73_856_093)
        ^ (at[1] as u32).wrapping_mul(19_349_663)
        ^ (at[2] as u32).wrapping_mul(83_492_791);
    let unit = |shift: u32| ((hash >> shift) & 0xff) as f32 / 255.0;
    let share = 0.35 + 0.65 * unit(0);
    let axis = Vec3::new(unit(8) - 0.5, 0.25, unit(16) - 0.5).normalize();
    (share, Quat::from_axis_angle(axis, 0.3 + 0.5 * unit(24)))
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
    /// Piles changed and being drawn again, still on screen until their new meshes are.
    replacing: BTreeMap<[i32; 3], Entity>,
    /// The texture arrays' layers each pile drawn wears, kept resident while it lies there.
    layers: BTreeMap<[i32; 3], Vec<u16>>,
    making: Option<(Task<Vec<Made>>, Instant)>,
}

/// A pile's meshes made on a worker, about its middle.
struct Made {
    at: [i32; 3],
    pile: Pile,
    centre: Vec3,
    /// What is drawn in the texture arrays, as a section's surfaces are (D290), and the
    /// layers it wears; and what has no layer there, in its own material.
    arrays: Option<Mesh>,
    layers: Vec<u16>,
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
    options: Res<crate::shell::Options>,
    (mut arrays, assets, time): (Option<ResMut<Arrays>>, Res<AssetServer>, Res<Time<Real>>),
) {
    let Some(kits) = look.kits.clone() else {
        return;
    };
    let material = arrays.as_ref().map(|arrays| arrays.material.clone());
    let material = material.as_ref();
    // What debris lies about keeps its layers of the arrays resident, as sections do.
    if let Some(arrays) = arrays.as_mut() {
        let now = time.elapsed_secs();
        for drawn in piles.0.values() {
            for layers in drawn.layers.values() {
                arrays.want(layers, &assets, now);
            }
        }
    }
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
                    (&look, material),
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
                drawn.layers.remove(&at);
                // A pile changed stays on screen until it is drawn again, so it never blinks
                // out for the frames its new meshes take on the worker; one gathered goes.
                if now.contains_key(&at) {
                    if let Some(older) = drawn.replacing.insert(at, entity) {
                        commands.entity(older).despawn();
                    }
                } else {
                    commands.entity(entity).despawn();
                }
            }
        }
        let left: Vec<[i32; 3]> = drawn
            .replacing
            .keys()
            .filter(|at| !now.contains_key(*at))
            .copied()
            .collect();
        for at in left {
            if let Some(entity) = drawn.replacing.remove(&at) {
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
        // A scripted run makes them at once, as it lays sections, so its pictures are the
        // same every time.
        if options.scripted() && !options.bench {
            for (at, pile) in new {
                let made = make(at, pile, &kits, &look.layers);
                put(
                    &mut commands,
                    build,
                    gathered,
                    (&look, material),
                    drawn,
                    made,
                    &mut meshes,
                );
            }
            continue;
        }
        let kits = kits.clone();
        let layers = look.layers.clone();
        let task = AsyncComputeTaskPool::get().spawn(async move {
            new.into_iter()
                .map(|(at, pile)| make(at, pile, &kits, &layers))
                .collect()
        });
        drawn.making = Some((task, Instant::now()));
    }
}

/// A pile's meshes, about its middle on what it lies on.
fn make(at: [i32; 3], pile: Pile, kits: &terra_build::kit::Kits, layers: &Layers) -> Made {
    let place = IVec3::from_array(at);
    let centre = Vec3::new(
        (place.x as f32 + 0.5) * CELL_M,
        pile.floor_cm as f32 / 100.0,
        (place.z as f32 + 0.5) * CELL_M,
    );
    let surfaces = terra_build::debris::draw(place, &pile, kits);
    let mut used = BTreeSet::new();
    let (arrays, rest) = draw::in_arrays(&surfaces, layers, centre, &BTreeMap::new(), &mut used);
    Made {
        at,
        pile,
        centre,
        arrays,
        layers: used.into_iter().collect(),
        meshes: rest
            .into_iter()
            .map(|(kind, material, mesh)| ((kind, material), mesh))
            .collect(),
    }
}

/// Puts a pile made on a worker on screen, if it is still what lies there: new, or more
/// fallen onto it, falling from where the highest of it was.
fn put(
    commands: &mut Commands,
    build: &Build,
    gathered: bool,
    (look, arrays): (&BuildLook, Option<&Handle<BuildMaterial>>),
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
    if let Some(older) = drawn.replacing.remove(&made.at) {
        commands.entity(older).despawn();
    }
    let was = drawn.was.remove(&made.at).unwrap_or(0);
    let (share, turned) = scattered(made.at);
    let height = if pile.cubes() + pile.props > was {
        ((pile.from as f32 + 0.5) * CELL_M - made.centre.y).max(0.0) * share
    } else {
        0.0
    };
    let start = if height > 0.0 { turned } else { Quat::IDENTITY };
    let root = commands
        .spawn((
            Name::new("Debris"),
            Debris { at: made.at },
            Transform::from_translation(made.centre + Vec3::Y * height).with_rotation(start),
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
            from: height,
            turned,
        });
    }
    if let (Some(mesh), Some(arrays)) = (made.arrays, arrays) {
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(arrays.clone()),
            Transform::default(),
            ChildOf(root),
        ));
    }
    drawn.layers.insert(made.at, made.layers);
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
        at.rotation = Quat::IDENTITY.slerp(fall.turned, fall.height / fall.from.max(1e-3));
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

#[cfg(test)]
mod tests {
    use super::scattered;

    #[test]
    fn a_wall_brought_down_rains_from_through_its_height_not_as_one_sheet() {
        // The piles along a wall ten cubes long: each starts its fall between a third of the
        // way up and the top, the same each time, and not all from one height.
        let starts: Vec<f32> = (0..10).map(|x| scattered([x, 0, 4]).0).collect();
        assert!(starts.iter().all(|share| (0.35..=1.0).contains(share)));
        let again: Vec<f32> = (0..10).map(|x| scattered([x, 0, 4]).0).collect();
        assert_eq!(starts, again);
        let low = starts.iter().copied().fold(1.0, f32::min);
        let high = starts.iter().copied().fold(0.0, f32::max);
        assert!(high - low > 0.25, "{starts:?}");
        // Each is turned where it starts, and so tumbles as it falls.
        let turned = scattered([3, 0, 4]).1;
        assert!(turned.angle_between(bevy::prelude::Quat::IDENTITY) > 0.2);
    }
}
