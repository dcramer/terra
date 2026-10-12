//! Item pictures in the house style (D130): each look drawn from the look itself (D129),
//! in the world's light, the way the game will draw it for a slot. Held things lie corner
//! to corner, square on, broad side to the front, working end up and to the right; worn
//! things face the front, seen from a little above. One rig for every picture, no
//! background. The playbook is `.claude/skills/item-pictures/SKILL.md`.
//!
//! `--model <path>` draws one look under `assets/` (repeatable); with none, every held
//! look and every worn look on one base body. `--sheet <prefix>` lays them out on empty
//! slots and saves `<prefix>.png` at 128 pixels a picture and `<prefix>-64.png` at 64,
//! then exits.
//!
//! The sandbox's camera draws into an image on a render layer of its own, so the planet
//! and its sun are not in the picture; the picture's lights are on that layer too.

use std::path::{Path, PathBuf};

use bevy::{
    asset::RenderAssetUsages,
    camera::{RenderTarget, ScalingMode, primitives::Aabb, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    gltf::GltfAssetLabel,
    post_process::bloom::Bloom,
    prelude::*,
    render::{
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
        view::{
            ColorGrading, ColorGradingGlobal, ColorGradingSection,
            screenshot::{Screenshot, ScreenshotCaptured},
        },
    },
    world_serialization::{WorldAssetRoot, WorldInstanceReady},
};

use crate::shell::Options;

const LAYER: usize = 1;
/// Drawn at this size, then shrunk, which smooths the edges.
const DRAWN: u32 = 256;
const SHOWN: [u32; 2] = [128, 64];
const COLUMNS: usize = 8;
/// Frames to let a new view settle before a capture.
const SETTLE: u32 = 6;
/// The empty slot behind a picture on the sheet, and the gap between slots, as the play
/// screens have them.
const SLOT: [f32; 3] = [0.12, 0.13, 0.16];
const GAP: [f32; 3] = [0.05, 0.06, 0.08];

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

#[derive(Resource)]
struct Studio {
    files: Vec<String>,
    target: Handle<Image>,
    next: usize,
    item: Option<Entity>,
    /// The item's corners in the world, once it has loaded and been laid.
    corners: Option<Vec<Vec3>>,
    waited: u32,
    waiting: bool,
    /// Every picture so far, in order, as RGBA.
    pictures: Vec<Vec<u8>>,
}

#[derive(Component)]
struct Ready;

/// The picture's sun and edge light, turned with the camera for each picture.
#[derive(Component)]
enum PictureLight {
    Sun,
    Edge,
}

pub fn plugin(app: &mut App) {
    app.add_systems(PostStartup, setup).add_systems(Update, run);
}

fn is_held(file: &str) -> bool {
    file.starts_with("models/held/")
}

/// What `--model` names, or every held look and every worn look on the first base body.
fn files(options: &Options) -> Vec<String> {
    if !options.models.is_empty() {
        return options.models.clone();
    }
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let listed = |folder: &str| {
        let mut names: Vec<String> = std::fs::read_dir(assets.join(folder))
            .into_iter()
            .flatten()
            .flatten()
            .map(|file| file.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    let looks = |folder: String| {
        listed(&folder)
            .into_iter()
            .filter(|name| name.ends_with(".glb"))
            .map(move |name| format!("{folder}/{name}"))
    };
    let mut files: Vec<String> = looks("models/held".to_string()).collect();
    if let Some(body) = listed("models/gear").first() {
        files.extend(looks(format!("models/gear/{body}")));
    }
    files
}

fn setup(
    mut commands: Commands,
    camera: Single<Entity, With<Camera3d>>,
    options: Res<Options>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut image = Image::new_target_texture(
        DRAWN,
        DRAWN,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    );
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = images.add(image);
    // The sandbox's one camera becomes the picture's, so nothing else needs to know.
    commands.entity(*camera).insert((
        Camera {
            clear_color: Color::NONE.into(),
            ..default()
        },
        RenderTarget::Image(target.clone().into()),
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
    ));
    for (light, colour, lux) in [
        (PictureLight::Sun, SUN, SUN_LUX),
        (PictureLight::Edge, EDGE, EDGE_LUX),
    ] {
        commands.spawn((
            Name::new("Picture light"),
            DirectionalLight {
                color: colour,
                illuminance: lux,
                shadow_maps_enabled: matches!(light, PictureLight::Sun),
                ..default()
            },
            light,
            RenderLayers::layer(LAYER),
        ));
    }
    let files = files(&options);
    if files.is_empty() {
        error!("pictures: no looks to draw");
    }
    commands.insert_resource(Studio {
        files,
        target,
        next: 0,
        item: None,
        corners: None,
        waited: 0,
        waiting: false,
        pictures: Vec::new(),
    });
}

fn run(
    mut commands: Commands,
    studio: Option<ResMut<Studio>>,
    options: Res<Options>,
    assets: Res<AssetServer>,
    ready: Query<(), With<Ready>>,
    children: Query<&Children>,
    bounds: Query<(&Aabb, &GlobalTransform)>,
    mut camera: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
    mut lights: Query<(&mut Transform, &PictureLight), Without<Camera3d>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut studio) = studio else { return };
    if studio.waiting {
        return;
    }
    if studio.next >= studio.files.len() {
        if let Some(prefix) = &options.sheet {
            save(&studio.pictures, prefix);
        }
        exit.write(AppExit::Success);
        return;
    }
    let Some(item) = studio.item else {
        let file = studio.files[studio.next].clone();
        let item = commands
            .spawn((
                Name::new(format!("Picture of {file}")),
                WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(file))),
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
        studio.item = Some(item);
        studio.corners = None;
        studio.waited = 0;
        return;
    };
    if !ready.contains(item) {
        return;
    }
    studio.waited += 1;
    let held = is_held(&studio.files[studio.next]);
    if studio.corners.is_none() {
        // Bounds are worked out after the frame the look appears in.
        if studio.waited < 3 {
            return;
        }
        // The look stands unturned at the origin, so these are its own corners.
        let corners: Vec<Vec3> = children
            .iter_descendants(item)
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
            .collect();
        if corners.is_empty() {
            warn!(
                "pictures: {} has nothing to draw",
                studio.files[studio.next]
            );
        }
        let turn = if held { lay(&corners) } else { Quat::IDENTITY };
        commands.entity(item).insert(Transform::from_rotation(turn));
        studio.corners = Some(corners.into_iter().map(|corner| turn * corner).collect());
        studio.waited = 0;
        return;
    }
    if studio.waited == 1 {
        let corners = studio.corners.as_deref().unwrap_or_default();
        let rise = if held { 0.0 } else { WORN_RISE_DEGREES };
        if let Ok((mut transform, mut projection)) = camera.single_mut() {
            (*transform, *projection) = frame(corners, rise);
            for (mut at, light) in &mut lights {
                *at = shining(light, &transform);
            }
        }
    }
    if studio.waited < SETTLE {
        return;
    }
    studio.waiting = true;
    commands
        .spawn(Screenshot::image(studio.target.clone()))
        .observe(
            |captured: On<ScreenshotCaptured>,
             mut studio: ResMut<Studio>,
             mut commands: Commands| {
                let picture = match captured.image.clone().try_into_dynamic() {
                    Ok(picture) => picture.to_rgba8().into_raw(),
                    Err(error) => {
                        error!("pictures: could not read a capture: {error}");
                        vec![0; (DRAWN * DRAWN * 4) as usize]
                    }
                };
                studio.pictures.push(picture);
                studio.waiting = false;
                studio.next += 1;
                if let Some(item) = studio.item.take() {
                    commands.entity(item).despawn();
                }
            },
        );
}

/// How to turn a held look, handle along +Y, so it lies corner to corner with its working
/// end up and to the right and its broad side to the front (+Z): whichever of X and Z it
/// is thinner along is turned to face the camera.
fn lay(corners: &[Vec3]) -> Quat {
    let (low, high) = span(corners);
    let size = high - low;
    let broadside = if size.x < size.z {
        Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
    } else {
        Quat::IDENTITY
    };
    Quat::from_rotation_z(-std::f32::consts::FRAC_PI_4) * broadside
}

fn span(corners: &[Vec3]) -> (Vec3, Vec3) {
    corners.iter().fold(
        (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
        |(low, high), corner| (low.min(*corner), high.max(*corner)),
    )
}

/// The camera, from the front and `rise` degrees above, looking at the corners and
/// seeing all of them with a margin, without perspective.
fn frame(corners: &[Vec3], rise: f32) -> (Transform, Projection) {
    let (low, high) = span(corners);
    let middle = (low + high) * 0.5;
    let rise = rise.to_radians();
    let out = Vec3::new(0.0, libm::sinf(rise), libm::cosf(rise));
    let mut transform =
        Transform::from_translation(middle + out * 10.0).looking_at(middle, Vec3::Y);
    // Centre the picture on what is seen, not on the box's middle.
    let (right, up) = (*transform.right(), *transform.up());
    let (mut left, mut most_right, mut bottom, mut top) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for corner in corners {
        let offset = *corner - middle;
        let (x, y) = (offset.dot(right), offset.dot(up));
        (left, most_right, bottom, top) =
            (left.min(x), most_right.max(x), bottom.min(y), top.max(y));
    }
    transform.translation += right * (left + most_right) * 0.5 + up * (bottom + top) * 0.5;
    let side = (most_right - left).max(top - bottom).max(0.01) * MARGIN;
    let projection = Projection::Orthographic(OrthographicProjection {
        scaling_mode: ScalingMode::Fixed {
            width: side,
            height: side,
        },
        near: 0.0,
        far: 20.0,
        ..OrthographicProjection::default_3d()
    });
    (transform, projection)
}

/// Where a light shines from, by the camera: the sun from above, in front and to the
/// left; the edge light from behind and to the right.
fn shining(light: &PictureLight, camera: &Transform) -> Transform {
    let (back, up, right) = (*camera.back(), *camera.up(), *camera.right());
    let from = match light {
        PictureLight::Sun => back * 0.7 + up * 0.8 - right * 0.6,
        PictureLight::Edge => -back * 0.9 + up * 0.35 + right * 0.5,
    };
    Transform::IDENTITY.looking_to(-from.normalize(), Vec3::Y)
}

/// Lays the pictures out on empty slots, `COLUMNS` across, at each shown size.
fn save(pictures: &[Vec<u8>], prefix: &Path) {
    for (index, size) in SHOWN.into_iter().enumerate() {
        let path = if index == 0 {
            PathBuf::from(format!("{}.png", prefix.display()))
        } else {
            PathBuf::from(format!("{}-{size}.png", prefix.display()))
        };
        let gap = (size / 16).max(3) as usize;
        let size = size as usize;
        let columns = COLUMNS.min(pictures.len()).max(1);
        let rows = pictures.len().div_ceil(columns).max(1);
        let (width, height) = (columns * (size + gap) + gap, rows * (size + gap) + gap);
        let mut rgba: Vec<u8> = (0..width * height).flat_map(|_| bytes(GAP)).collect();
        for (at, picture) in pictures.iter().enumerate() {
            let (left, top) = (
                gap + (at % columns) * (size + gap),
                gap + (at / columns) * (size + gap),
            );
            let shrunk = shrink(picture, DRAWN as usize, size);
            for y in 0..size {
                for x in 0..size {
                    let from = (y * size + x) * 4;
                    let alpha = shrunk[from + 3];
                    let to = ((top + y) * width + left + x) * 4;
                    for c in 0..3 {
                        // The shrunk colour is already weighted by its coverage.
                        let value = shrunk[from + c] + SLOT[c] * (1.0 - alpha);
                        rgba[to + c] = (value.clamp(0.0, 1.0) * 255.0) as u8;
                    }
                }
            }
        }
        let image = Image::new(
            Extent3d {
                width: width as u32,
                height: height as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            rgba,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).ok();
        }
        match image.try_into_dynamic().map(|picture| picture.save(&path)) {
            Ok(Ok(())) => println!("SANDBOX saved {}", path.display()),
            Ok(Err(error)) => error!("pictures: could not save {}: {error}", path.display()),
            Err(error) => error!("pictures: could not make {}: {error}", path.display()),
        }
    }
}

fn bytes(colour: [f32; 3]) -> [u8; 4] {
    [
        (colour[0] * 255.0) as u8,
        (colour[1] * 255.0) as u8,
        (colour[2] * 255.0) as u8,
        255,
    ]
}

/// A square RGBA picture averaged down to `to` on a side, each channel 0 to 1, colour
/// weighted by coverage so edges blend with what is behind them.
fn shrink(rgba: &[u8], from: usize, to: usize) -> Vec<f32> {
    let step = from / to;
    let mut out = vec![0.0; to * to * 4];
    for y in 0..to {
        for x in 0..to {
            let mut sum = [0.0f32; 4];
            for dy in 0..step {
                for dx in 0..step {
                    let at = ((y * step + dy) * from + x * step + dx) * 4;
                    let alpha = f32::from(rgba[at + 3]) / 255.0;
                    for c in 0..3 {
                        sum[c] += f32::from(rgba[at + c]) / 255.0 * alpha;
                    }
                    sum[3] += alpha;
                }
            }
            let count = (step * step) as f32;
            for c in 0..4 {
                out[(y * to + x) * 4 + c] = sum[c] / count;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrinking_weights_colour_by_coverage() {
        // Two by two: one opaque white pixel, three clear, to one pixel a quarter covered.
        let rgba = [255, 255, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let out = shrink(&rgba, 2, 1);
        assert_eq!(out, vec![0.25, 0.25, 0.25, 0.25]);
    }

    #[test]
    fn a_frame_sees_every_corner() {
        let corners = [Vec3::new(-1.0, -0.2, -0.1), Vec3::new(1.0, 0.2, 0.1)];
        let (transform, projection) = frame(&corners, WORN_RISE_DEGREES);
        let Projection::Orthographic(projection) = projection else {
            panic!("pictures are drawn without perspective");
        };
        let ScalingMode::Fixed { width, .. } = projection.scaling_mode else {
            panic!("a picture's size is fixed");
        };
        for corner in corners {
            let offset = corner - transform.translation;
            assert!(offset.dot(*transform.right()).abs() < width * 0.5);
            assert!(offset.dot(*transform.up()).abs() < width * 0.5);
        }
    }

    #[test]
    fn a_held_look_lies_broadside_with_its_working_end_up_and_right() {
        // An axe head thin along X, as the held contract has it (edge toward +Z).
        let corners = [Vec3::new(-0.01, 0.0, -0.1), Vec3::new(0.01, 0.6, 0.1)];
        let turn = lay(&corners);
        let thin = turn * Vec3::X;
        assert!(
            thin.z.abs() > 0.99,
            "its thin side faces the camera: {thin}"
        );
        let working_end = turn * Vec3::Y;
        assert!(working_end.x > 0.6 && working_end.y > 0.6, "{working_end}");
    }
}
