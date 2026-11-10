//! The boxes the workbench measures and frames by: how far a model reaches above and under
//! the floor (`measure`), the world box round what a part holds (`view`), and the camera
//! that holds a body in frame through a sheet's moments (`fit`).

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

/// The camera `distance` from `target`, `degrees` round from `front` about `up`, raised
/// by `height`, looking at the target.
pub(super) fn orbit(
    (up, front): (Vec3, Vec3),
    target: Vec3,
    degrees: f32,
    distance: f32,
    height: f32,
) -> Transform {
    let side = up.cross(front);
    let turn = degrees.to_radians();
    let out = (front * libm::cosf(turn) + side * libm::sinf(turn)).normalize();
    Transform::from_translation(target + out * distance + up * height).looking_at(target, up)
}

/// How much of a frame's half width and half height a fitted body may fill (`fit`).
const FILLS: f32 = 0.92;

/// How far outside the frame the worst of `points`, each a ball of `pad` metres, lies
/// from `camera`, as the metres the camera would step back to hold it: nothing or less
/// when all are inside. `tans` are the tangents of the frame's half width and half height.
pub(super) fn outside(camera: &Transform, points: &[Vec3], pad: f32, tans: (f32, f32)) -> f32 {
    let (right, above, ahead) = (camera.right(), camera.up(), camera.forward());
    points
        .iter()
        .map(|point| {
            let from = *point - camera.translation;
            let depth = from.dot(*ahead) - pad;
            let across = (from.dot(*right).abs() + pad) / tans.0;
            let along = (from.dot(*above).abs() + pad) / tans.1;
            f32::max(across, along) - depth
        })
        .fold(f32::NEG_INFINITY, f32::max)
}

/// The camera that sees all of `points` (a body's joints at every moment of a sheet, each
/// with `pad` metres of flesh round it) from `degrees` round the `front`, as close as
/// holds them within `FILLS` of a frame whose half width and half height have the
/// tangents `tans`: a clip that carries its body off its feet is framed by where the body
/// goes, not by where it stood. No nearer than `closest`; `None` without points.
pub(super) fn fit(
    frame: (Vec3, Vec3),
    points: &[Vec3],
    pad: f32,
    degrees: f32,
    tans: (f32, f32),
    (closest, height): (f32, f32),
) -> Option<Transform> {
    let first = *points.first()?;
    let (low, high) = points.iter().fold((first, first), |(low, high), point| {
        (low.min(*point), high.max(*point))
    });
    let target = (low + high) * 0.5;
    let tans = (tans.0 * FILLS, tans.1 * FILLS);
    // The raised camera's aim turns a little as it backs off, so step until it holds.
    let mut distance = closest;
    for _ in 0..8 {
        let camera = orbit(frame, target, degrees, distance, height);
        distance = f32::max(closest, distance + outside(&camera, points, pad, tans));
    }
    Some(orbit(frame, target, degrees, distance, height))
}

/// The eight corners of `bounds`, a box in the space of `at`, in the world.
pub(super) fn corners(bounds: &Aabb, at: &GlobalTransform) -> impl Iterator<Item = Vec3> + use<> {
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

    /// A body whose clip carries it off its feet stays in frame: a 4 m creature that
    /// stands, then lunges 2.3 m forward and down onto its fists, is whole in a sheet's
    /// cell (3/4 as wide as high, 45° up) from every side at both moments, where the
    /// camera stood for its rest pose loses it.
    #[test]
    fn a_body_whose_clip_travels_stays_in_frame() {
        let frame = (Vec3::Y, Vec3::Z);
        let half = libm::tanf(22.5f32.to_radians());
        let tans = (half * 0.75, half);
        // Feet, hips, head and hands: standing, then down and ahead.
        let standing = [
            Vec3::new(-0.6, 0.0, 0.0),
            Vec3::new(0.6, 0.0, 0.0),
            Vec3::new(0.0, 2.2, 0.0),
            Vec3::new(0.0, 4.0, 0.0),
            Vec3::new(-1.2, 1.6, 0.0),
            Vec3::new(1.2, 1.6, 0.0),
        ];
        let landed = [
            Vec3::new(-0.6, 0.0, -0.4),
            Vec3::new(0.6, 0.0, 0.7),
            Vec3::new(0.0, 1.3, 1.0),
            Vec3::new(0.0, 1.5, 2.2),
            Vec3::new(-0.2, 0.3, 2.3),
            Vec3::new(0.2, 0.3, 2.3),
        ];
        let both: Vec<Vec3> = standing.iter().chain(&landed).copied().collect();
        let pad = 0.4;
        for degrees in [0.0, 60.0, 90.0, 120.0, 210.0, 300.0] {
            let camera = fit(frame, &both, pad, degrees, tans, (0.25, 0.7)).unwrap();
            let out = outside(&camera, &both, pad, tans);
            assert!(out <= 0.0, "from {degrees}: {out} m outside");
            // And no farther than it must: a metre nearer loses something.
            let nearer = Transform::from_translation(camera.translation + camera.forward() * 1.0)
                .with_rotation(camera.rotation);
            assert!(outside(&nearer, &both, pad, tans) > 0.0, "from {degrees}");
            // The rest pose's camera: at the standing body's middle, as far as its height asks.
            let rest = orbit(frame, Vec3::Y * 2.0, degrees, 2.0 / half * 1.2, 0.7);
            if degrees == 90.0 {
                let lost = outside(&rest, &landed, pad, tans);
                assert!(
                    lost > 0.0,
                    "the rest pose's camera held the landing: {lost}"
                );
            }
        }
        // A body that stands still is framed by itself alone, from as near as shows it.
        let still = fit(frame, &standing, pad, 90.0, tans, (0.25, 0.7)).unwrap();
        let far = fit(frame, &both, pad, 90.0, tans, (0.25, 0.7)).unwrap();
        assert!(still.translation.length() < far.translation.length());
        assert!(fit(frame, &[], pad, 90.0, tans, (0.25, 0.7)).is_none());
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
