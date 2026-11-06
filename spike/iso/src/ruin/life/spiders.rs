//! Spiders on the ruin's stones, each by a torch's light so it reads: two on the back
//! wall's face either side of the fight, one on the tall pillar. Each rests until a blow
//! lands, then runs from the fight up its way in two dashes, stops at the far end, and
//! creeps back, so in a film they scuttle and stop. The Spider is the repository's
//! (assets/models/creatures/Spider.glb, not the Tripo pipeline), its own walk and idle.

use super::{Blows, Gait, along, heading, point};
use crate::ruin::effects::kit::Clock;
use crate::ruin::walls::{BACK_Z, PILLARS, THICK};
use bevy::prelude::*;

const SPIDER: &str = "models/creatures/Spider.glb";
const CLIPS: [&str; 2] = ["SpiderArmature|Spider_Walk", "SpiderArmature|Spider_Idle"];
/// The model spans 5.75 m across its legs: a tenth makes a 0.58 m ruin spider, about 50 px
/// at the game camera, as the ARPGs' rats and spiders stand beside their heroes.
const SCALE: f32 = 0.1;
/// How fast the walk clip carries it at its scale (m/s): its clip plays at its pace over this.
const STRIDE: f32 = 0.5;
/// How far it stands proud of the stone it clings to (m): over the face's few cm of bulge.
const PROUD: f32 = 0.06;
/// The flight from a blow: how far along its way (0 at the fight's end, 1 the far) against
/// the time since the blow (s): a dash, a stop, a dash to the end, a wait, a creep back.
pub const FLEE: [(f32, f32); 6] = [
    (0.0, 0.0),
    (0.6, 0.45),
    (1.5, 0.45),
    (2.0, 1.0),
    (4.4, 1.0),
    (9.5, 0.0),
];

/// What a spider clings to: the back wall's south face (a way's x, height), or a pillar's
/// round (a way's arc length about it from due south, height).
#[derive(Clone, Copy)]
pub enum Stone {
    Face,
    Pillar(Vec2, f32),
}

impl Stone {
    /// The point on the stone for a way's (u, height), and the stone's outward normal there.
    pub fn at(self, p: Vec2) -> (Vec3, Vec3) {
        match self {
            Stone::Face => (Vec3::new(p.x, p.y, BACK_Z + THICK * 0.5 + PROUD), Vec3::Z),
            Stone::Pillar(mid, r) => {
                let a = p.x / r;
                let n = Vec3::new(a.sin(), 0.0, a.cos());
                (Vec3::new(mid.x, p.y, mid.y) + n * (r + PROUD), n)
            }
        }
    }
}

/// The spiders: the stone each clings to, its way over it from the fight's end, how long it
/// rests between flights (s), and where in its flight a still holds it.
pub const SPIDERS: [(Stone, &[Vec2], f32, f32); 3] = [
    // Left of the torch by the breach, up and away from the fight.
    (
        Stone::Face,
        &[
            Vec2::new(0.5, 0.5),
            Vec2::new(0.1, 1.1),
            Vec2::new(-0.4, 0.8),
        ],
        0.0,
        0.35,
    ),
    // Right of the banner, where the wall falls away.
    (
        Stone::Face,
        &[
            Vec2::new(3.95, 0.6),
            Vec2::new(4.3, 1.2),
            Vec2::new(4.7, 0.75),
        ],
        2.5,
        3.0,
    ),
    // Up the tall pillar's south side, round from its torch.
    (
        Stone::Pillar(PILLARS[0], 0.4),
        &[
            Vec2::new(0.3, 0.5),
            Vec2::new(0.12, 1.3),
            Vec2::new(0.25, 2.0),
        ],
        5.0,
        6.5,
    ),
];

/// A spider: which of `SPIDERS` it is and when its last flight began (the clock's time).
#[derive(Component)]
pub struct Scuttle {
    which: usize,
    start: f32,
}

pub fn spawn(mut commands: Commands, asset: Res<AssetServer>) {
    for (which, (stone, way, ..)) in SPIDERS.into_iter().enumerate() {
        let (at, _) = stone.at(way[0]);
        let body = crate::scene::model(&mut commands, &asset, SPIDER, at, 0.0, SCALE);
        commands.entity(body).insert((
            Scuttle {
                which,
                start: -FLEE[FLEE.len() - 1].0,
            },
            Gait::new(asset.load(SPIDER), CLIPS, 0.2 * which as f32),
            crate::rim::Fighter::default(),
        ));
    }
}

/// Where spider `which` is `tau` s into its flight, its rotation (belly to the stone, head
/// along its way), and its pace (m/s, 0 at rest).
pub fn pose(which: usize, tau: f32) -> (Vec3, Quat, f32) {
    let (stone, way, ..) = SPIDERS[which];
    let frac = along(&FLEE, tau);
    let (p, len) = point(way, frac);
    let ahead = point(way, frac + heading(&FLEE, tau) * 0.02).0;
    let (at, up) = stone.at(p);
    let dir = stone.at(ahead).0 - at;
    let fwd = (dir - up * dir.dot(up)).normalize_or(Vec3::Y);
    let rot = Quat::from_mat3(&Mat3::from_cols(up.cross(fwd), up, fwd));
    let pace = (along(&FLEE, tau + 0.05) - frac).abs() / 0.05 * len;
    (at, rot, pace)
}

/// Each spider starts its flight at the first blow after its rest, and goes through it.
pub fn go(
    clock: Res<Clock>,
    blows: Res<Blows>,
    mut spiders: Query<(&mut Scuttle, &mut Gait, &mut Transform)>,
) {
    let end = FLEE[FLEE.len() - 1].0;
    for (mut s, mut gait, mut tr) in &mut spiders {
        let (.., rest, still) = SPIDERS[s.which];
        if blows.last > s.start + end + rest {
            s.start = blows.last;
        }
        let tau = if clock.still {
            still
        } else {
            clock.t - s.start
        };
        let (at, rot, pace) = pose(s.which, tau);
        tr.translation = at;
        tr.rotation = rot;
        gait.moving = pace > 0.01;
        gait.speed = (pace / STRIDE).clamp(0.4, 3.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ruin::effects::Stage;
    use crate::ruin::walls::{BACK, back_top, ridge};

    /// Every spider keeps to its stone, clear of the fight, the torches and the banner, all
    /// through its flight: on the wall's face below its broken top and over the rubble at
    /// its foot, or on the pillar's south side below its top.
    #[test]
    fn the_spiders_keep_to_the_stones_clear_of_the_fight() {
        let stage = Stage::default();
        let torches = [Vec3::new(-6.2, 1.5, BACK_Z), Vec3::new(1.2, 1.5, BACK_Z)];
        for (which, (stone, ..)) in SPIDERS.into_iter().enumerate() {
            for i in 0..=200 {
                let tau = i as f32 * 0.05;
                let (at, rot, pace) = pose(which, tau);
                let flat = at.xz();
                for fighter in [stage.knight, stage.foe, stage.target] {
                    let d = flat.distance(fighter);
                    assert!(d > 2.5, "spider {which} at {at} is {d} m from a fighter");
                }
                assert!(at.y > 0.4, "spider {which} at {at} is down in the rubble");
                assert!(pace < 2.0, "spider {which} runs {pace} m/s");
                // Belly to the stone.
                let (_, up) = stone.at(Vec2::ZERO);
                if let Stone::Face = stone {
                    assert!(
                        (rot * Vec3::Y).dot(up) > 0.99,
                        "spider {which} is off the face"
                    );
                    let top = ridge(BACK.0, BACK.1, BACK.2, back_top, flat).y;
                    assert!(
                        at.y < top - 0.7,
                        "spider {which} at {at} is over the top {top}"
                    );
                    for t in torches {
                        let d = Vec2::new(at.x - t.x, at.y - t.y).length();
                        assert!(d > 0.6, "spider {which} at {at} is in a torch");
                    }
                    let banner = at.x > 2.2 && at.x < 3.6 && at.y > 0.6;
                    assert!(!banner, "spider {which} at {at} is under the banner");
                } else {
                    assert!(
                        at.y < 2.4,
                        "spider {which} at {at} is over the pillar's top"
                    );
                    // The pillar's torch hangs on its west side.
                    assert!(
                        at.x > PILLARS[0].x - 0.1,
                        "spider {which} at {at} is by the torch"
                    );
                }
            }
        }
    }

    /// A spider runs from the fight when the blow lands: its first dash takes it further
    /// from the knight.
    #[test]
    fn a_spider_runs_from_the_fight() {
        let stage = Stage::default();
        for which in 0..SPIDERS.len() {
            let d = |tau: f32| pose(which, tau).0.xz().distance(stage.knight);
            let rest = pose(which, 0.0).0;
            let dashed = pose(which, 2.2).0;
            assert!(dashed.distance(rest) > 0.5, "spider {which} hardly moves");
            assert!(
                pose(which, 0.3).2 > 0.1,
                "spider {which} is not running at first"
            );
            assert!(pose(which, 1.0).2 < 0.01, "spider {which} does not stop");
            assert!(
                d(2.2) + 0.01 >= d(0.0) || dashed.y > rest.y,
                "spider {which} runs at the fight"
            );
        }
    }
}
