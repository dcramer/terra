//! A crown as solid lumps, one a branch: near, the rough balls its shadow is cast from
//! (D234); far off, where a lump is a few pixels across, a double pyramid of twelve
//! triangles that covers what the ball does from the side and from above, as foliage levels
//! of detail drop vertices with distance.

use super::{Build, Tree};
use bevy::prelude::Vec3;
use std::f32::consts::{PI, TAU};

/// How far a far lump's points stand above and below its middle, of its half height: so it
/// covers from the side what the ball of rings does (2.83 of its half width times half
/// height, a double pyramid's being 2 times this), its middle ring being the ball's widest.
const FAR_POINT: f32 = std::f32::consts::SQRT_2;

/// What a crown's shadow is cast from (`shadow`), or what stands for its crown far off: a
/// rounded lump for each branch, as big as most of its leaves, solid so the GPU can skip
/// what is hidden. The light no longer comes through in flecks, which nobody sees at that
/// distance.
pub(super) fn crown_clumps(tree: &Tree, scale: f32, shadow: bool) -> Build {
    let mut build = Build {
        offset_in_colour: shadow,
        ..Default::default()
    };
    for (centre, radii) in lumps_of(tree, scale) {
        if shadow {
            lump(&mut build, centre, radii);
        } else {
            far_lump(&mut build, centre, radii);
        }
    }
    build
}

/// A crown's lumps, one a branch, as big as most of its leaves times `scale`: their middles
/// and half sizes, in the tree's own frame.
pub(super) fn lumps_of(tree: &Tree, scale: f32) -> Vec<(Vec3, Vec3)> {
    (0..tree.branches)
        .filter_map(|branch| {
            let tips: Vec<(Vec3, f32)> = tree
                .tips
                .iter()
                .zip(&tree.branch_of)
                .filter(|(_, of)| **of == branch)
                .map(|(tip, _)| *tip)
                .collect();
            if tips.is_empty() {
                return None;
            }
            let count = tips.len() as f32;
            let centre = tips.iter().fold(Vec3::ZERO, |sum, (tip, _)| sum + *tip) / count;
            let size = tips.iter().map(|(_, size)| *size).sum::<f32>() / count;
            let spread = tips
                .iter()
                .map(|(tip, _)| (*tip - centre).abs())
                .fold(Vec3::ZERO, Vec3::max);
            let radii = (spread * 0.8 + Vec3::splat(size * 0.6)) * Vec3::new(1.0, 0.75, 1.0);
            Some((centre, radii * scale))
        })
        .collect()
}

/// A point of a lump's middle ring of six, `step` round, and its normal.
fn round(step: u32) -> Vec3 {
    let turn = step as f32 / 6.0 * TAU;
    Vec3::new(libm::cosf(turn), 0.0, libm::sinf(turn))
}

/// A rough ball: rings of six round, four rings high.
fn lump(build: &mut Build, centre: Vec3, radii: Vec3) {
    let base = build.positions.len() as u32;
    let (around, rings) = (6, 4);
    for ring in 0..=rings {
        let tilt = ring as f32 / rings as f32 * PI;
        for step in 0..around {
            let flat = round(step);
            let normal = Vec3::new(
                libm::sinf(tilt) * flat.x,
                libm::cosf(tilt),
                libm::sinf(tilt) * flat.z,
            );
            let index = build.vertex(
                centre + normal * radii,
                normal,
                bevy::prelude::Vec2::ZERO,
                bevy::prelude::Vec2::ZERO,
                Vec3::ONE,
            );
            // Its offset from the lump's middle, for the shader to grow it from (D234).
            if build.offset_in_colour {
                let offset = normal * radii;
                build.colours[index as usize] = [offset.x, offset.y, offset.z, 1.0];
            }
        }
    }
    // Wound anticlockwise seen from outside, the front face Bevy keeps, so back-face culling
    // drops the far half and the near one is lit by normals that face the eye.
    for ring in 0..rings {
        for step in 0..around {
            let a = base + ring * around + step;
            let b = base + ring * around + (step + 1) % around;
            build
                .indices
                .extend([a, b, a + around, b, b + around, a + around]);
        }
    }
}

/// A far lump: the ball's widest ring of six with a point above and below, `FAR_POINT` of
/// its half height out, smooth shaded as the ball is.
fn far_lump(build: &mut Build, centre: Vec3, radii: Vec3) {
    let mut vertex = |at: Vec3, normal: Vec3| {
        build.vertex(
            centre + at * radii,
            normal,
            bevy::prelude::Vec2::ZERO,
            bevy::prelude::Vec2::ZERO,
            Vec3::ONE,
        )
    };
    let top = vertex(Vec3::Y * FAR_POINT, Vec3::Y);
    let bottom = vertex(Vec3::NEG_Y * FAR_POINT, Vec3::NEG_Y);
    let ring = (0..6)
        .map(|step| vertex(round(step), round(step)))
        .collect::<Vec<_>>();
    for step in 0..6 {
        let (a, b) = (ring[step], ring[(step + 1) % 6]);
        // Wound as the ball's are, anticlockwise seen from outside.
        build.indices.extend([top, b, a, a, b, bottom]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The corners of each triangle of a mesh.
    fn triangles(build: &Build) -> Vec<[Vec3; 3]> {
        build
            .indices
            .chunks(3)
            .map(|t| [t[0], t[1], t[2]].map(|i| Vec3::from(build.positions[i as usize])))
            .collect()
    }

    /// How much of the plane across `axis` a mesh covers, seen down it: counted on a fine
    /// grid of points over a square `half` either side of its middle.
    fn covered(build: &Build, axis: usize, half: f32) -> f32 {
        let flat = |p: Vec3| {
            let mut kept = [p.x, p.y, p.z].into_iter().enumerate();
            let mut next = || kept.find(|(i, _)| *i != axis).map(|(_, v)| v).unwrap();
            (next(), next())
        };
        let shapes: Vec<[(f32, f32); 3]> =
            triangles(build).into_iter().map(|t| t.map(flat)).collect();
        let side = |(ax, ay): (f32, f32), (bx, by): (f32, f32), (x, y): (f32, f32)| {
            (bx - ax) * (y - ay) - (by - ay) * (x - ax)
        };
        let inside = |p: (f32, f32)| {
            shapes.iter().any(|[a, b, c]| {
                let (u, v, w) = (side(*a, *b, p), side(*b, *c, p), side(*c, *a, p));
                (u >= 0.0 && v >= 0.0 && w >= 0.0) || (u <= 0.0 && v <= 0.0 && w <= 0.0)
            })
        };
        let n = 300;
        let cell = 2.0 * half / n as f32;
        let hits = (0..n * n)
            .filter(|i| {
                let (x, y) = (i % n, i / n);
                inside((
                    -half + (x as f32 + 0.5) * cell,
                    -half + (y as f32 + 0.5) * cell,
                ))
            })
            .count();
        hits as f32 * cell * cell
    }

    /// Every face of a lump, near ball and far pyramid, is its front seen from outside: wound
    /// anticlockwise there, as Bevy's front face is, and facing the way its corners' normals
    /// do. So back-face culling keeps the half toward the eye, lit by normals facing it, not
    /// the far inside of each lump with its normals turned away (forest perf round 3).
    #[test]
    fn every_face_of_a_lump_points_out() {
        let (centre, radii) = (Vec3::new(3.0, 11.0, -2.0), Vec3::new(1.4, 0.9, 1.1));
        let (mut ball, mut far) = (Build::default(), Build::default());
        lump(&mut ball, centre, radii);
        far_lump(&mut far, centre, radii);
        for (name, build) in [("ball", &ball), ("far lump", &far)] {
            let mut faced = 0;
            for t in build.indices.chunks(3) {
                let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from(build.positions[i as usize]));
                let front = (b - a).cross(c - a);
                if front.length() < 1.0e-6 {
                    continue;
                }
                let out = (a + b + c) / 3.0 - centre;
                assert!(front.dot(out) > 0.0, "a {name}'s face {t:?} points in");
                let normals = t
                    .iter()
                    .map(|i| Vec3::from(build.normals[*i as usize]))
                    .sum::<Vec3>();
                assert!(
                    front.dot(normals) > 0.0,
                    "a {name}'s face {t:?} faces from its normals"
                );
                faced += 1;
            }
            assert!(
                faced >= build.triangles() / 2,
                "a {name} has {faced} faces with area"
            );
        }
    }

    /// Far off, a lump is drawn in a quarter of the ball's triangles, covering within a few
    /// hundredths what the ball covers seen from either side and from above: where it is a
    /// few pixels, the crown it makes looks as big as it did.
    #[test]
    fn a_far_lump_covers_what_the_ball_does_in_a_quarter_of_its_triangles() {
        let radii = Vec3::new(1.0, 0.75, 1.0);
        let (mut ball, mut far) = (Build::default(), Build::default());
        lump(&mut ball, Vec3::ZERO, radii);
        far_lump(&mut far, Vec3::ZERO, radii);
        assert_eq!(far.triangles() * 4, ball.triangles());
        for axis in 0..3 {
            let (seen_ball, seen_far) = (covered(&ball, axis, 1.2), covered(&far, axis, 1.2));
            let off = (seen_far / seen_ball - 1.0).abs();
            assert!(
                off < 0.04,
                "seen down axis {axis} the far lump covers {seen_far:.3}, the ball {seen_ball:.3}"
            );
        }
    }
}
