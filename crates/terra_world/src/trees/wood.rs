//! A tree's wood: its limbs drawn as tubes with bark on them, curving through their grown
//! points to a fine tip, a trunk's foot swelling into roots (D264, D289).

use super::build::Build;
use super::{Limb, Species, Tree};
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};
use terra_gen::{Seed, smoothstep};

/// The level of a tree's wood that is its finest, its twigs and shoots: drawn with the near
/// level but casting no shadow (`wood`).
pub const FINE: usize = 3;

/// A tree's wood at a level of detail: all of it, then without the twigs, then only the
/// trunk and great limbs; rounder near the camera. A conifer's many thin branches are few
/// sided, and gone at the last level, where its needles hide them. `bark_m` is how much of
/// a limb one repeat of its bark's picture covers, round it and along it.
///
/// The near level's finest wood, a broadleaf's twigs and a conifer's shoots and stubs, is a
/// level of its own (`FINE`), drawn with the near level but casting no shadow: a shadow cannot
/// dither, and a canopy's worth of twigs leaving the sun's view at once, where the near level
/// gives way to the next, popped the light shafts through it (D309). The crown's shadow
/// stand-in already casts their share.
pub fn wood(tree: &Tree, detail: usize, height: f32, bark_m: Vec2, pivots: &[[f32; 4]]) -> Build {
    // Near the camera, round enough that no limb shows facets and a trunk's roots each have
    // sides to swell on (D264).
    let (deepest, sides) = match tree.species {
        Species::Oak => (
            [3, 2, 1, 3][detail],
            [[36, 12, 7, 4], [14, 6, 4, 3], [7, 4, 3, 3], [36, 12, 7, 4]][detail],
        ),
        Species::Pine | Species::Spruce => (
            [2, 1, 0, 2][detail],
            [[28, 6, 4, 3], [12, 4, 3, 3], [7, 3, 3, 3], [28, 6, 4, 3]][detail],
        ),
    };
    let finest = if detail == 0 || detail == FINE {
        deepest
    } else {
        u32::MAX
    };
    let mut build = Build {
        levels: !pivots.is_empty(),
        ..default()
    };
    let mut dead = Vec::new();
    for (i, limb) in tree.limbs.iter().enumerate() {
        if limb.depth > deepest
            || (detail == 0 && limb.depth == finest)
            || (detail == FINE && limb.depth != finest)
        {
            continue;
        }
        build.pivot = pivots.get(i).copied();
        let from = build.positions.len();
        tube(
            &mut build,
            limb,
            sides[limb.depth.min(3) as usize],
            height,
            bark_m,
        );
        if tree.dead.contains(&i) {
            dead.push(from..build.positions.len());
        }
    }
    // A Scots pine's bark is thick, grey-brown and fissured low on its bole and thin, flaking
    // and orange up in its crown, which tells a pine at a glance; one shade all the way up
    // read as one orange tube (D264). Where it turns, and how orange, is each pine's own
    // (D276). Carried in its vertices' colour, which tints the scan.
    if tree.species == Species::Pine {
        let (from, orange) = (tree.plates.x, tree.plates.y);
        let plates = Vec3::ONE + (Vec3::new(1.2, 0.98, 0.78) - Vec3::ONE) * orange;
        for (at, colour) in build.positions.iter().zip(&mut build.colours) {
            let up = smoothstep(from, from + 0.35, at[1] / height);
            // Low on the bole the scan's orange taken to a grey-brown, its plates' red gone
            // (D309): at (0.7, 0.72, 0.8) the whole stand read as redwoods; at (0.52, 0.72,
            // 1.2), a neutral grey, its shaded side went slate-blue in the sky's light, and
            // with the stand's trunks filling the view, so did the shade (D318).
            let shade = Vec3::new(0.55, 0.7, 0.8).lerp(plates, up);
            colour[0] = shade.x;
            colour[1] = shade.y;
            colour[2] = shade.z;
        }
    }
    // Dead wood weathered silver-grey, lighter than the living bark round it, so a pine's
    // stubs show against its bole (D309).
    for range in dead {
        for colour in &mut build.colours[range] {
            colour[0] = 1.15;
            colour[1] = 1.18;
            colour[2] = 1.28;
        }
    }
    build
}

/// A limb's rings: as grown, and for a trunk also between, every 0.35 m near the ground
/// and 1 m above, so its roots and lumps have rings to show on.
/// A branch's line of points drawn as the curve through them, `between` more rings in each
/// step, as SpeedTree draws a branch as a spline through its nodes: the straight runs
/// between a grown line's points read as pick-up sticks with elbows (D289). Catmull-Rom,
/// centripetal so a kink does not overshoot into a loop; the radius runs along it evenly.
pub fn curved(points: &[Vec3], radii: &[f32], between: usize) -> (Vec<Vec3>, Vec<f32>) {
    let n = points.len();
    if between == 0 || n < 3 {
        return (points.to_vec(), radii.to_vec());
    }
    let at = |i: isize| -> Vec3 {
        if i < 0 {
            points[0] * 2.0 - points[1]
        } else if i as usize >= n {
            points[n - 1] * 2.0 - points[n - 2]
        } else {
            points[i as usize]
        }
    };
    let (mut out, mut out_radii) = (vec![points[0]], vec![radii[0]]);
    for i in 0..n - 1 {
        let [p0, p1, p2, p3] = [-1, 0, 1, 2].map(|k| at(i as isize + k));
        // Centripetal knots: the square root of each step's length.
        let knot = |a: Vec3, b: Vec3| libm::sqrtf(a.distance(b)).max(1.0e-4);
        let (t1, t2, t3) = {
            let t1 = knot(p0, p1);
            let t2 = t1 + knot(p1, p2);
            (t1, t2, t2 + knot(p2, p3))
        };
        for s in 1..=between + 1 {
            let f = s as f32 / (between + 1) as f32;
            let t = t1 + (t2 - t1) * f;
            let a1 = p0 * ((t1 - t) / t1) + p1 * (t / t1);
            let a2 = p1 * ((t2 - t) / (t2 - t1)) + p2 * ((t - t1) / (t2 - t1));
            let a3 = p2 * ((t3 - t) / (t3 - t2)) + p3 * ((t - t2) / (t3 - t2));
            let b1 = a1 * ((t2 - t) / t2) + a2 * (t / t2);
            let b2 = a2 * ((t3 - t) / (t3 - t1)) + a3 * ((t - t1) / (t3 - t1));
            out.push(b1 * ((t2 - t) / (t2 - t1)) + b2 * ((t - t1) / (t2 - t1)));
            out_radii.push(radii[i] + (radii[i + 1] - radii[i]) * f);
        }
    }
    (out, out_radii)
}

pub fn rings(limb: &Limb, between: usize) -> (Vec<Vec3>, Vec<f32>) {
    if limb.depth > 0 {
        // A branch runs out to a point, as a real one does, rather than ending in an open,
        // flat-cut tube (D247).
        let (mut points, mut radii) = if limb.pointed {
            curved(&limb.points, &limb.radii, between)
        } else {
            (limb.points.clone(), limb.radii.clone())
        };
        // It runs on past its last point, narrowing to a point over several of its own
        // widths and at least a hand's length, as a twig does, not to a pin at once: a tip
        // a few widths long read from below as a blunt cap (D289).
        if let (true, &[.., before, last], Some(&r)) =
            (limb.pointed, points.as_slice(), radii.last())
        {
            let ahead = (last - before).normalize_or(Vec3::Y);
            let reach = (r * 9.0).max(0.12);
            for (along, narrow) in [(0.3, 0.6), (0.65, 0.3), (1.0, 0.04)] {
                points.push(last + ahead * reach * along);
                radii.push(r * narrow);
            }
        }
        return (points, radii);
    }
    let (mut points, mut radii) = (vec![limb.points[0]], vec![limb.radii[0]]);
    for k in 1..limb.points.len() {
        let (a, b) = (limb.points[k - 1], limb.points[k]);
        // Close at the foot, where the roots swell out of it.
        let spacing = if a.y < 1.2 {
            0.12
        } else if a.y < 3.0 {
            0.35
        } else {
            1.0
        };
        let count = (a.distance(b) / spacing).ceil().max(1.0) as usize;
        for n in 1..=count {
            let t = n as f32 / count as f32;
            points.push(a.lerp(b, t));
            radii.push(limb.radii[k - 1] + (limb.radii[k] - limb.radii[k - 1]) * t);
        }
    }
    (points, radii)
}

/// How a limb's radius varies round it and along it, as a share of its radius: shallow
/// flutes running up the bark and broad lumps, and on a trunk six to eight roots swelling
/// out of its foot, each of its own size, width and height and curling a little round the
/// trunk as it climbs, so no foot is a star of equal points (D264); each tree its own by
/// where it grew.
pub struct Shape {
    phase: f32,
    trunk: bool,
    /// Each root: where round the trunk, how far it stands out (a share of the radius), how
    /// wide (radians), and how high up the trunk it reaches (metres).
    roots: Vec<(f32, f32, f32, f32)>,
}

impl Shape {
    pub fn of(limb: &Limb) -> Self {
        let p = limb.points[0];
        let phase =
            (libm::sinf(p.x * 12.9898 + p.z * 78.233 + p.y * 37.719) * 43_758.547).fract() * TAU;
        let trunk = limb.depth == 0;
        let roots = if trunk {
            let mut dice = Seed((phase * 1.0e6) as u64).derive("roots").rng();
            let count = 6 + dice.below(3);
            let r = limb.radii[0];
            (0..count)
                .map(|k| {
                    (
                        (k as f32 + dice.range(-0.3, 0.3)) / count as f32 * TAU + phase,
                        dice.range(0.3, 0.85),
                        dice.range(0.16, 0.32),
                        r * dice.range(0.5, 1.3) + 0.25,
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        Self {
            phase,
            trunk,
            roots,
        }
    }

    pub fn at(&self, angle: f32, along: f32, up: f32) -> f32 {
        let deep = if self.trunk { 1.0 } else { 0.5 };
        let flutes = 0.035 * libm::cosf(7.0 * angle + self.phase + 0.25 * along)
            + 0.02 * libm::cosf(13.0 * angle + 2.1 * self.phase - 0.4 * along);
        let lumps = 0.05
            * libm::sinf(3.0 * angle + 1.7 * along + self.phase)
            * libm::sinf(0.9 * along + 2.0 * angle);
        let mut shape = 1.0 + deep * (flutes + lumps);
        if self.trunk {
            // Each root a ridge, highest at the ground and dying into the bole over its own
            // height; the trunk drawn in a little between them.
            let mut swell = 0.0f32;
            for &(at, out, wide, high) in &self.roots {
                let off = (angle - at - 0.12 * up + PI).rem_euclid(TAU) - PI;
                let fall = libm::expf(-(up - 0.15).max(0.0) / high * 2.2);
                let ridge = libm::expf(-(off / wide) * (off / wide));
                swell = swell.max(out * ridge * fall);
            }
            let between = 0.1 * (1.0 - smoothstep(0.0, 1.2, up));
            shape *= 1.0 + swell - between;
        }
        shape
    }
}

/// A limb as a tube with bark round it.
pub fn tube(build: &mut Build, limb: &Limb, sides: u32, height: f32, bark_m: Vec2) {
    let start = build.positions.len() as u32;
    // A branch's curve drawn finer the rounder it is drawn: a near great limb two rings
    // between its points, a near branch one, the thin and the far their points alone.
    let between = match sides {
        10.. => 2,
        6.. => 1,
        _ => 0,
    };
    let (points, radii) = rings(limb, between);
    let shape = Shape::of(limb);
    let mut along = 0.0;
    // One repeat of the bark's picture is about `bark_m` round the limb a third of the way
    // up, as many times all the way up: a count that changed from ring to ring cut the bark
    // into bands. Along the limb it repeats as often as round it at each ring, so the bark
    // keeps its own proportions as the limb thins, rather than stretching up it (D247).
    let wraps = (TAU * radii[radii.len() / 3] / bark_m.x).round().max(1.0);
    let repeat_along =
        |r: f32| (TAU * r.max(0.004) / wraps * bark_m.y / bark_m.x).min(bark_m.y * 2.0);
    let mut v = 0.0;
    let mut side = (points[1] - points[0]).any_orthonormal_vector();
    // How far along the limb each ring is, and its radius at each side, so a normal can
    // follow how the radius changes along it too: a root's back faces up out of the ground,
    // not level, and a twig's tip leans forward as it narrows.
    let mut alongs = vec![0.0f32; points.len()];
    for i in 1..points.len() {
        alongs[i] = alongs[i - 1] + points[i].distance(points[i - 1]);
    }
    let radius_at = |i: usize, angle: f32| radii[i] * shape.at(angle, alongs[i], points[i].y);
    for (i, (&point, &radius)) in points.iter().zip(&radii).enumerate() {
        if i > 0 {
            let step = point.distance(points[i - 1]);
            along += step;
            // Along its surface, which slopes out over a flared foot, not its middle line, or
            // the bark is drawn out in streaks down the roots.
            let surface = libm::sqrtf(step * step + (radius - radii[i - 1]).powi(2));
            v += surface / repeat_along(0.5 * (radius + radii[i - 1]));
        }
        let (before, after) = (i.saturating_sub(1), (i + 1).min(points.len() - 1));
        let forward = if i + 1 < points.len() {
            points[i + 1] - point
        } else {
            point - points[i - 1]
        }
        .normalize();
        // Carry the ring's orientation along the limb, so it does not twist.
        side = (side - forward * side.dot(forward)).normalize_or(forward.any_orthonormal_vector());
        let other = forward.cross(side);
        let bend = (point.y / height).clamp(0.0, 1.0);
        let flutter = if limb.depth >= 2 { 0.15 } else { 0.0 };
        for j in 0..=sides {
            let angle = j as f32 / sides as f32 * TAU;
            let out = side * libm::cosf(angle) + other * libm::sinf(angle);
            let round = side * -libm::sinf(angle) + other * libm::cosf(angle);
            // The trunk's own shape: flutes, lumps and roots, and the normal that follows it
            // round, from how fast the radius changes with the angle.
            let r = radius * shape.at(angle, along, point.y);
            let step = 0.01;
            let slope = radius
                * (shape.at(angle + step, along, point.y) - shape.at(angle - step, along, point.y))
                / (2.0 * step);
            let rise = (radius_at(after, angle) - radius_at(before, angle))
                / (alongs[after] - alongs[before]).max(1.0e-4);
            let normal = (out - round * slope / r.max(1.0e-4) - forward * rise).normalize_or(out);
            let uv = Vec2::new(j as f32 / sides as f32 * wraps, v);
            let index = build.vertex(
                point + out * r,
                normal,
                uv,
                Vec2::new(bend * bend, flutter),
                Vec3::ONE,
            );
            // How much of the sky it sees, under its own crown and its neighbours': least at
            // the foot, which stays damp, dark and mossy for a metre or so, more up the bole,
            // most up in the crown (D216). The foot's darkening reached eye height, and the
            // trunks there read black (D264).
            build.colours[index as usize][3] =
                0.3 + 0.2 * smoothstep(0.0, 1.4, point.y) + 0.5 * smoothstep(0.0, 0.8, bend);
        }
    }
    let ring = sides + 1;
    for i in 0..(points.len() as u32 - 1) {
        for j in 0..sides {
            let a = start + i * ring + j;
            let b = a + ring;
            // Wound so each triangle faces out of the tube, as its normals do: the other way the
            // outside was culled and the inside of the far wall drawn, lit from behind.
            build.indices.extend([a, a + 1, b, a + 1, b + 1, b]);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// A branch is drawn as the curve through its grown points, not straight runs between
    /// them with elbows, and runs out to a fine point at least a hand's length past its last
    /// point (D289).
    #[test]
    fn a_branch_curves_through_its_points_to_a_fine_tip() {
        // A crooked branch: out, then kinked up, then on.
        let points = vec![
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.8, 0.6, 0.0),
            Vec3::new(2.8, 0.8, 0.0),
        ];
        let limb = Limb {
            points: points.clone(),
            radii: vec![0.08, 0.06, 0.04, 0.02],
            depth: 2,
            pointed: true,
        };
        let (drawn, radii) = rings(&limb, 2);
        // Through every grown point.
        for point in &points {
            assert!(
                drawn.iter().any(|p| p.distance(*point) < 1.0e-4),
                "misses {point}"
            );
        }
        // Between them it bends a little at each ring, not all at once at an elbow.
        let sharpest = drawn
            .windows(3)
            .map(|w| (w[1] - w[0]).angle_between(w[2] - w[1]))
            .fold(0.0, f32::max);
        let elbow = (points[1] - points[0]).angle_between(points[2] - points[1]);
        assert!(
            sharpest < elbow * 0.7,
            "a {sharpest} turn of a {elbow} elbow"
        );
        // A fine tip, a hand's length on.
        let tip = drawn[drawn.len() - 1].distance(points[3]);
        assert!(tip >= 0.12, "a tip {tip} m long");
        let end = radii[radii.len() - 1];
        assert!(end < 0.002, "ends {end} m thick");
        // A cut log keeps its points and its open end.
        let log = Limb {
            pointed: false,
            ..limb
        };
        assert_eq!(rings(&log, 2).0, points);
    }
}
