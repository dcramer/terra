//! Bodies: creatures, people and the player take up room, so they cannot stand in each
//! other or in a tree. Each is a circle on the ground; where two overlap, both are pushed
//! apart, the lighter further. Trees and rocks do not move.
//!
//! Players are not pushed here: their own walk stops them at others' bodies (see
//! `push_out`), so the camera never sees one shoved after it has moved. Others are pushed
//! off them instead.
//!
//! A stand-in until creatures walk on voxels with a real character controller (D53).

use super::Player;
use super::flora::Obstacles;
use crate::planet::Planet;
use crate::spatial::SpatialHash;
use bevy::{prelude::*, transform::TransformSystems};
use terra_build::Walker;

/// How much room something takes up on the ground, and how hard it is to shove.
#[derive(Component, Clone, Copy)]
pub struct Body {
    pub radius: f32,
    /// Heavier bodies give way less. Zero never gives way.
    pub weight: f32,
}

impl Body {
    pub fn new(radius: f32, weight: f32) -> Self {
        Self { radius, weight }
    }

    /// One that holds its place, such as someone bound.
    pub fn fixed(radius: f32) -> Self {
        Self {
            radius,
            weight: 0.0,
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, keep_apart.before(TransformSystems::Propagate));
}

/// How far each of two overlapping bodies moves, by weight: the heavier moves less, a fixed
/// one not at all.
fn shares(a: Body, b: Body) -> (f32, f32) {
    match (a.weight == 0.0, b.weight == 0.0) {
        (true, true) => (0.0, 0.0),
        (true, false) => (0.0, 1.0),
        (false, true) => (1.0, 0.0),
        (false, false) => {
            let total = a.weight + b.weight;
            (b.weight / total, a.weight / total)
        }
    }
}

/// Pushes a point at `at` with a body of `radius` out of every body in `others`, along
/// the ground. For a walker that should stop at bodies rather than be shoved by them.
pub fn push_out<'a>(
    mut at: Vec3,
    radius: f32,
    others: impl Iterator<Item = (&'a Body, &'a Transform)>,
) -> Vec3 {
    let up = at.normalize();
    for (body, transform) in others {
        let apart = (at - transform.translation).reject_from(up);
        let reach = radius + body.radius;
        if apart.length_squared() < reach * reach && apart.length_squared() > 1e-8 {
            at += apart.normalize() * (reach - apart.length());
        }
    }
    at
}

/// Moves overlapping bodies apart, and out of trees, rocks and players, then puts them
/// back on the ground. Each body is compared only with those filed near it, so the cost
/// grows with how crowded a place is, not with how many bodies there are (D127).
fn keep_apart(
    planet: Res<Planet>,
    obstacles: Res<Obstacles>,
    players: Query<&Transform, With<Player>>,
    mut bodies: Query<(&Body, &mut Transform), Without<Player>>,
    mut filed: Local<SpatialHash<usize>>,
    mut standing: Local<SpatialHash<()>>,
) {
    let mut all: Vec<_> = bodies.iter_mut().collect();
    let widest = all.iter().map(|(body, _)| body.radius).fold(0.0, f32::max);
    filed.clear();
    filed.extend(
        all.iter()
            .enumerate()
            .map(|(index, (_, transform))| (transform.translation, index)),
    );
    let mut moved = vec![Vec3::ZERO; all.len()];
    for i in 0..all.len() {
        let (a, here) = (*all[i].0, all[i].1.translation);
        let up = here.normalize();
        for &(there, j) in filed.near(here, a.radius + widest) {
            if j <= i {
                continue;
            }
            let b = *all[j].0;
            let apart = (here - there).reject_from(up);
            let reach = a.radius + b.radius;
            if apart.length_squared() >= reach * reach {
                continue;
            }
            // Two in the same spot part along any line.
            let away = apart.normalize_or(up.any_orthonormal_vector());
            let overlap = reach - apart.length();
            let (share_a, share_b) = shares(a, b);
            moved[i] += away * overlap * share_a;
            moved[j] -= away * overlap * share_b;
        }
    }
    let walker = Walker::default().radius;
    standing.clear();
    standing.extend(players.iter().map(|player| (player.translation, ())));
    for (index, (body, transform)) in all.iter_mut().enumerate() {
        if body.weight == 0.0 {
            continue;
        }
        let start = transform.translation + moved[index];
        let mut at = start;
        let up = start.normalize();
        // Off players, who do not give way here, and out of trees and rocks.
        let reach = body.radius + walker;
        let solid = standing
            .near(start, reach)
            .map(|&(place, ())| (place, walker))
            .chain(
                obstacles
                    .near(start, body.radius)
                    .filter(|(place, _)| (start - *place).dot(up).abs() < 3.0),
            );
        for (place, radius) in solid {
            let apart = (at - place).reject_from(up);
            let reach = radius + body.radius;
            if apart.length_squared() < reach * reach {
                at += apart.normalize_or(up.any_orthonormal_vector()) * (reach - apart.length());
            }
        }
        if at != transform.translation {
            transform.translation = planet.ground_at(at);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lighter_body_gives_way_more() {
        let (deer, spider) = (Body::new(0.5, 3.0), Body::new(0.5, 1.0));
        let (deer_moves, spider_moves) = shares(deer, spider);
        assert!(spider_moves > deer_moves);
        assert!((deer_moves + spider_moves - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_fixed_body_never_moves() {
        let (bound, spider) = (Body::fixed(0.4), Body::new(0.5, 1.0));
        assert_eq!(shares(bound, spider), (0.0, 1.0));
        assert_eq!(shares(bound, bound), (0.0, 0.0));
    }

    #[test]
    fn a_walker_stops_at_a_body() {
        let up = Vec3::Y * 100.0;
        let spider = (Body::new(0.6, 1.0), Transform::from_translation(up));
        let walked = push_out(
            up + Vec3::X * 0.3,
            0.4,
            std::iter::once((&spider.0, &spider.1)),
        );
        assert!(((walked - up).reject_from(Vec3::Y).length() - 1.0).abs() < 1e-4);
    }
}
