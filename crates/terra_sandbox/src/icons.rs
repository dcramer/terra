//! Item icons (D129) in the house style (D130): each drawn from the item's own look by a
//! camera of its own into an image, the first time something asks for it, and kept. Held
//! things lie along the icon's diagonal, square on, broad side to the front, working end
//! up and to the right; worn things face the front, seen from a little above. One rig for
//! every icon, no background. Nothing here knows any one item. The playbook is
//! `.claude/skills/item-icons/SKILL.md`.
//!
//! Ask with [`Icons::want`] and read with [`Icons::get`]. The resource counts as
//! changed only when an icon is made or given up on, so a screen can lay itself out again
//! then. Icons are drawn one at a time, a few frames each, on a render layer of their
//! own, so the world and its lights are not in them.

use std::collections::{HashMap, VecDeque};

use bevy::{
    asset::LoadState,
    camera::{RenderTarget, ScalingMode, primitives::Aabb, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    gltf::GltfAssetLabel,
    post_process::bloom::Bloom,
    prelude::*,
    render::{
        ExtractSchedule, MainWorld, RenderApp,
        render_resource::{PipelineCache, TextureFormat, TextureUsages},
        view::{
            ColorGrading, ColorGradingGlobal, ColorGradingSection,
            screenshot::{Screenshot, ScreenshotCaptured},
        },
    },
    world_serialization::{WorldAsset, WorldAssetRoot, WorldInstanceReady},
};

use crate::character::Tint;

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

/// What an icon shows: a look, how it is coloured, and the cells its item fills, which
/// set the icon's shape.
#[derive(Clone, Debug, PartialEq)]
pub struct Icon {
    pub look: String,
    pub tint: Tint,
    pub cells: (u8, u8),
}

impl Icon {
    pub fn new(look: impl Into<String>, cells: (u8, u8)) -> Self {
        Icon {
            look: look.into(),
            tint: Tint::default(),
            cells: (cells.0.max(1), cells.1.max(1)),
        }
    }

    fn key(&self) -> Key {
        Key {
            look: self.look.clone(),
            tint: self
                .tint
                .0
                .iter()
                .map(|(role, colour)| {
                    (
                        role.clone(),
                        colour.to_linear().to_f32_array().map(f32::to_bits),
                    )
                })
                .collect(),
            cells: self.cells,
        }
    }

    /// Held things are laid across their icon; worn things stand as worn.
    fn held(&self) -> bool {
        self.look.starts_with("models/held/")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    look: String,
    tint: Vec<(String, [u32; 4])>,
    cells: (u8, u8),
}

enum Made {
    Waiting,
    Ready(Handle<Image>),
    /// The look did not load or had nothing to draw; whoever asked shows something else.
    Failed,
}

/// Every icon asked for, made or on its way.
#[derive(Resource, Default)]
pub struct Icons {
    made: HashMap<Key, Made>,
    queue: VecDeque<Icon>,
    drawing: Option<Drawing>,
    /// Keep a copy of each icon on the CPU as it is drawn, for tools that save them. A
    /// icon can only be read back while its camera draws it.
    pub keep_copies: bool,
    copies: HashMap<Key, Vec<u8>>,
}

impl Icons {
    /// Asks for an icon, if it has not been asked for already. Call through
    /// `bypass_change_detection`, so asking does not count as a change.
    pub fn want(&mut self, icon: &Icon) {
        if let std::collections::hash_map::Entry::Vacant(entry) = self.made.entry(icon.key()) {
            entry.insert(Made::Waiting);
            self.queue.push_back(icon.clone());
        }
    }

    /// The icon, once it is made.
    pub fn get(&self, subject: &Icon) -> Option<Handle<Image>> {
        match self.made.get(&subject.key()) {
            Some(Made::Ready(image)) => Some(image.clone()),
            _ => None,
        }
    }

    /// A copy of the icon, as RGBA, if `keep_copies` was set when it was drawn.
    pub fn copy(&self, subject: &Icon) -> Option<&[u8]> {
        self.copies.get(&subject.key()).map(Vec::as_slice)
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
    /// A copy has been asked for.
    copying: bool,
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
        .init_resource::<ShadersReady>()
        .add_systems(Startup, setup)
        .add_systems(Update, draw);
    // Only the render world knows whether shaders are compiling; it says so as it extracts.
    app.sub_app_mut(RenderApp)
        .add_systems(ExtractSchedule, shaders_ready);
}

/// Whether no shader was compiling when the last frame was extracted.
#[derive(Resource, Default)]
struct ShadersReady(bool);

fn shaders_ready(mut main_world: ResMut<MainWorld>, pipelines: Res<PipelineCache>) {
    if let Some(mut ready) = main_world.get_resource_mut::<ShadersReady>() {
        ready.0 = pipelines.waiting_pipelines().next().is_none();
    }
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
    mut icons: ResMut<Icons>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    ready: Query<(), With<Ready>>,
    children: Query<&Children>,
    bounds: Query<(&Aabb, &GlobalTransform)>,
    mut lights: Query<(&mut Transform, &IconLight)>,
    shaders: Res<ShadersReady>,
) {
    let icons = icons.bypass_change_detection();
    let Some(drawing) = icons.drawing.as_mut() else {
        let Some(subject) = icons.queue.pop_front() else {
            return;
        };
        let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(subject.look.clone()));
        let look = commands
            .spawn((
                Name::new(format!("Icon of {}", subject.look)),
                WorldAssetRoot(scene.clone()),
                subject.tint.clone(),
                Transform::IDENTITY,
            ))
            .observe(
                |ready: On<WorldInstanceReady>,
                 mut commands: Commands,
                 children: Query<&Children>| {
                    for each in children.iter_descendants(ready.entity) {
                        commands.entity(each).insert(RenderLayers::layer(LAYER));
                    }
                    commands.entity(ready.entity).insert(Ready);
                },
            )
            .id();
        icons.drawing = Some(Drawing {
            subject,
            look,
            scene,
            frames: 0,
            placed: false,
            copying: false,
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
        icons.made.insert(drawing.subject.key(), Made::Failed);
        icons_changed(&mut commands);
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
            icons.made.insert(drawing.subject.key(), Made::Failed);
            icons_changed(&mut commands);
            return;
        }
        let (w, h) = drawing.subject.cells;
        let turn = if drawing.subject.held() {
            lay(&corners, f32::from(w) / f32::from(h))
        } else {
            Quat::IDENTITY
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
    drawing.shaders_ready_for = if shaders.0 {
        drawing.shaders_ready_for + 1
    } else {
        0
    };
    let drawn = drawing.frames >= DRAW_FRAMES && drawing.shaders_ready_for >= SHADERS_READY_FRAMES;
    if !drawn && drawing.frames < MOST_DRAW_FRAMES {
        return;
    }
    if icons.keep_copies {
        let key = drawing.subject.key();
        if icons.copies.contains_key(&key) {
            // Copied; done.
        } else {
            if !drawing.copying
                && let Some(image) = drawing.image.clone()
            {
                drawing.copying = true;
                commands.spawn(Screenshot::image(image)).observe(
                    move |captured: On<ScreenshotCaptured>, mut icons: ResMut<Icons>| {
                        let copy = match captured.image.clone().try_into_dynamic() {
                            Ok(icon) => icon.to_rgba8().into_raw(),
                            Err(error) => {
                                error!("icons: could not copy an icon: {error}");
                                Vec::new()
                            }
                        };
                        icons
                            .bypass_change_detection()
                            .copies
                            .insert(key.clone(), copy);
                    },
                );
            }
            return;
        }
    }
    let drawing = icons.drawing.take().expect("drawing");
    commands.entity(drawing.look).despawn();
    if let Some(camera) = drawing.camera {
        commands.entity(camera).despawn();
    }
    if let Some(image) = drawing.image {
        icons.made.insert(drawing.subject.key(), Made::Ready(image));
    }
    icons_changed(&mut commands);
}

/// Marks the icons changed, so screens showing them lay themselves out again.
fn icons_changed(commands: &mut Commands) {
    commands.queue(|world: &mut World| world.resource_mut::<Icons>().set_changed());
}

/// The corners of every part of a look, in the world.
fn corners(
    look: Entity,
    children: &Query<&Children>,
    bounds: &Query<(&Aabb, &GlobalTransform)>,
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
/// the front (+Z): whichever of X and Z it is thinner along is turned to face the camera.
pub fn lay(corners: &[Vec3], aspect: f32) -> Quat {
    let (low, high) = span(corners);
    let size = high - low;
    let broadside = if size.x < size.z {
        Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
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
        let turn = lay(&corners, 1.0);
        let thin = turn * Vec3::X;
        assert!(
            thin.z.abs() > 0.99,
            "its thin side faces the camera: {thin}"
        );
        let working_end = turn * Vec3::Y;
        assert!(working_end.x > 0.6 && working_end.y > 0.6, "{working_end}");
        // Two cells wide, it lies flatter, along that icon's diagonal.
        let working_end = lay(&corners, 2.0) * Vec3::Y;
        assert!(
            (working_end.y / working_end.x - 0.5).abs() < 1e-4,
            "{working_end}"
        );
    }

    #[test]
    fn a_icon_is_asked_for_once() {
        let mut icons = Icons::default();
        let axe = Icon::new("models/held/stone_axe.glb", (2, 1));
        icons.want(&axe);
        icons.want(&axe);
        assert_eq!(icons.queue.len(), 1);
        assert!(icons.get(&axe).is_none());
        // Another tint or shape is another icon.
        let mut dyed = axe.clone();
        dyed.tint = Tint(vec![("Wood".into(), Color::BLACK)]);
        icons.want(&dyed);
        icons.want(&Icon::new("models/held/stone_axe.glb", (1, 1)));
        assert_eq!(icons.queue.len(), 3);
    }
}
