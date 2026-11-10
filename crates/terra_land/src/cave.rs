//! Where a world's den is and what shape it takes (D591, D643, docs/caves.md): the troll's
//! cave, a system placed from the seed as the gate is (`gate`), made to fit a 4 m body and a
//! fight and to be reached by a walk from the start on every seed, as Valheim places its
//! troll caves when a world is made.
//!
//! Of the land round the start, on the survey's grid, a cell may hold the den's mouth if a
//! walk from the start reaches it, it is dry and well over the sea, no steeper than a walk
//! takes, neither too near the start nor too far, and clear of the gate. The seed orders
//! those cells; the first where the den's shape has rock enough over it and its floor is
//! over the sea is the world's. The shape is made of parts by the seed ([`Layout`]): a
//! passage going down from the mouth, into the hill where there is one, a second leg, a
//! chamber, alcoves off it. [`faults`] holds a den to the same rules read afresh, for the
//! sweep.

use crate::Land;
use crate::dry::Dry;
use crate::survey::{Biome, CELL_M, Grid, PLOT_OVER_SEA_M};
use glam::Vec3;
use terra_voxel::caves::{Part, System};

/// The nearest a den's mouth is to the start, in metres; how far it is looked for first;
/// and, failing that, the farthest. First values: how far the boss lives from where a
/// player begins is the developers' to tune.
pub const NEAR_M: f32 = 150.0;
pub const FAR_M: f32 = 500.0;
pub const FARTHEST_M: f32 = 900.0;
/// A mouth stands no nearer the gate than this.
pub const FROM_GATE_M: f32 = 80.0;
/// Steeper than this, in degrees, ground holds no mouth: nobody walks up to it.
pub const MOUTH_STEEPEST_DEG: f32 = 30.0;
/// From this slope a passage goes into the hill; on leveller ground the seed says which way.
const HILLSIDE_DEG: f32 = 6.0;
/// The rock over the chamber and over the passage past its mouth is at least this thick.
pub const COVER_M: f32 = 3.0;
/// How far from the mouth the passage's roof may still be thin or open: on level ground a
/// way down at 17 degrees is a trench for its first ten metres and has its 3 m of rock
/// over it by twenty.
const MOUTH_M: f32 = 22.0;
/// A passage's radius: a 4 m body passes (caves.md, "5 m and more" across).
pub const PASSAGE_M: f32 = 3.0;
/// The floor stays this far over the sea.
const OVER_SEA_M: f32 = 1.0;
/// The first leg: how long, and how steeply it goes down, in degrees: a walk takes it.
const FIRST_M: f32 = 26.0;
const DOWN_DEG: f32 = 17.0;
const SECOND_M: f32 = 16.0;

/// How a den is laid out: one of 27 (D182), told apart by the way in and the room.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Layout {
    /// The way in: 0 straight on, 1 one bend, 2 a dog-leg that keeps going down.
    pub way: u8,
    /// The room: 0 low and wide, 1 the middling one, 2 tall.
    pub room: u8,
    /// Alcoves off the room: none, one or two.
    pub alcoves: u8,
}

impl Layout {
    /// The layout a number picks.
    pub fn of(pick: u32) -> Self {
        Self {
            way: (pick % 3) as u8,
            room: (pick / 3 % 3) as u8,
            alcoves: (pick / 9 % 3) as u8,
        }
    }

    /// The room's size: across, high, and its floor below its middle, in metres.
    fn room_m(self) -> (f32, f32, f32) {
        match self.room {
            0 => (30.0, 11.0, 3.6),
            1 => (26.0, 13.0, 4.3),
            _ => (24.0, 17.0, 5.6),
        }
    }
}

/// A world's den: where its mouth opens on the ground, the way its passage goes in, the
/// middle of its chamber's floor, and the parts it is carved as.
#[derive(Clone, Debug, PartialEq)]
pub struct Den {
    pub mouth: Vec3,
    pub heading: Vec3,
    pub floor: Vec3,
    pub layout: Layout,
    pub parts: Vec<Part>,
}

impl Den {
    /// The den as a system to carve, its walls' roughness the world's.
    pub fn system(&self, land: &Land) -> System {
        System::new(self.parts.clone(), land.ground.spec().seed.derive("den"))
    }
}

/// The point on the ground along a direction from the centre.
fn on_ground(land: &Land, direction: Vec3) -> Vec3 {
    let direction = direction.normalize();
    direction * (land.radius_m() + land.ground.height_m(direction))
}

/// How much rock lies over a point with so much open space above it, in metres.
fn cover_m(land: &Land, point: Vec3, open_above_m: f32) -> f32 {
    on_ground(land, point).length() - point.length() - open_above_m
}

/// A den of a layout whose mouth is at a place on the ground, its passage going a way
/// (level, any length), turned to a side by the seed's sign.
fn shape(mouth: Vec3, heading: Vec3, layout: Layout, turn: f32) -> Den {
    let up = mouth.normalize();
    let ahead = (heading - up * heading.dot(up)).normalize();
    let side = up.cross(ahead);
    // A place so far ahead of the mouth, so far down from it, and so far to the side.
    let at =
        |forward: f32, down: f32, across: f32| mouth + ahead * forward + side * across - up * down;
    let down = libm::tanf(DOWN_DEG.to_radians());
    let first_down = FIRST_M * down;
    let (bend_deg, second_down): (f32, f32) = match layout.way {
        0 => (0.0, SECOND_M * libm::tanf(8f32.to_radians())),
        1 => (35.0, SECOND_M * libm::tanf(8f32.to_radians())),
        _ => (50.0, SECOND_M * down),
    };
    let bend = (bend_deg * turn).to_radians();
    let (forward, across) = (libm::cosf(bend), libm::sinf(bend));
    let turned = ahead * forward + side * across;
    let corner = at(FIRST_M, first_down, 0.0);
    let end = at(
        FIRST_M + SECOND_M * forward,
        first_down + second_down,
        SECOND_M * across,
    );
    let (room_across, room_high, room_floor) = layout.room_m();
    // The passage comes in at the room's floor.
    let floor = end + turned * (room_across * 0.5 - 3.0) - up * PASSAGE_M;
    let mut parts = vec![
        // From outside and over the ground, so the mouth is cut open.
        Part::Passage {
            from: at(-3.0, -1.2, 0.0),
            to: corner,
            radius_m: PASSAGE_M,
        },
        Part::Passage {
            from: corner,
            to: end,
            radius_m: PASSAGE_M,
        },
        Part::Chamber {
            middle: floor + up * room_floor,
            across_m: room_across,
            high_m: room_high,
            floor_m: room_floor,
        },
    ];
    // Alcoves off the room's far side, either hand, their floors the room's.
    let far = turned.cross(up);
    for hand in [1.0, -1.0].into_iter().take(layout.alcoves as usize) {
        parts.push(Part::Chamber {
            middle: floor
                + (turned * 0.4 + far * hand).normalize() * (room_across * 0.5 + 1.0)
                + up * 2.6,
            across_m: 9.0,
            high_m: 6.5,
            floor_m: 2.6,
        });
    }
    Den {
        mouth,
        heading: ahead,
        floor,
        layout,
        parts,
    }
}

/// What is wrong with a den's shape under this land, if anything: thin rock over it, or a
/// floor at the sea.
fn wrong_shape(land: &Land, den: &Den) -> Option<String> {
    let mut thinnest = (f32::MAX, "its chamber");
    for part in &den.parts {
        match *part {
            Part::Passage { from, to, radius_m } => {
                let length = from.distance(to);
                for step in 0..=8 {
                    let point = from.lerp(to, step as f32 / 8.0);
                    if point.distance(den.mouth) < MOUTH_M || length < 1.0 {
                        continue;
                    }
                    let cover = cover_m(land, point, radius_m);
                    if cover < thinnest.0 {
                        thinnest = (cover, "its passage");
                    }
                }
            }
            Part::Chamber {
                middle,
                across_m,
                high_m,
                ..
            } => {
                let up = middle.normalize();
                let (east, north) = (
                    up.any_orthonormal_vector(),
                    up.cross(up.any_orthonormal_vector()),
                );
                // Its middle, and a ring most of the way out, where its roof is lower.
                let ring = (0..8).map(|at| {
                    let turn = at as f32 * std::f32::consts::FRAC_PI_4;
                    (east * libm::cosf(turn) + north * libm::sinf(turn)) * across_m * 0.4
                });
                for out in std::iter::once(Vec3::ZERO).chain(ring) {
                    let share = out.length() / (across_m * 0.5);
                    let roof = high_m * 0.5 * libm::sqrtf(1.0 - share * share);
                    let cover = cover_m(land, middle + out, roof);
                    if cover < thinnest.0 {
                        thinnest = (cover, "its chamber");
                    }
                }
            }
        }
    }
    let over_sea = den.floor.length() - land.sea_radius_m();
    if thinnest.0 < COVER_M {
        Some(format!(
            "{} has {:.1} m of rock over it, under {COVER_M} m",
            thinnest.1, thinnest.0
        ))
    } else if over_sea < OVER_SEA_M {
        Some(format!(
            "its floor is {over_sea:.1} m over the sea, under {OVER_SEA_M} m"
        ))
    } else {
        None
    }
}

/// No water stands within so many metres of a den's mouth, as the world draws it (`dry`):
/// the open length of its passage is dry and nothing floods it.
pub const DRY_M: f32 = 25.0;

/// The grid round a start that a den is looked for on, its lakes the world's (`dry`), and
/// the start's cell in it.
fn grid(land: &Land, start: Vec3, dry: &Dry) -> (Grid, (usize, usize)) {
    let mut grid = Grid::round(land, start, FARTHEST_M + 2.0 * CELL_M);
    dry.drown(&mut grid);
    let middle = grid.region.cells / 2;
    let cell = grid.region.cell_of(start).unwrap_or((middle, middle));
    (grid, cell)
}

/// What is wrong with a den's mouth at a cell, if anything: the first rule it breaks.
fn wrong_mouth(
    land: &Land,
    grid: &Grid,
    reached: &[bool],
    gate: Option<Vec3>,
    (column, row): (usize, usize),
) -> Option<String> {
    let region = &grid.region;
    let k = row * region.cells + column;
    let away = region.offset(column, row).length();
    let over_sea = region.heights[k] - (land.sea_radius_m() - land.radius_m());
    let slope = region.slope_deg(column, row);
    let from_gate = gate.map_or(f32::MAX, |gate| {
        region.direction(column, row).angle_between(gate) * land.radius_m()
    });
    if !(NEAR_M..=FARTHEST_M).contains(&away) {
        Some(format!(
            "its mouth is {away:.0} m from the start, not {NEAR_M:.0} to {FARTHEST_M:.0} m"
        ))
    } else if grid.biomes[k] == Biome::Water || over_sea < PLOT_OVER_SEA_M {
        Some(format!(
            "its mouth is in water or {over_sea:.1} m over the sea, under {PLOT_OVER_SEA_M} m"
        ))
    } else if slope > MOUTH_STEEPEST_DEG {
        Some(format!(
            "its mouth's ground slopes {slope:.0} degrees, over {MOUTH_STEEPEST_DEG:.0}"
        ))
    } else if from_gate < FROM_GATE_M {
        Some(format!(
            "its mouth is {from_gate:.0} m from the gate, under {FROM_GATE_M:.0} m"
        ))
    } else if !reached[k] {
        Some("no walk from the start reaches its mouth".to_string())
    } else {
        None
    }
}

/// The den at a cell, by the seed: into the hill where the ground slopes, else the way the
/// seed says; the first of that way and the two beside it whose shape the land holds.
fn den_at(land: &Land, grid: &Grid, seed: u32, (column, row): (usize, usize)) -> Option<Den> {
    let region = &grid.region;
    let (c, r) = (column as u32, row as u32);
    let pick = terra_gen::hash::mix3(seed, c, r, 1);
    let rise = region.gradient(column, row);
    let uphill = region.east * rise.x + region.north * rise.y;
    let seeded = {
        let turn = terra_gen::hash::unit3(seed, c, r, 2) * std::f32::consts::TAU;
        region.east * libm::cosf(turn) + region.north * libm::sinf(turn)
    };
    let heading = if region.slope_deg(column, row) >= HILLSIDE_DEG {
        uphill
    } else {
        seeded
    };
    let mouth = on_ground(land, region.direction(column, row));
    let up = mouth.normalize();
    let turn = if pick & 1 == 0 { 1.0 } else { -1.0 };
    [heading, heading.cross(up), up.cross(heading)]
        .into_iter()
        .map(|heading| shape(mouth, heading, Layout::of(pick >> 1), turn))
        .find(|den| wrong_shape(land, den).is_none())
}

/// A world's den, for a start; none if no ground round the start holds one, which the sweep
/// counts a fault of the world.
pub fn find(land: &Land, start: Vec3) -> Option<Den> {
    let dry = Dry::round(land, start);
    let (grid, cell) = grid(land, start, &dry);
    let n = grid.region.cells;
    let reached = grid.reached(cell, n);
    let gate = crate::gate::find(land, start);
    let seed = land.ground.spec().seed.derive("den").0 as u32;
    let mut may: Vec<(usize, usize)> = (0..n * n)
        .map(|k| (k % n, k / n))
        .filter(|&at| wrong_mouth(land, &grid, &reached, gate, at).is_none())
        .collect();
    // The seed's order among the cells that may hold the mouth.
    may.sort_by_key(|&(column, row)| terra_gen::hash::mix3(seed, column as u32, row as u32, 0));
    [FAR_M, FARTHEST_M].into_iter().find_map(|far| {
        may.iter()
            .filter(|&&(column, row)| grid.region.offset(column, row).length() <= far)
            // The first whose shape the land holds, with no pond at its mouth either.
            .filter_map(|&at| den_at(land, &grid, seed, at))
            .find(|den| !dry.wet(den.mouth.normalize(), DRY_M))
    })
}

/// What is wrong with a den, for a start: nothing if a walker reaches its mouth and the land
/// holds its shape. Read afresh from the land, not from what [`find`] kept.
pub fn faults(land: &Land, start: Vec3, den: &Den) -> Vec<String> {
    let dry = Dry::round(land, start);
    let (grid, cell) = grid(land, start, &dry);
    let Some(at) = grid.region.cell_of(den.mouth.normalize()) else {
        return vec![format!(
            "the den's mouth is over {FARTHEST_M:.0} m from the start, off the land searched"
        )];
    };
    let reached = grid.reached(cell, grid.region.cells);
    let gate = crate::gate::find(land, start);
    wrong_mouth(land, &grid, &reached, gate, at)
        .or_else(|| wrong_shape(land, den))
        .or_else(|| {
            dry.wet(den.mouth.normalize(), DRY_M).then(|| {
                format!(
                    "water stands within {DRY_M:.0} m of its mouth, as the world draws its lakes"
                )
            })
        })
        .map(|why| format!("the den: {why}"))
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests;
