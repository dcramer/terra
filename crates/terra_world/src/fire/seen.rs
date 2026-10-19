//! Where the fires are on the picture (docs/fire.md, "The automatic check"; D415): over the
//! remote, `fire.seen` tells the screen box round each kind of fire the camera draws, the
//! kept fires (campfires, hearths), the other blazes (a burning building's), and each breath's
//! stream and fireball, so the fire sheet (`tools/fire/sheet.ts`) crops each fire from its
//! projected bounds rather than from boxes drawn by hand. The bounds are the cards' and the
//! breath hull's reach as `fire.wesl` places them, a little generous: the measurer finds the
//! flame inside them.

use super::{Blaze, Blazes, Jet};
use crate::planet::MainCamera;
use crate::remote::method;
use bevy::prelude::*;
use bevy::remote::BrpResult;
use serde_json::{Value, json};

pub fn plugin(app: &mut App) {
    method(app, "fire.seen", seen);
}

/// A box on the picture, in its pixels: least and most corner.
type Seen = Option<(Vec2, Vec2)>;

/// Points round a blaze's flames and the smoke just over them: as wide as its sheet or its
/// tongues spread (`fire.wesl`, the flame and hearth cards), from its foot to nearly twice as
/// tall as it is.
pub(super) fn blaze_points(blaze: &Blaze) -> Vec<Vec3> {
    let up = blaze.at.normalize_or(Vec3::Y);
    let across = up.any_orthonormal_vector();
    let along = up.cross(across);
    let half = 0.5 * blaze.wide.max((blaze.tall * 0.8).clamp(0.8, 2.8)) + 0.3;
    let mut points = Vec::with_capacity(8);
    for h in [-0.2, blaze.tall * 1.8] {
        for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            points.push(blaze.at + up * h + across * a * half + along * b * half);
        }
    }
    points
}

/// The stream's radius so far along it, as `fire.wesl` `stream_radius` has it.
fn stream_radius(jet: &Jet, z: f32) -> f32 {
    0.22 + jet.spread * z * (0.7 + 0.5 * (z / jet.reach.max(0.01)).clamp(0.0, 1.0))
}

/// Points round a breath's stream, from the jaw to where its fireball takes it over, and
/// round its fireball, as `fire.wesl` `ball_radius` and `ball_middle` place it.
pub(super) fn jet_points(jet: &Jet) -> (Vec<Vec3>, Vec<Vec3>) {
    let way = jet.way.normalize_or(Vec3::Z);
    let across = way.any_orthonormal_vector();
    let other = way.cross(across);
    let ball = 0.6 + 1.4 * stream_radius(jet, jet.reach);
    let ring = |at: Vec3, r: f32, into: &mut Vec<Vec3>| {
        for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            into.push(at + across * a * r + other * b * r);
        }
    };
    let mut stream = Vec::new();
    let end = (jet.reach - 1.5 * ball).max(jet.reach * 0.3);
    for step in 0..=8 {
        let z = end * step as f32 / 8.0;
        ring(jet.from + way * z, stream_radius(jet, z) * 1.2, &mut stream);
    }
    let up = (jet.from + way * jet.reach).normalize_or(Vec3::Y);
    let middle = jet.from + way * (jet.reach - 0.85 * ball) + up * ball * 0.45;
    let r = ball * 1.55;
    let mut fireball = Vec::new();
    for h in [-r, r] {
        ring(middle + way * h, r, &mut fireball);
        ring(middle + up * h, r, &mut fireball);
    }
    (stream, fireball)
}

/// The box on the picture round points, clamped to it; none if every point is behind the
/// eye or the box is off the picture.
fn on_picture(camera: &Camera, eye: &GlobalTransform, size: Vec2, points: &[Vec3]) -> Seen {
    let mut low = Vec2::splat(f32::INFINITY);
    let mut high = Vec2::splat(f32::NEG_INFINITY);
    for &point in points {
        if let Ok(at) = camera.world_to_viewport(eye, point) {
            low = low.min(at);
            high = high.max(at);
        }
    }
    let (low, high) = (low.max(Vec2::ZERO), high.min(size));
    (high.x - low.x >= 2.0 && high.y - low.y >= 2.0).then_some((low, high))
}

/// A box as the measurer takes a crop: `WxH+X+Y`.
fn crop(seen: Seen) -> Value {
    seen.map_or(Value::Null, |(low, high)| {
        let (w, h) = (high - low).round().into();
        let (x, y) = low.floor().into();
        json!(format!("{w}x{h}+{x}+{y}"))
    })
}

/// The union of boxes.
fn union(boxes: impl Iterator<Item = Seen>) -> Seen {
    boxes
        .flatten()
        .reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))
}

fn seen(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    let mut cameras = world.query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
    let Some((camera, eye)) = cameras.iter(world).find(|(camera, _)| camera.is_active) else {
        return Err(crate::remote::refused(
            "there is no camera to see the fires from",
        ));
    };
    let Some(size) = camera.logical_viewport_size() else {
        return Err(crate::remote::refused("the camera has no picture yet"));
    };
    let blazes = world.resource::<Blazes>();
    let boxed = |points: Vec<Vec3>| on_picture(camera, eye, size, &points);
    let kept = union(blazes.kept.iter().map(|b| boxed(blaze_points(b))));
    let blazing = union(blazes.blazes.iter().map(|b| boxed(blaze_points(b))));
    let jets: Vec<(Seen, Seen)> = blazes
        .jets
        .iter()
        .filter(|jet| jet.on)
        .map(|jet| {
            let (stream, ball) = jet_points(jet);
            (boxed(stream), boxed(ball))
        })
        .collect();
    let stream = union(jets.iter().map(|j| j.0));
    let ball = union(jets.iter().map(|j| j.1));
    let picture = format!("{}x{}", size.x.round(), size.y.round());
    let said = |name: &str, seen: Seen| format!("{name} {}", crop(seen).as_str().unwrap_or("none"));
    Ok(json!({
        "what": format!(
            "fire seen: picture {picture}; {}; {}; {}; {}",
            said("kept", kept),
            said("blazes", blazing),
            said("stream", stream),
            said("ball", ball),
        ),
        "picture": picture,
        "kept": crop(kept),
        "blazes": crop(blazing),
        "stream": crop(stream),
        "ball": crop(ball),
        "counts": { "kept": blazes.kept.len(), "blazes": blazes.blazes.len(), "jets": jets.len() },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How far points reach along a direction from a point: least and most.
    fn reach(points: &[Vec3], from: Vec3, way: Vec3) -> (f32, f32) {
        points
            .iter()
            .map(|p| (*p - from).dot(way))
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), d| {
                (a.min(d), b.max(d))
            })
    }

    #[test]
    fn a_blazes_bounds_hold_its_flame_from_its_foot_to_its_tallest_tongue_and_across_its_sheet() {
        let at = Vec3::new(0.0, 6_400_000.0, 0.0);
        let blaze = Blaze {
            at,
            tall: 2.0,
            heat: 1.0,
            seed: 0.0,
            wide: 4.0,
        };
        let points = blaze_points(&blaze);
        let (low, high) = reach(&points, at, Vec3::Y);
        // A tongue reaches 1.1 of its fire's height at the top of its breath (`fire.wesl`).
        assert!(low <= 0.0 && high >= 2.0 * 1.1 * 1.2, "{low} {high}");
        let across = Vec3::Y.any_orthonormal_vector();
        let (left, right) = reach(&points, at, across);
        assert!(left <= -2.0 && right >= 2.0, "{left} {right}");
    }

    #[test]
    fn a_breaths_fireball_bounds_hold_where_it_strikes_and_its_stream_starts_at_the_jaw() {
        let from = Vec3::new(0.0, 6_400_020.0, 0.0);
        let jet = Jet {
            from,
            way: Vec3::X,
            reach: 30.0,
            spread: 0.1,
            on: true,
            face: 0.0,
            normal: Vec3::ZERO,
        };
        let (stream, ball) = jet_points(&jet);
        let (near, far) = reach(&stream, from, Vec3::X);
        assert!(near <= 0.0 && far < 30.0, "{near} {far}");
        let (back, front) = reach(&ball, from, Vec3::X);
        assert!(back < 30.0 && front >= 30.0, "{back} {front}");
    }
}
