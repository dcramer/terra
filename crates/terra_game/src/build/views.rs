//! Building's debug views (D385), as every engine ships them (Unreal's view modes, Unity's
//! scene view draw modes): one key, F4, cycles what a building's surfaces are coloured by,
//! Shift+F4 back, and `debug.view <name>` over the remote sets one, so a look or a fault is
//! read at a glance rather than measured (research/building-workflow-retro-2026-10-16.md).
//! Each view puts a legend in the top right saying what it shows and how to read it:
//!
//! - **texels**: screen pixels a texel seen from a metre off, against [`AIM_PIXELS`], hatched
//!   where a texel is spread over more than a pixel where it is seen now.
//! - **roles**: each edge a cube has coloured by its role (`terra_build_layout::layout::roles`),
//!   each roof line by its part, faces tinted by what is beyond them; within [`ROLES_M`].
//! - **detail**: each mesh by the detail it draws (near, middle, far, hull, cell), each
//!   section's box by the detail it is laid for, and the parts an edit draws again.
//! - **damage**: each cube's health lost, and where the damage drawing breaks a surface.
//! - **shade**: how much light a surface keeps after occlusion and baked shade, and what
//!   takes the most.
//! - **normals**: the normal as the normal map makes it, and each texture's tangent frame.
//! - **plan**: what is built, a plan's ghosts and what goes in next, in three colours.
//! - **light**: the light alone, as Unreal's Lighting Only view: every surface an 18% grey
//!   lit as it is, its normal map and occlusion kept, so its light reads against a grey card
//!   stood beside it (`sandbox.card`; the material bench's sky check, D386).
//!
//! Off, nothing of it runs or is compiled in: the construction materials' shader takes the
//! view's code only while one is on (`arrays::ViewKey`), and every system here waits on it.
//! While one is on, the construction materials' arrays draw it themselves
//! (`shaders/build.wesl`); every other mesh of a building (a kit's modelled pieces, what is
//! drawn plain, the far detail, hulls) is drawn in a [`ViewMaterial`] made from its own
//! material (`shaders/build_view_piece.wesl`), the same colours from the same code
//! (`shaders/build_views.wesl`), and given its own back when the view changes.

use super::arrays::{Arrays, BuildMaterial};
use super::draw::GhostMaterial;
use super::store::BaseRoot;
use super::{Build, BuildLook, Built, Site};
use crate::planet::MainCamera;
use crate::remote::{method, refused};
use bevy::{
    asset::RenderAssetUsages,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
    render::render_resource::{AsBindGroup, PrimitiveTopology},
    shader::ShaderRef,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use terra_build::{CELL_M, Fill, Kind};
use terra_build_layout::layout::roles::{Beyond, Dir, Role, Roles};
use terra_build_layout::layout::roofs::RoofPart;
use terra_build_layout::layout::{self, Piece};
use terra_build_mesh::sections::{CLUSTER, Detail, Reach};

/// The screen pixels a texel should cover seen square on from a metre off: one, a texel a
/// pixel, as a 2,048-texel stone wall two metres a copy lays at 1080p (D362,
/// research/texture-detail-2026-10-15.md).
pub const AIM_PIXELS: f32 = 1.0;
/// How far from the camera the roles are worked out and drawn, in metres.
pub const ROLES_M: f32 = 40.0;
/// How far the camera moves before the roles round it are worked out again, in metres.
const ROLES_AGAIN_M: f32 = 8.0;

/// What a building's surfaces are coloured by, numbered as `shaders/build_views.wesl`
/// numbers them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Off,
    Texels,
    Roles,
    Detail,
    Damage,
    Shade,
    Normals,
    Plan,
    Light,
}

impl View {
    pub const ALL: [View; 9] = [
        View::Off,
        View::Texels,
        View::Roles,
        View::Detail,
        View::Damage,
        View::Shade,
        View::Normals,
        View::Plan,
        View::Light,
    ];

    /// Its name, as the remote's `debug.view` takes it.
    pub fn name(self) -> &'static str {
        match self {
            View::Off => "off",
            View::Texels => "texels",
            View::Roles => "roles",
            View::Detail => "detail",
            View::Damage => "damage",
            View::Shade => "shade",
            View::Normals => "normals",
            View::Plan => "plan",
            View::Light => "light",
        }
    }

    pub fn named(name: &str) -> Option<View> {
        View::ALL.into_iter().find(|view| view.name() == name)
    }

    /// The number the shaders know it by.
    fn number(self) -> f32 {
        View::ALL.iter().position(|v| *v == self).unwrap_or(0) as f32
    }

    /// The next view along, or back, round to off and on again.
    pub fn step(self, back: bool) -> View {
        let at = View::ALL.iter().position(|v| *v == self).unwrap_or(0);
        let n = View::ALL.len();
        View::ALL[if back { (at + n - 1) % n } else { (at + 1) % n }]
    }

    fn title(self) -> &'static str {
        match self {
            View::Off => "",
            View::Texels => "Texel density",
            View::Roles => "Roles",
            View::Detail => "Detail level, sections and parts",
            View::Damage => "Damage and health",
            View::Shade => "Occlusion and shade",
            View::Normals => "Normals and tangents",
            View::Plan => "Plan and what stands",
            View::Light => "Light alone",
        }
    }

    /// How to read it, in a sentence or two.
    fn reading(self) -> &'static str {
        match self {
            View::Off => "",
            View::Texels => {
                "Screen pixels a texel covers seen from a metre off, square on, at this \
                 resolution: a surface's texture density wherever the camera is. Green, a \
                 texel a pixel, the aim (D362); blue, too few texels, blurred up close; red, \
                 more than a metre off needs. Hatched: blurred where it is seen now, a texel \
                 spread over more than a pixel."
            }
            View::Roles => {
                "Each edge a cube has, by its role (layout::roles), and each roof line by its \
                 part; faces tinted by what is beyond them, untinted where open to the air. \
                 Within 40 m of the camera."
            }
            View::Detail => {
                "Each mesh by the detail it is drawing (D213, D278). Boxes: each section, or \
                 a house's group, in the colour of the detail it is laid for; thin white \
                 boxes, the parts of the base being built that an edit draws again."
            }
            View::Damage => {
                "Each cube's health lost (D305): grey whole, yellow a little, red all but \
                 gone; white where the damage drawing breaks the surface. Damaged cubes \
                 boxed in the same colours."
            }
            View::Shade => {
                "How much light a surface keeps after occlusion and baked shade, unlit: \
                 white all, black none. Where something takes a tenth or more, tinted by \
                 what takes the most."
            }
            View::Normals => {
                "The normal with its normal map, in the world's axes (half bright faces the \
                 other way). Ticks on a lattice of each texture, eight a copy: its tangent \
                 frame. Ticks that turn or flip across a face are a mapping fault."
            }
            View::Plan => "What is built, a plan's ghosts not yet in, and what goes in next.",
            View::Light => {
                "Every surface an 18% grey, lit as it is, its normal map and occlusion kept \
                 and its colours left out: the light it takes, to read against a grey card \
                 beside it (sandbox.card). In shade it is as blue as the card; in the sun, \
                 warm."
            }
        }
    }

    /// Its legend: each colour and what it means.
    fn key(self) -> Vec<([f32; 3], String)> {
        let named = |items: &[([f32; 3], &str)]| {
            items
                .iter()
                .map(|(colour, name)| (*colour, name.to_string()))
                .collect::<Vec<_>>()
        };
        match self {
            View::Off => Vec::new(),
            View::Texels => named(&[
                (TEXEL_RAMP[4], "4 pixels a texel or more at 1 m"),
                (TEXEL_RAMP[3], "2 pixels a texel at 1 m"),
                (TEXEL_RAMP[2], "a texel a pixel at 1 m: the aim"),
                (TEXEL_RAMP[1], "2 texels a pixel at 1 m"),
                (TEXEL_RAMP[0], "4 texels a pixel or more at 1 m"),
                ([0.22, 0.22, 0.22], "no texture: colours baked"),
            ]),
            View::Roles => {
                let mut key: Vec<([f32; 3], String)> = Role::ALL
                    .into_iter()
                    .map(|role| (role_colour(role), format!("{role:?}")))
                    .collect();
                key.extend(ROOF_PARTS.into_iter().filter_map(|part| {
                    roof_colour(part).map(|colour| (colour, format!("roof, dashed: {part:?}")))
                }));
                key.extend(FACES.into_iter().map(|(beyond, name)| {
                    (
                        face_colour(beyond).unwrap_or([0.42; 3]),
                        format!("face: {name}"),
                    )
                }));
                key
            }
            View::Detail => named(&[
                (level_colour(Level::Near), "near: every piece as made"),
                (level_colour(Level::Mid), "middle: a few metres off"),
                (level_colour(Level::Far), "far: textures baked"),
                (level_colour(Level::Hull), "hull: its cubes' faces"),
                (level_colour(Level::Cell), "cell: a base far off"),
                (level_colour(Level::Every), "every detail: plain, next"),
            ]),
            View::Damage => named(&[
                ([0.3, 0.3, 0.3], "whole"),
                ([0.8, 0.7, 0.05], "a little lost"),
                ([0.8, 0.3, 0.02], "half lost"),
                ([0.75, 0.04, 0.04], "all but gone"),
                ([0.85, 0.85, 0.85], "broken by the damage drawing"),
            ]),
            View::Shade => named(&[
                ([0.85, 0.85, 0.85], "keeps all its light"),
                ([0.21, 0.21, 0.21], "keeps a quarter"),
                ([0.81, 0.31, 0.27], "most taken by its texture's occlusion"),
                ([0.36, 0.77, 0.36], "most taken by the screen's (SSAO)"),
                ([0.41, 0.48, 0.85], "most taken by its corners' colour"),
            ]),
            View::Normals => {
                let axis = |n: [f32; 3]| n.map(|c| (c * 0.5 + 0.5) * 0.8);
                named(&[
                    (axis([1.0, 0.0, 0.0]), "faces east"),
                    (axis([0.0, 1.0, 0.0]), "faces up"),
                    (axis([0.0, 0.0, 1.0]), "faces south"),
                    (axis([0.0, -1.0, 0.0]), "faces down"),
                    ([0.85, 0.05, 0.05], "tick: tangent"),
                    ([0.05, 0.75, 0.05], "tick: bitangent"),
                    ([0.1, 0.25, 0.95], "tick: normal"),
                ])
            }
            View::Plan => named(&[
                (BUILT, "built: in"),
                (GHOST, "plan: not yet in"),
                (NEXT, "what goes in next"),
            ]),
            View::Light => named(&[
                ([0.18, 0.18, 0.18], "an 18% grey, as a grey card"),
                ([0.12, 0.16, 0.26], "in shade: lit by the blue sky"),
                ([0.3, 0.26, 0.2], "in the sun: its warm light"),
            ]),
        }
    }
}

/// Texel density's colours, four texels a pixel to four pixels a texel at a metre, as
/// `build_views.wesl`'s `TEXEL_RAMP`.
const TEXEL_RAMP: [[f32; 3]; 5] = [
    [0.75, 0.06, 0.04],
    [0.78, 0.42, 0.03],
    [0.10, 0.62, 0.10],
    [0.04, 0.50, 0.70],
    [0.10, 0.14, 0.80],
];
/// The plan view's colours: what is built, as `build_views.wesl`'s `BUILT`, a plan's ghosts
/// and what goes in next.
const BUILT: [f32; 3] = [0.8, 0.36, 0.06];
const GHOST: [f32; 3] = [0.75, 0.1, 0.8];
const NEXT: [f32; 3] = [0.85, 0.8, 0.1];

/// The view on.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildView(pub View);

fn on(view: Res<BuildView>) -> bool {
    view.0 != View::Off
}

fn viewing(which: View) -> impl Fn(Res<BuildView>) -> bool + Clone {
    move |view: Res<BuildView>| view.0 == which
}

/// The detail a mesh of a building draws, for the detail view: put on each as it is
/// spawned (`draw`, `hlod`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Near,
    Mid,
    Far,
    Hull,
    /// A cell of a base far off, merged (`hlod`).
    Cell,
    /// Drawn with every detail but the hull: what is drawn plain, the next piece lit.
    Every,
}

impl Level {
    pub fn of(detail: Detail) -> Level {
        match detail {
            Detail::Near => Level::Near,
            Detail::Mid => Level::Mid,
            Detail::Far => Level::Far,
            Detail::Proxy => Level::Hull,
        }
    }
}

fn level_colour(level: Level) -> [f32; 3] {
    match level {
        Level::Near => [0.1, 0.62, 0.1],
        Level::Mid => [0.8, 0.7, 0.05],
        Level::Far => [0.8, 0.36, 0.03],
        Level::Hull => [0.75, 0.06, 0.04],
        Level::Cell => [0.45, 0.1, 0.7],
        Level::Every => [0.4, 0.4, 0.4],
    }
}

/// A section's box in the colour of the detail it is laid for.
fn reach_colour(reach: Reach) -> [f32; 3] {
    match reach {
        Reach::Close | Reach::Near => level_colour(Level::Near),
        Reach::Mid => level_colour(Level::Mid),
        Reach::Far => level_colour(Level::Far),
        Reach::Hull => level_colour(Level::Hull),
    }
}

fn role_colour(role: Role) -> [f32; 3] {
    match role {
        Role::Arris => [0.85, 0.08, 0.08],
        Role::Jamb => [0.9, 0.45, 0.05],
        Role::Inside => [0.42, 0.22, 0.06],
        Role::Crest => [0.92, 0.85, 0.1],
        Role::Nose => [0.5, 0.9, 0.1],
        Role::Sill => [0.08, 0.6, 0.15],
        Role::Head => [0.05, 0.7, 0.62],
        Role::Soffit => [0.15, 0.6, 0.95],
        Role::Foot => [0.12, 0.2, 0.9],
        Role::Border => [0.5, 0.2, 0.9],
        Role::Meet => [0.9, 0.2, 0.8],
        Role::Post => [1.0, 0.6, 0.72],
    }
}

/// The roof's lines, as their parts; rafters and battens, which are no edge, are left out.
const ROOF_PARTS: [RoofPart; 8] = [
    RoofPart::Eave,
    RoofPart::Verge,
    RoofPart::Ridge,
    RoofPart::Hip,
    RoofPart::Valley,
    RoofPart::Flashing,
    RoofPart::Step,
    RoofPart::Gutter,
];

fn roof_colour(part: RoofPart) -> Option<[f32; 3]> {
    Some(match part {
        RoofPart::Eave => [0.95, 0.95, 0.95],
        RoofPart::Verge => [0.6, 0.62, 0.66],
        RoofPart::Ridge => [0.03, 0.03, 0.03],
        RoofPart::Hip => [0.5, 0.04, 0.12],
        RoofPart::Valley => [0.04, 0.06, 0.42],
        RoofPart::Flashing => [0.75, 0.6, 0.25],
        RoofPart::Step => [0.55, 0.42, 0.15],
        RoofPart::Gutter => [0.3, 0.45, 0.35],
        RoofPart::Rafter | RoofPart::Batten => return None,
    })
}

/// The faces tinted, by what is beyond them.
const FACES: [(Beyond, &str); 5] = [
    (Beyond::Air, "open air"),
    (Beyond::Hole, "an opening"),
    (Beyond::Ground, "the ground"),
    (Beyond::Other(Kind::PLAIN), "another kind"),
    (Beyond::Cut, "a cut cube of its kind"),
];

fn face_colour(beyond: Beyond) -> Option<[f32; 3]> {
    match beyond {
        Beyond::Same | Beyond::Air => None,
        Beyond::Hole => Some([0.1, 0.75, 0.85]),
        Beyond::Ground => Some([0.45, 0.28, 0.1]),
        Beyond::Other(_) => Some([0.6, 0.25, 0.85]),
        Beyond::Cut => Some([0.55, 0.6, 0.1]),
    }
}

fn colour([r, g, b]: [f32; 3]) -> Color {
    Color::linear_rgb(r, g, b)
}

/// What every mesh of a building not drawn by the construction materials' arrays is drawn
/// in while a view is on, and every mesh in the detail view: its own material with the
/// view's shader.
pub type ViewMaterial = ExtendedMaterial<StandardMaterial, PieceView>;

/// What `shaders/build_view_piece.wesl` is given. Field order must match the struct there.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct PieceView {
    /// x the view, y texels across a copy of its texture (0 none), z the screen pixels a
    /// texel should cover a metre off.
    #[uniform(100)]
    pub view: Vec4,
    /// The colour the view gives it whole: its detail's, or built, planned or next.
    #[uniform(100)]
    pub flat: Vec4,
}

impl MaterialExtension for PieceView {
    fn fragment_shader() -> ShaderRef {
        "shaders/build_view_piece.wesl".into()
    }
}

/// A mesh of a building drawn in a view's material, and the material it is given back.
#[derive(Component)]
enum Swapped {
    Standard(Handle<StandardMaterial>),
    Arrays(Handle<BuildMaterial>),
}

/// What a view drew over a building of its own, left alone by the swapping.
#[derive(Component)]
struct Overlay;

/// The view's materials made, one for each material and detail, and the ghosts' colours
/// as they were before the plan view changed them.
#[derive(Resource, Default)]
struct Swaps {
    made: BTreeMap<(AssetId<StandardMaterial>, Option<u8>), Handle<ViewMaterial>>,
    arrays: BTreeMap<u8, Handle<ViewMaterial>>,
    ghosts: Vec<(Handle<GhostMaterial>, LinearRgba, f32)>,
}

/// Lines drawn over a building: wide, and a little toward the eye, so an edge under a
/// dressing's profile still shows.
#[derive(Default, Reflect, GizmoConfigGroup)]
struct ViewGizmos;

/// A roof's lines, dashed, so they are told from a wall's edges whatever their colour.
#[derive(Default, Reflect, GizmoConfigGroup)]
struct RoofGizmos;

/// The legend on screen.
#[derive(Component)]
struct Legend;

/// The roles round the camera, worked out when a base changes or the camera moves on.
#[derive(Resource, Default)]
struct RolesSeen {
    /// Each base's revision, and where the camera was, when they were last worked out.
    revisions: Vec<(Entity, u64)>,
    eye: Option<Vec3>,
    lines: Vec<Line>,
}

/// A line of the roles view, in the world: its ends, its colour, and whether it is a roof's.
type Line = (Vec3, Vec3, [f32; 3], bool);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<BuildView>()
        .init_resource::<Swaps>()
        .init_resource::<RolesSeen>()
        .init_gizmo_group::<ViewGizmos>()
        .init_gizmo_group::<RoofGizmos>()
        .add_plugins(MaterialPlugin::<ViewMaterial>::default())
        .add_systems(Startup, gizmo_style)
        .add_systems(
            Update,
            (
                cycle,
                (restore, legend, ghosts).run_if(resource_changed::<BuildView>),
                keep_arrays.run_if(|view: Res<BuildView>| view.0 != View::Off || view.is_changed()),
                swap.run_if(on),
                roles.run_if(viewing(View::Roles)),
                sections.run_if(viewing(View::Detail)),
                damaged.run_if(viewing(View::Damage)),
            )
                .chain(),
        );
    method(app, "debug.view", remote_view);
}

fn gizmo_style(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<ViewGizmos>();
    config.line.width = 3.0;
    config.depth_bias = -0.015;
    let (config, _) = store.config_mut::<RoofGizmos>();
    config.line.width = 4.0;
    config.line.style = GizmoLineStyle::Dashed {
        gap_scale: 1.5,
        line_scale: 3.0,
    };
    config.depth_bias = -0.015;
}

/// F4 to the next view, Shift+F4 back.
fn cycle(keys: Res<ButtonInput<KeyCode>>, mut view: ResMut<BuildView>) {
    if !keys.just_pressed(KeyCode::F4) {
        return;
    }
    let back = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    view.0 = view.0.step(back);
    info!("Build view: {}", view.0.name());
}

#[derive(Deserialize)]
struct Named {
    name: String,
}

/// `debug.view {name}`: a view by its name, `off`, or `next` or `back` along them.
fn remote_view(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Named { name } = parse_some(params)?;
    let now = world.resource::<BuildView>().0;
    let view = match name.as_str() {
        "next" => now.step(false),
        "back" => now.step(true),
        other => View::named(other).ok_or_else(|| {
            let names: Vec<&str> = View::ALL.iter().map(|v| v.name()).collect();
            refused(format!(
                "no view called {other}: one of {}, next or back",
                names.join(", ")
            ))
        })?,
    };
    world.resource_mut::<BuildView>().0 = view;
    info!("Build view: {}", view.name());
    Ok(json!({
        "view": view.name(),
        "title": view.title(),
        "reading": view.reading(),
        "key": view.key().into_iter().map(|(_, name)| name).collect::<Vec<_>>(),
    }))
}

/// The construction materials' arrays draw the view on (`shaders/build.wesl`), kept so
/// when the arrays are made again, as a preset changes.
fn keep_arrays(
    view: Res<BuildView>,
    arrays: Option<Res<Arrays>>,
    mut materials: ResMut<Assets<BuildMaterial>>,
) {
    let Some(arrays) = arrays else {
        return;
    };
    // The detail view draws every mesh in its detail's colour instead (`swap`).
    let number = match view.0 {
        View::Detail => 0.0,
        other => other.number(),
    };
    let wanted = Vec4::new(number, arrays.colour_side() as f32, AIM_PIXELS, 0.0);
    if materials
        .get(&arrays.material)
        .is_some_and(|material| material.extension.view != wanted)
        && let Some(mut material) = materials.get_mut(&arrays.material)
    {
        material.extension.view = wanted;
    }
}

/// Whether an entity is a mesh of a building: under a base, as its sections' meshes, its
/// groups' and its cells' are.
fn in_building(entity: Entity, up: &Query<&ChildOf>, roots: &Query<(), With<BaseRoot>>) -> bool {
    let mut at = entity;
    for _ in 0..3 {
        let Ok(parent) = up.get(at) else {
            return false;
        };
        at = parent.parent();
        if roots.contains(at) {
            return true;
        }
    }
    false
}

/// Each mesh of a building drawn in its material's view, as it comes on screen.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn swap(
    mut commands: Commands,
    view: Res<BuildView>,
    look: Option<Res<BuildLook>>,
    standard: Query<(Entity, &MeshMaterial3d<StandardMaterial>, Option<&Level>), Without<Overlay>>,
    arrays: Query<(Entity, &MeshMaterial3d<BuildMaterial>, Option<&Level>)>,
    up: Query<&ChildOf>,
    roots: Query<(), With<BaseRoot>>,
    standards: Res<Assets<StandardMaterial>>,
    images: Res<Assets<Image>>,
    mut views: ResMut<Assets<ViewMaterial>>,
    mut swaps: ResMut<Swaps>,
) {
    let view = view.0;
    let next = look.as_ref().map(|look| look.looks.next.id());
    let level_of = |level: Option<&Level>| level.copied().unwrap_or(Level::Every);
    for (entity, material, level) in &standard {
        if !in_building(entity, &up, &roots) {
            continue;
        }
        let Some(base) = standards.get(&material.0) else {
            continue;
        };
        let level = level_of(level);
        let key = (
            material.0.id(),
            (view == View::Detail).then_some(level as u8),
        );
        let handle = swaps
            .made
            .entry(key)
            .or_insert_with(|| {
                let side = base.base_color_texture.as_ref().map_or(0.0, |texture| {
                    images
                        .get(texture)
                        .map_or(2048.0, |image| image.width() as f32)
                });
                let flat = match view {
                    View::Detail => level_colour(level),
                    _ if Some(material.0.id()) == next => NEXT,
                    _ => BUILT,
                };
                views.add(ViewMaterial {
                    base: base.clone(),
                    extension: PieceView {
                        view: Vec4::new(view.number(), side, AIM_PIXELS, 0.0),
                        flat: Vec4::new(flat[0], flat[1], flat[2], 1.0),
                    },
                })
            })
            .clone();
        commands
            .entity(entity)
            .try_remove::<MeshMaterial3d<StandardMaterial>>()
            .try_insert((
                MeshMaterial3d(handle),
                Swapped::Standard(material.0.clone()),
            ));
    }
    // The arrays draw every view but the detail view themselves.
    if view != View::Detail {
        return;
    }
    for (entity, material, level) in &arrays {
        if !in_building(entity, &up, &roots) {
            continue;
        }
        let level = level_of(level);
        let handle = swaps
            .arrays
            .entry(level as u8)
            .or_insert_with(|| {
                let flat = level_colour(level);
                views.add(ViewMaterial {
                    base: StandardMaterial::default(),
                    extension: PieceView {
                        view: Vec4::new(view.number(), 0.0, AIM_PIXELS, 0.0),
                        flat: Vec4::new(flat[0], flat[1], flat[2], 1.0),
                    },
                })
            })
            .clone();
        commands
            .entity(entity)
            .try_remove::<MeshMaterial3d<BuildMaterial>>()
            .try_insert((MeshMaterial3d(handle), Swapped::Arrays(material.0.clone())));
    }
}

/// As the view changes, every mesh given its own material back, the view's let go, and
/// what it drew over buildings taken away; the next view swaps them again.
fn restore(
    mut commands: Commands,
    swapped: Query<(Entity, &Swapped)>,
    overlays: Query<Entity, With<Overlay>>,
    mut swaps: ResMut<Swaps>,
    mut seen: ResMut<RolesSeen>,
) {
    for (entity, swapped) in &swapped {
        let mut entity = commands.entity(entity);
        entity
            .try_remove::<(MeshMaterial3d<ViewMaterial>, Swapped)>()
            .try_remove::<MeshMaterial3d<StandardMaterial>>();
        match swapped {
            Swapped::Standard(material) => entity.try_insert(MeshMaterial3d(material.clone())),
            Swapped::Arrays(material) => entity.try_insert(MeshMaterial3d(material.clone())),
        };
    }
    for entity in &overlays {
        commands.entity(entity).try_despawn();
    }
    swaps.made.clear();
    swaps.arrays.clear();
    *seen = RolesSeen::default();
}

/// The plan view's ghosts in its own colour, and back as they were after.
fn ghosts(
    view: Res<BuildView>,
    look: Option<Res<BuildLook>>,
    mut materials: ResMut<Assets<GhostMaterial>>,
    mut swaps: ResMut<Swaps>,
) {
    for (handle, colour, edge) in swaps.ghosts.drain(..) {
        if let Some(mut material) = materials.get_mut(&handle) {
            material.colour = colour;
            material.edge = edge;
        }
    }
    let (View::Plan, Some(look)) = (view.0, look) else {
        return;
    };
    for handle in look.looks.ghosts() {
        if let Some(mut material) = materials.get_mut(&handle) {
            swaps
                .ghosts
                .push((handle.clone(), material.colour, material.edge));
            material.colour = LinearRgba::new(GHOST[0], GHOST[1], GHOST[2], 0.22);
            material.edge = 0.8;
        }
    }
}

/// The legend in the top right: the view's name, how to read it, its colours, and the keys.
fn legend(mut commands: Commands, view: Res<BuildView>, shown: Query<Entity, With<Legend>>) {
    for entity in &shown {
        commands.entity(entity).despawn();
    }
    let view = view.0;
    if view == View::Off {
        return;
    }
    let panel = commands
        .spawn((
            Name::new("Build view legend"),
            Legend,
            Node {
                position_type: PositionType::Absolute,
                right: px(8),
                top: px(8),
                width: px(360),
                padding: UiRect::all(px(8)),
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            GlobalZIndex(i32::MAX - 2),
        ))
        .id();
    let text = |commands: &mut Commands, words: String, size: f32, tint: Color| {
        commands.spawn((
            Text::new(words),
            TextFont::from_font_size(size),
            TextColor(tint),
            ChildOf(panel),
        ));
    };
    text(
        &mut commands,
        format!("Building view: {}", view.title()),
        16.0,
        Color::WHITE,
    );
    text(
        &mut commands,
        view.reading().to_string(),
        12.0,
        Color::srgb(0.85, 0.85, 0.85),
    );
    let rows = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                row_gap: px(2),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    let key = view.key();
    // Two columns where the legend is long.
    let width = if key.len() > 8 {
        Val::Percent(50.0)
    } else {
        Val::Percent(100.0)
    };
    for (swatch, name) in key {
        let row = commands
            .spawn((
                Node {
                    width,
                    column_gap: px(6),
                    align_items: AlignItems::Center,
                    ..default()
                },
                ChildOf(rows),
            ))
            .id();
        commands.spawn((
            Node {
                width: px(12),
                height: px(12),
                ..default()
            },
            BackgroundColor(colour(swatch)),
            ChildOf(row),
        ));
        commands.spawn((
            Text::new(name),
            TextFont::from_font_size(12.0),
            TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ChildOf(row),
        ));
    }
    text(
        &mut commands,
        format!(
            "F4 next view, Shift+F4 back; remote: debug.view {}",
            view.name()
        ),
        11.0,
        Color::srgb(0.6, 0.6, 0.6),
    );
}

/// Every build, the player's and the rest.
fn builds<'a>(site: Option<&'a Site>, built: &'a Query<&Built>) -> impl Iterator<Item = &'a Build> {
    site.map(|site| &site.build)
        .into_iter()
        .chain(built.iter().map(|built| &built.0))
}

fn eye(camera: &Query<&GlobalTransform, With<MainCamera>>) -> Option<Vec3> {
    camera.iter().next().map(GlobalTransform::translation)
}

/// A point of a base's own space in the world.
fn world(build: &Build, local: Vec3) -> Vec3 {
    build.frame.transform_point(local)
}

/// The camera in a base's own space.
fn local(build: &Build, eye: Vec3) -> Vec3 {
    build.frame.compute_affine().inverse().transform_point3(eye)
}

/// A cube's edge with a role, in metres in its base's own space, stood a centimetre out of
/// both its faces so it is not lost in them.
fn edge_line(cube: IVec3, edge: layout::roles::Edge) -> (Vec3, Vec3) {
    let (from, to) = layout::roles::line(cube, edge);
    let metres = |t: [i32; 3]| Vec3::from_array(t.map(layout::metres));
    let out = (edge.side.step() + edge.other.step()).as_vec3() * 0.01;
    (metres(from) + out, metres(to) + out)
}

/// A face of a cube, its four corners in metres in its base's own space, stood a few
/// millimetres out of it.
fn face_quad(cube: IVec3, toward: Dir) -> [Vec3; 4] {
    let axis = toward.axis();
    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
    let low = cube.as_vec3() * CELL_M;
    let mut at = low;
    at[axis] += if toward.sign() > 0 { CELL_M } else { 0.0 } + toward.sign() as f32 * 0.006;
    let corner = |a: f32, b: f32| {
        let mut p = at;
        p[u] += a * CELL_M;
        p[v] += b * CELL_M;
        p
    };
    [
        corner(0.0, 0.0),
        corner(1.0, 0.0),
        corner(1.0, 1.0),
        corner(0.0, 1.0),
    ]
}

/// The roles round the camera: worked out again when a base changes or the camera moves
/// on, their edges drawn every frame, their faces a mesh of their own over each base.
#[allow(clippy::too_many_arguments)]
fn roles(
    mut commands: Commands,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    site: Option<Res<Site>>,
    built: Query<&Built>,
    overlays: Query<Entity, With<Overlay>>,
    mut seen: ResMut<RolesSeen>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut gizmos: Gizmos<ViewGizmos>,
    mut roofs: Gizmos<RoofGizmos>,
) {
    let Some(eye) = eye(&camera) else {
        return;
    };
    let revisions: Vec<(Entity, u64)> = builds(site.as_deref(), &built)
        .map(|build| (build.root, build.revision))
        .collect();
    let moved = seen
        .eye
        .is_none_or(|last| last.distance(eye) > ROLES_AGAIN_M);
    if moved || revisions != seen.revisions {
        for entity in &overlays {
            commands.entity(entity).try_despawn();
        }
        let mut lines = Vec::new();
        let tint = materials.add(StandardMaterial {
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            depth_bias: 50.0,
            ..default()
        });
        for build in builds(site.as_deref(), &built) {
            if build.frame.translation.distance(eye) > ROLES_M + 200.0 {
                continue;
            }
            let (faces, edges) = roles_near(build, local(build, eye));
            lines.extend(
                edges
                    .into_iter()
                    .map(|(a, b, c, roof)| (world(build, a), world(build, b), c, roof)),
            );
            if let Some(mesh) = faces {
                commands.spawn((
                    Name::new("Build view: faces by role"),
                    Overlay,
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(tint.clone()),
                    bevy::light::NotShadowCaster,
                    Transform::IDENTITY,
                    ChildOf(build.root),
                ));
            }
        }
        *seen = RolesSeen {
            revisions,
            eye: Some(eye),
            lines,
        };
    }
    for &(a, b, c, roof) in &seen.lines {
        if roof {
            roofs.line(a, b, colour(c));
        } else {
            gizmos.line(a, b, colour(c));
        }
    }
}

/// A base's roles within [`ROLES_M`] of a point of its own space: a mesh of its faces
/// tinted by what is beyond them, and its edges and roof lines, each with its colour, in
/// its own space.
fn roles_near(build: &Build, eye: Vec3) -> (Option<Mesh>, Vec<Line>) {
    let base = &build.base;
    let mut lines = Vec::new();
    let mut quads: Vec<([Vec3; 4], [f32; 3])> = Vec::new();
    let Some(kits) = build.kits.as_deref() else {
        return (None, lines);
    };
    let near = |cube: IVec3| (cube.as_vec3() + 0.5) * CELL_M;
    let mut by_kind: BTreeMap<Kind, Vec<IVec3>> = BTreeMap::new();
    for (at, cube) in base.cubes() {
        if near(at).distance(eye) <= ROLES_M {
            by_kind.entry(cube.kind).or_default().push(at);
        }
    }
    for (kind, cubes) in &by_kind {
        let Some(kit) = kits.of(*kind) else {
            continue;
        };
        if kit.pattern == terra_build::kit::Pattern::Roof {
            continue;
        }
        let roles = Roles::new(base, *kind, kits);
        for &at in cubes {
            for edge in roles.edges(at) {
                let (a, b) = edge_line(at, edge);
                lines.push((a, b, role_colour(edge.role), false));
            }
            let solid = base
                .cube(at)
                .is_some_and(|cube| layout::drawn_fill(&cube) == Fill::Solid);
            if !solid {
                continue;
            }
            for toward in Dir::ALL {
                if let Some(tint) = face_colour(roles.beyond(at + toward.step())) {
                    quads.push((face_quad(at, toward), tint));
                }
            }
        }
    }
    // The roofs' lines, from their kits' pieces in the sections near.
    let sections: Vec<IVec3> = terra_build_mesh::sections::drawable(base)
        .into_iter()
        .map(IVec3::from_array)
        .filter(|at| {
            let side = terra_build::cubes::SECTION as f32 * CELL_M;
            let middle = (at.as_vec3() + 0.5) * side;
            middle.distance(eye) <= ROLES_M + side
        })
        .collect();
    for kit in kits.0.iter().filter(|kit| {
        kit.pattern == terra_build::kit::Pattern::Roof && by_kind.contains_key(&kit.kind())
    }) {
        for &section in &sections {
            for piece in layout::lay_section(base, kit, kits, section).pieces {
                if let Piece::Roof { part, from, to, .. } = piece
                    && let Some(tint) = roof_colour(part)
                {
                    let lift = Vec3::Y * 0.02;
                    lines.push((from.metres() + lift, to.metres() + lift, tint, true));
                }
            }
        }
    }
    if quads.is_empty() {
        return (None, lines);
    }
    let mut positions = Vec::with_capacity(quads.len() * 4);
    let mut colours = Vec::with_capacity(quads.len() * 4);
    let mut indices = Vec::with_capacity(quads.len() * 6);
    for (corners, tint) in quads {
        let first = positions.len() as u32;
        positions.extend(corners.map(|p| p.to_array()));
        colours.extend([[tint[0], tint[1], tint[2], 0.55]; 4]);
        indices.extend([0, 1, 2, 0, 2, 3].map(|i| first + i));
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
    .with_inserted_indices(bevy::mesh::Indices::U32(indices));
    (Some(mesh), lines)
}

/// Each section's box, or a group's, in the colour of the detail it is laid for; and for
/// the base being built, the parts an edit draws again.
fn sections(
    camera: Query<&GlobalTransform, With<MainCamera>>,
    site: Option<Res<Site>>,
    built: Query<&Built>,
    mut gizmos: Gizmos<ViewGizmos>,
) {
    let Some(eye) = eye(&camera) else {
        return;
    };
    let side = terra_build::cubes::SECTION as f32 * CELL_M;
    for build in builds(site.as_deref(), &built) {
        if build.frame.translation.distance(eye) > 1000.0 {
            continue;
        }
        let unit = build.sections.unit.max(1);
        for (at, drawn) in &build.sections.drawn {
            let low = IVec3::from_array(*at).as_vec3() * side;
            let (low, high) = drawn
                .bounds
                .unwrap_or((low, low + Vec3::splat(side * unit as f32)));
            let tint = colour(reach_colour(drawn.reach));
            cuboid(&mut gizmos, build, low, high, 0.03, tint);
            if unit > 1 || drawn.by_detail {
                continue;
            }
            let part = CLUSTER as f32 * CELL_M;
            let per = terra_build::cubes::SECTION / CLUSTER;
            for (i, shown) in drawn.parts.iter().enumerate() {
                if shown.entities.is_empty() {
                    continue;
                }
                let i = i as i32;
                let at = IVec3::new(i % per, (i / per) % per, i / (per * per));
                let from = low + at.as_vec3() * part;
                cuboid(
                    &mut gizmos,
                    build,
                    from,
                    from + Vec3::splat(part),
                    -0.03,
                    Color::srgba(1.0, 1.0, 1.0, 0.5),
                );
            }
        }
    }
}

/// A box of a base's own space, grown (or shrunk) by `pad` metres, outlined in the world.
fn cuboid(
    gizmos: &mut Gizmos<ViewGizmos>,
    build: &Build,
    low: Vec3,
    high: Vec3,
    pad: f32,
    tint: Color,
) {
    let (low, high) = (low - Vec3::splat(pad), high + Vec3::splat(pad));
    let place = Transform::from_translation((low + high) * 0.5).with_scale(high - low);
    let placed = build.frame.mul_transform(place);
    gizmos.cube(placed, tint);
}

/// Each damaged cube boxed in the colour of how much of its health it has lost.
fn damaged(
    camera: Query<&GlobalTransform, With<MainCamera>>,
    site: Option<Res<Site>>,
    built: Query<&Built>,
    mut gizmos: Gizmos<ViewGizmos>,
) {
    let Some(eye) = eye(&camera) else {
        return;
    };
    for build in builds(site.as_deref(), &built) {
        if build.frame.translation.distance(eye) > 300.0 {
            continue;
        }
        let rules = build
            .kits
            .as_deref()
            .map(|kits| kits.rules())
            .unwrap_or_default();
        for (at, damage) in build.base.damaged() {
            let Some(cube) = build.base.cube(at) else {
                continue;
            };
            let health = f32::from(rules.toughness(cube.kind).health.max(1));
            let lost = (f32::from(damage) / health).clamp(0.0, 1.0);
            let low = at.as_vec3() * CELL_M;
            cuboid(
                &mut gizmos,
                build,
                low,
                low + Vec3::splat(CELL_M),
                0.01,
                colour(health_colour(lost)),
            );
        }
    }
}

/// How much of its health a cube has lost, as `build_views.wesl`'s damage view colours it.
fn health_colour(lost: f32) -> [f32; 3] {
    let smooth = |a: f32, b: f32, x: f32| {
        let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let mix = |a: [f32; 3], b: [f32; 3], t: f32| [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t);
    let c = mix([0.8, 0.7, 0.05], [0.8, 0.3, 0.02], smooth(0.0, 0.5, lost));
    mix(c, [0.75, 0.04, 0.04], smooth(0.5, 1.0, lost))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_view_is_reached_by_its_name_and_by_cycling() {
        for view in View::ALL {
            assert_eq!(View::named(view.name()), Some(view));
        }
        let mut seen = vec![View::Off];
        let mut at = View::Off.step(false);
        while at != View::Off {
            seen.push(at);
            at = at.step(false);
        }
        assert_eq!(seen, View::ALL.to_vec());
        for view in View::ALL {
            assert_eq!(view.step(false).step(true), view);
        }
    }

    #[test]
    fn every_view_on_says_what_it_shows_and_has_a_legend() {
        for view in View::ALL.into_iter().filter(|v| *v != View::Off) {
            assert!(!view.title().is_empty(), "{view:?}");
            assert!(!view.reading().is_empty(), "{view:?}");
            assert!(view.key().len() >= 3, "{view:?}");
        }
    }

    #[test]
    fn every_role_and_roof_line_has_a_colour_of_its_own() {
        let mut colours: Vec<[f32; 3]> = Role::ALL.into_iter().map(role_colour).collect();
        colours.extend(ROOF_PARTS.into_iter().filter_map(roof_colour));
        for (i, a) in colours.iter().enumerate() {
            for b in &colours[i + 1..] {
                let apart: f32 = (0..3).map(|k| (a[k] - b[k]).abs()).sum();
                assert!(apart > 0.15, "{a:?} and {b:?} are too alike to tell apart");
            }
        }
        let legend = View::Roles.key();
        for role in Role::ALL {
            assert!(legend.iter().any(|(_, name)| *name == format!("{role:?}")));
        }
    }

    #[test]
    fn a_crest_runs_along_the_top_of_its_face() {
        let cube = IVec3::new(2, 1, -3);
        let edge = layout::roles::Edge {
            role: Role::Crest,
            side: Dir::South,
            other: Dir::Up,
        };
        let (a, b) = edge_line(cube, edge);
        // Along x, at the cube's top and its south face, a centimetre out of both.
        assert!((a.y - (2.0 * CELL_M + 0.01)).abs() < 1e-4, "{a}");
        assert!((a.z - (-2.0 * CELL_M + 0.01)).abs() < 1e-4, "{a}");
        assert!((b.x - a.x - CELL_M).abs() < 1e-4 && (b.y - a.y).abs() < 1e-6);
    }

    #[test]
    fn a_face_lies_on_its_side_of_the_cube() {
        let quad = face_quad(IVec3::ZERO, Dir::West);
        assert!(quad.iter().all(|p| (p.x + 0.006).abs() < 1e-5));
        let quad = face_quad(IVec3::ZERO, Dir::Up);
        assert!(quad.iter().all(|p| (p.y - CELL_M - 0.006).abs() < 1e-5));
        let spans = |i: usize| quad.iter().map(|p| p[i]).fold(f32::MIN, f32::max);
        assert!((spans(0) - CELL_M).abs() < 1e-5 && (spans(2) - CELL_M).abs() < 1e-5);
    }

    #[test]
    fn health_lost_runs_from_yellow_to_red() {
        let little = health_colour(0.05);
        let gone = health_colour(1.0);
        assert!(little[1] > 0.6, "a little lost reads yellow: {little:?}");
        assert!(
            gone[1] < 0.1 && gone[0] > 0.7,
            "all but gone reads red: {gone:?}"
        );
    }
}
