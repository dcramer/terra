//! Turning a base's shape into triangles.
//!
//! The shape is tiles; what makes it read as a building is the trim, and the trim is
//! chosen by what each piece is next to. A wall gets a post where it turns a corner and a
//! seam where it carries on; a parapet where the roof begins and a plain line where
//! another storey starts. Openings get frames, and windows a sill.
//!
//! A room is a plain box. Lamps, roof fittings and anything else that is a thing in its
//! own right is for a player to build, not part of the room.

use crate::{Base, CELL_M, DOOR_HEIGHT_M, Face, Panel, STOREY_M, Side};
use glam::{IVec3, Vec3};

// Colours are as they should appear on screen (sRGB).
const HULL: [f32; 3] = [0.93, 0.9, 0.82];
const POST: [f32; 3] = [0.84, 0.81, 0.74];
const BAND: [f32; 3] = [0.9, 0.46, 0.17];
const DARK: [f32; 3] = [0.17, 0.18, 0.23];
const PARAPET: [f32; 3] = [0.33, 0.34, 0.4];
const ROOF: [f32; 3] = [0.74, 0.73, 0.7];
const UNDERSIDE: [f32; 3] = [0.42, 0.43, 0.47];
const INNER_WALL: [f32; 3] = [0.88, 0.82, 0.72];
const FLOOR: [f32; 3] = [0.6, 0.45, 0.33];
const FLOOR_ALTERNATE: [f32; 3] = [0.54, 0.4, 0.3];
const CEILING: [f32; 3] = [0.93, 0.91, 0.86];
const GLASS: [f32; 3] = [0.55, 0.8, 0.9];

/// Height of the coloured band at the foot of every outside wall.
const BAND_HEIGHT_M: f32 = 0.5;
/// The floor's two shades alternate in squares this size.
const FLOOR_SQUARE_M: f32 = 3.0;
/// Outside walls have a seam this often, and long windows a mullion.
const SEAM_EVERY_M: f32 = 3.0;
const MULLION_EVERY_M: f32 = 1.5;
/// Half the thickness of an interior wall.
const PARTITION_HALF_M: f32 = 0.06;
/// How far the parapet rises above the roof it surrounds.
const PARAPET_RISE_M: f32 = 0.32;

/// Triangles with flat faces: three entries per triangle, no shared corners.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Surface {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub colours: Vec<[f32; 3]>,
}

impl Surface {
    pub fn triangle_count(&self) -> usize {
        self.positions.len() / 3
    }

    /// A rectangle seen from the side its normal points to. Corners go low-left,
    /// low-right, high-right, high-left as seen from that side.
    fn quad(&mut self, corners: [Vec3; 4], normal: Vec3, colour: [f32; 3]) {
        let [a, b, c, d] = corners;
        self.positions.extend([a, b, c, a, c, d]);
        self.normals.extend([normal; 6]);
        self.colours.extend([colour; 6]);
    }
}

/// A base ready to draw, in the base's own space, in metres.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BaseMesh {
    /// Walls, floors, roofs and trim, seen from outside and from inside.
    pub structure: Surface,
    /// Window glass, to be drawn see-through and from both sides.
    pub glass: Surface,
}

/// A rectangle on a face, in metres from the face's low-left corner as seen from outside.
#[derive(Clone, Copy)]
struct Rect {
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
}

impl Rect {
    fn is_empty(self) -> bool {
        self.right - self.left <= 1e-4 || self.top - self.bottom <= 1e-4
    }
}

pub(crate) fn build(base: &Base) -> BaseMesh {
    let mut mesh = BaseMesh::default();
    for face in base.hull() {
        let frame = Frame::of(face);
        let (outside, inside) = match face.side {
            Side::Up => (ROOF, CEILING),
            Side::Down => (UNDERSIDE, floor_colour(face.tile())),
            _ => (HULL, INNER_WALL),
        };
        let hole = opening_in(base.panel(face), &frame);
        for solid in around(frame.whole(), hole) {
            mesh.structure
                .quad(frame.corners(solid, false), frame.normal, outside);
            mesh.structure
                .quad(frame.corners(solid, true), -frame.normal, inside);
        }
        if face.side.is_wall() {
            wall_trim(&mut mesh, base, &frame, face);
            opening_trim(&mut mesh, base, &frame, face, hole);
        }
    }
    // Interior walls: a slab with some thickness, so it reads as a wall from both sides,
    // with the same openings an outside wall can have.
    for (face, panel) in base.partitions() {
        let frame = Frame::of(face);
        let hole = opening_in(panel, &frame);
        for solid in around(frame.whole(), hole) {
            frame.block(
                &mut mesh.structure,
                solid,
                -PARTITION_HALF_M,
                PARTITION_HALF_M,
                INNER_WALL,
            );
        }
        opening_trim(&mut mesh, base, &frame, face, hole);
    }
    // Where one cell sits on another there is a floor between them: the ceiling of the
    // lower storey and the floor of the upper one.
    for (tile, _) in base.tiles() {
        if base.contains(tile + IVec3::Y) {
            let frame = Frame::of(Face::new(tile, Side::Up));
            mesh.structure.quad(
                frame.corners(frame.whole(), false),
                frame.normal,
                floor_colour(tile + IVec3::Y),
            );
            mesh.structure
                .quad(frame.corners(frame.whole(), true), -frame.normal, CEILING);
        }
    }
    mesh
}

/// The part of a wall cell that a window or door leaves open: its full width, and as
/// high as the panel says.
fn opening_in(panel: Panel, frame: &Frame) -> Option<Rect> {
    let (bottom, top) = match panel {
        Panel::Solid => return None,
        Panel::Window { sill, top } => (sill, frame.height - top),
        Panel::Door => (0.0, DOOR_HEIGHT_M),
    };
    Some(Rect {
        left: 0.0,
        right: frame.width,
        bottom,
        top,
    })
}

/// What is left of a wall cell around an opening: the strip under it and the strip over
/// it, or the whole cell if there is none.
fn around(whole: Rect, hole: Option<Rect>) -> Vec<Rect> {
    let strips = match hole {
        None => vec![whole],
        Some(hole) => vec![
            Rect {
                top: hole.bottom,
                ..whole
            },
            Rect {
                bottom: hole.top,
                ..whole
            },
        ],
    };
    strips
        .into_iter()
        .filter(|strip| !strip.is_empty())
        .collect()
}

/// Floors are laid in two shades, in squares, so a room's size can be read at a glance.
fn floor_colour(tile: IVec3) -> [f32; 3] {
    let square = crate::cells(FLOOR_SQUARE_M);
    if (tile.x.div_euclid(square) + tile.z.div_euclid(square)).rem_euclid(2) == 0 {
        FLOOR
    } else {
        FLOOR_ALTERNATE
    }
}

/// Whether a distance along a wall falls on one of the lines a rhythm of `every` metres
/// makes. Seams and mullions are spaced this way, so they do not depend on the grid.
fn on_rhythm(along: f32, every: f32) -> bool {
    ((along / every).round() * every - along).abs() < 1e-3
}

/// Everything fixed to the outside of one cell of an outside wall. What goes on it is
/// chosen by what is beside it: a post where the wall turns a corner, a seam where it
/// carries on, a parapet where the roof begins and a line where another storey starts.
fn wall_trim(mesh: &mut BaseMesh, base: &Base, frame: &Frame, face: Face) {
    let (width, height) = (frame.width, frame.height);
    let along = frame.right.as_ivec3();
    let continues = |step: IVec3| base.is_hull(Face::new(face.tile() + step, face.side));
    let solid = |step: IVec3| base.panel(Face::new(face.tile() + step, face.side)) == Panel::Solid;

    // The coloured band at the wall's foot, with a dark line over it, broken by a door.
    // Starting a hair above the floor, so that on an upper storey its underside is not
    // exactly where the ceiling below is.
    if base.panel(face) != Panel::Door {
        let band = Rect {
            left: 0.0,
            right: width,
            bottom: 0.004,
            top: BAND_HEIGHT_M,
        };
        frame.block(&mut mesh.structure, band, 0.0, 0.05, BAND);
        let line = Rect {
            bottom: BAND_HEIGHT_M,
            top: BAND_HEIGHT_M + 0.05,
            ..band
        };
        frame.block(&mut mesh.structure, line, 0.0, 0.065, DARK);
    }

    // A post where the wall turns a corner. Where it carries on, a seam every so often,
    // but not through a window or a door.
    for (edge, step) in [(0.0, -along), (width, along)] {
        if !continues(step) {
            let post = Rect {
                left: edge - 0.1,
                right: edge + 0.1,
                bottom: 0.004,
                top: height,
            };
            frame.block(&mut mesh.structure, post, 0.0, 0.09, POST);
        } else if edge > 0.0
            && solid(IVec3::ZERO)
            && solid(step)
            && on_rhythm(frame.along(edge), SEAM_EVERY_M)
        {
            let seam = Rect {
                left: edge - 0.012,
                right: edge + 0.012,
                bottom: BAND_HEIGHT_M + 0.05,
                top: height - 0.14,
            };
            frame.block(&mut mesh.structure, seam, 0.0, 0.012, DARK);
        }
    }

    // A parapet round the roof, or a line where the next storey starts.
    if base.contains(face.tile() + IVec3::Y) {
        let line = Rect {
            left: 0.0,
            right: width,
            bottom: height - 0.05,
            top: height + 0.05,
        };
        frame.block(&mut mesh.structure, line, 0.0, 0.04, DARK);
    } else {
        // In two pieces, so that the part below the roof stays outside the room. It
        // reaches past the end of the wall only where the wall ends.
        let reach = |step: IVec3| if continues(step) { 0.0 } else { 0.12 };
        let fascia = Rect {
            left: -reach(-along),
            right: width + reach(along),
            bottom: height - 0.14,
            top: height,
        };
        frame.block(&mut mesh.structure, fascia, 0.0, 0.12, PARAPET);
        let rail = Rect {
            bottom: height,
            top: height + PARAPET_RISE_M,
            ..fascia
        };
        frame.block(&mut mesh.structure, rail, -0.1, 0.12, PARAPET);
    }
}

/// The frame and glass of one cell of a window or doorway, in an outside wall or an
/// interior one. A run of cells is one opening: each cell brings its own stretch of head
/// and sill, and only the cells at the ends bring the uprights.
fn opening_trim(mesh: &mut BaseMesh, base: &Base, frame: &Frame, face: Face, hole: Option<Rect>) {
    let Some(hole) = hole else { return };
    let Some(panel) = base.filling(face) else {
        return;
    };
    let along = frame.right.as_ivec3();
    let on_hull = base.is_hull(face);
    let same = |step: IVec3| {
        let next = Face::new(face.tile() + step, face.side);
        base.is_hull(next) == on_hull && base.filling(next) == Some(panel)
    };
    let (starts, ends) = (!same(-along), !same(along));
    // An interior wall is seen from both sides, so its frame stands out both ways.
    let (back, front) = if on_hull { (-0.09, 0.07) } else { (-0.1, 0.1) };
    let member = if panel == Panel::Door { 0.09 } else { 0.06 };
    let mut bar = |rect: Rect, colour: [f32; 3]| {
        if !rect.is_empty() {
            frame.block(&mut mesh.structure, rect, back, front, colour);
        }
    };
    if starts {
        bar(
            Rect {
                right: hole.left + member,
                ..hole
            },
            DARK,
        );
    }
    if ends {
        bar(
            Rect {
                left: hole.right - member,
                ..hole
            },
            DARK,
        );
    }
    // The head, and for a window the bottom of the frame.
    bar(
        Rect {
            bottom: hole.top - member,
            ..hole
        },
        DARK,
    );
    if let Panel::Window { .. } = panel {
        bar(
            Rect {
                top: hole.bottom + member,
                ..hole
            },
            DARK,
        );
        mesh.glass
            .quad(frame.corners(hole, false), frame.normal, GLASS);
        // A long window is divided every so often.
        if !ends && on_rhythm(frame.along(hole.right), MULLION_EVERY_M) {
            let mullion = Rect {
                left: hole.right - 0.03,
                right: hole.right + 0.03,
                ..hole
            };
            frame.block(
                &mut mesh.structure,
                mullion,
                back + 0.03,
                front - 0.02,
                DARK,
            );
        }
        let sill = Rect {
            bottom: hole.bottom - 0.06,
            top: hole.bottom,
            ..hole
        };
        frame.block(&mut mesh.structure, sill, back - 0.03, front + 0.08, POST);
    }
}

pub(crate) fn corners(face: Face) -> [Vec3; 4] {
    let frame = Frame::of(face);
    frame.corners(frame.whole(), false)
}

/// A face's position and axes, so rectangles can be laid out on it in two dimensions.
struct Frame {
    origin: Vec3,
    right: Vec3,
    up: Vec3,
    normal: Vec3,
    width: f32,
    height: f32,
}

impl Frame {
    fn of(face: Face) -> Frame {
        let size = Vec3::new(CELL_M, STOREY_M, CELL_M);
        let low = face.tile().as_vec3() * size;
        let normal = face.side.offset().as_vec3();
        // `right` and `up` are chosen so that right x up = normal, which makes the
        // triangles face outward.
        let (right, up) = match face.side {
            Side::East => (Vec3::NEG_Z, Vec3::Y),
            Side::West => (Vec3::Z, Vec3::Y),
            Side::South => (Vec3::X, Vec3::Y),
            Side::North => (Vec3::NEG_X, Vec3::Y),
            Side::Up => (Vec3::X, Vec3::NEG_Z),
            Side::Down => (Vec3::X, Vec3::Z),
        };
        // The corner of the tile where both `right` and `up` start from zero.
        let corner = low
            + (normal.max(Vec3::ZERO) + (-right).max(Vec3::ZERO) + (-up).max(Vec3::ZERO)) * size;
        Frame {
            origin: corner,
            right,
            up,
            normal,
            width: (right.abs() * size).max_element(),
            height: (up.abs() * size).max_element(),
        }
    }

    /// How far a place across this face is along the wall it belongs to, in metres from
    /// the grid's origin. The same line on the wall gives the same answer from any cell.
    fn along(&self, across: f32) -> f32 {
        (self.origin + self.right * across).dot(self.right.abs())
    }

    /// The whole face.
    fn whole(&self) -> Rect {
        Rect {
            left: 0.0,
            right: self.width,
            bottom: 0.0,
            top: self.height,
        }
    }

    /// A box standing on this face: a rectangle on it, carried from `back` to `front`
    /// metres along the face's normal. Negative is behind the face.
    fn block(&self, surface: &mut Surface, rect: Rect, back: f32, front: f32, colour: [f32; 3]) {
        let at = |x: f32, y: f32, depth: f32| {
            self.origin + self.right * x + self.up * y + self.normal * depth
        };
        let (l, r, b, t) = (rect.left, rect.right, rect.bottom, rect.top);
        let (k, f) = (back, front);
        let sides = [
            (
                [at(l, b, f), at(r, b, f), at(r, t, f), at(l, t, f)],
                self.normal,
            ),
            (
                [at(r, b, k), at(l, b, k), at(l, t, k), at(r, t, k)],
                -self.normal,
            ),
            (
                [at(l, b, k), at(l, b, f), at(l, t, f), at(l, t, k)],
                -self.right,
            ),
            (
                [at(r, b, f), at(r, b, k), at(r, t, k), at(r, t, f)],
                self.right,
            ),
            (
                [at(l, t, f), at(r, t, f), at(r, t, k), at(l, t, k)],
                self.up,
            ),
            (
                [at(l, b, k), at(r, b, k), at(r, b, f), at(l, b, f)],
                -self.up,
            ),
        ];
        for (corners, normal) in sides {
            // A box standing flush on the face has no back to see, and drawing one would
            // put it exactly where the other side of the wall is.
            if back >= 0.0 && normal == -self.normal {
                continue;
            }
            surface.quad(corners, normal, colour);
        }
    }

    /// The corners of a rectangle on this face, ordered for whichever side is looking.
    fn corners(&self, rect: Rect, from_inside: bool) -> [Vec3; 4] {
        let at = |x: f32, y: f32| self.origin + self.right * x + self.up * y;
        let corners = [
            at(rect.left, rect.bottom),
            at(rect.right, rect.bottom),
            at(rect.right, rect.top),
            at(rect.left, rect.top),
        ];
        if from_inside {
            [corners[1], corners[0], corners[3], corners[2]]
        } else {
            corners
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Base, BuildOp, Face, Panel, STOREY_M, Side, cells};
    use glam::IVec3;

    /// A room six metres square.
    fn room() -> Base {
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [cells(6.0) - 1, 0, cells(6.0) - 1],
        });
        base
    }

    fn east(z: i32) -> Face {
        Face::new(IVec3::new(cells(6.0) - 1, 0, z), Side::East)
    }

    #[test]
    fn every_triangle_faces_the_way_its_normal_says() {
        let mut base = room();
        for z in 2..6 {
            base.apply(BuildOp::SetPanel(east(z), Panel::WINDOW));
        }
        for x in 3..6 {
            base.apply(BuildOp::SetPanel(
                Face::new(IVec3::new(x, 0, cells(6.0) - 1), Side::South),
                Panel::Door,
            ));
        }
        base.apply(BuildOp::SetPartition(
            Face::new(IVec3::new(4, 0, 4), Side::East),
            Some(Panel::TALL_WINDOW),
        ));
        base.apply(BuildOp::AddRoom {
            min: [0, 1, 0],
            max: [3, 1, 3],
        });
        let mesh = base.mesh();
        for surface in [&mesh.structure, &mesh.glass] {
            assert!(surface.triangle_count() > 0);
            for (corners, normals) in surface.positions.chunks(3).zip(surface.normals.chunks(3)) {
                let winding = (corners[1] - corners[0])
                    .cross(corners[2] - corners[0])
                    .normalize();
                assert!(
                    winding.dot(normals[0]) > 0.99,
                    "triangle winds against its normal"
                );
            }
        }
    }

    #[test]
    fn a_run_of_window_cells_is_one_window() {
        let mut base = room();
        let plain = base.mesh();
        assert_eq!(plain.glass.triangle_count(), 0);

        base.apply(BuildOp::SetPanel(east(4), Panel::WINDOW));
        let one = base.mesh();
        assert_eq!(one.glass.triangle_count(), 2);
        // A second cell beside it adds its glass, head and sill, but takes away an
        // upright from each of the two cells where they now meet.
        base.apply(BuildOp::SetPanel(east(5), Panel::WINDOW));
        let two = base.mesh();
        assert_eq!(two.glass.triangle_count(), 4);
        let added_by_first = one.structure.triangle_count() - plain.structure.triangle_count();
        let added_by_second = two.structure.triangle_count() - one.structure.triangle_count();
        assert!(added_by_second < added_by_first);

        // A door is an opening with no glass.
        base.apply(BuildOp::SetPanel(east(8), Panel::Door));
        assert_eq!(base.mesh().glass.triangle_count(), 4);
    }

    #[test]
    fn a_wall_has_a_post_where_it_turns_and_none_where_it_carries_on() {
        // Two rooms in a line have one long wall each side, so no more posts than one
        // room; a room built off the side of another adds corners, and so posts.
        let count = |base: &Base| base.mesh().structure.triangle_count();
        let mut in_line = room();
        in_line.apply(BuildOp::AddRoom {
            min: [cells(6.0), 0, 0],
            max: [cells(12.0) - 1, 0, cells(6.0) - 1],
        });
        let mut off_the_side = room();
        off_the_side.apply(BuildOp::AddRoom {
            min: [cells(6.0), 0, cells(3.0)],
            max: [cells(12.0) - 1, 0, cells(9.0) - 1],
        });
        assert_eq!(in_line.tile_count(), off_the_side.tile_count());
        assert!(count(&off_the_side) > count(&in_line));
    }

    #[test]
    fn an_interior_wall_is_drawn_with_its_opening() {
        let mut base = room();
        let plain = base.mesh().structure.triangle_count();
        let wall = Face::new(IVec3::new(2, 0, 2), Side::East);
        base.apply(BuildOp::SetPartition(wall, Some(Panel::Solid)));
        // One slab: six sides, two triangles each.
        assert_eq!(base.mesh().structure.triangle_count(), plain + 12);
        base.apply(BuildOp::SetPartition(wall, Some(Panel::WINDOW)));
        assert_eq!(base.mesh().glass.triangle_count(), 2);
        assert!(base.mesh().structure.triangle_count() > plain + 12);
    }

    #[test]
    fn the_mesh_stays_close_to_the_cells() {
        // Trim stands a little proud of the walls.
        let mesh = room().mesh();
        for p in &mesh.structure.positions {
            assert!(p.x >= -0.2 && p.x <= 6.0 + 0.2);
            assert!(p.y >= -1e-4 && p.y <= STOREY_M + 0.4);
            assert!(p.z >= -0.2 && p.z <= 6.0 + 0.2);
        }
    }

    #[test]
    fn drawing_the_same_base_twice_gives_the_same_mesh() {
        assert_eq!(room().mesh(), room().mesh());
    }
}
