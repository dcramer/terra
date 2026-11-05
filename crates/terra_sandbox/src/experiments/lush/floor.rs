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

mod builds;
mod laying;
mod placements;
mod reach;
mod under;
mod variants;
mod wood;

use super::shade::{self, NearShadows};
use super::{
    BARK_M, Build, CLEARING_M, FADE, FOREST_M, Foliage, FoliageExtension, Footing, Limb,
    smoothstep, tube,
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
use builds::*;
use placements::{PLACEMENTS, Placement};
use std::f32::consts::{PI, TAU};
use terra_gen::{Noise, Rng, Seed};
pub(super) use terra_world::cover::forest::{Crowns, Floor, Here, Stand, Ways, crown_of};
use terra_world::detail::DetailScale;
use terra_world::flora::Growth;
use terra_world::flora_kinds::{Cuttable, Flora};
use terra_world::grown::Cards;
use terra_world::grown::build::{self as grown_build, Placing};
use terra_world::planet::Planet;
use terra_world::sky_occlusion::SkyOccluders;
use terra_world::{climate::SnowCover, instance::Building::View, instance::InWorld};
use under::*;
pub(super) use variants::Variants;
use wood::*;
pub(super) use wood::{Sink, stump};

/// A rock's scanned material with moss over what faces the sky (`mossy.wesl`).
pub(super) type Mossy = ExtendedMaterial<StandardMaterial, MossExtension>;

/// What the moss shader is told. Field order must match the struct in mossy.wesl.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub(super) struct MossExtension {
    /// The moss's colour, linear, and in w how much grows.
    #[uniform(100)]
    colour: Vec4,
    /// The ground's layers, its moss among them (D294).
    #[texture(101, dimension = "2d_array")]
    #[sampler(102)]
    ground_color: Option<Handle<Image>>,
    #[texture(103, dimension = "2d_array")]
    ground_normal: Option<Handle<Image>>,
    /// How much of the ground round the camera snow covers (`climate::SnowCover`) in x: snow
    /// lies on a rock's tops as on the ground beside it (winter round 11). In y, 1 for a
    /// plant's scan: no moss or lichen, and it withers under the snow (winter round 13).
    #[uniform(104)]
    snow: Vec4,
}

/// The rocks' snow is told again only when the cover has moved by this much, so an eased
/// cover does not rewrite every rock's material each frame.
const SNOW_TOLD_BY: f32 = 0.02;

/// Every rock's material learns how much snow lies round the camera, when that has changed.
fn follow_snow(cover: Option<Res<SnowCover>>, mut mossy: ResMut<Assets<Mossy>>) {
    let Some(cover) = cover.filter(|cover| cover.is_changed()) else {
        return;
    };
    let stale: Vec<_> = mossy
        .iter()
        .filter(|(_, material)| {
            let told = material.extension.snow.x;
            (told - cover.0).abs() > SNOW_TOLD_BY || (cover.0 == 0.0 && told != 0.0)
        })
        .map(|(id, _)| id)
        .collect();
    for id in stale {
        if let Some(mut material) = mossy.get_mut(id) {
            material.extension.snow.x = cover.0;
        }
    }
}

impl MaterialExtension for MossExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/mossy.wesl".into()
    }
}

/// Rocks' and plants' glTF materials still to be copied into the mossy materials they are
/// drawn with, and the tint each scan's colour is taken times.
#[derive(Resource, Default)]
pub(super) struct MossyScans(Vec<(Handle<GltfMaterial>, Handle<Mossy>, Color)>);

/// The forest walk's own: its soil.
pub(super) fn plugin(app: &mut App) {
    // After the scene has set the open world's soil, in each world entered.
    app.add_systems(OnEnter(InWorld), forest_soil.in_set(View));
}

/// What every floor laid by these rules needs, the forest walk's and the woods' (D619): always
/// on, as the woods are.
pub(super) fn shared(app: &mut App) {
    app.add_plugins(MaterialPlugin::<Mossy>::default())
        .init_resource::<MossyScans>()
        .init_resource::<Variants>()
        .init_resource::<FloorShadows>()
        .add_systems(
            Update,
            (
                dress_mossy,
                follow_snow,
                follow_shadows,
                shade::follow_camera,
            ),
        );
    terra_world::remote::method(app, "lush.undergrowth", show_undergrowth);
    reach::plugin(app);
}

/// The forest's own soil: a dark loam, grey-brown, as a wood's trodden paths and bare
/// earth are, rather than the pale sandy brown of the open world's.
fn forest_soil(mut tweaks: ResMut<terra_world::planet::Tweaks>) {
    tweaks.ground_low = Color::srgb(0.24, 0.21, 0.18);
    tweaks.ground_high = Color::srgb(0.29, 0.26, 0.22);
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

/// Copies each rock's or plant's scanned glTF material, once loaded, under its moss and snow:
/// its colour times a tint, and its cut-out and sides as the scan has them.
fn dress_mossy(
    mut scans: ResMut<MossyScans>,
    gltf: Res<Assets<GltfMaterial>>,
    ground: Option<Res<terra_world::planet::GroundTextures>>,
    mut materials: ResMut<Assets<Mossy>>,
) {
    let Some(ground) = ground else {
        return;
    };
    scans.0.retain(|(from, to, tint)| {
        let (Some(from), Some(mut to)) = (gltf.get(from), materials.get_mut(to)) else {
            return true;
        };
        to.extension.ground_color = Some(ground.color().clone());
        to.extension.ground_normal = Some(ground.normal().clone());
        to.base = StandardMaterial {
            base_color: {
                let (a, b) = (from.base_color.to_linear(), tint.to_linear());
                Color::linear_rgba(a.red * b.red, a.green * b.green, a.blue * b.blue, a.alpha)
            },
            base_color_texture: from.base_color_texture.clone(),
            normal_map_texture: from.normal_map_texture.clone(),
            metallic_roughness_texture: from.metallic_roughness_texture.clone(),
            occlusion_texture: from.occlusion_texture.clone(),
            perceptual_roughness: from.perceptual_roughness,
            metallic: from.metallic,
            reflectance: 0.3,
            alpha_mode: from.alpha_mode,
            double_sided: from.double_sided,
            cull_mode: from.cull_mode,
            ..default()
        };
        false
    });
}

/// The floor runs on past the forest's edge this far, as meadow with scrub, bracken and
/// flowers at its edge, so the forest grades into the open rather than ending.
pub(super) const MEADOW_M: f32 = 70.0;
/// How far round its middle a shrub's foot spreads, at its scale: a stool's stems leave
/// their root a hand or two apart.
const FOOT_M: f32 = 0.25;
/// How many variants of each kind of undergrowth are grown, each its own tint.
const VARIANTS: u64 = 6;

/// A shrub's or young tree's foot is filed for its well when it is set at a place, at its
/// scale.
fn planted<W: Ways>(map: &mut Floor<W>, kind: Under, at: Vec2, scale: f32) {
    if kind.stands_in_a_well() {
        map.feet.add(at, FOOT_M * scale);
    }
}

/// Where a floor is laid on its map: rocks, logs and stumps from `from_m` to `to_m` round
/// `middle`, the undergrowth from `inner_m` to `outer_m`, each as thickly as the forest walk
/// lays its own (`YARD`), its tries in proportion to the ground; and whether only among
/// trees and off water, as round the woods' trees of a world with land (D619), where past
/// them is the land's own heath, lake or rock.
pub(super) struct Area {
    pub middle: Vec2,
    pub from_m: f32,
    pub to_m: f32,
    pub inner_m: f32,
    pub outer_m: f32,
    pub among_trees: bool,
}

impl Area {
    /// The forest walk's: its forest round the clearing, and its meadow's edge.
    pub const YARD: Area = Area {
        middle: Vec2::ZERO,
        from_m: CLEARING_M,
        to_m: FOREST_M,
        inner_m: 3.0,
        outer_m: FOREST_M + MEADOW_M * 0.6,
        among_trees: false,
    };

    /// So many of the forest walk's tries, for this area's ground: the same where it is
    /// the forest walk's.
    fn tries(&self, yard: u32, undergrowth: bool) -> u32 {
        let ring = |from: f32, to: f32| f64::from(to * to - from * from);
        let (here, there) = if undergrowth {
            (
                ring(self.inner_m, self.outer_m),
                ring(Self::YARD.inner_m, Self::YARD.outer_m),
            )
        } else {
            (
                ring(self.from_m, self.to_m),
                ring(Self::YARD.from_m, Self::YARD.to_m),
            )
        };
        (f64::from(yard) * here / there).round() as u32
    }

    /// A place for a rock, log or stump.
    fn place(&self, dice: &mut Rng) -> Vec2 {
        self.middle + disc(dice, self.from_m, self.to_m)
    }

    /// Whether a place is one the floor is laid at: among trees where it asks.
    fn holds(&self, here: &Here) -> bool {
        !self.among_trees || here.bark < terra_world::cover::forest::NO_BARK_M - 0.5
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
    pub mossy: &'a mut Assets<Mossy>,
    pub mossy_scans: &'a mut MossyScans,
    pub flora: &'a Flora,
    pub cards: &'a Cards,
    /// The undergrowth's variants, kept for the app.
    pub variants: &'a mut Variants,
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
    /// The crowns as the sun's light sees them and the way toward the sun as the floor is
    /// laid, where the world says where it is: what grows in its patches is put by them.
    pub sun: Option<(&'a SkyOccluders, Vec3)>,
}

/// The cut end of a log or a stump: the rings of a section of wood, weathered grey.
pub(super) fn end_grain(
    assets: &AssetServer,
    plain: &mut Assets<StandardMaterial>,
) -> Handle<StandardMaterial> {
    let map = |file: &str, colour: bool| -> Handle<Image> {
        assets
            .load_builder()
            .with_settings(move |settings: &mut ImageLoaderSettings| settings.is_srgb = colour)
            .load(format!("materials/log_end/{file}"))
    };
    let arm = map("arm.jpg", false);
    plain.add(StandardMaterial {
        // Weathered: cut wood greys and darkens in a season.
        base_color: Color::srgb(0.62, 0.56, 0.48),
        base_color_texture: Some(map("color.jpg", true)),
        normal_map_texture: Some(map("normal.jpg", false)),
        occlusion_texture: Some(arm.clone()),
        metallic_roughness_texture: Some(arm),
        perceptual_roughness: 1.0,
        reflectance: 0.25,
        ..default()
    })
}

/// A stump or log of the floor's wood, standing on its own as `place` says and cut as `cut`
/// says: its bark, and its end grain where it is sawn, its footing to be fitted to it (D267).
pub(super) fn laid_wood(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    name: &'static str,
    (bark, end): (Build, Option<Build>),
    place: Transform,
    (material, end_grain): (Handle<Foliage>, Handle<StandardMaterial>),
    cut: Option<Cuttable>,
) -> Entity {
    let mut mesh = bark.mesh();
    // Kept on the CPU too, for its footing to be fitted to (D267).
    mesh.asset_usage = bevy::asset::RenderAssetUsages::all();
    mesh.generate_tangents()
        .expect("a stump or log has normals and texture places");
    let end = end.map(|end| meshes.add(end.mesh()));
    let mut entity = commands.spawn((
        Name::new(name),
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(material),
        place,
        within(110.0),
        Footing,
        Growth::new(cut, None, 0.4),
    ));
    if let Some(end) = end {
        entity.with_child((Mesh3d(end), MeshMaterial3d(end_grain), within(110.0)));
    }
    entity.id()
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
/// The middle layer's leaves tinted two thirds as bright as the crowns' oak leaves are
/// (`CROWN_TINTS`, D359), their cards graded to living leaves' colour as the crowns' are
/// (D292). Tinted a third, as when the cards were pale (D268), a hazel's leaves were near
/// black-green fills a stride off; as bright as the crowns', in the trailer's low sun the
/// bushes were the brightest things in the frame bar the mist, lime pom-poms, against the
/// deep olive of Sons of the Forest's undergrowth (0.47 to 0.54 grey against 0.27). A
/// bramble's leaves a little darker, a holly's darker and bluer, as theirs are.
const THICKET_TINT: Vec3 = Vec3::new(0.6, 0.63, 0.56);
const BRAMBLE_TINT: Vec3 = Vec3::new(0.55, 0.58, 0.5);
const HOLLY_TINT: Vec3 = Vec3::new(0.45, 0.53, 0.47);

fn variant_tint(dice: &mut Rng, base: Vec3) -> Vec3 {
    let value = dice.range(0.82, 1.12);
    base * Vec3::new(
        value * dice.range(0.92, 1.1),
        value,
        value * dice.range(0.8, 1.05),
    )
}

/// The middle layer's shadows are cast in full to this share of their distance, then shrink
/// away (D234).
const SHADOW_SHRINK: f32 = 0.55;

/// How each variant of a kind is drawn: its parts, and the copies only the sun sees that
/// its shadow is cast from.
#[derive(Clone, Default)]
struct Drawn {
    parts: Vec<Part>,
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
    /// A plant's scan, under the season (`Lay::plant_scan`).
    Scan(Handle<Mossy>),
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
            shadow_set: None,
        }
    }
}

/// Lays the forest floor, by what the trees and paths leave: rocks and boulders, stumps,
/// fallen logs, and the undergrowth in drifts; and gives back the map the ground cover is
/// grown from.
pub(super) fn lay<W: Ways>(lay: &mut Lay, map: &mut Floor<W>, area: &Area) {
    let mut dice = lay.seed.derive("floor").rng();

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
    let mut rock = |lay: &mut Lay, map: &mut Floor<W>, dice: &mut Rng, at: Vec2, boulder: bool| {
        let here = map.here(at);
        if here.path < 1.2 || here.bark < 0.8 || here.lying < 0.2 || !area.holds(&here) {
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
        if lay.off(area, &place) {
            return;
        }
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
    for _ in 0..area.tries(70, false) {
        let at = area.place(&mut dice);
        rock(lay, map, &mut dice, at, true);
        // Stones broken from it round its foot.
        for _ in 0..dice.below(5) {
            let near = round(&mut dice, at, 2.2);
            rock(lay, map, &mut dice, near, false);
        }
    }
    for _ in 0..area.tries(1400, false) {
        let at = area.place(&mut dice);
        // More in the shade, where the soil is thin over them.
        if dice.unit() > 0.4 + 0.6 * (1.0 - map.here(at).light) {
            continue;
        }
        rock(lay, map, &mut dice, at, false);
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
    for _ in 0..area.tries(180, false) {
        let at = area.place(&mut dice);
        let here = map.here(at);
        if dice.unit() > 0.25 + 0.75 * (1.0 - here.light) || !area.holds(&here) {
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
        if !clear || lay.off(area, &place) {
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
    for _ in 0..area.tries(110, false) {
        let at = area.place(&mut dice);
        let here = map.here(at);
        if here.path < 1.0
            || here.bark < 1.2
            || here.lying < 0.5
            || !area.holds(&here)
            || dice.unit() > 0.3 + 0.7 * (1.0 - here.light)
        {
            continue;
        }
        let (bark_mesh, end_mesh, radius) = stumps[dice.below(STUMP_KINDS as u32) as usize].clone();
        let scale = dice.range(0.8, 1.25);
        let place = lay.on(at, &mut dice, 0.0, scale);
        if lay.off(area, &place) {
            continue;
        }
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
        map.canopy.add(Stand::stump(at, radius * scale * 1.3));
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
        let clear = placement.kind.clear_of();
        let mut count = 0;
        let mut closest = Vec2::splat(f32::MAX);
        for _ in 0..area.tries(placement.tries, true) {
            let Some(at) = placement.kind.tried_at(&map.ways, area, &mut dice) else {
                continue;
            };
            let drifts = smoothstep(
                0.38,
                0.62,
                0.5 + 0.5 * drift.fbm(Vec3::new(at.x, 0.0, at.y) / placement.drift_m, 2, 2.0, 0.5),
            );
            if !lay.starts(placement.kind, at, map.here(at), dice.unit(), drifts) {
                continue;
            }
            let many = placement.clump.0 + dice.below(placement.clump.1 - placement.clump.0 + 1);
            for k in 0..many {
                let p = if k == 0 {
                    at
                } else {
                    round(&mut dice, at, placement.spread_m)
                };
                let member = lay.member(placement.kind, p, map.here(p), clear);
                let Some(here) = member.filter(|here| area.holds(here)) else {
                    continue;
                };
                // Smaller toward the clump's edge.
                let edge = 1.0 - 0.3 * (p.distance(at) / (placement.spread_m * 2.0)).min(1.0);
                let scale = dice.range(placement.scale.0, placement.scale.1) * edge;
                let place = lay.on(p, &mut dice, 0.03 * scale, scale);
                let pick = dice.below(variants.len() as u32) as usize;
                let drawn = variants[pick].clone();
                // Rolled as before, so every other plant stays where it was, then left out
                // if it would stand in the path's line.
                if !placement.kind.keeps_path_line(here.path) || lay.off(area, &place) {
                    continue;
                }
                lay.plant(placement, &drawn, p, place);
                planted(map, placement.kind, p, scale);
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
    lay.hand_over_shadows();
}

/// A stump at `stump_at` and a fallen log at `log_at`, of the floor's own wood, as if the
/// clearing they lie in had been cut: cut as the flora's "Stump" and "Log" kinds say, the
/// first wood to be had (they stood for the old modelled stump and log by the door).
pub(super) fn by_the_door<W: Ways>(
    lay: &mut Lay,
    map: &mut Floor<W>,
    stump_at: Vec2,
    log_at: Vec2,
) {
    let mut dice = lay.seed.derive("by the door").rng();
    let end_grain = lay.end_grain();
    let stump = stump(&mut dice, true);
    let log = fallen_log(&mut dice);
    let wood = [
        ("Stump", stump_at, stump.bark, stump.end, stump.radius, 0.0),
        ("Log", log_at, log.bark, log.end, log.radius, 0.12),
    ];
    for (name, at, bark, end, radius, sink) in wood {
        let place = lay.on(at, &mut dice, sink, 1.0);
        let material = if map.here(at).needles > 0.5 {
            lay.pine_bark.clone()
        } else {
            lay.bark.clone()
        };
        let cut = lay.cut_as(name, 1.0);
        laid_wood(
            lay.commands,
            lay.meshes,
            name,
            (bark, end),
            place,
            (material, end_grain.clone()),
            cut,
        );
        if name == "Stump" {
            map.canopy.add(Stand::stump(at, radius * 1.3));
        } else {
            map.lying.add(at, radius);
        }
    }
}

impl Lay<'_, '_, '_> {
    /// Hands the floor's shadow stand-ins and near shadows to the world, once all is laid.
    fn hand_over_shadows(&mut self) {
        let shadows = self
            .shrinking
            .iter()
            .map(|((_, end), material)| (material.clone(), *end as f32 / 4.0))
            .collect();
        self.commands.insert_resource(FloorShadows(shadows));
        let shade = std::mem::replace(
            &mut self.shade,
            NearShadows::new(self.start, self.east, self.south),
        );
        self.commands.insert_resource(shade);
    }
}

#[cfg(test)]
#[path = "floor_tests.rs"]
mod tests;
