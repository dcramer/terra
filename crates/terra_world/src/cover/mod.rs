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
pub mod litter;

use crate::detail::DetailScale;
use crate::footprint::Footprint;
use crate::planet::{MainCamera, Planet};
use crate::start::Start;
use bevy::{
    light::NotShadowCaster,
    pbr::{ExtendedMaterial, MaterialExtension},
    platform::collections::HashMap,
    prelude::*,
    render::{
        render_resource::{AsBindGroup, Face},
        storage::ShaderBuffer,
    },
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
    /// How much of the forest floor is fallen needles, under conifers, rather than leaves.
    pub needles: f32,
    /// How much of it is duff and roots: at a tree's foot, along a path's edge.
    pub roots: f32,
    /// How much canopy stands over it, from open sky to none seen: the ground's light from
    /// the sky is dimmed and warmed by it (D216).
    pub shade: f32,
    /// How much taller than usual the grass grows: 0 as usual, 1 half as tall again, as a
    /// meadow's does in the open.
    pub tall: f32,
    /// How close it is to something standing or lying on the floor, a trunk, a log or a
    /// rock, from 0 away from it to 1 against it: where litter heaps up and climbs onto it
    /// (D294).
    pub heap: f32,
}

impl Growing {
    /// Whether the ground is covered by something other than grass: leaves, needles, moss
    /// or duff, as the bare-ground measure counts it.
    pub fn floored(litter: f32, moss: f32, needles: f32, roots: f32) -> bool {
        litter + moss + needles + roots > 0.5
    }
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
    /// Where blades bend away from bodies walking through them and shrink from the camera
    /// (`crate::parting`, D271).
    #[storage(101, read_only)]
    pub parting: Handle<ShaderBuffer>,
    /// What the climate leaves round the camera (`climate::ClimateMaps`, D327): where it is,
    /// whether there is any, which painting, and grass's layer of the leaves map, or -1. The
    /// blades sink under snow, brown in the long cold and take rime in a frost.
    #[uniform(100)]
    pub climate_from_world: Mat4,
    #[uniform(100)]
    pub climate_on: u32,
    #[uniform(100)]
    pub climate_painting: u32,
    #[uniform(100)]
    pub grass_layer: i32,
    /// The colour grass browns to, linear (its year's `autumn`).
    #[uniform(100)]
    pub grass_autumn: Vec4,
    #[texture(102)]
    #[sampler(103)]
    pub climate_ground: Option<Handle<Image>>,
    #[texture(104, dimension = "2d_array")]
    #[sampler(105)]
    pub climate_leaves: Option<Handle<Image>>,
}

impl GrassExtension {
    /// Reads the climate's maps as they are now painted.
    pub fn read_climate(&mut self, maps: &crate::climate::ClimateMaps) {
        self.climate_from_world = maps.from_world;
        self.climate_on = maps.ground.is_some() as u32;
        self.climate_painting = maps.painting;
        self.grass_layer = maps.grass_layer;
        self.grass_autumn = maps.autumn_of(maps.grass_layer);
        self.climate_ground = maps.ground.clone();
        self.climate_leaves = maps.leaves.clone();
    }
}

impl MaterialExtension for GrassExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/grass.wesl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/grass.wesl".into()
    }

    // Shaded only where in front, after the prepass (`grown::test_depth_early`).
    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        crate::grown::test_depth_early(descriptor, key.mesh_key);
        Ok(())
    }
}

pub type GrassMaterial = ExtendedMaterial<StandardMaterial, GrassExtension>;

/// A square of grass: what it is drawn as now, and what is being built to replace it.
struct Square {
    ring: usize,
    entity: Option<Entity>,
    litter: Option<Entity>,
    covered: Vec<bool>,
    path: Vec<bool>,
    built: bool,
    building: Option<(usize, Task<Built>)>,
}

#[derive(Resource)]
struct Meadow {
    material: Handle<GrassMaterial>,
    /// What fallen leaves, needles and twigs are drawn with, and the cards they are cut from.
    litter: Handle<litter::LitterMaterial>,
    cards: Arc<litter::LitterCards>,
    squares: HashMap<(i32, i32), Square>,
    /// What is built or planned, which the squares are kept clear of.
    clear: Arc<Vec<Footprint>>,
    /// Each footprint as the squares were last grown for, so a change grows again only the
    /// squares where it changed.
    seen: HashMap<Entity, Footprint>,
}

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<GrassMaterial>::default())
        .add_plugins(MaterialPlugin::<litter::LitterMaterial>::default())
        .init_resource::<BareGround>()
        .add_systems(
            Update,
            (grow, measure, follow_climate)
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
    footprints: Query<(Entity, Ref<Footprint>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GrassMaterial>>,
    mut litter_materials: ResMut<Assets<litter::LitterMaterial>>,
    assets: Res<AssetServer>,
    detail: Res<DetailScale>,
    quality: Res<crate::quality::Quality>,
    parting: Option<Res<crate::parting::Parting>>,
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
    if detail.is_changed()
        && let Some(meadow) = &meadow
        && let Some(mut material) = litter_materials.get_mut(&meadow.litter)
    {
        material.extension.fade =
            Vec4::new(litter::FADE_M[0], litter::FADE_M[1], 0.0, 0.0) * detail.0;
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
                parting: parting.map(|parting| parting.0.clone()).unwrap_or_default(),
                climate_from_world: Mat4::IDENTITY,
                climate_on: 0,
                climate_painting: 0,
                grass_layer: -1,
                grass_autumn: Vec4::splat(0.2),
                climate_ground: None,
                climate_leaves: None,
            },
        });
        let cards = crate::grown::Cards::read_file(litter::FILE);
        let litter = litter_materials.add(litter::LitterMaterial {
            base: StandardMaterial {
                base_color_texture: Some(assets.load(cards.atlas.clone())),
                normal_map_texture: Some(crate::grown::leaf_normals(&cards, &assets)),
                alpha_mode: AlphaMode::Mask(cards.cut),
                perceptual_roughness: 0.85,
                // No sheen: under a canopy the sky it would reflect is mostly hidden, which
                // the sky occlusion volume (D270) tells the diffuse light but not this.
                reflectance: 0.0,
                cull_mode: None::<Face>,
                double_sided: true,
                ..default()
            },
            extension: litter::LitterExtension {
                fade: Vec4::new(litter::FADE_M[0], litter::FADE_M[1], 0.0, 0.0) * detail.0,
                climate_from_world: Mat4::IDENTITY,
                climate_on: 0,
                climate_painting: 0,
                climate_ground: None,
            },
        });
        commands.insert_resource(Meadow {
            material,
            litter,
            cards: Arc::new(litter::LitterCards::from(&cards)),
            squares: HashMap::default(),
            clear: Arc::new(
                footprints
                    .iter()
                    .map(|(_, f)| f.into_inner().clone())
                    .collect(),
            ),
            seen: footprints
                .iter()
                .map(|(entity, f)| (entity, f.into_inner().clone()))
                .collect(),
        });
        return;
    };
    let Meadow {
        material,
        litter,
        cards,
        squares,
        clear,
        seen,
    } = &mut *meadow;
    // When something built changes shape, the squares where it changed, the places it took
    // or gave up and a blade's clearance round them, are built again without what now
    // stands there; the rest of a big base is not looked at again.
    let mut changes: Vec<Footprint> = Vec::new();
    for (entity, footprint) in &footprints {
        if footprint.is_changed() {
            let footprint = footprint.into_inner();
            changes.extend(footprint.changes_from(seen.get(&entity)));
            seen.insert(entity, footprint.clone());
        }
    }
    if !changes.is_empty() {
        *clear = Arc::new(
            footprints
                .iter()
                .map(|(_, f)| f.into_inner().clone())
                .collect(),
        );
        let to_start = start.compute_affine().inverse();
        let mut touched: Vec<(i32, i32)> = Vec::new();
        for change in &changes {
            let reach = change.cell_m * 0.71 + blades::CLEAR_M + CELL_M;
            for place in change.places() {
                let middle = to_start.transform_point3(change.frame.transform_point(Vec3::new(
                    (place.x as f32 + 0.5) * change.cell_m,
                    0.0,
                    (place.y as f32 + 0.5) * change.cell_m,
                )));
                let key = |m: f32| (m / SQUARE_M).floor() as i32;
                for i in key(middle.x - reach)..=key(middle.x + reach) {
                    for j in key(middle.z - reach)..=key(middle.z + reach) {
                        if !touched.contains(&(i, j)) {
                            touched.push((i, j));
                        }
                    }
                }
            }
        }
        for key in touched {
            if let Some(square) = squares.get_mut(&key) {
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
        if !keep {
            for entity in [square.entity, square.litter].into_iter().flatten() {
                commands.entity(entity).despawn();
            }
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
        for old in [square.entity.take(), square.litter.take()]
            .into_iter()
            .flatten()
        {
            commands.entity(old).despawn();
        }
        square.litter = built.litter.map(|(place, mesh)| {
            commands
                .spawn((
                    Name::new("Litter"),
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(litter.clone()),
                    place,
                    NotShadowCaster,
                ))
                .id()
        });
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
        let (source, planet, start, seed, clear, cards) = (
            rules.source.clone(),
            planet.clone(),
            *start,
            rules.seed,
            clear.clone(),
            cards.clone(),
        );
        let shape: Ring = RINGS[ring].0.with_density(density);
        let task = threads.spawn(async move {
            blades::square(
                source.as_ref(),
                &planet,
                &start,
                seed,
                key,
                shape,
                &clear,
                Some(&cards),
            )
        });
        let square = squares.entry(key).or_insert_with(|| Square {
            ring,
            entity: None,
            litter: None,
            covered: Vec::new(),
            path: Vec::new(),
            built: false,
            building: None,
        });
        square.building = Some((ring, task));
    }
}

/// The grass reads the climate's maps again each time they are painted.
fn follow_climate(
    maps: Option<Res<crate::climate::ClimateMaps>>,
    meadow: Option<Res<Meadow>>,
    mut materials: ResMut<Assets<GrassMaterial>>,
    mut litters: ResMut<Assets<litter::LitterMaterial>>,
) {
    let (Some(maps), Some(meadow)) = (maps, meadow) else {
        return;
    };
    if !maps.is_changed() && !meadow.is_added() {
        return;
    }
    if let Some(mut grass) = materials.get_mut(&meadow.material) {
        grass.extension.read_climate(&maps);
    }
    if let Some(mut litter) = litters.get_mut(&meadow.litter) {
        litter.extension.read_climate(&maps);
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
                        litter: None,
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
