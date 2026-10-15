//! Walking in and around a base: what holds a walker up and what stops them.
//!
//! This is deliberately simple. A base is cubes on a grid, so a walker can be handled with
//! arithmetic instead of a physics engine: the tops of cubes hold them up, and solid cubes
//! stop them, while openings, doorways and windows, let them through. Coordinates are the base's own,
//! in metres, with `y` at the feet.

use crate::cubes::{Cube, Cubes, FULL};
use crate::kit::Kits;
use crate::layout::Piece;
use crate::{Base, CELL_M};
use glam::{IVec2, IVec3, Vec3};

/// How far into a cube a piece must reach to stand in it.
const SLIVER_M: f32 = 0.05;

/// How thick a roof's cube is under its top, as a fraction of the cube: the roof's own
/// thickness (`mesh::ROOF_THICK_M`), so its underside is where it is drawn.
const ROOF_SHELL: f32 = crate::mesh::ROOF_THICK_M / CELL_M;

/// How far from the middle of a walker's feet, as a fraction of their radius, what is
/// under them holds them up.
const FOOT_REACH: f32 = 0.6;

/// How wide a post stops a walker, square about its column's middle: as wide as the
/// widest any kit draws one (a stone pillar's 36 cm), not its whole cube, so a walker passes
/// close by a post as they would a real one.
pub const POST_M: f32 = 0.36;

/// The size of something walking through a base.
#[derive(Clone, Copy, Debug)]
pub struct Walker {
    pub radius: f32,
    pub height: f32,
    /// The tallest step that can be walked up without jumping.
    pub step_up: f32,
}

impl Default for Walker {
    fn default() -> Self {
        Walker {
            radius: 0.3,
            height: 1.8,
            step_up: 0.4,
        }
    }
}

/// The column of cubes a point is over.
fn column_at(point: Vec3) -> IVec2 {
    IVec2::new(
        (point.x / CELL_M).floor() as i32,
        (point.z / CELL_M).floor() as i32,
    )
}

impl Base {
    /// The base as far as it stands while it goes up: only the cubes the pieces in so far
    /// stand in, so a plan's ghost is walked through and what is built stops a walker
    /// (docs/building-kits.md); a roof's cube once any piece of it is in.
    pub fn standing<'a>(&self, done: impl IntoIterator<Item = &'a Piece>) -> Base {
        let mut cubes = Cubes::default();
        let mut bounds = Vec::new();
        for piece in done {
            if let Some(cell) = piece.roof_cell() {
                let low = cell.cube.as_vec3() * CELL_M;
                bounds.push((low, low + glam::Vec3::splat(CELL_M)));
            } else if let Some(found) = piece.bounds() {
                bounds.push(found);
            }
        }
        for (min, max) in bounds {
            // The cubes it fills more than a sliver of.
            let low = ((min + SLIVER_M) / CELL_M).floor().as_ivec3();
            let high = ((max - SLIVER_M) / CELL_M).floor().as_ivec3();
            for x in low.x..=high.x {
                for y in low.y..=high.y {
                    for z in low.z..=high.z {
                        let at = IVec3::new(x, y, z);
                        if let Some(cube) = self.cube(at) {
                            cubes
                                .set(at, Some(Cube { part: None, ..cube }))
                                .expect("no parts");
                        }
                    }
                }
            }
        }
        Base::from_cubes(cubes)
    }

    /// The layers of a column of cubes that hold a walker up or stop them, with their
    /// bottoms and tops at a point over it: each cube to its fill, as high as its shape's
    /// top there (a stair's step, a slope's ramp, a half); a roof's cube only its shell, as
    /// thick as a roof under its top, so a walker stands under a roof's slope inside it
    /// (D241). Openings and planned cubes with nothing in them are walked through.
    fn solid_layers(&self, at: Vec3) -> impl Iterator<Item = (f32, f32)> + '_ {
        let column = column_at(at);
        let (u, v) = (
            (at.x / CELL_M - column.x as f32).clamp(0.0, 1.0),
            (at.z / CELL_M - column.y as f32).clamp(0.0, 1.0),
        );
        // Within a post's box, or outside it.
        let off_post = (u - 0.5).abs().max((v - 0.5).abs()) * CELL_M > POST_M * 0.5;
        self.column(column.x, column.y).filter_map(move |layer| {
            let cube = self.cube(IVec3::new(column.x, layer, column.y))?;
            if !cube.is_in() || (cube.shape == crate::Shape::Post && off_post) {
                return None;
            }
            let foot = layer as f32 * CELL_M;
            let fill = f32::from(cube.level()) / f32::from(FULL);
            // From its underside, an upper half's or an inverted slope's (D231), up
            // to its top or as far as it is filled.
            let (mut bottom, top) = (cube.shape.bottom(u, v), fill.min(cube.shape.top(u, v)));
            if cube.kind.is_roof() {
                bottom = bottom.max(cube.shape.top(u, v) - ROOF_SHELL);
            }
            (top > bottom).then_some((foot + CELL_M * bottom, foot + CELL_M * top))
        })
    }

    /// The height of whatever would hold up feet at `point`: the highest top of a cube in
    /// that column that is no more than a step above them. `None` if the base offers
    /// nothing there, in which case the ground decides.
    pub fn support(&self, point: Vec3, walker: &Walker) -> Option<f32> {
        // Under the feet, which are as wide as the body is near enough: a step's edge a
        // little ahead is stood on as the body reaches it, as a capsule climbs a stair.
        let reach = walker.radius * FOOT_REACH;
        let feet = [
            Vec3::ZERO,
            Vec3::X * reach,
            Vec3::NEG_X * reach,
            Vec3::Z * reach,
            Vec3::NEG_Z * reach,
        ];
        let tops = feet
            .into_iter()
            .flat_map(move |offset| self.solid_layers(point + offset).map(|(_, top)| top));
        tops.filter(|&surface| surface <= point.y + walker.step_up)
            .max_by(|a, b| a.total_cmp(b))
    }

    /// The height of the ceiling over a point: the lowest cube's underside over it, a
    /// roof's too.
    pub fn ceiling(&self, point: Vec3) -> Option<f32> {
        let above = point.y + 0.1;
        self.solid_layers(point)
            .map(|(bottom, _)| bottom)
            .filter(|&bottom| bottom >= above)
            .min_by(|a, b| a.total_cmp(b))
    }

    /// Moves a walker along the floor from `from` toward `to`, stopping at walls and
    /// windows and passing through openings. Returns where they end up. Height is taken
    /// from `to` unchanged; use [`Base::support`] for that. With the kits it is laid by, a
    /// door's or shutters' leaves are met where they are drawn, on their wall's own face.
    pub fn walk(&self, from: Vec3, to: Vec3, walker: &Walker, kits: Option<&Kits>) -> Vec3 {
        // A door's or shutters' leaves stop them where they stand, shut or open (D207);
        // the cubes have the last word.
        let to = self.off_leaves(from, to, walker, kits);
        let mut at = from;
        // One axis at a time, so that walking into a wall at an angle slides along it.
        for axis in [0, 2] {
            let mut wanted = to[axis];
            let step = wanted - at[axis];
            if step != 0.0 {
                let direction = step.signum();
                let column = column_at(at);
                let index = if axis == 0 { 0 } else { 1 };
                let edge = (column[index] + i32::from(direction > 0.0)) as f32 * CELL_M;
                let reaches_edge = (wanted + direction * walker.radius - edge) * direction > 0.0;
                // A walker is wider than a point: everything across their width has to
                // be open, so a doorway one narrow cell wide does not let them through.
                let sideways = 2 - axis;
                let stopped = [-walker.radius, 0.0, walker.radius].iter().any(|&shift| {
                    let mut beside = at;
                    beside[sideways] += shift;
                    // Just over the edge into the next column, where its shape's top is
                    // met: a stair's lower step, a slope's foot.
                    beside[axis] = edge + direction * 0.01;
                    self.blocks(beside, at.y, walker)
                });
                if reaches_edge && stopped {
                    wanted = edge - direction * (walker.radius + 0.01);
                    // Never be pushed backward by a wall already too close.
                    if (wanted - at[axis]) * direction < 0.0 {
                        wanted = at[axis];
                    }
                }
                // Within a column, an underside overhead may come lower than at its edge,
                // as an inverted slope's does: the body stops where it would meet it,
                // unless it is caught in it already and moving out.
                let caught = |along: f32| {
                    [-walker.radius, 0.0, walker.radius].iter().any(|&shift| {
                        let mut beside = at;
                        beside[sideways] += shift;
                        beside[axis] = along;
                        self.blocks(beside, at.y, walker)
                    })
                };
                if !caught(at[axis]) {
                    // Swept in steps a tenth of a cube or less, stopped at the last clear.
                    let steps = ((wanted - at[axis]).abs() / (CELL_M * 0.1)).ceil().max(1.0);
                    let mut clear = at[axis];
                    for i in 1..=steps as usize {
                        let along = at[axis] + (wanted - at[axis]) * i as f32 / steps;
                        if caught(along) {
                            wanted = clear;
                            break;
                        }
                        clear = along;
                    }
                }
            }
            let start = at[axis];
            at[axis] = wanted;
            at = self.off_posts(at, axis, start, from.y, walker);
        }
        at.y = to.y;
        at
    }

    /// Where a walker going from one place to another ends up, kept out of the leaves of
    /// every door and shutters near, as they stand (`fitting::push_off`): each leaf where it
    /// is drawn (`fitting::hung`), shutters on their own wall's face, a log wall's crests or
    /// a stone wall's cubes.
    fn off_leaves(&self, from: Vec3, to: Vec3, walker: &Walker, kits: Option<&Kits>) -> Vec3 {
        let reach = walker.radius + 2.5;
        let near = |prop: &&crate::Prop| {
            let low = IVec3::from_array(prop.min).as_vec3() * CELL_M - reach;
            let high = (IVec3::from_array(prop.max) + IVec3::ONE).as_vec3() * CELL_M + reach;
            to.x > low.x && to.x < high.x && to.z > low.z && to.z < high.z
        };
        let leaves = self.props().filter(near).flat_map(|prop| {
            let open = if prop.open { 1.0 } else { 0.0 };
            crate::fitting::hung(self, prop, kits)
                .into_iter()
                .map(move |leaf| (leaf, open))
        });
        let body = (from.y + walker.step_up, from.y + walker.height);
        crate::fitting::push_off(from, to, walker.radius, body, leaves)
    }

    /// A walker's place along an axis, having moved there from `start`, stopped at the
    /// first post whose box their body would have come into on the way, as a post's box
    /// stops them (`POST_M`): touching it, round its corner as far as the other way puts
    /// them beside it.
    fn off_posts(&self, at: Vec3, axis: usize, start: f32, feet: f32, walker: &Walker) -> Vec3 {
        let mut at = at;
        let (low, high) = (feet + walker.step_up, feet + walker.height);
        let other_axis = 2 - axis;
        let half = POST_M * 0.5;
        let (from, to) = (start.min(at[axis]), start.max(at[axis]));
        // The columns the body sweeps past, and those beside them a post's half away.
        let reach = walker.radius + half;
        let mut lo = Vec3::new(at.x, 0.0, at.z) - Vec3::splat(reach);
        let mut hi = Vec3::new(at.x, 0.0, at.z) + Vec3::splat(reach);
        lo[axis] = from - reach;
        hi[axis] = to + reach;
        let (first, last) = (column_at(lo), column_at(hi));
        for x in first.x..=last.x {
            for z in first.y..=last.y {
                let post = self.column(x, z).any(|layer| {
                    self.cube(IVec3::new(x, layer, z)).is_some_and(|cube| {
                        cube.is_in()
                            && cube.shape == crate::Shape::Post
                            && (layer as f32 * CELL_M) < high
                            && ((layer + 1) as f32 * CELL_M) > low
                    })
                });
                if !post {
                    continue;
                }
                let middle = Vec3::new((x as f32 + 0.5) * CELL_M, 0.0, (z as f32 + 0.5) * CELL_M);
                // How far beside the box the walker is the other way, and so how close they
                // may come along this one.
                let beside = ((at[other_axis] - middle[other_axis]).abs() - half).max(0.0);
                if beside >= walker.radius {
                    continue;
                }
                let clear = half + (walker.radius * walker.radius - beside * beside).sqrt();
                let (near, far) = (middle[axis] - clear, middle[axis] + clear);
                // Came into it this step: stopped on the side they came from.
                if to > near && from < far {
                    if start <= middle[axis] {
                        at[axis] = at[axis].min(near - 0.001).max(start.min(near - 0.001));
                    } else {
                        at[axis] = at[axis].max(far + 0.001).min(start.max(far + 0.001));
                    }
                }
            }
        }
        at
    }

    /// Whether the column of cubes at a point has something solid there where a walker's
    /// body would be, above what they can step up.
    fn blocks(&self, at: Vec3, feet: f32, walker: &Walker) -> bool {
        let (low, high) = (feet + walker.step_up, feet + walker.height);
        self.solid_layers(at)
            .any(|(bottom, top)| top > low && bottom < high)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BuildOp, Kind, Panel, STOREY_M, cells};

    /// A room nine metres square outside on the ground, with a doorway a metre wide in its
    /// south wall and a window in its east wall. These tests are written in metres and
    /// assume the grid divides half a metre.
    fn room() -> Base {
        let mut base = Base::default();
        let far = cells(9.0) - 1;
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [far, 0, far],
            kind: Kind::PLAIN,
        });
        for x in cells(4.0)..cells(5.0) {
            base.apply(BuildOp::SetWall {
                tile: [x, 0, far],
                panel: Some(Panel::Door),
                kind: Kind::PLAIN,
            });
        }
        for z in cells(3.0)..cells(6.0) {
            base.apply(BuildOp::SetWall {
                tile: [far, 0, z],
                panel: Some(Panel::WINDOW),
                kind: Kind::PLAIN,
            });
        }
        base
    }

    fn walker() -> Walker {
        Walker::default()
    }

    /// Walks in small steps, as a game would frame by frame.
    fn stroll(base: &Base, from: Vec3, to: Vec3) -> Vec3 {
        let mut at = from;
        for i in 1..=400 {
            let target = from.lerp(to, i as f32 / 400.0);
            at = base.walk(at, Vec3::new(target.x, at.y, target.z), &walker(), None);
        }
        at
    }

    /// A wall of logs four metres long, as the log kit lays it, east and west.
    fn log_wall() -> (Base, Vec<Piece>) {
        let kit = crate::kit::Kit::default();
        let mut base = Base::default();
        for x in 0..8 {
            base.apply(BuildOp::SetWall {
                tile: [x, 0, 4],
                panel: Some(Panel::Solid),
                kind: kit.kind(),
            });
        }
        let pieces = crate::layout::lay(&base, &kit).pieces;
        (base, pieces)
    }

    #[test]
    fn a_planned_wall_is_walked_through_and_a_built_one_stops_you() {
        let (base, pieces) = log_wall();
        let (from, to) = (Vec3::new(2.0, 0.0, 0.5), Vec3::new(2.0, 0.0, 4.0));
        let plan = base.standing([]);
        assert!((stroll(&plan, from, to) - to).length() < 0.05);
        let built = base.standing(&pieces);
        assert!(stroll(&built, from, to).z < 2.0);
        assert_eq!(built.cubes().count(), base.cubes().count());
        // With only its first course in, it stands in the bottom layer of cubes alone.
        let low: Vec<&Piece> = pieces
            .iter()
            .filter(|p| p.bounds().is_some_and(|(_, max)| max.y < 0.45))
            .collect();
        assert!(
            base.standing(low.iter().copied())
                .cubes()
                .all(|(at, _)| at.y == 0)
        );
    }

    #[test]
    fn a_door_lets_you_in_and_out() {
        let base = room();
        let inside = stroll(&base, Vec3::new(4.5, 0.0, 14.0), Vec3::new(4.5, 0.0, 4.5));
        assert!((inside.z - 4.5).abs() < 0.01, "stopped at z {}", inside.z);
        let back_out = stroll(&base, inside, Vec3::new(4.5, 0.0, 14.0));
        assert!((back_out.z - 14.0).abs() < 0.01);
    }

    #[test]
    fn a_doorway_narrower_than_a_walker_keeps_them_out() {
        let mut base = room();
        // One cell of doorway in the north wall.
        base.apply(BuildOp::SetWall {
            tile: [cells(4.5), 0, 0],
            panel: Some(Panel::Door),
            kind: Kind::PLAIN,
        });
        let at = Vec3::new(4.5 + CELL_M * 0.5, 0.0, 4.5);
        let stopped = stroll(&base, at, Vec3::new(at.x, 0.0, -6.0));
        assert!(stopped.z > 0.0, "squeezed through at z {}", stopped.z);
    }

    #[test]
    fn walls_and_windows_stop_you_from_both_sides() {
        let base = room();
        // Walking north at a solid part of the south wall, from outside.
        let outside = stroll(&base, Vec3::new(1.5, 0.0, 14.0), Vec3::new(1.5, 0.0, 4.5));
        assert!(
            (outside.z - (9.0 + 0.31)).abs() < 0.02,
            "stopped at z {}",
            outside.z
        );
        // Walking east at the window, from inside: its hole is open, but the wall under its
        // sill stops a walker.
        let at_window = stroll(&base, Vec3::new(4.5, 0.0, 4.5), Vec3::new(14.0, 0.0, 4.5));
        assert!(
            (at_window.x - (8.5 - 0.31)).abs() < 0.02,
            "stopped at x {}",
            at_window.x
        );
        // Clipping the side of the doorway counts as hitting the wall beside it.
        let beside_door = stroll(&base, Vec3::new(3.9, 0.0, 14.0), Vec3::new(3.9, 0.0, 4.5));
        assert!(beside_door.z > 9.0);
    }

    #[test]
    fn walking_into_a_wall_at_an_angle_slides_along_it() {
        let base = room();
        let slid = stroll(&base, Vec3::new(1.5, 0.0, 4.5), Vec3::new(7.5, 0.0, -6.0));
        assert!(slid.z > 0.5, "stopped by the north wall");
        assert!((slid.x - 7.5).abs() < 0.01, "but carried on east along it");
    }

    #[test]
    fn floors_and_roofs_hold_you_up() {
        let mut base = room();
        let w = walker();
        // Inside: the floor. A little below it: the floor is still within a step. Far
        // below: nothing.
        assert_eq!(base.support(Vec3::new(4.5, 0.0, 4.5), &w), Some(0.0));
        assert_eq!(base.support(Vec3::new(4.5, -0.3, 4.5), &w), Some(0.0));
        assert_eq!(base.support(Vec3::new(4.5, -2.0, 4.5), &w), None);
        // Beside the base: nothing. In the doorway: its threshold.
        assert_eq!(base.support(Vec3::new(-4.5, 0.0, 4.5), &w), None);
        assert_eq!(base.support(Vec3::new(4.25, 0.0, 8.75), &w), Some(0.0));
        // On the roof.
        assert_eq!(base.support(Vec3::new(4.5, 3.2, 4.5), &w), Some(3.0));
        // With an upper storey, its floor rather than the roof below.
        base.apply(BuildOp::AddRoom {
            min: [cells(3.0), 1, cells(3.0)],
            max: [cells(6.0), 1, cells(6.0)],
            kind: Kind::PLAIN,
        });
        assert_eq!(base.support(Vec3::new(4.5, 3.0, 4.5), &w), Some(3.0));
        assert_eq!(base.support(Vec3::new(4.5, 6.5, 4.5), &w), Some(6.0));
    }

    #[test]
    fn there_is_a_ceiling_only_under_a_roof() {
        let base = room();
        assert_eq!(base.ceiling(Vec3::new(4.5, 0.0, 4.5)), Some(2.5));
        assert_eq!(base.ceiling(Vec3::new(4.5, 0.0, 14.0)), None);
        assert_eq!(base.ceiling(Vec3::new(4.5, 3.0, 4.5)), None);
    }

    #[test]
    fn a_roof_of_cubes_is_stood_on_and_its_underside_is_the_ceiling_under_it() {
        use crate::{Cube, Fill};
        // A room four metres by three, under a gable of roof cubes rising a cube a cube in
        // from its north and south sides, its flat roof off.
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [7, 0, 5],
            kind: Kind::PLAIN,
        });
        base.apply(BuildOp::Fill {
            min: [1, 5, 1],
            max: [6, 5, 4],
            cube: None,
        });
        let footprint = (0..8).flat_map(|x| (0..6).map(move |z| [x, z])).collect();
        let plan = crate::roofer::over(&footprint, 6, crate::roofer::Style::Gable, None);
        for (at, shape) in plan.roof {
            base.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: Some(Cube {
                    shape,
                    ..Cube::new(Kind::FIRST_ROOF, Fill::Solid)
                }),
            });
        }
        let w = walker();
        // A metre in from the north side, and at the ridge, a metre and a half in.
        let (side, ridge) = (STOREY_M + 1.0, STOREY_M + 1.5);
        let near = |a: Option<f32>, b: f32| a.is_some_and(|a| (a - b).abs() < 0.02);
        // Stood on as far up the slope as the feet reach, as a ramp is.
        let feet = w.radius * FOOT_REACH;
        let stood = base.support(Vec3::new(2.0, side, 1.0), &w).unwrap();
        assert!(
            stood >= side - 1e-3 && stood <= side + feet + 1e-3,
            "{stood}"
        );
        assert!(near(base.support(Vec3::new(2.0, ridge, 1.5), &w), ridge));
        // Inside, the floor holds you up, and the roof's underside is over you.
        let thick = crate::mesh::ROOF_THICK_M;
        assert_eq!(base.support(Vec3::new(2.0, 0.0, 1.0), &w), Some(0.0));
        assert!(near(
            base.ceiling(Vec3::new(2.0, 0.0, 0.9)),
            STOREY_M + 0.9 - thick
        ));
        assert!(near(
            base.ceiling(Vec3::new(2.0, 0.0, 1.45)),
            ridge - 0.05 - thick
        ));
        // Beyond its walls, nothing.
        assert_eq!(base.ceiling(Vec3::new(6.0, 0.0, 1.0)), None);
        // Walking up it from the eave, a slope a cube a cube, as up a ramp.
        let mut at = Vec3::new(2.0, STOREY_M, 0.05);
        for i in 1..=60 {
            let to = Vec3::new(2.0, at.y, 0.05 + 1.4 * i as f32 / 60.0);
            at = base.walk(at, to, &w, None);
            at.y = base.support(at, &w).unwrap_or(at.y);
        }
        assert!(at.z > 1.4 && (at.y - (STOREY_M + at.z)).abs() < 0.1, "{at}");
    }

    /// A base of one cube of a shape at the origin.
    fn shaped(shape: crate::Shape) -> Base {
        use crate::{Cube, Fill};
        let mut base = Base::default();
        base.apply(BuildOp::Fill {
            min: [0, 0, 0],
            max: [0, 0, 0],
            cube: Some(Cube {
                shape,
                ..Cube::new(Kind::PLAIN, Fill::Solid)
            }),
        });
        base
    }

    #[test]
    fn a_half_cube_is_stepped_onto_and_a_whole_one_or_a_stair_or_slope_from_its_top_is_not() {
        use crate::{Facing, Shape};
        let w = walker();
        // Each walked at from the west; the stair and slope rise toward the west, so their
        // high side faces the walker.
        for (shape, top) in [
            (Shape::Half, CELL_M * 0.5),
            (Shape::Whole, CELL_M),
            (Shape::Post, CELL_M),
            (Shape::Stair(Facing::West), CELL_M),
            (Shape::Slope(Facing::West), CELL_M),
        ] {
            let base = shaped(shape);
            let near = Vec3::new(0.0, 0.3, CELL_M * 0.5);
            assert_eq!(base.support(near, &w), Some(top), "{shape:?}");
            let (from, to) = (Vec3::new(-1.0, 0.0, 0.25), Vec3::new(1.5, 0.0, 0.25));
            let passed = (stroll(&base, from, to) - to).length() < 0.01;
            assert_eq!(passed, top <= w.step_up, "{shape:?}");
        }
    }

    #[test]
    fn a_post_stops_a_walker_by_its_own_width_not_its_cube() {
        let base = shaped(crate::Shape::Post);
        // Straight at it, stopped touching it.
        let at = stroll(&base, Vec3::new(-1.0, 0.0, 0.25), Vec3::new(1.5, 0.0, 0.25));
        let face = CELL_M * 0.5 - POST_M * 0.5;
        assert!((at.x - (face - walker().radius)).abs() < 0.01, "{at}");
        // Past it close by, a body's width from its middle, where its cube would have
        // stopped them.
        let z = CELL_M * 0.5 + POST_M * 0.5 + walker().radius + 0.01;
        let to = Vec3::new(1.5, 0.0, z);
        assert!((stroll(&base, Vec3::new(-1.0, 0.0, z), to) - to).length() < 0.01);
    }

    #[test]
    fn a_stair_holds_a_walker_on_its_two_steps_and_a_slope_on_its_ramp() {
        use crate::{Facing, Shape};
        let w = walker();
        // Rising east: the west half is the lower step, the east half the upper.
        let stair = shaped(Shape::Stair(Facing::East));
        let at = |u: f32, y: f32| Vec3::new(u * CELL_M, y, CELL_M * 0.5);
        assert_eq!(stair.support(at(0.2, 0.0), &w), Some(CELL_M * 0.5));
        assert_eq!(stair.support(at(0.8, 0.25), &w), Some(CELL_M));
        // Walked at from the low side, it is stepped onto.
        let (from, to) = (Vec3::new(-1.0, 0.0, 0.25), Vec3::new(0.1, 0.0, 0.25));
        assert!((stroll(&stair, from, to) - to).length() < 0.01);
        let slope = shaped(Shape::Slope(Facing::South));
        // Under a point, not the width of feet.
        let point = Walker { radius: 0.0, ..w };
        for v in [0.1, 0.5, 0.9] {
            let height = slope
                .support(Vec3::new(0.25, CELL_M, v * CELL_M), &point)
                .unwrap();
            assert!((height - v * CELL_M).abs() < 1e-5, "{v}: {height}");
        }
        // Under a stair's upper step there is nothing to hold a walker, and nothing over.
        assert_eq!(stair.ceiling(at(0.8, -2.0)), Some(0.0));
    }

    #[test]
    fn a_corner_slope_holds_a_walker_on_its_ramps_and_an_upper_half_is_walked_under() {
        use crate::{Facing, Shape};
        let point = Walker {
            radius: 0.0,
            ..walker()
        };
        let at = |u: f32, v: f32, y: f32| Vec3::new(u * CELL_M, y, v * CELL_M);
        // An outer corner rising north and east: low on its south and west, high at its
        // north east corner; an inner one high but for its south west corner.
        let outer = shaped(Shape::OuterCorner(Facing::North));
        let top = |base: &Base, u: f32, v: f32| base.support(at(u, v, CELL_M), &point).unwrap();
        assert!((top(&outer, 0.9, 0.1) - 0.9 * CELL_M).abs() < 1e-4);
        assert!((top(&outer, 0.2, 0.1) - 0.2 * CELL_M).abs() < 1e-4);
        assert!((top(&outer, 0.9, 0.7) - 0.3 * CELL_M).abs() < 1e-4);
        let inner = shaped(Shape::InnerCorner(Facing::North));
        assert!((top(&inner, 0.2, 0.7) - 0.3 * CELL_M).abs() < 1e-4);
        assert!((top(&inner, 0.9, 0.7) - 0.9 * CELL_M).abs() < 1e-4);
        // Under an upper half there is a gap half a cube high; its underside is a ceiling.
        let upper = shaped(Shape::UpperHalf);
        assert_eq!(upper.ceiling(at(0.5, 0.5, 0.0)), Some(CELL_M * 0.5));
        assert_eq!(upper.support(at(0.5, 0.5, 0.0), &point), None);
        assert_eq!(upper.support(at(0.5, 0.5, CELL_M), &point), Some(CELL_M));
        // An inverted slope rising east: its underside rakes from the cube's top on the west
        // down to its foot on the east.
        let under = shaped(Shape::InvertedSlope(Facing::East));
        let ceiling = under.ceiling(at(0.25, 0.5, -1.0)).unwrap();
        assert!((ceiling - 0.75 * CELL_M).abs() < 1e-4, "{ceiling}");
        // A walker passes under an upper half a storey up, and is stopped by one at the
        // height of their body.
        let mut high = Base::default();
        high.apply(BuildOp::Fill {
            min: [0, 4, 0],
            max: [0, 4, 3],
            cube: Some(crate::Cube {
                shape: Shape::UpperHalf,
                ..crate::Cube::new(Kind::PLAIN, crate::Fill::Solid)
            }),
        });
        let (from, to) = (Vec3::new(-1.0, 0.0, 0.75), Vec3::new(1.5, 0.0, 0.75));
        assert!((stroll(&high, from, to) - to).length() < 0.01);
        let low = shaped(Shape::UpperHalf);
        assert!(stroll(&low, from, to).x < 0.0);
    }

    /// Walks a flight in small steps, each frame stood on what holds the feet up, as the
    /// game does.
    fn climb(base: &Base, from: Vec3, to: Vec3) -> Vec3 {
        let w = walker();
        let mut at = from;
        for i in 1..=600 {
            let target = from.lerp(to, i as f32 / 600.0);
            at = base.walk(at, Vec3::new(target.x, at.y, target.z), &w, None);
            at.y = base.support(at, &w).unwrap_or(0.0).max(0.0);
        }
        at
    }

    #[test]
    fn a_flight_of_stairs_or_a_ramp_is_walked_up_a_storey() {
        use crate::{Cube, Facing, Fill, Shape};
        for top in [Shape::Stair(Facing::North), Shape::Slope(Facing::North)] {
            // Six columns rising north, each a cube higher, whole cubes under each top.
            let mut base = Base::default();
            for i in 0..6 {
                let z = -i;
                if i > 0 {
                    base.apply(BuildOp::Fill {
                        min: [0, 0, z],
                        max: [1, i - 1, z],
                        cube: Some(Cube::new(Kind::PLAIN, Fill::Solid)),
                    });
                }
                base.apply(BuildOp::Fill {
                    min: [0, i, z],
                    max: [1, i, z],
                    cube: Some(Cube {
                        shape: top,
                        ..Cube::new(Kind::PLAIN, Fill::Solid)
                    }),
                });
            }
            let end = climb(&base, Vec3::new(0.5, 0.0, 1.5), Vec3::new(0.5, 0.0, -2.45));
            assert!(end.y > STOREY_M - 0.1, "{top:?}: up to {end}");
            assert!(end.z < -2.3, "{top:?}: stopped at {end}");
        }
    }

    #[test]
    fn walking_over_a_base_is_not_blocked_by_its_walls() {
        let base = room();
        // On the roof, walking off the east edge.
        let off_roof = stroll(&base, Vec3::new(4.5, 3.0, 4.5), Vec3::new(14.0, 3.0, 4.5));
        assert!((off_roof.x - 14.0).abs() < 0.01);
    }

    #[test]
    fn an_interior_wall_stops_you_and_its_doorway_lets_you_through() {
        let mut base = room();
        // A wall right across the room, two and a half metres from its west side.
        let wall = |z: i32, panel| BuildOp::SetWall {
            tile: [cells(2.5), 0, z],
            panel: Some(panel),
            kind: Kind::PLAIN,
        };
        let (west, east) = (Vec3::new(1.5, 0.0, 4.5), Vec3::new(4.5, 0.0, 4.5));
        assert!((stroll(&base, west, east).x - east.x).abs() < 0.01);
        for z in 1..cells(9.0) - 1 {
            base.apply(wall(z, Panel::Solid));
        }
        assert!(stroll(&base, west, east).x < 2.5);
        assert!(stroll(&base, east, west).x > 3.0);
        for z in cells(4.0)..cells(5.0) {
            base.apply(wall(z, Panel::Door));
        }
        assert!((stroll(&base, west, east).x - east.x).abs() < 0.01);
    }
}
