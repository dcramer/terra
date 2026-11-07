//! The marsh's life about the fight: a tree monster wading the far channel, herons fishing
//! the mere's shallows (one striking, one stalking), a third flying over, and mallards
//! swimming the far channel in a line. The birds are the repository's own animals
//! (assets/models/creatures, not the Tripo pipeline). Each goes round a flat ellipse by the
//! effects' clock (a still is one moment, a film plays), facing along its way, playing one
//! looped clip (the ruin's `Pose`).

use super::land::{MERE, far, shape};
use super::{KNIGHT, LEVEL};
use crate::ruin::effects::kit::Clock;
use crate::ruin::fight::Pose;
use crate::scene::model;
use bevy::prelude::*;

const HERON: &str = "models/creatures/Heron.glb";
const MALLARD: &str = "models/creatures/Mallard.glb";
const TREE_MONSTER: &str = "models/iso/tree-monster.glb";

/// A body going round an ellipse: its middle, half-axes, angular rate (rad/s, 0 stands),
/// phase, how deep it may wade (m), height over the land or water, and whether its middle
/// line follows the far channel.
#[derive(Component, Clone, Copy)]
pub struct Round {
    pub mid: Vec2,
    pub radii: Vec2,
    pub rate: f32,
    pub phase: f32,
    pub wade: f32,
    pub lift: f32,
    pub channel: bool,
    /// Floats on the face rather than standing on the bottom.
    pub floats: bool,
}

impl Round {
    /// Where it is at time `t` (x, z) and which way it heads.
    pub fn at(&self, t: f32) -> (Vec2, Vec2) {
        let a = self.rate * t + self.phase;
        let p = |a: f32| {
            let q = self.mid + Vec2::new(a.cos() * self.radii.x, a.sin() * self.radii.y);
            if self.channel {
                Vec2::new(q.x, q.y + far(q.x) - far(self.mid.x))
            } else {
                q
            }
        };
        let here = p(a);
        let dir = if self.rate == 0.0 {
            KNIGHT - here
        } else {
            (p(a + 0.05 * self.rate.signum()) - here).normalize_or(Vec2::Y)
        };
        (here, dir)
    }

    pub fn height(&self, p: Vec2) -> f32 {
        let y = if self.floats {
            LEVEL - 0.04
        } else {
            shape(p).max(LEVEL - self.wade)
        };
        y + self.lift
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, go);
}

fn spawn(mut commands: Commands, asset: Res<AssetServer>) {
    let still = Round {
        mid: Vec2::ZERO,
        radii: Vec2::ZERO,
        rate: 0.0,
        phase: 0.0,
        wade: 0.25,
        lift: 0.0,
        channel: false,
        floats: false,
    };
    let fc = Vec2::new(-1.5, far(-1.5) - 0.2);
    let cast = [
        // The tree monster wading the far channel's length behind the fight.
        (
            TREE_MONSTER,
            "walk",
            1.0,
            Round {
                mid: fc,
                radii: Vec2::new(3.2, 0.45),
                rate: 0.12,
                phase: 1.2,
                wade: 0.4,
                ..still
            },
        ),
        // A heron striking in the mere's shallows, one stalking round its far side.
        (
            HERON,
            "Strike",
            1.0,
            Round {
                mid: MERE + Vec2::new(-1.9, 0.6),
                wade: 0.25,
                ..still
            },
        ),
        (
            HERON,
            "Walk",
            1.0,
            Round {
                mid: MERE,
                radii: Vec2::new(1.95, 1.8),
                rate: -0.08,
                phase: 4.6,
                wade: 0.25,
                ..still
            },
        ),
        // A third crossing high over the marsh, round again off the frame's edges.
        (
            HERON,
            "Fly",
            1.0,
            Round {
                mid: Vec2::new(0.0, -2.0),
                radii: Vec2::new(16.0, 7.0),
                rate: 0.16,
                phase: 3.5,
                lift: 5.0,
                floats: true,
                ..still
            },
        ),
        // Mallards in a line along the far channel.
        (
            MALLARD,
            "Swim",
            0.9,
            Round {
                mid: Vec2::new(4.0, far(4.0)),
                radii: Vec2::new(2.6, 0.35),
                rate: 0.12,
                phase: 0.0,
                channel: true,
                floats: true,
                ..still
            },
        ),
        (
            MALLARD,
            "Swim",
            0.9,
            Round {
                mid: Vec2::new(4.0, far(4.0)),
                radii: Vec2::new(2.6, 0.35),
                rate: 0.12,
                phase: -0.22,
                channel: true,
                floats: true,
                ..still
            },
        ),
        (
            MALLARD,
            "Dabble",
            0.9,
            Round {
                mid: Vec2::new(4.0, far(4.0)),
                radii: Vec2::new(2.6, 0.35),
                rate: 0.12,
                phase: -0.5,
                channel: true,
                floats: true,
                ..still
            },
        ),
    ];
    for (i, (path, clip, scale, round)) in cast.into_iter().enumerate() {
        let (p, dir) = round.at(0.0);
        let body = model(
            &mut commands,
            &asset,
            path,
            p.extend(round.height(p)).xzy(),
            dir.x.atan2(dir.y),
            scale,
        );
        commands.entity(body).insert((
            round,
            Pose {
                gltf: asset.load(path),
                clip,
                at: 0.37 * i as f32,
                once: false,
            },
        ));
    }
}

/// Each body to its place on its round, facing its way (the models face +Z).
fn go(clock: Res<Clock>, mut bodies: Query<(&Round, &mut Transform)>) {
    for (r, mut tr) in &mut bodies {
        if r.rate == 0.0 {
            continue;
        }
        let (p, dir) = r.at(clock.t);
        tr.translation = p.extend(r.height(p)).xzy();
        tr.rotation = Quat::from_rotation_y(dir.x.atan2(dir.y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mallards keep to open water along the far channel all the way round.
    #[test]
    fn the_mallards_keep_to_the_water() {
        let r = Round {
            mid: Vec2::new(4.0, far(4.0)),
            radii: Vec2::new(2.6, 0.35),
            rate: 0.12,
            phase: 0.0,
            wade: 0.0,
            lift: 0.0,
            channel: true,
            floats: true,
        };
        for i in 0..100 {
            let (p, _) = r.at(i as f32 * 0.6);
            assert!(
                shape(p) < LEVEL - 0.1,
                "a mallard aground at {p}: {}",
                shape(p)
            );
        }
    }
}
