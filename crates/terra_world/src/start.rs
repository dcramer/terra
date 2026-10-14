//! The start: the fixed place on the planet that every position given as metres east and
//! south is measured from, by scripts, the map, the log and what is placed around it. It is
//! the world's, not any system's: chosen once from the planet, as the most level ground near
//! the planet's own start that the example base fits on, so a run with no building in it
//! starts in the same place as one with.

use crate::planet::Planet;
use bevy::prelude::*;
use terra_build::{Base, CELL_M, Kind, example};

/// A base's floor clears the highest ground under it by this much.
pub const FLOOR_CLEARANCE_M: f32 = 0.12;

/// Where the start is: its point on the ground, and its east and south as the frame's
/// x and z. A base is first laid with its corner here.
#[derive(Resource, Clone, Copy, Deref, Debug)]
pub struct Start(pub Transform);

impl Start {
    /// The start of a planet. On the yard, the flat test level, it is the planet's own
    /// start, facing east.
    pub fn choose(planet: &Planet, yard: bool) -> Self {
        let mut footprint = Base::default();
        for op in example(Kind::PLAIN) {
            footprint.apply(op);
        }
        Self(if yard {
            let start = planet.start();
            let up = start.normalize();
            let east = up.cross(Vec3::X).normalize();
            Transform::from_translation(start + up * FLOOR_CLEARANCE_M).looking_to(east, up)
        } else {
            lifted(choose_site(planet, &footprint), &footprint, planet)
        })
    }

    /// The point so many metres east and south of the start, on its level.
    pub fn at(&self, east: f32, south: f32) -> Vec3 {
        self.transform_point(Vec3::new(east, 0.0, south))
    }

    /// A point on the planet as metres east, up and south of the start: the terms `--at`,
    /// `--face` and the log give places in.
    pub fn local(&self, point: Vec3) -> Vec3 {
        self.compute_affine().inverse().transform_point3(point)
    }
}

/// Where to stand a base: the most level ground near the start that is clear of the sea,
/// turned so its door looks out over the water if there is any.
fn choose_site(planet: &Planet, base: &Base) -> Transform {
    let start = planet.start();
    let up = start.normalize();
    let east = up.cross(Vec3::X).normalize();
    let north = east.cross(up);
    let sea = planet.sea_radius_m();

    let ground_floor = || {
        base.room_cells()
            .filter(|(at, _)| at.y == 0)
            .map(|(at, room)| (at.with_y(0), room))
    };
    let low = ground_floor().fold(IVec3::MAX, |low, (tile, _)| low.min(tile));
    let high = ground_floor().fold(IVec3::MIN, |high, (tile, _)| high.max(tile));
    let (low, high) = (
        low.as_vec3() * CELL_M - 3.0,
        (high + 1).as_vec3() * CELL_M + 3.0,
    );
    let middle = (low + high) * 0.5;

    let mut best: Option<(f32, Transform)> = None;
    for ring in 3..22 {
        for spoke in 0..24 {
            let angle = spoke as f32 / 24.0 * std::f32::consts::TAU;
            let offset =
                (east * libm::cosf(angle) + north * libm::sinf(angle)) * (ring as f32 * 6.0);
            let anchor = planet.ground_at(start + offset);
            let anchor_up = anchor.normalize();
            for turn in 0..8 {
                let facing =
                    Quat::from_axis_angle(anchor_up, turn as f32 * std::f32::consts::TAU / 8.0)
                        * east.reject_from(anchor_up).normalize();
                let mut frame = Transform::from_translation(anchor).looking_to(facing, anchor_up);
                // Put the middle of the base, not its corner, on the chosen spot.
                frame.translation -= frame.rotation * Vec3::new(middle.x, 0.0, middle.z);

                // How far the ground under the base rises above and falls below the frame.
                let (mut lowest, mut highest) = (f32::MAX, f32::MIN);
                let mut x = low.x;
                while x <= high.x {
                    let mut z = low.z;
                    while z <= high.z {
                        let height = -planet.distance(frame.transform_point(Vec3::new(x, 0.0, z)));
                        lowest = lowest.min(height);
                        highest = highest.max(height);
                        z += 3.0;
                    }
                    x += 3.0;
                }
                let above_sea = anchor.length() + lowest - sea;
                if above_sea < 1.6 {
                    continue;
                }
                if highest - lowest > 2.5 {
                    continue;
                }
                // Doors are on the base's +Z side; water a short walk out that way is
                // worth a little slope.
                let water_ahead = [40.0, 60.0, 85.0]
                    .iter()
                    .filter(|&&ahead| {
                        let point = frame.transform_point(Vec3::new(middle.x, 0.0, ahead));
                        planet.under_sea(point)
                    })
                    .count();
                // Grass to stand on, at the base and out in front of it.
                let grassy: f32 = [0.0, 12.0, 24.0]
                    .iter()
                    .map(|&ahead| {
                        let point = frame.transform_point(Vec3::new(middle.x, 0.0, ahead));
                        let point = planet.ground_at(point);
                        planet.cover(point, planet.normal(point)).grass
                    })
                    .sum();
                let score = (highest - lowest) - 0.8 * water_ahead as f32 - 0.5 * grassy
                    + 0.04 * (above_sea - 4.0).abs();
                if best.as_ref().is_none_or(|(least, _)| score < *least) {
                    best = Some((score, frame));
                }
            }
        }
    }
    best.map(|(_, frame)| frame).unwrap_or_else(|| {
        let anchor = planet.ground_at(start + east * 60.0);
        Transform::from_translation(anchor).looking_to(north, anchor.normalize())
    })
}

/// Raises a build's frame until its floor clears the highest ground under it: under its
/// rooms, or with none, as a floor laid alone, under its floor. The stand-in planet cannot
/// be dug, so a build has to stay out of the ground's way; a foundation and steps make up
/// the difference.
pub fn lifted(mut frame: Transform, base: &Base, planet: &Planet) -> Transform {
    let mut columns: Vec<IVec3> = base
        .room_cells()
        .filter(|(at, _)| at.y == 0)
        .map(|(at, _)| at)
        .collect();
    if columns.is_empty() {
        let floor = terra_build::floor_layer(0);
        columns = base
            .cubes()
            .filter(|(at, _)| at.y == floor)
            .map(|(at, _)| at.with_y(0))
            .collect();
    }
    if columns.is_empty() {
        return frame;
    }
    let highest_ground = columns
        .into_iter()
        .flat_map(|tile| {
            [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0), (0.5, 0.5)].map(|(dx, dz)| {
                let on_floor = Vec3::new(
                    (tile.x as f32 + dx) * CELL_M,
                    0.0,
                    (tile.z as f32 + dz) * CELL_M,
                );
                -planet.distance(frame.transform_point(on_floor))
            })
        })
        .fold(f32::MIN, f32::max);
    let up = *frame.up();
    frame.translation += up * (highest_ground + FLOOR_CLEARANCE_M);
    frame
}
