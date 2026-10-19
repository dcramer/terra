//! The fires' light, by docs/fire.md's light rules (D415): one flicker per fire, the same
//! signal driving the light's power, the flame's and embers' brightness and the char's glow;
//! the light swaying on three axes by it, so the shadows it casts move as a real flame's do;
//! shadows from the nearest few fire lights, the kept fires' among them glowing in a fog volume
//! of their own (`fog`); brighter by night than by day (`by_night`); and a
//! dozen lights however much burns, gathered over cells, chosen by what the eye is near and
//! eased out by 40 m (D127).
//!
//! The flicker is a product of three incommensurate sines (Valheim's ratios, 1, 0.564 and
//! 0.758) plus a low value noise, as fast and as deep as a fire that size puffs: a pool fire
//! of width D puffs at about 1.5 / sqrt(D) Hz (research/2026-10-16-real-time-fire-rendering.md),
//! a torch by a third either way, a campfire by about 15%, a house fire slower and shallower.

use super::{Blaze, Blazes, HEARTHS};
use crate::planet::MainCamera;
use crate::quality::Quality;
use bevy::{
    light::VolumetricLight,
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::RenderAssets,
        renderer::RenderQueue,
        storage::{GpuShaderBuffer, ShaderBuffer},
    },
};

/// How many point lights the fires are lit by, and how far round each its fire is gathered,
/// in metres.
pub(super) const LIGHTS: usize = 12;
const LIGHT_CELL_M: f32 = 9.0;
/// How far out from the middle of all that burns a gathering's light hangs, metres.
const LIGHT_OUT_M: f32 = 3.0;
/// A blaze's light, in lumens for a blaze a metre tall at full heat, and the most one light
/// gives: still some times a real fire's, for the film, but no longer so much that sunlit
/// stone round a fire goes salmon pink; a breath's brighter, as every game that draws one
/// well lights the ground under it. Eight lights at 3e7 left the walls round a burning keep
/// unlit by it in daylight (castle film r7).
pub(super) const LUMENS_A_METRE: f32 = 500_000.0;
const MOST_LUMENS: f32 = 4.0e7;
/// How far a blaze's light and a jet's reach, metres: at 55 every light tinted every sunlit
/// wall of the burg a uniform salmon (r8's first probe), where firelight by day shows only
/// close round the fire.
const LIGHT_RANGE_M: f32 = 22.0;
const JET_RANGE_M: f32 = 30.0;
/// A jet's light just past the mouth, on the breather's head and neck, along its stream and
/// where it strikes: at 1e7 the dragon and the wall it struck showed none of it by day
/// (castle film r7). Its colour the stream's yellow, a blaze's the orange of burning timber.
const JET_LUMENS: f32 = 5.0e7;
const JET_LIGHT: Color = Color::srgb(1.0, 0.72, 0.38);
const BLAZE_LIGHT: Color = Color::srgb(1.0, 0.62, 0.3);
/// A campfire's, wood's flame about 1,900 K: an orange, not the yellow the grass turned a
/// timber blaze's (lake film r5), and with no blue, as a blackbody that cool has none
/// (Mitchell Charity's table, 1,900 K #ff8300): at sRGB 1.0, 0.5, 0.18 its blue put the lit
/// stones' hue under 25 degrees, red, where with none the campfire's night crop is 38%
/// orange and 59% red, not 25 and 72 (fire sheet, campfire round 2026-10-19).
const HEARTH_LIGHT: Color = Color::srgb(1.0, 0.514, 0.0);
/// A kept fire's light, lumens for a metre of flame at full heat, and how far it reaches.
/// A campfire's 1.3 m at 0.9 heat gives 117,000 lm, about 9,000 cd: a real campfire's order
/// (some 0.5 m² of flame at 10^4 cd/m²), so the flame stays the brightest thing by it, its
/// stones at a metre some four stops under its core (research/2026-10-19-fire-step-back.md,
/// plan step 1). At 323,000 a metre, and 2.5 stops more by night, the stones drew past the
/// tonemapper's white, as bright as the flame (night stills: stones 251,232,227 against a
/// flame of 253,243,205; at this light and no night boost 195,114,123 against 254,235,119).
const HEARTH_LUMENS_A_METRE: f32 = 100_000.0;
const HEARTH_RANGE_M: f32 = 14.0;
/// Where a fire's light eases out with the eye's distance, metres (Valheim's 40 m).
const LIGHT_FADE_M: [f32; 2] = [30.0, 40.0];
/// Where a fire light's shadows are cast from, metres from the eye: switched on inside the
/// first, kept on until past the second, so a light at the edge does not blink (Valheim's 20 m).
const SHADOW_M: [f32; 2] = [20.0, 23.0];
/// How far a light sways for each metre of its flame's height, and the least and most.
const SWAY_A_METRE: f32 = 0.07;
const SWAY_M: [f32; 2] = [0.005, 0.4];
/// How many stops brighter a fire's light is by night than by day, as Witcher 3 keys its
/// fire's brightness to the hour in its environment curves: the night is drawn some stops
/// brighter than a real one (the moon at 700 lux, the eye at EV 9, `daylight`), so a fire as
/// bright as by day lit the ground round a campfire about as much as the moon did, grey and
/// barely there (fire sheet, round 5: 380,000 lm, about 1,000 lux at 3 m), where in every
/// game's night and every photograph a campfire's ground glows orange a few metres out; and
/// as bright by day as by night, sunlit stone round it went salmon. Not a kept fire's
/// (a hearth's or torch's): boosted so, a campfire's stones out-shone its flame
/// (`HEARTH_LUMENS_A_METRE`).
const NIGHT_STOPS: f32 = 2.5;

/// How many times brighter a fire's light is so far into the night (`Sky::night`, 0 by day,
/// 1 in full dark) than by day.
pub(super) fn by_night(night: f32) -> f32 {
    libm::exp2f(NIGHT_STOPS * night.clamp(0.0, 1.0))
}

/// How many fire lights' places and flickers the char and embers of what burns are told.
pub const GLOWS: usize = 16;

/// How a fire flickers: how deep (the most its light strays from its mean, a share) and how
/// many times a second it puffs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flicker {
    pub depth: f32,
    pub puffs_hz: f32,
}

impl Flicker {
    /// A fire's flicker by how wide its flames are, metres: a torch's (0.1 m) a third deep at
    /// about three puffs a second, a campfire's (0.7 m) 15% at 1.8, a house's (6 m) 7% at 0.6.
    pub fn of(wide_m: f32) -> Flicker {
        let wide = wide_m.clamp(0.02, 40.0);
        let x = libm::log10f(wide);
        let depth = if wide < 0.7 {
            lerp(0.35, 0.15, (x + 1.0) / (libm::log10f(0.7) + 1.0))
        } else {
            lerp(
                0.15,
                0.07,
                (x - libm::log10f(0.7)) / (libm::log10f(6.0) - libm::log10f(0.7)),
            )
        };
        Flicker {
            depth: depth.clamp(0.06, 0.35),
            puffs_hz: (1.5 / libm::sqrtf(wide)).clamp(0.4, 3.0),
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// A fire's flicker at a time, -1 to 1, from its own number: the shape every fire shares.
fn wave(seed: f32, t: f32, flicker: Flicker) -> f32 {
    // The product's turns come at about 1.8 times its first sine's rate.
    let a = core::f32::consts::TAU * flicker.puffs_hz / 1.8 * t + seed * 40.0;
    let sines = libm::sinf(a) * libm::sinf(0.564 * a + 2.1) * libm::cosf(0.758 * a + 0.7);
    let x = t * flicker.puffs_hz * 0.7;
    let cell = libm::floorf(x);
    let f = x - cell;
    let u = f * f * (3.0 - 2.0 * f);
    let own = (seed.rem_euclid(1.0) * 65_536.0) as u32;
    let at = |i: f32| terra_gen::hash::unit3(own, i as i32 as u32, 0x6669, 0x7265) * 2.0 - 1.0;
    let noise = at(cell) * (1.0 - u) + at(cell + 1.0) * u;
    (1.8 * sines + 0.3 * noise).clamp(-1.0, 1.0)
}

/// How bright a fire of its own number and flicker burns at a time, a share of its mean:
/// what its light, its flames, its embers and its char are all multiplied by.
pub fn signal(seed: f32, t: f32, flicker: Flicker) -> f32 {
    1.0 + flicker.depth * wave(seed, t, flicker)
}

/// How far a fire's light is off its middle at a time, on three axes by the same signal at a
/// third of a turn apart, `reach` at most.
pub fn sway(seed: f32, t: f32, flicker: Flicker, reach: f32) -> Vec3 {
    Vec3::new(
        wave(seed, t, flicker),
        wave(seed + 0.333, t, flicker),
        wave(seed + 0.667, t, flicker),
    ) * reach
}

/// How wide a blaze's flames are, metres.
fn width(blaze: &Blaze) -> f32 {
    if blaze.wide > 0.0 {
        blaze.wide
    } else {
        blaze.tall * 0.5
    }
}

/// How bright a blaze burns now, a share of its mean (`signal`): its slot's for the shaders.
pub(super) fn glow(blaze: &Blaze, t: f32) -> f32 {
    signal(blaze.seed, t, Flicker::of(width(blaze)))
}

/// One of the fires' lights, as it should be now.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Lit {
    /// Where it hangs, its flicker's sway in it.
    pub at: Vec3,
    /// Its lumens now, its flicker in them.
    pub lumens: f32,
    pub color: Color,
    pub range: f32,
    /// Its flame's half-width, metres.
    pub radius: f32,
    /// Its flicker now, a share of its mean.
    pub glow: f32,
    /// Whether it is a kept fire's, which glows in the air round it (`fog`).
    pub kept: bool,
}

/// The flash where a jet's stream first strikes (docs/fire.md, "Composites": 0.1 to 0.3 s),
/// as bright as the blast it lights, then gone: `FLASH_S` long, easing out, its light the
/// white of the blast's hottest.
const FLASH_LUMENS: f32 = 4.0e8;
const FLASH_S: f32 = 0.25;
const FLASH_LIGHT: Color = Color::srgb(1.0, 0.85, 0.62);

/// How bright a flash is `age` seconds after its stream struck, a share of its peak.
pub(super) fn flash(age: f32) -> f32 {
    if !(0.0..FLASH_S).contains(&age) {
        return 0.0;
    }
    let left = 1.0 - age / FLASH_S;
    left * left
}

/// Each jet's flash, while it lasts: `jets` each one's state as `Drawn` keeps it (on, since
/// when, when it last came on).
pub(super) fn flashes(blazes: &Blazes, jets: &[(bool, f32, f32)], t: f32) -> Vec<Lit> {
    blazes
        .jets
        .iter()
        .zip(jets)
        .filter_map(|(jet, &(on, since, came_on))| {
            let struck = came_on + jet.reach / super::JET_SPEED_MPS;
            // Struck only if it was still on when its front got there.
            let reached = on || struck <= since;
            let share = flash(t - struck);
            (reached && share > 0.0).then(|| {
                let way = jet.way.normalize_or(Vec3::Z);
                let at = jet.from + way * (jet.reach - 1.5) + jet.from.normalize_or(Vec3::Y) * 1.5;
                Lit {
                    at,
                    lumens: FLASH_LUMENS * share,
                    color: FLASH_LIGHT,
                    range: JET_RANGE_M * 1.5,
                    radius: 2.0,
                    glow: 1.0 + share,
                    kept: false,
                }
            })
        })
        .collect()
}

/// The lights the fires want now, the jets' first, then the kept fires', then each cell's.
pub(super) fn lit(blazes: &Blazes, t: f32) -> Vec<Lit> {
    // Each cell: its weighted place, its weight, its weighted glow, its widest flame and its
    // heaviest blaze's number.
    let mut cells: std::collections::BTreeMap<[i64; 3], (Vec3, f32, f32, f32, (f32, f32))> =
        std::collections::BTreeMap::new();
    let hearths = blazes.kept.len().min(HEARTHS);
    for blaze in blazes.all().skip(hearths) {
        let weight = blaze.tall * blaze.heat;
        let key = (blaze.at / LIGHT_CELL_M).floor().as_i64vec3().to_array();
        let cell = cells
            .entry(key)
            .or_insert((Vec3::ZERO, 0.0, 0.0, 0.0, (0.0, 0.0)));
        cell.0 += blaze.at * weight;
        cell.1 += weight;
        cell.2 += glow(blaze, t) * weight;
        cell.3 = cell.3.max(width(blaze));
        if weight > cell.4.0 {
            cell.4 = (weight, blaze.seed);
        }
    }
    let up = |at: Vec3| at.normalize_or(Vec3::Y);
    let mut lit: Vec<Lit> = blazes
        .jets
        .iter()
        .filter(|jet| jet.on)
        .enumerate()
        .flat_map(|(j, jet)| {
            let way = jet.way.normalize_or(Vec3::Z);
            let flicker = Flicker::of(0.3);
            [0.06, 0.4, 0.85].map(|along| {
                let at = jet.from + way * jet.reach * along;
                let seed = j as f32 * 0.37 + along;
                let glow = signal(seed, t, flicker);
                Lit {
                    at: at + up(at) * 1.5 + sway(seed, t, flicker, 0.3),
                    lumens: JET_LUMENS * glow,
                    color: JET_LIGHT,
                    range: JET_RANGE_M,
                    radius: 0.5,
                    glow,
                    kept: false,
                }
            })
        })
        .collect();
    // A hearth's light hangs in the body of its flames, clear of its logs' ends, so it lights
    // the ground round it and the faces of those sat by it, and its glow in the air (`fog`) is
    // a halo round the flames: at 0.8 of their height, by their tips, the glow was a ball
    // beside the fire (fire sheet review, 3/10); level with a 0.9 m teepee's ends the logs
    // were the brightest thing by it (lake film r6), which the campfire's 0.55 m stack clears.
    lit.extend(blazes.kept.iter().take(hearths).map(|blaze| {
        let flicker = Flicker::of(width(blaze));
        let glow = signal(blaze.seed, t, flicker);
        let reach = (blaze.tall * SWAY_A_METRE).clamp(SWAY_M[0], SWAY_M[1]);
        Lit {
            at: blaze.at + up(blaze.at) * blaze.tall * 0.6 + sway(blaze.seed, t, flicker, reach),
            lumens: blaze.tall * blaze.heat * HEARTH_LUMENS_A_METRE * glow,
            color: HEARTH_LIGHT,
            range: HEARTH_RANGE_M,
            // Its flames as a disc light as wide as they are (`crate::falloff`): its fall-off
            // levels off over the stones at its foot rather than rising inverse-square.
            radius: width(blaze),
            glow,
            kept: true,
        }
    }));
    // Each gathering's light hangs out from the middle of all that burns, on the outside of
    // the walls its flames stand on: at the gathering's own middle, inside a burning building,
    // it lit the inner faces and none of the stone the eye sees (castle film r8).
    let (sum, weight) = cells
        .values()
        .fold((Vec3::ZERO, 0.0), |(s, w), c| (s + c.0, w + c.1));
    let middle = if weight > 0.0 {
        sum / weight
    } else {
        Vec3::ZERO
    };
    let outward = |at: Vec3| {
        let away = at - middle;
        let flat = away - up(at) * away.dot(up(at));
        if flat.length() > 1.0 {
            flat.normalize() * LIGHT_OUT_M
        } else {
            Vec3::ZERO
        }
    };
    let mut gathered: Vec<Lit> = cells
        .into_values()
        .filter(|cell| cell.1 > 0.0)
        .map(|(sum, weight, glows, wide, (_, seed))| {
            let at = sum / weight;
            // The cell's light is the sum of its fires', each at its own flicker; it sways as
            // its biggest fire does, as far as a flame of the cell's whole height would.
            let glow = glows / weight;
            let tall = weight.sqrt();
            let reach = (tall * SWAY_A_METRE).clamp(SWAY_M[0], SWAY_M[1]);
            Lit {
                at: at + outward(at) + up(at) * 1.5 + sway(seed, t, Flicker::of(wide), reach),
                lumens: (weight * LUMENS_A_METRE).min(MOST_LUMENS) * glow,
                color: BLAZE_LIGHT,
                range: LIGHT_RANGE_M,
                radius: (wide * 0.5).min(2.0),
                glow,
                kept: false,
            }
        })
        .collect();
    gathered.sort_by(|a, b| b.lumens.total_cmp(&a.lumens));
    lit.extend(gathered);
    lit
}

/// How many fire lights cast shadows, by preset: two, four on the high (docs/fire.md), one on
/// the low, whose GPUs a cube of six shadow maps a light costs most.
pub fn shadowed(quality: Quality) -> usize {
    match quality {
        Quality::Low => 1,
        Quality::Medium => 2,
        Quality::High => 4,
    }
}

/// What one of the light entities was last frame: where, and whether it cast shadows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Was {
    pub at: Vec3,
    pub on: bool,
    pub shadows: bool,
}

/// A light to show: what it is, how far its distance has eased it, and whether it casts
/// shadows (and so shines in the fog).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Shown {
    pub lit: Lit,
    pub fade: f32,
    pub shadows: bool,
}

fn smooth(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The lights to show on each light entity: the most a fire's light matters where the eye is
/// (its lumens over its distance, eased out from 30 to 40 m), each kept on the entity that
/// showed the nearest of last frame's, so a light's shadows stay on it as fires come and go;
/// the nearest `shadows` within 20 m cast shadows, kept to 23 m once on.
pub(super) fn choose(
    lit: Vec<Lit>,
    eye: Option<Vec3>,
    was: &[Was],
    shadows: usize,
) -> Vec<Option<Shown>> {
    let far = |at: Vec3| eye.map_or(0.0, |eye| eye.distance(at));
    let mut wanted: Vec<(f32, Shown)> = lit
        .into_iter()
        .filter_map(|lit| {
            let d = far(lit.at);
            let fade = 1.0 - smooth(LIGHT_FADE_M[0], LIGHT_FADE_M[1], d);
            (fade > 0.0).then(|| {
                let weight = lit.lumens * fade / (1.0 + (d / 10.0).powi(2));
                (
                    weight,
                    Shown {
                        lit,
                        fade,
                        shadows: false,
                    },
                )
            })
        })
        .collect();
    wanted.sort_by(|a, b| b.0.total_cmp(&a.0));
    wanted.truncate(was.len());
    let mut out: Vec<Option<Shown>> = vec![None; was.len()];
    for (_, shown) in wanted {
        let free = |i: &usize| out[*i].is_none();
        let near = (0..was.len())
            .filter(free)
            .filter(|i| was[*i].on && was[*i].at.distance(shown.lit.at) < 4.0)
            .min_by(|a, b| {
                was[*a]
                    .at
                    .distance(shown.lit.at)
                    .total_cmp(&was[*b].at.distance(shown.lit.at))
            });
        let slot = near
            .or_else(|| (0..was.len()).filter(free).find(|i| !was[*i].on))
            .or_else(|| (0..was.len()).find(free));
        if let Some(slot) = slot {
            out[slot] = Some(shown);
        }
    }
    // Shadows: nearest first, a light that had them counted 3 m nearer, so two at much the
    // same distance do not trade them back and forth.
    let mut near: Vec<(f32, usize)> = out
        .iter()
        .enumerate()
        .filter_map(|(i, shown)| {
            let shown = shown.as_ref()?;
            let d = far(shown.lit.at);
            let had = was[i].shadows;
            let within = if had { SHADOW_M[1] } else { SHADOW_M[0] };
            (eye.is_some() && d < within).then_some((d - if had { 3.0 } else { 0.0 }, i))
        })
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    for &(_, i) in near.iter().take(shadows) {
        if let Some(shown) = out[i].as_mut() {
            shown.shadows = true;
        }
    }
    out
}

/// The fire lights' places and flickers (`GLOWS` of them: xyz where, w its glow, 0 for
/// none), for what burns to glow with: the char's cracks on a burning building flare as the
/// fire nearest them does. A buffer of the render world, written each frame.
#[derive(Resource, Clone, Debug)]
pub struct FireGlows {
    pub buffer: Handle<ShaderBuffer>,
    pub glows: [[f32; 4]; GLOWS],
}

impl ExtractResource<RenderApp> for FireGlows {
    type Source = FireGlows;

    fn extract_resource(source: &FireGlows) -> Self {
        source.clone()
    }
}

pub(super) fn plugin(app: &mut App) {
    let buffer = app
        .world_mut()
        .resource_mut::<Assets<ShaderBuffer>>()
        .add(ShaderBuffer::from(vec![[0.0f32; 4]; GLOWS]));
    app.insert_resource(FireGlows {
        buffer,
        glows: [[0.0; 4]; GLOWS],
    });
    if app.get_sub_app(RenderApp).is_none() {
        return;
    }
    app.add_plugins(ExtractResourcePlugin::<FireGlows>::default());
    app.sub_app_mut(RenderApp)
        .add_systems(Render, write_glows.in_set(RenderSystems::PrepareResources));
}

fn write_glows(
    glows: Option<Res<FireGlows>>,
    buffers: Res<RenderAssets<GpuShaderBuffer>>,
    queue: Res<RenderQueue>,
) {
    let Some(glows) = glows else {
        return;
    };
    if let Some(target) = buffers.get(&glows.buffer) {
        queue.write_buffer(
            &target.buffer,
            0,
            &glows
                .glows
                .iter()
                .flatten()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<u8>>(),
        );
    }
}

/// The fires' light, each frame: the lights the fires want, chosen for where the eye is, on
/// the light entities, and their places and flickers told to what burns.
#[allow(clippy::too_many_arguments)]
pub(super) fn light(
    mut commands: Commands,
    time: Res<Time>,
    quality: Res<Quality>,
    sky: Option<Res<crate::sky::Sky>>,
    blazes: Res<Blazes>,
    drawn: Res<super::Drawn>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut lights: Query<(
        &mut PointLight,
        &mut Transform,
        &mut Visibility,
        Has<VolumetricLight>,
    )>,
    mut glows: ResMut<FireGlows>,
    mut fog: ResMut<super::fog::Glows>,
) {
    let t = time.elapsed_secs_wrapped();
    let was: Vec<Was> = drawn
        .lights
        .iter()
        .map(|&entity| {
            lights.get(entity).map_or(
                Was {
                    at: Vec3::ZERO,
                    on: false,
                    shadows: false,
                },
                |(light, transform, visibility, _)| Was {
                    at: transform.translation,
                    on: *visibility != Visibility::Hidden,
                    shadows: light.shadow_maps_enabled,
                },
            )
        })
        .collect();
    let eye = camera.iter().next().map(GlobalTransform::translation);
    let mut wants = flashes(&blazes, &drawn.jets, t);
    wants.extend(lit(&blazes, t));
    let shown = choose(wants, eye, &was, shadowed(*quality));
    let by_night = by_night(sky.map_or(0.0, |sky| sky.night));
    let mut told = [[0.0f32; 4]; GLOWS];
    let mut wanted = Vec::new();
    for (i, (&entity, shown)) in drawn.lights.iter().zip(shown).enumerate() {
        let Ok((mut light, mut transform, mut visibility, volumetric)) = lights.get_mut(entity)
        else {
            continue;
        };
        match shown {
            Some(Shown { lit, fade, shadows }) => {
                // A kept fire's light is a real one's by night as by day: its flame, drawn on
                // the screen's scale, is what the night is seen by (D415's exposure rule).
                light.intensity = lit.lumens * fade * if lit.kept { 1.0 } else { by_night };
                light.range = lit.range;
                light.color = lit.color;
                light.radius = lit.radius;
                transform.translation = lit.at;
                *visibility = Visibility::Inherited;
                light.shadow_maps_enabled = shadows;
                // A kept fire's shadowed light shines in the air round it (`fog`), sampling
                // its shadows there; a blaze's stays out of the fog, where its tens of
                // millions of lumens lit a ball of air brighter than the house (round 6).
                let scatters = shadows && lit.kept;
                if scatters != volumetric {
                    if scatters {
                        commands.entity(entity).insert(VolumetricLight);
                    } else {
                        commands.entity(entity).remove::<VolumetricLight>();
                    }
                }
                if scatters {
                    wanted.push(super::fog::Glow::of(lit.at, lit.radius, fade));
                }
                if let Some(slot) = told.get_mut(i) {
                    *slot = lit.at.extend(lit.glow * fade.max(0.01)).to_array();
                }
            }
            None => {
                light.intensity = 0.0;
                light.shadow_maps_enabled = false;
                if volumetric {
                    commands.entity(entity).remove::<VolumetricLight>();
                }
                *visibility = Visibility::Hidden;
            }
        }
    }
    if told != glows.glows {
        glows.glows = told;
    }
    if fog.0 != wanted {
        fog.0 = wanted;
    }
}

#[cfg(test)]
mod tests;
