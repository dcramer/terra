//! Light from the whole sky (D14): the shade is lit by the sky above and by the sunlit
//! ground below, as it is outdoors, not by one flat colour from everywhere. A side facing
//! open sky is bright and cool, an underside takes the ground's warm or green bounce, and
//! shiny things reflect the sky. Shipped games do this with an environment map captured
//! from their sky; this paints a small one from the sky's colours, the sun and the
//! ground, and Bevy filters it into diffuse and specular light every frame
//! (`GeneratedEnvironmentMapLight`).

use crate::planet::{MainCamera, Tweaks};
use crate::sky::Sky;
use bevy::{
    asset::RenderAssetUsages,
    light::{EnvironmentMapLight, GeneratedEnvironmentMapLight},
    prelude::*,
    render::render_resource::{
        Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
    },
};

/// Pixels along each face of the painted sky: it is smooth, so small is enough.
const FACE: u32 = 32;

/// The light the sky gives the shade, which `daylight` sets for the hour.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct SkyLight {
    /// The colour the shade takes from the sky overhead, and how bright.
    pub shade: Color,
    pub brightness: f32,
}

impl Default for SkyLight {
    fn default() -> Self {
        Self {
            shade: Color::srgb(0.66, 0.76, 1.0),
            brightness: 1800.0,
        }
    }
}

/// The open sky's diffuse light toward each side, as the painted sky gives it, in the sky's own
/// frame (+Y up, east +X, south +Z): for each of +X, -X, +Y, -Y, +Z, -Z, what comes from above
/// the horizon and what from the ground below it, each as the environment map's diffuse light
/// is, the irradiance over pi, before its brightness. What hides some of the sky, as a
/// forest's crowns do, takes its share away (`sky_occlusion`).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct OpenSky {
    pub sky: [Vec3; 6],
    pub ground: [Vec3; 6],
    pub brightness: f32,
}

/// The painted sky, what it was last painted from, and for how many more frames Bevy is to
/// filter it into the light the shading reads.
#[derive(Resource)]
struct Painted {
    image: Handle<Image>,
    from: Option<[f32; 16]>,
    filtering: u32,
}

/// What snow lying round the camera gives back of the light, linear: the ground's bounce in the
/// sky's light where it lies, darker than its albedo, as what it lights faces it only in part
/// and the snow in shade is shaded too; at its full albedo, shaded snow was a quarter darker
/// than sunlit, against half in Sons of the Forest's winter.
const SNOW_BOUNCE: Vec3 = Vec3::new(0.45, 0.47, 0.52);

/// Bevy filters a generated environment map into its diffuse and specular light every frame
/// it is there, nearly two milliseconds of the GPU's time, though the sky is painted again only
/// a few times a minute. So it is there only for this many frames after each painting, enough
/// for the new sky to reach the GPU and be filtered; the filtered light stays on the camera
/// between (`EnvironmentMapLight`), and its brightness and turn are set on that directly.
const FILTER_FRAMES: u32 = 4;
/// For the first painting, longer, and counted only once the game is playing and no shader is
/// compiling: Bevy compiles the filter's pipelines in the background, and removed before they
/// ran, the light the shading reads is never filtered and stays black, so every shade was
/// black in a run whose sky did not change, a scripted shot of the kit sheet.
const FIRST_FILTER_FRAMES: u32 = 30;

/// How much of the open sky's reflection reaches an eye with so much sky open round it, over
/// it and to its sides (`SkySeen::around_at`, D336), in steps of a fiftieth so the light is
/// not changed every frame. Under a canopy a shiny thing mirrors leaves and stems, not sky:
/// the irradiance volume takes away the sky's diffuse light where the crowns hide it (D270),
/// but not its reflection, so leather, skin and stone under the trees mirrored the whole open sky and read lilac by a low sun. Shipped
/// engines occlude the reflection so (specular occlusion); this does it once, for where the
/// eye is, which is right for what stands near it. Beyond the volume the environment map gives
/// the diffuse light as well, and that dims with it while the eye is under the trees.
pub fn sky_reflected(open: f32) -> f32 {
    libm::roundf(open.clamp(0.0, 1.0) * 50.0) / 50.0
}

pub fn plugin(app: &mut App) {
    app.init_resource::<SkyLight>()
        .init_resource::<OpenSky>()
        // The sky's light replaces the flat ambient light.
        .insert_resource(GlobalAmbientLight {
            brightness: 0.0,
            ..default()
        })
        .add_systems(Startup, setup)
        .add_systems(PostStartup, light_camera)
        .add_systems(PostUpdate, paint);
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_fill(
        Extent3d {
            width: FACE,
            height: FACE,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        &[0; 16],
        TextureFormat::Rgba32Float,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    commands.insert_resource(Painted {
        image: images.add(image),
        from: None,
        filtering: FIRST_FILTER_FRAMES,
    });
}

fn light_camera(
    mut commands: Commands,
    camera: Single<Entity, With<MainCamera>>,
    painted: Res<Painted>,
    light: Res<SkyLight>,
) {
    commands
        .entity(*camera)
        .insert(GeneratedEnvironmentMapLight {
            environment_map: painted.image.clone(),
            intensity: light.brightness,
            ..default()
        });
}

/// Paints the sky again when its light has changed enough to see, and turns it so its
/// up is the camera's: the planet's centre is the world's origin.
fn paint(
    mut commands: Commands,
    light: Res<SkyLight>,
    sky: Res<Sky>,
    tweaks: Res<Tweaks>,
    mut painted: ResMut<Painted>,
    mut open: ResMut<OpenSky>,
    sky_seen: crate::sky_occlusion::SkySeen,
    snow: Option<Res<crate::climate::SnowCover>>,
    shaders: Option<Res<crate::loading::Shaders>>,
    screen: Option<Res<State<crate::loading::Screen>>>,
    mut images: ResMut<Assets<Image>>,
    mut camera: Query<
        (
            Entity,
            &GlobalTransform,
            Option<&mut GeneratedEnvironmentMapLight>,
            Option<&mut EnvironmentMapLight>,
        ),
        With<MainCamera>,
    >,
) {
    let Ok((entity, at, generated, filtered)) = camera.single_mut() else {
        return;
    };
    let up = at.translation().normalize_or(Vec3::Y);
    // Squared: the volume's coarse crowns leave about a third of the sky open round an eye in
    // the forest where the vantage views measure a few hundredths of it seen (D336); leather
    // still read lilac with the third.
    let around = sky_seen.around_at(at.translation());
    let intensity = light.brightness * sky_reflected(around * around);
    let rotation = Quat::from_rotation_arc(Vec3::Y, up);
    // Bevy makes the light the shading reads, `EnvironmentMapLight`, from the generated
    // one, and does not follow later changes to its brightness or turn, so they are given
    // to it as well; without this the shade kept the first frame's light all day.
    if let Some(mut filtered) = filtered {
        if filtered.intensity != intensity {
            filtered.intensity = intensity;
        }
        if filtered.rotation.angle_between(rotation) > 0.01 {
            filtered.rotation = rotation;
        }
    }
    let generating = generated.is_some();
    if let Some(mut generated) = generated {
        if generated.intensity != intensity {
            generated.intensity = intensity;
        }
        if generated.rotation.angle_between(rotation) > 0.01 {
            generated.rotation = rotation;
        }
    }
    // Counted only while the filter can run: playing, with nothing compiling.
    let ready = shaders.is_none_or(|shaders| shaders.idle())
        && screen.is_none_or(|screen| *screen.get() == crate::loading::Screen::Playing);
    if ready {
        painted.filtering = painted.filtering.saturating_sub(1);
    }

    let linear = |c: Color| Vec3::from_slice(&c.to_linear().to_f32_array()[..3]);
    let sun_height = tweaks.sun_direction.normalize_or(Vec3::Y).dot(up).max(0.0);
    // How low the sun is: 1 at 4° and below, 0 from 22° up.
    let low = 1.0 - ((sun_height - 0.07) / 0.31).clamp(0.0, 1.0);
    let low = low * low * (3.0 - 2.0 * low);
    let colours = Colours {
        low,
        shade: linear(light.shade),
        horizon: linear(sky.horizon),
        zenith: linear(sky.zenith),
        // Sunlit ground, as bright beside the sky as the sun makes it; under snow, the
        // snow's white (D327).
        ground: linear(tweaks.grass_lush)
            .lerp(linear(tweaks.ground_low), 0.4)
            .lerp(SNOW_BOUNCE, snow.map_or(0.0, |snow| snow.0))
            * linear(tweaks.sun_color)
            * (tweaks.sun_illuminance * sun_height
                / (std::f32::consts::PI * light.brightness.max(1.0))
                + 1.0),
    };
    if open.brightness != light.brightness {
        open.brightness = light.brightness;
    }
    let key = colours.key();
    // Repainted only when something moved by more than a shade: a few times a minute
    // while the sun crosses the sky.
    if painted
        .from
        .is_some_and(|from| from.iter().zip(key).all(|(a, b)| (a - b).abs() < 0.005))
    {
        // Unchanged: once the last painting is filtered, Bevy need not filter it again.
        if generating && painted.filtering == 0 {
            commands
                .entity(entity)
                .remove::<GeneratedEnvironmentMapLight>();
        }
        return;
    }
    // The first painting waits on Bevy compiling its filter, which takes many frames.
    painted.filtering = if painted.from.is_none() {
        FIRST_FILTER_FRAMES
    } else {
        FILTER_FRAMES
    };
    painted.from = Some(key);
    if !generating {
        commands
            .entity(entity)
            .insert(GeneratedEnvironmentMapLight {
                environment_map: painted.image.clone(),
                intensity,
                rotation,
                ..default()
            });
    }
    let (sky, ground) = colours.lobes();
    open.sky = sky;
    open.ground = ground;
    let Some(mut image) = images.get_mut(&painted.image) else {
        return;
    };
    image.data = Some(colours.cube());
}

/// What the sky is painted from, as linear colours.
#[derive(Clone, Copy)]
struct Colours {
    /// How low the sun is, 0 to 1.
    low: f32,
    shade: Vec3,
    horizon: Vec3,
    zenith: Vec3,
    ground: Vec3,
}

impl Colours {
    fn key(&self) -> [f32; 16] {
        let mut key = [0.0; 16];
        for (i, c) in [self.shade, self.horizon, self.zenith, self.ground]
            .iter()
            .enumerate()
        {
            key[i * 3..i * 3 + 3].copy_from_slice(&c.to_array());
        }
        key[12] = self.low;
        key
    }

    /// The light from a direction, +Y up. The sky takes its hue from the drawn sky, half
    /// way to the shade's colour, which the art chose, and is brighter toward the horizon
    /// as a clear sky is; below the horizon is the ground.
    fn toward(&self, d: Vec3) -> Vec3 {
        let luma = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722)).max(1e-4);
        let up = d.y;
        let hue = |c: Vec3| c / luma(c);
        let sky = |e: f32| {
            let drawn = hue(self.horizon.lerp(self.zenith, e.max(0.0).sqrt()));
            // While the sun is low the drawn horizon is orange and pink, and half of it washed
            // the whole forest salmon at dusk; the shade under trees is lit by the sky above
            // it, so it takes mostly the shade's cool colour then (D256).
            hue(drawn.lerp(hue(self.shade), 0.5 + 0.38 * self.low)) * (1.15 - 0.35 * e.max(0.0))
        };
        if up >= 0.0 {
            sky(up)
        } else {
            sky(0.0).lerp(self.ground, (-up / 0.15).clamp(0.0, 1.0))
        }
    }

    /// The diffuse light toward each of +X, -X, +Y, -Y, +Z, -Z, from the sky above the horizon
    /// and from the ground below it: the cosine-weighted sum of the light from every way,
    /// over pi, as Bevy filters the painted sky into its diffuse light.
    fn lobes(&self) -> ([Vec3; 6], [Vec3; 6]) {
        const N: usize = 48;
        let normals = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z];
        let (mut sky, mut ground) = ([Vec3::ZERO; 6], [Vec3::ZERO; 6]);
        // Directions spread evenly over the sphere: even in height, even round.
        for i in 0..N {
            let y = 1.0 - 2.0 * (i as f32 + 0.5) / N as f32;
            let across = (1.0 - y * y).max(0.0).sqrt();
            for j in 0..N * 2 {
                let round = (j as f32 + 0.5) / (N * 2) as f32 * std::f32::consts::TAU;
                let d = Vec3::new(across * libm::cosf(round), y, across * libm::sinf(round));
                let light = self.toward(d);
                for (k, n) in normals.iter().enumerate() {
                    // Each direction stands for 4 pi / (2 N squared) of the sphere.
                    let share = d.dot(*n).max(0.0) * 4.0 / (2 * N * N) as f32;
                    if y >= 0.0 {
                        sky[k] += light * share;
                    } else {
                        ground[k] += light * share;
                    }
                }
            }
        }
        (sky, ground)
    }

    /// The six faces of the cube, +X, -X, +Y, -Y, +Z, -Z, as RGBA floats.
    fn cube(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity((FACE * FACE * 6 * 16) as usize);
        for face in 0..6 {
            for y in 0..FACE {
                for x in 0..FACE {
                    let u = (x as f32 + 0.5) / FACE as f32 * 2.0 - 1.0;
                    let v = (y as f32 + 0.5) / FACE as f32 * 2.0 - 1.0;
                    let d = match face {
                        0 => Vec3::new(1.0, -v, -u),
                        1 => Vec3::new(-1.0, -v, u),
                        2 => Vec3::new(u, 1.0, v),
                        3 => Vec3::new(u, -1.0, -v),
                        4 => Vec3::new(u, -v, 1.0),
                        _ => Vec3::new(-u, -v, -1.0),
                    };
                    let c = self.toward(d.normalize());
                    for channel in [c.x, c.y, c.z, 1.0] {
                        data.extend_from_slice(&channel.to_le_bytes());
                    }
                }
            }
        }
        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn under_a_closed_canopy_nothing_mirrors_the_sky_and_in_the_open_all_of_it() {
        assert_eq!(sky_reflected(0.0), 0.0);
        assert_eq!(sky_reflected(1.0), 1.0);
        assert!(sky_reflected(0.3) < sky_reflected(0.6));
        // Small changes as the eye walks are not followed.
        assert_eq!(sky_reflected(0.401), sky_reflected(0.405));
    }

    fn colours() -> Colours {
        Colours {
            low: 0.0,
            shade: Vec3::new(0.4, 0.5, 1.0),
            horizon: Vec3::new(0.9, 0.85, 0.8),
            zenith: Vec3::new(0.1, 0.25, 0.7),
            ground: Vec3::new(0.3, 0.35, 0.1),
        }
    }

    #[test]
    fn the_sky_is_blue_above_and_the_ground_below() {
        let c = colours();
        let above = c.toward(Vec3::Y);
        assert!(above.z > above.x, "{above}");
        assert_eq!(c.toward(-Vec3::Y), c.ground);
    }

    #[test]
    fn a_clear_sky_is_brighter_at_the_horizon() {
        let c = colours();
        let luma = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
        assert!(luma(c.toward(Vec3::X)) > luma(c.toward(Vec3::Y)));
    }

    #[test]
    fn each_side_takes_a_cosine_weighted_share_of_what_it_faces() {
        let c = colours();
        let (sky, ground) = c.lobes();
        // Facing straight up sees only sky, and its light is a weighted mean of the sky's:
        // between the zenith's and the horizon's, the brighter.
        let luma = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
        assert!(luma(ground[2]) < 1e-6 && luma(sky[3]) < 1e-6);
        let (zenith, horizon) = (luma(c.toward(Vec3::Y)), luma(c.toward(Vec3::X)));
        let up = luma(sky[2]);
        assert!(
            up > zenith.min(horizon) * 0.98 && up < zenith.max(horizon) * 1.02,
            "{up}"
        );
        // Facing down sees only the ground, which is one colour: all of it.
        assert!(ground[3].distance(c.ground) < 0.02, "{}", ground[3]);
        // A side sees half sky and half ground.
        let side = sky[0] + ground[0];
        let half = (c.ground + c.toward(Vec3::X)) * 0.5;
        assert!(luma(side) > luma(half) * 0.7 && luma(side) < luma(half) * 1.3);
    }

    #[test]
    fn the_cube_has_the_sky_on_its_top_face_and_the_ground_on_its_bottom() {
        let c = colours();
        let cube = c.cube();
        // The pixel by the middle of a face, faces in the order +X, -X, +Y, -Y, +Z, -Z.
        let middle = |face: u32| {
            let pixel = (face * FACE + FACE / 2) * FACE + FACE / 2;
            let at = pixel as usize * 16;
            let float =
                |i: usize| f32::from_le_bytes(cube[at + i * 4..at + i * 4 + 4].try_into().unwrap());
            (Vec3::new(float(0), float(1), float(2)), float(3))
        };
        let (top, opaque) = middle(2);
        assert_eq!(opaque, 1.0);
        assert!(top.distance(c.toward(Vec3::Y)) < 0.01, "{top}");
        let (bottom, _) = middle(3);
        assert!(bottom.distance(c.ground) < 0.01, "{bottom}");
    }
}
