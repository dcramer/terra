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

/// What snow lying round the camera gives back of the sun's light, linear, darker than its
/// albedo, as the snow in the shade of what it lights is shaded too; at its full albedo,
/// shaded snow was a quarter darker than sunlit, against half in Sons of the Forest's winter.
const SNOW_BOUNCE: Vec3 = Vec3::new(0.45, 0.47, 0.52);
/// What it gives back of the sky's light: its albedo, as ground.wesl's SNOW, since the sky
/// lights the snow round a shaded thing as it lights the rest. At SNOW_BOUNCE, under a full
/// deck the forest's trunks took a third of the light from the snow below them that a side
/// facing snow of 0.8 takes (weather round 4).
const SNOW_ALBEDO: Vec3 = Vec3::new(0.85, 0.89, 0.89);

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
    nightfall: Option<Res<crate::daylight::Nightfall>>,
    dust: Option<Res<crate::air::AirDust>>,
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
    // The sun's light as it reaches the ground through the air, as Bevy's atmosphere gives it
    // on what the sun lights directly: the ground's bounce is of that light, dimmed and
    // warmed by a low sun, not of the sun's colour above the air.
    let through = crate::air::sun_through(libm::asinf(sun_height), dust.map_or(1.0, |d| d.0));
    let bare = linear(tweaks.grass_lush).lerp(linear(tweaks.ground_low), 0.4);
    let cover = snow.map_or(0.0, |snow| snow.0);
    let mut colours = Colours {
        low,
        day: 1.0 - nightfall.map_or(0.0, |n| n.0),
        shade: linear(light.shade),
        horizon: linear(sky.horizon),
        zenith: linear(sky.zenith),
        ground: Vec3::ZERO,
        overcast: sky.deck.x.clamp(0.0, 1.0),
    };
    // Sunlit ground, as bright beside the sky as the sun makes it, and lit by the sky too, in
    // the sky's colour (white, the grass bounced its own green into a low sun's shade); under
    // snow, the snow's white (D327).
    colours.ground = bare.lerp(SNOW_BOUNCE, cover)
        * (linear(tweaks.sun_color)
            * through
            * (tweaks.sun_illuminance * sun_height
                / (std::f32::consts::PI * light.brightness.max(1.0))))
        + bare.lerp(SNOW_ALBEDO, cover) * colours.overhead();
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

/// The colour of daylight of a correlated colour temperature, as the CIE's daylight locus
/// gives it (Judd, MacAdam and Wyszecki's from measured daylight and skylight, 1964; 4,000 to
/// 25,000 K), in linear sRGB with a luma of 1.
fn daylight(kelvin: f32) -> Vec3 {
    let t = kelvin.clamp(4_000.0, 25_000.0);
    let x = if t <= 7_000.0 {
        0.244063 + 0.09911e3 / t + 2.9678e6 / (t * t) - 4.6070e9 / (t * t * t)
    } else {
        0.237040 + 0.24748e3 / t + 1.9018e6 / (t * t) - 2.0064e9 / (t * t * t)
    };
    let y = -3.0 * x * x + 2.87 * x - 0.275;
    let (cx, cz) = (x / y, (1.0 - x - y) / y);
    let rgb = Vec3::new(
        3.2406 * cx - 1.5372 - 0.4986 * cz,
        -0.9689 * cx + 1.8758 + 0.0415 * cz,
        0.0557 * cx - 0.2040 + 1.0570 * cz,
    );
    rgb / rgb.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

/// A sky's colour moved onto the daylight locus, as warm or cool as it is (its blue over its
/// red) and as bright (its luma): real skylight lies along the locus, blue to white to the low
/// sun's amber, never off it toward magenta or green.
fn on_daylight_locus(c: Vec3) -> Vec3 {
    let luma = c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
    if luma <= 0.0 {
        return c;
    }
    let cool = c.z / c.x.max(1e-6);
    // By mired, in which the locus is nearly even: blue over red rises with the temperature.
    let (mut warm, mut cold) = (1e6 / 4_000.0, 1e6 / 25_000.0);
    for _ in 0..24 {
        let mid = 0.5 * (warm + cold);
        let d = daylight(1e6 / mid);
        if d.z / d.x < cool {
            warm = mid;
        } else {
            cold = mid;
        }
    }
    daylight(2e6 / (warm + cold)) * luma
}

/// The same for a colour given as sRGB.
pub(crate) fn srgb_on_daylight_locus(srgb: Vec3) -> Vec3 {
    let linear = Color::srgb(srgb.x, srgb.y, srgb.z).to_linear();
    let on = on_daylight_locus(Vec3::new(linear.red, linear.green, linear.blue));
    let back = Color::linear_rgb(on.x, on.y, on.z).to_srgba();
    Vec3::new(back.red, back.green, back.blue)
}

/// What the sky is painted from, as linear colours.
#[derive(Clone, Copy)]
struct Colours {
    /// How low the sun is, 0 to 1.
    low: f32,
    /// How much it is day, 1, and not night, 0: by night the moon's sky is its own colour.
    day: f32,
    shade: Vec3,
    horizon: Vec3,
    zenith: Vec3,
    ground: Vec3,
    /// How much of the sky an overcast deck covers (`Sky::deck`'s x, D418).
    overcast: f32,
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
        key[13] = self.day;
        key[14] = self.overcast;
        key
    }

    /// The light from a direction, +Y up. The sky takes its hue from the drawn sky, half
    /// way to the shade's colour, which the art chose, and is brighter toward the horizon
    /// as a clear sky is; below the horizon is the ground.
    fn toward(&self, d: Vec3) -> Vec3 {
        self.at_height(d.y)
    }

    /// The same by the height alone, the sine of the elevation: the painted sky is the same
    /// all round its up, so `lobes` and `cube` work each height out once (D489).
    fn at_height(&self, up: f32) -> Vec3 {
        let luma = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722)).max(1e-4);
        let hue = |c: Vec3| c / luma(c);
        let sky = |e: f32| {
            let drawn = hue(self.horizon.lerp(self.zenith, e.max(0.0).sqrt()));
            // While the sun is low the drawn horizon is orange and pink, and half of it washed
            // the whole forest salmon at dusk; the shade under trees is lit by the sky above
            // it, so it takes mostly the shade's cool colour then (D256).
            let mixed = hue(drawn.lerp(hue(self.shade), 0.5 + 0.38 * self.low));
            // By day on the daylight locus: mixed along a straight line from the horizon's orange
            // to the zenith's blue, it passed through magenta, and the shade it lit read lilac.
            // Not while the sun is low: twilight's sky lies off the locus toward violet, the
            // ozone's Chappuis band taking its orange (Hulburt 1953), and moved onto it the
            // shade keys' warm violet came out a neutral grey the green ground bounce tinted
            // (settlement r21).
            // Squared, so a dawn of 10 degrees and up (the forest's 07:30) keeps most of it.
            mixed.lerp(
                on_daylight_locus(mixed),
                self.day * (1.0 - self.low * self.low),
            ) * self.brightening(e.max(0.0))
        };
        if up >= 0.0 {
            sky(up)
        } else {
            sky(0.0).lerp(self.ground, (-up / 0.15).clamp(0.0, 1.0))
        }
    }

    /// How bright the sky is at a height `e` (the sine of its elevation) beside its mean. A
    /// clear sky is brighter toward the horizon; an overcast one is three times brighter
    /// overhead than at the horizon, the CIE's standard overcast sky (Moon and Spencer 1942,
    /// L = Lz (1 + 2 sin e) / 3), so under a deck what faces up is lit well over what faces
    /// out or down, where the deck was lit evenly and a snow load's top and underside took
    /// the same light (weather round 18). Scaled so open ground takes the clear sky's light.
    fn brightening(&self, e: f32) -> f32 {
        let clear = 1.15 - 0.35 * e;
        // The clear sky's cosine-weighted mean over the hemisphere, 1.15 - 0.35 * 2 / 3, over
        // the overcast law's, 7 / 9.
        let overcast = (1.15 - 0.35 * 2.0 / 3.0) * 9.0 / 7.0 * (1.0 + 2.0 * e) / 3.0;
        clear + (overcast - clear) * self.overcast
    }

    /// The hue of the light the sky sheds on open ground, at a luma of 1.
    /// The sky is the same all round, so the cosine-weighted sum over the upper hemisphere is
    /// one over its height: even in height is even in area, each height weighted by itself.
    fn overhead(&self) -> Vec3 {
        const N: usize = 16;
        let mut sum = Vec3::ZERO;
        for i in 0..N {
            let y = (i as f32 + 0.5) / N as f32;
            let across = (1.0 - y * y).max(0.0).sqrt();
            sum += self.toward(Vec3::new(across, y, 0.0)) * y;
        }
        sum / sum.dot(Vec3::new(0.2126, 0.7152, 0.0722)).max(1e-4)
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
            // The same all round: once a height, not once a direction, which was most of a
            // repaint's 7 ms on the main thread.
            let light = self.at_height(y);
            for j in 0..N * 2 {
                let round = (j as f32 + 0.5) / (N * 2) as f32 * std::f32::consts::TAU;
                let d = Vec3::new(across * libm::cosf(round), y, across * libm::sinf(round));
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
        // Each height once: the faces' pixels share a few hundred of them, by symmetry.
        let mut heights = std::collections::HashMap::<u32, Vec3>::new();
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
                    let up = d.normalize().y;
                    let c = *heights
                        .entry(up.to_bits())
                        .or_insert_with(|| self.at_height(up));
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
    fn under_an_overcast_deck_what_faces_up_is_lit_over_what_faces_out() {
        let luma = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
        let lit = |overcast: f32| {
            let mut c = colours();
            c.overcast = overcast;
            // The ground under snow, lit by the sky over it, as `paint` sets it.
            c.ground = SNOW_ALBEDO * c.overhead();
            let (sky, ground) = c.lobes();
            let side = luma(sky[0] + ground[0]);
            (luma(sky[2] + ground[2]), side, luma(sky[3] + ground[3]))
        };
        let (up, side, down) = lit(1.0);
        let (clear_up, clear_side, clear_down) = lit(0.0);
        // An overcast sky is three times brighter overhead than at its horizon, so a side,
        // which sees half the sky, mostly its low part, and half the snow, takes a sixth less
        // than what faces up (CIE: a wall takes 0.4 of the open ground's sky light); a clear
        // sky, brightest low, lit them nearly alike.
        assert!(up > 1.15 * side, "up {up} side {side}");
        assert!(
            up / side > clear_up / clear_side + 0.1,
            "{clear_up} {clear_side}"
        );
        assert!(up / down > clear_up / clear_down, "{clear_up} {clear_down}");
        // Open ground takes about the light it took under a clear sky.
        assert!((up - clear_up).abs() < 0.1 * clear_up, "{up} {clear_up}");
    }

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
            day: 1.0,
            shade: Vec3::new(0.4, 0.5, 1.0),
            horizon: Vec3::new(0.9, 0.85, 0.8),
            zenith: Vec3::new(0.1, 0.25, 0.7),
            ground: Vec3::new(0.3, 0.35, 0.1),
            overcast: 0.0,
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

    #[test]
    fn worked_out_once_a_height_the_painting_is_what_each_direction_gives() {
        // A low sun under some cloud, so every branch of `at_height` is taken.
        let mut c = colours();
        c.low = 0.4;
        c.overcast = 0.3;
        let cube = c.cube();
        let mut at = 0;
        for face in 0..6 {
            for y in 0..FACE {
                for x in 0..FACE {
                    let u = (x as f32 + 0.5) / FACE as f32 * 2.0 - 1.0;
                    let v = (y as f32 + 0.5) / FACE as f32 * 2.0 - 1.0;
                    let d = [
                        Vec3::new(1.0, -v, -u),
                        Vec3::new(-1.0, -v, u),
                        Vec3::new(u, 1.0, v),
                        Vec3::new(u, -1.0, -v),
                        Vec3::new(u, -v, 1.0),
                        Vec3::new(-u, -v, -1.0),
                    ][face];
                    let light = c.toward(d.normalize());
                    for channel in [light.x, light.y, light.z, 1.0] {
                        assert_eq!(
                            cube[at..at + 4],
                            channel.to_le_bytes(),
                            "face {face} {x},{y}"
                        );
                        at += 4;
                    }
                }
            }
        }
        // The lobes' sky light is the cosine-weighted sum of each direction's, by any order.
        let (sky, _) = c.lobes();
        let mut up = Vec3::ZERO;
        let n = 2000;
        for i in 0..n {
            let y = (i as f32 + 0.5) / n as f32;
            // Even in height is even in area; a side's cosine toward +Y is the height.
            up += c.toward(Vec3::new((1.0 - y * y).sqrt(), y, 0.0)) * y * 2.0 / n as f32;
        }
        assert!(sky[2].distance(up) < 0.01 * up.length(), "{} {up}", sky[2]);
    }

    #[test]
    fn daylight_of_6500_kelvin_is_srgb_white_and_the_locus_runs_amber_to_blue() {
        // sRGB's white is D65, CIE daylight of 6,504 K.
        let white = daylight(6_504.0);
        assert!((white - Vec3::ONE).abs().max_element() < 0.03, "{white}");
        let warm = daylight(4_000.0);
        let cold = daylight(15_000.0);
        assert!(warm.x > warm.y && warm.y > warm.z, "{warm}");
        assert!(cold.z > cold.y && cold.y > cold.x, "{cold}");
    }

    #[test]
    fn a_low_sun_keeps_the_shade_violet_and_the_ground_lit_by_the_sky_is_not_green() {
        // The settlement's dusk: an orange horizon, a violet shade key, grass on the ground.
        let mut c = Colours {
            low: 1.0,
            day: 1.0,
            shade: srgb_linear(0.76, 0.68, 0.92),
            horizon: srgb_linear(1.0, 0.8, 0.62),
            zenith: srgb_linear(0.35, 0.45, 0.75),
            ground: Vec3::ZERO,
            overcast: 0.0,
        };
        let sky = c.overhead();
        // Violet: red and blue both over green, as the keys give it; on the locus green sat
        // between them and the shade read grey.
        assert!(sky.y < sky.x && sky.y < sky.z, "{sky}");
        // Grass lit by that sky gives back a grey, not its own green.
        let grass = Vec3::new(0.05, 0.09, 0.03);
        c.ground = grass * sky;
        let lit = c.ground / c.ground.dot(Vec3::new(0.2126, 0.7152, 0.0722));
        let unlit = grass / grass.dot(Vec3::new(0.2126, 0.7152, 0.0722));
        assert!(lit.y < unlit.y, "{lit} {unlit}");
        // By day it stays on the locus.
        c.low = 0.0;
        let day = c.overhead();
        assert!(day.y >= day.x.min(day.z), "{day}");
    }

    #[test]
    fn a_colour_on_the_locus_is_never_magenta_and_keeps_its_brightness() {
        let luma = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
        let lilac = Vec3::new(0.45, 0.4, 0.8);
        let on = on_daylight_locus(lilac);
        // Daylight's green lies between its red and its blue; lilac's is under both.
        assert!(on.y >= on.x.min(on.z), "{on}");
        assert!((luma(on) - luma(lilac)).abs() < 1e-3, "{on}");
        // As cool as it was: blue over red kept within the locus's reach.
        assert!(on.z > on.x, "{on}");
    }

    fn srgb_linear(r: f32, g: f32, b: f32) -> Vec3 {
        Vec3::from_slice(&Color::srgb(r, g, b).to_linear().to_f32_array()[..3])
    }

    #[test]
    fn above_twilight_an_orange_and_lilac_sky_lights_the_shade_daylight_not_lilac() {
        // An orange horizon, a blue zenith and a lilac shade colour, with the sun above
        // twilight; every way up into the sky and the light toward each side is daylight.
        let c = Colours {
            low: 0.0,
            day: 1.0,
            shade: srgb_linear(0.68, 0.66, 0.9),
            horizon: srgb_linear(0.95, 0.55, 0.38),
            zenith: srgb_linear(0.16, 0.24, 0.52),
            ground: Vec3::new(0.1, 0.1, 0.08),
            overcast: 0.0,
        };
        for d in [Vec3::Y, Vec3::new(1.0, 0.05, 0.0), Vec3::new(1.0, 0.5, 1.0)] {
            let light = c.toward(d.normalize());
            assert!(light.y >= light.x.min(light.z) * 0.999, "{d}: {light}");
        }
        let (sky, _) = c.lobes();
        for side in sky {
            assert!(side.y >= side.x.min(side.z) * 0.999, "{side}");
        }
    }
}
