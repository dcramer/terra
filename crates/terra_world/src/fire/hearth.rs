//! A hearth's logs as its flames are laid on them (`fire.wesl`'s `hearth` and `hearth_tongue`):
//! a campfire's flame rises off the burning wood, along each log's length and tallest where
//! the logs meet, so its body follows its own logs rather than standing as a cluster in the
//! middle. Shipped games put the flame on the fuel this way: Unreal's Niagara spawns particles
//! on a mesh's surface (its Static Mesh data interface, "Static Mesh Location"), and a
//! flipbook campfire lays a card every log's width or so along each log's top (RealTimeVFX's
//! campfire breakdowns; research/2026-10-23-campfire-one-body.md).

use super::{Blaze, Blazes, Kindled};
use bevy::prelude::*;

/// How a hearth's logs lie, in its model, over its fire's foot: so many leaned in a teepee,
/// evenly round it, the first at bearing `turn` (radians from the model's +X toward +Z), each
/// foot `foot` metres out and `foot_up` up, each top `top` out and `top_up` up, turned `twist`
/// on past its foot (the logs cross rather than meet in a point), and `radius` thick.
/// No logs (`count` 0), and the flames stand round the middle as a brazier's would.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Logs {
    pub count: u32,
    pub turn: f32,
    pub twist: f32,
    pub radius: f32,
    pub foot: f32,
    pub foot_up: f32,
    pub top: f32,
    pub top_up: f32,
}

impl Logs {
    /// The campfire's (`tools/props/hearth.ts`, `layout(1)`, its means; its test holds them):
    /// seven split logs from 0.34 m out to 0.11 m, meeting 0.24 m over its base.
    pub const CAMPFIRE: Logs = Logs {
        count: 7,
        turn: -0.020,
        twist: 0.696,
        radius: 0.091,
        foot: 0.343,
        foot_up: 0.091,
        top: 0.112,
        top_up: 0.241,
    };

    /// None: the flames round the middle.
    pub const NONE: Logs = Logs {
        count: 0,
        turn: 0.0,
        twist: 0.0,
        radius: 0.0,
        foot: 0.0,
        foot_up: 0.0,
        top: 0.0,
        top_up: 0.0,
    };

    /// The same logs as seen from a foot so far over the model's base.
    pub const fn below(self, metres: f32) -> Logs {
        Logs {
            foot_up: self.foot_up - metres,
            top_up: self.top_up - metres,
            ..self
        }
    }

    /// What `fire.wesl`'s `hearth_log` reads: how many, the first's bearing in the world (in
    /// the frame the shader lays a hearth's cards in round `up`), the twist and thickness; then
    /// the feet's and tops' reach and height, all in metres as the hearth stands.
    pub fn packed(self, placed: &GlobalTransform) -> [Vec4; 2] {
        let (scale, turned, at) = placed.to_scale_rotation_translation();
        let up = at.normalize_or(Vec3::Y);
        let (east, north) = frame(up);
        let x = turned * Vec3::X;
        let bearing = libm::atan2f(x.dot(north), x.dot(east)) + self.turn;
        let s = scale.x;
        [
            Vec4::new(self.count as f32, bearing, self.twist, self.radius * s),
            Vec4::new(
                self.foot * s,
                self.foot_up * s,
                self.top * s,
                self.top_up * s,
            ),
        ]
    }
}

/// The fires kept where something kindled stands, each with a number of its own from its
/// place, so no two flicker alike, and the logs each burns on.
pub(super) fn keep(kindled: Query<(&Kindled, &GlobalTransform)>, mut blazes: ResMut<Blazes>) {
    if kindled.is_empty() && blazes.kept.is_empty() {
        return;
    }
    let hearths: Vec<[Vec4; 2]> = kindled
        .iter()
        .map(|(fire, placed)| fire.logs.packed(placed))
        .collect();
    let kept: Vec<Blaze> = kindled
        .iter()
        .map(|(fire, at)| {
            let at = at.translation();
            Blaze {
                at,
                tall: fire.tall,
                heat: fire.heat,
                seed: (at.x * 0.731 + at.z * 0.377).rem_euclid(1.0),
                wide: fire.wide,
            }
        })
        .collect();
    if kept != blazes.kept || hearths != blazes.hearths {
        blazes.kept = kept;
        blazes.hearths = hearths;
    }
}

/// The frame a hearth's cards are laid in round its `up` (`fire.wesl`'s `hearth`).
pub(super) fn frame(up: Vec3) -> (Vec3, Vec3) {
    let east = (up.cross(Vec3::Z) + Vec3::splat(1.0e-4)).normalize();
    (east, east.cross(up))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where a log's foot is in the world as the shader lays it from the packed logs.
    fn shader_foot(packed: [Vec4; 2], foot: Vec3, i: f32) -> Vec3 {
        let up = foot.normalize_or(Vec3::Y);
        let (east, north) = frame(up);
        let a = packed[0].y + i * std::f32::consts::TAU / packed[0].x;
        foot + (east * libm::cosf(a) + north * libm::sinf(a)) * packed[1].x + up * packed[1].y
    }

    #[test]
    fn a_turned_hearths_logs_are_laid_where_its_model_has_them() {
        let logs = Logs::CAMPFIRE;
        for yaw in [0.0, 0.7, 2.4, -1.9] {
            // On a planet "up" is out from its middle: far up, as near level as a hearth stands.
            let at = Vec3::new(0.3, 1000.0, -0.2);
            let placed = GlobalTransform::from(
                Transform::from_translation(at).with_rotation(Quat::from_rotation_y(yaw)),
            );
            let packed = logs.packed(&placed);
            for i in 0..logs.count {
                // The model's own log foot, turned and moved as the hearth stands.
                let a = logs.turn + i as f32 * std::f32::consts::TAU / logs.count as f32;
                let model = Vec3::new(
                    libm::cosf(a) * logs.foot,
                    logs.foot_up,
                    libm::sinf(a) * logs.foot,
                );
                let world = placed.transform_point(model);
                let drawn = shader_foot(packed, at, i as f32);
                assert!(
                    drawn.distance(world) < 1.0e-3,
                    "yaw {yaw} log {i}: drawn {drawn}, model {world}"
                );
            }
        }
    }

    #[test]
    fn a_hearth_scaled_up_has_its_logs_scaled_with_it() {
        let placed = GlobalTransform::from(Transform::from_scale(Vec3::splat(2.0)));
        let packed = Logs::CAMPFIRE.packed(&placed);
        assert!((packed[1].x - 2.0 * Logs::CAMPFIRE.foot).abs() < 1.0e-5);
        assert!((packed[1].w - 2.0 * Logs::CAMPFIRE.top_up).abs() < 1.0e-5);
    }
}
