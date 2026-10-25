//! Light from the whole sky (D14): the shade is lit by the sky above and by the sunlit
//! ground below, as it is outdoors, not by one flat colour from everywhere. A side facing
//! open sky is bright and cool, an underside takes the ground's warm or green bounce, and
//! shiny things reflect the sky. Shipped games do this with an environment map captured
//! from their sky; this paints a small one from the sky's colours, the sun and the
//! ground, brighter and warmer toward the sun as the drawn sky is (D490); its diffuse light is
//! its spherical harmonics, worked out here (D492), and Bevy filters it into the specular
//! light (`GeneratedEnvironmentMapLight`).

mod harmonics;

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
/// Texels along each face of its diffuse light, smoother still: three bands of harmonics.
const DIFFUSE: u32 = 16;

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

/// The open sky's diffuse light toward each side, as the painted sky gives it, in the
/// painting's frame (+Y up, turned into the world by `frame`): for each of +X, -X, +Y, -Y,
/// +Z, -Z, what comes from above the horizon and what from the ground below it, each as the
/// environment map's diffuse light is, the irradiance over pi, before its brightness. The sky
/// is brighter toward the sun (D490), so the sides differ round as well as up and down. What
/// hides some of the sky, as a forest's crowns do, takes its share away (`sky_occlusion`).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct OpenSky {
    pub sky: [Vec3; 6],
    pub ground: [Vec3; 6],
    pub brightness: f32,
    /// The turn from the painting's frame into the world's, as the environment map's.
    pub frame: Quat,
}

/// The painted sky, what it was last painted from, and for how many more frames Bevy is to
/// filter it into the light the shading reads.
#[derive(Resource)]
struct Painted {
    image: Handle<Image>,
    /// Its diffuse light, filled here, not by Bevy's filter (D492).
    diffuse: Handle<Image>,
    from: Option<[f32; 22]>,
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

/// Bevy filters a generated environment map into its specular light every frame it is there
/// (its diffuse light is ours, D492), a third of a millisecond of the GPU's time, though the sky
/// is painted again only a few times a minute. So it is there only for the frame of each
/// painting (D489): `paint` runs before the images' change events are sent, so that frame
/// extracts the new painting, writes it into the same texture before the filter is encoded
/// (`GpuImage::prepare_asset` reuses it) and filters it, every painting of a walk measured so; the
/// filtered light stays on the camera between (`EnvironmentMapLight`), and its brightness and
/// turn are set on that directly. Were the painting's upload held back by the cap
/// (`uploads`), the reflection would lag a painting, a shade, until the next.
const FILTER_FRAMES: u32 = 1;
/// For the first painting, longer, and counted only once the game is playing and no shader is
/// compiling: Bevy compiles the filter's pipelines in the background, and removed before they
/// ran, the light the shading reads is never filtered and stays black, so every shade was
/// black in a run whose sky did not change, a scripted shot of the kit sheet. So the filter
/// runs through all of the loading screen, some 300 frames, at no cost to play.
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
        // Before the images' change events are sent, so a painting is extracted, uploaded and
        // filtered in the frame it is made (as bevy_animation orders its graphs).
        .add_systems(PostUpdate, paint.before(bevy::asset::AssetEventSystems));
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
    // Not a storage texture, so Bevy's filter leaves it as it is (patches/bevy_pbr, D492).
    let mut diffuse = Image::new_fill(
        Extent3d {
            width: DIFFUSE,
            height: DIFFUSE,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        &[0; 8],
        TextureFormat::Rgba16Float,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    diffuse.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    commands.insert_resource(Painted {
        image: images.add(image),
        diffuse: images.add(diffuse),
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
        // Bevy makes its own diffuse map to filter into; the shading reads ours instead.
        if filtered.diffuse_map != painted.diffuse {
            filtered.diffuse_map = painted.diffuse.clone();
        }
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
        sun: rotation.inverse() * tweaks.sun_direction.normalize_or(Vec3::Y),
        glow: linear(sky.glow),
    };
    let lit = colours.lit();
    // Sunlit ground, as bright beside the sky as the sun makes it, and lit by the sky too, in
    // the sky's colour (white, the grass bounced its own green into a low sun's shade); under
    // snow, the snow's white (D327).
    colours.ground = bare.lerp(SNOW_BOUNCE, cover)
        * (linear(tweaks.sun_color)
            * through
            * (tweaks.sun_illuminance * sun_height
                / (std::f32::consts::PI * light.brightness.max(1.0))))
        + bare.lerp(SNOW_ALBEDO, cover) * colours.overhead(&lit);
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
    let painting = colours.painting(&lit);
    (open.sky, open.ground) = painting.lobes();
    open.frame = rotation;
    if let Some(mut image) = images.get_mut(&painted.diffuse) {
        image.data = Some(painting.diffuse());
    }
    let Some(mut image) = images.get_mut(&painted.image) else {
        return;
    };
    image.data = Some(painting.cube);
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

/// The CIE's standard clear sky, type 12 of its standard general skies (CIE S 011/E:2003,
/// ISO 15469:2004; Kittler, Perez and Darula 1998, from Perez, Seals and Michalsky's
/// all-weather sky, 1993): a sky element's luminance goes as f(chi) phi(Z), chi its angle from
/// the sun and Z from the zenith, by the gradation phi(Z) = 1 + a exp(b / cos Z) and the
/// scattering indicatrix f(chi) = 1 + c (exp(d chi) - exp(d pi / 2)) + e cos^2 chi, whose c term
/// is the aureole round the sun. a, b, c, d and e as the standard's table gives them (checked
/// against colour-science's `cie2003.py`).
const CLEAR_SKY: [f32; 5] = [-1.0, -0.32, 10.0, -3.0, 0.45];

/// The clear sky's gradation at a height (the cosine of the zenith angle): darker overhead.
fn gradation(e: f32) -> f32 {
    let [a, b, ..] = CLEAR_SKY;
    if e > 1e-3 {
        1.0 + a * libm::expf(b / e)
    } else {
        1.0
    }
}

/// The clear sky's indicatrix toward a way at so much of a cosine from the sun, and the part of
/// it that is the aureole, the sunlight the air's haze scatters forward.
fn indicatrix(toward_sun: f32) -> (f32, f32) {
    let [_, _, c, d, e] = CLEAR_SKY;
    let chi = libm::acosf(toward_sun.clamp(-1.0, 1.0));
    let aureole = c * (libm::expf(d * chi) - libm::expf(d * std::f32::consts::FRAC_PI_2));
    (
        1.0 + aureole + e * toward_sun * toward_sun,
        aureole.max(0.0),
    )
}

/// The old clear sky's brightness at a height, the same all round: what the night's sky keeps,
/// and the mean the CIE's clear sky is scaled to.
fn even_clear(e: f32) -> f32 {
    1.15 - 0.35 * e
}

/// Its cosine-weighted mean over the upper hemisphere: 1.15 - 0.35 * 2 / 3.
const CLEAR_MEAN: f32 = 1.15 - 0.35 * 2.0 / 3.0;

/// The way in the painting's frame that a texel of a cube's face, +X, -X, +Y, -Y, +Z, -Z, is
/// read for, not of unit length. The faces are laid as the GPU's cube maps are (the Vulkan and
/// WebGPU face selection), but Bevy reads them left-handed: it turns the world's way into the
/// painting's frame and negates z before it samples (`environment_map.wesl`'s
/// `compute_cubemap_sample_dir` and its diffuse light alike), as its own atmosphere's cube
/// negates z when it paints (`atmosphere/environment.wesl`). So the texel the GPU finds at
/// (x, y, z) is read for (x, y, -z); painted for (x, y, z), the sky was mirrored north to south
/// and the sun's warm side lit the shade facing away from it.
fn texel_direction(face: u32, x: u32, y: u32, size: u32) -> Vec3 {
    let u = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
    let v = (y as f32 + 0.5) / size as f32 * 2.0 - 1.0;
    let sampled = match face {
        0 => Vec3::new(1.0, -v, -u),
        1 => Vec3::new(-1.0, -v, u),
        2 => Vec3::new(u, 1.0, v),
        3 => Vec3::new(u, -1.0, -v),
        4 => Vec3::new(u, -v, 1.0),
        _ => Vec3::new(-u, -v, -1.0),
    };
    sampled * Vec3::new(1.0, 1.0, -1.0)
}

fn luma(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

/// What the sky is painted from, as linear colours, in the painting's frame (+Y up).
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
    /// The way to the sun, of unit length, below the horizon too.
    sun: Vec3,
    /// The colour of the glow round the sun (`Sky::glow`).
    glow: Vec3,
}

/// What a painting works out once from its colours: how far the CIE's clear sky is scaled to
/// light open ground as the old even sky did, the glow's hue by its logarithm, and how much
/// of the aureole takes it.
struct Lit {
    clear: f32,
    glow: Vec3,
    warmth: f32,
}

/// The painted sky (D490, D492): the cube Bevy filters into the specular light, and the
/// spherical harmonics of what is above the horizon and what below, from which the diffuse
/// light is read.
struct Painting {
    cube: Vec<u8>,
    sky: harmonics::Harmonics,
    ground: harmonics::Harmonics,
}

impl Colours {
    fn key(&self) -> [f32; 22] {
        let mut key = [0.0; 22];
        for (i, c) in [
            self.shade,
            self.horizon,
            self.zenith,
            self.ground,
            self.glow,
        ]
        .iter()
        .enumerate()
        {
            key[i * 3..i * 3 + 3].copy_from_slice(&c.to_array());
        }
        // The sun's way by half, so it is followed to about half a degree: its aureole is
        // tens of degrees wide.
        key[15..18].copy_from_slice(&(self.sun * 0.5).to_array());
        key[18] = self.low;
        key[19] = self.day;
        key[20] = self.overcast;
        key
    }

    fn lit(&self) -> Lit {
        // The clear sky's cosine-weighted mean, even in height (even in area) and round.
        const N: usize = 24;
        let mut sum = 0.0;
        for i in 0..N {
            let y = (i as f32 + 0.5) / N as f32;
            let across = (1.0 - y * y).max(0.0).sqrt();
            for j in 0..N * 2 {
                let round = (j as f32 + 0.5) / (N * 2) as f32 * std::f32::consts::TAU;
                let d = Vec3::new(across * libm::cosf(round), y, across * libm::sinf(round));
                sum += indicatrix(d.dot(self.sun)).0 * gradation(y) * y;
            }
        }
        let mean = sum / (N * N) as f32;
        // By day on the daylight locus, as the sky's hue is: the low sun's orange glow,
        // mixed with a day's blue, was a little magenta.
        let glow = self.glow.max(Vec3::splat(1e-4));
        let glow = glow / luma(glow);
        let glow = glow.lerp(
            on_daylight_locus(glow),
            self.day * (1.0 - self.low * self.low),
        );
        Lit {
            clear: CLEAR_MEAN / mean.max(1e-6),
            glow: glow.max(Vec3::splat(1e-4)).map(libm::logf),
            // A low sun's glow is the drawn sky's colour as its orange horizon is, and the
            // shade takes mostly the shade's cool colour then (D256): a quarter of it.
            warmth: 1.0 - 0.76 * self.low,
        }
    }

    /// The sky's hue at a height `e`, the sine of its elevation, at a luma of 1, and its
    /// logarithm: the same all round, so a painting works each height out once (D489). It
    /// takes its hue from the drawn sky, half way to the shade's colour, which the art chose.
    fn hue(&self, e: f32) -> (Vec3, Vec3) {
        let hue = |c: Vec3| c / luma(c).max(1e-4);
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
        let c = mixed.lerp(
            on_daylight_locus(mixed),
            self.day * (1.0 - self.low * self.low),
        );
        (c, c.max(Vec3::splat(1e-4)).map(libm::logf))
    }

    /// The light from a way `d` of unit length, given the sky's hue at its height, or at the
    /// horizon below it. The sky is as bright round it as the CIE's clear sky by day,
    /// brightest round the sun and toward the horizon, and the share of it that is the
    /// aureole, sunlight scattered forward by the haze, takes the glow's colour, so a side
    /// facing the sun's half of the sky takes more light and warmer (D490). An overcast sky is
    /// three times brighter overhead than at the horizon, the CIE's standard overcast sky
    /// (Moon and Spencer 1942, L = Lz (1 + 2 sin e) / 3), so under a deck what faces up is lit
    /// well over what faces out or down, where the deck was lit evenly and a snow load's top
    /// and underside took the same light (weather round 18). Scaled so open ground takes the
    /// light the even clear sky gave it. Below the horizon is the ground.
    fn light(&self, lit: &Lit, d: Vec3, (hue, ln): (Vec3, Vec3)) -> Vec3 {
        let sky = |d: Vec3| {
            let e = d.y.max(0.0);
            let (f, aureole) = indicatrix(d.dot(self.sun));
            let phi = gradation(e);
            let clear = even_clear(e) + (lit.clear * f * phi - even_clear(e)) * self.day;
            // The even clear sky's mean over the overcast law's, 7 / 9.
            let overcast = CLEAR_MEAN * 9.0 / 7.0 * (1.0 + 2.0 * e) / 3.0;
            let luminance = clear + (overcast - clear) * self.overcast;
            let sunward = (1.0 - self.overcast) * self.day * lit.clear * aureole * phi
                / luminance.max(1e-6)
                * lit.warmth;
            if sunward < 1e-4 {
                return hue * luminance;
            }
            // Mixed by the logarithm, which keeps a blue sky warmed toward an orange glow off
            // magenta, as daylight's locus nearly is a line in it.
            let warmed = ln.lerp(lit.glow, sunward).map(libm::expf);
            warmed / luma(warmed) * luminance
        };
        if d.y >= 0.0 {
            return sky(d);
        }
        let under = (-d.y / 0.15).clamp(0.0, 1.0);
        if under >= 1.0 {
            return self.ground;
        }
        let level = Vec3::new(d.x, 0.0, d.z).normalize_or(Vec3::X);
        sky(level).lerp(self.ground, under)
    }

    /// The hue of the light the sky sheds on open ground, at a luma of 1: the cosine-weighted
    /// sum over the upper hemisphere, even in height (even in area) and round.
    fn overhead(&self, lit: &Lit) -> Vec3 {
        const N: usize = 16;
        let mut sum = Vec3::ZERO;
        for i in 0..N {
            let y = (i as f32 + 0.5) / N as f32;
            let across = (1.0 - y * y).max(0.0).sqrt();
            let hue = self.hue(y);
            for j in 0..N * 2 {
                let round = (j as f32 + 0.5) / (N * 2) as f32 * std::f32::consts::TAU;
                let d = Vec3::new(across * libm::cosf(round), y, across * libm::sinf(round));
                sum += self.light(lit, d, hue) * y;
            }
        }
        sum / luma(sum).max(1e-4)
    }

    /// Paints the cube's six faces, +X, -X, +Y, -Y, +Z, -Z, as RGBA floats, each texel the
    /// light of the way Bevy reads it for (`texel_direction`), and projects that light onto the
    /// harmonics along the same way by the solid angle it covers.
    fn painting(&self, lit: &Lit) -> Painting {
        let angles = harmonics::texel_solid_angles(FACE);
        let mut cube = Vec::with_capacity((FACE * FACE * 6 * 16) as usize);
        let (mut sky, mut ground) = Default::default();
        // Each height's hue once: the faces' pixels share a few hundred of them, by symmetry.
        let mut heights = std::collections::HashMap::<u32, (Vec3, Vec3)>::new();
        for face in 0..6 {
            for y in 0..FACE {
                for x in 0..FACE {
                    let d = texel_direction(face, x, y, FACE).normalize();
                    let e = d.y.max(0.0);
                    let hue = *heights.entry(e.to_bits()).or_insert_with(|| self.hue(e));
                    let c = self.light(lit, d, hue);
                    let angle = angles[(y * FACE + x) as usize];
                    let part: &mut harmonics::Harmonics =
                        if d.y >= 0.0 { &mut sky } else { &mut ground };
                    part.add(d, c, angle);
                    for channel in [c.x, c.y, c.z, 1.0] {
                        cube.extend_from_slice(&channel.to_le_bytes());
                    }
                }
            }
        }
        Painting { cube, sky, ground }
    }
}

/// The open sky of a clear day with the sun at `sun` in the world, the painting turned into
/// the world by `frame`, as `paint` leaves it: for the irradiance volume's tests.
#[cfg(test)]
pub(crate) fn clear_sky(sun: Vec3, frame: Quat) -> OpenSky {
    let c = Colours {
        low: 0.6,
        day: 1.0,
        shade: Vec3::new(0.4, 0.5, 1.0),
        horizon: Vec3::new(0.9, 0.85, 0.8),
        zenith: Vec3::new(0.1, 0.25, 0.7),
        ground: Vec3::new(0.3, 0.35, 0.1),
        overcast: 0.0,
        sun: frame.inverse() * sun.normalize(),
        glow: Vec3::new(1.0, 0.5, 0.18),
    };
    let (sky, ground) = c.painting(&c.lit()).lobes();
    OpenSky {
        sky,
        ground,
        brightness: 1.0,
        frame,
    }
}

impl Painting {
    /// The diffuse light toward each of +X, -X, +Y, -Y, +Z, -Z, from the sky above the horizon
    /// and from the ground below it, as the environment map's diffuse light is.
    fn lobes(&self) -> ([Vec3; 6], [Vec3; 6]) {
        let normals = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z];
        (
            normals.map(|n| self.sky.diffuse(n)),
            normals.map(|n| self.ground.diffuse(n)),
        )
    }

    /// The environment map's diffuse light, a cube of `DIFFUSE` texels a side as RGBA half
    /// floats: what Bevy's filter worked out on the GPU by 1,024 random samples a texel.
    fn diffuse(&self) -> Vec<u8> {
        let all = self.sky.plus(&self.ground);
        let mut data = Vec::with_capacity((DIFFUSE * DIFFUSE * 6 * 8) as usize);
        for face in 0..6 {
            for y in 0..DIFFUSE {
                for x in 0..DIFFUSE {
                    let c = all.diffuse(texel_direction(face, x, y, DIFFUSE).normalize());
                    for channel in [c.x, c.y, c.z, 1.0] {
                        data.extend_from_slice(&half::f16::from_f32(channel).to_le_bytes());
                    }
                }
            }
        }
        data
    }
}

#[cfg(test)]
mod tests;
