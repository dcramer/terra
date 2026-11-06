//! The ruin's walls: a back wall of coursed rubble broken to every height with a breach in
//! it, a low broken wall down the west side, two round pillars on the east standing to
//! different heights, and the stones that fell from them heaped at their feet.
//!
//! A wall is one continuous skin (Poly Haven's scanned Castle Wall Varriation box-mapped at
//! its true 2 m, so its courses run unbroken), its top broken to a jagged line along its run
//! and to a ridge across it, each face a little proud or back, darker and damp at the foot
//! and mossed on the broken top: never a stack of boxes.

use super::blocks::Blocks;
use super::ground::smooth;
use crate::forest::Rng;
use crate::scene::material;
use crate::terrain::fbm;
use bevy::prelude::*;

/// The back wall's middle line (north), the west wall's, and the two pillars.
pub const BACK_Z: f32 = -6.5;
pub const WEST_X: f32 = -9.0;
pub const THICK: f32 = 0.7;
pub const PILLARS: [Vec2; 2] = [Vec2::new(6.6, -3.2), Vec2::new(7.2, 1.6)];
/// castle_wall_varriation is 2 m across.
const TILE: f32 = 2.0;
/// The wall's skin is sampled this often along its run.
const STEP: f32 = 0.16;
/// Below this a wall is gone, down to its rubble.
const GONE: f32 = 0.22;
const PILLAR_R: f32 = 0.4;

pub fn spawn(
    mut commands: Commands,
    asset: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut b = Blocks::default();
    let mut rng = Rng::new(0x3a11_5);
    // The back wall: 3.5 m where it stands best, down to a course or two, and a breach
    // behind the fight where the tree monster comes through.
    wall(
        &mut b,
        &mut rng,
        Vec2::new(-10.5, BACK_Z),
        Vec2::new(5.5, BACK_Z),
        11,
        |x| {
            let breach = 1.0 - smooth(0.6, 1.6, (x + 2.6).abs());
            let top = 2.3 + 1.2 * (x * 0.55).sin() + 0.5 * (x * 1.7 + 1.0).sin();
            (top * (1.0 - breach) + 0.3 * breach).max(0.3) * (1.0 - 0.6 * smooth(3.0, 5.5, x))
        },
    );
    // The west wall: low, a course or three, gone in places.
    wall(
        &mut b,
        &mut rng,
        Vec2::new(WEST_X, BACK_Z - THICK * 0.5),
        Vec2::new(WEST_X, 4.0),
        23,
        |z| (0.9 + 0.6 * (z * 0.8).sin() + 0.3 * (z * 2.3).sin()).max(0.0),
    );
    // The pillars: round drums stacked, each turned a little on the last, the top broken.
    for (p, drums) in [(PILLARS[0], 6), (PILLARS[1], 3)] {
        let mut y = 0.0;
        for k in 0..drums {
            let h = rng.range(0.45, 0.6);
            let r = PILLAR_R * rng.range(0.94, 1.02);
            let broken = if k + 1 == drums { 0.12 } else { 0.0 };
            drum(&mut b, &mut rng, Vec3::new(p.x, y, p.y), h - 0.015, r, broken);
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
    b.append(lying, Transform::from_translation(at).with_rotation(rolled), TILE);
    commands.spawn((
        Mesh3d(meshes.add(b.mesh())),
        MeshMaterial3d(materials.add(material(&asset, "castle_wall_varriation", Color::WHITE))),
    ));
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
    b.add(Vec3::new(at.x, size.y * 0.3, at.y), size, rot, TILE, shift, tint);
}

/// The colour of the wall's skin at `p`: darker and damp toward the foot, varied along the
/// run in value only; the broken top greened by moss.
fn skin(p: Vec3, top: bool) -> Color {
    let v = (0.62 + 0.38 * fbm(p.xz() * 0.45, 401)) * (0.5 + 0.5 * smooth(-0.1, 1.3, p.y));
    if top {
        Color::srgb(v * 0.72, v * 0.8, v * 0.56)
    } else {
        Color::srgb(v, v * 0.97, v * 0.93)
    }
}

/// A face through four corners, its winding turned to face `out`.
fn face(b: &mut Blocks, mut p: [Vec3; 4], out: Vec3, top: bool) {
    let n = (p[1] - p[0]).cross(p[2] - p[0]) + (p[2] - p[0]).cross(p[3] - p[0]);
    if n.dot(out) < 0.0 {
        p.reverse();
    }
    let shift = Vec2::ZERO;
    b.quad(p, TILE, shift, p.map(|q| skin(q, top)));
}

/// A wall from `a` to `z` along its middle line, standing `top(along)` high (along its run's
/// world axis): its top broken by noise along the run, a ridge across it, each face proud
/// or back by a few centimetres; rubble at its foot, most where it has fallen lowest.
fn wall(b: &mut Blocks, rng: &mut Rng, a: Vec2, z: Vec2, seed: u32, top: impl Fn(f32) -> f32) {
    let d = (z - a).normalize();
    let len = (z - a).length();
    let across = Vec2::new(-d.y, d.x);
    let n = (len / STEP).ceil() as usize;
    let noise = |s: f32, k: u32| fbm(Vec2::new(s, 0.5), seed * 8 + k);
    // Each sample: its two side faces' foot and top, and the ridge between them.
    struct Cut {
        foot: [Vec3; 2],
        side: [Vec3; 2],
        ridge: Vec3,
        h: f32,
    }
    let cuts: Vec<Cut> = (0..=n)
        .map(|i| {
            let s = len * i as f32 / n as f32;
            let c = a + d * s;
            let along = if d.x.abs() > d.y.abs() { c.x } else { c.y };
            let jag = (noise(s * 0.9, 0) - 0.5) * 0.9 + (noise(s * 3.3, 1) - 0.5) * 0.35;
            let h = (top(along) + jag).max(0.0);
            let half = |k: u32| THICK * 0.5 + (noise(s * 1.7, k) - 0.5) * 0.08;
            let (ta, tb) = (half(2), -half(3));
            let drop = |k: u32| 0.04 + 0.3 * noise(s * 2.1, k);
            let at = |t: f32, y: f32| Vec3::new(c.x + across.x * t, y, c.y + across.y * t);
            let lean = (noise(s * 1.3, 6) - 0.5) * THICK * 0.4;
            Cut {
                foot: [at(ta, -0.05), at(tb, -0.05)],
                side: [at(ta, h - drop(4)), at(tb, h - drop(5))],
                ridge: at(lean, h),
                h,
            }
        })
        .collect();
    let out_a = Vec3::new(across.x, 0.0, across.y);
    let run = Vec3::new(d.x, 0.0, d.y);
    for i in 0..n {
        let (p, q) = (&cuts[i], &cuts[i + 1]);
        let (stand_p, stand_q) = (p.h > GONE, q.h > GONE);
        if stand_p && stand_q {
            face(b, [p.foot[0], q.foot[0], q.side[0], p.side[0]], out_a, false);
            face(b, [p.foot[1], q.foot[1], q.side[1], p.side[1]], -out_a, false);
            face(b, [p.side[0], q.side[0], q.ridge, p.ridge], out_a + Vec3::Y, true);
            face(b, [p.side[1], q.side[1], q.ridge, p.ridge], -out_a + Vec3::Y, true);
        }
        // The broken ends: where the wall stops, its section shows.
        let ends = [
            (i == 0 && stand_p, p, -run),
            (stand_p && !stand_q, p, run),
            (!stand_p && stand_q, q, -run),
            (i + 1 == n && stand_q, q, run),
        ];
        for (_, cut, out) in ends.into_iter().filter(|e| e.0) {
            face(b, [cut.foot[0], cut.foot[1], cut.side[1], cut.side[0]], out, false);
            face(b, [cut.side[0], cut.side[1], cut.ridge, cut.ridge], out, true);
        }
        // Rubble at the foot, both sides, most where the wall is lowest; stones on its top.
        let s = len * i as f32 / n as f32;
        let low = 1.0 - smooth(0.4, 2.4, p.h);
        if rng.f() < (0.18 + 0.5 * low) * STEP / 0.3 {
            let side = if rng.f() < 0.65 { 1.0 } else { -1.0 };
            let off = side * (THICK * 0.5 + rng.range(0.05, 0.9 + 0.8 * low));
            let at = a + d * (s + rng.range(-0.3, 0.3)) + across * off;
            let big = rng.range(0.5, 0.9 + 0.3 * low);
            fallen(b, rng, at, big);
        }
        if stand_p && rng.f() < 0.06 {
            let size = Vec3::new(rng.range(0.25, 0.5), rng.range(0.15, 0.25), rng.range(0.2, 0.4));
            let rot = Quat::from_euler(
                EulerRot::YXZ,
                rng.range(0.0, 3.0),
                rng.range(-0.3, 0.3),
                rng.range(-0.3, 0.3),
            );
            let shift = Vec2::new(rng.f(), rng.f());
            let tint = stone(rng);
            b.add(p.ridge - Vec3::Y * 0.04, size, rot, TILE, shift, tint);
        }
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
            (foot + o, foot + o * (1.0 - chip * 0.6) + Vec3::Y * (h - chip))
        })
        .collect();
    let crown = foot + Vec3::Y * (h - broken * 0.3);
    for k in 0..SIDES {
        let (a, b2) = (rim[k], rim[(k + 1) % SIDES]);
        let out = ((a.0 + b2.0) * 0.5 - foot).normalize_or(Vec3::X);
        face(b, [a.0, b2.0, b2.1, a.1], out, false);
        face(b, [a.1, b2.1, crown, crown], Vec3::Y, true);
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

    /// A wall stands to its height along its run and is gone where its height is nothing.
    #[test]
    fn wall_stands_where_its_top_is() {
        let mut b = Blocks::default();
        let mut rng = Rng::new(7);
        wall(&mut b, &mut rng, Vec2::ZERO, Vec2::new(8.0, 0.0), 1, |x| {
            if x < 4.0 { 2.0 } else { 0.0 }
        });
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
