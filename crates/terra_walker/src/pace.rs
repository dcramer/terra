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

#[cfg(test)]
mod tests {
    use super::*;

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
