//! A pitch torch's light rig (its colour, `TORCH_LIGHT`): one shadowed key hung just over its
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
/// The share of its light at its flames' tips, unshadowed, and how far that reaches.
const TORCH_TIP: f32 = 0.15;
const TORCH_TIP_RANGE_M: f32 = 3.5;

/// A torch's two lights: `hearth` is the hearth rig's key as it would be (its colour, glow and
/// sway), `hang` where a share of its flames' height hangs, `still` its steady twin's.
pub(super) fn lights(
    hearth: &Lit,
    blaze: &Blaze,
    shines: f32,
    swayed: Vec3,
    hang: impl Fn(f32) -> Vec3,
    still: impl Fn(Vec3) -> Option<shadows::Still>,
) -> [Lit; 2] {
    let lumens = blaze.tall * blaze.heat * TORCH_LUMENS_A_METRE * shines;
    let rest = hang(TORCH_LIGHT_UP);
    let key = Lit {
        at: rest + swayed,
        lumens: lumens * (1.0 - TORCH_TIP),
        range: TORCH_RANGE_M,
        radius: TORCH_RADIUS_M,
        night_stops: TORCH_NIGHT_STOPS,
        dusk_gain: 1.0,
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
