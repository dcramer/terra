//! Drawing the voxel ground (docs/terrain.md, A2): chunks around the camera at the detail
//! `terra_voxel` chooses, meshed on other threads, and drawn again where the ground is dug
//! or filled.
//!
//! The first view is meshed before the first frame, across every core, so nothing is
//! seen half built. After that, as the camera moves on, the chunks it now wants are meshed
//! in the background, and the ones it no longer wants stay until their replacements are
//! ready, so the ground never shows holes. Which chunks are wanted is worked out on a
//! worker too, as Voxel Tools updates its level-of-detail octree off the main thread: it
//! took 17 ms of a frame every 4 m walked on a 30 km world. A scripted run waits for every
//! chunk and every choice each frame, so the same script shows the same ground on any
//! machine.

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
use terra_gen::smoothstep;
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
    /// The chunks wanted from a place, being worked out; and the edits made since the
    /// chunks were last brought up to date, whose chunks are meshed again once they are.
    choosing: Option<Task<HashMap<ChunkId, Sides>>>,
    changed: Vec<(Vec3, f32)>,
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

/// Chunks this coarse and coarser split within `FAR_SPLIT` of their sizes on a world with
/// mountains: 256 m chunks, from about half a kilometre off, so the walls across a valley are
/// meshed at a third of the cell they were (D322) and their couloirs stand in the mesh.
const FAR_FROM: u8 = 4;
const FAR_SPLIT: f32 = 3.0;

/// The tree's shape, every split `reach` times as far as high has it
/// (`Quality::ground_reach`, D428).
fn detail(planet: &Planet, reach: f32) -> Detail {
    let detail = Detail::for_planet(planet.radius_m(), planet.relief_m());
    let detail = if planet.has_mountains() {
        Detail {
            far_from: FAR_FROM,
            far_split: FAR_SPLIT,
            ..detail
        }
    } else {
        detail
    };
    Detail {
        split: detail.split * reach,
        far_split: detail.far_split * reach,
        ..detail
    }
}

/// The chunks a viewer at a place should see.
fn wanted(planet: &Planet, viewer: Vec3, reach: f32) -> HashMap<ChunkId, Sides> {
    let ground = planet.voxels();
    chunks_around(&[viewer], &detail(planet, reach), |id| {
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
        // In mountains stone at the surface is their bare rock, drawn as [`lying`] says.
        material::STONE if planet.has_mountains() => 0.0,
        material::STONE => 0.5,
        material::ORE => 1.0,
        _ => 0.0,
    };
    let cover = planet.cover(point, normal);
    let laid_bare = planet.has_mountains()
        && matches!(
            made_of,
            material::SCREE | material::GRAVEL | material::STONE
        );
    let grass = if rock > 0.0 || laid_bare {
        0.0
    } else {
        cover.grass
    };
    [grass, cover.lush, rock, 1.0]
}

/// What the land's rules lay on the ground, for the ground shader (D322), in a vertex's two
/// texture coordinates:
/// - the first's x what loose stones lie there, 1 scree and 2 gravel, 0 none; its y how deep
///   the snow lies, in metres;
/// - the second's x the bare rock there, 0 to 1, or below 0 the woods standing on it, as how
///   close they stand, negated; its y how far it stands above the nearest lake's water, in
///   metres, below 0 under it, [`DRY_M`] far from any.
fn lying(
    planet: &Planet,
    point: Vec3,
    normal: Vec3,
    made_of: MaterialId,
    bed: &mut dyn FnMut(Vec3) -> f32,
) -> ([f32; 2], [f32; 2]) {
    use terra_land::woods::Footing;
    let up = point.normalize_or(Vec3::Y);
    let Some(round) = planet.round() else {
        let loose = match made_of {
            material::SCREE => 1.0,
            material::GRAVEL => 2.0,
            _ => 0.0,
        };
        let bare = if made_of == material::STONE { 1.0 } else { 0.0 };
        return ([loose, 0.0], [bare, DRY_M]);
    };
    let lake = round.water_near(up, crate::shore::REACH_CELLS);
    let above_water = lake.map_or(DRY_M, |level| (point.length() - level).clamp(-DRY_M, DRY_M));
    let snow = round.snow_m(point, normal);
    // On a mountain's steep faces a summer's snow lies in its gullies' beds and is gone from
    // the ribs between, so from across a valley it streaks down the couloirs rather than
    // capping every bump of the wall (D395).
    let steep = 1.0 - smoothstep(STEEP_SNOW.0, STEEP_SNOW.1, normal.dot(up));
    let snow = if snow > 0.0 && steep > 0.0 {
        snow * (1.0 - steep * (1.0 - smoothstep(SNOW_BED.0, SNOW_BED.1, bed(point))))
    } else {
        snow
    };
    // Avalanches off a range's walls feed its couloirs and the cirque floors and ledges under
    // them, so in summer their snow lies well below the snowline, as it lies there some
    // hundreds of metres higher: the Tatras' and the Alps' summer snow patches. On a wall the
    // depth is carried up to about 70 degrees, where the ground shader lays it only on the
    // ledges its joints and bedding break the rock into (D426), as the Matterhorn's and the
    // Odle's faces are streaked; without, the walls past 42 degrees were bare and the range
    // held snow on 4 percent of its rock (the r7 review).
    let (snow, couloir) = if planet.has_mountains()
        && matches!(
            made_of,
            material::STONE | material::SCREE | material::GRAVEL
        ) {
        let fed = round.snow_m(point + up * FED_M, up) * FED_SHARE;
        let level = smoothstep(FED_WALL.0, FED_WALL.1, normal.dot(up));
        let here = smoothstep(SNOW_BED.0, SNOW_BED.1, bed(point));
        let snow = snow.max(fed * level.max(here));
        // Avalanches run down the gullies and fill the couloirs they gather into: a couloir
        // with gully above it all the way up lies deep (D426, round 10), a dimple alone thin.
        // Only in a gully's own bed: caught wherever gully lay above, the ribs between held it
        // too, and each horn's upper wall lay in one white cap brighter than the sky (round 11).
        if fed > COULOIR_FED_M && level < 1.0 && here > 0.0 {
            let couloir = (1.0 - level) * here * couloir_catch(point, normal, up, bed);
            (snow.max(couloir * fed), couloir)
        } else {
            (snow, 0.0)
        }
    } else {
        (snow, 0.0)
    };
    let (loose, footing) = match made_of {
        material::SCREE => (1.0, Footing::Loose),
        material::GRAVEL => (2.0, Footing::Loose),
        // A wall's couloir, to the ground shader, as rock below none: its avalanche snow
        // (`couloir_catch`), told from the wall's own fed snow, which lies as deep on its tops.
        material::STONE => (-couloir, Footing::Rock),
        _ => (0.0, Footing::Soil),
    };
    let footing = if above_water < 0.0 {
        Footing::Water
    } else {
        footing
    };
    let rock_or_woods = if footing == Footing::Rock {
        1.0
    } else {
        -round.stand(point, normal, footing).density
    };
    ([loose, snow], [rock_or_woods, above_water])
}

/// How far `point` lies in a wall's couloir, 0 to 1: the gullies' beds averaged up the fall line
/// above it, a flow accumulation along one path, as the snow and the water of the slope above run
/// down into it. Its avalanche snow lies as deep as the wall's fed snow, no deeper: twice as deep,
/// the horns' tops lay past the depth the ground shader breaks into rock and couloirs (`ledge_cut`)
/// and stood in solid white caps under the skyline, brighter than the sky (round 11).
/// Terrain tools (World Machine's and Gaea's flow maps) accumulate over a heightfield's every
/// cell (D8); a chunk's vertices see no grid of the range above them, so one path stands in.
fn couloir_catch(point: Vec3, normal: Vec3, up: Vec3, bed: &mut dyn FnMut(Vec3) -> f32) -> f32 {
    let uphill = up - normal * normal.dot(up);
    let level_up = (uphill - up * uphill.dot(up)).normalize_or_zero();
    if level_up == Vec3::ZERO {
        return 0.0;
    }
    let mut gathered = 0.0;
    for reach in COULOIR_PATH_M {
        gathered += bed(point + level_up * reach);
    }
    let flow = gathered / COULOIR_PATH_M.len() as f32;
    smoothstep(COULOIR_FLOW.0, COULOIR_FLOW.1, flow)
}

/// Where along the level way uphill a couloir's catch is gathered, in metres.
const COULOIR_PATH_M: [f32; 5] = [0.0, 20.0, 50.0, 110.0, 220.0];
/// A couloir holds snow from the first of these mean depths into its gullies' beds up the path,
/// whole from the second.
const COULOIR_FLOW: (f32, f32) = (0.1, 0.3);
/// A wall holds a couloir's snow where its fed snow lies at least this deep, in metres: deepened
/// by more, the tops' snow lay in one white sheet brighter than the sky (round 10).
const COULOIR_FED_M: f32 = 0.05;

/// How far above a lake a vertex is said to be when no lake is near.
const DRY_M: f32 = 50.0;

/// How much higher a range's avalanche-fed snow lies as, in metres, and the share of that
/// depth it keeps: from 350 m the horns kept only their tips white, and from 1200 m their
/// upper third, short of the third of the range above the tree line D425 asks (round 12).
const FED_M: f32 = 1750.0;
const FED_SHARE: f32 = 1.0;
/// A wall carries its fed snow's depth from the first of these, how far its normal points up
/// (about 72 degrees), whole from the second (about 57).
const FED_WALL: (f32, f32) = (0.3, 0.55);

/// Snow keeps to the gullies on ground steeper than the first of these, how far a vertex's
/// normal points up (about 53 degrees), lying whole under the second (about 42 degrees), as
/// it lies on faces under about 50 degrees (D425): from 30 degrees it left the horns' shoulders
/// and cirque floors bare (the r6 review)...
const STEEP_SNOW: (f32, f32) = (0.6, 0.74);
/// ...lying from the first of these shares of the way down into a gully's bed, deep from the
/// second: low, as most gullies are cut shallow (`Mountains::gully_depths`), their beds a
/// share of the deepest's, and buttresses narrow them to slots.
const SNOW_BED: (f32, f32) = (0.12, 0.4);

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
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    // Only a world with mountains carries what they lay, so other worlds' ground costs no more.
    if planet.has_mountains() {
        // The gullies' beds at the chunk's own coarseness, nodes shared by its vertices.
        let mut beds = field.memo_for(terra_voxel::VOXEL_M * (1u32 << id.level) as f32);
        let (laid, bare): (Vec<[f32; 2]>, Vec<[f32; 2]>) = chunk
            .materials
            .iter()
            .zip(chunk.positions.iter().zip(&chunk.normals))
            .map(|(weights, (&point, &normal))| {
                let mut bed = |at: Vec3| field.field().gully_bed(at, &mut beds);
                lying(planet, point, normal, weights.ids[0], &mut bed)
            })
            .unzip();
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, laid);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, bare);
    }
    Some(
        mesh.with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, chunk.positions)
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
    asked: Option<Res<crate::quality::Asked>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut terrain: ResMut<Terrain>,
) {
    let viewer = camera.translation;
    // The preset is chosen in the first frame; one asked for is known already.
    let reach = asked
        .and_then(|asked| asked.0)
        .unwrap_or_default()
        .ground_reach();
    let wanted = wanted(&planet, viewer, reach);
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

/// Chooses the chunks again, on a worker, when the camera has moved on or the ground has
/// changed, and once they are chosen starts meshing those that are new or out of date.
fn choose(
    planet: Res<Planet>,
    options: Res<Options>,
    camera: Single<&GlobalTransform, With<MainCamera>>,
    quality: Res<crate::quality::Quality>,
    mut terrain: ResMut<Terrain>,
) {
    let viewer = camera.translation();
    let edits = planet.voxels().edit_count();
    let reach = quality.ground_reach();
    if terrain.choosing.is_none() {
        // A preset that meshes the ground further out chooses again where the camera is.
        let moved = quality.is_changed()
            || terrain
                .chosen_from
                .is_none_or(|from| from.distance(viewer) > RECHOOSE_M);
        if !moved && edits == terrain.edits_seen {
            return;
        }
        // Chunks an edit reached are meshed again even if nothing else changed.
        let since: Vec<(Vec3, f32)> = planet.voxels().edits_since(terrain.edits_seen).collect();
        terrain.changed.extend(since);
        terrain.edits_seen = edits;
        // A dig can open chunks that held no surface before, so the ground is chosen
        // again after any edit, not only when the camera moves.
        if moved {
            terrain.chosen_from = Some(viewer);
        }
        let from = planet.clone();
        terrain.choosing =
            Some(AsyncComputeTaskPool::get().spawn(async move { wanted(&from, viewer, reach) }));
    }
    let Some(task) = terrain.choosing.as_mut() else {
        return;
    };
    let chosen = if options.scripted() && !options.bench {
        Some(block_on(task))
    } else {
        block_on(future::poll_once(task))
    };
    let Some(chosen) = chosen else {
        return;
    };
    terrain.choosing = None;
    terrain.wanted = chosen;
    let changed = std::mem::take(&mut terrain.changed);
    // What is being meshed for a chunk no longer wanted stops (dropping a task cancels it).
    // Left to finish, the meshing of chunks walked past piles up faster than it is done,
    // keeps the threads busy long after the walk, and keeps old chunks from being taken
    // away, which waits for nothing to be pending.
    let Terrain {
        pending, wanted, ..
    } = &mut *terrain;
    pending.retain(|id, _| wanted.contains_key(id));
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
        // A scripted run waits for each chunk, so it sees the same ground every time. A
        // benchmark does not, so it times what a player would see: waiting stalls a frame
        // each time the camera has moved far enough for chunks to be chosen again.
        let ready = if options.scripted() && !options.bench {
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
