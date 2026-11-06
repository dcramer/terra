//! The ruin's walls: a back wall of coursed rubble broken to every height with a breach in
//! it, a low broken wall down the west side, two pillars on the east standing to different
//! heights, and the stones that fell from them lying at their feet.

use super::blocks::Blocks;
use crate::forest::Rng;
use crate::scene::material;
use bevy::prelude::*;

/// The back wall's middle line (north), the west wall's, and the two pillars.
pub const BACK_Z: f32 = -6.5;
pub const WEST_X: f32 = -9.0;
pub const THICK: f32 = 0.7;
pub const PILLARS: [Vec2; 2] = [Vec2::new(6.6, -3.2), Vec2::new(7.2, 1.6)];
/// castle_wall_varriation is 2 m across.
const TILE: f32 = 2.0;

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
        Vec2::new(-10.5, BACK_Z - THICK * 0.5),
        Vec2::new(5.5, BACK_Z - THICK * 0.5),
        |x| {
            let breach = 1.0 - super::ground::smooth(0.6, 1.6, (x + 2.6).abs());
            let top = 2.3 + 1.2 * (x * 0.55).sin() + 0.5 * (x * 1.7 + 1.0).sin();
            (top * (1.0 - breach) + 0.3 * breach).max(0.3) * (1.0 - 0.6 * smooth_end(x, 5.5))
        },
    );
    // The west wall: low, two or three courses, gone in places.
    wall(
        &mut b,
        &mut rng,
        Vec2::new(WEST_X, BACK_Z - THICK),
        Vec2::new(WEST_X, 4.0),
        |z| (0.9 + 0.6 * (z * 0.8).sin() + 0.3 * (z * 2.3).sin()).max(0.0),
    );
    // The pillars: drums stacked, each turned a little on the last, the top ones fallen.
    for (p, drums) in [(PILLARS[0], 6), (PILLARS[1], 3)] {
        let mut y = 0.0;
        for _ in 0..drums {
            let h = rng.range(0.45, 0.6);
            let yaw = Quat::from_rotation_y(rng.range(-0.06, 0.06));
            let tint = stone(&mut rng);
            let shift = Vec2::new(rng.f(), rng.f());
            b.add(
                Vec3::new(p.x, y + h * 0.5, p.y),
                Vec3::new(0.8, h - 0.015, 0.8),
                yaw,
                TILE,
                shift,
                tint,
            );
            y += h;
        }
    }
    // Fallen stones: at the walls' feet and across the floor from the pillars, lying any way.
    let heaps = [
        (Vec2::new(-2.6, BACK_Z + 1.0), 9, 1.6),
        (Vec2::new(-7.0, BACK_Z + 1.0), 5, 1.2),
        (Vec2::new(3.4, BACK_Z + 0.9), 4, 1.0),
        (Vec2::new(WEST_X + 1.0, -1.5), 4, 1.0),
        (Vec2::new(PILLARS[1].x - 1.0, PILLARS[1].y + 1.2), 3, 1.2),
    ];
    for (at, n, spread) in heaps {
        for _ in 0..n {
            let size = Vec3::new(
                rng.range(0.35, 0.8),
                rng.range(0.25, 0.4),
                rng.range(0.4, THICK),
            );
            let off = Vec2::new(rng.range(-1.0, 1.0), rng.range(-0.6, 0.6)) * spread;
            let rot = Quat::from_euler(
                EulerRot::YXZ,
                rng.range(0.0, 6.28),
                rng.range(-0.25, 0.25),
                rng.range(-0.25, 0.25),
            );
            let tint = stone(&mut rng);
            let shift = Vec2::new(rng.f(), rng.f());
            let c = at + off;
            b.add(
                Vec3::new(c.x, size.y * 0.4, c.y),
                size,
                rot,
                TILE,
                shift,
                tint,
            );
        }
    }
    // A pillar's drum toppled and rolled toward the fight.
    b.add(
        Vec3::new(PILLARS[1].x - 1.8, 0.38, PILLARS[1].y - 0.4),
        Vec3::new(0.78, 0.5, 0.78),
        Quat::from_rotation_y(0.5) * Quat::from_rotation_z(1.45),
        TILE,
        Vec2::new(0.3, 0.7),
        stone(&mut rng),
    );
    commands.spawn((
        Mesh3d(meshes.add(b.mesh())),
        MeshMaterial3d(materials.add(material(&asset, "castle_wall_varriation", Color::WHITE))),
    ));
}

/// A wall's last metres crumbling to its end.
fn smooth_end(x: f32, end: f32) -> f32 {
    super::ground::smooth(end - 2.5, end, x)
}

fn stone(rng: &mut Rng) -> Color {
    let v = rng.range(0.6, 1.0);
    Color::srgb(v, v * 0.96, v * 0.9)
}

/// A wall of courses from `a` to `b` (its north face's line), standing `top(along)` high:
/// blocks of every length in courses of every height, each set a little proud or back of
/// the face, the topmost broken off in places.
fn wall(b: &mut Blocks, rng: &mut Rng, a: Vec2, z: Vec2, top: impl Fn(f32) -> f32) {
    let d = (z - a).normalize();
    let len = (z - a).length();
    let rot = Quat::from_rotation_y((-d.y).atan2(d.x));
    // Into the wall from its line: the left of its run.
    let across = Vec2::new(d.y, -d.x);
    let along_of = |s: f32| {
        if d.x.abs() > d.y.abs() {
            a.x + d.x * s
        } else {
            a.y + d.y * s
        }
    };
    let mut y = 0.0;
    let mut course = 0;
    while y < 4.0 {
        let h = rng.range(0.28, 0.42);
        let mut s = if course % 2 == 0 { 0.0 } else { -0.3 };
        while s < len {
            let l = rng.range(0.45, 0.9);
            let mid = (s + l * 0.5).clamp(0.0, len);
            s += l;
            let stand = top(along_of(mid));
            // The block stands if the wall stands to its middle; the top course ragged.
            if y + h * 0.5 > stand || (y + h > stand && rng.f() < 0.4) {
                continue;
            }
            let proud = rng.range(-0.02, 0.02);
            let c = a + d * mid - across * (THICK * 0.5 + proud);
            let jitter = Quat::from_rotation_y(rng.range(-0.02, 0.02));
            let tint = stone(rng);
            let shift = Vec2::new(rng.f(), rng.f());
            b.add(
                Vec3::new(c.x, y + h * 0.5, c.y),
                Vec3::new(l - 0.02, h - 0.02, THICK + rng.range(-0.04, 0.04)),
                rot * jitter,
                TILE,
                shift,
                tint,
            );
        }
        y += h;
        course += 1;
    }
}
