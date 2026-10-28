//! The fires' light, by docs/fire.md's light rules (D415): one flicker per fire, the same
//! signal driving the light's power, the flame's and embers' brightness and the char's glow;
//! the light swaying on three axes by it, so the shadows it casts move as a real flame's do;
//! shadows from the nearest few fire lights, the kept fires' among them glowing in a fog volume
//! of their own (`fog`); brighter by night than by day (`by_night`, each light by its own stops); and a
//! dozen lights however much burns, gathered over cells, chosen by what the eye is near and
//! eased out by 40 m (D127).
//!
//! The flicker is a sum of three sines round its puff rate, their phases wandering slowly
//! (`TONES`), as fast and as deep as a fire that size puffs: a pool fire of width D puffs at
//! about 1.5 / sqrt(D) Hz (research/2026-10-16-real-time-fire-rendering.md), a torch by a
//! third either way, a campfire by about 15%, a house fire slower and shallower. A torch
//! is lit by its own rig (`Rig::Torch`, `torch`).

use super::{Blaze, Blazes};
use crate::planet::MainCamera;
use crate::quality::Quality;
use bevy::{light::VolumetricLight, prelude::*};

mod glows;
mod jets;
mod shadows;
mod shut;
mod torch;
pub use glows::FireGlows;
pub(super) use glows::plugin;
#[cfg(test)]
pub(super) use jets::{flash, jet_light};
use jets::{flashes, jet_lights, strikes};
use shadows::ShadowFade;

/// How many point lights the fires are lit by (docs/fire.md's budget: 16 unshadowed and the
/// shadowed ones), and how far round each its fire is gathered, in metres. At 12 a dungeon's
/// thirty torches, two lights each, traded the lights 22 times a second along a corridor.
pub(super) const LIGHTS: usize = 20;
const LIGHT_CELL_M: f32 = 9.0;
/// How far out from the middle of all that burns a gathering's light hangs, metres, where its
/// blazes face no one way (a roof, a yard).
const LIGHT_OUT_M: f32 = 3.0;
/// How far out of the face its blazes burn on a gathering's light hangs, and how far over
/// their foot, metres, as games hang a burning building's lights out in front of its walls
/// rather than in its flames: hung 1.5 m over the flames' middle, a burning hoarding's light
/// met the stone under it edge-on and lit none of it (castle round 4, every judge).
const FACE_OUT_M: f32 = 4.0;
const FACE_UP_M: f32 = 0.5;
/// How much of a gathering's weight must face one way for its light to hang out of that face.
const FACING: f32 = 0.3;

/// The way a gathering's blazes face across the ground (`Blaze::out`, weighted by `weight`, its
/// up taken out), if enough of them face one way.
fn facing(at: Vec3, out: Vec3, weight: f32) -> Option<Vec3> {
    let up = at.normalize_or(Vec3::Y);
    let flat = out - up * out.dot(up);
    (flat.length() > FACING * weight).then(|| flat.normalize())
}
/// A blaze's light, in lumens for a blaze a metre tall at full heat, and the most one light
/// gives: still some times a real fire's, for the film, but no longer so much that sunlit
/// stone round a fire goes salmon pink; a breath's brighter, as every game that draws one
/// well lights the ground under it. Eight lights at 3e7 left the walls round a burning keep
/// unlit by it in daylight (castle film r7).
pub(super) const LUMENS_A_METRE: f32 = 500_000.0;
const MOST_LUMENS: f32 = 4.0e7;
/// How far a blaze's light reaches, metres (a jet's, `jets`): at 55 every light tinted every
/// sunlit wall of the burg a uniform salmon (r8's first probe), where firelight by day shows
/// only close round the fire.
const LIGHT_RANGE_M: f32 = 22.0;
const BLAZE_LIGHT: Color = Color::srgb(1.0, 0.62, 0.3);
/// A campfire's light: wood flame's orange, warmer than a blaze's, as the games white-balance
/// firelight (about 2,300 K, Mitchell Charity's table: 2,200 K #ff932c, 2,500 K #ffa148), its
/// blue under the blackbody's. At 1,900 K (#ff8300) its green was so low that the night's
/// blue on the lit stones and the tonemapper's shoulder took them salmon pink (fire round 13);
/// at (1.0, 0.6, 0.12) its pool and stones still read pink-salmon (fire round 21). With the
/// shoulder kept off the lit stones since, the green came down again: at (1.0, 0.58, 0.12) our
/// olive grass round it took it as yellow-olive, hue 36 at a flame's height out, where RDR2's
/// night pools are hue 20-21 at saturation 0.7-0.9; at (1.0, 0.46, 0.08) it is 25, the stones
/// hue 22 at 0.74, orange not salmon (campfire step round); at that the stones sat on the
/// groundlight check's least hue (20), so a little more green keeps a margin (campfire short round);
/// with the flame drawn lower over low logs they fell to 20-21 at 0.48, so more again
/// (campfire low-lay round), but at 0.53 the grass round it went to hue 31, past the hearth
/// test's 30, so 0.50 (campfire soot round).
pub const HEARTH_LIGHT: Color = Color::srgb(1.0, 0.50, 0.08);
/// A pitch torch's light, about 2,700 K (Charity's table: #ffa957), as games white-balance
/// torchlit stone warm off-white: at the campfire's a dungeon's walls read one rust cast
/// (dungeon round 11). A torch burns it, lit by its rig (`Rig::Torch`).
pub const TORCH_LIGHT: Color = Color::srgb(1.0, 0.66, 0.32);

/// The lights a kept fire is lit by, its kind's (`Kindled::rig`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Rig {
    /// A campfire's or a hearth's: a shadowed key in the body of its flames and an unshadowed
    /// light at their top for the faces sat by it (`HEARTH_TOP`).
    #[default]
    Hearth,
    /// A torch's: a short shadowed key just over its head, lighting a pool round its foot, and
    /// a dim light at its tips (`torch`).
    Torch,
}
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
/// reach: the ground round the ring warmer, the field past it dark (fire round 23). With the
/// top light keyed down by night (`HEARTH_TOP_NIGHT_STOPS`), 3.0: at 2.5 the logs and stones
/// shadowing the pit light left the night pool darker than both RDR2 frames by its stones
/// (sRGB luma 60 at a flame height out against 83 and 53; campfire pale round). 3.6 (g54): the
/// ground past the stones luma 128 (was 98), the pool still falling 10.9 times from 1.5 to 4.5 m.
const HEARTH_NIGHT_STOPS: f32 = 3.6;
/// The most a kept fire is keyed up against the dusk sky (`hearth_by_hour`), whatever its own
/// night stops: 3.0 (was 2.5, g54); at 3.5 the night's stones kept 0.71 of their light alike.
const HEARTH_DUSK_STOPS: f32 = 3.0;
/// How much wider than its flames a hearth's light is as a disc (`crate::falloff`): wider
/// levels its fall-off over the stones at its foot, so the ground metres out takes more of the
/// night's boost and the stones less. At 3 times its 1.3 m flames the disc's 1/(d²+r²) stayed
/// level out to some 4 m, so the ground had no pool, only an even tint to the horizon; as wide
/// as its flames, the pool falls off past the stones (blind review 5/10 against 4, fire round 19).
const HEARTH_SPREAD: f32 = 1.0;
/// A campfire is lit by two shadowed lights, as games and film light one: a low key in the
/// pit, for the stones and the ground at its foot, and a brighter light at the top of its
/// flames, for the faces of those sat round it. Shipped games hang a fire's light about its
/// flames' tips and afford it a shadow; a light in the pit alone lights the ring and leaves a
/// face a metre off grazed from below and lit no brighter than the sky lights it.
///
/// How far up its flames the key hangs: in their body, over the logs' ends. Hung low (0.4),
/// the ring's stones are lit orange on their fire-facing sides and dark away from it, where
/// hung at 0.75 they were lit khaki on top and the pool read yellow-green (fire round 26).
const HEARTH_LIGHT_UP: f32 = 0.4;
/// The share of a hearth's light at the top of its flames (`HEARTH_TOP_UP`), and how much
/// brighter it is than its share. With a shadowless fill 0.15 of the light, 1.5 times its
/// share, at 0.9 of the flames, the troll sat 1.2 m from the lake's fire at dusk took only
/// 0.36 of the sky's light on his face from it, his far arm in the sky's glow brighter than
/// his firelit face, the shadow factor at him about 1 (lake round 18's probe, light by light).
/// At 0.6 of it, 3 times its share, at the flames' top, his face is 2.4 times as lit as with
/// no firelight and warm (R/B 3.5), the ground round the ring 297 times the far ground (161
/// before), the stones still warm brown (sRGB 224,160,78). Unshadowed (`Lit::casts`), still
/// shining in the air's glow: its cube cost 1.5 ms by dusk (fire round g29), and the sitter's
/// shadow on the ground behind him is the pit key's (shadowed, it was 183 on his face against
/// 180 unshadowed).
/// The fire sheet's dusk campfire crop, 22% red and orange with it (34% in an earlier run), is
/// not its doing: kept out of the air's glow it stayed at 22%, and at 2.5 times it was 24%
/// (lake round 20).
const HEARTH_TOP: f32 = 0.6;
const HEARTH_TOP_GAIN: f32 = 3.0;
/// How high the top light hangs, over its flames' height: half as high again as their tips,
/// over the camp (`HEARTH_TOP_DUSK`). Hung at 0.8 his face took less (154 against 185) and the
/// ground more; at their tips, keyed up at dusk, his face took nearly twice what it does here
/// for the same pool 3 m out.
const HEARTH_TOP_UP: f32 = 1.5;
/// How far the top light reaches, metres: farther than the key, as a fire's light hung high
/// lights the camp round it. A light's reach is a window on its fall-off that takes it to
/// nothing at its range (Bevy's, after Unreal's), and at the key's 6 m the ground 3 to 5 m out,
/// 4 to 5.5 m from a lake fire's 2 m top, took a half to an eighth of its light: the pool past
/// 2 m was weak on the ground and the troll's rock (lake round 19's review). At 10 m, hung
/// over the tips, it takes some 0.9 to 0.8.
const HEARTH_TOP_RANGE_M: f32 = 10.0;
/// How much wider than its flames the top light is as a disc, and how many times more it is
/// keyed up against the dusk sky than the pit light (`hearth_by_hour`). Seen against a civil
/// dusk's sky, both keyed alike, the lake's campfire gave the ground 2 m out a third of the
/// sky's light and 4 m out a sixteenth: its pool grey past the stones (lake round 20's
/// review, "about 2x up close in a neutral grey ambient"), where the games' dusk campfires
/// (RDR2's, The Witcher 3's) light the camp orange a few metres round. A fire's light by
/// inverse-square alone cannot: what lights the ground 4 m out to the sky's light lights a
/// face a metre off past the white. So the top light, hung over the flames' tips as a camp's
/// light is in a film's campfire rig (a soft source over the fire), is wider and keyed up at
/// dusk alone; the pit light, which lights the stones, keeps the shared key, so they keep
/// their colour; by night it gives way to the night's stops (lake round 21). Modelled
/// (`a_campfires_pool_reads_warm_a_few_metres_out_at_dusk`): the ground 3 m out takes half
/// the sky's light and reads orange, a sitter's face under five times as lit as by the sky.
/// 9 (was 7) for the dusk pool (g52); at 10 a face 2.6 m out took 5.6 times the sky's (g54).
const HEARTH_TOP_SPREAD: f32 = 2.0;
const HEARTH_TOP_DUSK: f32 = 9.0;
/// How many times more the pit key is keyed up against the dusk sky (`hearth_by_hour`), as the
/// top light is (`HEARTH_TOP_DUSK`). With the dusk's boost on the top light alone (the key 1),
/// hung high and wide it gave the ground some 97% of its firelight and lit it evenly: on the
/// fire sheet's dusk strip the ground fell only to 0.38 of its light by the stones three to
/// four ring radii out (linear), to 0.71 at two, warm alike to the frame's edges, where RDR2's
/// night pool falls to 0.40 at two and 0.14 at three to four (g80's measure). With the boost
/// moved to the key (30, the top 3) the pool fell off, but the key, low in the flames, lights
/// the stones' fire-facing sides square on: they took 5.5 times main's light, past the
/// tonemapper's shoulder, and every judge's worst fault was a ring and ash bed of flat,
/// saturated clay-orange and a brown ribbon of lit smoke over it (g82's panel faults). At 5,
/// the pool's fall-off drawn by the top light's reach at dusk (`HEARTH_TOP_DUSK_REACH`), the
/// stones take some 1.6 times main's and a face by the stones under five times the sky's.
const HEARTH_KEY_DUSK: f32 = 5.0;
/// How far the top light reaches at dusk, metres for each metre of flame: drawn in from its
/// night's reach (`HEARTH_TOP_RANGE_M`) as `hearth_by_hour` keys it up, giving way to it as the
/// night comes (`range_by_hour`). A light's reach is a window on its fall-off (Bevy's, after
/// Unreal's attenuation radius, as games shape a light's pool), so at the yard's dusk the
/// ground three metres from a 1.5 m campfire takes half the top light it took at 10 m and the
/// field past 4.7 m none: the pool falls off past the camp, where at the full reach the
/// ground 6 m out stood 1.15 times as lit as the far field
/// (`a_campfires_dusk_pool_is_brightest_by_its_stones_and_falls_off_within_a_few_metres`). At
/// 2.7 the lake's 1.95 m fire left the ground 4 m out no longer warm (red 1.15 times blue,
/// `a_campfires_pool_reads_warm_a_few_metres_out_at_dusk`). A fire raised off the ground (a
/// brazier) lights the ground under it by its key at dusk.
const HEARTH_TOP_DUSK_REACH: f32 = 3.0;
/// How many stops brighter the top light is by night, fewer than the pit light's: hung high
/// and wide it lit the ground evenly, so at the pit light's 2.5 the night pool stood flat to
/// three flame heights out (sRGB luma 89 at one, 59 at three), where RDR2's campfires fall to
/// a third or a fifth (83 to 52, 53 to 32; campfire step-back, difference 2). At 1.0 the pit
/// light's steeper fall-off leads by night (so at 1.8 with its 3.6, g54), a bright core going
/// dark within a few fire widths; by dusk it is keyed up as before (`hearth_by_hour`).
const HEARTH_TOP_NIGHT_STOPS: f32 = 1.8;
/// Where a fire's light eases out with the eye's distance, metres (Valheim's 40 m).
const LIGHT_FADE_M: [f32; 2] = [30.0, 40.0];
/// Where a fire light's shadows are cast from, metres from the eye: switched on inside the
/// first, kept on until past the second, so a light at the edge does not blink (Valheim's 20 m).
const SHADOW_M: [f32; 2] = [20.0, 23.0];
/// A light shown is eased in from the weight of the first left out to so many times it.
const CUT_EASE: f32 = 2.0;
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

/// The sky's light on the shade (`SkyLight::brightness`) against which a kept fire takes
/// its whole night's boost once the sun is down. Keyed to the night alone, a campfire at civil
/// dusk took a fifth of a stop where the sky's light on the shade was still most of the day's:
/// at the lake (the sun a degree under, `night` 0.13, the sky's light 1,240) the troll by its
/// fire was lit 2x on its fire side and not warm, and the grass by the fire matched the far
/// grass (lake film r15). A fire's light is weighed by the eye against the light it is seen
/// in, as Witcher 3 keys its fires up as the sun goes, so the boost follows the sky's light
/// down from the day: at 240 the lake's dusk (1,240) takes 5.2 times (3.4 at 360, before g54),
/// and the yard's darker dusk (830, the sun five degrees under) keeps its night's share, 3.5 at
/// 3.6 stops; under 239 it asks more, its lit ground taking the sheet's campfire out of orange.
const HEARTH_DUSK_SKY: f32 = 240.0;

/// How many times brighter a kept fire's light of so many night stops is at an hour: by the
/// night's stops (`by_night`), and as the sun goes down (`Sky::twilight`, 0 with the sun six
/// degrees up, 1 a degree under) at least as much as the sky's light on the shade is over
/// `HEARTH_DUSK_SKY`, up to `HEARTH_DUSK_STOPS`' boost, and that so many times more
/// (`Lit::dusk_gain`), the gain giving way to the night's own stops as the night comes. By
/// day none: sunlit stone round a fire as bright as by night went salmon (`NIGHT_STOPS`).
pub(super) fn hearth_by_hour(
    stops: f32,
    dusk_gain: f32,
    night: f32,
    twilight: f32,
    sky_light: f32,
) -> f32 {
    let full = libm::exp2f(HEARTH_DUSK_STOPS);
    let gain = 1.0 + (dusk_gain - 1.0) * (1.0 - night.clamp(0.0, 1.0));
    let against_sky = (sky_light / HEARTH_DUSK_SKY).clamp(1.0, full) * gain;
    let down = twilight.clamp(0.0, 1.0);
    by_night(stops, night).max(1.0 + (against_sky - 1.0) * down * down)
}

/// How far a kept fire's light reaches at an hour: its range, drawn in to the share it keeps at
/// dusk (`Lit::dusk_reach`) as the sun goes down, as `hearth_by_hour` keys it up, and given
/// back as the night comes. By day and by night, the whole of it.
pub(super) fn range_by_hour(range: f32, dusk_reach: f32, night: f32, twilight: f32) -> f32 {
    let down = twilight.clamp(0.0, 1.0);
    let dusk = (1.0 - night.clamp(0.0, 1.0)) * down * down;
    range * (1.0 + (dusk_reach - 1.0) * dusk)
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
    /// about two puffs a second (`TORCH_PUFFS`), a campfire's (0.7 m) 15% at 1.8, a house's
    /// (6 m) 7% at 0.6.
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
            puffs_hz: (1.5 / libm::sqrtf(wide)).clamp(0.4, TORCH_PUFFS),
        }
    }
}

/// How much of a deep flicker a kept fire's light takes: a torch's flame strays a third from its
/// mean, its light half that, never shallower than a campfire's ([`LIGHT_DEPTH_LEAST`]), as
/// deep as its flame from there down. A light is the whole flame's sum, which strays less than
/// the flame's look: at its flame's depth every torch of a corridor pulsed its pool on the walls
/// 5-12% a frame, which read as jitter (dungeon light round, a held crypt corridor's pops).
const LIGHT_FLICKER: f32 = 0.5;
const LIGHT_DEPTH_LEAST: f32 = 0.15;

impl Flicker {
    /// How far its light strays from its mean for each of its flame's strays ([`LIGHT_FLICKER`]).
    pub fn light_share(self) -> f32 {
        (self.depth * LIGHT_FLICKER).max(self.depth.min(LIGHT_DEPTH_LEAST)) / self.depth
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// A fire's flicker's tones, as many times its puff rate, and their shares of its depth: a
/// sum, not a product, so its power sits just above its puffs, about 1.7 times their rate on
/// the whole (3 Hz for a campfire, its first tone 2.4, as filmed ones flicker, 1.8-3.8 Hz:
/// research/2026-10-25-fire-motion.md). The product of Valheim's three sines (1, 0.564 and
/// 0.758) put its power in their beats, at 0.32, 0.81, 1.19 and 2.32 Hz for a campfire, half
/// of it under 1 Hz, and through the coals and the flame's foot the whole fire pumped in place
/// at 0.8 Hz (fire round g14). Its first tone at the puff rate itself (1, 1.4 and 1.9) became
/// a baked campfire's whole flicker, 1.8 Hz, through the light on the logs and ground under its
/// see-through flame (fire round g27); the flame's own puffing is its rising field's.
const TONES: [(f32, f32); 3] = [(1.35, 0.45), (1.9, 0.33), (2.55, 0.22)];

/// The most a fire puffs a second, a torch's: its light's first tone (`TONES`) at 3 Hz, a
/// flicker over 0.3-0.4 s as The Witcher 3's torches (docs/fire.md), though by the pool-fire
/// rate its 0.1 m flame would puff at 4.7.
const TORCH_PUFFS: f32 = 2.2;

/// How far each tone's phase wanders, radians, how many times a second its wander turns, and
/// how far a fire's own rates are off another's, a share either way: so it never repeats and
/// two fires drift apart, while the wander, slow and shallow, only widens each tone (its rate
/// strays 0.3 Hz at most).
const JITTER: (f32, f32, f32) = (1.5, 0.4, 0.08);

/// Smooth value noise, -1 to 1, one cell a unit of `x`, from a fire's own number and a salt.
fn value_noise(own: u32, x: f32, salt: (u32, u32)) -> f32 {
    let cell = libm::floorf(x);
    let f = x - cell;
    let u = f * f * (3.0 - 2.0 * f);
    let at = |i: f32| terra_gen::hash::unit3(own, i as i32 as u32, salt.0, salt.1) * 2.0 - 1.0;
    at(cell) * (1.0 - u) + at(cell + 1.0) * u
}

/// A fire's own number as the hash's.
fn own(seed: f32) -> u32 {
    (seed.rem_euclid(1.0) * 65_536.0) as u32
}

/// A fire's flicker at a time, -1 to 1, from its own number: the shape every fire shares.
fn wave(seed: f32, t: f32, flicker: Flicker) -> f32 {
    let own = own(seed);
    let (wander, wander_hz, spread) = JITTER;
    TONES
        .iter()
        .enumerate()
        .map(|(i, &(rate, share))| {
            let i = i as u32;
            let mine = 1.0 + spread * (terra_gen::hash::unit3(own, i, 0x7261, 0x7465) * 2.0 - 1.0);
            let start = core::f32::consts::TAU * terra_gen::hash::unit3(own, i, 0x7068, 0x6173);
            let drift = wander * value_noise(own, t * wander_hz + i as f32 * 17.0, (0x6a69, i));
            let a = core::f32::consts::TAU * flicker.puffs_hz * rate * mine * t + start + drift;
            share * libm::sinf(a)
        })
        .sum()
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

/// How bright a blaze burns now, a share of its mean (`signal`, its `swell` in it): its slot's
/// for the shaders, which its flames' height and brightness, its embers and its char follow.
pub(super) fn glow(blaze: &Blaze, t: f32) -> f32 {
    let flicker = Flicker::of(width(blaze));
    signal(blaze.seed, t, flicker) * swell(blaze.seed, t, flicker)
}

/// How deep a campfire's swell is, a share of its mean, and how many times a second it turns.
/// Shallow: at g6's fifth the whole marched flame pumped in place, its area's sd/mean 0.25-0.36
/// against filmed campfires' 0.09-0.19 and its flicker 0.83 Hz against their 1.8-3.8
/// (research/2026-10-25-fire-motion.md); its motion is its field's rising puffs (fire round g13).
/// At 0.06 the campfire's area at dusk still varied 0.191 (fire round g15).
const SWELL: (f32, f32) = (0.03, 0.7);

/// A fire's slow swell and sink at a time, a share of its mean, from its own number: smooth
/// value noise at `SWELL`'s rate, as deep as a campfire's for a fire that flickers as deep or
/// deeper, shallower with a wider fire's shallower flicker.
pub fn swell(seed: f32, t: f32, flicker: Flicker) -> f32 {
    let depth = SWELL.0 * (flicker.depth / 0.15).min(1.0);
    1.0 + depth * value_noise(own(seed), t * SWELL.1, (0x7377, 0x656c))
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
    /// How many stops brighter it is by night (`NIGHT_STOPS`, `HEARTH_NIGHT_STOPS`, ...).
    pub night_stops: f32,
    /// How many times more it is keyed up against the dusk sky than a kept fire's pit light
    /// (`hearth_by_hour`): a hearth's top light's `HEARTH_TOP_DUSK`, a torch's lights'
    /// `torch::TORCH_DUSK`, every other's 1.
    pub dusk_gain: f32,
    /// The share of its reach it keeps at dusk (`range_by_hour`): a hearth's top light's
    /// `HEARTH_TOP_DUSK_REACH`, every other's 1.
    pub dusk_reach: f32,
    /// Whether it may cast shadows: a hearth's top light does not (`choose`); each of a cube's
    /// six views is drawn every frame, 0.55 ms of GPU and 0.6 of render CPU (fire round g29).
    pub casts: bool,
    /// A kept fire's light as it shines unshadowed, steady (`shadows::shone`).
    pub still: Option<shadows::Still>,
}

/// The lights the fires want now, the jets' first (`jets`), then the kept fires', then each
/// cell's.
pub(super) fn lit(blazes: &Blazes, t: f32, tier: super::BreathTier) -> Vec<Lit> {
    // The jets' lights by the clock, the flames' by the time they move by, so a light flickers
    // with the glow its flames are drawn by (`fire::show`).
    let mut lit = jet_lights(blazes, t, tier);
    let t = super::flame_time(t);
    let up = |at: Vec3| at.normalize_or(Vec3::Y);
    // A hearth's key hangs in the body of its flames, clear of its logs' ends, so it lights the
    // ground round it and its glow in the air (`fog`) is a halo round the flames: at 0.8 of
    // their height the glow was a ball beside the fire (fire sheet review, 3/10); level with a
    // 0.9 m teepee's ends the logs were the brightest thing by it (lake film r6). An unshadowed
    // light at its flames' top lights the faces sat by it (`HEARTH_TOP`); a torch has its own.
    let colours = blazes.lights.iter().chain(std::iter::repeat(&HEARTH_LIGHT));
    let rigs = blazes.rigs.iter().chain(std::iter::repeat(&Rig::Hearth));
    lit.extend(
        blazes
            .kept
            .iter()
            .zip(colours.zip(rigs))
            .enumerate()
            .flat_map(|(fire, (blaze, (&colour, &rig)))| {
                let flicker = Flicker::of(width(blaze));
                let glow = glow(blaze, t);
                let reach = (blaze.tall * SWAY_A_METRE).clamp(SWAY_M[0], SWAY_M[1]);
                // Its light strays from its mean less than its flame does (`LIGHT_FLICKER`).
                let shines = 1.0 + (glow - 1.0) * flicker.light_share();
                let lumens = blaze.tall * blaze.heat * HEARTH_LUMENS_A_METRE * shines;
                // It sways up from where it hangs, never down: swayed a hand's breadth below, the
                // shadowed light passed under a log's end, whose shadow ballooned for a fifth of a
                // second into a hard black wedge over the ground (fire round 30, clip frames 198-208).
                let swayed = sway(blaze.seed, t, flicker, reach);
                let rise = swayed.dot(up(blaze.at));
                let swayed = swayed + up(blaze.at) * (rise.abs() - rise);
                // Where it hangs, unswayed and at its mean (`shadows::shone`).
                let still = |at| Some(shadows::Still { at, shines, fire });
                let rest = blazes.hangs(fire, blaze, HEARTH_LIGHT_UP);
                let main = Lit {
                    at: rest + swayed,
                    lumens: lumens * (1.0 - HEARTH_TOP),
                    color: colour,
                    range: HEARTH_RANGE_M,
                    // Its flames as a disc light (`crate::falloff`), wider than they are
                    // (`HEARTH_SPREAD`): its fall-off levels off over the stones at its foot.
                    radius: width(blaze) * HEARTH_SPREAD,
                    glow,
                    kept: true,
                    night_stops: HEARTH_NIGHT_STOPS,
                    dusk_gain: HEARTH_KEY_DUSK,
                    dusk_reach: 1.0,
                    casts: true,
                    still: still(rest),
                };
                if rig == Rig::Torch {
                    let hang = |share| blazes.hangs(fire, blaze, share);
                    return torch::lights(&main, blaze, shines, swayed, hang, still);
                }
                let top_rest = blazes.hangs(fire, blaze, HEARTH_TOP_UP);
                let top = Lit {
                    lumens: lumens * HEARTH_TOP * HEARTH_TOP_GAIN,
                    at: top_rest + swayed,
                    range: HEARTH_TOP_RANGE_M,
                    radius: width(blaze) * HEARTH_TOP_SPREAD,
                    night_stops: HEARTH_TOP_NIGHT_STOPS,
                    dusk_gain: HEARTH_TOP_DUSK,
                    dusk_reach: (blaze.tall * HEARTH_TOP_DUSK_REACH / HEARTH_TOP_RANGE_M).min(1.0),
                    casts: false,
                    still: still(top_rest),
                    ..main.clone()
                };
                [main, top]
            }),
    );
    lit.extend(gathered(blazes, t));
    lit
}

/// The wild blazes' lights, gathered over cells, the brightest first; `t` the time the flames
/// move by.
fn gathered(blazes: &Blazes, t: f32) -> Vec<Lit> {
    // Each cell: its weighted place, its weight, its weighted glow, its widest flame, its
    // heaviest blaze's number and the way its burning faces look, weighted.
    type Cell = (Vec3, f32, f32, f32, (f32, f32), Vec3);
    let mut cells: std::collections::BTreeMap<[i64; 3], Cell> = std::collections::BTreeMap::new();
    // Only the wild blazes are gathered: a kept fire past the nearest `HEARTHS` (a dungeon has
    // thirty torches) lights as a kept fire still. Gathered, the far torches were lit as
    // blazes, each cell some five torches' light hung 1.5 m up and 3 m out from the middle of
    // all that burns, in the rock or the next corridor: 75% of the crypt's fire light and 5
    // of its 12 lights, moving each time the nearest sixteen changed (dungeon light round).
    for blaze in &blazes.blazes {
        let weight = blaze.tall * blaze.heat;
        let key = (blaze.at / LIGHT_CELL_M).floor().as_i64vec3().to_array();
        let cell = cells
            .entry(key)
            .or_insert((Vec3::ZERO, 0.0, 0.0, 0.0, (0.0, 0.0), Vec3::ZERO));
        cell.0 += blaze.at * weight;
        cell.1 += weight;
        cell.2 += glow(blaze, t) * weight;
        cell.3 = cell.3.max(width(blaze));
        if weight > cell.4.0 {
            cell.4 = (weight, blaze.seed);
        }
        cell.5 += blaze.out * weight;
    }
    let up = |at: Vec3| at.normalize_or(Vec3::Y);
    // A gathering whose blazes face no one way (`facing`) hangs its light out from the middle
    // of all that burns, on the outside of the walls its flames stand on: at the gathering's
    // own middle, inside a burning building, it lit the inner faces and none of the stone the
    // eye sees (castle film r8).
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
        .map(|(sum, weight, glows, wide, (_, seed), out)| {
            let at = sum / weight;
            // The cell's light is the sum of its fires', each at its own flicker; it sways as
            // its biggest fire does, as far as a flame of the cell's whole height would.
            let glow = glows / weight;
            let tall = weight.sqrt();
            let reach = (tall * SWAY_A_METRE).clamp(SWAY_M[0], SWAY_M[1]);
            let hung = match facing(at, out, weight) {
                Some(face) => at + face * FACE_OUT_M + up(at) * FACE_UP_M,
                None => at + outward(at) + up(at) * 1.5,
            };
            Lit {
                at: hung + sway(seed, t, Flicker::of(wide), reach),
                lumens: (weight * LUMENS_A_METRE).min(MOST_LUMENS) * glow,
                color: BLAZE_LIGHT,
                range: LIGHT_RANGE_M,
                radius: (wide * 0.5).min(2.0),
                glow,
                kept: false,
                night_stops: NIGHT_STOPS,
                dusk_gain: 1.0,
                dusk_reach: 1.0,
                casts: true,
                still: None,
            }
        })
        .collect();
    gathered.sort_by(|a, b| b.lumens.total_cmp(&a.lumens));
    gathered
}

/// How many fire lights cast shadows, by preset: one in play, four in films (docs/fire.md). A
/// shadowed light is a cube of six shadow views: in a crypt hall of four torches each past the
/// first cost about 0.6 ms of GPU and 0.7 of render CPU on the RTX 4090, so play keeps one.
pub fn shadowed(quality: Quality) -> usize {
    match quality {
        Quality::Low | Quality::Medium | Quality::High => 1,
        Quality::Ultra => 4,
    }
}

/// What one of the light entities was last frame: where, and whether it cast shadows or
/// shone in the fog (`near`, held on to as shadows are).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Was {
    pub at: Vec3,
    pub on: bool,
    pub near: bool,
}

/// A light to show: what it is, how far its distance has eased it, whether it casts shadows,
/// and whether it shines in the fog (a kept fire's near light, shadowed or not).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Shown {
    pub lit: Lit,
    pub fade: f32,
    pub shadows: bool,
    pub scatters: bool,
}

fn smooth(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `choose_within` at play's distances.
#[cfg(test)]
pub(super) fn choose(
    lit: Vec<Lit>,
    eye: Option<Vec3>,
    was: &[Was],
    shadows: usize,
) -> Vec<Option<Shown>> {
    choose_within(lit, eye, was, shadows, 1.0)
}

/// How many times play's distance a fire's light reaches before it eases out, by preset: three
/// in films, as the cinematic tier's distances are (docs/fire.md): at play's 40 m the castle
/// fall's cameras, 35 to 50 m off, saw every burning wall's light eased out (castle round 4).
pub fn light_reach(quality: Quality) -> f32 {
    match quality {
        Quality::Low | Quality::Medium | Quality::High => 1.0,
        Quality::Ultra => 3.0,
    }
}

/// The lights to show on each light entity: the most a fire's light matters where the eye is
/// (its lumens over its distance, eased out from 30 to 40 m, `reach` times that in a film,
/// `light_reach`), each kept on the entity that showed the nearest of last frame's, so a
/// light's shadows stay on it as fires come and go; of the lights that may cast them
/// (`Lit::casts`) the nearest `shadows` within 20 m cast shadows, kept to 23 m once on, and as
/// many of the rest, a kept fire's, shine in the fog.
pub(super) fn choose_within(
    lit: Vec<Lit>,
    eye: Option<Vec3>,
    was: &[Was],
    shadows: usize,
    reach: f32,
) -> Vec<Option<Shown>> {
    let far = |at: Vec3| eye.map_or(0.0, |eye| eye.distance(at));
    let mut wanted: Vec<(f32, Shown)> = lit
        .into_iter()
        .filter_map(|lit| {
            let d = far(lit.at);
            let fade = 1.0 - smooth(LIGHT_FADE_M[0] * reach, LIGHT_FADE_M[1] * reach, d);
            (fade > 0.0).then(|| {
                let weight = lit.lumens * fade / (1.0 + (d / 10.0).powi(2));
                (
                    weight,
                    Shown {
                        lit,
                        fade,
                        shadows: false,
                        scatters: false,
                    },
                )
            })
        })
        .collect();
    wanted.sort_by(|a, b| b.0.total_cmp(&a.0));
    // Past the last light there is room for, the lights nearest the cut ease out as their
    // weight comes down to the first one left out's, so a light is never cut while it shows:
    // cut at once, the light on a far wall blinked each time two torches traded places.
    if let Some(&(cut, _)) = wanted.get(was.len()) {
        for (weight, shown) in &mut wanted {
            shown.fade *= smooth(cut, cut * CUT_EASE, *weight);
        }
    }
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
            let had = was[i].near;
            let within = if had { SHADOW_M[1] } else { SHADOW_M[0] };
            (eye.is_some() && d < within).then_some((d - if had { 3.0 } else { 0.0 }, i))
        })
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (mut casting, mut glowing) = (0, 0);
    for &(_, i) in &near {
        let Some(shown) = out[i].as_mut() else {
            continue;
        };
        if shown.lit.casts {
            if casting < shadows {
                casting += 1;
                shown.shadows = true;
                shown.scatters = shown.lit.kept;
            }
        } else if shown.lit.kept && glowing < shadows {
            glowing += 1;
            shown.scatters = true;
        }
    }
    out
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
    mut lights: shadows::Lights,
    mut glows: ResMut<FireGlows>,
    mut fog: ResMut<super::fog::Glows>,
    tier: Res<super::BreathTier>,
    sky_light: Option<Res<crate::skylight::SkyLight>>,
    shut_off: Option<Res<crate::indoor::ShutOff>>,
    mut shadow_fade: Local<ShadowFade>,
) {
    let t = time.elapsed_secs_wrapped();
    // Whether a light had shadows, for `choose` to hold them on it: whether it wanted them
    // (one waiting for another's to fade has none yet), or shone in the fog unshadowed.
    let was: Vec<Was> = drawn
        .lights
        .iter()
        .enumerate()
        .map(|(i, &entity)| {
            let wanted = shadow_fade.wanted.get(i).copied().unwrap_or(false);
            lights.get(entity).map_or(
                Was {
                    at: Vec3::ZERO,
                    on: false,
                    near: false,
                },
                |(light, transform, visibility, volumetric)| Was {
                    at: transform.translation,
                    on: *visibility != Visibility::Hidden,
                    near: wanted || (volumetric && !light.shadow_maps_enabled),
                },
            )
        })
        .collect();
    let eye = camera.iter().next().map(GlobalTransform::translation);
    let mut wants = flashes(&blazes, &drawn.jets, t);
    wants.extend(strikes(&blazes, &drawn.jets, t, *tier));
    wants.extend(lit(&blazes, t, *tier));
    // In a room shut off from the sky the fires burn as by night: nothing outshines them (D488).
    let shut_off = shut_off.map_or(0.0, |s| s.0);
    let night = crate::indoor::fires_night(sky.as_ref().map_or(0.0, |sky| sky.night), shut_off);
    let twilight = sky.as_ref().map_or(0.0, |sky| sky.twilight);
    let sky_light = sky_light.map_or(0.0, |lit| lit.brightness);
    // Each light is brighter by night by its own stops: a kept fire's fewer than a blaze's, so
    // its flame stays the brightest thing by it (D415), and a marched breath's fewer again.
    // Weighed so before the lights are chosen: weighed by their day's lumens, a torch's
    // shadowed key (0.4 of its light by day, half of it by night) lost its light to the top
    // light and the blazes, and a dungeon's torches cast shadows 0.36 lights a frame.
    for want in &mut wants {
        if want.kept {
            want.lumens *=
                hearth_by_hour(want.night_stops, want.dusk_gain, night, twilight, sky_light);
            want.range = range_by_hour(want.range, want.dusk_reach, night, twilight);
        } else {
            want.lumens *= by_night(want.night_stops, night);
        }
    }
    shut::give_top_to_key(&mut wants, shut_off);
    let shown = choose_within(wants, eye, &was, shadowed(*quality), light_reach(*quality));
    shadow_fade.ease(&shown, eye, shadowed(*quality), time.delta_secs());
    shadow_fade.spawn_twins(&mut commands);
    let mut twins = Vec::new();
    let mut told = [[0.0f32; 4]; GLOWS];
    let mut wanted = Vec::new();
    let shone = shadows::shone(&shown, &shadow_fade.strength);
    for (i, ((&entity, shown), shone)) in drawn.lights.iter().zip(shown).zip(shone).enumerate() {
        let Ok((mut light, mut transform, mut visibility, volumetric)) = lights.get_mut(entity)
        else {
            continue;
        };
        match shown {
            Some(Shown {
                lit,
                fade,
                scatters,
                ..
            }) => {
                // Its shadows part-way, the rest of its light is its twin's (`shadows`).
                let strength = shadow_fade.strength.get(i).copied().unwrap_or(0.0);
                let Some(shadows::Shone { own, twin }) = shone else {
                    continue;
                };
                if let Some(twin) = twin {
                    let lumens = twin.lumens;
                    twins.push((twin, lumens));
                }
                light.intensity = own.lumens;
                light.range = lit.range;
                light.color = lit.color;
                light.radius = lit.radius;
                transform.translation = own.at;
                *visibility = Visibility::Inherited;
                light.shadow_maps_enabled = strength > 0.0;
                // A kept fire's near lights shine in the air round it (`fog`), the shadowed
                // one sampling its shadows there, eased with them; a blaze's stays out of the
                // fog, where its tens of millions of lumens lit a ball of air brighter than the
                // house (round 6).
                let scatters = if lit.casts {
                    lit.kept && strength > 0.0
                } else {
                    scatters
                };
                if scatters != volumetric {
                    if scatters {
                        commands.entity(entity).insert(VolumetricLight);
                    } else {
                        commands.entity(entity).remove::<VolumetricLight>();
                    }
                }
                // One glow a fire, round its top light, which its pit key shines in as well:
                // each glow the eye is inside is marched over the whole screen, 3.6 ms by night
                // a glow at a campfire 4.4 m off (fire round g29).
                if scatters && !lit.casts {
                    wanted.push(super::fog::Glow::of(own.at, lit.radius, fade));
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
    shadow_fade.show_twins(&twins, &mut lights);
    if told != glows.glows {
        glows.glows = told;
    }
    if fog.0 != wanted {
        fog.0 = wanted;
    }
}

#[cfg(test)]
mod face_tests;
#[cfg(test)]
mod pool_tests;
#[cfg(test)]
mod tests;
