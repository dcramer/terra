//! A pitch torch's light rig (`Rig::Torch`): one shadowed key hung just over its
//! head, short in reach, so it lights a pool round the torch's foot that falls off a few
//! metres out, and a dim unshadowed light at its flames' tips for the wall or the faces over
//! it. Shipped games light a torch with one point light at its head, short in range (Skyrim's
//! wall torches, about 4-6 m; Valheim's standing torch), not a campfire's rig: lit as the
//! campfire is, a high wide top light reaching 10 m, the staged torch's ground took an even
//! tint to the horizon (torch round t3, all six judges: "no light at night").

use super::{Blaze, Lit, shadows};
use bevy::prelude::*;

/// A torch's light, lumens for a metre of flame at full heat: a little under a hearth's two
/// lights' together (60,000 a metre at 0.4 and 1.8 of it), as all of it now hangs low.
const TORCH_LUMENS_A_METRE: f32 = 110_000.0;
/// How far up its flames the key hangs: just over the head, in the flame's foot.
const TORCH_LIGHT_UP: f32 = 0.3;
/// How far the key reaches, metres: a pool, not the yard (a hearth's top light reaches 10).
const TORCH_RANGE_M: f32 = 7.0;
/// Its key as a disc so wide, metres (`crate::falloff`): wide enough that a wall a hand from
/// the flame does not blow out to white (the crypt's wall torches at the flame's 0.12 m width,
/// torch round t3), narrow against the 1.4 m down to the ground, so the pool still falls off
/// from the foot.
const TORCH_RADIUS_M: f32 = 0.45;
/// How many stops brighter it is by night: fewer than a hearth's pit light (3.6), its light
/// all in one key.
const TORCH_NIGHT_STOPS: f32 = 3.0;
/// How many times more its key is keyed up against the dusk sky than a hearth's pit light
/// (`Lit::dusk_gain`), giving way to its night stops as the night comes: at 1 the staked
/// torch's pool at dusk took half the light the hearth's rig gave it, whose top light's gain is
/// 9 (`HEARTH_TOP_DUSK`; near its foot 0.066 against 0.128, torch round t4). At 6 its foot takes
/// a little more than the hearth's rig gave it at the yard's dusk, all of it from low, so it
/// still falls off as a pool.
const TORCH_DUSK: f32 = 6.0;
/// The share of its light at its flames' tips, unshadowed, and how far that reaches.
const TORCH_TIP: f32 = 0.15;
const TORCH_TIP_RANGE_M: f32 = 3.5;
/// How far a torch carried in hand reaches (`Rig::Held`), metres: twice a staked one's. A
/// light's range is where its inverse-square light is windowed to nothing, and at 7 m the
/// window had taken most of it by 4 m: at midnight in the meadow the ground 4 m ahead of the
/// holder took a twentieth of the light at their feet and the ground 6 m ahead 1.3 times the
/// moonlit night's, so the pool ended three paces out (tools/sandbox/torchlight.ts, torch-light
/// j1; the developers: "the light from the torch isnt amazing"). Skyrim's held torch reaches
/// 512 units, 7.3 m, with no such window inside it, and its best-used lighting mods take it to
/// 12-13 m; Valheim's is cited at 10 m by its mods.
pub(super) const HELD_RANGE_M: f32 = 14.0;
/// How many times a staked torch's light a carried one gives, flame for flame: its flames are
/// two thirds a wall torch's, and at a staked torch's light the ground 6 m out stood under
/// the moon's with the window gone.
const HELD_GAIN: f32 = 1.5;
/// How far up its flames a carried torch's key hangs: 0.3 m over a staked one's on a hand
/// torch's 0.3 m flame, over the holder's head. In the flame's foot it hung at the holder's
/// shoulder, 0.3 m from their body, and their shadow lay 0.81 to 1.15 m wide on the ground
/// 1.5 m from their feet (nine stills at midnight in the meadow); 0.3 m higher it is 0.24 m
/// wide, 0.5 m higher 0.13 m (torch-light j3). Games leave the holder out of a held torch's
/// shadows or hang its light clear of them; this keeps their shadow and shortens it.
const HELD_LIGHT_UP: f32 = 1.3;

/// A torch's two lights: `hearth` is the hearth rig's key as it would be (its colour, glow and
/// sway), `hang` where a share of its flames' height hangs, `still` its steady twin's, `held`
/// whether it is carried in hand (`Rig::Held`).
pub(super) fn lights(
    hearth: &Lit,
    blaze: &Blaze,
    shines: f32,
    swayed: Vec3,
    hang: impl Fn(f32) -> Vec3,
    still: impl Fn(Vec3) -> Option<shadows::Still>,
    held: bool,
) -> [Lit; 2] {
    let (gain, range, up) = if held {
        (HELD_GAIN, HELD_RANGE_M, HELD_LIGHT_UP)
    } else {
        (1.0, TORCH_RANGE_M, TORCH_LIGHT_UP)
    };
    let lumens = blaze.tall * blaze.heat * TORCH_LUMENS_A_METRE * shines * gain;
    let rest = hang(up);
    let key = Lit {
        at: rest + swayed,
        lumens: lumens * (1.0 - TORCH_TIP),
        range,
        radius: TORCH_RADIUS_M,
        night_stops: TORCH_NIGHT_STOPS,
        dusk_gain: TORCH_DUSK,
        dusk_reach: 1.0,
        casts: true,
        still: still(rest),
        ..hearth.clone()
    };
    let tip_rest = hang(1.0);
    let tip = Lit {
        at: tip_rest + swayed,
        lumens: lumens * TORCH_TIP,
        range: TORCH_TIP_RANGE_M,
        casts: false,
        still: still(tip_rest),
        ..key.clone()
    };
    [key, tip]
}
