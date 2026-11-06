//! Pigeons perched on the ruin's back wall's broken top and on the toppled drum: each sits
//! until a blow lands near, then takes off, circles once over the court clear of the fight,
//! and lands where it sat. The Pigeon is the repository's (assets/models/creatures, not the
//! Tripo pipeline), its own Fly and Idle.

use super::{Blows, Gait};
use crate::ruin::effects::kit::Clock;
use crate::ruin::walls::{BACK, back_top, ridge};
use bevy::prelude::*;

const PIGEON: &str = "models/creatures/Pigeon.glb";
const CLIPS: [&str; 2] = ["Fly", "Idle"];
/// 0.31 m as it comes: a little over, a 0.37 m rock dove, about 35 px at the game camera.
const SCALE: f32 = 1.2;
/// A flight's length (s) and how high it rises over its perch (m), so it stays in frame.
const FLIGHT: f32 = 5.0;
const LIFT: f32 = 1.8;

/// The birds: where each perches (on the back wall's top at an x, or a fixed point), the
/// middle of its circle, which way round (1 anticlockwise seen from above), its rest
/// between flights (s), and where in its flight a still holds it (below 0, perched).
pub const BIRDS: [(Perch, Vec2, f32, f32, f32); 3] = [
    (Perch::Wall(1.8), Vec2::new(2.6, -5.0), 1.0, 1.0, 1.6),
    (Perch::Wall(4.3), Vec2::new(4.6, -4.6), -1.0, 4.0, -1.0),
    // On the drum toppled from the short pillar (walls.rs), its top 0.78 m up.
    (
        Perch::At(Vec3::new(5.4, 0.78, 1.2)),
        Vec2::new(4.6, 3.0),
        1.0,
        7.0,
        -1.0,
    ),
];

#[derive(Clone, Copy)]
pub enum Perch {
    Wall(f32),
    At(Vec3),
}

impl Perch {
    pub fn at(self) -> Vec3 {
        match self {
            Perch::Wall(x) => ridge(BACK.0, BACK.1, BACK.2, back_top, Vec2::new(x, BACK.0.y)),
            Perch::At(p) => p,
        }
    }
}

/// A bird: which of `BIRDS` it is, its perch, and when its last flight began.
#[derive(Component)]
pub struct Flock {
    which: usize,
    perch: Vec3,
    start: f32,
}

pub fn spawn(mut commands: Commands, asset: Res<AssetServer>) {
    for (which, (perch, ..)) in BIRDS.into_iter().enumerate() {
        let at = perch.at();
        let body = crate::scene::model(&mut commands, &asset, PIGEON, at, 0.0, SCALE);
        commands.entity(body).insert((
            Flock {
                which,
                perch: at,
                start: -FLIGHT - 10.0,
            },
            Gait::new(asset.load(PIGEON), CLIPS, 0.3 * which as f32),
            crate::rim::Fighter::default(),
        ));
    }
}

/// Where bird `which` is `tau` s into its flight from `perch`, its heading, and whether it flies.
pub fn pose(which: usize, perch: Vec3, tau: f32) -> (Vec3, Vec2, bool) {
    let (_, mid, way, ..) = BIRDS[which];
    let at = |tau: f32| {
        let u = (tau / FLIGHT).clamp(0.0, 1.0);
        let e = u * u * (3.0 - 2.0 * u);
        let from = perch.xz() - mid;
        let a = from.y.atan2(from.x) + way * std::f32::consts::TAU * e;
        let p = mid + Vec2::new(a.cos(), a.sin()) * from.length();
        p.extend(perch.y + LIFT * (std::f32::consts::PI * u).sin())
            .xzy()
    };
    let flying = tau > 0.0 && tau < FLIGHT;
    let t = tau.clamp(0.01, FLIGHT - 0.11);
    let dir = (at(t + 0.1) - at(t)).xz().normalize_or(Vec2::Y);
    (at(tau), dir, flying)
}

pub fn go(
    clock: Res<Clock>,
    blows: Res<Blows>,
    mut birds: Query<(&mut Flock, &mut Gait, &mut Transform)>,
) {
    for (mut b, mut gait, mut tr) in &mut birds {
        let (.., rest, still) = BIRDS[b.which];
        if blows.last > b.start + FLIGHT + rest {
            b.start = blows.last;
        }
        let tau = if clock.still {
            still
        } else {
            clock.t - b.start
        };
        let (at, dir, flying) = pose(b.which, b.perch, tau);
        tr.translation = at;
        tr.rotation = Quat::from_rotation_y(dir.x.atan2(dir.y));
        gait.moving = flying;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ruin::effects::Stage;

    /// Every bird's circle keeps clear of the fighters (each a 3.4 m column), the braziers'
    /// flames and the pillars, and it comes back to its perch.
    #[test]
    fn the_birds_circle_clear_of_the_fight() {
        let stage = Stage::default();
        let fighters = [stage.knight, stage.foe, stage.target].map(|p| (p, 3.4, 2.0));
        let posts = crate::ruin::fire::BRAZIERS
            .map(|p| (p, 1.9, 0.6))
            .into_iter()
            .chain(crate::ruin::walls::PILLARS.map(|p| (p, 3.3, 0.5)));
        let columns: Vec<_> = fighters.into_iter().chain(posts).collect();
        for (which, (perch, ..)) in BIRDS.into_iter().enumerate() {
            let home = perch.at();
            assert!(home.y > 0.5, "bird {which} perches on nothing at {home}");
            for i in 0..=100 {
                let (p, _, _) = pose(which, home, FLIGHT * i as f32 / 100.0);
                for (c, h, clear) in &columns {
                    let d = Vec2::new(p.xz().distance(*c), (p.y - h).max(0.0)).length();
                    assert!(
                        d > *clear,
                        "bird {which} at {p} is {d} m from a column at {c}"
                    );
                }
            }
            let (back, _, flying) = pose(which, home, FLIGHT + 0.5);
            assert!(
                back.distance(home) < 1e-3 && !flying,
                "bird {which} does not land"
            );
            assert!(pose(which, home, 1.0).2, "bird {which} does not fly");
        }
    }
}
