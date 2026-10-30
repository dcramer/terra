//! Each moving clip's own pace: how fast the ground must pass under a body for its feet not
//! to slide, measured by the tools and kept in the body's file (`tools/characters/pace.py`):
//! `{"paces": {"Jog_Fwd_Loop": 5.51}}` in its glTF extras. A clip is played at the speed the
//! character moves divided by its pace, as every engine's locomotion does, so a planted foot
//! stays where it was put.

use std::collections::HashMap;

use bevy::gltf::{Gltf, GltfNode};
use bevy::prelude::*;

/// How far a gait's clip may be stretched to its speed (docs/animation.md rule 1, D585): at the
/// speed its gait moves a body, a clip plays at most this share faster or slower than made and
/// its stride is warped at most this share longer (Unreal's stride warping keeps to this
/// range). A speed past both wants its own clip, or the game's speed fitted to the clip
/// (project-whiskeyjack moved its jog rather than stretch it), never a longer stretch:
/// `matched` says which clip and speed break it.
pub const MATCHED: f32 = 0.1;

/// The slowest and fastest a clip is played, against its own speed, between its gait's speeds
/// alone (a start, a stop, a change of gait): past these it looks wrong whatever its feet do,
/// and another clip should be playing.
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

/// A figure's size against its model, the `scale` its paces are taken at, from its animation
/// player's `place` in the world and that node's `own` transform. A pace is measured in its
/// model file's own world, through the armature object's scale (`tools/characters/pace.py`),
/// and Bevy's glTF loader puts the animation player on the scene's root node, the armature,
/// whose transform carries that scale (base.glb's Rig 1.078, the Cetri's 3.296): the player's
/// world scale holds it a second time, so it is taken out.
pub fn size(place: &GlobalTransform, own: &Transform) -> f32 {
    let own = own.scale.y;
    if own <= 0.0 {
        return place.scale().y;
    }
    place.scale().y / own
}

/// The longest a stride is warped, against its clip's: past it a leg reaches too far, and
/// another clip should be playing (`MATCHED`; it was 1.4, and the sprint's legs bent). A stride
/// is only lengthened: a body slower than its clip plays at the slowest has a slower gait's
/// clip to play (Lyra's walk under its jog), and its locked feet hold it meanwhile (a start, a
/// landing's slowing).
const LONGEST_STRIDE: f32 = 1.0 + MATCHED;

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
/// speed: played up to `MATCHED` faster first, then its stride warped up to `LONGEST_STRIDE`
/// (Unreal's Stride Warping, its documentation: stride scale = locomotion speed over the
/// animation's), then played faster still, to `FASTEST`, only for a speed between gaits (a
/// change of gait, a start); slower than its clip, played slower to `SLOWEST`, unwarped. A gait
/// at its own speed is shown within `MATCHED` of as made (`matched`). Standing still plays
/// slowest, unwarped.
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
    let warp = (need / need.clamp(SLOWEST, 1.0 + MATCHED)).clamp(1.0, LONGEST_STRIDE);
    Gait {
        rate: (need / warp).clamp(SLOWEST, FASTEST),
        warp,
    }
}

/// Whether a gait's clip `clip` of `pace` is shown within `MATCHED` of as made at its gait's
/// own `speed` (on a body of the base's height), and if not, what breaks the rule and what to
/// do (docs/animation.md rule 1, D585).
pub fn matched(clip: &str, pace: f32, speed: f32) -> Result<Gait, String> {
    let shown = gait(speed, pace, 1.0);
    let off = stretch(speed, pace, 1.0);
    if off <= MATCHED + 1e-4 {
        return Ok(shown);
    }
    let (low, high) = (
        pace * (1.0 - MATCHED),
        pace * (1.0 + MATCHED) * LONGEST_STRIDE,
    );
    Err(format!(
        "{clip} (made at {pace:.2} m/s) is shown at {speed:.2} m/s played x{:.2}, stride x{:.2}, \
         {:.0}% off as made, past the {:.0}% a gait's clip may be stretched (docs/animation.md \
         rule 1, D585): move the gait's speed within {low:.2}-{high:.2} m/s, or make the clip at \
         {speed:.2} m/s (motion.ts); never widen MATCHED or the stride warp",
        shown.rate,
        shown.warp,
        off * 100.0,
        MATCHED * 100.0
    ))
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
    /// against the speed a man moves while it plays, the worst `stretch`, printed for the
    /// controller's scorecard; and each within `MATCHED` of as made (D585), or the test fails
    /// naming the clip and the speed.
    #[test]
    fn each_gait_is_shown_within_a_tenth_of_its_clip_at_its_speed() {
        let clips = crate::model_files::json(crate::figure::CLIPS);
        let paces: HashMap<String, f32> =
            serde_json::from_value(crate::model_files::extra(&clips, "paces").unwrap()).unwrap();
        let man = crate::movement::man();
        let mut worst = 0.0f32;
        let mut broken = Vec::new();
        for (clip, speed) in [("Jog_Fwd_Loop", man.jog), ("Sprint_Loop", man.sprint)] {
            let pace = paces.get(clip).copied().unwrap_or(0.0);
            let shown = gait(speed, pace, 1.0);
            let each = stretch(speed, pace, 1.0);
            println!(
                "pace\t{clip} {pace} m/s at {speed}: rate {:.2}, stride x{:.2}, {each:.3}",
                shown.rate, shown.warp
            );
            worst = worst.max(each);
            broken.extend(matched(clip, pace, speed).err());
        }
        println!("scorecard\tpace.match\t{worst}");
        assert!(broken.is_empty(), "{}", broken.join("\n"));
    }

    #[test]
    fn a_stride_is_played_a_tenth_faster_then_warped_a_tenth_then_faster_between_gaits() {
        // A clip made at the speed is shown as made.
        assert_eq!(gait(4.0, 4.0, 1.0), Gait::AS_MADE);
        assert_eq!(stretch(4.0, 4.0, 1.0), 0.0);
        // Within a tenth, played faster or slower, never warped.
        assert_eq!(gait(4.3, 4.0, 1.0).warp, 1.0);
        assert!((gait(4.3, 4.0, 1.0).rate - 1.075).abs() < 1e-5);
        assert!((gait(3.7, 4.0, 1.0).rate - 0.925).abs() < 1e-5);
        // Past it, played a tenth faster and the rest warped, up to a tenth longer: still
        // matched, the ground passing as fast as the stride shown.
        let warped = gait(4.6, 4.0, 1.0);
        assert!((warped.rate - 1.1).abs() < 1e-5, "{warped:?}");
        assert!((4.0 * warped.rate * warped.warp - 4.6).abs() < 1e-4);
        assert!(matched("a jog", 4.0, 4.6).is_ok());
        // Past both, played faster still, a speed between gaits, where no gait may stand.
        let between = gait(5.6, 4.0, 1.0);
        assert_eq!(between.warp, LONGEST_STRIDE);
        assert!((4.0 * between.rate * between.warp - 5.6).abs() < 1e-4);
        let why = matched("Sprint_Loop", 3.9, 7.4).unwrap_err();
        assert!(why.contains("Sprint_Loop") && why.contains("7.40"), "{why}");
        // A slower speed is played slower, never shortened.
        assert!(matched("a jog", 4.0, 3.5).is_err());
        assert_eq!(gait(3.5, 4.0, 1.0).warp, 1.0);
        // Past every limit, the ground passes faster than the stride shown and the stretch
        // says so.
        let fast = gait(FASTEST * LONGEST_STRIDE * 1.5, 1.0, 1.0);
        assert_eq!((fast.rate, fast.warp), (FASTEST, LONGEST_STRIDE));
        assert!(stretch(FASTEST * LONGEST_STRIDE * 1.5, 1.0, 1.0) >= 0.5 - 1e-5);
        // A taller body takes longer strides: the same speed asks less of its clip.
        assert!(gait(4.6, 4.0, 1.1).warp < gait(4.6, 4.0, 1.0).warp);
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
