//! Each moving clip's own pace: how fast the ground must pass under a body for its feet not
//! to slide, measured by the tools and kept in the body's file (`tools/characters/pace.py`):
//! `{"paces": {"Jog_Fwd_Loop": 5.51}}` in its glTF extras. A clip is played at the speed the
//! character moves divided by its pace, as every engine's locomotion does, so a planted foot
//! stays where it was put.

use std::collections::HashMap;

use bevy::gltf::{Gltf, GltfNode};
use bevy::prelude::*;

/// The slowest and fastest a clip is played, against its own speed: past these it looks
/// wrong whatever its feet do, and another clip should be playing.
const SLOWEST: f32 = 0.5;
const FASTEST: f32 = 1.6;

/// The paces kept in a body file, by clip, from whichever of its nodes carries them.
pub fn paces(gltf: &Gltf, nodes: &Assets<GltfNode>) -> HashMap<String, f32> {
    gltf.nodes
        .iter()
        .filter_map(|node| nodes.get(node)?.extras.as_ref())
        .find_map(|extras| {
            let value: serde_json::Value = serde_json::from_str(&extras.value).ok()?;
            serde_json::from_value(value.get("paces")?.clone()).ok()
        })
        .unwrap_or_default()
}

/// How fast to play a clip of `pace` (metres a second, on a body of the base's height) for a
/// body `scale` times that height moving at `speed`.
pub fn rate(speed: f32, pace: f32, scale: f32) -> f32 {
    if pace <= 0.0 || scale <= 0.0 {
        return 1.0;
    }
    (speed / (pace * scale)).clamp(SLOWEST, FASTEST)
}

/// The longest a stride is warped, against its clip's: past it a leg reaches too far, and
/// another clip should be playing. A stride is only lengthened: a body slower than its clip
/// plays at the slowest has a slower gait's clip to play (Lyra's walk under its jog), and its
/// locked feet hold it meanwhile (a start, a landing's slowing).
const LONGEST_STRIDE: f32 = 1.4;

/// How a stride clip is shown at a speed: played `rate` times as fast and its feet's travel
/// along the way stretched `warp` times (stride warping), so the ground its planted feet stand
/// on passes at `rate * warp` times its pace.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gait {
    pub rate: f32,
    pub warp: f32,
}

impl Gait {
    /// Played as made.
    pub const AS_MADE: Gait = Gait {
        rate: 1.0,
        warp: 1.0,
    };
}

/// How to show a stride clip of `pace` (metres a second, on a body of the base's height) for a
/// body `scale` times that height moving at `speed`, as Lyra matches a cycle to the capsule's
/// speed: played as fast as the speed asks within `SLOWEST`-`FASTEST` first, and what the play
/// rate cannot cover taken by warping the stride (Unreal's Stride Warping, its documentation:
/// stride scale = locomotion speed over the animation's), up to `LONGEST_STRIDE`. A man's sprint at 7.4 m/s from a 3.9 m/s clip of 2.4 steps a second of
/// 1.6 m is shown at about 3.8 steps a second of 1.9 m, a real sprinter's; a jog within the
/// play rate is not warped. Standing still plays slowest, unwarped.
pub fn gait(speed: f32, pace: f32, scale: f32) -> Gait {
    if pace <= 0.0 || scale <= 0.0 {
        return Gait::AS_MADE;
    }
    let need = speed / (pace * scale);
    if need <= 0.0 {
        return Gait {
            rate: SLOWEST,
            warp: 1.0,
        };
    }
    let rate = need.clamp(SLOWEST, FASTEST);
    Gait {
        rate,
        warp: (need / rate).clamp(1.0, LONGEST_STRIDE),
    }
}

/// How far from as made a stride clip of `pace` is shown at `speed` (`gait`), the larger of
/// its play rate's and its stride warp's distance from 1, or of what neither covers (the
/// ground passing faster or slower than the shown stride, the lock's to hide): 0 for a clip
/// made at the gait's speed.
pub fn stretch(speed: f32, pace: f32, scale: f32) -> f32 {
    if pace <= 0.0 || scale <= 0.0 {
        return f32::INFINITY;
    }
    let shown = gait(speed, pace, scale);
    let left = speed / (pace * scale * shown.rate * shown.warp);
    [shown.rate, shown.warp, left]
        .into_iter()
        .map(|x| (x - 1.0).abs())
        .fold(0.0, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `pace.match` (docs/controller.md criterion 21c): each gait's clip in the figure's file
    /// against the speed a man moves while it plays, the worst `stretch`; printed for the
    /// controller's scorecard.
    #[test]
    fn the_gaits_stride_as_fast_as_a_man_moves() {
        let clips = crate::model_files::json(crate::figure::CLIPS);
        let paces: HashMap<String, f32> =
            serde_json::from_value(crate::model_files::extra(&clips, "paces").unwrap()).unwrap();
        let man = crate::movement::man();
        let mut worst = 0.0f32;
        for (clip, speed) in [("Jog_Fwd_Loop", man.jog), ("Sprint_Loop", man.sprint)] {
            let pace = paces.get(clip).copied().unwrap_or(0.0);
            let shown = gait(speed, pace, 1.0);
            let each = stretch(speed, pace, 1.0);
            println!(
                "pace\t{clip} {pace} m/s at {speed}: rate {:.2}, stride x{:.2}, {each:.3}",
                shown.rate, shown.warp
            );
            worst = worst.max(each);
        }
        println!("scorecard\tpace.match\t{worst}");
        assert!(worst.is_finite(), "a gait with no pace");
    }

    #[test]
    fn a_stride_is_played_faster_first_and_warped_for_the_rest() {
        // A clip made at the speed is shown as made.
        assert_eq!(gait(4.0, 4.0, 1.0), Gait::AS_MADE);
        assert_eq!(stretch(4.0, 4.0, 1.0), 0.0);
        // Within the play rate's span, played faster or slower, never warped.
        assert_eq!(gait(5.0, 4.0, 1.0).warp, 1.0);
        assert!((gait(5.0, 4.0, 1.0).rate - 1.25).abs() < 1e-5);
        // A sprint at nearly twice its clip's pace: played as fast as it may be, its stride
        // warped the rest, matching the speed.
        let sprint = gait(7.4, 3.9, 1.0);
        assert_eq!(sprint.rate, FASTEST);
        assert!(
            sprint.warp > 1.1 && sprint.warp < LONGEST_STRIDE,
            "{sprint:?}"
        );
        assert!((3.9 * sprint.rate * sprint.warp - 7.4).abs() < 1e-4);
        assert!(stretch(7.4, 3.9, 1.0) <= FASTEST - 1.0 + 1e-5);
        // Past both, the ground passes faster than the stride shown and the stretch says so.
        let fast = gait(FASTEST * LONGEST_STRIDE * 1.5, 1.0, 1.0);
        assert_eq!((fast.rate, fast.warp), (FASTEST, LONGEST_STRIDE));
        assert!(stretch(FASTEST * LONGEST_STRIDE * 1.5, 1.0, 1.0) >= 0.5 - 1e-5);
        // A taller body takes longer strides: the same speed asks less of its clip.
        assert!(gait(7.4, 3.9, 1.1).warp < gait(7.4, 3.9, 1.0).warp);
        // Standing still plays slowest, unwarped; no pace is no stride to show.
        assert_eq!(gait(0.0, 3.9, 1.0).warp, 1.0);
        assert_eq!(gait(3.0, 0.0, 1.0), Gait::AS_MADE);
        assert_eq!(stretch(3.0, 0.0, 1.0), f32::INFINITY);
    }

    #[test]
    fn a_clip_plays_as_fast_as_the_ground_passes_under_it() {
        // A jog made for 5.5 m/s, run at 4.4 m/s, plays at four fifths.
        assert!((rate(4.4, 5.5, 1.0) - 0.8).abs() < 1e-5);
        // A body a tenth taller takes longer strides: the same speed plays it slower.
        assert!(rate(4.4, 5.5, 1.1) < rate(4.4, 5.5, 1.0));
    }

    #[test]
    fn the_base_bodys_paces_are_for_its_clips() {
        let base = crate::model_files::json(crate::appearance::BASE);
        // As `paces` reads them, which gives none when they do not read.
        let paces: HashMap<String, f32> = serde_json::from_value(
            crate::model_files::extra(&base, "paces").expect("the base body carries no paces"),
        )
        .unwrap();
        assert!(!paces.is_empty());
        let clips = crate::model_files::animations(&base);
        for (clip, pace) in &paces {
            assert!(clips.contains(clip), "a pace for no clip {clip}");
            assert!(*pace > 0.0, "{clip}");
        }
    }

    #[test]
    fn it_is_never_played_absurdly_slow_or_fast() {
        assert_eq!(rate(0.1, 5.5, 1.0), SLOWEST);
        assert_eq!(rate(20.0, 5.5, 1.0), FASTEST);
        assert_eq!(rate(3.0, 0.0, 1.0), 1.0);
    }
}
