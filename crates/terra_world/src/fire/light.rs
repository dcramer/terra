//! The fires' light, by docs/fire.md's light rules (D415): one flicker per fire, the same
//! signal driving the light's power, the flame's and embers' brightness and the char's glow;
//! the light swaying on three axes by it, so the shadows it casts move as a real flame's do;
//! shadows from the nearest few fire lights, the kept fires' among them glowing in a fog volume
//! of their own (`fog`); brighter by night than by day (`by_night`, each light by its own stops); and a
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
/// (castle film r7), but at 5e7 and a pale yellow the ground, the wall and the dragon by night
/// stood brighter than the flame, near white (breath r27: the fireball's crop 365% as lit as
/// at noon), where the flame is the brightest thing by it (D415). Its colour an orange-yellow,
/// greener than a blaze's, and half as bright: at #ff9438 and 4e6 the wall and ground it
/// struck went past the tonemapper's highlight limit as salmon pink, as a campfire's stones
/// did at a red light's low green (`HEARTH_LIGHT`), where the references' firelit walls read
/// orange-yellow (breath r33, r34).
const JET_LUMENS: f32 = 2.0e6;
const JET_LIGHT: Color = Color::srgb(1.0, 0.74, 0.32);
/// A marched breath's light (D430), dimmer than the cards': at theirs the wall and ground
/// round its larger fireball went past Neutral's shoulder at dusk (sRGB 237, 185, 119 head-on)
/// and read salmon beside it. Orange, not yellow: at (1, 0.78, 0.3) and 1.4e6 the grass under
/// it read a flat yellow-green flood (vol10's review); the film mask (`BREATH_LIT`) keeps the
/// orange from going salmon past the shoulder now. Dimmer at 0.35e6: at 0.6e6 the ground by
/// the hit stood 1.6 to 5 times white before the tonemapper (vol12's probe). Amber: redder
/// only made up for the film mask's shoulder taken a channel at a time, which ran the ground's
/// green out with its red to a pale lime-yellow; the shoulder keeps the hue now, and at
/// (1, 0.5, 0.16) the wall read coral, at (1, 0.58, 0.22) terracotta (fire/hue's blind A/Bs).
const MARCHED_LUMENS: f32 = 0.35e6;
const MARCHED_LIGHT: Color = Color::srgb(1.0, 0.64, 0.26);
const BLAZE_LIGHT: Color = Color::srgb(1.0, 0.62, 0.3);
/// A campfire's light: wood flame's orange, warmer than a blaze's, as the games white-balance
/// firelight (about 2,300 K, Mitchell Charity's table: 2,200 K #ff932c, 2,500 K #ffa148), its
/// blue under the blackbody's. At 1,900 K (#ff8300) its green was so low that the night's
/// blue on the lit stones and the tonemapper's shoulder took them salmon pink (fire round 13);
/// at (1.0, 0.6, 0.12) its pool and stones still read pink-salmon, so its green is 2,700 K's
/// (#ffa957), its blue still half the blackbody's (fire round 21: alone against main, blind, 5/10 against 3.5/10).
const HEARTH_LIGHT: Color = Color::srgb(1.0, 0.58, 0.12);
/// A kept fire's light, lumens for a metre of flame at full heat, and how far it reaches.
/// A campfire's 1.3 m at 0.9 heat gives 117,000 lm, about 9,000 cd: a real campfire's order
/// (some 0.5 m² of flame at 10^4 cd/m²), so the flame stays the brightest thing by it, its
/// stones at a metre some four stops under its core (research/2026-10-19-fire-step-back.md,
/// plan step 1). At 323,000 a metre, and 2.5 stops more by night, the stones drew past the
/// tonemapper's white, as bright as the flame (night stills: stones 251,232,227 against a
/// flame of 253,243,205; at this light and no night boost 195,114,123 against 254,235,119).
/// Tightened with the pool (fire round 19): 50,000 a metre (58,500 lm) and a 7 m reach (6 m from round 23), the light's
/// disc as wide as its flames, keep the pool inside 4-5 m.
const HEARTH_LUMENS_A_METRE: f32 = 60_000.0;
const HEARTH_RANGE_M: f32 = 6.0;
/// How many stops brighter a kept fire's light is by night, as Witcher 3 keys a fire to the
/// hour: by its real lumens the yard's ground took a 1.2x pool at 1 m and none past 1.5 m
/// against the night's moonlit ground, drawn some stops over a real night's; the Witcher 3 and
/// RDR2 pools read to 3-4 m. At 3.2 stops, with `HEARTH_SPREAD`, the 6 m view's ground is
/// 3.4x the far ground at 2 m and 1.6x at 4 m, its stones 1.7 stops under the flame
/// (fire round 7, 2026-10-19). With the night's moon at a quarter (fire round 10), 2.2: at
/// 3.2 the firelit stones and logs went past the tonemapper's shoulder and read salmon-pink.
/// With the night's moon back at a half (fire round 11), 3.0: at 2.2 the ground round the
/// stones had no firelit pool (linear luma 0.058 at 0.8 m in the 2.5 m view), at 3.0 it is
/// 0.12, warm (R/B 6.4), as the games' pools read (fire round 12). Hung lower (0.35 of its
/// flames), the ring of stones shadowed the ground outside it: the pool halved. With a
/// greener light, 2.0: at 3.0 the lit stones still passed the shoulder
/// (sRGB 232,126,88), at 2.0 they are 192,106,61, warm brown (blind review 6/10 against 3,
/// fire round 13). With the deep night's moon and ambient at 0.6 (`daylight`), 2.5 and a 6 m
/// reach: the ground round the ring warmer, the field past it dark (fire round 23).
const HEARTH_NIGHT_STOPS: f32 = 2.5;
/// How much wider than its flames a hearth's light is as a disc (`crate::falloff`): wider
/// levels its fall-off over the stones at its foot, so the ground metres out takes more of the
/// night's boost and the stones less. At 3 times its 1.3 m flames the disc's 1/(d²+r²) stayed
/// level out to some 4 m, so the ground had no pool, only an even tint to the horizon; as wide
/// as its flames, the pool falls off past the stones (blind review 5/10 against 4, fire round 19).
const HEARTH_SPREAD: f32 = 1.0;
/// How far up its flames a hearth's light hangs: in their body, over the logs' ends.
/// Hung low (0.4, its fill at 0.9 and 0.15 of its light, the light's green 2,200 K's), the ring's
/// stones are lit orange on their fire-facing sides and dark away from it, where hung at 0.75 with
/// its fill at 1.6 they were lit khaki on top and the pool read yellow-green (fire round 26).
const HEARTH_LIGHT_UP: f32 = 0.4;
/// The share of a hearth's light that casts no shadows: all of it shadowed, its own stones
/// threw black lobes a metre out over the lit ground (round 7's side view), where the games'
/// are soft. Bevy's PCSS on the cube map (`soft_shadows_enabled`) softened nothing seen. At a
/// half, with `HEARTH_FILL_GAIN` 4, four fifths of what the ground took had no shadow, so the
/// stones cast none and the pool had no edge; at 0.3 they shadow the ground outside the ring
/// (fire round 19).
const HEARTH_FILL: f32 = 0.15;
/// How high over its fire a hearth's fill hangs, over its flames' height, and how much
/// brighter it is than its share (`HEARTH_FILL`): the pool of firelit ground the games show
/// to 3-4 m, without the stones at the fire's foot passing the tonemapper's shoulder. Hung in
/// the flames with the shadowed light, the ground metres out took it at a grazing angle and
/// the stones a metre off head on, so lighting the pool lit the stones salmon (fire round 13);
/// hung high, the ground takes it more nearly straight down and the stones farther off. At
/// 2.2 and 4 times, the ground 2.5-3.5 m out takes three times the light by night and the
/// stones' lit faces 1.6 times, still warm brown (sRGB 113,82,51; fire round 14; landed alone in round 15).
/// With the tighter disc, 1.6 and 1.5 times: the pool's near ground kept, the far left dark
/// (fire round 19).
const HEARTH_FILL_UP: f32 = 0.9;
const HEARTH_FILL_GAIN: f32 = 1.5;
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
/// as bright by day as by night, sunlit stone round it went salmon. A kept fire (a hearth's
/// or torch's) has its own (`HEARTH_NIGHT_STOPS`), with a wider disc.
const NIGHT_STOPS: f32 = 2.5;
/// A marched breath's strike light (D430; its stream's take none, `lit`), brighter by night by
/// fewer: at a blaze's 2.5 the ground and the wall's foot under its stream clipped to pale
/// cream by night (sRGB 247, 215, 151; vol7's L), where by dusk they took orange.
pub(super) const BREATH_NIGHT_STOPS: f32 = 1.0;

/// How many times brighter a light of so many night stops (`Lit::night_stops`) is so far
/// into the night (`Sky::night`, 0 by day, 1 in full dark) than by day.
pub(super) fn by_night(stops: f32, night: f32) -> f32 {
    libm::exp2f(stops * night.clamp(0.0, 1.0))
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
    /// Whether it never casts shadows: a hearth's fill (`HEARTH_FILL`).
    pub shadowless: bool,
    /// How many stops brighter it is by night (`NIGHT_STOPS`, `HEARTH_NIGHT_STOPS`,
    /// `BREATH_NIGHT_STOPS`).
    pub night_stops: f32,
}

/// A jet's light, its lumens and colour, as its breath is drawn.
pub(super) fn jet_light(tier: super::BreathTier) -> (f32, Color) {
    match tier {
        super::BreathTier::Cards => (JET_LUMENS, JET_LIGHT),
        super::BreathTier::Marched => (MARCHED_LUMENS, MARCHED_LIGHT),
    }
}

/// The flash where a jet's stream first strikes (docs/fire.md, "Composites": 0.1 to 0.3 s),
/// as bright as the blast it lights, then gone: `FLASH_S` long, easing out, its light the
/// white of the blast's hottest.
const FLASH_LUMENS: f32 = 8.0e7;
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
                    shadowless: false,
                    night_stops: NIGHT_STOPS,
                }
            })
        })
        .collect()
}

/// A marched breath's fireball's light on the face it strikes (D430): its jet's lights hang
/// over the stream, the nearest most of a metre from the face, so the wall showed a small
/// patch of weak orange and the ground none (vol5's review), where the references' walls and
/// ground glow orange metres round the hit. It stands out from the face, so it spreads wide
/// and even rather than burning one patch past the tonemapper's shoulder, orange rather than
/// yellow, and grows in with the fireball over the 0.6 s after the stream strikes. Brighter
/// and nearer the face than at first (2e6, 3 m out), it is the pool the ground's light falls
/// off from, the stream's lights taking no night stop (vol10's review: a flat flood). Under
/// half that (at 3.2e6 the ground by the hit stood 5 times white, vol12's probe), and orange:
/// the film mask's shoulder keeps the hue, so the redder (1, 0.42, 0.1) that made up for its
/// running green out with red read coral on the wall, and (1, 0.5, 0.16) terracotta (fire/hue's
/// blind A/Bs).
const STRIKE_LUMENS: f32 = 1.5e6;
const STRIKE_LIGHT: Color = Color::srgb(1.0, 0.6, 0.2);
const STRIKE_OUT_M: f32 = 1.8;
const STRIKE_GROW_S: f32 = 0.6;

/// Each marched jet's fireball light while it burns: `jets` each one's state as `Drawn` keeps
/// it (on, since when, when it last came on).
pub(super) fn strikes(
    blazes: &Blazes,
    jets: &[(bool, f32, f32)],
    t: f32,
    tier: super::BreathTier,
) -> Vec<Lit> {
    if tier != super::BreathTier::Marched {
        return Vec::new();
    }
    blazes
        .jets
        .iter()
        .zip(jets)
        .filter_map(|(jet, &(on, since, _))| {
            let struck = since + jet.face / super::JET_SPEED_MPS;
            let grown = ((t - struck) / STRIKE_GROW_S).clamp(0.0, 1.0);
            (on && jet.face > 0.0 && grown > 0.0).then(|| {
                let way = jet.way.normalize_or(Vec3::Z);
                let hit = jet.from + way * jet.face;
                let normal = jet.normal.normalize_or(-way);
                let flicker = Flicker::of(0.3);
                let glow = signal(7.1, t, flicker);
                Lit {
                    at: hit + normal * STRIKE_OUT_M + hit.normalize_or(Vec3::Y) * 1.0,
                    lumens: STRIKE_LUMENS * grown * glow,
                    color: STRIKE_LIGHT,
                    range: JET_RANGE_M,
                    radius: 1.5,
                    glow,
                    kept: false,
                    shadowless: false,
                    night_stops: BREATH_NIGHT_STOPS,
                }
            })
        })
        .collect()
}

/// The lights the fires want now, the jets' first, then the kept fires', then each cell's.
pub(super) fn lit(blazes: &Blazes, t: f32, tier: super::BreathTier) -> Vec<Lit> {
    let (jet_lumens, jet_light) = jet_light(tier);
    // Marched, its stream's lights take no night stop, only its strike's: hung metres over the
    // ground, they lit it as one flat flood past the shoulder (vol10's review), where the
    // references' ground glows round the hit and falls off away from it (`strikes`).
    let jet_stops = match tier {
        super::BreathTier::Cards => NIGHT_STOPS,
        super::BreathTier::Marched => 0.0,
    };
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
                    lumens: jet_lumens * glow,
                    color: jet_light,
                    night_stops: jet_stops,
                    range: JET_RANGE_M,
                    radius: 0.5,
                    glow,
                    kept: false,
                    shadowless: false,
                }
            })
        })
        .collect();
    // A hearth's light hangs in the body of its flames, clear of its logs' ends, so it lights
    // the ground round it and the faces of those sat by it, and its glow in the air (`fog`) is
    // a halo round the flames: at 0.8 of their height, by their tips, the glow was a ball
    // beside the fire (fire sheet review, 3/10); level with a 0.9 m teepee's ends the logs
    // were the brightest thing by it (lake film r6), which the campfire's 0.55 m stack clears.
    // Two lights: one shadowed, one its fill (`HEARTH_FILL`).
    lit.extend(blazes.kept.iter().take(hearths).flat_map(|blaze| {
        let flicker = Flicker::of(width(blaze));
        let glow = signal(blaze.seed, t, flicker);
        let reach = (blaze.tall * SWAY_A_METRE).clamp(SWAY_M[0], SWAY_M[1]);
        let lumens = blaze.tall * blaze.heat * HEARTH_LUMENS_A_METRE * glow;
        // It sways up from where it hangs, never down: swayed a hand's breadth below, the
        // shadowed light passed under a log's end, whose shadow ballooned for a fifth of a
        // second into a hard black wedge over the ground (fire round 30, clip frames 198-208).
        let swayed = sway(blaze.seed, t, flicker, reach);
        let rise = swayed.dot(up(blaze.at));
        let swayed = swayed + up(blaze.at) * (rise.abs() - rise);
        let main = Lit {
            at: blaze.at + up(blaze.at) * blaze.tall * HEARTH_LIGHT_UP + swayed,
            lumens: lumens * (1.0 - HEARTH_FILL),
            color: HEARTH_LIGHT,
            range: HEARTH_RANGE_M,
            // Its flames as a disc light (`crate::falloff`), wider than they are
            // (`HEARTH_SPREAD`): its fall-off levels off over the stones at its foot.
            radius: width(blaze) * HEARTH_SPREAD,
            glow,
            kept: true,
            night_stops: HEARTH_NIGHT_STOPS,
            shadowless: false,
        };
        let fill = Lit {
            lumens: lumens * HEARTH_FILL * HEARTH_FILL_GAIN,
            shadowless: true,
            at: blaze.at + up(blaze.at) * blaze.tall * HEARTH_FILL_UP + swayed,
            ..main.clone()
        };
        [main, fill]
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
                shadowless: false,
                night_stops: NIGHT_STOPS,
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
        Quality::High | Quality::Ultra => 4,
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
            let shown = shown.as_ref().filter(|shown| !shown.lit.shadowless)?;
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
    tier: Res<super::BreathTier>,
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
    wants.extend(strikes(&blazes, &drawn.jets, t, *tier));
    wants.extend(lit(&blazes, t, *tier));
    let shown = choose(wants, eye, &was, shadowed(*quality));
    let night = sky.map_or(0.0, |sky| sky.night);
    let mut told = [[0.0f32; 4]; GLOWS];
    let mut wanted = Vec::new();
    for (i, (&entity, shown)) in drawn.lights.iter().zip(shown).enumerate() {
        let Ok((mut light, mut transform, mut visibility, volumetric)) = lights.get_mut(entity)
        else {
            continue;
        };
        match shown {
            Some(Shown { lit, fade, shadows }) => {
                // Each light is brighter by night by its own stops: a kept fire's fewer than a
                // blaze's, so its flame stays the brightest thing by it (D415), and a marched
                // breath's fewer again.
                light.intensity = lit.lumens * fade * by_night(lit.night_stops, night);
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
