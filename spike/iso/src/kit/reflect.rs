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
//! Next (the lane's handoff): several probes placed by the biome file as Bevy `LightProbe`s
//! with parallax boxes, then screen-space reflections if they work at the ortho camera.

use super::biome::Biome;
use super::light::{Light, Sky};
use crate::camera::MainCamera;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, Exposure, RenderTarget};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::diagnostic::FrameCount;
use bevy::light::GeneratedEnvironmentMapLight;
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

/// A capture camera, its face's index.
#[derive(Component)]
struct Face(usize);

/// A face's readback, its index.
#[derive(Component)]
struct Reading(usize);

pub fn plugin(app: &mut App) {
    app.init_resource::<Capture>()
        .add_systems(Update, (place, read).chain());
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
                Face(i),
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
    commands
        .entity(camera)
        .insert(GeneratedEnvironmentMapLight {
            environment_map: images.add(cube),
            intensity: r.gain / k,
            ..default()
        });
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
        assert_eq!((r.gain, r.size, r.sky), (1.0, 256, None));
    }
}
