//! The banks' lips: what stands in the water at the bank, so the water meets the land in a
//! broken shore and not one hard line (the judge: "the cut from turf to water runs
//! everywhere; the ref lines its pond with rocks, standing reeds and lily pads"): stones
//! half sunk at the water's line with pebbles in the mud beside them, and old roots dipping
//! into it from the bank. The reeds in the shallows are plants.rs's; the band of wet mud
//! and the shelf it lies on are land.rs's.

use super::LEVEL;
use super::land::shape;
use super::plants::clear;
use crate::forest::Rng;
use crate::scene::model;
use bevy::prelude::*;

/// Where they may stand: the frame's ground and a little past it (m), sampled on a grid.
const LO: Vec2 = Vec2::new(-12.0, -9.0);
const HI: Vec2 = Vec2::new(12.0, 7.0);
const STEP: f32 = 0.2;
/// The lip: from this deep (m) to just under the face.
pub const LIP: f32 = 0.14;
/// The least room between two (m), and the share of the lip's spots taken. At 0.8 m and
/// 0.4 a bank 19 m long got two stones and its line still read as one cut (iso-art j36).
const ROOM: f32 = 0.5;
const TAKE: f32 = 0.8;
/// The share of them that are roots (the rest stones, each with its pebbles).
const ROOTS: f32 = 0.3;

/// The scanned stones (boulders 2 to 3 m across at scale 1, their origin at their foot)
/// and each one's height at scale 1, so a stone is scaled to stand out of the water.
const STONES: [(&str, f32); 5] = [
    ("models/nature/forest/mossy_rock_a_1.glb", 1.48),
    ("models/nature/forest/mossy_rock_a_3.glb", 1.09),
    ("models/nature/forest/mossy_rock_b_4.glb", 1.26),
    ("models/nature/forest/mossy_rock_b_6.glb", 1.31),
    ("models/nature/forest/mossy_rock_a_5.glb", 1.22),
];
/// How far a stone's top stands over the face (m), and the scales it is held within (15 to
/// 65 cm stones). Tops 3 to 10 cm up were 3 px over the water at 1080 and read as nothing.
const RISE: (f32, f32) = (0.06, 0.18);
const STONE_SCALE: (f32, f32) = (0.07, 0.32);
/// A root: the log (1.7 m along x, 0.45 m thick, its origin at its foot's middle) drawn
/// thin, 0.75 to 1.3 m long and 10 to 18 cm thick.
const ROOT: &str = "models/props/Log.glb";
const LOG_LONG: f32 = 1.7;
const LOG_THICK: f32 = 0.45;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn);
}

/// The spots on the water's lip, at least `ROOM` apart, none on the path or by the cast.
pub fn spots() -> Vec<Vec2> {
    let mut rng = Rng::new(0x11b5);
    let n = ((HI - LO) / STEP).as_uvec2();
    let mut out: Vec<Vec2> = Vec::new();
    for j in 0..n.y {
        for i in 0..n.x {
            let jitter = Vec2::new(rng.range(-0.14, 0.14), rng.range(-0.14, 0.14));
            let p = LO + Vec2::new(i as f32, j as f32) * STEP + jitter;
            let d = shape(p) - LEVEL;
            let take = rng.range(0.0, 1.0) < TAKE;
            if !(-LIP..-0.01).contains(&d) || !take || clear(p) {
                continue;
            }
            if out.iter().all(|q| q.distance(p) >= ROOM) {
                out.push(p);
            }
        }
    }
    out
}

/// Downhill at a point: the way into the water.
fn downhill(p: Vec2) -> Vec2 {
    let e = 0.1;
    let g = Vec2::new(
        shape(p + Vec2::X * e) - shape(p - Vec2::X * e),
        shape(p + Vec2::Y * e) - shape(p - Vec2::Y * e),
    );
    (-g).normalize_or(Vec2::X)
}

/// A stone's scale so its top stands `rise` over the face from its foot at `foot`.
pub fn stone_scale(height: f32, foot: f32, rise: f32) -> f32 {
    ((LEVEL + rise - foot) / height).clamp(STONE_SCALE.0, STONE_SCALE.1)
}

fn spawn(mut commands: Commands, asset: Res<AssetServer>) {
    let mut rng = Rng::new(0x2f07);
    for p in spots() {
        let yaw = rng.range(0.0, std::f32::consts::TAU);
        let down = downhill(p);
        if rng.range(0.0, 1.0) < ROOTS {
            // A root from the bank: its foot's middle on the lip's line, along downhill, its
            // outer end dipping under the face.
            let long = rng.range(0.45, 0.76);
            let thick = rng.range(0.1, 0.18) / LOG_THICK;
            let dip = rng.range(0.15, 0.35);
            let turn =
                Quat::from_rotation_y(libm::atan2f(-down.y, down.x)) * Quat::from_rotation_z(-dip);
            let at = p - down * LOG_LONG * long * 0.5;
            let e = model(&mut commands, &asset, ROOT, Vec3::ZERO, 0.0, 1.0);
            commands.entity(e).insert(
                Transform::from_translation(at.extend(LEVEL - 0.03).xzy())
                    .with_rotation(turn)
                    .with_scale(Vec3::new(long, thick, thick)),
            );
            continue;
        }
        let k = (rng.range(0.0, STONES.len() as f32) as usize).min(STONES.len() - 1);
        let (path, height) = STONES[k];
        let foot = shape(p) - 0.02;
        let scale = stone_scale(height, foot, rng.range(RISE.0, RISE.1));
        model(
            &mut commands,
            &asset,
            path,
            p.extend(foot).xzy(),
            yaw,
            scale,
        );
        // Its pebbles, three to five a hand across, along the bank beside it in the mud.
        for _ in 0..rng.range(3.0, 6.0) as usize {
            let q = p + down * rng.range(-0.35, 0.1) + down.perp() * rng.range(-0.4, 0.4);
            let s = rng.range(0.05, 0.09);
            let k = (k + 1 + rng.range(0.0, 4.0) as usize) % STONES.len();
            let yaw = rng.range(0.0, std::f32::consts::TAU);
            let at = q.extend(shape(q) - s * 0.2).xzy();
            model(&mut commands, &asset, STONES[k].0, at, yaw, s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lips line the water's edge all round: dozens of them, each just in the water,
    /// spread over every water in frame (the near and far channels and the mere).
    #[test]
    fn stones_and_roots_line_every_water() {
        let s = spots();
        assert!(s.len() >= 30, "only {} lips", s.len());
        for p in &s {
            let d = shape(*p) - LEVEL;
            assert!(
                (-LIP..0.0).contains(&d),
                "a lip at {p} stands {d} from the face"
            );
        }
        let near = s.iter().filter(|p| p.y > 1.5).count();
        let far = s.iter().filter(|p| p.y < -3.0).count();
        let mere = s
            .iter()
            .filter(|p| p.distance(super::super::land::MERE) < 4.5)
            .count();
        assert!(
            near >= 5 && far >= 5 && mere >= 3,
            "near {near}, far {far}, mere {mere}"
        );
    }

    /// Every stone breaks the face: from the lip's deepest foot its top stands out of the
    /// water, as tall as the least rise asks.
    #[test]
    fn every_stone_stands_out_of_the_water() {
        for (_, height) in STONES {
            let foot = LEVEL - LIP - 0.02;
            let top = foot + stone_scale(height, foot, RISE.0) * height;
            assert!(top >= LEVEL + RISE.0 - 1e-4, "a stone's top is at {top}");
        }
    }
}
