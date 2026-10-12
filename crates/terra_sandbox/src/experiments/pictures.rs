//! A spike for D130, what item pictures look like: every held and worn look, drawn each
//! of the ways in `STYLES` from the look itself (D129), in the game's own light, laid out
//! as one sheet with a column per item and a row per style. `--sheet <prefix>` saves
//! `<prefix>.png` at 128 pixels a picture and `<prefix>-64.png` at 64, then exits.
//!
//! The camera draws into an image on a layer of its own, so the planet and its sun are not
//! in the picture; the look's own lights are on that layer too.

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

use crate::{character::Tint, shell::Options};

const LAYER: usize = 1;
/// Drawn at this size, then shrunk, which smooths the edges.
const DRAWN: u32 = 256;
const SHOWN: [u32; 2] = [128, 64];
/// Frames to let a new view and light settle before a capture.
const SETTLE: u32 = 6;
/// The empty slot behind a picture, and the gap between slots, as the play screens have them.
const SLOT: [f32; 3] = [0.12, 0.13, 0.16];
const GAP: [f32; 3] = [0.05, 0.06, 0.08];
/// A plate behind the picture: the play scene's sky, darkened, from zenith to horizon.
const PLATE_TOP: [f32; 3] = [0.13, 0.22, 0.38];
const PLATE_BOTTOM: [f32; 3] = [0.42, 0.34, 0.27];

/// One way of drawing a picture.
struct Style {
    /// Seen from a little above and to the side, rather than square on.
    above: bool,
    /// A cool light from behind, outlining the shape without a line.
    rim: bool,
    light: Light,
    plate: bool,
}

#[derive(Clone, Copy)]
enum Light {
    /// The play scene's: a warm sun, shade lit by a blue sky.
    World,
    /// A darker realm's, to see what a realm's own light would do: cold moon, violet shade.
    Gloom,
}

const STYLES: [Style; 5] = [
    Style {
        above: true,
        rim: false,
        light: Light::World,
        plate: false,
    },
    Style {
        above: true,
        rim: true,
        light: Light::World,
        plate: false,
    },
    Style {
        above: false,
        rim: true,
        light: Light::World,
        plate: false,
    },
    Style {
        above: true,
        rim: true,
        light: Light::World,
        plate: true,
    },
    Style {
        above: true,
        rim: true,
        light: Light::Gloom,
        plate: false,
    },
];

/// One picture's subject: a look, and how it is coloured.
struct Entry {
    file: String,
    /// Held things are laid across the picture, handle low and working end high; worn ones
    /// stand as worn.
    held: bool,
    tint: Tint,
}

#[derive(Resource)]
struct Studio {
    entries: Vec<Entry>,
    target: Handle<Image>,
    entry: usize,
    style: usize,
    item: Option<Entity>,
    /// The item's corners in the world, once it has loaded and been placed.
    corners: Option<Vec<Vec3>>,
    waited: u32,
    waiting: bool,
    /// Every picture so far, entry by entry, style by style, as RGBA.
    pictures: Vec<Vec<u8>>,
}

#[derive(Component)]
struct Key;
#[derive(Component)]
struct Rim;
#[derive(Component)]
struct Ready;

pub fn plugin(app: &mut App) {
    app.add_systems(PostStartup, setup).add_systems(Update, run);
}

/// Every held look, every worn look on one body, and a few tinted, sorted by file.
fn entries() -> Vec<Entry> {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let mut entries = Vec::new();
    for (folder, held) in [("models/held", true), ("models/gear/base-female", false)] {
        let mut files: Vec<String> = std::fs::read_dir(assets.join(folder))
            .into_iter()
            .flatten()
            .flatten()
            .map(|file| file.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".glb"))
            .collect();
        files.sort();
        entries.extend(files.into_iter().map(|name| Entry {
            file: format!("{folder}/{name}"),
            held,
            tint: Tint::default(),
        }));
    }
    let dyed = |file: &str, role: &str, colour: Color| Entry {
        file: file.to_string(),
        held: false,
        tint: Tint(vec![(role.to_string(), colour)]),
    };
    entries.push(dyed(
        "models/gear/base-female/leather_coat.glb",
        "Leather",
        Color::srgb(0.25, 0.35, 0.6),
    ));
    entries.push(dyed(
        "models/gear/base-female/leather_coat.glb",
        "Leather",
        Color::srgb(0.55, 0.18, 0.15),
    ));
    entries.push(dyed(
        "models/gear/base-female/small_pack.glb",
        "Pack",
        Color::srgb(0.4, 0.45, 0.22),
    ));
    entries
}

fn setup(
    mut commands: Commands,
    camera: Single<Entity, With<Camera3d>>,
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
    commands.spawn((
        Name::new("Picture key light"),
        Key,
        DirectionalLight {
            shadow_maps_enabled: true,
            ..default()
        },
        RenderLayers::layer(LAYER),
    ));
    commands.spawn((
        Name::new("Picture rim light"),
        Rim,
        DirectionalLight::default(),
        RenderLayers::layer(LAYER),
    ));
    commands.insert_resource(Studio {
        entries: entries(),
        target,
        entry: 0,
        style: 0,
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
    mut camera: Query<(&mut Transform, &mut Projection, Entity), With<Camera3d>>,
    mut key: Query<(&mut Transform, &mut DirectionalLight), (With<Key>, Without<Camera3d>)>,
    mut rim: Query<
        (&mut Transform, &mut DirectionalLight),
        (With<Rim>, Without<Key>, Without<Camera3d>),
    >,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut studio) = studio else { return };
    if studio.waiting {
        return;
    }
    if studio.entry >= studio.entries.len() {
        if let Some(prefix) = &options.sheet {
            save(&studio, prefix);
        }
        exit.write(AppExit::Success);
        return;
    }
    let Some(item) = studio.item else {
        let entry = &studio.entries[studio.entry];
        let lay = if entry.held {
            lay_across()
        } else {
            Quat::IDENTITY
        };
        let item = commands
            .spawn((
                Name::new(format!("Picture of {}", entry.file)),
                WorldAssetRoot(
                    assets.load(GltfAssetLabel::Scene(0).from_asset(entry.file.clone())),
                ),
                entry.tint.clone(),
                Transform::from_rotation(lay),
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
    if studio.corners.is_none() {
        // Bounds are worked out after the frame the look appears in.
        if studio.waited < 3 {
            return;
        }
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
                studio.entries[studio.entry].file
            );
        }
        studio.corners = Some(corners);
        studio.waited = 0;
    }
    let style = &STYLES[studio.style];
    if studio.waited == 1 {
        let corners = studio.corners.as_deref().unwrap_or_default();
        if let Ok((mut transform, mut projection, _)) = camera.single_mut() {
            (*transform, *projection) = frame(corners, style.above);
            let (key_light, rim_light) = lights(style, &transform);
            if let Ok((mut at, mut light)) = key.single_mut() {
                (*at, *light) = key_light;
            }
            if let Ok((mut at, mut light)) = rim.single_mut() {
                (*at, *light) = rim_light;
            }
            let entity = camera.single().map(|(_, _, entity)| entity).ok();
            if let Some(entity) = entity {
                commands.entity(entity).insert(ambient(style.light));
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
                studio.waited = 0;
                studio.style += 1;
                if studio.style == STYLES.len() {
                    studio.style = 0;
                    studio.entry += 1;
                    if let Some(item) = studio.item.take() {
                        commands.entity(item).despawn();
                    }
                }
            },
        );
}

/// Turns a held look, handle along +Y, so it lies corner to corner, working end up and to
/// the right, its broad side to the front.
fn lay_across() -> Quat {
    Quat::from_rotation_z(-std::f32::consts::FRAC_PI_4)
}

/// The camera, square on or from a little above and to the side, looking at the corners'
/// middle and seeing all of them with a margin, without perspective.
fn frame(corners: &[Vec3], above: bool) -> (Transform, Projection) {
    let (low, high) = corners.iter().fold(
        (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
        |(low, high), corner| (low.min(*corner), high.max(*corner)),
    );
    let middle = (low + high) * 0.5;
    let (turn, rise) = if above { (30f32, 25f32) } else { (0.0, 0.0) };
    let (turn, rise) = (turn.to_radians(), rise.to_radians());
    let out = Vec3::new(
        libm::sinf(turn) * libm::cosf(rise),
        libm::sinf(rise),
        libm::cosf(turn) * libm::cosf(rise),
    );
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
    let side = (most_right - left).max(top - bottom).max(0.01) * 1.15;
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

/// The key light from above, front and left of the camera, and the rim from behind.
fn lights(
    style: &Style,
    camera: &Transform,
) -> ((Transform, DirectionalLight), (Transform, DirectionalLight)) {
    let (back, up, right) = (*camera.back(), *camera.up(), *camera.right());
    let toward = |direction: Vec3| Transform::IDENTITY.looking_to(-direction.normalize(), Vec3::Y);
    let (key_colour, key_lux, rim_colour) = match style.light {
        Light::World => (
            Color::srgb(1.0, 0.87, 0.7),
            21_000.0,
            Color::srgb(0.85, 0.92, 1.0),
        ),
        Light::Gloom => (
            Color::srgb(0.62, 0.72, 1.0),
            9_000.0,
            Color::srgb(0.75, 0.55, 1.0),
        ),
    };
    let key = DirectionalLight {
        color: key_colour,
        illuminance: key_lux,
        shadow_maps_enabled: true,
        ..default()
    };
    let rim = DirectionalLight {
        color: rim_colour,
        illuminance: if style.rim { key_lux * 3.0 } else { 0.0 },
        ..default()
    };
    (
        (toward(back * 0.7 + up * 0.8 - right * 0.6), key),
        (toward(-back * 0.9 + up * 0.35 + right * 0.5), rim),
    )
}

/// Shade lit by the sky: blue in the play scene's world, violet in the gloom.
fn ambient(light: Light) -> AmbientLight {
    match light {
        Light::World => AmbientLight {
            color: Color::srgb(0.66, 0.76, 1.0),
            brightness: 1800.0,
            ..default()
        },
        Light::Gloom => AmbientLight {
            color: Color::srgb(0.55, 0.45, 0.8),
            brightness: 900.0,
            ..default()
        },
    }
}

/// Lays the pictures out, a column per entry and a row per style, at each shown size.
fn save(studio: &Studio, prefix: &Path) {
    for (index, size) in SHOWN.into_iter().enumerate() {
        let path = if index == 0 {
            PathBuf::from(format!("{}.png", prefix.display()))
        } else {
            PathBuf::from(format!("{}-{size}.png", prefix.display()))
        };
        let gap = (size / 16).max(3) as usize;
        let size = size as usize;
        let columns = studio.entries.len();
        let rows = STYLES.len();
        let (width, height) = (columns * (size + gap) + gap, rows * (size + gap) + gap);
        let mut rgba: Vec<u8> = (0..width * height).flat_map(|_| bytes(GAP)).collect();
        for (at, picture) in studio.pictures.iter().enumerate() {
            let (column, row) = (at / rows, at % rows);
            let (left, top) = (gap + column * (size + gap), gap + row * (size + gap));
            let shrunk = shrink(picture, DRAWN as usize, size);
            for y in 0..size {
                let behind = if STYLES[row].plate {
                    let along = y as f32 / size as f32;
                    std::array::from_fn(|c| PLATE_TOP[c] + (PLATE_BOTTOM[c] - PLATE_TOP[c]) * along)
                } else {
                    SLOT
                };
                for x in 0..size {
                    let from = (y * size + x) * 4;
                    let alpha = shrunk[from + 3];
                    let to = ((top + y) * width + left + x) * 4;
                    for c in 0..3 {
                        // The capture's colour is already weighted by its coverage.
                        let value = shrunk[from + c] + behind[c] * (1.0 - alpha);
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
        let (transform, projection) = frame(&corners, true);
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
}
