//! Bevy's colour grading blends its shadows into its midtones, as its comment says it does
//! (D302's dim colour needs it). In `bevy_core_pipeline/src/tonemapping.wesl` (MIT OR
//! Apache-2.0, by the Bevy contributors; Bevy 0.20 and its main branch), the crossfade
//! below the midtones gives the highlights the weight that is the shadows': a colour just
//! under the crossfade is graded wholly as a shadow, one just over it half as a highlight,
//! so wherever the shadows and highlights are graded apart a colour steps across that
//! brightness. At dusk the shadows keep a fifth of their saturation and the highlights all
//! of theirs, and the sky, its brightness changing smoothly round the moon, showed that
//! step as a crisp grey disc (lake film r11). The one line is swapped for Blender's
//! compositor's, which Bevy's formulas follow: the shadows' weight is what the midtones'
//! leaves.

use bevy::prelude::*;

/// Where Bevy keeps the library with the grading.
const LIBRARY: &str = "embedded://bevy_core_pipeline/tonemapping.wesl";

/// Bevy's crossfade from the shadows to the midtones, weighting the highlights.
const WRONG: &str = "        levels.y = ((level - midtone_range.x) * LEVEL_MARGIN_DIV) + 0.5;
        levels.z = 1.0 - levels.y;";

/// The crossfade weighting the shadows.
const RIGHT: &str = "        levels.y = ((level - midtone_range.x) * LEVEL_MARGIN_DIV) + 0.5;
        levels.x = 1.0 - levels.y;";

pub fn plugin(app: &mut App) {
    super::shader_patch::patch_library(
        app,
        LIBRARY,
        patched,
        "Bevy's crossfade from shadows to midtones was not found: where they are graded apart a colour may step at the shadows' edge",
    );
}

/// The library with the shadows weighted in their crossfade, if it has Bevy's.
fn patched(source: &str) -> Option<String> {
    super::shader_patch::swap_once(source, WRONG, RIGHT)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shadows', midtones' and highlights' weights for a colour at `level`, as the
    /// library's crossfades give them, the one below the midtones from `low`.
    fn weights(level: f32, low: &str) -> [f32; 3] {
        let (from, margin, div) = (0.2, 0.1, 0.5 / 0.1);
        let mut w = [0.0; 3];
        if level < from - margin {
            w[0] = 1.0;
        } else if level < from + margin {
            w[1] = (level - from) * div + 0.5;
            let rest = 1.0 - w[1];
            if low.contains("levels.x = 1.0 - levels.y") {
                w[0] = rest;
            } else {
                w[2] = rest;
            }
        } else {
            w[1] = 1.0;
        }
        w
    }

    #[test]
    fn the_shadows_fade_into_the_midtones_without_a_step() {
        let library = format!("fn a() {{\n    }} else if (x) {{\n{WRONG}\n    }}\n}}\n");
        let out = patched(&library).unwrap();
        assert!(out.contains(RIGHT) && !out.contains(WRONG));
        // A dusk grade: the shadows at a fifth of their saturation, the rest at all of it.
        let saturation = |level: f32, low: &str| {
            let w = weights(level, low);
            w[0] * 0.2 + w[1] + w[2]
        };
        let step = |low: &str| (saturation(0.0999, low) - saturation(0.1001, low)).abs();
        assert!(step(WRONG) > 0.3, "Bevy's own crossfade steps");
        assert!(step(&out) < 0.01, "the patched crossfade is smooth");
        // Not there, or there twice: left alone.
        assert!(patched("fn a() {}").is_none());
        assert!(patched(&format!("{library}{library}")).is_none());
    }
}
