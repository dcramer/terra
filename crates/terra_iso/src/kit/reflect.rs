//! A biome's reflections (`reflect` in its file, iso-kit uc-reflect j1): the scene itself
//! captured into a cubemap from one point and lit back as the view's environment map, so
//! glass, a car's paint and glossy stone mirror the street's lamps, signs and fronts instead
//! of nothing. Without it the spike's PBR has no image-based light at all: a window's
//! specular sees black and reads as grey glass.
//!
//! - It is a baked reflection probe as shipped games make one (Unreal's Scene Capture Cube
//!   and its Sphere/Box Reflection Captures, Unity's baked Reflection Probe, Frostbite's
//!   local light probes): six 90-degree cameras at `at` render the scene once, the six faces
//!   are read back and stacked as a cube, and Bevy's own runtime filter
//!   (`GeneratedEnvironmentMapLight`, bevy_pbr light_probe/generate.rs: SPD mips, a GGX
//!   convolution a mip and a Lambertian one for the diffuse) makes the specular and diffuse
//!   maps from it. Bevy 0.20 cannot render a camera into a cube's face, hence the readback.
//! - The faces' order and turn are wgpu's cube convention under Bevy's sampling of
//!   (x, y, -z) (environment_map.wesl: "cube maps are left-handed"); `faces` says each.
//! - The capture cameras share the main camera's exposure, layers but the water's faces
//!   (their shader projects through the main view's mirror) and no post: a face stores the
//!   scene's radiance times that exposure, so the light's intensity is `gain` over it and
//!   the reflected lamps are as bright as the lamps.
//! - Timing: the cameras come up a third of the way to the still and are read three
//!   quarters of the way, so the scene's models are loaded and the new views' pipelines
//!   compiled; the cameras then go. A film or a still after that has the probe from its
//!   first frame.
//!
//! - `diffuse` splits the two (uc-reflect j2): Bevy scales an environment map's diffuse and
//!   specular by one intensity, so the capture is filtered twice, as is and lowered, and the
//!   view lights with the first's specular map and the second's diffuse.
//!
//! - `bounds` boxes it (uc-reflect j4): a Bevy `LightProbe` at `at`, parallax-corrected to
//!   its box, lights in place of the view.
//!
//! Next (the lane's handoff): several boxed probes placed by the biome file. Not
//! screen-space reflections: Bevy 0.20's are deferred-only and march a perspective depth.

use super::biome::Biome;
use super::light::{Light, Sky};
use crate::camera::MainCamera;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, Exposure, RenderTarget};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::diagnostic::FrameCount;
use bevy::light::{EnvironmentMapLight, GeneratedEnvironmentMapLight, LightProbe};
use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete, ReadbackOnce};
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
    TextureViewDimension,
};
use serde::Deserialize;

/// A biome's `reflect`: where its probe stands and how strongly it lights.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Reflections {
    /// The capture's point (m): where a reflection is seen from, about a car's roof over
    /// the middle of the place.
    pub at: (f32, f32, f32),
    /// The environment light's scale: 1 lights it back as bright as the scene was.
    #[serde(default = "one")]
    pub gain: f32,
    /// A face's side in texels, a power of two (Bevy's filter needs one).
    #[serde(default = "side")]
    pub size: u32,
    /// What a face shows where nothing is drawn; the light preset's sky fill if none.
    #[serde(default)]
    pub sky: Option<Sky>,
    /// The diffuse light's own scale, apart from the reflections' `gain` (none: `gain`
    /// lights both, as Bevy's environment map does), so a street's specular can mirror its
    /// lamps at full while its shade is lifted less (uc-reflect j1's risk: the pools of
    /// light flattened). Bevy scales an environment map's two maps by one intensity, so the
    /// capture is filtered twice, once as is for the specular and once times
    /// `diffuse / gain` for the diffuse, and the view takes a map from each (`join`).
    /// Not an irradiance volume: a whole-place one segfaulted the spike on the worker the
    /// frame after it came up, twice (uc-reflect j2).
    #[serde(default)]
    pub diffuse: Option<f32>,
    /// A parallax box (m, its full x, y, z, centred on `at`; uc-reflect j4): the capture
    /// lights through a Bevy `LightProbe` of that size instead of the view, and each
    /// reflection ray is traced to the box's walls before the cube is read (Bevy's
    /// `ParallaxCorrection::Auto`; Unreal's Box Reflection Capture, Lagarde and Zanuttini,
    /// "Local image-based lighting with parallax-corrected cubemaps", 2012), so a front or
    /// a lamp mirrors in the paving and the car's paint where it stands, not as if
    /// infinitely far. Outside the box nothing is lit by it. None: the view's map, unboxed.
    #[serde(default)]
    pub bounds: Option<(f32, f32, f32)>,
}

impl Reflections {
    /// The probe's transform when it is boxed: at `at`, scaled to the box (Bevy's light
    /// probe is a unit cube about its origin).
    pub fn boxed(&self) -> Option<Transform> {
        self.bounds.map(|(x, y, z)| {
            Transform::from_translation(Vec3::new(self.at.0, self.at.1, self.at.2))
                .with_scale(Vec3::new(x, y, z))
        })
    }
}

/// The boxed probe (`Reflections::bounds`), which lights in place of the view.
#[derive(Component)]
struct Probe;

/// A copy of a captured cube with every texel's colour times `k`.
pub fn scaled(cube: &Image, k: f32) -> Image {
    let mut copy = cube.clone();
    let s = cube.texture_descriptor.size;
    for layer in 0..s.depth_or_array_layers {
        for y in 0..s.height {
            for x in 0..s.width {
                let Ok(c) = cube.get_color_at_3d(x, y, layer) else {
                    continue;
                };
                let c = c.to_linear();
                let c = Color::linear_rgba(c.red * k, c.green * k, c.blue * k, c.alpha);
                let _ = copy.set_color_at_3d(x, y, layer, c);
            }
        }
    }
    copy
}

/// The two filterings of `diffuse` (`Reflections::diffuse`), each its own map's source.
#[derive(Component)]
struct Half {
    specular: bool,
}

fn one() -> f32 {
    1.0
}

fn side() -> u32 {
    256
}

/// The capture's state: the six faces' targets and what has been read back of each.
#[derive(Resource, Default)]
struct Capture {
    targets: Vec<Handle<Image>>,
    read: Vec<Option<Vec<u8>>>,
    done: bool,
}

/// A capture camera.
#[derive(Component)]
struct Face;

/// A face's readback, its index.
#[derive(Component)]
struct Reading(usize);

pub fn plugin(app: &mut App) {
    app.init_resource::<Capture>()
        .add_systems(Update, (place, read, join).chain());
}

/// With `diffuse`: the view's environment light from the two filterings' maps (the
/// specular of the capture as is, the diffuse of its lowered copy), once Bevy has made
/// them; their GPU filtering goes on writing into the same textures.
fn join(
    mut commands: Commands,
    biome: Option<Res<Biome>>,
    halves: Query<(&Half, &EnvironmentMapLight)>,
    main: Query<(Entity, Option<&Exposure>), (With<MainCamera>, Without<EnvironmentMapLight>)>,
    probe: Query<Entity, (With<Probe>, Without<EnvironmentMapLight>)>,
) {
    let Some(r) = biome.as_ref().and_then(|b| b.reflect) else {
        return;
    };
    let (mut specular_map, mut diffuse_map) = (None, None);
    for (half, light) in &halves {
        if half.specular {
            specular_map = Some(light.specular_map.clone());
        } else {
            diffuse_map = Some(light.diffuse_map.clone());
        }
    }
    let (Some(specular_map), Some(diffuse_map)) = (specular_map, diffuse_map) else {
        return;
    };
    let Ok((camera, exposure)) = main.single() else {
        return;
    };
    let k = exposure.copied().unwrap_or_default().exposure();
    let target = if r.bounds.is_some() {
        let Ok(p) = probe.single() else {
            return;
        };
        p
    } else {
        camera
    };
    commands.entity(target).insert(EnvironmentMapLight {
        diffuse_map,
        specular_map,
        intensity: r.gain / k,
        ..default()
    });
}

/// Each cube face's look direction and up, in wgpu's layer order (+X, -X, +Y, -Y, +Z, -Z)
/// as Bevy samples them: its lookup is (x, y, -z), so the +Z layer holds the world's -Z.
/// A camera's right is look x up, which is each face's u axis under that lookup.
pub fn faces() -> [(Vec3, Vec3); 6] {
    [
        (Vec3::X, Vec3::Y),
        (Vec3::NEG_X, Vec3::Y),
        (Vec3::Y, Vec3::Z),
        (Vec3::NEG_Y, Vec3::NEG_Z),
        (Vec3::NEG_Z, Vec3::Y),
        (Vec3::Z, Vec3::Y),
    ]
}

/// The frames the cameras come up and are read at, for a still at `frames`.
pub fn timing(frames: u32) -> (u32, u32) {
    (frames / 3, frames * 3 / 4)
}

#[allow(clippy::too_many_arguments)]
fn place(
    mut commands: Commands,
    frame: Res<FrameCount>,
    args: Res<crate::Args>,
    biome: Option<Res<Biome>>,
    light: Option<Res<Light>>,
    mut capture: ResMut<Capture>,
    mut images: ResMut<Assets<Image>>,
    main: Query<(Option<&Exposure>, Option<&RenderLayers>), With<MainCamera>>,
) {
    let Some(r) = biome.as_ref().and_then(|b| b.reflect) else {
        return;
    };
    let (up, read) = timing(args.frames);
    let Ok((exposure, layers)) = main.single() else {
        return;
    };
    if frame.0 == up && capture.targets.is_empty() {
        let exposure = exposure.copied().unwrap_or_default();
        let layers = layers
            .cloned()
            .unwrap_or_default()
            .without(super::water::LAYER);
        let sky = r.sky.or(light.map(|l| l.sky));
        let clear = sky.map_or(Vec3::ZERO, |s| {
            let c = s.colour.color().to_linear();
            Vec3::new(c.red, c.green, c.blue) * s.nits
        }) * exposure.exposure();
        let at = Vec3::new(r.at.0, r.at.1, r.at.2);
        for (i, (look, up)) in faces().into_iter().enumerate() {
            let mut image =
                Image::new_target_texture(r.size, r.size, TextureFormat::Rgba16Float, None);
            image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
            let image = images.add(image);
            capture.targets.push(image.clone());
            commands.spawn((
                Camera3d::default(),
                Camera {
                    order: -2 - i as isize,
                    clear_color: ClearColorConfig::Custom(Color::linear_rgb(
                        clear.x, clear.y, clear.z,
                    )),
                    ..default()
                },
                RenderTarget::Image(image.into()),
                Projection::from(PerspectiveProjection {
                    fov: core::f32::consts::FRAC_PI_2,
                    aspect_ratio: 1.0,
                    near: 0.05,
                    far: 200.0,
                    ..default()
                }),
                Transform::from_translation(at).looking_to(look, up),
                bevy::camera::Hdr,
                Msaa::Off,
                // The fight's decals and the kit's soft effects read the depth prepass.
                bevy::core_pipeline::prepass::DepthPrepass,
                Tonemapping::None,
                exposure,
                layers.clone(),
                Face,
            ));
        }
        capture.read = vec![None; 6];
    }
    if frame.0 == read && !capture.targets.is_empty() && !capture.done {
        for (i, t) in capture.targets.clone().into_iter().enumerate() {
            commands
                .spawn((Readback::texture(t), ReadbackOnce, Reading(i)))
                .observe(keep);
        }
    }
}

/// A face's bytes as they come back (once: `ReadbackOnce` reads it a single frame).
fn keep(event: On<ReadbackComplete>, readings: Query<&Reading>, mut capture: ResMut<Capture>) {
    if let Ok(Reading(i)) = readings.get(event.entity)
        && capture.read[*i].is_none()
    {
        capture.read[*i] = Some(event.data.clone());
    }
}

/// Once all six faces are back: the cube, on the main camera as its environment light, and
/// the capture's cameras gone.
fn read(
    mut commands: Commands,
    biome: Option<Res<Biome>>,
    mut capture: ResMut<Capture>,
    mut images: ResMut<Assets<Image>>,
    main: Query<(Entity, Option<&Exposure>), With<MainCamera>>,
    cameras: Query<Entity, Or<(With<Face>, With<Reading>)>>,
) {
    let Some(r) = biome.as_ref().and_then(|b| b.reflect) else {
        return;
    };
    if capture.done || capture.read.is_empty() || capture.read.iter().any(Option::is_none) {
        return;
    }
    let Ok((camera, exposure)) = main.single() else {
        return;
    };
    let bytes = (r.size * r.size * 8) as usize;
    let mut data = Vec::with_capacity(bytes * 6);
    for face in &capture.read {
        let face = face.as_ref().unwrap();
        if face.len() != bytes {
            warn!(
                "reflect: a face came back {} bytes, not {bytes} ({} texels square, Rgba16Float); \
                 the probe is off. Fix: a `size` whose row is a multiple of 256 bytes (32 or more, \
                 a power of two) in the biome's `reflect`",
                face.len(),
                r.size
            );
            capture.done = true;
            return;
        }
        data.extend_from_slice(face);
    }
    let mut cube = Image::new(
        Extent3d {
            width: r.size,
            height: r.size,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba16Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    cube.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    cube.texture_descriptor.usage |= TextureUsages::COPY_SRC | TextureUsages::TEXTURE_BINDING;
    let k = exposure.copied().unwrap_or_default().exposure();
    // Boxed: a light probe of its own lights in place of the view.
    let target = match r.boxed() {
        Some(at) => commands
            .spawn((
                Name::new("Reflection probe"),
                LightProbe::default(),
                at,
                Probe,
            ))
            .id(),
        None => camera,
    };
    match r.diffuse {
        // Two filterings on entities of their own (Bevy's generator makes an
        // `EnvironmentMapLight` on whatever holds the `GeneratedEnvironmentMapLight`, and only a
        // view or a `LightProbe` lights with one); `join` puts a map of each on the view.
        Some(d) if r.gain > 0.0 => {
            let lower = scaled(&cube, d / r.gain);
            for (specular, source) in [(true, cube), (false, lower)] {
                commands.spawn((
                    Name::new("Reflections' filtering"),
                    GeneratedEnvironmentMapLight {
                        environment_map: images.add(source),
                        ..default()
                    },
                    Half { specular },
                ));
            }
            info!("reflect: the diffuse apart, at {d} x");
        }
        _ => {
            commands
                .entity(target)
                .insert(GeneratedEnvironmentMapLight {
                    environment_map: images.add(cube),
                    intensity: r.gain / k,
                    ..default()
                });
        }
    }
    for c in &cameras {
        commands.entity(c).despawn();
    }
    capture.done = true;
    info!(
        "reflect: the probe at {:?} captured ({} texels a face), lit at {} x",
        r.at, r.size, r.gain
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each face's camera right (look x up, Bevy's looking_to) is the cube face's u axis and
    /// its down the v axis, for Bevy's lookup of a world direction d at (d.x, d.y, -d.z) in
    /// wgpu's convention (+X: u -z, v -y; -X: u +z; +Y: u +x, v +z; -Y: u +x, v -z;
    /// +Z: u +x, v -y; -Z: u -x, v -y), so a reflection is not mirrored.
    #[test]
    fn faces_follow_the_cube_convention() {
        // (major axis in lookup space, u and v axes in lookup space), wgpu's table.
        let table = [
            (Vec3::X, Vec3::NEG_Z, Vec3::NEG_Y),
            (Vec3::NEG_X, Vec3::Z, Vec3::NEG_Y),
            (Vec3::Y, Vec3::X, Vec3::Z),
            (Vec3::NEG_Y, Vec3::X, Vec3::NEG_Z),
            (Vec3::Z, Vec3::X, Vec3::NEG_Y),
            (Vec3::NEG_Z, Vec3::NEG_X, Vec3::NEG_Y),
        ];
        let lookup = |d: Vec3| Vec3::new(d.x, d.y, -d.z);
        for (i, (look, up)) in faces().into_iter().enumerate() {
            let t = Transform::default().looking_to(look, up);
            let (major, u, v) = table[i];
            assert!(
                lookup(look).distance(major) < 1e-6,
                "face {i} looks the wrong way"
            );
            assert!(
                lookup(*t.right()).distance(u) < 1e-6,
                "face {i} is mirrored in u"
            );
            assert!(
                lookup(*t.down()).distance(v) < 1e-6,
                "face {i} is flipped in v"
            );
        }
    }

    /// The cameras come up before they are read, and both before the still.
    #[test]
    fn the_capture_is_read_before_the_still() {
        for frames in [90, 300, 600] {
            let (up, read) = timing(frames);
            assert!(up < read && read < frames, "{frames}: {up} {read}");
            assert!(
                read - up >= 30,
                "{frames}: pipelines get {} frames",
                read - up
            );
        }
    }

    /// A biome's `reflect` parses with its defaults.
    #[test]
    fn a_biome_names_its_probe() {
        let r: Reflections =
            crate::kit::parse("(at: (0.0, 1.5, 0.0))", "r.ron", "reflect.rs").unwrap();
        assert_eq!((r.gain, r.size, r.sky, r.diffuse), (1.0, 256, None, None));
        assert_eq!(r.boxed(), None);
    }

    /// A boxed probe is Bevy's unit-cube light probe scaled to the box about the capture's
    /// point, so a point on the box's wall is half its size from `at`.
    #[test]
    fn a_boxed_probe_spans_its_box_about_the_capture() {
        let r: Reflections = crate::kit::parse(
            "(at: (1.0, 1.5, -0.5), bounds: (24.0, 16.0, 14.0))",
            "r.ron",
            "reflect.rs",
        )
        .unwrap();
        let t = r.boxed().unwrap();
        let corner = t.transform_point(Vec3::splat(0.5));
        assert!(
            (corner - Vec3::new(13.0, 9.5, 6.5)).length() < 1e-5,
            "{corner}"
        );
        assert_eq!(t.translation, Vec3::new(1.0, 1.5, -0.5));
    }

    /// A captured cube, each face one colour.
    fn cube(size: u32, faces: [f32; 6]) -> Image {
        let mut image = Image::new_fill(
            Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 6,
            },
            TextureDimension::D2,
            &[0; 8],
            TextureFormat::Rgba16Float,
            RenderAssetUsages::MAIN_WORLD,
        );
        for (layer, l) in faces.into_iter().enumerate() {
            for y in 0..size {
                for x in 0..size {
                    image
                        .set_color_at_3d(x, y, layer as u32, Color::linear_rgb(l, l, l))
                        .unwrap();
                }
            }
        }
        image
    }

    /// The lowered copy is every face's texels times the scale, the cube's shape kept, so
    /// the diffuse filtered from it is the capture's times `diffuse / gain`.
    #[test]
    fn the_lowered_copy_is_the_capture_times_the_scale() {
        let faces = [0.2, 0.4, 0.8, 1.0, 2.0, 4.0];
        let low = scaled(&cube(32, faces), 0.4);
        assert_eq!(low.texture_descriptor.size.depth_or_array_layers, 6);
        for (layer, l) in faces.into_iter().enumerate() {
            let c = low
                .get_color_at_3d(17, 9, layer as u32)
                .unwrap()
                .to_linear();
            assert!(
                (c.red - l * 0.4).abs() < 2e-3 * l,
                "face {layer}: {}",
                c.red
            );
        }
    }
}
