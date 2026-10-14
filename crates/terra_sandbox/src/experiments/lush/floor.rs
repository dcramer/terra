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

use super::shade::{self, NearShadows, ShadowSet};
use super::species::Species;
use super::thicket::{self, Stool};
use super::{
    BARK_M, Build, CLEARING_M, FADE, FOREST_M, Foliage, FoliageExtension, Footing, Limb, Path,
    Scanned, smoothstep, square, tube,
};
use bevy::{
    camera::visibility::VisibilityRange,
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
use terra_world::detail::DetailScale;
use terra_world::flora::Growth;
use terra_world::flora_kinds::{Cuttable, Flora};
use terra_world::grown::build::{self as grown_build, Placing};
use terra_world::grown::{Cards, plants};
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
        .init_resource::<FloorShadows>()
        // After the scene has set the open world's soil.
        .add_systems(PostStartup, forest_soil)
        .add_systems(Update, (dress_mossy, follow_shadows, shade::follow_camera));
    terra_world::remote::method(app, "lush.undergrowth", show_undergrowth);
}

/// The forest's own soil: a dark loam, grey-brown, as a wood's trodden paths and bare
/// earth are, rather than the pale sandy brown of the open world's.
fn forest_soil(mut tweaks: ResMut<terra_world::planet::Tweaks>) {
    tweaks.ground_low = Color::srgb(0.3, 0.25, 0.2);
    tweaks.ground_high = Color::srgb(0.36, 0.31, 0.25);
}

/// The materials the middle layer's shadows shrink away in, and where each has shrunk away
/// by, as authored.
#[derive(Resource, Default)]
pub(super) struct FloorShadows(Vec<(Handle<Foliage>, f32)>);

/// The middle layer's shadows shrink away at distances scaled as every other level of detail
/// is (D230).
fn follow_shadows(
    detail: Res<DetailScale>,
    shadows: Res<FloorShadows>,
    mut materials: ResMut<Assets<Foliage>>,
) {
    if !(detail.is_changed() || shadows.is_changed()) {
        return;
    }
    for (material, end) in &shadows.0 {
        if let Some(mut material) = materials.get_mut(material) {
            material.extension.shrink = Vec4::new(end * SHADOW_SHRINK, *end, 0.0, 0.0) * detail.0;
        }
    }
}

/// What the camera sees of the undergrowth, shrubs, saplings, snags, ferns and the rest: each
/// mesh of it the camera draws, not the copies only the sun sees, so hiding it leaves every
/// shadow where it was.
#[derive(Component)]
pub(super) struct Undergrowth;

#[derive(serde::Deserialize)]
struct Show {
    hidden: bool,
    within: Option<f32>,
}

/// `lush.undergrowth {hidden, within?}`: hides what the camera sees of the undergrowth
/// within so many metres of it, or shows it all again, so `tools/sandbox/vantage.ts` can
/// measure how much of a view it hides by comparing the two pictures.
fn show_undergrowth(
    In(params): In<Option<serde_json::Value>>,
    world: &mut World,
) -> bevy::remote::BrpResult {
    let Show { hidden, within } = bevy::remote::builtin_methods::parse_some(params)?;
    let eye = world
        .query_filtered::<&GlobalTransform, With<terra_world::planet::MainCamera>>()
        .iter(world)
        .next()
        .map(GlobalTransform::translation)
        .ok_or_else(|| terra_world::remote::refused("there is no camera"))?;
    let within = within.unwrap_or(f32::MAX);
    let parts: Vec<(Entity, bool)> = world
        .query_filtered::<(Entity, &GlobalTransform), With<Undergrowth>>()
        .iter(world)
        .map(|(entity, place)| (entity, hidden && place.translation().distance(eye) < within))
        .collect();
    let mut count = 0;
    for (entity, hide) in parts {
        count += hide as u32;
        world.entity_mut(entity).insert(if hide {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
    Ok(serde_json::json!({ "hidden": count }))
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

/// The floor runs on past the forest's edge this far, as meadow with scrub, bracken and
/// flowers at its edge, so the forest grades into the open rather than ending.
pub(super) const MEADOW_M: f32 = 70.0;
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
    crown_at: Vec2,
    crown_m: f32,
    closes: f32,
    conifer: bool,
}

impl Stand {
    /// A tree of a species standing at `at`, its crown's middle at `crown_at` reaching
    /// `crown_m` round it: an oak's crown hides most of the sky under it, a Scots pine's lets
    /// much of the light through, a spruce's little; a young tree's less.
    pub(super) fn of(
        at: Vec2,
        crown_at: Vec2,
        crown_m: f32,
        species: Species,
        young: bool,
        trunk_m: f32,
    ) -> Self {
        let closes = match species {
            Species::Oak => 0.85,
            Species::Pine => 0.62,
            Species::Spruce => 0.92,
        };
        Self {
            at,
            // The root flare spreads past the trunk.
            trunk_m: trunk_m * 1.3,
            crown_at,
            crown_m,
            closes: closes * if young { 0.7 } else { 1.0 },
            conifer: species != Species::Oak,
        }
    }
}

/// A crown's middle, level with the foot, and how far its leaves reach from it, from where
/// its clumps of leaves are and how big.
pub(super) fn crown_of(tips: &[(Vec3, f32)]) -> (Vec3, f32) {
    let count = tips.len().max(1) as f32;
    let middle = tips.iter().fold(Vec3::ZERO, |sum, (tip, _)| {
        sum + Vec3::new(tip.x, 0.0, tip.z)
    }) / count;
    let reach = tips
        .iter()
        .map(|(tip, size)| Vec2::new(tip.x - middle.x, tip.z - middle.z).length() + size * 0.5)
        .fold(0.5, f32::max);
    (middle, reach)
}

impl Stand {
    /// How much of the crown is over a place, 0 to 1: all of it under its middle, none past
    /// its edge.
    fn under(&self, at: Vec2) -> f32 {
        1.0 - smoothstep(
            0.4 * self.crown_m,
            1.0 * self.crown_m,
            self.crown_at.distance(at),
        )
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
        // Under a broadleaf crown its own leaves fall over the needles.
        let mut leaves_miss = 1.0f32;
        for di in -CROWN_REACH..=CROWN_REACH {
            for dj in -CROWN_REACH..=CROWN_REACH {
                for stand in self.filed.get(&(i + di, j + dj)).into_iter().flatten() {
                    bark = bark.min(stand.at.distance(at) - stand.trunk_m);
                    light *= 1.0 - stand.under(at) * stand.closes;
                    if stand.conifer {
                        // Needles fall a little past the crown.
                        let d = stand.crown_at.distance(at);
                        let fall = 1.0 - smoothstep(0.8 * stand.crown_m, 2.4 * stand.crown_m, d);
                        needles_miss *= 1.0 - fall * 0.95;
                    } else {
                        leaves_miss *= 1.0 - stand.under(at) * 0.9;
                    }
                }
            }
        }
        Over {
            shade: 1.0 - light,
            needles: (1.0 - needles_miss) * (0.25 + 0.75 * leaves_miss),
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
            + (self.noise(at, 71.0, 2.6) - 0.5) * 1.3
            + (self.noise(at, 73.0, 0.6) - 0.5) * 0.4
    }

    fn here(&self, at: Vec2) -> Here {
        let over = self.canopy.over(at);
        let path = self.path_edge(at);
        // Gaps in the canopy, where a crown is thin or a tree has fallen.
        let gap = smoothstep(0.58, 0.82, self.noise(at, 19.0, 10.0));
        let shade = over.shade * (1.0 - 0.45 * gap);
        // A path's corridor lets the light down to its edges.
        let corridor = 1.0 - smoothstep(0.5, 5.0, path.max(0.0));
        Here {
            light: (1.0 - shade + 0.45 * corridor).min(1.0),
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
        let off_path = smoothstep(0.05, 1.5, path);
        let fray = (1.0 - smoothstep(0.0, 1.5, path)) * smoothstep(-0.35, 0.1, path);
        let patch = 0.6
            + 0.4
                * self
                    .noise
                    .fbm(Vec3::new(at.x, 0.0, at.y) / 9.0, 2, 2.0, 0.5);
        // In the shade grass grows mostly where a fleck of light falls, in tufts, thinning out
        // gradually into the litter rather than ending.
        let tufts = smoothstep(0.45, 0.85, self.noise(at, 23.0, 2.6));
        let lit = smoothstep(0.22, 0.72, here.light);
        // Pushed back from the bark, the logs and the rocks.
        let wander = (self.noise(at, 53.0, 1.3) - 0.5) * 1.2;
        let foot = smoothstep(0.2, 0.85, here.bark + wander * 0.4);
        let beside = smoothstep(0.0, 0.55, here.lying + wander * 0.3);
        // How thick the light lets it grow, thinner and drier on the needles under pines.
        let light_grows = (1.0 - 0.55 * smoothstep(0.2, 0.7, here.needles))
            * (lit + (1.0 - lit) * (0.06 + 0.3 * tufts));
        // Thin grass grows in clumps, not as a stubble of lone blades spread evenly.
        let clumps = smoothstep(0.35, 0.65, self.noise(at, 89.0, 1.6));
        let clumped = clumps + (1.0 - clumps) * smoothstep(0.3, 0.75, light_grows);
        let grass = off_path * foot * beside * patch * light_grows * clumped;
        // Faint grass is none.
        let grass = grass * smoothstep(0.04, 0.2, grass);
        let floor = 1.0 - lit;
        let canopied = floor.max(0.75 * smoothstep(0.04, 0.3, 1.0 - here.light));
        let on_floor = smoothstep(-0.3, 0.3, path);
        // Needles carpet a conifer stand, between its trees as well as under them.
        let needles = smoothstep(0.1, 0.5, here.needles) * off_path.max(0.4);
        let mossy = self.noise(at, 31.0, 3.5);
        // Duff and roots round a tree's foot, its edge wandering; along logs and rocks.
        let ring = 1.0 - smoothstep(0.0, 1.5, here.bark + wander);
        let along = 1.0 - smoothstep(0.0, 0.7, here.lying + wander * 0.4);
        // Dark duff in the damp hollows under the canopy.
        let damp = floor * smoothstep(0.6, 0.85, here.wet + (mossy - 0.5) * 0.4) * 0.65;
        let roots = (ring * 0.95)
            .max(along * 0.7)
            .max(damp)
            .max(fray * (0.35 + 0.5 * self.noise(at, 79.0, 0.9)));
        // Moss round a foot only on some sides of it, in broken patches, never a halo; along
        // logs and rocks; on the floor only in the dampest hollows, in cushions.
        let broken = smoothstep(0.4, 0.7, self.noise(at, 37.0, 0.9));
        let moss = (ring * broken * (0.2 + 0.6 * mossy))
            .max(along * 0.5)
            .max(floor * smoothstep(0.6, 0.88, mossy * 0.7 + here.wet * 0.4) * 0.75)
            * off_path.max(0.3);
        Growing {
            grass,
            // Grass in the shade is a deeper green; in the open it dries in patches.
            lush: self.noise(at, 7.0, 20.0).max(floor * 0.8),
            path: path < 0.0,
            // Fallen leaves cover the floor wherever trees stand near, not only in their deep shade:
            // off the paths no forest floor is bare; on them a few drift into the hollows.
            litter: (canopied * (1.0 - needles)).max(fray * 0.7) * on_floor
                + (1.0 - on_floor) * 0.45 * smoothstep(0.45, 0.8, self.noise(at, 97.0, 1.7)),
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
    /// A young spruce's grey-brown bark.
    pub spruce_bark: Handle<Foliage>,
    /// The cards of the shared atlas.
    pub atlas: Handle<Foliage>,
    /// The middle layer's cards: the shared atlas, shaded deeper inside and lit through.
    pub thicket: Handle<Foliage>,
    pub seed: Seed,
    /// The foliage materials, and those the middle layer's shadows shrink away in, by the
    /// leaves they are cut to and where they have shrunk away by, in quarter metres.
    pub foliage: &'a mut Assets<Foliage>,
    pub shrinking: HashMap<(Option<AssetId<Foliage>>, u32), Handle<Foliage>>,
    /// The plants whose shadows are given them only near the camera.
    pub shade: NearShadows,
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

/// The kinds of undergrowth: the floor's, knee-high and below, and the middle layer's, one
/// to five metres (D268).
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
    Coppice,
    Holly,
    Spruce,
    Bramble,
    Snag,
    Twigs,
}

/// How each kind of undergrowth is placed: how many drift centres are tried, how many
/// plants a clump has and how far it spreads, how big they are, how far each is drawn,
/// where its near level hands over to its far one (none at 0), where its shadow has shrunk
/// away (none at 0), and how broad its drifts are.
struct Placement {
    kind: Under,
    name: &'static str,
    tries: u32,
    clump: (u32, u32),
    spread_m: f32,
    scale: (f32, f32),
    seen_m: f32,
    near_m: f32,
    shadow_m: f32,
    drift_m: f32,
}

/// The middle layer's shadows are cast in full to this share of their distance, then shrink
/// away (D234).
const SHADOW_SHRINK: f32 = 0.55;

const PLACEMENTS: [Placement; 16] = [
    Placement {
        kind: Under::Bracken,
        name: "Bracken",
        tries: 11000,
        clump: (5, 13),
        spread_m: 1.1,
        scale: (0.7, 1.5),
        seen_m: 55.0,
        near_m: 0.0,
        shadow_m: 0.0,
        drift_m: 16.0,
    },
    Placement {
        kind: Under::Fern,
        name: "Fern",
        tries: 6000,
        clump: (2, 5),
        spread_m: 0.8,
        scale: (0.6, 1.7),
        seen_m: 45.0,
        near_m: 0.0,
        shadow_m: 0.0,
        drift_m: 9.0,
    },
    Placement {
        kind: Under::Frond,
        name: "Fern",
        tries: 14000,
        clump: (5, 12),
        spread_m: 0.9,
        scale: (0.55, 1.6),
        seen_m: 45.0,
        near_m: 0.0,
        shadow_m: 0.0,
        drift_m: 9.0,
    },
    Placement {
        kind: Under::Herb,
        name: "Herb",
        tries: 12000,
        clump: (6, 14),
        spread_m: 0.7,
        scale: (0.6, 1.1),
        seen_m: 35.0,
        near_m: 0.0,
        shadow_m: 0.0,
        drift_m: 7.0,
    },
    Placement {
        kind: Under::Hazel,
        name: "Hazel",
        tries: 6000,
        clump: (1, 3),
        spread_m: 1.4,
        scale: (0.85, 1.2),
        seen_m: 85.0,
        near_m: 25.0,
        shadow_m: 24.0,
        drift_m: 20.0,
    },
    Placement {
        kind: Under::Bilberry,
        name: "Bilberry",
        tries: 7000,
        clump: (5, 11),
        spread_m: 0.8,
        scale: (0.45, 0.8),
        seen_m: 40.0,
        near_m: 0.0,
        shadow_m: 0.0,
        drift_m: 11.0,
    },
    Placement {
        kind: Under::Scrub,
        name: "Bush",
        tries: 3500,
        clump: (1, 3),
        spread_m: 1.0,
        scale: (0.85, 1.2),
        seen_m: 70.0,
        near_m: 22.0,
        shadow_m: 24.0,
        drift_m: 14.0,
    },
    Placement {
        kind: Under::Sapling,
        name: "Sapling",
        tries: 10000,
        clump: (1, 4),
        spread_m: 2.2,
        scale: (0.8, 1.2),
        seen_m: 85.0,
        near_m: 28.0,
        shadow_m: 32.0,
        drift_m: 24.0,
    },
    Placement {
        kind: Under::Branches,
        name: "Branches",
        tries: 6000,
        clump: (1, 2),
        spread_m: 1.2,
        scale: (0.5, 1.2),
        seen_m: 45.0,
        near_m: 0.0,
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
        near_m: 0.0,
        shadow_m: 0.0,
        drift_m: 7.0,
    },
    Placement {
        kind: Under::Coppice,
        name: "Hazel",
        tries: 18000,
        clump: (1, 4),
        spread_m: 2.6,
        scale: (0.8, 1.15),
        seen_m: 85.0,
        near_m: 28.0,
        shadow_m: 32.0,
        drift_m: 22.0,
    },
    Placement {
        kind: Under::Holly,
        name: "Holly",
        tries: 18000,
        clump: (1, 4),
        spread_m: 2.2,
        scale: (0.8, 1.2),
        seen_m: 85.0,
        near_m: 28.0,
        shadow_m: 32.0,
        drift_m: 26.0,
    },
    Placement {
        kind: Under::Spruce,
        name: "Young spruce",
        tries: 16000,
        clump: (2, 6),
        spread_m: 2.4,
        scale: (0.75, 1.2),
        seen_m: 90.0,
        near_m: 28.0,
        shadow_m: 32.0,
        drift_m: 20.0,
    },
    Placement {
        kind: Under::Bramble,
        name: "Bramble",
        tries: 8000,
        clump: (1, 4),
        spread_m: 1.8,
        scale: (0.8, 1.3),
        seen_m: 70.0,
        near_m: 25.0,
        shadow_m: 24.0,
        drift_m: 14.0,
    },
    Placement {
        kind: Under::Snag,
        name: "Snag",
        tries: 5000,
        clump: (1, 1),
        spread_m: 0.0,
        scale: (0.9, 1.1),
        seen_m: 110.0,
        near_m: 35.0,
        shadow_m: 45.0,
        drift_m: 30.0,
    },
    Placement {
        kind: Under::Twigs,
        name: "Twigs",
        tries: 9000,
        clump: (1, 3),
        spread_m: 1.6,
        scale: (0.8, 1.2),
        seen_m: 30.0,
        near_m: 0.0,
        shadow_m: 0.0,
        drift_m: 12.0,
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
        // The middle layer is thickest where light comes down the side of something open,
        // a clearing's or a path's edge or a gap, and thinner in the deep shade.
        let edge = near(here.path, 2.0, 9.0);
        match self {
            // Half shade: the edges of clearings, gaps, the paths' sides.
            Under::Bracken => between(0.05, 1.2, here.light) * (0.4 + 0.6 * broadleaf),
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
            Under::Sapling => {
                smoothstep(0.15, 0.6, here.light)
                    * smoothstep(2.0, 4.0, here.bark)
                    * (0.6 + 0.4 * edge)
            }
            // Fallen from the trees, round their feet and by the logs.
            Under::Branches => {
                (1.0 - smoothstep(0.4, 0.8, here.light))
                    * (0.4 + 0.6 * near(here.bark, 2.0, 6.0).max(near(here.lying, 1.0, 3.0)))
            }
            // In the sun, and along the paths most.
            Under::Flowers => {
                smoothstep(0.65, 0.95, here.light) * (0.4 + 0.6 * near(here.path, 0.5, 4.0))
            }
            // Hazel stools under oaks, thickest in half shade at the edges.
            Under::Coppice => {
                (0.45 + 0.55 * between(0.05, 1.1, here.light)) * broadleaf * (0.6 + 0.4 * edge)
            }
            // Holly takes the deep shade under the oaks that little else does.
            Under::Holly => (1.0 - smoothstep(0.55, 0.95, here.light)) * broadleaf,
            // Young spruces come up in the gaps under the conifers.
            Under::Spruce => {
                smoothstep(0.25, 0.6, here.needles) * (0.55 + 0.45 * between(0.05, 1.1, here.light))
            }
            // Bramble in the light: gaps, the paths' and the clearing's edges.
            Under::Bramble => smoothstep(0.3, 0.75, here.light) * (0.5 + 0.5 * edge),
            // Small trees that lost the race for light, among the big ones.
            Under::Snag => {
                (1.0 - smoothstep(0.5, 0.9, here.light)) * smoothstep(1.5, 3.5, here.bark)
            }
            // Twigs fallen from the canopy, thickest under it and round the trunks.
            Under::Twigs => {
                (1.0 - smoothstep(0.5, 0.95, here.light)) * (0.5 + 0.5 * near(here.bark, 1.0, 6.0))
            }
        }
    }

    /// How far it stays from bark, from what lies on the floor, and from a path's edge.
    fn clear_of(self) -> (f32, f32, f32) {
        match self {
            Under::Hazel | Under::Scrub => (1.0, 0.3, 0.7),
            Under::Sapling | Under::Holly | Under::Spruce => (2.0, 0.6, 1.4),
            Under::Coppice => (1.8, 0.6, 1.8),
            Under::Bramble => (1.0, 0.3, 1.2),
            Under::Snag => (1.6, 0.8, 2.5),
            Under::Branches => (0.5, 0.1, 0.4),
            Under::Twigs => (0.3, 0.05, 0.3),
            Under::Flowers => (0.6, 0.1, 0.05),
            _ => (0.35, 0.1, 0.2),
        }
    }
}

/// How each variant of a kind is drawn: its parts, and the copies only the sun sees that
/// its shadow is cast from.
#[derive(Clone, Default)]
struct Drawn {
    parts: Vec<Part>,
    shadows: Vec<(Handle<Mesh>, Handle<Foliage>)>,
    /// Its shadows as filed for the camera to bring near (`shade`).
    shadow_set: Option<usize>,
}

/// A mesh of a plant in its material, over a band of distance; solid if it stops a walker.
#[derive(Clone)]
struct Part {
    mesh: Handle<Mesh>,
    look: Look,
    band: Band,
    solid: bool,
}

#[derive(Clone)]
enum Look {
    Foliage(Handle<Foliage>),
    Plain(Handle<StandardMaterial>),
}

/// Which distances a part is drawn over: all its kind's, or only near or only past where
/// its near level hands over.
#[derive(Clone, Copy)]
enum Band {
    All,
    Near,
    Far,
}

impl Drawn {
    fn one(mesh: Handle<Mesh>, look: Look) -> Self {
        Self {
            parts: vec![Part {
                mesh,
                look,
                band: Band::All,
                solid: false,
            }],
            shadows: Vec::new(),
            shadow_set: None,
        }
    }
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

    /// The material a shadow stand-in is drawn in, cast in full to `SHADOW_SHRINK` of `end`
    /// and shrinking away by `end` (D234): the leaves' own picture, cut to their outline, or
    /// for wood plain and solid.
    fn shrinking_material(
        &mut self,
        leaves: Option<&Handle<Foliage>>,
        end: f32,
    ) -> Handle<Foliage> {
        let key = (leaves.map(Handle::id), (end * 4.0) as u32);
        if let Some(material) = self.shrinking.get(&key) {
            return material.clone();
        }
        let base = leaves
            .and_then(|handle| self.foliage.get(handle))
            .map(|material| material.base.clone())
            .unwrap_or_default();
        let wind = self
            .foliage
            .get(&self.atlas)
            .expect("the floor's cards are made before it is laid")
            .extension
            .clone();
        let material = self.foliage.add(Foliage {
            base,
            extension: FoliageExtension {
                shrink: Vec4::new(end * SHADOW_SHRINK, end, 0.0, 0.0),
                leaf: Vec4::ZERO,
                ..wind
            },
        });
        self.shrinking.insert(key, material.clone());
        material
    }

    /// A mesh's shadow stand-in, shrinking away by `end`.
    fn shadow_of(
        &mut self,
        mesh: &Mesh,
        leaves: Option<&Handle<Foliage>>,
        end: f32,
    ) -> (Handle<Mesh>, Handle<Foliage>) {
        let material = self.shrinking_material(leaves, end);
        (self.meshes.add(thicket::shrinking(mesh)), material)
    }

    /// A plant of the middle layer drawn: what stands first is what the plant's own entity
    /// draws, its leaves near or a snag's trunk, so each plant is as few entities as it can
    /// be; then its wood, its leaves farther off, and its shadow.
    fn layered(&mut self, shrub: thicket::Shrub, bark: Handle<Foliage>, shadow_m: f32) -> Drawn {
        let mut drawn = Drawn::default();
        let [wood_near, wood_far] = shrub.wood;
        let leafy = shrub.leaves.is_some();
        let wood = |lay: &mut Self, build: grown_build::Build, band: Band, solid: bool| {
            if build.indices.is_empty() {
                return None;
            }
            let mut mesh = build.mesh(true);
            ensure_tangents(&mut mesh);
            if solid {
                // Kept on the CPU too, for its footing to be fitted to (D267).
                mesh.asset_usage = bevy::asset::RenderAssetUsages::all();
            }
            Some(Part {
                mesh: lay.meshes.add(mesh),
                look: Look::Foliage(bark.clone()),
                band,
                solid,
            })
        };
        if let Some(trunk) = shrub.trunk {
            drawn.parts.extend(wood(self, trunk, Band::All, true));
        }
        if let Some([near, far]) = shrub.leaves {
            for (build, band) in [(near, Band::Near), (far, Band::Far)] {
                drawn.parts.push(Part {
                    mesh: self.meshes.add(build.mesh(false)),
                    look: Look::Foliage(self.thicket.clone()),
                    band,
                    solid: false,
                });
            }
        }
        // A leafy plant's stems are thin, and drawn at one level; a dead one's branches are
        // all there is of it, and have two.
        if leafy {
            drawn.parts.extend(wood(self, wood_near, Band::All, false));
        } else {
            drawn.parts.extend(wood(self, wood_near, Band::Near, false));
            drawn.parts.extend(wood(self, wood_far, Band::Far, false));
        }
        if let Some(shadow) = shrub.shadow {
            let atlas = self.atlas.clone();
            let cut_to = leafy.then_some(&atlas);
            drawn
                .shadows
                .push(self.shadow_of(&shadow.mesh(false), cut_to, shadow_m));
        }
        drawn
    }

    /// Each kind's variants.
    fn grow(&mut self, placement: &Placement) -> Vec<Drawn> {
        let kind = placement.kind;
        let mut dice = self
            .seed
            .derive("undergrowth kinds")
            .index(kind as u64)
            .rng();
        let cards = self.cards;
        let card = |name: &str| cards.get(name).clone();
        let shadow_m = placement.shadow_m;
        let mut out = Vec::new();
        for n in 0..VARIANTS {
            let atlas = Look::Foliage(self.atlas.clone());
            let drawn = match kind {
                Under::Bracken => {
                    let tint = variant_tint(&mut dice, Vec3::new(0.62, 0.64, 0.52));
                    let build = bracken(&mut dice, &card("fern"), tint);
                    Drawn::one(self.meshes.add(build.mesh(false)), atlas)
                }
                Under::Fern => {
                    // The scan is one; the rest are its frond grown.
                    if n > 0 {
                        break;
                    }
                    let (mesh, material) = self.scan("fern", Color::srgb(0.6, 0.64, 0.58));
                    Drawn::one(mesh, Look::Plain(material))
                }
                Under::Frond => {
                    let tint = variant_tint(&mut dice, Vec3::new(0.54, 0.6, 0.52));
                    // Lady fern's shuttlecocks, broad and arching, and male fern's, upright
                    // and narrow, as two kinds of each.
                    let upright = n % 2 == 1;
                    let def = terra_world::grown::RosetteDef {
                        card: "fern".into(),
                        tint: (1.0, 1.0, 1.0),
                        leaves: if upright { (6, 9) } else { (9, 15) },
                        tilt: if upright { (55.0, 78.0) } else { (25.0, 60.0) },
                        stalk: (0.03, 0.97),
                        tip: (0.97, 0.03),
                    };
                    let mut build = plants::rosette(&mut dice, &def, &card("fern"), tint);
                    // Some young and small, some old and broad.
                    let size = [1.0, 0.8, 1.25, 0.65, 1.1, 0.9][n as usize];
                    for p in &mut build.positions {
                        for c in p.iter_mut() {
                            *c *= size;
                        }
                    }
                    Drawn::one(self.meshes.add(build.mesh(false)), atlas)
                }
                Under::Herb => {
                    let tint = variant_tint(&mut dice, Vec3::new(0.6, 0.62, 0.54));
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
                    Drawn::one(self.meshes.add(build.mesh(false)), atlas)
                }
                Under::Hazel => {
                    // Young stools on the floor, a metre or two: rods, a vase, a fan, each
                    // with its stems showing, at the size their leaves are (D276).
                    let tint = variant_tint(&mut dice, Vec3::new(0.34, 0.37, 0.34));
                    let stool = [Stool::Rods, Stool::Vase, Stool::Fan][n as usize % 3];
                    let tallest = dice.range(1.3, 2.3);
                    let shrub = thicket::coppice(&mut dice, &card("hazel"), tint, stool, tallest);
                    self.layered(shrub, self.bark.clone(), shadow_m)
                }
                Under::Bilberry => {
                    let tint = variant_tint(&mut dice, Vec3::new(0.66, 0.7, 0.62));
                    let def = terra_world::grown::BushDef {
                        leaves: "berry".into(),
                        tint: (1.0, 1.0, 1.0),
                        clumps: (6, 9),
                        spread: 1.7,
                    };
                    let build = plants::bush(&mut dice, &def, &card("berry"), tint);
                    Drawn::one(self.meshes.add(build.mesh(false)), atlas)
                }
                Under::Scrub => {
                    // Young oak scrub, darker than the oak spray's own lime.
                    let tint = variant_tint(&mut dice, Vec3::new(0.4, 0.42, 0.37));
                    let shrub = thicket::scrub(&mut dice, &card("oak"), tint);
                    self.layered(shrub, self.bark.clone(), shadow_m)
                }
                Under::Sapling => {
                    let tint = variant_tint(&mut dice, Vec3::new(0.4, 0.42, 0.37));
                    let leaves = if n % 2 == 0 { "oak" } else { "hazel" };
                    let shrub = thicket::sapling(&mut dice, &card(leaves), tint);
                    self.layered(shrub, self.bark.clone(), shadow_m)
                }
                Under::Branches => {
                    let mut build = branches(&mut dice);
                    let mut mesh = std::mem::take(&mut build).mesh();
                    mesh.generate_tangents()
                        .expect("sticks have normals and texture places");
                    Drawn::one(self.meshes.add(mesh), Look::Foliage(self.bark.clone()))
                }
                Under::Twigs => {
                    let mut mesh = twigs(&mut dice).mesh();
                    mesh.generate_tangents()
                        .expect("twigs have normals and texture places");
                    Drawn::one(self.meshes.add(mesh), Look::Foliage(self.bark.clone()))
                }
                Under::Flowers => {
                    let tint = variant_tint(&mut dice, Vec3::new(0.9, 0.9, 0.86));
                    let name = ["daisy", "buttercup", "campion"][n as usize % 3];
                    let build = plants::tuft(&mut dice, 2 + (n % 2) as u32, &card(name), tint);
                    Drawn::one(self.meshes.add(build.mesh(false)), atlas)
                }
                Under::Coppice => {
                    let tint = variant_tint(&mut dice, Vec3::new(0.34, 0.37, 0.34));
                    let stool =
                        [Stool::Vase, Stool::Fan, Stool::Rods, Stool::Overstood][n as usize % 4];
                    let tallest = match stool {
                        Stool::Vase => dice.range(2.8, 4.6),
                        Stool::Fan => dice.range(2.6, 4.2),
                        Stool::Rods => dice.range(2.0, 3.2),
                        Stool::Overstood => dice.range(4.0, 5.4),
                    };
                    let shrub = thicket::coppice(&mut dice, &card("hazel"), tint, stool, tallest);
                    self.layered(shrub, self.bark.clone(), shadow_m)
                }
                Under::Holly => {
                    // Dark, glossy, nearly black-green in the shade.
                    let tint = variant_tint(&mut dice, Vec3::new(0.42, 0.5, 0.44));
                    let shrub = thicket::holly(&mut dice, &card("holly"), tint);
                    self.layered(shrub, self.bark.clone(), shadow_m)
                }
                Under::Spruce => {
                    let tint = variant_tint(&mut dice, Vec3::new(1.6, 1.75, 0.7));
                    let height = dice.range(1.4, 4.4);
                    let shrub = thicket::evergreen(&mut dice, &card("spruce"), tint, height, 0.5);
                    self.layered(shrub, self.spruce_bark.clone(), shadow_m)
                }
                Under::Bramble => {
                    let tint = variant_tint(&mut dice, Vec3::new(0.5, 0.54, 0.46));
                    let shrub = thicket::bramble(&mut dice, &card("bramble"), tint);
                    self.layered(shrub, self.bark.clone(), shadow_m)
                }
                Under::Snag => {
                    // Weathered silver-grey, the bark half gone.
                    let conifer = n % 2 == 0;
                    let grey = dice.range(0.82, 1.0);
                    let shrub =
                        thicket::snag(&mut dice, conifer, Vec3::new(1.0, 0.97, 0.92) * grey);
                    let bark = if conifer {
                        self.pine_bark.clone()
                    } else {
                        self.bark.clone()
                    };
                    self.layered(shrub, bark, shadow_m)
                }
            };
            let mut drawn = drawn;
            if !drawn.shadows.is_empty() {
                drawn.shadow_set = Some(self.shade.add_set(ShadowSet {
                    parts: std::mem::take(&mut drawn.shadows),
                    end_m: shadow_m,
                }));
            }
            out.push(drawn);
        }
        out
    }

    /// Spawns one plant of a kind, standing at `at` on the map: its first part drawn by its
    /// own entity, the rest by its children, and its shadow filed for when the camera is
    /// near (`shade`).
    fn plant(&mut self, placement: &Placement, drawn: &Drawn, at: Vec2, place: Transform) {
        let cut = match placement.kind {
            Under::Hazel | Under::Scrub | Under::Sapling | Under::Coppice | Under::Holly => {
                self.cut_as("Bush", place.scale.y)
            }
            Under::Bilberry => self.cut_as("Berry bush", place.scale.y),
            _ => None,
        };
        let (seen, near) = (placement.seen_m, placement.near_m);
        let band = |band: Band| match band {
            Band::All => within(seen),
            Band::Near => within(near),
            Band::Far => VisibilityRange {
                start_margin: near * (1.0 - FADE)..near,
                end_margin: seen * (1.0 - FADE)..seen,
                use_aabb: false,
            },
        };
        let drawn_as = |entity: &mut EntityCommands, part: &Part| {
            entity.insert((
                Mesh3d(part.mesh.clone()),
                band(part.band),
                NotShadowCaster,
                Undergrowth,
            ));
            match &part.look {
                Look::Foliage(material) => entity.insert(MeshMaterial3d(material.clone())),
                Look::Plain(material) => entity.insert(MeshMaterial3d(material.clone())),
            };
            if part.solid {
                entity.insert(Footing);
            }
        };
        let Some((first, rest)) = drawn.parts.split_first() else {
            return;
        };
        // It fades as a whole while it is between the camera and the player (D274).
        let mut entity = self.commands.spawn((
            Name::new(placement.name),
            place,
            terra_world::parting::ObstructionFade::default(),
        ));
        drawn_as(&mut entity, first);
        if cut.is_some() {
            entity.insert(Growth::new(cut, None, 0.5));
        }
        if !rest.is_empty() {
            entity.with_children(|plant| {
                for part in rest {
                    drawn_as(&mut plant.spawn_empty(), part);
                }
            });
        }
        // Its shadow from a copy only the sun sees, which shrinks away rather than ending,
        // as Bevy's shadow passes do not dither (D234), so the plant itself never dithers
        // between two of itself.
        if let Some(set) = drawn.shadow_set {
            let plant = entity.id();
            self.shade.add(plant, at, set);
        }
    }
}

/// Tangents for a mesh of bark, which its normal map needs.
fn ensure_tangents(mesh: &mut Mesh) {
    if !mesh.contains_attribute(Mesh::ATTRIBUTE_TANGENT)
        && let Err(problem) = mesh.generate_tangents()
    {
        warn!("No tangents for the floor's wood: {problem}");
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
            // Many small stones and a few big ones.
            let u = dice.unit();
            (0.4 + 1.8 * u * u, &rocks, 0.35)
        };
        // Most sit a fifth to two fifths in the ground; a third are half buried or more,
        // only their mossy backs showing, as hummocks of the floor.
        let sink = if dice.unit() < 0.35 {
            dice.range(0.45, 0.75)
        } else {
            dice.range(0.2, 0.4)
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
            Sink(sink),
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
    for _ in 0..1400 {
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
            // Kept on the CPU too, for its footing to be fitted to (D267).
            mesh.asset_usage = bevy::asset::RenderAssetUsages::all();
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
            // Kept on the CPU too, for its footing to be fitted to (D267).
            mesh.asset_usage = bevy::asset::RenderAssetUsages::all();
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
            crown_at: at,
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
        let variants = lay.grow(placement);
        let drift = Noise::new(lay.seed.derive(placement.name).index(placement.kind as u64));
        let (bark_m, lying_m, path_m) = placement.kind.clear_of();
        let mut count = 0;
        let mut closest = Vec2::splat(f32::MAX);
        for _ in 0..placement.tries {
            let at = disc(&mut dice, 3.0, FOREST_M + MEADOW_M * 0.6);
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
                lay.plant(placement, &drawn, p, place);
                count += 1;
                if p.length() < closest.length() {
                    closest = p;
                }
            }
        }
        counts.push(format!(
            "{count} {:?} (the nearest at {:.1},{:.1})",
            placement.kind, closest.x, closest.y
        ));
    }
    info!("lush: undergrowth: {}.", counts.join(", "));
    let shadows = lay
        .shrinking
        .iter()
        .map(|((_, end), material)| (material.clone(), *end as f32 / 4.0))
        .collect();
    lay.commands.insert_resource(FloorShadows(shadows));
    let shade = std::mem::replace(
        &mut lay.shade,
        NearShadows::new(lay.start, lay.east, lay.south),
    );
    lay.commands.insert_resource(shade);
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
            pointed: false,
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

/// Dead branches fallen from the canopy: one to three limbs, each lying along the floor sunk a
/// little into the litter and bending as it lies, thinning to its broken tip, forked into side
/// branches, some reaching down to the ground and propping the limb's far end up off it,
/// others sticking up and broken short; in damp brown bark, darker below.
fn branches(dice: &mut Rng) -> Build {
    let mut build = Build::default();
    for _ in 0..1 + dice.below(3) {
        let angle = dice.range(0.0, TAU);
        let along = Vec3::new(libm::cosf(angle), 0.0, libm::sinf(angle));
        let across = along.cross(Vec3::Y);
        let length = dice.range(1.2, 3.2);
        let radius = dice.range(0.035, 0.09);
        let start =
            Vec3::new(dice.range(-0.4, 0.4), 0.0, dice.range(-0.4, 0.4)) - along * length * 0.5;
        // Where the forks prop it, its thin end stands off the ground.
        let propped = if dice.unit() < 0.6 {
            dice.range(0.05, 0.3)
        } else {
            0.0
        };
        let bend = dice.range(-0.25, 0.25);
        let kink = dice.range(-0.08, 0.08);
        let radius_at = |t: f32| radius * (1.0 - 0.65 * t);
        let at = |t: f32| {
            start
                + along * t * length
                + across * (bend * libm::sinf(t * PI) + kink * libm::sinf(t * 3.0 * PI))
                // Sunk a third of its thickness into the litter where it lies on it.
                + Vec3::Y * (radius_at(t) * 0.65 + propped * t * t)
        };
        let points: Vec<Vec3> = (0..=6).map(|i| at(i as f32 / 6.0)).collect();
        let radii: Vec<f32> = (0..=6).map(|i| radius_at(i as f32 / 6.0)).collect();
        tube(
            &mut build,
            &Limb {
                points,
                radii,
                depth: 1,
                pointed: false,
            },
            7,
            1.0e4,
            BARK_M * 0.5,
        );
        for _ in 0..2 + dice.below(4) {
            let t = dice.range(0.25, 0.9);
            let from = at(t);
            let side = if dice.unit() < 0.5 { -1.0 } else { 1.0 };
            let turn = dice.range(0.45, 1.1);
            let way = along * libm::cosf(turn) + across * side * libm::sinf(turn);
            let fork = length * dice.range(0.15, 0.4);
            let r = radius_at(t) * dice.range(0.45, 0.65);
            // Down to the ground where it props the limb up, else up off it, broken short.
            let reaching = from.y > 0.12 && dice.unit() < 0.7;
            let end_y = if reaching {
                r * 0.6
            } else {
                from.y + fork * dice.range(0.05, 0.35)
            };
            let end = from + way * fork;
            let end = Vec3::new(end.x, end_y, end.z);
            let middle = from.lerp(end, 0.5)
                + Vec3::Y * dice.range(0.0, 0.06)
                + across * dice.range(-0.04, 0.04);
            tube(
                &mut build,
                &Limb {
                    points: vec![from, middle, end],
                    radii: vec![r, r * 0.75, r * 0.45],
                    depth: 1,
                    pointed: false,
                },
                5,
                1.0e4,
                BARK_M * 0.5,
            );
        }
    }
    // Brown, as damp dead bark is, rather than the living trunks' grey.
    for colour in &mut build.colours {
        colour[0] *= 0.78;
        colour[1] *= 0.64;
        colour[2] *= 0.5;
        colour[3] = 0.4;
    }
    build
}

/// Twig litter: a dozen to thirty thin dead twigs scattered over a couple of metres, lying
/// every way, some crooked, as the canopy drops them in every wind; brown with damp.
fn twigs(dice: &mut Rng) -> Build {
    let mut build = Build::default();
    for _ in 0..12 + dice.below(19) {
        let at = Vec3::new(dice.range(-1.1, 1.1), 0.0, dice.range(-1.1, 1.1));
        let angle = dice.range(0.0, TAU);
        let along = Vec3::new(libm::cosf(angle), 0.0, libm::sinf(angle));
        let across = along.cross(Vec3::Y);
        let length = dice.range(0.2, 0.9);
        let radius = dice.range(0.006, 0.02);
        let kink = dice.range(-0.06, 0.06);
        let points = (0..=2)
            .map(|i| {
                let t = i as f32 / 2.0;
                at + along * (t - 0.5) * length
                    + Vec3::Y * radius
                    + across * kink * libm::sinf(t * PI)
            })
            .collect();
        tube(
            &mut build,
            &Limb {
                points,
                radii: vec![radius, radius * 0.8, radius * 0.5],
                depth: 1,
                pointed: false,
            },
            3,
            1.0e4,
            BARK_M * 0.3,
        );
    }
    for colour in &mut build.colours {
        colour[0] *= 0.76;
        colour[1] *= 0.64;
        colour[2] *= 0.52;
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
                Stand::of(foot, foot, 7.0, species, false, 0.5)
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
        let young = Stand::of(at, at, 2.0, Species::Oak, true, 0.4);
        let (map, _) = map_with(&[young]);
        let foot = map.growing(at + Vec2::X * 0.55);
        assert!(foot.grass < 0.1, "{foot:?}");
        assert!(foot.roots > 0.5, "{foot:?}");
    }
}
