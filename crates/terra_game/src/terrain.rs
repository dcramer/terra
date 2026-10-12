//! Drawing the voxel ground (docs/terrain.md, A2): chunks around the camera at the detail
//! `terra_voxel` chooses, meshed on other threads, and drawn again where the ground is dug
//! or filled.
//!
//! The first view is meshed before the first frame, across every core, so nothing is
//! seen half built. After that, as the camera moves on, the chunks it now wants are meshed
//! in the background, and the ones it no longer wants stay until their replacements are
//! ready, so the ground never shows holes. A scripted run waits for every chunk each
//! frame, so the same script shows the same ground on any machine.

use crate::loading::{self, Progress};
use crate::planet::MainCamera;
use crate::planet::{GroundLook, Planet};
use crate::shell::Options;
use bevy::{
    asset::RenderAssetUsages,
    mesh::Indices,
    prelude::*,
    render::render_resource::PrimitiveTopology,
    tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future},
};
use std::collections::HashMap;
use terra_core::{ChunkId, ChunkMesh, MaterialId};
use terra_voxel::{
    Detail, Field, Sides, chunk_origin, chunk_size_m, chunks_around, material, may_hold_surface,
    mesh_chunk,
};

/// The camera moves this far before the chunks are chosen again.
const RECHOOSE_M: f32 = 4.0;

/// A chunk of the ground on screen.
#[derive(Component)]
pub struct Chunk;

/// What is drawn, what is being meshed, and where the chunks were last chosen from.
#[derive(Resource, Default)]
struct Terrain {
    shown: HashMap<ChunkId, Shown>,
    pending: HashMap<ChunkId, Pending>,
    wanted: HashMap<ChunkId, Sides>,
    chosen_from: Option<Vec3>,
    /// How many edits the ground had when its chunks were last brought up to date.
    edits_seen: usize,
}

/// A chunk drawn: with which faces stitched to finer neighbours, and its entity, if its
/// mesh has any triangles.
struct Shown {
    sides: Sides,
    entity: Option<Entity>,
}

struct Pending {
    sides: Sides,
    task: Task<Option<Mesh>>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Terrain>()
        .add_systems(PostStartup, first_view)
        .add_systems(
            Update,
            (choose, receive, track.in_set(loading::Track)).chain(),
        );
}

/// The loading screen waits for the ground in view to be meshed.
fn track(terrain: Res<Terrain>, mut progress: ResMut<Progress>) {
    progress.report(
        "Shaping the ground",
        terrain.pending.len(),
        terrain.shown.len(),
    );
}

fn detail(planet: &Planet) -> Detail {
    Detail::for_planet(planet.radius_m(), planet.relief_m())
}

/// The chunks a viewer at a place should see.
fn wanted(planet: &Planet, viewer: Vec3) -> HashMap<ChunkId, Sides> {
    let ground = planet.voxels();
    chunks_around(&[viewer], &detail(planet), |id| {
        may_hold_surface(&**ground, id)
    })
    .into_iter()
    .collect()
}

/// How a vertex's colour tells the ground shader what grows there and what it is made of:
/// red is grass, green how lush, blue rock showing through (half for stone, all for ore),
/// and alpha how much of the sky it sees (see [`sky_seen`]).
fn colour(planet: &Planet, point: Vec3, normal: Vec3, made_of: MaterialId) -> [f32; 4] {
    let rock = match made_of {
        material::STONE => 0.5,
        material::ORE => 1.0,
        _ => 0.0,
    };
    let cover = planet.cover(point, normal);
    let grass = if rock > 0.0 { 0.0 } else { cover.grass };
    [grass, cover.lush, rock, 1.0]
}

/// Chunks this fine or finer work out how much sky each vertex sees; coarser ones are
/// far enough off that it does not show.
const SHADED_LEVELS: u8 = 1;

/// How much of the sky a point on the ground sees, from 0 in a narrow pit to 1 in the
/// open, judged by how far the ground is in a few directions round its face. The sky's
/// light is dimmed by it, so a hole is dark at the bottom rather than lit as open ground.
fn sky_seen<F: Field>(field: &F, memo: &mut F::Memo, point: Vec3, normal: Vec3) -> f32 {
    let up = point.normalize();
    let across = up.any_orthonormal_vector();
    let mut open = 0.0;
    let mut asked = 0.0;
    for turn in 0..6 {
        let around = Quat::from_axis_angle(up, turn as f32 * std::f32::consts::TAU / 6.0);
        let slant = (up + around * across * 1.2).normalize();
        for reach in [0.8, 2.0] {
            let at = point + normal * 0.3 + slant * reach;
            open += (field.distance_in(at, memo) / (reach * 0.7)).clamp(0.0, 1.0);
            asked += 1.0;
        }
    }
    open / asked
}

/// A chunk's mesh as Bevy draws it, or nothing if it has no surface.
fn to_mesh(planet: &Planet, id: ChunkId, chunk: ChunkMesh) -> Option<Mesh> {
    if chunk.indices.is_empty() {
        return None;
    }
    let field = &**planet.voxels();
    let mut memo = field.memo_for(terra_voxel::VOXEL_M);
    let colours: Vec<[f32; 4]> = chunk
        .positions
        .iter()
        .zip(&chunk.normals)
        .zip(&chunk.materials)
        .map(|((&point, &normal), weights)| {
            let mut colour = colour(planet, point, normal, weights.ids[0]);
            if id.level <= SHADED_LEVELS {
                colour[3] = sky_seen(field, &mut memo, point, normal);
            }
            colour
        })
        .collect();
    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, chunk.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, chunk.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
        .with_inserted_indices(Indices::U32(chunk.indices)),
    )
}

fn mesh(planet: &Planet, id: ChunkId, sides: Sides) -> Option<Mesh> {
    to_mesh(planet, id, mesh_chunk(&**planet.voxels(), id, sides))
}

fn show(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    look: &GroundLook,
    id: ChunkId,
    mesh: Option<Mesh>,
) -> Option<Entity> {
    let mesh = mesh?;
    Some(
        commands
            .spawn((
                Name::new(format!("Ground {} {}", id.level, id.coord)),
                Chunk,
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(look.0.clone()),
                Transform::default(),
            ))
            .id(),
    )
}

/// Meshes the first view before anything is drawn, on every core.
fn first_view(
    mut commands: Commands,
    planet: Res<Planet>,
    look: Res<GroundLook>,
    camera: Single<&Transform, With<MainCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut terrain: ResMut<Terrain>,
) {
    let viewer = camera.translation;
    let wanted = wanted(&planet, viewer);
    let chunks: Vec<(ChunkId, Sides)> = wanted.iter().map(|(&id, &sides)| (id, sides)).collect();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let planet_ref = &*planet;
    let meshed: Vec<(ChunkId, Sides, Option<Mesh>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = chunks
            .chunks(chunks.len().div_ceil(threads).max(1))
            .map(|some| {
                scope.spawn(move || {
                    some.iter()
                        .map(|&(id, sides)| (id, sides, mesh(planet_ref, id, sides)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap_or_default())
            .collect()
    });
    let count = meshed.len();
    for (id, sides, mesh) in meshed {
        let entity = show(&mut commands, &mut meshes, &look, id, mesh);
        terrain.shown.insert(id, Shown { sides, entity });
    }
    info!("Meshed the first view of the ground: {count} chunks.");
    terrain.wanted = wanted;
    terrain.chosen_from = Some(viewer);
    terrain.edits_seen = planet.voxels().edit_count();
}

/// Chooses the chunks again when the camera has moved on or the ground has changed, and
/// starts meshing those that are new or out of date.
fn choose(
    planet: Res<Planet>,
    camera: Single<&GlobalTransform, With<MainCamera>>,
    mut terrain: ResMut<Terrain>,
) {
    let viewer = camera.translation();
    let edits = planet.voxels().edit_count();
    let moved = terrain
        .chosen_from
        .is_none_or(|from| from.distance(viewer) > RECHOOSE_M);
    if !moved && edits == terrain.edits_seen {
        return;
    }
    // Chunks an edit reached are meshed again even if nothing else changed.
    let changed: Vec<(Vec3, f32)> = planet.voxels().edits_since(terrain.edits_seen).collect();
    terrain.edits_seen = edits;
    // A dig can open chunks that held no surface before, so the ground is chosen again
    // after any edit, not only when the camera moves.
    terrain.wanted = wanted(&planet, viewer);
    if moved {
        terrain.chosen_from = Some(viewer);
    }
    let touched = |id: ChunkId| {
        let low = chunk_origin(id);
        let high = low + Vec3::splat(chunk_size_m(id.level));
        changed
            .iter()
            .any(|&(centre, reach)| centre.clamp(low, high).distance(centre) <= reach)
    };
    let pool = AsyncComputeTaskPool::get();
    let wanted: Vec<(ChunkId, Sides)> = terrain.wanted.iter().map(|(&id, &s)| (id, s)).collect();
    for (id, sides) in wanted {
        let up_to_date = terrain
            .shown
            .get(&id)
            .is_some_and(|shown| shown.sides == sides)
            && !touched(id);
        let underway = terrain
            .pending
            .get(&id)
            .is_some_and(|pending| pending.sides == sides)
            && !touched(id);
        if up_to_date || underway {
            continue;
        }
        let planet = planet.clone();
        let task = pool.spawn(async move { mesh(&planet, id, sides) });
        terrain.pending.insert(id, Pending { sides, task });
    }
}

/// Draws chunks whose meshes are ready, and once nothing is waiting, takes away the ones
/// no longer wanted.
fn receive(
    mut commands: Commands,
    options: Res<Options>,
    look: Option<Res<GroundLook>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut terrain: ResMut<Terrain>,
) {
    let Some(look) = look else { return };
    let ids: Vec<ChunkId> = terrain.pending.keys().copied().collect();
    for id in ids {
        let Some(pending) = terrain.pending.get_mut(&id) else {
            continue;
        };
        // A scripted run waits for each chunk, so it sees the same ground every time.
        let ready = if options.scripted() {
            Some(block_on(&mut pending.task))
        } else {
            block_on(future::poll_once(&mut pending.task))
        };
        let Some(mesh) = ready else { continue };
        let sides = pending.sides;
        terrain.pending.remove(&id);
        if let Some(old) = terrain.shown.remove(&id).and_then(|shown| shown.entity) {
            commands.entity(old).despawn();
        }
        let entity = show(&mut commands, &mut meshes, &look, id, mesh);
        terrain.shown.insert(id, Shown { sides, entity });
    }
    if terrain.pending.is_empty() {
        let stale: Vec<ChunkId> = terrain
            .shown
            .keys()
            .filter(|id| !terrain.wanted.contains_key(id))
            .copied()
            .collect();
        for id in stale {
            if let Some(entity) = terrain.shown.remove(&id).and_then(|shown| shown.entity) {
                commands.entity(entity).despawn();
            }
        }
    }
}
