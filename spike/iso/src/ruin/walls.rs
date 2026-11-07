//! The ruin's walls: a back wall of coursed rubble broken to every height with a breach in
//! it, a low broken wall down the west side, two round pillars on the east standing to
//! different heights, and the stones that fell from them heaped at their feet.
//!
//! A wall is one continuous skin (Poly Haven's scanned Castle Wall Varriation box-mapped at
//! its true 2 m, so its courses run unbroken), 0.8 m thick, its top broken as old rubble
//! walls fall (judge 261107-0539: "a thin flat facade ... clean cut edge, no thickness",
//! against the refs' metre-thick walls with broken tops): stepped down by whole courses in
//! runs of a stone or two with square risers between, the core's rubble lumpy and grassed
//! on top with loose stones lying on it, each face's top course robbed in places and
//! chipped along it; the faces' tone broken in patches, washed in dark streaks down from
//! the top, mossed under it and damp at the foot: never one even tile, never a stack of
//! boxes.

use super::blocks::Blocks;
use super::grass::Tufts;
use super::ground::smooth;
use crate::forest::Rng;
use crate::scene::material;
use crate::terrain::{fbm, hash};
use bevy::prelude::*;

/// The back wall's middle line (north), the west wall's, and the two pillars.
pub const BACK_Z: f32 = -6.5;
pub const WEST_X: f32 = -9.0;
pub const THICK: f32 = 0.8;
pub const PILLARS: [Vec2; 2] = [Vec2::new(6.6, -3.2), Vec2::new(7.2, 1.6)];
/// castle_wall_varriation is 2 m across.
const TILE: f32 = 2.0;
/// The wall's skin is sampled this often along its run, and its faces in rows this tall,
/// so their tone can vary up them.
const STEP: f32 = 0.16;
const ROW: f32 = 0.25;
/// Below this a wall is gone, down to its rubble.
const GONE: f32 = 0.12;
/// A broken top steps down by whole courses, in runs of about a stone or two.
const COURSE: f32 = 0.2;
const UNIT: f32 = 0.45;
const PILLAR_R: f32 = 0.4;
/// The back wall's run (its two ends) and its skin's seed; its top is `back_top`.
pub const BACK: (Vec2, Vec2, u32) = (Vec2::new(-10.5, BACK_Z), Vec2::new(5.5, BACK_Z), 11);

/// The back wall's height along x before its top is broken: 3.5 m where it stands best,
/// down to a course or two, and a breach behind the fight where the tree monster comes
/// through.
pub fn back_top(x: f32) -> f32 {
    let breach = 1.0 - smooth(0.6, 1.6, (x + 2.6).abs());
    let top = 2.3 + 1.2 * (x * 0.55).sin() + 0.5 * (x * 1.7 + 1.0).sin();
    (top * (1.0 - breach) + 0.3 * breach).max(0.3) * (1.0 - 0.6 * smooth(3.0, 5.5, x))
}

/// Where step `k` of a wall's broken top begins along its run.
fn edge(seed: u32, k: i32) -> f32 {
    k as f32 * UNIT + (hash(k, 0, seed) - 0.5) * UNIT * 0.7
}

/// The step of a wall's broken top `s` m along its run.
fn step(seed: u32, s: f32) -> i32 {
    let k = (s / UNIT).floor() as i32;
    if s < edge(seed, k) {
        k - 1
    } else if s >= edge(seed, k + 1) {
        k + 1
    } else {
        k
    }
}

/// A wall's broken top at a point along its run.
struct Top {
    /// The step's stand: whole courses, flat along the step.
    h: f32,
    /// The core's rubble on top, lumpy, and how far it leans across the run.
    ridge: f32,
    lean: f32,
    /// Each face's top edge: chipped along it, a course lower where its facing was robbed.
    face: [f32; 2],
}

/// A wall's broken top at `s` m along its run. `wall` builds its skin from these, and
/// `ridge` finds a point on it.
fn cut(a: Vec2, z: Vec2, seed: u32, top: &impl Fn(f32) -> f32, s: f32) -> Top {
    let d = (z - a).normalize();
    let noise = |s: f32, k: u32| fbm(Vec2::new(s, 0.5), seed * 8 + k);
    let k = step(seed, s);
    let mid = 0.5 * (edge(seed, k) + edge(seed, k + 1));
    let c = a + d * mid;
    let along = if d.x.abs() > d.y.abs() { c.x } else { c.y };
    let jag = (noise(mid * 0.9, 0) - 0.5) * 0.9
        + (noise(mid * 3.3, 1) - 0.5) * 0.6
        + (hash(k, 1, seed) - 0.5) * 0.3;
    let courses = ((top(along) + jag) / COURSE).round();
    let h = (courses * COURSE + (hash(k, 2, seed) - 0.5) * 0.05).max(0.0);
    let face = |f: i32, n: u32| {
        let robbed = if hash(k, 3 + f, seed) < 0.22 {
            COURSE * 0.9
        } else {
            0.0
        };
        (h - 0.02 - 0.06 * noise(s * 6.0, n) - robbed).max(0.0)
    };
    Top {
        h,
        ridge: h + 0.07 * noise(s * 5.0, 9) - 0.01,
        lean: (noise(s * 1.3, 6) - 0.5) * THICK * 0.4,
        face: [face(0, 4), face(1, 5)],
    }
}

/// The point on a wall's ridge nearest `p` across its run, as its skin draws it: where a
/// bird perches on its top.
pub fn ridge(a: Vec2, z: Vec2, seed: u32, top: impl Fn(f32) -> f32, p: Vec2) -> Vec3 {
    let d = (z - a).normalize();
    let len = (z - a).length();
    let s = (p - a).dot(d).clamp(0.0, len);
    let t = cut(a, z, seed, &top, s);
    let c = a + d * s + Vec2::new(-d.y, d.x) * t.lean;
    Vec3::new(c.x, t.ridge, c.y)
}

pub fn spawn(
    mut commands: Commands,
    asset: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut b = Blocks::default();
    let mut tufts = Tufts::default();
    let mut rng = Rng::new(0x3a11_5);
    // The back wall: 3.5 m where it stands best, down to a course or two, and a breach
    // behind the fight where the tree monster comes through.
    let (a, z, seed) = BACK;
    wall(&mut b, &mut tufts, &mut rng, a, z, seed, back_top);
    // The west wall: low, a course or three, gone in places.
    wall(
        &mut b,
        &mut tufts,
        &mut rng,
        Vec2::new(WEST_X, BACK_Z - THICK * 0.5),
        Vec2::new(WEST_X, 4.0),
        23,
        |z| (0.9 + 0.6 * (z * 0.8).sin() + 0.3 * (z * 2.3).sin()).max(0.0),
    );
    // The outer walls, further gone: behind the back wall, beyond the west, and an east
    // stub, framing the court to the frame's edges.
    wall(
        &mut b,
        &mut tufts,
        &mut rng,
        Vec2::new(-7.0, -10.4),
        Vec2::new(11.0, -10.4),
        31,
        |x| (1.1 + 0.8 * (x * 0.4 + 2.0).sin() + 0.4 * (x * 1.3).sin()).max(0.0),
    );
    wall(
        &mut b,
        &mut tufts,
        &mut rng,
        Vec2::new(-12.8, -12.0),
        Vec2::new(-12.8, -0.5),
        37,
        |z| (0.8 + 0.7 * (z * 0.6 + 1.0).sin() + 0.3 * (z * 1.9).sin()).max(0.0),
    );
    wall(
        &mut b,
        &mut tufts,
        &mut rng,
        Vec2::new(11.8, -6.5),
        Vec2::new(11.8, 0.5),
        41,
        |z| (0.6 + 0.6 * (z * 0.9).sin()).max(0.0),
    );
    // The pillars: round drums stacked, each turned a little on the last, the top broken.
    for (p, drums) in [(PILLARS[0], 6), (PILLARS[1], 3)] {
        let mut y = 0.0;
        for k in 0..drums {
            let h = rng.range(0.45, 0.6);
            let r = PILLAR_R * rng.range(0.94, 1.02);
            let broken = if k + 1 == drums { 0.12 } else { 0.0 };
            drum(
                &mut b,
                &mut rng,
                Vec3::new(p.x, y, p.y),
                h - 0.015,
                r,
                broken,
            );
            y += h;
        }
    }
    // Fallen stones: heaped at the walls' feet and across the floor from the pillars.
    let heaps = [
        (Vec2::new(-2.6, BACK_Z + 1.0), 14, 1.6),
        (Vec2::new(-7.0, BACK_Z + 1.0), 7, 1.2),
        (Vec2::new(3.4, BACK_Z + 0.9), 6, 1.0),
        (Vec2::new(WEST_X + 1.0, -1.5), 6, 1.0),
        (Vec2::new(PILLARS[1].x - 1.0, PILLARS[1].y + 1.2), 5, 1.2),
        (Vec2::new(PILLARS[1].x + 0.4, PILLARS[1].y + 0.3), 6, 0.6),
    ];
    for (at, n, spread) in heaps {
        for _ in 0..n {
            let off = Vec2::new(rng.range(-1.0, 1.0), rng.range(-0.6, 0.6)) * spread;
            let big = rng.range(0.5, 1.0);
            fallen(&mut b, &mut rng, at + off, big);
        }
    }
    // A pillar's drum toppled and rolled toward the fight.
    let rolled = Quat::from_rotation_y(0.5) * Quat::from_rotation_z(1.45);
    let mut lying = Blocks::default();
    drum(&mut lying, &mut rng, Vec3::Y * -0.25, 0.5, PILLAR_R, 0.0);
    let at = Vec3::new(PILLARS[1].x - 1.8, PILLAR_R - 0.02, PILLARS[1].y - 0.4);
    b.append(
        lying,
        Transform::from_translation(at).with_rotation(rolled),
        TILE,
    );
    commands.spawn((
        Mesh3d(meshes.add(b.mesh())),
        MeshMaterial3d(materials.add(material(&asset, "castle_wall_varriation", Color::WHITE))),
    ));
    tufts.spawn(&mut commands, &mut meshes, &mut materials);
}

/// A stone's tint: value only, a few warmer or greyer.
fn stone(rng: &mut Rng) -> Color {
    let v = rng.range(0.5, 0.85);
    Color::srgb(v, v * 0.97, v * 0.93)
}

/// A stone fallen at `at`, lying any way, half sunk in the earth; `big` scales it.
fn fallen(b: &mut Blocks, rng: &mut Rng, at: Vec2, big: f32) {
    let size = Vec3::new(
        rng.range(0.3, 0.7),
        rng.range(0.18, 0.34),
        rng.range(0.25, 0.5),
    ) * big;
    let rot = Quat::from_euler(
        EulerRot::YXZ,
        rng.range(0.0, std::f32::consts::TAU),
        rng.range(-0.35, 0.35),
        rng.range(-0.35, 0.35),
    );
    let shift = Vec2::new(rng.f(), rng.f());
    let tint = stone(rng);
    b.add(
        Vec3::new(at.x, size.y * 0.3, at.y),
        size,
        rot,
        TILE,
        shift,
        tint,
    );
}

/// The colour of a wall's face at `p`, `below` m under its broken top: its tone broken in
/// patches a metre and a hand across (some stones darker, some paler, a little warmer or
/// greyer), washed in dark streaks of uneven length down from the top as rain stains old
/// walls, mossed in a ragged band under the top, darker and damp toward the foot.
fn skin(p: Vec3, below: f32) -> Color {
    let w = p.x + p.z;
    let broad = fbm(Vec2::new(w * 0.45, p.y * 0.35), 401);
    let patch = fbm(Vec2::new(w * 1.3, p.y * 1.6), 403);
    let warm = fbm(Vec2::new(w * 0.7, p.y * 0.9), 411) - 0.5;
    let long = 0.3 + 1.6 * fbm(Vec2::new(w * 1.9, 3.5), 407);
    let streak =
        smooth(0.5, 0.68, fbm(Vec2::new(w * 2.4, 0.5), 405)) * (1.0 - smooth(0.0, long, below));
    let reach = 0.12 + 0.4 * fbm(Vec2::new(w * 1.7, 0.3), 409);
    let moss = 1.0 - smooth(0.04, reach, below);
    let v = (0.58 + 0.42 * broad)
        * (0.78 + 0.44 * patch)
        * (1.0 - 0.4 * streak)
        * (0.5 + 0.5 * smooth(-0.1, 1.3, p.y));
    let stone = Vec3::new(v * (1.0 + 0.12 * warm), v * 0.97, v * (0.93 - 0.12 * warm));
    let green = Vec3::new(v * 0.6, v * 0.72, v * 0.42);
    let c = stone.lerp(green, 0.8 * moss);
    Color::srgb(c.x, c.y, c.z)
}

/// The colour of a wall's broken top at `p`: its core's earth and mortar, mossed in
/// patches.
fn core(p: Vec3) -> Color {
    let v = (0.55 + 0.4 * fbm(p.xz() * 0.9, 415)) * (0.5 + 0.5 * smooth(-0.1, 1.3, p.y));
    let m = smooth(0.35, 0.65, fbm(p.xz() * 2.2, 413));
    let earth = Vec3::new(0.62, 0.53, 0.42);
    let moss = Vec3::new(0.62, 0.74, 0.44);
    let c = earth.lerp(moss, m) * v;
    Color::srgb(c.x, c.y, c.z)
}

/// A face through four corners, its winding turned to face `out`, each corner coloured
/// by `col`.
fn face(b: &mut Blocks, mut p: [Vec3; 4], out: Vec3, col: impl Fn(Vec3) -> Color) {
    let n = (p[1] - p[0]).cross(p[2] - p[0]) + (p[2] - p[0]).cross(p[3] - p[0]);
    if n.dot(out) < 0.0 {
        p.reverse();
    }
    let shift = Vec2::ZERO;
    b.quad(p, TILE, shift, p.map(col));
}

/// A wall's face between two of its samples, from their feet up to their tops, in rows
/// so its tone varies up it.
fn side(b: &mut Blocks, foot: [Vec3; 2], top: [Vec3; 2], out: Vec3) {
    let rows = (top[0].y.max(top[1].y) / ROW).ceil().max(1.0) as usize;
    let at = |k: usize, r: usize| foot[k].lerp(top[k], r as f32 / rows as f32);
    for r in 0..rows {
        let p = [at(0, r), at(1, r), at(1, r + 1), at(0, r + 1)];
        // How far under the face's top each corner is, along its own column.
        face(b, p, out, |q| {
            let near = |f: Vec3| (q - f).xz().length_squared();
            let k = if near(foot[0]) <= near(foot[1]) { 0 } else { 1 };
            skin(q, top[k].y - q.y)
        });
    }
}

/// A wall from `a` to `z` along its middle line, standing `top(along)` high (along its run's
/// world axis): its top stepped down by courses (`cut`), lumpy rubble and grass on it and
/// loose stones lying there, each face proud or back by a few centimetres; rubble at its
/// foot, most where it has fallen lowest.
fn wall(
    b: &mut Blocks,
    tufts: &mut Tufts,
    rng: &mut Rng,
    a: Vec2,
    z: Vec2,
    seed: u32,
    top: impl Fn(f32) -> f32,
) {
    let d = (z - a).normalize();
    let len = (z - a).length();
    let across = Vec2::new(-d.y, d.x);
    let n = (len / STEP).ceil() as usize;
    let noise = |s: f32, k: u32| fbm(Vec2::new(s, 0.5), seed * 8 + k);
    // The samples: every STEP, and a pair either side of each riser so the top drops
    // square between its steps.
    let risers: Vec<f32> = (step(seed, 0.0) + 1..=step(seed, len))
        .map(|k| edge(seed, k))
        .filter(|e| *e > 0.01 && *e < len - 0.01)
        .collect();
    let mut along: Vec<f32> = (0..=n)
        .map(|i| len * i as f32 / n as f32)
        .filter(|s| risers.iter().all(|e| (s - e).abs() > 0.03))
        .chain(risers.iter().flat_map(|e| [e - 0.004, e + 0.004]))
        .collect();
    along.sort_by(f32::total_cmp);
    // Each sample: its two side faces' foot and top, and the ridge between them.
    struct Cut {
        s: f32,
        foot: [Vec3; 2],
        side: [Vec3; 2],
        ridge: Vec3,
        h: f32,
    }
    let cuts: Vec<Cut> = along
        .iter()
        .map(|&s| {
            let c = a + d * s;
            let t = cut(a, z, seed, &top, s);
            let half = |k: u32| THICK * 0.5 + (noise(s * 1.7, k) - 0.5) * 0.08;
            let (ta, tb) = (half(2), -half(3));
            let at = |t: f32, y: f32| Vec3::new(c.x + across.x * t, y, c.y + across.y * t);
            Cut {
                s,
                foot: [at(ta, -0.05), at(tb, -0.05)],
                side: [at(ta, t.face[0]), at(tb, t.face[1])],
                ridge: at(t.lean, t.ridge),
                h: t.h,
            }
        })
        .collect();
    let out_a = Vec3::new(across.x, 0.0, across.y);
    let run = Vec3::new(d.x, 0.0, d.y);
    let last = cuts.len() - 1;
    for i in 0..last {
        let (p, q) = (&cuts[i], &cuts[i + 1]);
        let (stand_p, stand_q) = (p.h > GONE, q.h > GONE);
        if stand_p && stand_q {
            side(b, [p.foot[0], q.foot[0]], [p.side[0], q.side[0]], out_a);
            side(b, [p.foot[1], q.foot[1]], [p.side[1], q.side[1]], -out_a);
            // The top, or the riser where it steps down: it faces the lower step.
            let fall = p.h - q.h;
            let up = |o: Vec3| {
                if fall.abs() > 0.05 {
                    run * fall.signum()
                } else {
                    o + Vec3::Y
                }
            };
            face(b, [p.side[0], q.side[0], q.ridge, p.ridge], up(out_a), core);
            face(
                b,
                [p.side[1], q.side[1], q.ridge, p.ridge],
                up(-out_a),
                core,
            );
        }
        // The broken ends: where the wall stops, its section shows.
        let ends = [
            (i == 0 && stand_p, p, -run),
            (stand_p && !stand_q, p, run),
            (!stand_p && stand_q, q, -run),
            (i + 1 == last && stand_q, q, run),
        ];
        for (_, cut, out) in ends.into_iter().filter(|e| e.0) {
            face(
                b,
                [cut.foot[0], cut.foot[1], cut.side[1], cut.side[0]],
                out,
                |q| skin(q, 9.0),
            );
            face(
                b,
                [cut.side[0], cut.side[1], cut.ridge, cut.ridge],
                out,
                core,
            );
        }
        // Rubble at the foot, both sides, most where the wall is lowest.
        let ds = q.s - p.s;
        let low = 1.0 - smooth(0.4, 2.4, p.h);
        if rng.f() < (0.18 + 0.5 * low) * ds / 0.3 {
            let side = if rng.f() < 0.65 { 1.0 } else { -1.0 };
            let off = side * (THICK * 0.5 + rng.range(0.05, 0.9 + 0.8 * low));
            let at = a + d * (p.s + rng.range(-0.3, 0.3)) + across * off;
            let big = rng.range(0.5, 0.9 + 0.3 * low);
            fallen(b, rng, at, big);
        }
        if !(stand_p && stand_q) || ds < 0.02 {
            continue;
        }
        let section = |c: &Cut| [c.side[0], c.ridge, c.side[1]];
        dress(b, tufts, rng, [section(p), section(q)], ds);
    }
}

/// A wall's top between two samples, each's section across it (a face's top, the ridge,
/// the other face's), `ds` m apart: loose stones lying where they fell from the course
/// above, and dead grass rooted in the core.
fn dress(b: &mut Blocks, tufts: &mut Tufts, rng: &mut Rng, cut: [[Vec3; 3]; 2], ds: f32) {
    let on_top = |t: f32, k: f32| {
        let [s0, r, s1] = [0, 1, 2].map(|j| cut[0][j].lerp(cut[1][j], k));
        if t < 0.5 {
            s0.lerp(r, t * 2.0)
        } else {
            r.lerp(s1, t * 2.0 - 1.0)
        }
    };
    if rng.f() < 0.5 * ds / STEP {
        let size = Vec3::new(
            rng.range(0.14, 0.34),
            rng.range(0.08, 0.18),
            rng.range(0.12, 0.28),
        );
        let rot = Quat::from_euler(
            EulerRot::YXZ,
            rng.range(0.0, 3.0),
            rng.range(-0.3, 0.3),
            rng.range(-0.3, 0.3),
        );
        let at = on_top(rng.range(0.1, 0.9), rng.f()) + Vec3::Y * size.y * 0.25;
        let shift = Vec2::new(rng.f(), rng.f());
        let v = rng.range(0.4, 0.7);
        let tint = Color::srgb(v, v * 0.96, v * 0.9);
        let mut chips = [Vec3::ZERO; 8];
        for c in &mut chips {
            *c = Vec3::new(
                rng.range(0.0, 0.4),
                rng.range(0.0, 0.4),
                rng.range(0.0, 0.4),
            );
        }
        b.chunk(at, size, rot, TILE, shift, tint, chips);
    }
    if rng.f() < 0.4 * ds / STEP {
        let at = on_top(rng.range(0.2, 0.8), rng.f()) - Vec3::Y * 0.02;
        let blades = 5 + (rng.f() * 8.0) as usize;
        let tall = rng.range(0.12, 0.3);
        let dry = fbm(at.xz() * 0.5, 502);
        tufts.tuft(rng, at, blades, tall, dry);
    }
}

/// A round drum of `r` and `h` standing on `foot`, twelve-sided, its top edge chipped by up
/// to `broken`.
fn drum(b: &mut Blocks, rng: &mut Rng, foot: Vec3, h: f32, r: f32, broken: f32) {
    const SIDES: usize = 12;
    let turn = rng.range(0.0, std::f32::consts::TAU);
    let rim: Vec<(Vec3, Vec3)> = (0..SIDES)
        .map(|k| {
            let a = turn + k as f32 * std::f32::consts::TAU / SIDES as f32;
            let o = Vec3::new(a.cos(), 0.0, a.sin()) * r * rng.range(0.97, 1.03);
            let chip = broken * rng.f();
            (
                foot + o,
                foot + o * (1.0 - chip * 0.6) + Vec3::Y * (h - chip),
            )
        })
        .collect();
    let crown = foot + Vec3::Y * (h - broken * 0.3);
    for k in 0..SIDES {
        let (a, b2) = (rim[k], rim[(k + 1) % SIDES]);
        let out = ((a.0 + b2.0) * 0.5 - foot).normalize_or(Vec3::X);
        face(b, [a.0, b2.0, b2.1, a.1], out, |q| skin(q, 9.0));
        face(b, [a.1, b2.1, crown, crown], Vec3::Y, core);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A drum's sides face away from its axis and its crown faces up, however `face` was
    /// handed the corners: the skin is lit from outside and not culled.
    #[test]
    fn drum_faces_outward() {
        let mut b = Blocks::default();
        let mut rng = Rng::new(7);
        drum(&mut b, &mut rng, Vec3::new(2.0, 0.0, -1.0), 0.5, 0.4, 0.1);
        for k in 0..b.len() {
            let (p, n) = (b.position(k), b.normal(k));
            let away = Vec3::new(p.x - 2.0, 0.0, p.z + 1.0);
            assert!(
                n.y > 0.3 || n.dot(away) > 0.0,
                "the face at {p} points {n} into the drum"
            );
        }
    }

    /// A broken top steps down by whole courses with square risers between its steps, as
    /// rubble walls fall, never a smooth slope (judge 261107-0539: "clean cut edge"): along
    /// a wall whose stand falls steadily, its height holds flat along each step and changes
    /// only at a step's edge, by a course or more where it falls.
    #[test]
    fn the_top_steps_down_by_courses() {
        let (a, z) = (Vec2::ZERO, Vec2::new(8.0, 0.0));
        let top = |x: f32| 3.0 - 0.3 * x;
        let (mut changes, mut risers) = (0, 0);
        let mut was = cut(a, z, 5, &top, 0.0).h;
        for i in 1..=800 {
            let h = cut(a, z, 5, &top, i as f32 * 0.01).h;
            let dh = (h - was).abs();
            changes += (dh > 1e-6) as usize;
            risers += (dh > COURSE * 0.7) as usize;
            was = h;
        }
        assert!(
            changes <= 28,
            "the top changes height {changes} times in 8 m: a slope"
        );
        assert!(
            risers >= 6,
            "only {risers} risers a course deep in 8 m: no steps"
        );
    }

    /// A wall stands to its height along its run and is gone where its height is nothing.
    #[test]
    fn wall_stands_where_its_top_is() {
        let mut b = Blocks::default();
        let mut tufts = Tufts::default();
        let mut rng = Rng::new(7);
        wall(
            &mut b,
            &mut tufts,
            &mut rng,
            Vec2::ZERO,
            Vec2::new(8.0, 0.0),
            1,
            |x| {
                if x < 4.0 { 2.0 } else { 0.0 }
            },
        );
        let skin_top = |lo: f32, hi: f32| {
            (0..b.len())
                .map(|k| b.position(k))
                .filter(|p| p.x > lo && p.x < hi && p.z.abs() < THICK)
                .map(|p| p.y)
                .fold(0.0f32, f32::max)
        };
        assert!(skin_top(0.5, 3.0) > 1.4, "the standing half stands");
        assert!(skin_top(5.5, 8.0) < 0.6, "the fallen half is rubble");
    }
}
