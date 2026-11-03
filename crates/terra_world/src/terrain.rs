//! Drawing the voxel ground (docs/terrain.md, A2): chunks around the camera at the detail
//! `terra_voxel` chooses, meshed on other threads, and drawn again where the ground is dug
//! or filled.
//!
//! The first view is meshed before the first frame, across every core, so nothing is
//! seen half built, all but its coarsest levels (`STREAM_FROM`), kilometres off, which are
//! meshed after the loading screen lifts (`crate::streaming`). After that, as the camera moves on, the chunks it now wants are meshed
//! in the background, and the ones it no longer wants stay until their replacements are
//! ready, so the ground never shows holes. Which chunks are wanted is worked out on a
//! worker too, as Voxel Tools updates its level-of-detail octree off the main thread: it
//! took 17 ms of a frame every 4 m walked on a 30 km world. No frame waits for either: a
//! scripted run is held while the ground is coming (`loading::hold`), so the same script
//! shows the same ground on any machine.

use crate::instance::{Building, InWorld, InstanceApp};
use crate::loading::{self, Progress};
use crate::planet::MainCamera;
use crate::planet::{GroundLook, Planet};
use crate::streaming::Streaming;
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
    Detail, Field, Sides, below_horizon, chunk_origin, chunk_size_m, chunks_around, material,
    may_hold_surface, mesh_chunk,
};

/// The camera moves this far before the chunks are chosen again.
const RECHOOSE_M: f32 = 4.0;

/// The first view's chunks this coarse and coarser (2 km and wider, from some kilometres
/// off) are meshed after the loading screen lifts, on the streaming pool: on the mountain
/// lake they were 39% of the first view's work (levels 7 to 9, 420 of 1080 thread-seconds).
const STREAM_FROM: u8 = 7;

/// A chunk of the ground on screen.
#[derive(Component)]
pub struct Chunk;

/// What is drawn, what is being meshed, and where the chunks were last chosen from.
#[derive(Resource, Default)]
pub(crate) struct Terrain {
    shown: HashMap<ChunkId, Shown>,
    pending: HashMap<ChunkId, Pending>,
    wanted: HashMap<ChunkId, Sides>,
    chosen_from: Option<Vec3>,
    /// The preset's reach the chunks were last chosen at (`Quality::ground_reach`).
    chosen_reach: f32,
    /// How many edits the ground had when its chunks were last brought up to date.
    edits_seen: usize,
    /// The chunks wanted from a place, being worked out; and the edits made since the
    /// chunks were last brought up to date, whose chunks are meshed again once they are.
    choosing: Option<Task<HashMap<ChunkId, Sides>>>,
    changed: Vec<(Vec3, f32)>,
    /// The first view's far chunks, meshed once the loading screen lifts ([`stream_far`]).
    later: HashMap<ChunkId, Sides>,
    /// When they began streaming and how many, until the last is in, for the log.
    streaming_since: Option<(std::time::Instant, usize)>,
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
    /// One of the first view's far chunks, which the loading screen does not wait for.
    streamed: bool,
}

/// The first view of each world is meshed in this set, on entering it, from where the
/// camera stands once its transform is propagated: whatever stands the camera where play
/// starts (the player's arrival) goes in `Building::Arrive`, so the first view is the one
/// seen and not meshed again where the camera is put on the first frame.
#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
pub struct FirstView;

/// The ground of the world the run is in: its chunks shown and coming are the world's (D469).
pub fn plugin(app: &mut App) {
    app.init_world_resource::<Terrain>()
        .configure_sets(OnEnter(InWorld), FirstView.in_set(Building::View))
        .add_systems(OnEnter(InWorld), first_view.in_set(FirstView))
        .init_resource::<Streaming>()
        .add_systems(
            Update,
            (
                stream_far.run_if(crate::streaming::lifted),
                choose,
                receive,
                track.in_set(loading::Track),
            )
                .chain(),
        );
}

impl Terrain {
    /// How much of the ground is still coming: chunks being meshed or waiting to be, and a
    /// choice being made. A scripted run is held for all of it (`loading::hold`).
    pub(crate) fn coming(&self) -> usize {
        self.pending.len() + self.later.len() + usize::from(self.choosing.is_some())
    }

    /// What of it the loading screen waits for: all but the first view's far chunks.
    fn coming_near(&self) -> usize {
        let streamed = self.pending.values().filter(|p| p.streamed).count();
        self.pending.len() - streamed + usize::from(self.choosing.is_some())
    }
}

/// The loading screen waits for the ground in view to be meshed, but for its far levels.
fn track(terrain: Res<Terrain>, mut progress: ResMut<Progress>) {
    progress.report(
        "Shaping the ground",
        terrain.coming_near(),
        terrain.shown.len(),
    );
}

/// Once the loading screen has lifted, meshes the first view's far chunks on the streaming
/// pool, the nearer (finer) levels first.
fn stream_far(planet: Res<Planet>, streaming: Res<Streaming>, mut terrain: ResMut<Terrain>) {
    if terrain.later.is_empty() {
        return;
    }
    let mut later: Vec<(ChunkId, Sides)> = terrain.later.drain().collect();
    later.sort_by_key(|(id, _)| (id.level, id.coord.to_array()));
    terrain.streaming_since = Some((std::time::Instant::now(), later.len()));
    for (id, sides) in later {
        let planet = planet.clone();
        let task = streaming.0.spawn(async move { mesh(&planet, id, sides) });
        let streamed = true;
        (terrain.pending).insert(
            id,
            Pending {
                sides,
                task,
                streamed,
            },
        );
    }
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

/// The chunks a viewer at a place should see: those the surface may pass through, less those
/// below the horizon from anywhere the camera goes before they are chosen again. The tree is
/// shaped first, so a chunk the horizon hides changes no neighbour's seams.
fn wanted(planet: &Planet, viewer: Vec3, reach: f32) -> HashMap<ChunkId, Sides> {
    let ground = planet.voxels();
    let detail = detail(planet, reach);
    chunks_around(&[viewer], &detail, |id| may_hold_surface(&**ground, id))
        .into_iter()
        .filter(|&(id, _)| !below_horizon(id, viewer, &detail, HORIZON_MOVED_M))
        .collect()
}

/// How far the camera may go between one choice of chunks and the next one coming in, for
/// the horizon's cull: far past `RECHOOSE_M`, as a choice on a worker takes a while.
const HORIZON_MOVED_M: f32 = 64.0;

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
    let (snow, couloir) = laid_snow(
        round,
        planet.has_mountains(),
        point,
        normal,
        made_of,
        bed,
        &|p, n| round.snow_m(p, n),
    );
    let (loose, footing) = match made_of {
        material::SCREE => (1.0, Footing::Loose),
        material::GRAVEL => (2.0, Footing::Loose),
        // A wall's couloir, to the ground shader, as rock below none: how far down into it its
        // avalanche snow lies (`couloir_key`), told from the wall's own fed snow, which lies as deep on its tops.
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
        // The woods as the trees stand them: none in a snowbed (`snowbed_m`).
        let stand = round.stand(point, normal, footing);
        if stand.density > 0.0 {
            let snowbed = |p: Vec3, n: Vec3| round.snowbed_m(p, n);
            let mountains = planet.has_mountains();
            let laid = laid_snow(round, mountains, point, normal, made_of, bed, &snowbed);
            -stand.under_snow(laid.0).density
        } else {
            0.0
        }
    };
    ([loose, snow], [rock_or_woods, above_water])
}

/// How deep the snow lies at a point on the ground facing `normal`, in metres, as the land lays
/// it, `snow_m` the climate's on the day asked, and how far down into a wall's couloir the point
/// lies (`couloir_key`): on a mountain's walls fed by the avalanches off the range above (D395,
/// D426).
fn laid_snow(
    round: &terra_land::round::Round,
    mountains: bool,
    point: Vec3,
    normal: Vec3,
    made_of: MaterialId,
    bed: &mut dyn FnMut(Vec3) -> f32,
    snow_m: &dyn Fn(Vec3, Vec3) -> f32,
) -> (f32, f32) {
    let up = point.normalize_or(Vec3::Y);
    let snow = snow_m(point, normal);
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
    if mountains
        && matches!(
            made_of,
            material::STONE | material::SCREE | material::GRAVEL
        )
    {
        let height_m = round.place(point, up).height_m;
        let fed = snow_m(point + up * fed_from_m(height_m), up) * FED_SHARE;
        let level = smoothstep(FED_WALL.0, FED_WALL.1, normal.dot(up));
        let in_bed = bed(point);
        let here = smoothstep(SNOW_BED.0, SNOW_BED.1, in_bed);
        let gullied = if here > 0.0 {
            snow_m(point + up * FED_GULLY_M, up) * FED_SHARE
        } else {
            0.0
        };
        // A low wall's benches keep the open feed only by the gullies that cross them, where
        // the avalanches off the bands above come to rest; level along their whole length,
        // every bench of the town's foothills lay in one hard white bar, a terrace of contour
        // lines (settlement r29-30).
        let low = 1.0 - smoothstep(FED_FALLS.0, FED_FALLS.1, height_m);
        let open = if low > 0.0 && level > 0.0 && fed > 0.0 {
            fed * bench_keep(low, here.max(gully_by(point, up, bed)))
        } else {
            fed
        };
        let snow = snow.max(fed_lying(open, gullied, level, here));
        // Avalanches run down the gullies and fill the couloirs they gather into: a couloir
        // with gully above it all the way up lies deep (D426, round 10), a dimple alone thin.
        // Only in a gully's own bed: caught wherever gully lay above, the ribs between held it
        // too, and each horn's upper wall lay in one white cap brighter than the sky (round 11).
        // Fed as its bed is (`FED_GULLY_M`): the far snow's breaks bare a foothill's slopes
        // but for its couloirs, so fed from `FED_M` alone its gullies lay bare too (r28).
        let fed = fed.max(gullied);
        if fed > COULOIR_FED_M && level < 1.0 && here > 0.0 {
            let flow = couloir_catch(point, normal, up, bed);
            let couloir = (1.0 - level) * here * flow;
            (snow.max(couloir * fed), couloir_key(level, in_bed, flow))
        } else {
            (snow, 0.0)
        }
    } else {
        (snow, 0.0)
    }
}

/// How deep the snow lies on [`terra_land::round::SNOWBED_DATE`] at a point on the ground
/// facing `normal`, in metres, as the land lays it: the climate's, and on a mountain's rock and
/// scree what its walls feed their benches and gullies. No tree stands where it lies
/// (`terra_land::woods::Stand::under_snow`); without the walls' feed, trees stood on benches
/// white with it (mountains round 30's review).
pub(crate) fn snowbed_m(
    planet: &Planet,
    round: &terra_land::round::Round,
    point: Vec3,
    normal: Vec3,
) -> f32 {
    let up = point.normalize_or(Vec3::Y);
    // No snow here and none where its walls' feed comes from, at most `FED_GULLY_M` higher: none
    // laid, without asking the ground what it is made of, which is most of the cost over a
    // forest's hundreds of thousands of trees (it added 20 s to the town's woods).
    if round.snowbed_m(point, normal) <= 0.0
        && round.snowbed_m(point + up * FED_GULLY_M.max(FED_M), up) <= 0.0
    {
        return 0.0;
    }
    let made_of = round.land.ground.sample(point - up * 0.3).material;
    let field = planet.voxels();
    let mut memo = field.memo_for(terra_voxel::VOXEL_M);
    let mut bed = |at: Vec3| field.field().gully_bed(at, &mut memo);
    let snow_m = |p: Vec3, n: Vec3| round.snowbed_m(p, n);
    laid_snow(
        round,
        planet.has_mountains(),
        point,
        normal,
        made_of,
        &mut bed,
        &snow_m,
    )
    .0
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

/// How far down into a wall's couloir a point lies, 0 to 1, for the ground shader to lay its
/// avalanche snow by (`caught` in ground.wesl): the gully's bed there, as deep as it lies, where
/// the slope above it gathers into it (`couloir_catch`), on walls only. Its bed as it lies, not
/// past a threshold: the shader keys the snow to the deepest part of it, so it lies in the
/// gullies' bottoms and the ribs between stand bare at any slope. Terrain tools lay a peak's
/// snow so, by a flow or concavity mask the slope above feeds, sharpened, rather than by the
/// slope of each facet (Wysilab Instant Terra's Advanced Snow, whose snow flows onto the
/// concave ground; World Machine's flow-map snow, its flow blurred and its contrast raised).
/// Keyed to the fragment's slope instead, a horn's walls at 72 to 80 degrees, on the snow's
/// slope limit, lost it bump by bump of the far mesh, in round dark holes the judges read as
/// camouflage blotches (mountains rounds 10 to 12).
fn couloir_key(level: f32, in_bed: f32, flow: f32) -> f32 {
    (1.0 - level) * in_bed.clamp(0.0, 1.0) * flow
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
/// upper third, short of the third of the range above the tree line D425 asks (round 12);
/// 1900 m once the aerial perspective held the far range under the sky (round 13); 1400 m when
/// the horns stood half white against the third D425 and D426 ask (the r20 review).
const FED_M: f32 = 1400.0;
const FED_SHARE: f32 = 1.0;
/// A gully's bed is fed as from this much higher, as round 13's whole range was: sheltered and
/// shaded, its avalanche snow outlasts the open faces' (the Tatras' summer snow lies in the
/// gullies' beds hundreds of metres under the open faces' snow). Fed from [`FED_M`] alone, the
/// town's foothills, under the open faces' reach, lay bare and flat in one brown value at dusk
/// (settlement r27); the horns' open faces and ledges keep `FED_M`'s depth, two thirds rock.
const FED_GULLY_M: f32 = 1900.0;

/// Over these heights above the sea, in metres, a wall's open feed rises from [`FED_GULLY_M`]
/// to [`FED_M`]: a low wall has more of the range above it to be fed from, and its benches
/// and ledges hold the avalanches' snow in streaks, while the horns keep two thirds rock (D425).
/// Fed from `FED_M` everywhere, the town's foothills lay bare and flat (settlement r27-28). The
/// band is wider than the feeds differ, so the snow still lies deeper the higher it is.
const FED_FALLS: (f32, f32) = (1500.0, 2500.0);

/// How much higher a wall at `height_m` above the sea is fed snow as, in metres.
fn fed_from_m(height_m: f32) -> f32 {
    FED_GULLY_M + (FED_M - FED_GULLY_M) * smoothstep(FED_FALLS.0, FED_FALLS.1, height_m)
}

/// The depth of a wall's fed snow, in metres: `open_m` fed from [`FED_M`] on its tops and ledges
/// (`level`, how level it lies, 0 to 1), the deeper of that and `gully_m`, fed from
/// [`FED_GULLY_M`], in a gully's bed (`here`, how far down into it, 0 to 1).
fn fed_lying(open_m: f32, gully_m: f32, level: f32, here: f32) -> f32 {
    (open_m * level).max(open_m.max(gully_m) * here)
}

/// How near a gully's bed `point` lies, 0 to 1, looking round it level at [`BENCH_GULLY_M`]:
/// the most of a gully's bed there, the farther ring counted less.
fn gully_by(point: Vec3, up: Vec3, bed: &mut dyn FnMut(Vec3) -> f32) -> f32 {
    let (a, b) = up.any_orthonormal_pair();
    let mut near: f32 = 0.0;
    for (reach, weight) in BENCH_GULLY_M {
        for way in [a, b, -a, -b] {
            near = near.max(smoothstep(SNOW_BED.0, SNOW_BED.1, bed(point + way * reach)) * weight);
        }
    }
    near
}

/// The share of the open feed a bench of a wall keeps, as low as `low` (1 under
/// [`FED_FALLS`], 0 over) says, `near` a gully's bed (`gully_by`): whole by a gully, a
/// [`BENCH_KEEP`] away from any, so its snow lies in fans under the gullies, lined up down
/// the wall from bench to bench, rather than along each bench.
fn bench_keep(low: f32, near: f32) -> f32 {
    1.0 - low * (1.0 - BENCH_KEEP) * (1.0 - near)
}

/// How far round a point a bench looks for a gully's bed, in metres, and how much each ring counts.
const BENCH_GULLY_M: [(f32, f32); 2] = [(15.0, 1.0), (35.0, 0.6)];
/// The share of the open feed a low wall's bench keeps away from any gully.
const BENCH_KEEP: f32 = 0.15;

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

/// From this level on, chunks 64 m across and more, a few hundred metres off, the sky a far
/// wall's vertex sees is dimmed by how its ground folds round it within tens of metres
/// (`terra_voxel::openness`, D434): its gullies, the corners under its cliff bands, its
/// notches. Coarser than the pits `sky_seen` finds, and the far mesh cannot shade them: lit
/// alike, a range at dusk read as smooth plaster (the mountains r22 review). Eased in over
/// the next two levels, so no chunk's sky changes at once as it splits.
const FOLDED_FROM: u8 = 2;
/// The reaches it looks within: a cliff band's riser and a couloir between buttresses. Within
/// 14 and 42 m a far wall's vertices scored 0.93 on average, 0.08 either way, and the lake's
/// and valley's views changed by under a fifth of a percent of their pixels (mountains r23).
const FOLD_REACHES_M: [f32; 2] = [40.0, 120.0];
/// The openness a wall's open ground scores, under which its sky dims, and how fast: within
/// 40 and 120 m a far wall scores 0.84 on average, 0.12 either way, so the folds dim it by
/// 0.3 either way of a quarter, and the deepest are left a third of the sky.
const FOLD_OPEN: f32 = 0.95;
const FOLD_GAIN: f32 = 2.5;
const FOLD_DEEPEST: f32 = 0.3;
/// Ground this level is left as it is; past the steeper share, the folds dim it whole, so a
/// far meadow's hollows stay lit and a wall's gullies and benches do not.
const FOLD_LEVEL: (f32, f32) = (0.97, 0.85);

/// How much of the sky a far chunk's vertex sees for the folds of the ground round it.
fn folded<F: Field>(field: &F, memo: &mut F::Memo, level: u8, point: Vec3, normal: Vec3) -> f32 {
    let eased = (f32::from(level - FOLDED_FROM) + 1.0) / 3.0;
    let steep = smoothstep(
        FOLD_LEVEL.0,
        FOLD_LEVEL.1,
        normal.dot(point.normalize_or(Vec3::Y)),
    );
    let weight = eased.min(1.0) * steep;
    if weight <= 0.0 {
        return 1.0;
    }
    let open = terra_voxel::openness(field, memo, point, normal, &FOLD_REACHES_M);
    let sky = (1.0 - FOLD_GAIN * (FOLD_OPEN - open)).clamp(FOLD_DEEPEST, 1.0);
    1.0 - weight * (1.0 - sky)
}

/// A chunk's mesh as Bevy draws it, or nothing if it has no surface.
fn to_mesh(planet: &Planet, id: ChunkId, chunk: ChunkMesh) -> Option<Mesh> {
    if chunk.indices.is_empty() {
        return None;
    }
    let field = &**planet.voxels();
    let mut memo = field.memo_for(terra_voxel::VOXEL_M);
    // A far chunk of a world with mountains asks how its ground folds at its own coarseness.
    let mut folds = (planet.has_mountains() && id.level >= FOLDED_FROM)
        .then(|| field.memo_for(terra_voxel::VOXEL_M * (1u32 << id.level) as f32));
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
            if let Some(memo) = folds.as_mut() {
                colour[3] *= folded(field, memo, id.level, point, normal);
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
    camera: Single<&GlobalTransform, With<MainCamera>>,
    asked: Option<Res<crate::quality::Asked>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut terrain: ResMut<Terrain>,
) {
    let started = std::time::Instant::now();
    let viewer = camera.translation();
    // The preset is chosen in the first frame; one asked for is known already.
    let reach = asked
        .and_then(|asked| asked.0)
        .unwrap_or_default()
        .ground_reach();
    let wanted = wanted(&planet, viewer, reach);
    let chosen = started.elapsed().as_secs_f32();
    let (chunks, later): (Vec<(ChunkId, Sides)>, Vec<(ChunkId, Sides)>) = wanted
        .iter()
        .map(|(&id, &sides)| (id, sides))
        .partition(|(id, _)| id.level < STREAM_FROM);
    terrain.later = later.into_iter().collect();
    let threads = terra_gen::par::width();
    let planet_ref = &*planet;
    let meshed = terra_gen::par::map(&chunks, |&(id, sides)| {
        let at = std::time::Instant::now();
        let chunk = mesh_chunk(&**planet_ref.voxels(), id, sides);
        let shaped = at.elapsed().as_secs_f32();
        let vertices = chunk.positions.len();
        let mesh = to_mesh(planet_ref, id, chunk);
        let coloured = at.elapsed().as_secs_f32() - shaped;
        (id, sides, mesh, [shaped, coloured], vertices)
    });
    let count = meshed.len();
    let mut levels: std::collections::BTreeMap<u8, (usize, f32, f32, usize)> = default();
    for (id, sides, mesh, [shaped, coloured], vertices) in meshed {
        let level = levels.entry(id.level).or_default();
        *level = (
            level.0 + 1,
            level.1 + shaped,
            level.2 + coloured,
            level.3 + vertices,
        );
        let entity = show(&mut commands, &mut meshes, &look, id, mesh);
        terrain.shown.insert(id, Shown { sides, entity });
    }
    // How long, on how many threads: what a change to a chunk's meshing or colouring costs.
    let secs = started.elapsed().as_secs_f32();
    let by_level: Vec<String> = levels
        .iter()
        .map(|(level, (n, shaped, coloured, vertices))| {
            format!("level {level} {n} chunks, {vertices} vertices, {shaped:.1} s shaped and {coloured:.1} s coloured")
        })
        .collect();
    info!(
        "Meshed the first view of the ground: {count} chunks in {secs:.2} s on {threads} threads, {chosen:.2} s of it choosing them, {} far ones of level {STREAM_FROM} and up left for after the loading screen; {} (summed over threads).",
        terrain.later.len(),
        by_level.join("; ")
    );
    terrain.wanted = wanted;
    terrain.chosen_from = Some(viewer);
    terrain.chosen_reach = reach;
    terrain.edits_seen = planet.voxels().edit_count();
}

/// Chooses the chunks again, on a worker, when the camera has moved on, the preset's reach
/// or the ground has changed, and once they are chosen starts meshing those that are new or
/// out of date. A choice started is never taken on the frame it starts, so a scripted run's
/// frame that moved the camera or dug always shows the ground as it was, and the next is
/// held until the new ground is drawn (`loading::hold`).
fn choose(
    planet: Res<Planet>,
    camera: Single<&GlobalTransform, With<MainCamera>>,
    quality: Res<crate::quality::Quality>,
    mut terrain: ResMut<Terrain>,
) {
    let viewer = camera.translation();
    let edits = planet.voxels().edit_count();
    let reach = quality.ground_reach();
    if terrain.choosing.is_none() {
        // A preset that meshes the ground further out chooses again where the camera is;
        // one that only says it changed (its first frame) does not.
        let moved = reach != terrain.chosen_reach
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
            terrain.chosen_reach = reach;
        }
        let from = planet.clone();
        terrain.choosing =
            Some(AsyncComputeTaskPool::get().spawn(async move { wanted(&from, viewer, reach) }));
        return;
    }
    let Some(task) = terrain.choosing.as_mut() else {
        return;
    };
    let Some(chosen) = block_on(future::poll_once(task)) else {
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
        pending,
        wanted,
        later,
        ..
    } = &mut *terrain;
    pending.retain(|id, _| wanted.contains_key(id));
    later.retain(|id, sides| wanted.get(id) == Some(sides));
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
        // A far chunk of the first view still waiting for the loading screen to lift.
        let waiting = terrain.later.contains_key(&id) && !touched(id);
        if up_to_date || underway || waiting {
            continue;
        }
        terrain.later.remove(&id);
        let planet = planet.clone();
        let task = pool.spawn(async move { mesh(&planet, id, sides) });
        let streamed = false;
        (terrain.pending).insert(
            id,
            Pending {
                sides,
                task,
                streamed,
            },
        );
    }
}

/// Draws chunks whose meshes are ready, and once nothing is waiting, takes away the ones
/// no longer wanted. No frame waits for a mesh, as streamed terrain is drawn in shipped
/// engines: the loading screen waits for the ground coming at the start ([`track`]), and a
/// scripted run is held, its frames not counted and its clock stopped, while the ground is
/// coming after (`loading::hold`), so it sees the same ground on any machine. Waiting here
/// instead stalled the play world's first frame 277 s on the async pool's four threads.
fn receive(
    mut commands: Commands,
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
        let Some(mesh) = block_on(future::poll_once(&mut pending.task)) else {
            continue;
        };
        let sides = pending.sides;
        terrain.pending.remove(&id);
        if let Some(old) = terrain.shown.remove(&id).and_then(|shown| shown.entity) {
            commands.entity(old).despawn();
        }
        let entity = show(&mut commands, &mut meshes, &look, id, mesh);
        terrain.shown.insert(id, Shown { sides, entity });
    }
    // The far chunks streaming in hold back no stale chunk's going: they are kilometres off,
    // and the chunks walked past would pile up, drawn over each other, meanwhile.
    let streaming = terrain.pending.values().any(|pending| pending.streamed);
    if let (false, Some((since, count))) = (streaming, terrain.streaming_since) {
        terrain.streaming_since = None;
        let secs = since.elapsed().as_secs_f32();
        info!(
            "Streamed the first view's far ground: {count} chunks in {secs:.1} s after the loading screen lifted"
        );
    }
    if terrain.pending.values().all(|pending| pending.streamed) {
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

#[cfg(test)]
#[path = "terrain_unit_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "terrain_tests.rs"]
mod streaming_tests;
