//! What frames the forest's fight: oaks in the frame (judge forest-r1: only their shadows
//! showed, a lawn, not a wood), their trunks and lower crowns at its edges and one before
//! the fight, which opens in a dithered hole about each of the cast as Diablo IV's and Path
//! of Exile 2's trees do (foliage.rs's hole, `foliage::Seen`); and ferns and mossy stones
//! over the bare bank and the dark corner below the fight.

use crate::forest::{Extra, Rng, Spot};
use crate::terrain::{self, Land};
use bevy::prelude::*;

/// The oaks, placed by projecting the camera (target/job/inv.mjs's sums, 960x540): the
/// lower-left corner, near the camera, its crown up the left edge; the top-left and top
/// edge, trunks and lower boughs going off the frame; the top right, two behind the
/// moose's walk; before the fight, by the camera, its crown rising up the frame
/// over the cast and open about each of them. At true proportions (forest::TALL, iso-kit
/// j24): trunks 0.85 to 1.1 m across, 12 to 16 m tall, crowns about as wide as at the
/// models' old 0.6 to 0.8.
pub const OAKS: [(Vec2, f32, f32); 6] = [
    (Vec2::new(0.3, 11.6), 0.345, 1.0),
    (Vec2::new(-4.3, 2.4), 0.32, 2.3),
    (Vec2::new(0.9, -1.6), 0.3, 4.1),
    (Vec2::new(7.4, -4.4), 0.32, 0.4),
    (Vec2::new(12.0, -3.6), 0.37, 5.2),
    (Vec2::new(4.5, 9.8), 0.28, 3.0),
];

/// The bare ground dressed: the pond's north bank (x 7.5 to 13.5, z -4.5 to 3.7, the frame's
/// upper right) and the corner below the fight (x -2 to 3, z 9 to 12).
const PATCHES: [(Vec2, Vec2, usize); 2] = [
    (Vec2::new(7.5, -4.5), Vec2::new(13.5, 3.7), 34),
    (Vec2::new(-2.0, 9.0), Vec2::new(3.0, 12.0), 14),
];

pub fn plugin(app: &mut App) {
    app.insert_resource(Extra(
        OAKS.iter()
            .map(|&(at, scale, yaw)| Spot { at, scale, yaw })
            .collect(),
    ))
    .add_systems(Startup, cover.after(crate::scene::build));
}

/// Whether a plant may stand at `p`: dry, off the moose's walk along the bank, clear of the
/// cast and of the oaks' feet.
pub fn free(p: Vec2) -> bool {
    let cast = [super::KNIGHT, super::FOE, super::TARGET, super::SECOND];
    terrain::pond(p) < -0.15
        && !(p.x > 7.0 && p.x < 11.5 && (p.y - 0.5).abs() < 1.0)
        && cast.iter().all(|c| c.distance(p) > 1.8)
        && OAKS.iter().all(|(o, ..)| o.distance(p) > 0.9)
        && p.distance(crate::scene::FIRE) > 3.0
}

fn cover(mut commands: Commands, asset: Res<AssetServer>, land: Res<Land>) {
    let mut rng = Rng::new(0xd2e55);
    let on = |p: Vec2| Vec3::new(p.x, land.height(p), p.y);
    for (lo, hi, n) in PATCHES {
        let mut placed = 0;
        let mut tries = 0;
        while placed < n && tries < 2_000 {
            tries += 1;
            let p = Vec2::new(rng.range(lo.x, hi.x), rng.range(lo.y, hi.y));
            if !free(p) {
                continue;
            }
            // Ferns in the main; one in five a mossy stone, set well into the soil (a
            // third of its height and more: at a quarter they sat on it as lumps, judge
            // 261107-0507), mossed and grimed (rim.rs `Stone`).
            let stone = rng.f() >= 0.8;
            let (path, scale, sink) = if !stone {
                (
                    "models/nature/forest/fern.glb".to_string(),
                    rng.range(0.9, 1.6),
                    0.05,
                )
            } else {
                let i = 1 + (rng.f() * 7.0) as usize % 7;
                let s = rng.range(0.5, 1.25);
                (
                    format!("models/nature/forest/mossy_rock_b_{i}.glb"),
                    s,
                    0.34 * s,
                )
            };
            let e = crate::scene::model(
                &mut commands,
                &asset,
                &path,
                on(p) - Vec3::Y * sink,
                rng.f() * std::f32::consts::TAU,
                scale,
            );
            if stone {
                commands
                    .entity(e)
                    .insert(crate::rim::Stone { floor: on(p).y });
            } else {
                commands.entity(e).insert(crate::rim::Shade);
            }
            placed += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The oaks stand on dry land, clear of the cast, the camp fire and the beasts' beats.
    #[test]
    fn the_oaks_stand_clear() {
        let cast = [
            super::super::KNIGHT,
            super::super::FOE,
            super::super::TARGET,
        ];
        for (p, ..) in OAKS {
            assert!(terrain::pond(p) < -0.1, "the oak at {p} is in the pond");
            assert!(
                p.distance(crate::scene::FIRE) > 3.5,
                "the oak at {p} is in the camp"
            );
            for c in cast {
                assert!(
                    c.distance(p) > 3.0,
                    "the oak at {p} is on the fighter at {c}"
                );
            }
        }
        // Each patch has room for its plants.
        for (lo, hi, _) in PATCHES {
            let room = (0..400)
                .filter(|k| {
                    let t = Vec2::new((k % 20) as f32 / 19.0, (k / 20) as f32 / 19.0);
                    free(lo + (hi - lo) * t)
                })
                .count();
            assert!(
                room > 40,
                "the patch at {lo} has room at {room} of 400 points"
            );
        }
    }
}
