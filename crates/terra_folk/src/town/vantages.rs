//! Where to stand to see the town, as the settlement gives its own (D69): its vantages, the
//! way to walk in, a walk for a film and where its square and market are, printed in the
//! terms `--vantage`, `--at`, `--face` and `--route` take, so a scripted run can find it.

use super::start_terms;
use bevy::prelude::*;
use terra_town::town::{ThingKind, Town};
use terra_world::start::Start;

/// Prints the town's vantages, arrival, route and square, `at` putting a place in the town on
/// the ground; returns where to stand to walk in and the point to face, as `--at` and `--face`
/// take them, when the town has a street to walk in by.
pub fn announce(
    town: &Town,
    start: &Start,
    at: &impl Fn(Vec2) -> Vec3,
    middle: Vec3,
) -> Option<(Vec2, Vec2)> {
    let vantage = |name: &str, eye: Vec2, eye_up: f32, look: Vec2, look_up: f32| {
        let point = |p: Vec2, h: f32| {
            let ground = at(p);
            start.local(ground + ground.normalize() * h)
        };
        let (eye, look) = (point(eye, eye_up), point(look, look_up));
        info!(
            "Town vantage: {name}: --vantage {:.2},{:.2},{:.2},{:.2},{:.2},{:.2}",
            eye.x, eye.y, eye.z, look.x, look.y, look.z
        );
    };
    let along = town.square.along;
    let across = town.square.across;
    let (low_a, high_a) = town
        .square
        .outline
        .iter()
        .map(|p| p.dot(along))
        .fold((f32::MAX, f32::MIN), |(l, h), x| (l.min(x), h.max(x)));
    vantage(
        "square",
        along * (low_a + 3.0) + across * 2.0,
        1.7,
        along * high_a * 0.5,
        1.6,
    );
    vantage(
        "market",
        along * high_a * 0.6 - across * 4.0,
        1.7,
        -along * 4.0,
        1.4,
    );
    // A hen yard, and the smithy, from a few metres off.
    for (name, kind, off) in [
        ("yard", ThingKind::Yard, -7.0),
        ("forge", ThingKind::Forge, 6.0),
    ] {
        if let Some(thing) = town.things.iter().find(|t| t.kind == kind) {
            let eye = thing.place(Vec2::new(off * 0.5, off));
            vantage(name, eye, 1.7, thing.at, 0.6);
        }
    }
    vantage(
        "above",
        along * low_a * 1.6 + across * 30.0,
        28.0,
        Vec2::ZERO,
        0.0,
    );
    let street = town.streets.first()?;
    let n = street.points.len();
    // Down the street toward the square, from a little off its middle.
    let (eye, toward) = (street.points[n / 2], street.points[n / 5]);
    let aside = (toward - eye).normalize_or_zero().perp() * 1.2;
    vantage("street", eye + aside, 1.7, toward, 1.8);
    // Where to stand to walk in, as --at and --face take it.
    let (from, toward) = (
        start_terms(start, at(street.points[n / 2])),
        start_terms(start, middle),
    );
    info!(
        "Town arrival: --at {:.1},{:.1} --face {:.1},{:.1}",
        from.x, from.y, toward.x, toward.y
    );
    // A walk for a film: in along the main street from halfway out, across the square
    // past the market, and on up the other main street a way, as `--route` takes it.
    let mut walk: Vec<Vec2> = street.points[..=n / 2].iter().rev().copied().collect();
    walk.push(along * low_a * 0.3 - across * 2.0);
    walk.push(along * high_a * 0.3 + across * 2.0);
    if let Some(other) = town.streets.get(1) {
        walk.extend(other.points.iter().take(other.points.len() / 3));
    }
    let route: Vec<String> = walk
        .iter()
        .step_by(2)
        .map(|&p| {
            let l = start_terms(start, at(p));
            format!("--route {:.1},{:.1}", l.x, l.y)
        })
        .collect();
    info!("Town route: {}", route.join(" "));
    // Where the square and its market are, in the terms a route takes, to plan one by.
    let spot = |p: Vec2| {
        let l = start_terms(start, at(p));
        format!("{:.1},{:.1}", l.x, l.y)
    };
    let marked = |kind: ThingKind| {
        town.things
            .iter()
            .filter(|t| t.kind == kind)
            .map(|t| spot(t.at))
            .collect::<Vec<_>>()
            .join(" ")
    };
    info!(
        "Town square: corners {}; well {}; stalls {}; benches {}",
        town.square
            .outline
            .iter()
            .map(|&p| spot(p))
            .collect::<Vec<_>>()
            .join(" "),
        marked(ThingKind::Well),
        marked(ThingKind::Stall),
        marked(ThingKind::Bench),
    );
    Some((from, toward))
}
