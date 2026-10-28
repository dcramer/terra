//! The boxes the workbench measures and frames by: how far a model reaches above and under
//! the floor (`measure`), and the world box round what a part holds (`view`).

use bevy::{camera::primitives::Aabb, prelude::*};

/// How far above `stand`, along `up`, the lowest and the highest corner of any of `boxes`
/// reach, each box in its entity's space; `None` when there are none.
pub(super) fn standing_span(
    boxes: impl Iterator<Item = (Aabb, GlobalTransform)>,
    stand: Vec3,
    up: Vec3,
) -> Option<(f32, f32)> {
    boxes
        .flat_map(|(bounds, at)| corners(&bounds, &at))
        .map(|corner| (corner - stand).dot(up))
        .fold(None, |was, h| match was {
            Some((low, high)) => Some((f32::min(low, h), f32::max(high, h))),
            None => Some((h, h)),
        })
}

/// The world's least and greatest corner round all of `boxes`, each in its entity's space
/// (a socket's held look); `None` when there are none.
pub(super) fn world_box(
    boxes: impl Iterator<Item = (Aabb, GlobalTransform)>,
) -> Option<(Vec3, Vec3)> {
    boxes
        .flat_map(|(bounds, at)| corners(&bounds, &at))
        .fold(None, |was, point| match was {
            Some((low, high)) => Some((Vec3::min(low, point), Vec3::max(high, point))),
            None => Some((point, point)),
        })
}

/// The eight corners of `bounds`, a box in the space of `at`, in the world.
fn corners(bounds: &Aabb, at: &GlobalTransform) -> impl Iterator<Item = Vec3> + use<> {
    let (middle, half) = (Vec3::from(bounds.center), Vec3::from(bounds.half_extents));
    let at = *at;
    (0..8u32).map(move |corner| {
        let sign = Vec3::new(
            if corner & 1 == 0 { -1.0 } else { 1.0 },
            if corner & 2 == 0 { -1.0 } else { 1.0 },
            if corner & 4 == 0 { -1.0 } else { 1.0 },
        );
        at.transform_point(middle + half * sign)
    })
}

#[cfg(test)]
mod tests {
    use super::super::SUNK_M;
    use super::*;

    /// A body is as tall as the top of its highest box, wherever and however that box
    /// is placed, measured along the ground's up.
    #[test]
    fn a_body_stands_as_tall_as_its_highest_box() {
        let stand = Vec3::new(0.0, 100.0, 0.0);
        let up = Vec3::Y;
        // A 7 m trunk from the ground up, its box in its entity's space.
        let trunk = (
            Aabb::from_min_max(Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 7.0, 1.0)),
            GlobalTransform::from_translation(stand),
        );
        // A crown half a metre across, scaled up four times, its middle 7.5 m up.
        let crown = (
            Aabb::from_min_max(Vec3::splat(-0.25), Vec3::splat(0.25)),
            GlobalTransform::from(
                Transform::from_translation(stand + up * 7.5).with_scale(Vec3::splat(4.0)),
            ),
        );
        let (low, high) = standing_span([trunk, crown].into_iter(), stand, up).unwrap();
        assert!(
            low.abs() < 1e-4 && (high - 8.5).abs() < 1e-4,
            "{low} {high}"
        );
        assert_eq!(standing_span(std::iter::empty(), stand, up), None);
        // A part made about a point inside it, an axe head's eye, reaches under the floor
        // by its beard: 5 cm, past SUNK_M, so the bench stands it up on the floor.
        let head = (
            Aabb::from_min_max(Vec3::new(-0.01, -0.05, -0.15), Vec3::new(0.01, 0.07, 0.04)),
            GlobalTransform::from_translation(stand),
        );
        let (low, high) = standing_span([head].into_iter(), stand, up).unwrap();
        assert!((low + 0.05).abs() < 1e-4 && (high - 0.07).abs() < 1e-4 && low < -SUNK_M);
    }

    /// A held tool is framed by the box round all its parts in the world: an axe hanging
    /// head down from a hand, its haft and its head turned half over, spans both.
    #[test]
    fn a_held_look_is_boxed_round_all_its_parts() {
        let hand = Vec3::new(0.3, 1.0, 0.0);
        let down = Quat::from_rotation_x(std::f32::consts::PI);
        let haft = (
            Aabb::from_min_max(Vec3::new(-0.02, -0.08, -0.02), Vec3::new(0.02, 0.6, 0.02)),
            GlobalTransform::from(Transform::from_translation(hand).with_rotation(down)),
        );
        let head = (
            Aabb::from_min_max(Vec3::new(-0.01, 0.0, -0.12), Vec3::new(0.01, 0.1, 0.03)),
            GlobalTransform::from(
                Transform::from_translation(hand - Vec3::Y * 0.55).with_rotation(down),
            ),
        );
        let (low, high) = world_box([haft, head].into_iter()).unwrap();
        // The head, turned over, hangs 0.55 to 0.65 m below the hand; the haft's butt 8 cm above it.
        assert!(
            (low.y - 0.35).abs() < 1e-4 && (high.y - 1.08).abs() < 1e-4,
            "{low} {high}"
        );
        assert!(
            (low.z + 0.03).abs() < 1e-4 && (high.z - 0.12).abs() < 1e-4,
            "{low} {high}"
        );
        assert_eq!(world_box(std::iter::empty()), None);
    }
}
