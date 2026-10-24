//! The ground shader's joint cracks (`assets/shaders/ground.wesl`'s `crack_of` and the open
//! lines `jointing` keys it) on the CPU, with the shader's constants read from it, so the rule
//! is tested without a GPU: only a share of a wall's joints stand open and are drawn, each
//! along both its sides, so a near wall is not netted in a grid of every joint (mountains
//! round 30's review: a crosshatch of 16 m squares, cracks on 63% of the valley's near wall).

use crate::far_air::constant;
use terra_gen::smoothstep;

/// `crack_of`, line for line: how dark the joints are at `f`, the bed, running and staggered
/// sets' places in their blocks, each set drawn where `open` says.
fn crack_of(f: [f32; 3], open: [f32; 3], soft: f32) -> f32 {
    let edge = |f: f32, wide: f32, open: f32| 1.0 + (f.min(1.0 - f) * wide - 1.0) * open;
    let bed = edge(f[0], 1.4, open[0]);
    let run = edge(f[1], 0.96, open[1]);
    let staggered = edge(f[2], 1.56, open[2]);
    1.0 - smoothstep(0.0, 0.025 + soft, bed.min(run).min(staggered))
}

/// Whether the joint line nearest `t` stands open, keyed to the line (`floor(t + 0.5)`) as the
/// shader keys it, one in `share`.
fn open_at(t: f32, set: u32, share: f32) -> f32 {
    let line = libm::floorf(t + 0.5) as i32 as u32;
    let roll = terra_gen::hash::unit3(17, set, line, 0);
    if roll >= 1.0 - share { 1.0 } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> String {
        std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/shaders/ground.wesl"
        ))
        .unwrap()
    }

    /// The share of a wall's face, a mid-bed scan of its upright sets over 60 by 60 blocks,
    /// that the cracks darken past half, with each set's lines open by `open`.
    fn cracked(open: impl Fn(f32, f32) -> [f32; 3]) -> f32 {
        let (mut dark, mut n) = (0.0, 0.0);
        let steps = 60 * 40;
        for i in 0..steps {
            for j in 0..steps {
                let (t1, t2) = (i as f32 / 40.0, j as f32 / 40.0 + 0.013);
                let f = [0.5, t1 - libm::floorf(t1), t2 - libm::floorf(t2)];
                if crack_of(f, open(t1, t2), 0.02) > 0.5 {
                    dark += 1.0;
                }
                n += 1.0;
            }
        }
        dark / n
    }

    #[test]
    fn only_a_walls_open_joints_are_drawn_and_each_along_both_its_sides() {
        let source = source();
        let run_open = constant(&source, "RUN_OPEN")[0];
        let staggered_open = constant(&source, "STAGGERED_OPEN")[0];
        // A wall whose joints all stand shut shows no crack anywhere.
        for k in 0..=100 {
            let t = k as f32 / 100.0;
            assert_eq!(crack_of([t, t, 1.0 - t], [0.0; 3], 0.02), 0.0);
        }
        // An open joint is dark on both sides of its line, its block's and the next one's.
        let line = (0..200)
            .map(|k| k as f32)
            .find(|&k| open_at(k, 1, run_open) == 1.0)
            .unwrap();
        for t in [line - 0.01, line + 0.01] {
            let f = [0.5, t - libm::floorf(t), 0.5];
            assert!(crack_of(f, [0.0, open_at(t, 1, run_open), 0.0], 0.02) > 0.8);
        }
        // Every joint drawn netted the wall in a grid; with the shader's shares open, the
        // cracks cover the open lines' share of it, a third or less of the grid's.
        let all = cracked(|_, _| [0.0, 1.0, 1.0]);
        let some = cracked(|t1, t2| {
            [
                0.0,
                open_at(t1, 1, run_open),
                open_at(t2, 2, staggered_open),
            ]
        });
        assert!(all > 0.05, "every joint drawn covers {all:.3}");
        assert!(
            some < all * (run_open.max(staggered_open) + 0.05),
            "open joints cover {some:.3} of the wall, every joint {all:.3}"
        );
        assert!(
            some > all * 0.15,
            "open joints cover {some:.3}, too few to read"
        );
    }

    #[test]
    fn the_twin_draws_the_shader_rule() {
        let source = source();
        for used in [
            "let bed = mix(1.0, min(f.x, 1.0 - f.x) * 1.4, open.x);",
            "let run = mix(1.0, min(f.y, 1.0 - f.y) * 0.96, open.y);",
            "let staggered = mix(1.0, min(f.z, 1.0 - f.z) * 1.56, open.z);",
            "return 1.0 - smoothstep(0.0, 0.025 + soft, min(bed, min(run, staggered)));",
            "step(1.0 - RUN_OPEN, hash(vec3<f32>(floor(t1 + 0.5), run, 1.9) + b.key * 7.0)),",
            "step(1.0 - STAGGERED_OPEN, hash(vec3<f32>(floor(t2 + 0.5), course, 4.1) + b.key * 7.0)),",
            "let crack = crack_of(f, open, soft);",
        ] {
            assert!(source.contains(used), "the cracks no longer read {used}");
        }
    }
}
