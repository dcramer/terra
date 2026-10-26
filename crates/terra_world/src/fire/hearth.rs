//! A hearth's logs as its flames are laid on them (`fire.wesl`'s `hearth` and `hearth_tongue`):
//! a campfire's flame rises off the burning wood, along each log's length and tallest where
//! the logs meet, so its body follows its own logs rather than standing as a cluster in the
//! middle. Shipped games put the flame on the fuel this way: Unreal's Niagara spawns particles
//! on a mesh's surface (its Static Mesh data interface, "Static Mesh Location"), and a
//! flipbook campfire lays a card every log's width or so along each log's top (RealTimeVFX's
//! campfire breakdowns; research/2026-10-23-campfire-one-body.md).

use super::{Blaze, Blazes, HEARTHS, Kindled};
use crate::planet::MainCamera;
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
    /// The campfire's (`tools/props/campfire.ts`, `layout(1)`, its means; its test holds them):
    /// five round logs 0.08-0.13 m through (0.104 m mean) leaned into a stack inside the stones,
    /// their burning lengths from 0.34 m out (`BURN_R`, near their butts) to 0.08 m, 0.36 m over
    /// its base.
    pub const CAMPFIRE: Logs = Logs {
        count: 5,
        turn: 0.393,
        twist: 0.402,
        radius: 0.052,
        foot: 0.340,
        foot_up: 0.066,
        top: 0.078,
        top_up: 0.361,
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

/// How a kept fire's flame body is drawn, chosen by its kind (D466, docs/fire-module.md): the
/// baked flipbook cards (its columns and tongues on the logs), or one marched flame field
/// (`fire.wesl`'s `hearth_march`), which beat the cards for the campfire in a blind panel
/// (fire round g4). Kinds move to the field one at a time; a torch (`TORCH_M`) keeps its card.
/// Or the same field reading its shape and motion from a baked looping simulation in place of
/// its lobes and noise (`Baked`, D493, `sim`); no kind chooses it until the developers do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Body {
    #[default]
    Cards,
    Marched,
    Baked,
}

impl Body {
    /// What `fire.wesl`'s `hearth_marched` and `hearth_baked` read: 1 for the field, 2 for the
    /// baked loop.
    pub fn told(self) -> f32 {
        match self {
            Body::Cards => 0.0,
            Body::Marched => 1.0,
            Body::Baked => 2.0,
        }
    }
}

/// The fires kept where something kindled stands, each with a number of its own from its
/// place, so no two flicker alike, the logs each burns on and how its body is drawn. The
/// nearest `HEARTHS` to the eye come first and are drawn as hearths; each keeps its slot while it stays among them, so
/// a slot's fire does not change as the eye moves (a dungeon has more torches than slots).
pub(super) fn keep(
    kindled: Query<(Entity, &Kindled, &GlobalTransform)>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut slots: Local<Vec<Entity>>,
    mut blazes: ResMut<Blazes>,
) {
    if kindled.is_empty() && blazes.kept.is_empty() {
        return;
    }
    let eye = camera
        .iter()
        .next()
        .map_or(Vec3::ZERO, |eye| eye.translation());
    let mut fires: Vec<(Entity, &Kindled, &GlobalTransform)> = kindled.iter().collect();
    fires.sort_by(|a, b| {
        let (da, db) = (a.2.translation(), b.2.translation());
        da.distance_squared(eye)
            .total_cmp(&db.distance_squared(eye))
    });
    let near: Vec<Entity> = fires.iter().take(HEARTHS).map(|fire| fire.0).collect();
    *slots = slotted(&slots, &near);
    // The slotted fires in their slots, then the rest, nearest first.
    let order = |entity: Entity| {
        slots
            .iter()
            .position(|&e| e == entity)
            .unwrap_or(usize::MAX)
    };
    fires.sort_by_key(|fire| order(fire.0));
    let hearths: Vec<[Vec4; 2]> = fires
        .iter()
        .map(|(_, fire, placed)| fire.logs.packed(placed))
        .collect();
    let bodies: Vec<f32> = fires.iter().map(|(_, fire, _)| fire.body.told()).collect();
    let lights: Vec<Color> = fires.iter().map(|(_, fire, _)| fire.light).collect();
    let kept: Vec<Blaze> = fires
        .iter()
        .map(|(_, fire, at)| {
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
    if kept != blazes.kept
        || hearths != blazes.hearths
        || bodies != blazes.bodies
        || lights != blazes.lights
    {
        blazes.kept = kept;
        blazes.lights = lights;
        blazes.hearths = hearths;
        blazes.bodies = bodies;
    }
}

/// The slots after `near` became the fires to draw as hearths: each that stays keeps its slot,
/// one that comes takes a freed slot, and a slot left free is filled from the last, so the
/// slots stay one after another.
fn slotted(slots: &[Entity], near: &[Entity]) -> Vec<Entity> {
    let mut kept: Vec<Option<Entity>> = slots
        .iter()
        .map(|&e| near.contains(&e).then_some(e))
        .collect();
    for &come in near.iter().filter(|e| !slots.contains(e)) {
        match kept.iter_mut().find(|slot| slot.is_none()) {
            Some(slot) => *slot = Some(come),
            None => kept.push(Some(come)),
        }
    }
    while let Some(hole) = kept.iter().position(Option::is_none) {
        kept.swap_remove(hole);
    }
    kept.into_iter().flatten().collect()
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
    fn a_fire_keeps_its_slot_while_it_stays_among_the_nearest() {
        let e = |i: u32| Entity::from_raw_u32(i).expect("a valid index");
        let slots = vec![e(1), e(2), e(3)];
        // 2 goes and 4 comes: 4 takes 2's slot, 1 and 3 keep theirs.
        assert_eq!(slotted(&slots, &[e(3), e(4), e(1)]), vec![e(1), e(4), e(3)]);
        // 1 goes and nothing comes: the last fills its slot, the slots stay together.
        assert_eq!(slotted(&slots, &[e(2), e(3)]), vec![e(3), e(2)]);
    }

    #[test]
    fn a_hearth_scaled_up_has_its_logs_scaled_with_it() {
        let placed = GlobalTransform::from(Transform::from_scale(Vec3::splat(2.0)));
        let packed = Logs::CAMPFIRE.packed(&placed);
        assert!((packed[1].x - 2.0 * Logs::CAMPFIRE.foot).abs() < 1.0e-5);
        assert!((packed[1].w - 2.0 * Logs::CAMPFIRE.top_up).abs() < 1.0e-5);
    }
}
