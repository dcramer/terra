//! Ground cover (docs/biomes.md, the cover tier): grass generated round the camera, which
//! no one has to agree on, so it is built on this machine alone, on other threads as the
//! camera moves, and dropped behind it.
//!
//! What grows where is the layer above's to say, through a [`CoverRules`]: a field of how
//! much grass grows at each place, how lush it is, and where paths are. The cover only
//! draws it: blades in squares round the camera, finer near it ([`blades`]), shrinking into
//! the ground with distance so the grass has no edge.
//!
//! Nothing grows where a base has built or planned anything: its [`Footprint`]s are kept
//! clear, and the squares under one are built again when it changes.
//!
//! It also measures what docs/biomes.md asks to be measured: the share of the ground within
//! 30 m of the camera that nothing grows on ([`BareGround`]), which scripted runs print with
//! their screenshots.

pub mod blades;
mod floor;

use crate::detail::DetailScale;
use crate::footprint::Footprint;
use crate::planet::{MainCamera, Planet};
use crate::start::Start;
use bevy::{
    light::NotShadowCaster,
    pbr::{ExtendedMaterial, MaterialExtension},
    platform::collections::HashMap,
    prelude::*,
    render::render_resource::{AsBindGroup, Face},
    shader::ShaderRef,
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};
use blades::{Built, CELL_M, CELLS, RINGS, Ring};
use std::sync::Arc;
use terra_gen::Seed;

/// Grass is built in squares this wide, in metres east and south of the start.
pub const SQUARE_M: f32 = 8.0;
/// Grass grows within this of the camera.
pub const REACH_M: f32 = 64.0;
/// Blades shrink into the ground over this stretch, so the grass has no edge.
const FADE_M: [f32; 2] = [42.0, REACH_M - 2.0];
/// A square changes ring only once the camera is this far past where the rings meet, so
/// it does not flick between them.
const HYSTERESIS_M: f32 = 2.0;
/// At most this many squares are being built at once, and this many arrive in a frame.
const BUILDING: usize = 8;
const ARRIVING: usize = 2;
/// The bare-ground measure looks this far round the camera.
pub const MEASURED_M: f32 = 30.0;

/// What grows at one place.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Growing {
    /// How thickly grass grows, from none to a full lawn.
    pub grass: f32,
    /// Dry at 0, green at 1.
    pub lush: f32,
    /// A path runs here: bare on purpose.
    pub path: bool,
    /// How much of the ground is forest floor, fallen leaves, where the grass gives out.
    pub litter: f32,
    /// How much moss grows on it.
    pub moss: f32,
}

/// What a world says grows where. Asked from other threads, so it holds what it needs
/// rather than reaching into the world.
pub trait CoverSource: Send + Sync + 'static {
    /// What grows at `at`, in metres east and south of the start, whose point on the ground
    /// is `ground`.
    fn growing(&self, planet: &Planet, ground: Vec3, at: Vec2) -> Growing;

    /// Whether anything may grow in the square round `middle`, `half` metres each way, so a
    /// square with nothing in it is skipped at once.
    fn reaches(&self, _middle: Vec2, _half: f32) -> bool {
        true
    }
}

/// The world's rules for what covers the ground. The layer above inserts it; without it
/// nothing grows.
#[derive(Resource, Clone)]
pub struct CoverRules {
    pub source: Arc<dyn CoverSource>,
    pub seed: Seed,
}

/// The share of the ground within [`MEASURED_M`] of the camera that nothing grows on, and
/// of that what paths take, as docs/biomes.md asks: none should be bare unless it means
/// something.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct BareGround {
    pub bare: f32,
    pub paths: f32,
    /// Whether every square within reach was built when it was measured.
    pub complete: bool,
}

/// What the grass shader is told. Field order must match the struct in grass.wesl.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct GrassExtension {
    #[uniform(100)]
    pub wind: Vec4,
    #[uniform(100)]
    pub fade: Vec4,
    /// How blades thin out one by one with distance (`blades::thinning`).
    #[uniform(100)]
    pub thinning: Vec4,
}

impl MaterialExtension for GrassExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/grass.wesl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/grass.wesl".into()
    }
}

pub type GrassMaterial = ExtendedMaterial<StandardMaterial, GrassExtension>;

/// A square of grass: what it is drawn as now, and what is being built to replace it.
struct Square {
    ring: usize,
    entity: Option<Entity>,
    covered: Vec<bool>,
    path: Vec<bool>,
    built: bool,
    building: Option<(usize, Task<Built>)>,
}

#[derive(Resource)]
struct Meadow {
    material: Handle<GrassMaterial>,
    squares: HashMap<(i32, i32), Square>,
    /// What is built or planned, which the squares are kept clear of.
    clear: Arc<Vec<Footprint>>,
}

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<GrassMaterial>::default())
        .init_resource::<BareGround>()
        .add_systems(
            Update,
            (grow, measure)
                .chain()
                .run_if(resource_exists::<CoverRules>),
        )
        .add_plugins(floor::plugin);
}

/// The ring of detail for a square this far from the camera, keeping the one it has
/// unless the camera is well past where they meet.
fn ring_for(distance: f32, now: Option<usize>) -> usize {
    let plain = RINGS
        .iter()
        .position(|(_, end)| distance < *end)
        .unwrap_or(RINGS.len() - 1);
    match now {
        Some(now) if now != plain => {
            let edge = if plain > now {
                RINGS[now].1
            } else {
                RINGS[plain].1
            };
            if (distance - edge).abs() < HYSTERESIS_M {
                now
            } else {
                plain
            }
        }
        _ => plain,
    }
}

fn square_middle((i, j): (i32, i32)) -> Vec2 {
    (Vec2::new(i as f32, j as f32) + 0.5) * SQUARE_M
}

/// How far a square's nearest point is: a square changes ring by it, so none of its
/// blades is still near enough to show when its ring changes.
fn square_nearest(key: (i32, i32), here: Vec2) -> f32 {
    let corner = Vec2::new(key.0 as f32, key.1 as f32) * SQUARE_M;
    here.clamp(corner, corner + SQUARE_M).distance(here)
}

/// Builds the squares round the camera, nearest first, each on another thread so moving
/// never waits for one; rebuilds a square in another ring when the camera has moved far
/// enough; and takes away those left behind.
fn grow(
    mut commands: Commands,
    rules: Res<CoverRules>,
    meadow: Option<ResMut<Meadow>>,
    planet: Res<Planet>,
    start: Res<Start>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    footprints: Query<Ref<Footprint>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GrassMaterial>>,
    detail: Res<DetailScale>,
    quality: Res<crate::quality::Quality>,
) {
    let density = quality.grass();
    let Some(camera) = camera.iter().next() else {
        return;
    };
    // How far detail reaches scales the rings and the fade as it does every other level of
    // detail (D230).
    if detail.is_changed()
        && let Some(meadow) = &meadow
        && let Some(mut material) = materials.get_mut(&meadow.material)
    {
        material.extension.fade = Vec4::new(FADE_M[0], FADE_M[1], 0.0, 0.0) * detail.0;
        material.extension.thinning =
            blades::thinning(density) * Vec4::new(1.0, detail.0, 1.0, detail.0);
    }
    let Some(mut meadow) = meadow else {
        let material = materials.add(GrassMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                perceptual_roughness: 0.85,
                reflectance: 0.15,
                // Light through the blades from behind.
                diffuse_transmission: 0.25,
                // Seen from both sides with the same normal, which leans up, so a blade
                // does not flip from lit to dark as it turns.
                cull_mode: None::<Face>,
                double_sided: false,
                ..default()
            },
            // The wind blows along the start's east, as it does through the trees.
            extension: GrassExtension {
                wind: (start.rotation * Vec3::X).extend(1.0),
                fade: Vec4::new(FADE_M[0], FADE_M[1], 0.0, 0.0),
                thinning: blades::thinning(density),
            },
        });
        commands.insert_resource(Meadow {
            material,
            squares: HashMap::default(),
            clear: Arc::new(footprints.iter().map(|f| f.into_inner().clone()).collect()),
        });
        return;
    };
    let Meadow {
        material,
        squares,
        clear,
    } = &mut *meadow;
    // When something built changes shape, the squares on or beside it, before or after,
    // are built again without what now stands there.
    let changed: Vec<Footprint> = footprints
        .iter()
        .filter(|f| f.is_changed())
        .map(|f| f.into_inner().clone())
        .collect();
    if !changed.is_empty() {
        let before = std::mem::replace(
            clear,
            Arc::new(footprints.iter().map(|f| f.into_inner().clone()).collect()),
        );
        let touched = |key: (i32, i32)| {
            let middle = square_middle(key);
            let point = planet.ground_at(start.at(middle.x, middle.y));
            changed
                .iter()
                .chain(before.iter())
                .any(|f| f.covers(point, SQUARE_M * 0.75))
        };
        for (&key, square) in squares.iter_mut() {
            if touched(key) {
                // Built again, from scratch: what is drawn stays until then, and one being
                // built from the old footprints is dropped.
                square.building = None;
                square.ring = usize::MAX;
            }
        }
    }
    // A new quality preset grows every square again at its density.
    if quality.is_changed() && !quality.is_added() {
        for square in squares.values_mut() {
            square.building = None;
            square.ring = usize::MAX;
        }
        if let Some(mut grass) = materials.get_mut(&*material) {
            grass.extension.thinning =
                blades::thinning(density) * Vec4::new(1.0, detail.0, 1.0, detail.0);
        }
    }
    let local = start.local(camera.translation());
    let here = Vec2::new(local.x, local.z);

    squares.retain(|&key, square| {
        let keep = square_middle(key).distance(here) < REACH_M * detail.0 + SQUARE_M;
        if !keep && let Some(entity) = square.entity {
            commands.entity(entity).despawn();
        }
        keep
    });

    let mut building = 0;
    let mut arrived = 0;
    for square in squares.values_mut() {
        let Some((ring, task)) = &mut square.building else {
            continue;
        };
        if arrived >= ARRIVING {
            building += 1;
            continue;
        }
        let Some(built) = block_on(poll_once(task)) else {
            building += 1;
            continue;
        };
        arrived += 1;
        square.ring = *ring;
        square.building = None;
        square.built = true;
        square.covered = built.covered;
        square.path = built.path;
        if let Some(old) = square.entity.take() {
            commands.entity(old).despawn();
        }
        square.entity = built.mesh.map(|(place, mesh)| {
            commands
                .spawn((
                    Name::new("Grass"),
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(material.clone()),
                    place,
                    NotShadowCaster,
                ))
                .id()
        });
    }

    // What is wanted: squares within reach that are missing or in the wrong ring, nearest
    // first.
    let reach = (REACH_M * detail.0 / SQUARE_M).ceil() as i32;
    let (ci, cj) = (
        (here.x / SQUARE_M).floor() as i32,
        (here.y / SQUARE_M).floor() as i32,
    );
    let mut wanted: Vec<(f32, (i32, i32), usize)> = (-reach..=reach)
        .flat_map(|di| (-reach..=reach).map(move |dj| (ci + di, cj + dj)))
        .filter_map(|key| {
            if square_middle(key).distance(here) >= REACH_M * detail.0 {
                return None;
            }
            let distance = square_nearest(key, here) / detail.0;
            let square = squares.get(&key);
            let current = square
                .map(|s| s.building.as_ref().map_or(s.ring, |(ring, _)| *ring))
                .filter(|&ring| ring < RINGS.len());
            let ring = ring_for(distance, current);
            (current != Some(ring)).then_some((distance, key, ring))
        })
        .collect();
    wanted.sort_by(|a, b| a.0.total_cmp(&b.0));
    let threads = AsyncComputeTaskPool::get();
    for (_, key, ring) in wanted.into_iter().take(BUILDING.saturating_sub(building)) {
        let (source, planet, start, seed, clear) = (
            rules.source.clone(),
            planet.clone(),
            *start,
            rules.seed,
            clear.clone(),
        );
        let shape: Ring = RINGS[ring].0.with_density(density);
        let task = threads.spawn(async move {
            blades::square(source.as_ref(), &planet, &start, seed, key, shape, &clear)
        });
        let square = squares.entry(key).or_insert_with(|| Square {
            ring,
            entity: None,
            covered: Vec::new(),
            path: Vec::new(),
            built: false,
            building: None,
        });
        square.building = Some((ring, task));
    }
}

/// Measures the bare ground round the camera, about twice a second.
fn measure(
    meadow: Option<Res<Meadow>>,
    start: Res<Start>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    time: Res<Time<Real>>,
    mut last: Local<f32>,
    mut bare: ResMut<BareGround>,
) {
    let (Some(meadow), Some(camera)) = (meadow, camera.iter().next()) else {
        return;
    };
    if time.elapsed_secs() - *last < 0.5 {
        return;
    }
    *last = time.elapsed_secs();
    let local = start.local(camera.translation());
    *bare = measure_bare(&meadow.squares, Vec2::new(local.x, local.z));
}

fn measure_bare(squares: &HashMap<(i32, i32), Square>, here: Vec2) -> BareGround {
    let (mut cells, mut bare, mut paths) = (0u32, 0u32, 0u32);
    let mut complete = true;
    let reach = (MEASURED_M / SQUARE_M).ceil() as i32 + 1;
    let (ci, cj) = (
        (here.x / SQUARE_M).floor() as i32,
        (here.y / SQUARE_M).floor() as i32,
    );
    for di in -reach..=reach {
        for dj in -reach..=reach {
            let key = (ci + di, cj + dj);
            let corner = Vec2::new(key.0 as f32, key.1 as f32) * SQUARE_M;
            let square = squares.get(&key).filter(|square| square.built);
            for b in 0..CELLS {
                for a in 0..CELLS {
                    let at = corner + (Vec2::new(a as f32, b as f32) + 0.5) * CELL_M;
                    if at.distance(here) > MEASURED_M {
                        continue;
                    }
                    cells += 1;
                    let Some(square) = square else {
                        complete = false;
                        bare += 1;
                        continue;
                    };
                    let index = b * CELLS + a;
                    if !square.covered[index] {
                        bare += 1;
                        if square.path[index] {
                            paths += 1;
                        }
                    }
                }
            }
        }
    }
    let share = |n: u32| {
        if cells == 0 {
            0.0
        } else {
            n as f32 / cells as f32
        }
    };
    BareGround {
        bare: share(bare),
        paths: share(paths),
        complete,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_square_keeps_its_ring_until_the_camera_is_well_past_the_edge() {
        let edge = RINGS[0].1;
        assert_eq!(ring_for(edge - 5.0, None), 0);
        assert_eq!(ring_for(edge + 5.0, None), 1);
        assert_eq!(
            ring_for(edge + 1.0, Some(0)),
            0,
            "just past the edge, it stays"
        );
        assert_eq!(ring_for(edge + HYSTERESIS_M + 0.5, Some(0)), 1);
        assert_eq!(ring_for(edge - 1.0, Some(1)), 1, "just inside, it stays");
        assert_eq!(ring_for(edge - HYSTERESIS_M - 0.5, Some(1)), 0);
    }

    #[test]
    fn ground_with_nothing_built_counts_as_bare_and_a_path_is_told_apart() {
        let mut squares = HashMap::default();
        assert_eq!(measure_bare(&squares, Vec2::ZERO).bare, 1.0);
        let reach = (MEASURED_M / SQUARE_M).ceil() as i32 + 1;
        for i in -reach..=reach {
            for j in -reach..=reach {
                // Every cell covered, but half of every square's rows a bare path.
                let path: Vec<bool> = (0..CELLS * CELLS).map(|n| n / CELLS < CELLS / 2).collect();
                let covered: Vec<bool> = path.iter().map(|p| !p).collect();
                squares.insert(
                    (i, j),
                    Square {
                        ring: 0,
                        entity: None,
                        covered,
                        path,
                        built: true,
                        building: None,
                    },
                );
            }
        }
        let measured = measure_bare(&squares, Vec2::ZERO);
        assert!(measured.complete);
        assert!((measured.bare - 0.5).abs() < 0.02, "{measured:?}");
        assert_eq!(measured.bare, measured.paths);
    }
}
