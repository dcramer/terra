//! The item icon renderer (D129), in the house style (D130): each icon rendered from the
//! item's own look by a camera of its own into an image. Held things lie along the icon's
//! diagonal, square on, broad side to the front, working end up and to the right; anything
//! else faces the front, seen from a little above. One rig for every icon, no background.
//! Nothing here knows any one item. The playbook is `.agents/skills/item-icons/SKILL.md`.
//!
//! Icons are baked during development, not rendered in play: `cargo xtask icons` runs the
//! `icons` experiment, which renders every item with this and writes `assets/icons/<item>.png`
//! and a manifest of what each was rendered from. The game loads the files ([`file`]).
//!
//! Ask with [`Icons::want`] and read with [`Icons::get`]. Icons are rendered one at a time,
//! a few frames each, on a render layer of their own, so the world and its lights are not
//! in them.

use std::collections::{HashMap, VecDeque};

use bevy::{
    asset::LoadState,
    camera::{RenderTarget, ScalingMode, primitives::Aabb, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    gltf::{GltfAssetLabel, GltfExtras, GltfMaterialName},
    post_process::bloom::Bloom,
    prelude::*,
    render::{
        render_resource::{TextureFormat, TextureUsages},
        view::{
            ColorGrading, ColorGradingGlobal, ColorGradingSection,
            screenshot::{Screenshot, ScreenshotCaptured},
        },
    },
    world_serialization::{WorldAsset, WorldAssetRoot, WorldInstanceReady},
};
use terra_walker::strap::Strap;
use terra_world::loading::Shaders;

/// The style's version: raise it whenever the rig changes, so every icon baked before is
/// stale and the check asks for them to be baked again.
pub const STYLE: u32 = 2;
/// Where the manifest of icons is, under `assets/`: a line per item, `<item> <source>
/// <look> <fingerprint of the look> <style>`, the source `rendered` or `painted`
/// (`tools/icons/paint.ts`). A painted icon of an item with no look has `-` for its look
/// and fingerprint; a painted icon has `-` for the render's style.
pub const MANIFEST: &str = "icons/manifest.txt";

/// A line of the manifest: where an item's icon came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Baked {
    pub item: String,
    pub painted: bool,
    pub look: Option<String>,
    pub fingerprint: Option<u64>,
    pub style: Option<u32>,
}

impl Baked {
    pub fn parse(line: &str) -> Option<Baked> {
        let mut words = line.split_whitespace();
        let item = words.next()?.to_string();
        let painted = match words.next()? {
            "painted" => true,
            "rendered" => false,
            _ => return None,
        };
        let dash = |word: &str| (word != "-").then(|| word.to_string());
        let look = dash(words.next()?);
        let fingerprint = dash(words.next()?).and_then(|hex| u64::from_str_radix(&hex, 16).ok());
        let style = dash(words.next()?).and_then(|style| style.parse().ok());
        Some(Baked {
            item,
            painted,
            look,
            fingerprint,
            style,
        })
    }

    pub fn line(&self) -> String {
        format!(
            "{} {} {} {} {}",
            self.item,
            if self.painted { "painted" } else { "rendered" },
            self.look.as_deref().unwrap_or("-"),
            self.fingerprint
                .map_or("-".to_string(), |hash| format!("{hash:016x}")),
            self.style
                .map_or("-".to_string(), |style| style.to_string()),
        )
    }
}

/// Where an item's baked icon is, under `assets/`, by its id (`character::item_id`).
pub fn file(item: &str) -> String {
    format!("icons/{item}.png")
}

/// A fingerprint of a look's file, so a baked icon is known to be stale when its look
/// changes: FNV-1a, which is stable across machines and versions.
pub fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The render layer icons are drawn on, apart from the world's.
pub const LAYER: usize = 7;
/// Pixels drawn for each cell an item fills: twice a backpack cell on screen, so an icon
/// stays sharp on a high-density display.
pub const PX_PER_CELL: u32 = 96;

/// The house style's rig: the play scene's warm sun and blue shade, and a cool light from
/// behind that picks the shape out of the slot without an outline.
const SUN: Color = Color::srgb(1.0, 0.87, 0.7);
const SUN_LUX: f32 = 21_000.0;
const SHADE: Color = Color::srgb(0.66, 0.76, 1.0);
const SHADE_BRIGHTNESS: f32 = 1800.0;
const EDGE: Color = Color::srgb(0.85, 0.92, 1.0);
const EDGE_LUX: f32 = SUN_LUX * 3.0;
/// How far above a worn thing the camera looks down from, so a cap shows its crown and
/// not its inside.
const WORN_RISE_DEGREES: f32 = 15.0;
/// Room round the look, as a share of its size.
const MARGIN: f32 = 1.15;
/// Frames the camera draws an icon for before it is kept.
const DRAW_FRAMES: u32 = 3;
/// An icon is kept once no shader has been compiling for this many frames in a row, so a
/// look whose shaders are not ready yet is not kept blank...
const SHADERS_READY_FRAMES: u32 = 2;
/// ...or after this many frames whatever else is compiling, so a busy world cannot stall it.
const MOST_DRAW_FRAMES: u32 = 120;

/// What an icon shows: a look, and the cells its item fills, which set the icon's shape.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Icon {
    pub look: String,
    pub cells: (u8, u8),
}

impl Icon {
    pub fn new(look: impl Into<String>, cells: (u8, u8)) -> Self {
        Icon {
            look: look.into(),
            cells: (cells.0.max(1), cells.1.max(1)),
        }
    }

    /// Held things are laid across their icon; worn things stand as worn.
    fn held(&self) -> bool {
        self.look.starts_with("models/held/")
    }
}

/// Every icon asked for: rendered, as RGBA `cells` times `PX_PER_CELL` on a side, or on
/// its way, or failed (its look did not load or had nothing to draw).
#[derive(Resource, Default)]
pub struct Icons {
    made: HashMap<Icon, Option<Vec<u8>>>,
    queue: VecDeque<Icon>,
    drawing: Option<Drawing>,
}

impl Icons {
    /// Asks for an icon, if it has not been asked for already.
    pub fn want(&mut self, icon: &Icon) {
        if let std::collections::hash_map::Entry::Vacant(entry) = self.made.entry(icon.clone()) {
            entry.insert(None);
            self.queue.push_back(icon.clone());
        }
    }

    /// The icon as RGBA, once it is rendered.
    pub fn get(&self, icon: &Icon) -> Option<&[u8]> {
        self.made.get(&icon.clone())?.as_deref()
    }

    /// Whether every icon asked for is made or given up on.
    pub fn settled(&self) -> bool {
        self.drawing.is_none() && self.queue.is_empty()
    }
}

/// The icon being drawn, and how far along it is.
struct Drawing {
    subject: Icon,
    look: Entity,
    scene: Handle<WorldAsset>,
    /// Frames since the look appeared, until it is placed; then frames drawn.
    frames: u32,
    placed: bool,
    /// A copy has been asked for, and has come back.
    copying: bool,
    copied: bool,
    /// Frames in a row with no shader compiling, while drawing.
    shaders_ready_for: u32,
    camera: Option<Entity>,
    image: Option<Handle<Image>>,
}

/// The icon's sun and edge light, turned with the camera for each icon.
#[derive(Component)]
enum IconLight {
    Sun,
    Edge,
}

/// A look being drawn, once it has loaded.
#[derive(Component)]
struct Ready;

pub fn plugin(app: &mut App) {
    app.init_resource::<Icons>()
        .add_systems(Startup, setup)
        .add_systems(Update, draw);
}

fn setup(mut commands: Commands) {
    for (light, colour, lux) in [
        (IconLight::Sun, SUN, SUN_LUX),
        (IconLight::Edge, EDGE, EDGE_LUX),
    ] {
        commands.spawn((
            Name::new("Icon light"),
            DirectionalLight {
                color: colour,
                illuminance: lux,
                shadow_maps_enabled: matches!(light, IconLight::Sun),
                ..default()
            },
            light,
            RenderLayers::layer(LAYER),
        ));
    }
}

/// A camera drawing one icon into `image`, from where `frame` puts it. Made for each
/// icon, since a camera keeps the target it starts with.
fn camera(image: Handle<Image>, at: Transform, projection: Projection) -> impl Bundle {
    (
        Name::new("Icon camera"),
        Camera3d::default(),
        Camera {
            // Before the view, so an icon is drawn before the screens that show it.
            order: -1,
            clear_color: Color::NONE.into(),
            ..default()
        },
        RenderTarget::Image(image.into()),
        at,
        projection,
        RenderLayers::layer(LAYER),
        AmbientLight {
            color: SHADE,
            brightness: SHADE_BRIGHTNESS,
            ..default()
        },
        Bloom {
            intensity: 0.2,
            ..Bloom::NATURAL
        },
        Tonemapping::TonyMcMapface,
        // As the play scene grades its camera.
        ColorGrading {
            global: ColorGradingGlobal {
                post_saturation: 1.15,
                ..default()
            },
            shadows: ColorGradingSection {
                contrast: 1.04,
                ..default()
            },
            ..default()
        },
    )
}

/// Draws the next icon asked for, a step a frame: load the look, place it and the
/// camera, draw it for a few frames, keep the image.
#[allow(clippy::too_many_arguments)]
fn draw(
    mut commands: Commands,
    icons: ResMut<Icons>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    ready: Query<(), With<Ready>>,
    turns: Query<&IconTurn>,
    strapped: Query<(), With<FacesBack>>,
    children: Query<&Children>,
    bounds: Query<(&Aabb, &GlobalTransform), Without<LeftOut>>,
    mut lights: Query<(&mut Transform, &IconLight)>,
    shaders: Res<Shaders>,
) {
    let icons = icons.into_inner();
    let Some(drawing) = icons.drawing.as_mut() else {
        let Some(subject) = icons.queue.pop_front() else {
            return;
        };
        let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(subject.look.clone()));
        let look = commands
            .spawn((
                Name::new(format!("Icon of {}", subject.look)),
                WorldAssetRoot(scene.clone()),
                Transform::IDENTITY,
            ))
            .observe(prepare)
            .id();
        icons.drawing = Some(Drawing {
            subject,
            look,
            scene,
            frames: 0,
            placed: false,
            copying: false,
            copied: false,
            shaders_ready_for: 0,
            camera: None,
            image: None,
        });
        return;
    };
    if let LoadState::Failed(error) = assets.load_state(&drawing.scene) {
        warn!("icons: {} did not load: {error}", drawing.subject.look);
        let drawing = icons.drawing.take().expect("drawing");
        commands.entity(drawing.look).despawn();
        icons.made.insert(drawing.subject.clone(), None);
        return;
    }
    if !ready.contains(drawing.look) {
        return;
    }
    drawing.frames += 1;
    if !drawing.placed {
        // Bounds are worked out after the frame the look appears in.
        if drawing.frames < 3 {
            return;
        }
        // The look stands unturned at the origin, so these are its own corners.
        let corners = corners(drawing.look, &children, &bounds);
        if corners.is_empty() {
            warn!("icons: {} has nothing to draw", drawing.subject.look);
            let drawing = icons.drawing.take().expect("drawing");
            commands.entity(drawing.look).despawn();
            icons.made.insert(drawing.subject.clone(), None);
            return;
        }
        let (w, h) = drawing.subject.cells;
        let turn = if drawing.subject.held() {
            let front = if strapped.contains(drawing.look) {
                Vec3::NEG_Z
            } else {
                Vec3::Z
            };
            lay(&corners, f32::from(w) / f32::from(h), front)
        } else {
            Quat::from_rotation_y(turns.get(drawing.look).map_or(0.0, |t| t.0.to_radians()))
        };
        commands
            .entity(drawing.look)
            .insert(Transform::from_rotation(turn));
        let corners: Vec<Vec3> = corners.into_iter().map(|corner| turn * corner).collect();
        let rise = if drawing.subject.held() {
            0.0
        } else {
            WORN_RISE_DEGREES
        };
        let (camera_at, projection) = frame(&corners, rise, f32::from(w) / f32::from(h));
        for (mut at, light) in &mut lights {
            *at = shining(light, &camera_at);
        }
        let mut image = Image::new_target_texture(
            u32::from(w) * PX_PER_CELL,
            u32::from(h) * PX_PER_CELL,
            TextureFormat::Rgba8Unorm,
            Some(TextureFormat::Rgba8UnormSrgb),
        );
        // So an icon can be read back, as the sheet does.
        image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let image = images.add(image);
        drawing.camera = Some(
            commands
                .spawn(camera(image.clone(), camera_at, projection))
                .id(),
        );
        drawing.image = Some(image);
        drawing.placed = true;
        drawing.frames = 0;
        return;
    }
    drawing.shaders_ready_for = if shaders.idle() {
        drawing.shaders_ready_for + 1
    } else {
        0
    };
    let drawn = drawing.frames >= DRAW_FRAMES && drawing.shaders_ready_for >= SHADERS_READY_FRAMES;
    if !drawn && drawing.frames < MOST_DRAW_FRAMES {
        return;
    }
    // An image can only be read back while a camera renders into it, so the camera keeps
    // rendering until the copy comes back.
    if !drawing.copied {
        if !drawing.copying
            && let Some(image) = drawing.image.clone()
        {
            drawing.copying = true;
            commands.spawn(Screenshot::image(image)).observe(
                |captured: On<ScreenshotCaptured>, mut icons: ResMut<Icons>| {
                    let Some(drawing) = icons.drawing.as_mut() else {
                        return;
                    };
                    drawing.copied = true;
                    let icon = drawing.subject.clone();
                    let copy = match captured.image.clone().try_into_dynamic() {
                        Ok(icon) => Some(icon.to_rgba8().into_raw()),
                        Err(error) => {
                            error!("icons: could not read an icon back: {error}");
                            None
                        }
                    };
                    icons.made.insert(icon, copy);
                },
            );
        }
        return;
    }
    let drawing = icons.drawing.take().expect("drawing");
    commands.entity(drawing.look).despawn();
    if let Some(camera) = drawing.camera {
        commands.entity(camera).despawn();
    }
}

/// Once a look has loaded, readies it for its icon: on the icons' layer, with what its
/// extras say of it (its turn, whether it is strapped, the parts it leaves out).
fn prepare(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    extras: Query<&GltfExtras>,
    materials: Query<&GltfMaterialName>,
) {
    let left_out = left_out(ready.entity, &children, &extras);
    if let Some(turn) = extra(ready.entity, &children, &extras, "icon_turn") {
        commands.entity(ready.entity).insert(IconTurn(turn));
    }
    if extra::<Strap>(ready.entity, &children, &extras, "strap").is_some() {
        commands.entity(ready.entity).insert(FacesBack);
    }
    for each in children.iter_descendants(ready.entity) {
        commands.entity(each).insert(RenderLayers::layer(LAYER));
        // A role is the material's name before any `.`, as tints read it.
        let role = |name: &GltfMaterialName| name.0.split('.').next().unwrap_or("").to_string();
        if materials
            .get(each)
            .is_ok_and(|name| left_out.contains(&role(name)))
        {
            commands.entity(each).insert((LeftOut, Visibility::Hidden));
        }
    }
    commands.entity(ready.entity).insert(Ready);
}

/// A part of a look its icon leaves out, by the look's own say.
#[derive(Component)]
struct LeftOut;

/// How far a look turns about its up, in degrees, to be drawn from the side it is seen from:
/// a pack from behind its wearer (`{"icon_turn": 180}` in its glTF extras).
#[derive(Component)]
struct IconTurn(f32);

/// A held look strapped to the forearm (a shield), whose front is its -Z, as the strap
/// has it (`terra_walker::strap`), where the held contract has +Z.
#[derive(Component)]
struct FacesBack;

/// A value under `key` in the glTF extras of any part of `look`.
fn extra<T: serde::de::DeserializeOwned>(
    look: Entity,
    children: &Query<&Children>,
    extras: &Query<&GltfExtras>,
    key: &str,
) -> Option<T> {
    children.iter_descendants(look).find_map(|each| {
        let value: serde_json::Value = serde_json::from_str(&extras.get(each).ok()?.value).ok()?;
        serde_json::from_value(value.get(key)?.clone()).ok()
    })
}

/// The material roles a look's icon leaves out, from its glTF extras: `{"icon_leaves_out":
/// ["Harness"]}`, as a pack's straps, which are made on the body and hang loose without it.
fn left_out(look: Entity, children: &Query<&Children>, extras: &Query<&GltfExtras>) -> Vec<String> {
    extra(look, children, extras, "icon_leaves_out").unwrap_or_default()
}

/// The corners of every part of a look the icon shows, in the world.
fn corners(
    look: Entity,
    children: &Query<&Children>,
    bounds: &Query<(&Aabb, &GlobalTransform), Without<LeftOut>>,
) -> Vec<Vec3> {
    children
        .iter_descendants(look)
        .filter_map(|each| bounds.get(each).ok())
        .flat_map(|(aabb, global)| {
            let (centre, half) = (Vec3::from(aabb.center), Vec3::from(aabb.half_extents));
            (0..8).map(move |corner| {
                let sign = Vec3::new(
                    if corner & 1 == 0 { -1.0 } else { 1.0 },
                    if corner & 2 == 0 { -1.0 } else { 1.0 },
                    if corner & 4 == 0 { -1.0 } else { 1.0 },
                );
                global.transform_point(centre + half * sign)
            })
        })
        .collect()
}

/// How to turn a held look, handle along +Y, so it lies along the diagonal of an icon
/// `aspect` times as wide as it is tall, working end up and to the right, broad side to
/// the front (+Z): whichever of X and Z it is thinner along is turned to face the camera,
/// and a look thin along Z shows the side its `front` (+Z or -Z) is on.
pub fn lay(corners: &[Vec3], aspect: f32, front: Vec3) -> Quat {
    let (low, high) = span(corners);
    let size = high - low;
    let broadside = if size.x < size.z {
        Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
    } else if front.z < 0.0 {
        Quat::from_rotation_y(std::f32::consts::PI)
    } else {
        Quat::IDENTITY
    };
    let diagonal = libm::atan2f(1.0, aspect);
    Quat::from_rotation_z(diagonal - std::f32::consts::FRAC_PI_2) * broadside
}

fn span(corners: &[Vec3]) -> (Vec3, Vec3) {
    corners.iter().fold(
        (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
        |(low, high), corner| (low.min(*corner), high.max(*corner)),
    )
}

/// The camera, from the front and `rise` degrees above, seeing every corner with a margin
/// in an icon `aspect` times as wide as it is tall, without perspective.
pub fn frame(corners: &[Vec3], rise: f32, aspect: f32) -> (Transform, Projection) {
    let (low, high) = span(corners);
    let middle = (low + high) * 0.5;
    let rise = rise.to_radians();
    let out = Vec3::new(0.0, libm::sinf(rise), libm::cosf(rise));
    let mut transform =
        Transform::from_translation(middle + out * 10.0).looking_at(middle, Vec3::Y);
    // Centre the icon on what is seen, not on the box's middle.
    let (right, up) = (*transform.right(), *transform.up());
    let (mut left, mut most_right, mut bottom, mut top) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for corner in corners {
        let offset = *corner - middle;
        let (x, y) = (offset.dot(right), offset.dot(up));
        (left, most_right, bottom, top) =
            (left.min(x), most_right.max(x), bottom.min(y), top.max(y));
    }
    transform.translation += right * (left + most_right) * 0.5 + up * (bottom + top) * 0.5;
    let width = (most_right - left).max((top - bottom) * aspect).max(0.01) * MARGIN;
    let projection = Projection::Orthographic(OrthographicProjection {
        scaling_mode: ScalingMode::Fixed {
            width,
            height: width / aspect,
        },
        near: 0.0,
        far: 20.0,
        ..OrthographicProjection::default_3d()
    });
    (transform, projection)
}

/// Where a light shines from, by the camera: the sun from above, in front and to the
/// left; the edge light from behind and to the right.
fn shining(light: &IconLight, camera: &Transform) -> Transform {
    let (back, up, right) = (*camera.back(), *camera.up(), *camera.right());
    let from = match light {
        IconLight::Sun => back * 0.7 + up * 0.8 - right * 0.6,
        IconLight::Edge => -back * 0.9 + up * 0.35 + right * 0.5,
    };
    Transform::IDENTITY.looking_to(-from.normalize(), Vec3::Y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fingerprint_is_fnv1a_as_the_painter_computes_it() {
        // The same values tools/icons/paint.test.ts checks.
        assert_eq!(fingerprint(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fingerprint(b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn a_manifest_line_reads_back() {
        let rendered = Baked {
            item: "stone_axe".into(),
            painted: false,
            look: Some("models/held/stone_axe.glb".into()),
            fingerprint: Some(0xff83_4e6b_0ad4_b8d2),
            style: Some(1),
        };
        assert_eq!(Baked::parse(&rendered.line()), Some(rendered.clone()));
        let stone = Baked {
            item: "stone".into(),
            painted: true,
            look: None,
            fingerprint: None,
            style: None,
        };
        assert_eq!(stone.line(), "stone painted - - -");
        assert_eq!(Baked::parse(&stone.line()), Some(stone));
    }

    #[test]
    fn a_frame_sees_every_corner_in_its_shape() {
        let corners = [Vec3::new(-1.0, -0.2, -0.1), Vec3::new(1.0, 0.2, 0.1)];
        for aspect in [1.0, 2.0, 0.5] {
            let (transform, projection) = frame(&corners, WORN_RISE_DEGREES, aspect);
            let Projection::Orthographic(projection) = projection else {
                panic!("icons are drawn without perspective");
            };
            let ScalingMode::Fixed { width, height } = projection.scaling_mode else {
                panic!("an icon's size is fixed");
            };
            assert!((width / height - aspect).abs() < 1e-4);
            for corner in corners {
                let offset = corner - transform.translation;
                assert!(offset.dot(*transform.right()).abs() < width * 0.5);
                assert!(offset.dot(*transform.up()).abs() < height * 0.5);
            }
        }
    }

    #[test]
    fn a_held_look_lies_broadside_along_the_diagonal() {
        // An axe head thin along X, as the held contract has it (edge toward +Z).
        let corners = [Vec3::new(-0.01, 0.0, -0.1), Vec3::new(0.01, 0.6, 0.1)];
        let turn = lay(&corners, 1.0, Vec3::Z);
        let thin = turn * Vec3::X;
        assert!(
            thin.z.abs() > 0.99,
            "its thin side faces the camera: {thin}"
        );
        let working_end = turn * Vec3::Y;
        assert!(working_end.x > 0.6 && working_end.y > 0.6, "{working_end}");
        // Two cells wide, it lies flatter, along that icon's diagonal.
        let working_end = lay(&corners, 2.0, Vec3::Z) * Vec3::Y;
        assert!(
            (working_end.y / working_end.x - 0.5).abs() < 1e-4,
            "{working_end}"
        );
    }

    #[test]
    fn a_board_shows_its_front_whichever_side_it_is_on() {
        // A shield thin along Z: the camera sits toward +Z, so the front must turn to +Z.
        let corners = [Vec3::new(-0.3, 0.0, -0.04), Vec3::new(0.3, 0.7, 0.04)];
        for front in [Vec3::Z, Vec3::NEG_Z] {
            let shown = lay(&corners, 1.0, front) * front;
            assert!(
                shown.z > 0.99,
                "its front {front} faces the camera: {shown}"
            );
            let working_end = lay(&corners, 1.0, front) * Vec3::Y;
            assert!(working_end.x > 0.6 && working_end.y > 0.6, "{working_end}");
        }
    }

    #[test]
    fn an_icon_is_asked_for_once() {
        let mut icons = Icons::default();
        let axe = Icon::new("models/held/stone_axe.glb", (2, 1));
        icons.want(&axe);
        icons.want(&axe);
        assert_eq!(icons.queue.len(), 1);
        assert!(icons.get(&axe).is_none());
        // Another shape is another icon.
        icons.want(&Icon::new("models/held/stone_axe.glb", (1, 1)));
        assert_eq!(icons.queue.len(), 2);
    }
}
