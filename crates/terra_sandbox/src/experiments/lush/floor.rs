//! The lush forest's floor (D246): what grows and lies under its trees, by the light the
//! canopy lets through, as a real floor's does.
//!
//! - **Grass follows the light.** Thick and tall in the clearings, along the paths and in
//!   the gaps between crowns; under a closed canopy it gives out to fallen leaves, to
//!   needles under the pines and spruces, to moss and to beds of ferns and herbs, so the
//!   floor is always full but never one lawn.
//! - **Feet in the ground.** Grass is pushed back from every trunk, a ring of duff, roots
//!   and moss round its foot; fallen logs and rocks lie in duff of their own, rocks sunk a
//!   fifth to two fifths of their height; a path's edge frays into dirt, roots and leaves.
//! - **An eye-height layer in drifts.** Bracken, ferns, herbs, hazel, bilberry, oak scrub,
//!   saplings, dead branches and flowers, each grown as a few variants of its own tint, and
//!   placed as a clustered process (a Thomas process, as Horizon Zero Dawn scatters its
//!   plants on density maps): drift centres where the light, the trunks, the logs and a
//!   noise of the kind's own like it, and a clump round each.

use super::species::Species;
use super::{
    BARK_M, BUSH_KINDS, Build, CLEARING_M, FADE, FOREST_M, Foliage, Footing, Limb, Path, SUN_ONLY,
    Scanned, grow_bush, smoothstep, square, tube,
};
use bevy::{
    camera::visibility::{RenderLayers, VisibilityRange},
    gltf::GltfMaterial,
    image::ImageLoaderSettings,
    light::NotShadowCaster,
    pbr::{ExtendedMaterial, MaterialExtension},
    platform::collections::HashMap,
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use std::f32::consts::{PI, TAU};
use terra_gen::{Noise, Rng, Seed};
use terra_world::cover::Growing;
use terra_world::flora::Growth;
use terra_world::flora_kinds::{Cuttable, Flora};
use terra_world::grown::build::{self as grown_build, Placing};
use terra_world::grown::{Cards, Habit, TreeDef, plants, tree as grown_tree};
use terra_world::planet::Planet;

/// A rock's scanned material with moss over what faces the sky (`mossy.wesl`).
pub(super) type Mossy = ExtendedMaterial<StandardMaterial, MossExtension>;

/// What the moss shader is told. Field order must match the struct in mossy.wesl.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub(super) struct MossExtension {
    /// The moss's colour, linear, and in w how much grows.
    #[uniform(100)]
    colour: Vec4,
}

impl MaterialExtension for MossExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/mossy.wesl".into()
    }
}

/// Rocks' glTF materials still to be copied into the mossy materials they are drawn with.
#[derive(Resource, Default)]
pub(super) struct MossyScans(Vec<(Handle<GltfMaterial>, Handle<Mossy>)>);

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<Mossy>::default())
        .init_resource::<MossyScans>()
        .add_systems(Update, dress_mossy);
}

/// Copies each rock scan's glTF material, once loaded, under its moss.
fn dress_mossy(
    mut scans: ResMut<MossyScans>,
    gltf: Res<Assets<GltfMaterial>>,
    mut materials: ResMut<Assets<Mossy>>,
) {
    scans.0.retain(|(from, to)| {
        let (Some(from), Some(mut to)) = (gltf.get(from), materials.get_mut(to)) else {
            return true;
        };
        to.base = StandardMaterial {
            base_color: from.base_color,
            base_color_texture: from.base_color_texture.clone(),
            normal_map_texture: from.normal_map_texture.clone(),
            metallic_roughness_texture: from.metallic_roughness_texture.clone(),
            occlusion_texture: from.occlusion_texture.clone(),
            perceptual_roughness: from.perceptual_roughness,
            metallic: from.metallic,
            reflectance: 0.3,
            ..default()
        };
        false
    });
}

/// Trees, logs and rocks are filed for the floor in squares this wide.
const FLOOR_SQUARE_M: f32 = 8.0;
/// How far round a place the canopy over it is looked for: the widest crown.
const CROWN_REACH: i32 = 2;
/// How many variants of each kind of undergrowth are grown, each its own tint.
const VARIANTS: u64 = 6;

/// A tree as the floor sees it: where its foot is, how far its bark is from its middle
/// there, how far its crown spreads and how much of the sky it hides under it, and whether
/// it drops needles.
#[derive(Clone, Copy)]
pub(super) struct Stand {
    at: Vec2,
    trunk_m: f32,
    crown_m: f32,
    closes: f32,
    conifer: bool,
}

impl Stand {
    /// A tree of a species `height` tall: an oak's crown spreads a third of its height, a
    /// veteran's half, a pine's and spruce's a sixth; a pine lets much of the light through,
    /// a spruce little; young trees less of either.
    pub(super) fn of(
        at: Vec2,
        species: Species,
        young: bool,
        veteran: bool,
        height: f32,
        trunk_m: f32,
    ) -> Self {
        let (spread, closes) = match species {
            Species::Oak if veteran => (0.5, 0.85),
            Species::Oak => (0.33, 0.82),
            Species::Pine => (0.17, 0.62),
            Species::Spruce => (if young { 0.24 } else { 0.18 }, 0.92),
        };
        Self {
            at,
            // The root flare spreads past the trunk.
            trunk_m: trunk_m * 1.3,
            crown_m: height * spread,
            closes: closes * if young { 0.6 } else { 1.0 },
            conifer: species != Species::Oak,
        }
    }
}

/// The trees over the floor, filed by square.
#[derive(Default)]
pub(super) struct Crowns {
    filed: HashMap<(i32, i32), Vec<Stand>>,
}

/// What the canopy makes of a place: how much of the sky it hides, how much of what falls
/// is needles, and how far the nearest bark is.
struct Over {
    shade: f32,
    needles: f32,
    bark: f32,
}

impl Crowns {
    pub(super) fn add(&mut self, stand: Stand) {
        self.filed
            .entry(square(stand.at, FLOOR_SQUARE_M))
            .or_default()
            .push(stand);
    }

    fn over(&self, at: Vec2) -> Over {
        let (i, j) = square(at, FLOOR_SQUARE_M);
        let (mut light, mut needles_miss, mut bark) = (1.0f32, 1.0f32, 8.0f32);
        for di in -CROWN_REACH..=CROWN_REACH {
            for dj in -CROWN_REACH..=CROWN_REACH {
                for stand in self.filed.get(&(i + di, j + dj)).into_iter().flatten() {
                    let d = stand.at.distance(at);
                    bark = bark.min(d - stand.trunk_m);
                    // Densest under the middle of the crown, none past its edge.
                    let under = 1.0 - smoothstep(0.4 * stand.crown_m, 1.05 * stand.crown_m, d);
                    light *= 1.0 - under * stand.closes;
                    if stand.conifer {
                        // Needles fall a little past the crown.
                        let fall = 1.0 - smoothstep(0.6 * stand.crown_m, 1.5 * stand.crown_m, d);
                        needles_miss *= 1.0 - fall * 0.9;
                    }
                }
            }
        }
        Over {
            shade: 1.0 - light,
            needles: 1.0 - needles_miss,
            bark,
        }
    }
}

/// Circles of what lies on the floor, logs and rocks, filed by square.
#[derive(Default)]
struct Lying {
    filed: HashMap<(i32, i32), Vec<(Vec2, f32)>>,
}

impl Lying {
    fn add(&mut self, at: Vec2, radius: f32) {
        self.filed
            .entry(square(at, FLOOR_SQUARE_M))
            .or_default()
            .push((at, radius));
    }

    /// How far a place is from the nearest edge of what lies there, up to 8 m; negative
    /// under it.
    fn edge(&self, at: Vec2) -> f32 {
        let (i, j) = square(at, FLOOR_SQUARE_M);
        let mut nearest = 8.0f32;
        for di in -1..=1 {
            for dj in -1..=1 {
                for (middle, radius) in self.filed.get(&(i + di, j + dj)).into_iter().flatten() {
                    nearest = nearest.min(middle.distance(at) - radius);
                }
            }
        }
        nearest
    }
}

/// The forest's map, on a flat patch laid over the ground: east and south of the start, in
/// metres. Asked by the ground cover from other threads.
pub(super) struct Map {
    path: Path,
    canopy: Crowns,
    lying: Lying,
    noise: Noise,
}

/// What the floor is like at a place, for the undergrowth to choose by.
struct Here {
    /// How much of the sky's light reaches it, with the light down a path's corridor.
    light: f32,
    needles: f32,
    /// How far the nearest bark, the nearest log or rock's edge, and the nearest path's
    /// edge are.
    bark: f32,
    lying: f32,
    path: f32,
    /// Damp hollows, where ferns and moss grow best.
    wet: f32,
}

impl Map {
    fn noise(&self, at: Vec2, height: f32, across: f32) -> f32 {
        0.5 + 0.5 * self.noise.sample(Vec3::new(at.x, height, at.y) / across)
    }

    /// How far outside a path's edge a place is, the edge wandering and fraying.
    fn path_edge(&self, at: Vec2) -> f32 {
        self.path.edge_distance(at)
            + (self.noise(at, 71.0, 1.9) - 0.5) * 0.8
            + (self.noise(at, 73.0, 0.5) - 0.5) * 0.3
    }

    fn here(&self, at: Vec2) -> Here {
        let over = self.canopy.over(at);
        let path = self.path_edge(at);
        // Gaps in the canopy, where a crown is thin or a tree has fallen.
        let gap = smoothstep(0.58, 0.82, self.noise(at, 19.0, 10.0));
        let shade = over.shade * (1.0 - 0.6 * gap);
        // A path's corridor lets the light down to its edges.
        let corridor = 1.0 - smoothstep(0.5, 4.0, path.max(0.0));
        Here {
            light: (1.0 - shade + 0.3 * corridor).min(1.0),
            needles: over.needles,
            bark: over.bark,
            lying: self.lying.edge(at),
            path,
            wet: smoothstep(0.35, 0.75, self.noise(at, 61.0, 13.0)),
        }
    }

    /// What grows at a place: grass by the light, thick in the open and in gaps and along
    /// paths, giving out under a closed canopy to leaves, needles under conifers, moss in
    /// the damp; pushed back from trunks, logs and rocks by a ring of duff and roots; none
    /// on the paths, whose edges fray into dirt, roots and leaves.
    pub(super) fn growing(&self, at: Vec2) -> Growing {
        let here = self.here(at);
        let path = here.path;
        // The path's edge: no grass on it, short and broken grass in a fraying band beside
        // it, dirt, roots and leaves in that band.
        let off_path = smoothstep(0.05, 1.1, path);
        let fray = (1.0 - smoothstep(0.0, 1.0, path)) * smoothstep(-0.25, 0.1, path);
        let patch = 0.6
            + 0.4
                * self
                    .noise
                    .fbm(Vec3::new(at.x, 0.0, at.y) / 9.0, 2, 2.0, 0.5);
        // In the shade grass grows only where a fleck of light falls, in tufts.
        let tufts = smoothstep(0.55, 0.8, self.noise(at, 23.0, 2.6));
        let lit = smoothstep(0.22, 0.72, here.light);
        // Pushed back from the bark, the logs and the rocks.
        let wander = (self.noise(at, 53.0, 1.3) - 0.5) * 1.2;
        let foot = smoothstep(0.2, 0.85, here.bark + wander * 0.4);
        let beside = smoothstep(0.0, 0.55, here.lying + wander * 0.3);
        let grass = off_path
            * foot
            * beside
            * patch
            // Thinner and drier on the needles under pines.
            * (1.0 - 0.55 * smoothstep(0.2, 0.7, here.needles))
            * (lit + (1.0 - lit) * 0.3 * tufts);
        let floor = 1.0 - lit;
        let needles = smoothstep(0.15, 0.6, here.needles) * off_path.max(0.5);
        let mossy = self.noise(at, 31.0, 3.5);
        // Duff and roots round a tree's foot, its edge wandering; along logs and rocks.
        let ring = 1.0 - smoothstep(0.0, 1.5, here.bark + wander);
        let along = 1.0 - smoothstep(0.0, 0.7, here.lying + wander * 0.4);
        let roots = (ring * 0.95)
            .max(along * 0.7)
            .max(fray * (0.35 + 0.5 * self.noise(at, 79.0, 0.9)));
        let moss = (ring * (0.25 + 0.6 * mossy))
            .max(along * 0.5)
            .max(floor * smoothstep(0.55, 0.85, mossy * 0.7 + here.wet * 0.4) * 0.85)
            * off_path.max(0.3);
        Growing {
            grass,
            // Grass in the shade is a deeper green; in the open it dries in patches.
            lush: self.noise(at, 7.0, 20.0).max(floor * 0.8),
            path: path < 0.0,
            litter: (floor * (1.0 - needles)).max(fray * 0.7) * smoothstep(-0.3, 0.3, path),
            moss,
            needles,
            roots,
            shade: 1.0 - here.light,
            // Tall meadow grass in patches where the sun reaches.
            tall: smoothstep(0.7, 0.95, here.light)
                * smoothstep(0.4, 0.7, self.noise(at, 41.0, 7.0)),
        }
    }
}

/// What the floor is laid with: where the ground is, the forest's materials, and the
/// world's assets.
pub(super) struct Lay<'a, 'w, 's> {
    pub commands: &'a mut Commands<'w, 's>,
    pub planet: &'a Planet,
    pub start: Vec3,
    pub east: Vec3,
    pub south: Vec3,
    pub assets: &'a AssetServer,
    pub meshes: &'a mut Assets<Mesh>,
    pub plain: &'a mut Assets<StandardMaterial>,
    pub scanned: &'a mut Scanned,
    pub mossy: &'a mut Assets<Mossy>,
    pub mossy_scans: &'a mut MossyScans,
    pub flora: &'a Flora,
    pub cards: &'a Cards,
    /// Oak bark, which logs, stumps, sticks and saplings are of.
    pub bark: Handle<Foliage>,
    /// Pine bark, for logs and stumps among the conifers.
    pub pine_bark: Handle<Foliage>,
    /// The cards of the shared atlas, and the bushes' own leaves.
    pub atlas: Handle<Foliage>,
    pub leaves: Handle<Foliage>,
    pub seed: Seed,
}

/// From one distance to another, crossfading at the far end.
fn within(m: f32) -> VisibilityRange {
    VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: m * (1.0 - FADE)..m,
        use_aabb: false,
    }
}

/// A place in a clump `spread` across round `at`: Gaussian, as a Thomas process scatters.
fn round(dice: &mut Rng, at: Vec2, spread: f32) -> Vec2 {
    let r = libm::sqrtf(-2.0 * libm::logf(dice.unit().max(1e-4))) * spread;
    let angle = dice.range(0.0, TAU);
    at + Vec2::new(libm::cosf(angle), libm::sinf(angle)) * r
}

/// A tint for one variant: a little lighter or darker, greener or yellower.
fn variant_tint(dice: &mut Rng, base: Vec3) -> Vec3 {
    let value = dice.range(0.82, 1.12);
    base * Vec3::new(
        value * dice.range(0.92, 1.1),
        value,
        value * dice.range(0.8, 1.05),
    )
}

fn tint_build(colours: &mut [[f32; 4]], tint: Vec3) {
    for colour in colours {
        colour[0] *= tint.x;
        colour[1] *= tint.y;
        colour[2] *= tint.z;
    }
}

/// The kinds of undergrowth.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Under {
    Bracken,
    Fern,
    Frond,
    Herb,
    Hazel,
    Bilberry,
    Scrub,
    Sapling,
    Branches,
    Flowers,
}

/// How each kind of undergrowth is placed: how many drift centres are tried, how many
/// plants a clump has and how far it spreads, how big they are, how far each is drawn and
/// how near its shadow is cast, and how broad its drifts are.
struct Placement {
    kind: Under,
    name: &'static str,
    tries: u32,
    clump: (u32, u32),
    spread_m: f32,
    scale: (f32, f32),
    seen_m: f32,
    shadow_m: f32,
    drift_m: f32,
}

const PLACEMENTS: [Placement; 10] = [
    Placement {
        kind: Under::Bracken,
        name: "Bracken",
        tries: 10000,
        clump: (4, 11),
        spread_m: 1.0,
        scale: (0.7, 1.45),
        seen_m: 55.0,
        shadow_m: 0.0,
        drift_m: 16.0,
    },
    Placement {
        kind: Under::Fern,
        name: "Fern",
        tries: 6000,
        clump: (2, 5),
        spread_m: 0.8,
        scale: (0.8, 1.6),
        seen_m: 45.0,
        shadow_m: 0.0,
        drift_m: 9.0,
    },
    Placement {
        kind: Under::Frond,
        name: "Fern",
        tries: 8000,
        clump: (4, 10),
        spread_m: 0.9,
        scale: (0.8, 1.5),
        seen_m: 45.0,
        shadow_m: 0.0,
        drift_m: 9.0,
    },
    Placement {
        kind: Under::Herb,
        name: "Herb",
        tries: 7000,
        clump: (6, 14),
        spread_m: 0.7,
        scale: (0.6, 1.1),
        seen_m: 35.0,
        shadow_m: 0.0,
        drift_m: 7.0,
    },
    Placement {
        kind: Under::Hazel,
        name: "Hazel",
        tries: 6000,
        clump: (1, 3),
        spread_m: 1.4,
        scale: (1.1, 2.1),
        seen_m: 85.0,
        shadow_m: 22.0,
        drift_m: 20.0,
    },
    Placement {
        kind: Under::Bilberry,
        name: "Bilberry",
        tries: 7000,
        clump: (5, 11),
        spread_m: 0.8,
        scale: (0.3, 0.55),
        seen_m: 40.0,
        shadow_m: 0.0,
        drift_m: 11.0,
    },
    Placement {
        kind: Under::Scrub,
        name: "Bush",
        tries: 3500,
        clump: (1, 3),
        spread_m: 1.0,
        scale: (0.7, 1.4),
        seen_m: 70.0,
        shadow_m: 20.0,
        drift_m: 14.0,
    },
    Placement {
        kind: Under::Sapling,
        name: "Sapling",
        tries: 3000,
        clump: (1, 3),
        spread_m: 2.0,
        scale: (0.7, 1.3),
        seen_m: 100.0,
        shadow_m: 25.0,
        drift_m: 24.0,
    },
    Placement {
        kind: Under::Branches,
        name: "Branches",
        tries: 3000,
        clump: (1, 2),
        spread_m: 1.2,
        scale: (0.7, 1.3),
        seen_m: 45.0,
        shadow_m: 0.0,
        drift_m: 10.0,
    },
    Placement {
        kind: Under::Flowers,
        name: "Flowers",
        tries: 5000,
        clump: (3, 9),
        spread_m: 0.6,
        scale: (0.8, 1.3),
        seen_m: 35.0,
        shadow_m: 0.0,
        drift_m: 7.0,
    },
];

/// A bell from `low` to `high`, highest between.
fn between(low: f32, high: f32, value: f32) -> f32 {
    let middle = (low + high) * 0.5;
    smoothstep(low, middle, value) * (1.0 - smoothstep(middle, high, value))
}

impl Under {
    /// How much a kind likes a place, 0 to 1, before its drifts.
    fn likes(self, here: &Here) -> f32 {
        let near = |d: f32, from: f32, to: f32| 1.0 - smoothstep(from, to, d);
        let broadleaf = 1.0 - smoothstep(0.2, 0.6, here.needles);
        match self {
            // Half shade: the edges of clearings, gaps, the paths' sides.
            Under::Bracken => between(0.2, 1.0, here.light) * (0.4 + 0.6 * broadleaf),
            // The shade, the damp, round the trunks and along the logs.
            Under::Fern | Under::Frond => {
                (1.0 - smoothstep(0.35, 0.8, here.light))
                    * (0.35 + 0.65 * here.wet)
                    * (0.5 + 0.5 * near(here.bark, 1.0, 4.0).max(near(here.lying, 0.6, 2.5)))
            }
            Under::Herb => {
                (1.0 - smoothstep(0.3, 0.7, here.light)) * broadleaf * (0.3 + 0.7 * here.wet)
            }
            // The understorey, under the edge of the canopy.
            Under::Hazel => between(-0.2, 0.8, here.light) * broadleaf,
            // Under the pines, on their needles.
            Under::Bilberry => smoothstep(0.3, 0.7, here.needles) * between(0.05, 1.0, here.light),
            // In the open and along the paths.
            Under::Scrub => {
                smoothstep(0.45, 0.9, here.light) * (0.5 + 0.5 * near(here.path, 1.0, 6.0))
            }
            // Where a gap lets the light down.
            Under::Sapling => smoothstep(0.4, 0.8, here.light) * smoothstep(2.0, 4.0, here.bark),
            // Fallen from the trees, round their feet and by the logs.
            Under::Branches => {
                (1.0 - smoothstep(0.4, 0.8, here.light))
                    * near(here.bark, 2.0, 6.0).max(near(here.lying, 1.0, 3.0))
            }
            // In the sun, and along the paths most.
            Under::Flowers => {
                smoothstep(0.65, 0.95, here.light) * (0.4 + 0.6 * near(here.path, 0.5, 4.0))
            }
        }
    }

    /// How far it stays from bark, from what lies on the floor, and from a path's edge.
    fn clear_of(self) -> (f32, f32, f32) {
        match self {
            Under::Hazel | Under::Scrub => (1.0, 0.3, 0.7),
            Under::Sapling => (2.0, 0.6, 1.2),
            Under::Branches => (0.5, 0.1, 0.4),
            Under::Flowers => (0.6, 0.1, 0.05),
            _ => (0.35, 0.1, 0.2),
        }
    }
}

/// How each variant of a kind is drawn.
#[derive(Clone)]
enum Drawn {
    /// One mesh, in a material.
    One(Handle<Mesh>, Handle<Foliage>),
    /// A scan in its own material.
    Scan(Handle<Mesh>, Handle<StandardMaterial>),
    /// A sapling: its wood and its leaves.
    Two([(Handle<Mesh>, Handle<Foliage>); 2]),
}

impl Lay<'_, '_, '_> {
    /// A place on the ground, `at` east and south of the start, turned at random about its
    /// up, sunk `sink`, `scale` times its size.
    fn on(&self, at: Vec2, dice: &mut Rng, sink: f32, scale: f32) -> Transform {
        let ground = self
            .planet
            .ground_at(self.start + self.east * at.x + self.south * at.y);
        let up = ground.normalize();
        let turn = Quat::from_axis_angle(up, dice.range(0.0, TAU));
        Transform {
            translation: ground - up * sink,
            rotation: turn * Quat::from_rotation_arc(Vec3::Y, up),
            scale: Vec3::splat(scale),
        }
    }

    /// Where a direction in the world points on the map.
    fn on_map(&self, direction: Vec3) -> Vec2 {
        Vec2::new(direction.dot(self.east), direction.dot(self.south)).normalize_or(Vec2::X)
    }

    fn cut_as(&self, name: &str, size: f32) -> Option<Cuttable> {
        self.flora
            .kinds()
            .iter()
            .find(|kind| kind.name == name)
            .and_then(|kind| kind.cuttable(size))
    }

    /// A scan's mesh, and a material of our own that its glTF material is copied into once
    /// loaded (`dress_scans`), as Bevy 0.20 loads a glTF's material as a `GltfMaterial`.
    fn scan(&mut self, file: &str, tint: Color) -> (Handle<Mesh>, Handle<StandardMaterial>) {
        let path = format!("models/nature/forest/{file}.glb");
        let material = self.plain.add(StandardMaterial::default());
        self.scanned.0.push((
            self.assets.load(
                GltfAssetLabel::Material {
                    index: 0,
                    is_scale_inverted: false,
                }
                .from_asset(path.clone()),
            ),
            material.clone(),
            tint,
        ));
        (
            self.assets.load(
                GltfAssetLabel::Primitive {
                    mesh: 0,
                    primitive: 0,
                }
                .from_asset(path),
            ),
            material,
        )
    }

    /// A rock's scan, its material under moss, `moss` of it.
    fn mossy_scan(&mut self, file: &str, moss: f32) -> (Handle<Mesh>, Handle<Mossy>) {
        let path = format!("models/nature/forest/{file}.glb");
        let material = self.mossy.add(Mossy {
            base: StandardMaterial::default(),
            extension: MossExtension {
                colour: Vec3::from_array(
                    Color::srgb(0.25, 0.32, 0.13)
                        .to_linear()
                        .to_f32_array_no_alpha(),
                )
                .extend(moss),
            },
        });
        self.mossy_scans.0.push((
            self.assets.load(
                GltfAssetLabel::Material {
                    index: 0,
                    is_scale_inverted: false,
                }
                .from_asset(path.clone()),
            ),
            material.clone(),
        ));
        (
            self.assets.load(
                GltfAssetLabel::Primitive {
                    mesh: 0,
                    primitive: 0,
                }
                .from_asset(path),
            ),
            material,
        )
    }

    /// The cut end of a log or a stump: the rings of a section of wood, weathered grey.
    fn end_grain(&mut self) -> Handle<StandardMaterial> {
        let map = |file: &str, colour: bool| -> Handle<Image> {
            self.assets
                .load_builder()
                .with_settings(move |settings: &mut ImageLoaderSettings| settings.is_srgb = colour)
                .load(format!("materials/log_end/{file}"))
        };
        let arm = map("arm.jpg", false);
        let material = StandardMaterial {
            // Weathered: cut wood greys and darkens in a season.
            base_color: Color::srgb(0.62, 0.56, 0.48),
            base_color_texture: Some(map("color.jpg", true)),
            normal_map_texture: Some(map("normal.jpg", false)),
            occlusion_texture: Some(arm.clone()),
            metallic_roughness_texture: Some(arm),
            perceptual_roughness: 1.0,
            reflectance: 0.25,
            ..default()
        };
        self.plain.add(material)
    }

    /// Each kind's variants.
    fn grow(&mut self, kind: Under) -> Vec<Drawn> {
        let mut dice = self
            .seed
            .derive("undergrowth kinds")
            .index(kind as u64)
            .rng();
        let cards = self.cards;
        let card = |name: &str| cards.get(name).clone();
        let mut out = Vec::new();
        for n in 0..VARIANTS {
            let drawn = match kind {
                Under::Bracken => {
                    let tint = variant_tint(&mut dice, Vec3::new(0.82, 0.9, 0.62));
                    let build = bracken(&mut dice, &card("fern"), tint);
                    Drawn::One(self.meshes.add(build.mesh(false)), self.atlas.clone())
                }
                Under::Fern => {
                    // The scan is one; the rest are its frond grown.
                    if n > 0 {
                        break;
                    }
                    let (mesh, material) = self.scan("fern", Color::srgb(0.85, 0.9, 0.8));
                    Drawn::Scan(mesh, material)
                }
                Under::Frond => {
                    let tint = variant_tint(&mut dice, Vec3::new(0.78, 0.88, 0.7));
                    let def = terra_world::grown::RosetteDef {
                        card: "fern".into(),
                        tint: (1.0, 1.0, 1.0),
                        leaves: (8, 13),
                        tilt: (30.0, 65.0),
                        stalk: (0.03, 0.97),
                        tip: (0.97, 0.03),
                    };
                    let build = plants::rosette(&mut dice, &def, &card("fern"), tint);
                    Drawn::One(self.meshes.add(build.mesh(false)), self.atlas.clone())
                }
                Under::Herb => {
                    let tint = variant_tint(&mut dice, Vec3::new(0.8, 0.86, 0.72));
                    let def = terra_world::grown::RosetteDef {
                        card: "herb".into(),
                        tint: (1.0, 1.0, 1.0),
                        leaves: (4, 7),
                        tilt: (15.0, 45.0),
                        stalk: (0.5, 0.97),
                        tip: (0.5, 0.05),
                    };
                    let mut build = plants::rosette(&mut dice, &def, &card("herb"), tint);
                    // A herb is a third of a metre across, not a metre.
                    for p in &mut build.positions {
                        for c in p.iter_mut() {
                            *c *= 0.4;
                        }
                    }
                    Drawn::One(self.meshes.add(build.mesh(false)), self.atlas.clone())
                }
                Under::Hazel => {
                    let tint = variant_tint(&mut dice, Vec3::new(0.5, 0.6, 0.45));
                    let def = terra_world::grown::BushDef {
                        leaves: "hazel".into(),
                        tint: (1.0, 1.0, 1.0),
                        clumps: (9, 14),
                        spread: 1.15,
                    };
                    let build = plants::bush(&mut dice, &def, &card("hazel"), tint);
                    Drawn::One(self.meshes.add(build.mesh(false)), self.atlas.clone())
                }
                Under::Bilberry => {
                    let tint = variant_tint(&mut dice, Vec3::new(0.8, 0.85, 0.75));
                    let def = terra_world::grown::BushDef {
                        leaves: "berry".into(),
                        tint: (1.0, 1.0, 1.0),
                        clumps: (6, 9),
                        spread: 1.7,
                    };
                    let build = plants::bush(&mut dice, &def, &card("berry"), tint);
                    Drawn::One(self.meshes.add(build.mesh(false)), self.atlas.clone())
                }
                Under::Scrub => {
                    if n >= BUSH_KINDS {
                        break;
                    }
                    // Young oak scrub, darker than the oak spray's own lime.
                    let mut build = grow_bush(&mut dice);
                    let tint = variant_tint(&mut dice, Vec3::new(0.62, 0.7, 0.55));
                    tint_build(&mut build.colours, tint);
                    Drawn::One(self.meshes.add(build.mesh()), self.leaves.clone())
                }
                Under::Sapling => {
                    let tint = variant_tint(&mut dice, Vec3::new(0.85, 0.92, 0.8));
                    let (wood, crown) = sapling(&mut dice, cards, n, tint);
                    let wood = self.meshes.add(wood.mesh(true));
                    Drawn::Two([
                        (wood, self.bark.clone()),
                        (self.meshes.add(crown.mesh(false)), self.atlas.clone()),
                    ])
                }
                Under::Branches => {
                    let mut build = branches(&mut dice);
                    let mut mesh = std::mem::take(&mut build).mesh();
                    mesh.generate_tangents()
                        .expect("sticks have normals and texture places");
                    Drawn::One(self.meshes.add(mesh), self.bark.clone())
                }
                Under::Flowers => {
                    let tint = variant_tint(&mut dice, Vec3::new(0.95, 0.95, 0.9));
                    let name = ["daisy", "buttercup", "campion"][n as usize % 3];
                    let build = plants::tuft(&mut dice, 2 + (n % 2) as u32, &card(name), tint);
                    Drawn::One(self.meshes.add(build.mesh(false)), self.atlas.clone())
                }
            };
            out.push(drawn);
        }
        out
    }

    /// Spawns one plant of a kind.
    fn plant(&mut self, placement: &Placement, drawn: &Drawn, place: Transform) {
        let cut = match placement.kind {
            Under::Hazel | Under::Scrub | Under::Sapling => self.cut_as("Bush", place.scale.y),
            Under::Bilberry => self.cut_as("Berry bush", place.scale.y),
            _ => None,
        };
        let (seen, shadow) = (placement.seen_m, placement.shadow_m);
        // A small plant, uncut and casting no shadow, is one entity.
        if cut.is_none() && shadow <= 0.0 {
            let mut entity = self.commands.spawn((
                Name::new(placement.name),
                place,
                within(seen),
                NotShadowCaster,
            ));
            match drawn {
                Drawn::One(mesh, material) => {
                    entity.insert((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone())));
                }
                Drawn::Scan(mesh, material) => {
                    entity.insert((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone())));
                }
                Drawn::Two(parts) => {
                    entity.insert((
                        Mesh3d(parts[0].0.clone()),
                        MeshMaterial3d(parts[0].1.clone()),
                    ));
                    entity.with_child((
                        Mesh3d(parts[1].0.clone()),
                        MeshMaterial3d(parts[1].1.clone()),
                        within(seen),
                        NotShadowCaster,
                    ));
                }
            }
            return;
        }
        let mut entity =
            self.commands
                .spawn((Name::new(placement.name), place, Visibility::default()));
        if cut.is_some() {
            entity.insert(Growth::new(cut, None, 0.5));
        }
        entity.with_children(|plant| {
            let parts: Vec<(
                Handle<Mesh>,
                Option<Handle<Foliage>>,
                Option<Handle<StandardMaterial>>,
            )> = match drawn {
                Drawn::One(mesh, material) => vec![(mesh.clone(), Some(material.clone()), None)],
                Drawn::Scan(mesh, material) => vec![(mesh.clone(), None, Some(material.clone()))],
                Drawn::Two(parts) => parts
                    .iter()
                    .map(|(mesh, material)| (mesh.clone(), Some(material.clone()), None))
                    .collect(),
            };
            for (mesh, foliage, plain) in parts {
                let mut part = plant.spawn((Mesh3d(mesh.clone()), within(seen), NotShadowCaster));
                if let Some(material) = &foliage {
                    part.insert(MeshMaterial3d(material.clone()));
                }
                if let Some(material) = plain {
                    part.insert(MeshMaterial3d(material));
                }
                // Its shadow from a copy only the sun sees, which fades out nearer, so the
                // plant itself never dithers between two of itself.
                if shadow > 0.0
                    && let Some(material) = foliage
                {
                    plant.spawn((
                        Mesh3d(mesh),
                        MeshMaterial3d(material),
                        within(shadow),
                        RenderLayers::layer(SUN_ONLY),
                    ));
                }
            }
        });
    }
}

/// Lays the forest floor, by what the trees and paths leave: rocks and boulders, stumps,
/// fallen logs, and the undergrowth in drifts; and gives back the map the ground cover is
/// grown from.
pub(super) fn lay(lay: &mut Lay, path: Path, canopy: Crowns) -> Map {
    let mut map = Map {
        path,
        canopy,
        lying: Lying::default(),
        noise: Noise::new(lay.seed.derive("patches")),
    };
    let mut dice = lay.seed.derive("floor").rng();
    let disc = |dice: &mut Rng, from: f32, to: f32| {
        let angle = dice.range(0.0, TAU);
        let distance = libm::sqrtf(dice.range(from * from, to * to));
        Vec2::new(libm::cosf(angle), libm::sinf(angle)) * distance
    };

    // Rocks and boulders, the rocks scattered round the boulders and alone; sunk a fifth to
    // two fifths of their height once their meshes are loaded (`Sink`).
    let rocks: Vec<_> = (1..=7)
        .map(|n| lay.mossy_scan(&format!("mossy_rock_b_{n}"), 0.8))
        .collect();
    let mut boulders: Vec<_> = (1..=6)
        .map(|n| lay.mossy_scan(&format!("mossy_rock_a_{n}"), 1.0))
        .collect();
    boulders.push(lay.mossy_scan("boulder_1", 0.9));
    let mut laid = [0; 4];
    // The nearest of each to the start, for scripted runs to look at.
    let mut nearest = [Vec2::splat(f32::MAX); 4];
    let note = |nearest: &mut [Vec2; 4], what: usize, at: Vec2| {
        if at.length() < nearest[what].length() {
            nearest[what] = at;
        }
    };
    let mut rock = |lay: &mut Lay, map: &mut Map, dice: &mut Rng, at: Vec2, boulder: bool| {
        let here = map.here(at);
        if here.path < 1.2 || here.bark < 0.8 || here.lying < 0.2 {
            return;
        }
        let (scale, kinds, size) = if boulder {
            (dice.range(0.6, 1.3), &boulders, 1.2)
        } else {
            (dice.range(0.6, 1.6), &rocks, 0.35)
        };
        let place = lay.on(at, dice, 0.0, scale);
        let (mesh, material) = kinds[dice.below(kinds.len() as u32) as usize].clone();
        lay.commands.spawn((
            Name::new(if boulder { "Boulder" } else { "Rock" }),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            place,
            within(if boulder { 160.0 } else { 80.0 }),
            Footing,
            Sink(dice.range(0.2, 0.4)),
        ));
        map.lying.add(at, size * scale * 0.6);
        laid[boulder as usize] += 1;
        note(&mut nearest, boulder as usize, at);
    };
    for _ in 0..70 {
        let at = disc(&mut dice, CLEARING_M, FOREST_M);
        rock(lay, &mut map, &mut dice, at, true);
        // Stones broken from it round its foot.
        for _ in 0..dice.below(5) {
            let near = round(&mut dice, at, 2.2);
            rock(lay, &mut map, &mut dice, near, false);
        }
    }
    for _ in 0..800 {
        let at = disc(&mut dice, CLEARING_M, FOREST_M);
        // More in the shade, where the soil is thin over them.
        if dice.unit() > 0.4 + 0.6 * (1.0 - map.here(at).light) {
            continue;
        }
        rock(lay, &mut map, &mut dice, at, false);
    }

    // Fallen logs, under the trees most, each from the forest's own trees: tapering,
    // broken at the crown's end, its stubs of branches, settled into the ground.
    let end_grain = lay.end_grain();
    let logs: Vec<(Handle<Mesh>, Option<Handle<Mesh>>, f32, f32)> = (0..LOG_KINDS)
        .map(|kind| {
            let mut dice = lay.seed.derive("log").index(kind).rng();
            let log = fallen_log(&mut dice);
            let mut mesh = log.bark.mesh();
            mesh.generate_tangents()
                .expect("a log has normals and texture places");
            (
                lay.meshes.add(mesh),
                log.end.map(|end| lay.meshes.add(end.mesh())),
                log.length,
                log.radius,
            )
        })
        .collect();
    for _ in 0..180 {
        let at = disc(&mut dice, CLEARING_M, FOREST_M);
        let here = map.here(at);
        if dice.unit() > 0.25 + 0.75 * (1.0 - here.light) {
            continue;
        }
        let (bark_mesh, end_mesh, length, radius) =
            logs[dice.below(LOG_KINDS as u32) as usize].clone();
        let scale = dice.range(0.85, 1.15);
        let place = lay.on(at, &mut dice, 0.12 * scale, scale);
        let along = lay.on_map(place.rotation * Vec3::X);
        let half = length * scale * 0.5;
        // Clear of the paths, the trunks and what else lies there, all along it.
        let steps = (half / 0.8).ceil() as i32;
        let clear = (-steps..=steps).all(|k| {
            let p = at + along * (k as f32 / steps as f32) * half;
            let h = map.here(p);
            h.path > 1.0 && h.bark > 0.6 && h.lying > 0.3
        });
        if !clear {
            continue;
        }
        for k in -steps..=steps {
            map.lying.add(
                at + along * (k as f32 / steps as f32) * half,
                radius * scale,
            );
        }
        // Of the trees round it.
        let bark = if here.needles > 0.5 {
            lay.pine_bark.clone()
        } else {
            lay.bark.clone()
        };
        let mut log = lay.commands.spawn((
            Name::new("Log"),
            Mesh3d(bark_mesh),
            MeshMaterial3d(bark),
            place,
            within(110.0),
            Footing,
        ));
        if let Some(end) = end_mesh {
            log.with_child((
                Mesh3d(end),
                MeshMaterial3d(end_grain.clone()),
                within(110.0),
            ));
        }
        laid[2] += 1;
        note(&mut nearest, 2, at);
    }

    // Stumps of trees cut or broken long ago.
    let stumps: Vec<(Handle<Mesh>, Option<Handle<Mesh>>, f32)> = (0..STUMP_KINDS)
        .map(|kind| {
            let mut dice = lay.seed.derive("stump").index(kind).rng();
            let stump = stump(&mut dice, kind.is_multiple_of(2));
            let mut mesh = stump.bark.mesh();
            mesh.generate_tangents()
                .expect("a stump has normals and texture places");
            (
                lay.meshes.add(mesh),
                stump.end.map(|end| lay.meshes.add(end.mesh())),
                stump.radius,
            )
        })
        .collect();
    for _ in 0..110 {
        let at = disc(&mut dice, CLEARING_M, FOREST_M);
        let here = map.here(at);
        if here.path < 1.0
            || here.bark < 1.2
            || here.lying < 0.5
            || dice.unit() > 0.3 + 0.7 * (1.0 - here.light)
        {
            continue;
        }
        let (bark_mesh, end_mesh, radius) = stumps[dice.below(STUMP_KINDS as u32) as usize].clone();
        let scale = dice.range(0.8, 1.25);
        let place = lay.on(at, &mut dice, 0.0, scale);
        let bark = if here.needles > 0.5 {
            lay.pine_bark.clone()
        } else {
            lay.bark.clone()
        };
        let mut stump = lay.commands.spawn((
            Name::new("Stump"),
            Mesh3d(bark_mesh),
            MeshMaterial3d(bark),
            place,
            within(90.0),
            Footing,
        ));
        if let Some(end) = end_mesh {
            stump.with_child((Mesh3d(end), MeshMaterial3d(end_grain.clone()), within(90.0)));
        }
        // Its foot in duff and roots, as a tree's.
        map.canopy.add(Stand {
            at,
            trunk_m: radius * scale * 1.3,
            crown_m: 0.0,
            closes: 0.0,
            conifer: false,
        });
        laid[3] += 1;
        note(&mut nearest, 3, at);
    }
    info!(
        "lush: the floor has {} rocks, {} boulders, {} fallen logs and {} stumps; the nearest \
         of each at {:.1},{:.1}, {:.1},{:.1}, {:.1},{:.1} and {:.1},{:.1} (east, south).",
        laid[0],
        laid[1],
        laid[2],
        laid[3],
        nearest[0].x,
        nearest[0].y,
        nearest[1].x,
        nearest[1].y,
        nearest[2].x,
        nearest[2].y,
        nearest[3].x,
        nearest[3].y
    );

    // The undergrowth, kind by kind, in drifts.
    let mut counts = Vec::new();
    for placement in &PLACEMENTS {
        let variants = lay.grow(placement.kind);
        let drift = Noise::new(lay.seed.derive(placement.name).index(placement.kind as u64));
        let (bark_m, lying_m, path_m) = placement.kind.clear_of();
        let mut count = 0;
        for _ in 0..placement.tries {
            let at = disc(&mut dice, 3.0, FOREST_M + 10.0);
            let drifts = smoothstep(
                0.38,
                0.62,
                0.5 + 0.5 * drift.fbm(Vec3::new(at.x, 0.0, at.y) / placement.drift_m, 2, 2.0, 0.5),
            );
            let here = map.here(at);
            if dice.unit() > placement.kind.likes(&here) * drifts {
                continue;
            }
            let many = placement.clump.0 + dice.below(placement.clump.1 - placement.clump.0 + 1);
            for k in 0..many {
                let p = if k == 0 {
                    at
                } else {
                    round(&mut dice, at, placement.spread_m)
                };
                let here = map.here(p);
                if here.bark < bark_m
                    || here.lying < lying_m
                    || here.path < path_m
                    || placement.kind.likes(&here) < 0.1
                {
                    continue;
                }
                // Smaller toward the clump's edge.
                let edge = 1.0 - 0.3 * (p.distance(at) / (placement.spread_m * 2.0)).min(1.0);
                let scale = dice.range(placement.scale.0, placement.scale.1) * edge;
                let place = lay.on(p, &mut dice, 0.03 * scale, scale);
                let drawn = variants[dice.below(variants.len() as u32) as usize].clone();
                lay.plant(placement, &drawn, place);
                count += 1;
            }
        }
        counts.push(format!("{count} {:?}", placement.kind));
    }
    info!("lush: undergrowth: {}.", counts.join(", "));
    map
}

/// A rock's share of its height that is under the ground, applied once its mesh has
/// loaded and its height is known (`footings`).
#[derive(Component)]
pub(super) struct Sink(pub f32);

/// How many kinds of fallen log and of stump there are; each is drawn many times.
const LOG_KINDS: u64 = 6;
const STUMP_KINDS: u64 = 6;

/// A log or stump: its bark, and its cut end, of end grain.
struct Wood {
    bark: Build,
    end: Option<Build>,
    radius: f32,
}

/// A fallen log, lying along its x: its bark and cut end, how long and how thick at its
/// foot.
struct Log {
    bark: Build,
    end: Option<Build>,
    length: f32,
    radius: f32,
}

/// A ring of points round a line, `sides` round, its radius shaped by `shape` at each
/// angle; and the texture places round it, `wraps` repeats of the bark's picture.
#[allow(clippy::too_many_arguments)]
fn ring(
    build: &mut Build,
    middle: Vec3,
    forward: Vec3,
    side: Vec3,
    sides: u32,
    radius: impl Fn(f32) -> f32,
    v: f32,
    wraps: f32,
    sky: impl Fn(Vec3) -> f32,
    colour: Vec3,
) {
    let other = forward.cross(side);
    for j in 0..=sides {
        let angle = j as f32 / sides as f32 * TAU;
        let out = side * libm::cosf(angle) + other * libm::sinf(angle);
        let round = side * -libm::sinf(angle) + other * libm::cosf(angle);
        let r = radius(angle);
        let slope = (radius(angle + 0.01) - radius(angle - 0.01)) / 0.02;
        let normal = (out * r - round * slope).normalize_or(out);
        let index = build.vertex(
            middle + out * r,
            normal,
            Vec2::new(j as f32 / sides as f32 * wraps, v),
            Vec2::ZERO,
            colour,
        );
        build.colours[index as usize][3] = sky(normal);
    }
}

/// Joins rings of `sides + 1` points from `first`, `count` of them, facing out.
fn join_rings(build: &mut Build, first: u32, count: u32, sides: u32) {
    let ring = sides + 1;
    for i in 0..count - 1 {
        for j in 0..sides {
            let a = first + i * ring + j;
            let b = a + ring;
            build.indices.extend([a, a + 1, b, a + 1, b + 1, b]);
        }
    }
}

/// A broken end: from the last ring, `sides + 1` points from `last`, splinters of wood
/// jutting on along `forward`, each its own length, closing to a point, paler where the
/// wood shows.
fn splinters(
    build: &mut Build,
    dice: &mut Rng,
    last: u32,
    sides: u32,
    middle: Vec3,
    forward: Vec3,
    radius: f32,
    v: f32,
) {
    let tip_v = v + 0.3;
    let mut teeth = Vec::new();
    let lengths: Vec<f32> = (0..sides)
        .map(|_| {
            let u = dice.unit();
            radius * (0.15 + 1.6 * u * u)
        })
        .collect();
    for j in 0..=sides {
        let from = Vec3::from(build.positions[(last + j) as usize]);
        let out = (from - middle) - forward * (from - middle).dot(forward);
        let length = lengths[j as usize % sides as usize];
        let at = middle + forward * length + out * 0.45;
        let index = build.vertex(
            at,
            (out.normalize_or(forward) * 0.6 + forward * 0.4).normalize(),
            Vec2::new(build.uvs[(last + j) as usize][0], tip_v),
            Vec2::ZERO,
            Vec3::new(1.35, 1.2, 1.0),
        );
        build.colours[index as usize][3] = 0.6;
        teeth.push(index);
    }
    let centre = build.vertex(
        middle + forward * radius * 0.3,
        forward,
        Vec2::new(0.5, tip_v),
        Vec2::ZERO,
        Vec3::new(1.4, 1.25, 1.0),
    );
    build.colours[centre as usize][3] = 0.6;
    for j in 0..sides {
        let (a, b) = (last + j, last + j + 1);
        let (ta, tb) = (teeth[j as usize], teeth[j as usize + 1]);
        build.indices.extend([a, b, ta, b, tb, ta]);
        build.indices.extend([ta, tb, centre]);
    }
}

/// A flat cut end, in its own mesh of end grain: a disc of the section's picture over the
/// last ring, `sides + 1` points from `last`, facing `normal`.
fn cut_end(bark: &Build, last: u32, sides: u32, middle: Vec3, normal: Vec3, side: Vec3) -> Build {
    let mut end = Build::default();
    let other = normal.cross(side);
    let centre = end.vertex(middle, normal, Vec2::splat(0.5), Vec2::ZERO, Vec3::ONE);
    let radius = (0..sides)
        .map(|j| Vec3::from(bark.positions[(last + j) as usize]).distance(middle))
        .fold(0.0, f32::max)
        .max(0.01);
    for j in 0..=sides {
        let at = Vec3::from(bark.positions[(last + j) as usize]);
        let local = at - middle;
        let uv = Vec2::new(local.dot(side), local.dot(other)) / radius * 0.46 + 0.5;
        end.vertex(at, normal, uv, Vec2::ZERO, Vec3::ONE);
    }
    for j in 0..sides {
        end.indices.extend([centre, centre + 1 + j, centre + 2 + j]);
    }
    // Facing out along the normal, whichever way the ring ran.
    let (a, b, c) = (
        Vec3::from(end.positions[0]),
        Vec3::from(end.positions[1]),
        Vec3::from(end.positions[2]),
    );
    if (b - a).cross(c - a).dot(normal) < 0.0 {
        for tri in end.indices.chunks_mut(3) {
            tri.swap(1, 2);
        }
    }
    end
}

/// A log fallen from the forest's own trees: a trunk's length tapering toward its crown,
/// a little bent and sagging, lumpy, its foot sawn or broken and its crown's end broken
/// off in splinters, stubs of branches on it, sunk a little into the ground. The bark
/// above sees the sky and grows moss; below it is dark and wet.
fn fallen_log(dice: &mut Rng) -> Log {
    let length = dice.range(4.0, 10.0);
    let radius = dice.range(0.22, 0.45);
    let bend = dice.range(-0.35, 0.35);
    let sides = 14;
    let steps = (length / 0.45).ceil() as u32;
    let phase = dice.range(0.0, TAU);
    let mut bark = Build::default();
    let wraps = (TAU * radius / BARK_M.x).round().max(1.0);
    let point = |t: f32| {
        Vec3::new(
            (t - 0.5) * length,
            radius * (1.0 - 0.3 * t) * 0.85 - 0.04 * libm::sinf(t * PI),
            bend * libm::sinf(t * PI),
        )
    };
    let radius_at = |t: f32| radius * (1.0 - 0.45 * t);
    // The sky it sees: most on top.
    let sky = |normal: Vec3| 0.3 + 0.25 * normal.y.max(0.0);
    let mut side = Vec3::Y;
    for k in 0..=steps {
        let t = k as f32 / steps as f32;
        let forward = (point((t + 0.01).min(1.0)) - point((t - 0.01).max(0.0))).normalize();
        side = (side - forward * side.dot(forward)).normalize();
        let r = radius_at(t);
        let along = t * length;
        ring(
            &mut bark,
            point(t),
            forward,
            side,
            sides,
            |angle| {
                // Lumpy, burred here and there, and a little flattened where it lies.
                let burr = libm::powf(0.5 + 0.5 * libm::sinf(along * 2.3 + phase * 3.0), 6.0)
                    * libm::powf(0.5 + 0.5 * libm::cosf(angle * 2.0 + along), 2.0);
                r * (1.0
                    + 0.1
                        * libm::sinf(3.0 * angle + along * 1.3 + phase)
                        * libm::sinf(along * 0.7 + 2.0 * angle)
                    + 0.04 * libm::cosf(7.0 * angle + phase + along * 0.4)
                    + 0.12 * burr
                    - 0.08 * libm::powf(0.5 + 0.5 * libm::sinf(angle), 3.0))
            },
            along / BARK_M.y,
            wraps,
            sky,
            Vec3::ONE,
        );
    }
    join_rings(&mut bark, 0, steps + 1, sides);
    // The crown's end broken off.
    let last = steps * (sides + 1);
    let tip_forward = (point(1.0) - point(0.98)).normalize();
    splinters(
        &mut bark,
        dice,
        last,
        sides,
        point(1.0),
        tip_forward,
        radius_at(1.0),
        length / BARK_M.y,
    );
    // Stubs of branches, mostly on its upper side, broken short.
    for _ in 0..2 + dice.below(4) {
        let t = dice.range(0.3, 0.92);
        let around = dice.range(-1.3, 1.3) + if dice.unit() < 0.5 { 0.0 } else { PI * 0.5 };
        let out = (Vec3::Y * libm::cosf(around) + Vec3::Z * libm::sinf(around)).normalize();
        let r = radius_at(t) * dice.range(0.22, 0.38);
        let reach = dice.range(0.15, 0.55);
        let foot = point(t) + out * radius_at(t) * 0.6;
        let lean = Vec3::X * dice.range(0.1, 0.6);
        let stub = Limb {
            points: vec![
                foot,
                foot + (out + lean).normalize() * (radius_at(t) * 0.4 + reach * 0.5),
                foot + (out + lean).normalize() * (radius_at(t) * 0.4 + reach),
            ],
            radii: vec![r, r * 0.8, r * 0.45],
            depth: 1,
        };
        let first = bark.positions.len();
        tube(&mut bark, &stub, 6, 1.0e4, BARK_M);
        for colour in &mut bark.colours[first..] {
            colour[3] = 0.5;
        }
    }
    // Its foot sawn, or broken too.
    let foot_forward = (point(0.0) - point(0.02)).normalize();
    let end = if dice.unit() < 0.6 {
        let up = (Vec3::Y - foot_forward * foot_forward.y).normalize();
        Some(cut_end(&bark, 0, sides, point(0.0), foot_forward, up))
    } else {
        // Splintered back along the log from its first ring.
        splinters(
            &mut bark,
            dice,
            0,
            sides,
            point(0.0),
            foot_forward,
            radius,
            0.0,
        );
        None
    };
    // Broken ends face out; the foot's splinters were wound for the other way round.
    fix_winding(&mut bark);
    Log {
        bark,
        end,
        length,
        radius,
    }
}

/// Turns every triangle to face the way its corners' normals do, so pieces built in either
/// order are seen from outside.
fn fix_winding(build: &mut Build) {
    for tri in build.indices.chunks_mut(3) {
        let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| Vec3::from(build.positions[i as usize]));
        let normal = Vec3::from(build.normals[tri[0] as usize])
            + Vec3::from(build.normals[tri[1] as usize])
            + Vec3::from(build.normals[tri[2] as usize]);
        if (b - a).cross(c - a).dot(normal) < 0.0 {
            tri.swap(1, 2);
        }
    }
}

/// A stump: a trunk's foot with its roots spreading into the ground, sawn flat with its end
/// grain showing, or broken off in splinters; mossy where it sees the sky.
fn stump(dice: &mut Rng, sawn: bool) -> Wood {
    let radius = dice.range(0.3, 0.55);
    let height = if sawn {
        dice.range(0.3, 0.65)
    } else {
        dice.range(0.7, 1.3)
    };
    let sides = 18;
    let phase = dice.range(0.0, TAU);
    let roots = 4.0 + dice.below(3) as f32;
    let heights: Vec<f32> = [-0.3, -0.05, 0.06, 0.15, 0.27, 0.42, 0.6, 0.85, 1.1, 1.4]
        .into_iter()
        .filter(|y| *y < height - 0.05)
        .chain([height])
        .collect();
    let wraps = (TAU * radius / BARK_M.x).round().max(1.0);
    let tilt = Vec3::new(dice.range(-0.04, 0.04), 1.0, dice.range(-0.04, 0.04)).normalize();
    let mut bark = Build::default();
    for &y in &heights {
        let flare = 1.0 - smoothstep(-0.1, 0.55, y);
        ring(
            &mut bark,
            tilt * y,
            tilt,
            Vec3::X,
            sides,
            |angle| {
                let root = libm::powf(0.5 + 0.5 * libm::cosf(roots * angle + phase), 3.0);
                radius
                    * (1.0 + flare * flare * (1.1 * root + 0.12))
                    * (1.0 + 0.05 * libm::sinf(3.0 * angle + 2.0 * y + phase))
            },
            y / BARK_M.y,
            wraps,
            |normal| {
                (0.3 + 0.2 * normal.y.max(0.0) + 0.25 * smoothstep(0.0, 1.0, (y + 0.1) / height))
                    .min(1.0)
            },
            Vec3::ONE,
        );
    }
    join_rings(&mut bark, 0, heights.len() as u32, sides);
    let last = (heights.len() as u32 - 1) * (sides + 1);
    let top = tilt * height;
    let end = if sawn {
        // Sawn a little aslant.
        let normal =
            (tilt + Vec3::new(dice.range(-0.12, 0.12), 0.0, dice.range(-0.12, 0.12))).normalize();
        for j in 0..=sides {
            let p = Vec3::from(bark.positions[(last + j) as usize]);
            let lifted = p + tilt * (top - p).dot(normal) / tilt.dot(normal);
            bark.positions[(last + j) as usize] = lifted.into();
        }
        Some(cut_end(&bark, last, sides, top, normal, Vec3::X))
    } else {
        splinters(
            &mut bark,
            dice,
            last,
            sides,
            top,
            tilt,
            radius * 0.95,
            height / BARK_M.y,
        );
        None
    };
    fix_winding(&mut bark);
    Wood { bark, end, radius }
}

/// A few dead branches fallen together: crooked sticks lying across each other, forked
/// some of them, grey with weather.
fn branches(dice: &mut Rng) -> Build {
    let mut build = Build::default();
    let heap = dice.range(0.3, 0.8);
    for k in 0..5 + dice.below(7) {
        let angle = dice.range(0.0, TAU);
        let along = Vec3::new(libm::cosf(angle), 0.0, libm::sinf(angle));
        let across = along.cross(Vec3::Y);
        let length = dice.range(0.8, 2.6);
        let radius = dice.range(0.03, 0.08);
        let middle = Vec3::new(
            dice.range(-heap, heap),
            radius + k as f32 * 0.025,
            dice.range(-heap, heap),
        );
        let lift = dice.range(-0.05, 0.2);
        let points: Vec<Vec3> = (0..=4)
            .map(|i| {
                let t = i as f32 / 4.0;
                middle
                    + along * (t - 0.5) * length
                    + Vec3::Y * lift * t
                    + across * dice.range(-0.06, 0.06)
            })
            .collect();
        let radii: Vec<f32> = (0..=4)
            .map(|i| radius * (1.0 - 0.6 * i as f32 / 4.0))
            .collect();
        tube(
            &mut build,
            &Limb {
                points: points.clone(),
                radii,
                depth: 1,
            },
            5,
            1.0e4,
            BARK_M * 0.5,
        );
        if dice.unit() < 0.6 {
            let from = points[1 + dice.below(2) as usize];
            let way = (along * dice.range(0.4, 0.9) + across * dice.range(-0.8, 0.8)).normalize();
            let twig = dice.range(0.3, 0.8);
            tube(
                &mut build,
                &Limb {
                    points: vec![
                        from,
                        from + way * twig * 0.5 + Vec3::Y * 0.02,
                        from + way * twig,
                    ],
                    radii: vec![radius * 0.5, radius * 0.35, radius * 0.2],
                    depth: 1,
                },
                4,
                1.0e4,
                BARK_M * 0.5,
            );
        }
    }
    for colour in &mut build.colours {
        colour[0] *= 0.82;
        colour[1] *= 0.78;
        colour[2] *= 0.72;
        colour[3] = 0.45;
    }
    build
}

/// Bracken: a few fronds, each rising on its stalk and arching out nearly level, its tip
/// drooping, as two cards of the frond's picture, the lower steep, the upper laid out from
/// its end.
fn bracken(dice: &mut Rng, leaf: &terra_world::grown::Card, tint: Vec3) -> grown_build::Build {
    let mut build = grown_build::Build::default();
    let stalk = Vec2::new(0.03, 0.97);
    let tip = Vec2::new(0.97, 0.03);
    let axis = tip - stalk;
    let reach = axis.length();
    let count = 3 + dice.below(4);
    let turn = dice.range(0.0, TAU);
    for i in 0..count {
        let around = turn + i as f32 / count as f32 * TAU + dice.range(-0.4, 0.4);
        let out = Vec3::new(libm::cosf(around), 0.0, libm::sinf(around));
        let across = out.cross(Vec3::Y).normalize();
        let foot = out * dice.range(0.0, 0.2);
        let mut at = foot;
        let height = dice.range(0.75, 1.2);
        for (part, (tilt, length)) in [
            (dice.range(55.0f32, 75.0), height * 0.75),
            (dice.range(-20.0f32, 5.0), height * 0.85),
        ]
        .into_iter()
        .enumerate()
        {
            let tilt = tilt.to_radians();
            let run = (out * libm::cosf(tilt) + Vec3::Y * libm::sinf(tilt)).normalize();
            let facing = across.cross(run).normalize();
            let scale = length / reach;
            let (ax, ay) = (axis.x / reach, axis.y / reach);
            let right = (run * ax + across * ay) * scale;
            let down = (run * ay - across * ax) * scale;
            let shade = if part == 0 { 0.8 } else { 1.0 } * dice.range(0.9, 1.08);
            grown_build::card(
                &mut build,
                leaf,
                &Placing {
                    pin: stalk,
                    at,
                    right,
                    down,
                    normal: (facing * 0.4 + Vec3::Y * 0.6).normalize(),
                    sway: Vec2::new(0.4 + 0.3 * part as f32, 0.6),
                    colour: tint * shade,
                },
            );
            at += run * length * 0.9;
        }
    }
    build
}

/// A sapling a few metres tall: a slender stem nearly to its top, its branches climbing and
/// its leaves, oak or hazel, in a few small clumps: grown as the game's trees are, small.
fn sapling(
    dice: &mut Rng,
    cards: &Cards,
    n: u64,
    tint: Vec3,
) -> (grown_build::Build, grown_build::Build) {
    let leaves = if n.is_multiple_of(2) { "oak" } else { "hazel" };
    let def = TreeDef {
        habit: Habit::Upright,
        bark: "oak_bark".into(),
        bark_m: 1.0,
        bark_tint: (1.0, 1.0, 1.0),
        leaves: leaves.into(),
        tint: (tint.x, tint.y, tint.z),
        clump_m: 0.7,
        trunk: 0.014,
    };
    let height = dice.range(2.4, 4.5);
    let shape = grown_tree::grow(dice, &def, height);
    let canopy = grown_tree::Canopy::of(&shape, tint);
    let mut wood = grown_tree::wood(&shape, 0, def.bark_m);
    let placed = grown_tree::crown_cards(dice, &shape, &canopy);
    let mut crown = grown_tree::crown(&placed, &shape, cards.get(leaves), 0);
    for build in [&mut wood, &mut crown] {
        build.see_sky(|at| canopy.sky(at));
    }
    (wood, crown)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A map with these trees and the forest's own paths, and a place well off them.
    fn map_with(stands: &[Stand]) -> (Map, Vec2) {
        let mut canopy = Crowns::default();
        for stand in stands {
            canopy.add(*stand);
        }
        let map = Map {
            path: Path::new(Seed(3)),
            canopy,
            lying: Lying::default(),
            noise: Noise::new(Seed(4)),
        };
        let away = (0..400)
            .map(|k| Vec2::new((k % 20) as f32 * 9.0 - 90.0, (k / 20) as f32 * 9.0 - 90.0))
            .find(|at| map.path.edge_distance(*at) > 11.0)
            .expect("somewhere off the paths");
        (map, away)
    }

    /// Trees of a kind in a ring round `at`, closing the canopy over it.
    fn stand_round(at: Vec2, species: Species) -> Vec<Stand> {
        (0..6)
            .map(|k| {
                let angle = k as f32 / 6.0 * TAU;
                let foot = at + Vec2::new(libm::cosf(angle), libm::sinf(angle)) * 3.5;
                Stand::of(foot, species, false, false, 22.0, 0.5)
            })
            .collect()
    }

    #[test]
    fn grass_grows_thick_in_the_open_and_gives_out_under_a_closed_canopy() {
        let (open, at) = map_with(&[]);
        let growing = open.growing(at);
        assert!(growing.grass > 0.4, "{growing:?}");
        assert!(growing.shade < 0.05, "{growing:?}");
        let (shaded, _) = map_with(&stand_round(at, Species::Oak));
        let growing = shaded.growing(at);
        assert!(growing.grass < 0.35, "{growing:?}");
        assert!(growing.shade > 0.6, "{growing:?}");
        assert!(growing.litter > 0.5, "leaves under oaks: {growing:?}");
        assert!(growing.needles < 0.1, "{growing:?}");
    }

    #[test]
    fn needles_lie_under_conifers() {
        let (_, at) = map_with(&[]);
        let (map, _) = map_with(&stand_round(at, Species::Spruce));
        let growing = map.growing(at);
        assert!(growing.needles > 0.6, "{growing:?}");
        assert!(growing.litter < 0.4, "{growing:?}");
    }

    #[test]
    fn grass_is_pushed_back_from_a_trunk_by_duff_and_roots() {
        let (_, at) = map_with(&[]);
        let young = Stand::of(at, Species::Oak, true, false, 6.0, 0.4);
        let (map, _) = map_with(&[young]);
        let foot = map.growing(at + Vec2::X * 0.55);
        assert!(foot.grass < 0.1, "{foot:?}");
        assert!(foot.roots > 0.5, "{foot:?}");
    }
}
