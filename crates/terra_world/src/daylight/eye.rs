//! The camera's exposure by the place round the eye: opened in a far-seeing world's open air
//! (D338), under the trees by night (D310), kept in a room shut off from the sky (D488).

use terra_gen::smoothstep;

/// How many stops brighter the eye is set by day out in the open on a world whose views reach
/// kilometres (D338). The day's exposures were tuned under the lush forest's canopy against
/// Sons of the Forest; out over the mountain lake at noon they left the sky at 0.17 where the
/// references' is 0.73, and the whole frame at 0.24 against 0.39. A stop and a half brighter
/// measured 0.55 and 0.45, as a photographer opens up for a bright open view a forest's
/// meter would stop down for.
pub(super) const OPEN_EYE_EV: f32 = 1.5;
/// And by night, on the open moonlit land of a far-seeing world (fire round 24): with the deep
/// night's moon and ambient at 0.6 (`keys`), tuned for the yard's campfire to sit in a pool of
/// its own light, the town's street and the mountain lake's shore went to silhouettes (the lake
/// at mean sRGB 2-5, every pixel under 8%). The small planet's yard, where the campfire's pool
/// is judged, keeps the key. With the ambient at a third (round 25), 1.45, under the day's: at
/// 1.6 the town's street was mean sRGB 14, 84% under 8%, against 18/69%, the lake 5.8 v 4.4.
pub(super) const OPEN_NIGHT_EV: f32 = 1.45;

/// How many stops the eye opens by night under a closed canopy, as an eye dark-adapts and as
/// Unreal's auto-exposure falls to its minimum EV in a dark wood. The key (fire round 25) is
/// set for the open yard round a campfire; under the crowns, where the moon reaches only in
/// flecks and the sky's light is hidden, it left the forest walk's path at mean sRGB 0.000
/// and its crowns at 0.003 against Sons of the Forest's night 0.075 (night round, 2026-10-25):
/// at 1.5, 2.5 and 3.5 stops the path measured 0.003, 0.013 and 0.053 and the crowns 0.009,
/// 0.026 and 0.079. The haze, which Bevy mixes in after the exposure, is raised with it.
pub(super) const WOODED_NIGHT_EV: f32 = 3.5;
/// How much less the eye opens under the crowns by night with the snow lying: at the full
/// 3.5 stops the winter walk's night stood at mean sRGB 0.23, brighter than Sons of the
/// Forest's moonlit frames (0.08-0.16); without them, 0.04.
pub(super) const SNOW_DARK_ADAPTS: f32 = 0.6;

/// The camera's exposure for the hour's `key` (a shut room's already, `indoor::key`), with the
/// sky so `open` over the eye, on a world that sees far or not, so far into the night and
/// `outdoors` (shut off, the night's, D488): the key in a forest by day; under trees opened by night; out in a far-seeing world's air, less.
/// `open` is `OpenOverhead`, None where no canopy is worked out round the eye (the yard, a world
/// with no trees): the sky open there, as `SkySeen` reads it, so its night keeps the key.
pub(super) fn open_exposure(
    key: f32,
    open: Option<f32>,
    far_seeing: bool,
    night: f32,
    outdoors: f32,
) -> f32 {
    let night = night.clamp(0.0, 1.0);
    let wooded = key - wooded_night_ev(open, night, outdoors);
    if !far_seeing {
        return wooded;
    }
    let open = smoothstep(0.55, 0.98, open.unwrap_or(1.0));
    wooded - open * (OPEN_EYE_EV * (1.0 - night) + OPEN_NIGHT_EV * night)
}

/// How many stops the eye opens by night under the trees, so far into the night with the sky
/// so `open` over it: none in the open, where the moon lights the land, all of [`WOODED_NIGHT_EV`]
/// under a closed canopy; none in a closed room (`outdoors`, `IndoorSky` 0), its dark meant and
/// its own fires setting the eye (D485), and in a room as much as its openings let in (D486).
/// None where no canopy is worked out (`open` None): the forest's haze takes such a place as
/// wooded (D273), but its night is the open yard's, which the key was set for (fire round 25).
pub(super) fn wooded_night_ev(open: Option<f32>, night: f32, outdoors: f32) -> f32 {
    let Some(open) = open else { return 0.0 };
    let dark = night.clamp(0.0, 1.0) * outdoors.clamp(0.0, 1.0);
    (1.0 - smoothstep(0.55, 0.98, open)) * dark * WOODED_NIGHT_EV
}
