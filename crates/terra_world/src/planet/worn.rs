//! Where a meadow's grass grows thin: the planet's worn patches, tens of metres across, the
//! soil showing through in flecks, as a grazed or dry patch of a meadow is, not bare. Their
//! grass thins over tens of metres, as a terrain's splat map blends one layer into the next.
//! One smooth field read by both the ground's vertices, metres to tens of metres apart, and
//! the ground cover's 2 m map near the camera (`cover::floor`), so the two draw a patch
//! alike. Cut bare in 3 m, the map drew orange blobs of soil 20 to 80 m across inside the
//! coarse ground's blur round a town; the ground shader turns grass under about half into
//! soil (`ground.wesl`, its `grass` weight), so a patch left over half its grass shows soil
//! only where the shader's own noise frays it.

use bevy::math::Vec3;
use terra_gen::{Noise, smoothstep};

/// How far across a patch's field varies, in metres.
const PATCH_M: f32 = 90.0;
/// The field's values from a patch's middle to where the grass is all there.
const WORN: [f32; 2] = [-0.62, -0.22];
/// How much of its grass a patch's middle keeps.
const THINNEST: f32 = 0.6;

/// How much of the grass the worn patches leave at a point, [`THINNEST`] in a patch's
/// middle to 1 all of it.
pub(super) fn grass_left(cover: &Noise, point: Vec3) -> f32 {
    // Two octaves, not three: the third's 20 m wiggles drew the edge sharp again.
    let patches = cover.fbm(point / PATCH_M, 2, 2.0, 0.5);
    THINNEST + (1.0 - THINNEST) * smoothstep(WORN[0], WORN[1], patches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::Seed;

    /// The grass along lines across many patches of several worlds, a metre apart.
    fn along_lines(mut each: impl FnMut(f32, f32)) {
        for seed in 0..6u64 {
            let noise = Noise::new(Seed(seed).derive("ground cover"));
            for line in 0..60 {
                let angle = line as f32 * 2.399_963;
                let way = Vec3::new(libm::cosf(angle), 0.0, libm::sinf(angle));
                let side = Vec3::new(-way.z, 0.0, way.x);
                let from = Vec3::new(0.0, 15_000.0, 0.0) + side * (line as f32 * 137.0);
                let mut last = grass_left(&noise, from);
                for metre in 1..2000 {
                    let now = grass_left(&noise, from + way * metre as f32);
                    each(last, now);
                    last = now;
                }
            }
        }
    }

    /// No patch's grass thins by a tenth of all of it in under 2.5 m, so from its middle to
    /// all grass takes at least 10 m, about the spacing of the ground's vertices a few
    /// hundred metres out, and they carry the edge the cover map draws; the cut this
    /// replaced (three octaves at 70 m, -0.4 to -0.26) went from all to none in under 3 m.
    /// No patch is left more bare than grown, which the shader draws as soil; and the
    /// patches are still there, a fifth of the grass gone somewhere.
    #[test]
    fn a_worn_patch_thins_its_grass_over_metres_and_is_never_bare() {
        let (mut steepest, mut least, mut thinned) = (0.0f32, 1.0f32, 0usize);
        along_lines(|last, now| {
            steepest = steepest.max((now - last).abs());
            least = least.min(now);
            thinned += usize::from(now < 0.8);
        });
        assert!(
            steepest < 0.04,
            "the grass changed by {steepest:.3} in a metre: a patch's edge {:.1} m across",
            (1.0 - least) / steepest
        );
        assert!(least >= 0.5, "a patch left {least:.2} of its grass");
        assert!(thinned > 0, "no patch thinned the grass to four fifths");
    }
}
