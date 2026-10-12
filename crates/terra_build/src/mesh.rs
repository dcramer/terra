//! Turning a base's walls, floors and roofs into triangles.
//!
//! What makes them read as a building is the trim, chosen by what each piece is next to
//! (D56). A wall gets a post where it ends or turns a corner and none where it carries
//! on; a band at its foot on the side that is outdoors; a parapet where a roof meets it
//! and a plain line where another storey starts. Openings get frames, and windows a sill.
//!
//! These are plain boxes, for now: each kind of construction laying its own pieces comes
//! later (docs/structures.md).

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
/// Long windows have a mullion this often.
const MULLION_EVERY_M: f32 = 1.5;
/// Half the thickness of a wall.
const WALL_HALF_M: f32 = 0.05;
/// Half the width of a post where walls end or turn.
const POST_HALF_M: f32 = 0.08;
/// How far the parapet rises above the roof it surrounds.
const PARAPET_RISE_M: f32 = 0.2;

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

    /// A triangle seen from the side its normal points to, wound counter-clockwise from
    /// there.
    pub(crate) fn triangle(&mut self, corners: [Vec3; 3], normal: Vec3, colour: [f32; 3]) {
        self.positions.extend(corners);
        self.normals.extend([normal; 3]);
        self.colours.extend([colour; 3]);
    }

    /// A flat, convex shape seen from the side its normal points to, its corners in order
    /// counter-clockwise from there. Corners that meet, as a gable's do at its foot, are
    /// left out.
    fn polygon(&mut self, corners: &[Vec3], normal: Vec3, colour: [f32; 3]) {
        let first = corners[0];
        for pair in corners[1..].windows(2) {
            let area = (pair[0] - first).cross(pair[1] - first).length();
            if area > 1e-6 {
                self.triangle([first, pair[0], pair[1]], normal, colour);
            }
        }
    }

    /// A rectangle seen from the side its normal points to. Corners go low-left,
    /// low-right, high-right, high-left as seen from that side.
    pub(crate) fn quad(&mut self, corners: [Vec3; 4], normal: Vec3, colour: [f32; 3]) {
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

/// Builds a base's mesh. With a kit's colours for a gable and a roof, its walls are left
/// to the kit's pieces, but for what is behind a gable's.
pub(crate) fn build(base: &Base, kit: Option<([f32; 3], [f32; 3])>) -> BaseMesh {
    let mut mesh = BaseMesh::default();
    let walls = kit.map(|(gable, _)| gable);
    let roof = kit.map_or(ROOF, |(_, roof)| roof);
    if let Some(colour) = walls {
        for (face, _) in base.walls() {
            gable(&mut mesh, base, &Frame::of(face), face, Some(colour));
        }
    }
    for (face, panel) in base.walls().filter(|_| walls.is_none()) {
        let frame = Frame::of(face);
        // Each side of a wall is coloured by whether it faces into a room.
        let front = side_colour(base.contains(face.tile() + face.side.offset()));
        let back = side_colour(base.contains(face.tile()));
        let hole = opening_in(panel, &frame);
        for solid in around(frame.whole(), hole) {
            frame.slab(&mut mesh.structure, solid, front, back);
        }
        wall_trim(&mut mesh, base, &frame, face, panel);
        opening_trim(&mut mesh, base, &frame, face, panel, hole);
    }
    if walls.is_none() {
        posts(&mut mesh, base);
    }
    for tile in base.floors() {
        let frame = Frame::of(Face::new(tile, Side::Down));
        mesh.structure.quad(
            frame.corners(frame.whole(), true),
            -frame.normal,
            floor_colour(tile),
        );
        // Its underside, unless a roof below is there to be seen instead.
        if !base.has_roof(tile - IVec3::Y) {
            mesh.structure
                .quad(frame.corners(frame.whole(), false), frame.normal, UNDERSIDE);
        }
    }
    for tile in base.roofs() {
        let frame = Frame::of(Face::new(tile, Side::Up));
        // Its top, unless a floor above is there to be seen instead.
        if !base.has_floor(tile + IVec3::Y) {
            mesh.structure
                .quad(frame.corners(frame.whole(), false), frame.normal, roof);
        }
        mesh.structure
            .quad(frame.corners(frame.whole(), true), -frame.normal, CEILING);
    }
    pitched_roofs(&mut mesh, base, roof);
    mesh
}

/// How finely a pitched roof is sampled, in metres: a quarter of a cell, on which every
/// ridge, hip and valley of a roof over walls in four directions falls, or on its
/// diagonals (research/procedural-buildings.md).
const ROOF_STEP_M: f32 = CELL_M * 0.5;
/// How thick a pitched roof is.
pub(crate) const ROOF_THICK_M: f32 = 0.1;

/// Every pitched roof, as its height above the walls sampled on a fine lattice: each small
/// square in four triangles from its middle, so the faces are as flat as the roof. Its top
/// is roof and its underside ceiling.
fn pitched_roofs(mesh: &mut BaseMesh, base: &Base, roof: [f32; 3]) {
    let mut squares = std::collections::BTreeSet::new();
    for section in base.sections() {
        let per = (CELL_M / ROOF_STEP_M).round() as i32;
        for x in section.min[0] * per..(section.max[0] + 1) * per {
            for z in section.min[2] * per..(section.max[2] + 1) * per {
                squares.insert([x, section.storey(), z]);
            }
        }
    }
    for [x, storey, z] in squares {
        let eaves = (storey + 1) as f32 * STOREY_M;
        let at = |dx: f32, dz: f32| {
            let along =
                glam::Vec2::new((x as f32 + dx) * ROOF_STEP_M, (z as f32 + dz) * ROOF_STEP_M);
            let rise = base.roof_rise(storey, along).unwrap_or(0.0);
            Vec3::new(along.x, eaves + rise, along.y)
        };
        let middle = at(0.5, 0.5);
        let corners = [at(0.0, 0.0), at(1.0, 0.0), at(1.0, 1.0), at(0.0, 1.0)];
        for i in 0..4 {
            let (a, b) = (corners[i], corners[(i + 1) % 4]);
            let normal = (a - middle).cross(b - middle).normalize_or_zero();
            // Wound so that its top faces up.
            let (a, b, normal) = if normal.y < 0.0 {
                (b, a, -normal)
            } else {
                (a, b, normal)
            };
            mesh.structure.triangle([middle, a, b], normal, roof);
            let down = Vec3::Y * ROOF_THICK_M;
            mesh.structure
                .triangle([middle - down, b - down, a - down], -normal, CEILING);
        }
    }
}

/// The colour of one side of a wall: inside a room, or outdoors.
fn side_colour(indoors: bool) -> [f32; 3] {
    if indoors { INNER_WALL } else { HULL }
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
/// makes. Mullions are spaced this way, so they do not depend on the grid.
fn on_rhythm(along: f32, every: f32) -> bool {
    ((along / every).round() * every - along).abs() < 1e-3
}

/// How far from the wall's middle a block on one side of it reaches: `from` to `to`
/// metres out from its face, in front (`1.0`) or behind (`-1.0`).
fn out_from(sign: f32, from: f32, to: f32) -> (f32, f32) {
    let (near, far) = (WALL_HALF_M + from, WALL_HALF_M + to);
    if sign > 0.0 {
        (near, far)
    } else {
        (-far, -near)
    }
}

/// Everything fixed to one cell of a wall, chosen by what is beside it: a band at its foot
/// on a side that is outdoors, a line where another storey starts above, and a parapet
/// where a roof meets it.
fn wall_trim(mesh: &mut BaseMesh, base: &Base, frame: &Frame, face: Face, panel: Panel) {
    let (width, height) = (frame.width, frame.height);
    let tile = face.tile();
    let beyond = tile + face.side.offset();
    // Front is the side the face's normal points to, the cell beyond; back is its own.
    let outdoors = [(1.0, !base.contains(beyond)), (-1.0, !base.contains(tile))];

    // The coloured band at the foot of an outdoor side, with a dark line over it, broken
    // by a door. Starting a hair above the floor, so that on an upper storey its
    // underside is not exactly where the ceiling below is.
    if panel != Panel::Door {
        for (sign, _) in outdoors.iter().filter(|(_, out)| *out) {
            let band = Rect {
                left: 0.0,
                right: width,
                bottom: 0.004,
                top: BAND_HEIGHT_M,
            };
            let (back, front) = out_from(*sign, 0.0, 0.02);
            frame.block(&mut mesh.structure, band, back, front, BAND);
            let line = Rect {
                bottom: BAND_HEIGHT_M,
                top: BAND_HEIGHT_M + 0.05,
                ..band
            };
            let (back, front) = out_from(*sign, 0.0, 0.03);
            frame.block(&mut mesh.structure, line, back, front, DARK);
        }
    }

    // Up to a pitched roof over it: a gable end, or a wall inside up to the slope.
    gable(mesh, base, frame, face, None);

    // A line where the wall carries on up another storey; else a cap on its top, and
    // where a roof meets it, a parapet over the roof and a fascia under its edge.
    if base
        .filling(Face::new(tile + IVec3::Y, face.side))
        .is_some()
    {
        let line = Rect {
            left: 0.0,
            right: width,
            bottom: height - 0.05,
            top: height + 0.05,
        };
        frame.block(
            &mut mesh.structure,
            line,
            -WALL_HALF_M - 0.02,
            WALL_HALF_M + 0.02,
            DARK,
        );
        return;
    }
    let roofed = base.has_roof(tile) || base.has_roof(beyond);
    let cap = Rect {
        left: 0.0,
        right: width,
        bottom: height,
        top: height + if roofed { PARAPET_RISE_M } else { 0.06 },
    };
    frame.block(
        &mut mesh.structure,
        cap,
        -WALL_HALF_M - 0.015,
        WALL_HALF_M + 0.015,
        PARAPET,
    );
    if roofed {
        for (sign, _) in outdoors.iter().filter(|(_, out)| *out) {
            let fascia = Rect {
                bottom: height - 0.14,
                top: height,
                ..cap
            };
            let (back, front) = out_from(*sign, 0.0, 0.05);
            frame.block(&mut mesh.structure, fascia, back, front, PARAPET);
        }
    }
}

/// The wall carried up to a pitched roof over it, as far as the roof's underside, so a
/// gable end is closed and a wall inside meets the slope. In two pieces along the cell,
/// so a ridge over its middle is followed.
fn gable(mesh: &mut BaseMesh, base: &Base, frame: &Frame, face: Face, colour: Option<[f32; 3]>) {
    let storey = face.tile[1];
    let rise = |across: f32| {
        let at = frame.origin + frame.right * across;
        base.roof_rise(storey, glam::Vec2::new(at.x, at.z))
            .map(|rise| (rise - ROOF_THICK_M).max(0.0))
    };
    let half = frame.width * 0.5;
    let (Some(start), Some(middle), Some(end)) = (rise(0.0), rise(half), rise(frame.width)) else {
        return;
    };
    let behind = colour.unwrap_or(side_colour(base.contains(face.tile())));
    let colour = colour.unwrap_or(side_colour(base.contains(face.tile() + face.side.offset())));
    let height = frame.height;
    let at = |across: f32, up: f32, depth: f32| {
        frame.origin + frame.right * across + frame.up * up + frame.normal * depth
    };
    for (left, right, rise_left, rise_right) in
        [(0.0, half, start, middle), (half, frame.width, middle, end)]
    {
        if rise_left.max(rise_right) < 0.01 {
            continue;
        }
        let (k, f) = (-WALL_HALF_M, WALL_HALF_M);
        let (top_left, top_right) = (height + rise_left, height + rise_right);
        mesh.structure.polygon(
            &[
                at(left, height, f),
                at(right, height, f),
                at(right, top_right, f),
                at(left, top_left, f),
            ],
            frame.normal,
            colour,
        );
        mesh.structure.polygon(
            &[
                at(right, height, k),
                at(left, height, k),
                at(left, top_left, k),
                at(right, top_right, k),
            ],
            -frame.normal,
            behind,
        );
    }
}

/// A post at every corner of a wall's storey where walls end, turn or meet, and none
/// where one wall carries straight on.
fn posts(mesh: &mut BaseMesh, base: &Base) {
    // The grid points at each end of every wall cell, with their storey.
    let mut points = std::collections::BTreeSet::new();
    for (face, _) in base.walls() {
        let tile = face.tile();
        let ends = match face.side {
            Side::East => [tile + IVec3::X, tile + IVec3::X + IVec3::Z],
            _ => [tile + IVec3::Z, tile + IVec3::X + IVec3::Z],
        };
        points.extend(ends.map(|point| point.to_array()));
    }
    for point in points {
        let point = IVec3::from_array(point);
        let has = |cell: IVec3, side: Side| base.filling(Face::new(cell, side)).is_some();
        let east = has(point - IVec3::Z, Side::South);
        let west = has(point - IVec3::X - IVec3::Z, Side::South);
        let south = has(point - IVec3::X, Side::East);
        let north = has(point - IVec3::X - IVec3::Z, Side::East);
        let straight = (east && west && !south && !north) || (south && north && !east && !west);
        if straight {
            continue;
        }
        let low = point.as_vec3() * Vec3::new(CELL_M, STOREY_M, CELL_M);
        let frame = Frame {
            origin: low + Vec3::new(0.0, 0.004, POST_HALF_M),
            right: Vec3::NEG_Z,
            up: Vec3::Y,
            normal: Vec3::X,
            width: POST_HALF_M * 2.0,
            height: STOREY_M - 0.004,
        };
        frame.block(
            &mut mesh.structure,
            frame.whole(),
            -POST_HALF_M,
            POST_HALF_M,
            POST,
        );
    }
}

/// The frame and glass of one cell of a window or doorway. A run of cells is one
/// opening: each cell brings its own stretch of head and sill, and only the cells at the
/// ends bring the uprights. A wall is seen from both sides, so its frame stands out both
/// ways.
fn opening_trim(
    mesh: &mut BaseMesh,
    base: &Base,
    frame: &Frame,
    face: Face,
    panel: Panel,
    hole: Option<Rect>,
) {
    let Some(hole) = hole else { return };
    let along = frame.right.as_ivec3();
    let same = |step: IVec3| base.filling(Face::new(face.tile() + step, face.side)) == Some(panel);
    let (starts, ends) = (!same(-along), !same(along));
    let (back, front) = (-WALL_HALF_M - 0.025, WALL_HALF_M + 0.025);
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
                front - 0.03,
                DARK,
            );
        }
        let sill = Rect {
            bottom: hole.bottom - 0.06,
            top: hole.bottom,
            ..hole
        };
        frame.block(&mut mesh.structure, sill, back - 0.05, front + 0.05, POST);
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

    /// A wall's slab over a rectangle of this face, `WALL_HALF_M` either side of it: its
    /// front in one colour, its back in another, its edges, seen at openings, in the
    /// front's.
    fn slab(&self, surface: &mut Surface, rect: Rect, front: [f32; 3], back: [f32; 3]) {
        let at = |x: f32, y: f32, depth: f32| {
            self.origin + self.right * x + self.up * y + self.normal * depth
        };
        let (l, r, b, t) = (rect.left, rect.right, rect.bottom, rect.top);
        let (k, f) = (-WALL_HALF_M, WALL_HALF_M);
        surface.quad(
            [at(l, b, f), at(r, b, f), at(r, t, f), at(l, t, f)],
            self.normal,
            front,
        );
        surface.quad(
            [at(r, b, k), at(l, b, k), at(l, t, k), at(r, t, k)],
            -self.normal,
            back,
        );
        let edges = [
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
        for (corners, normal) in edges {
            surface.quad(corners, normal, front);
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
    use crate::{Base, BuildOp, Face, Panel, RoofSection, RoofStyle, STOREY_M, Side, cells};
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
            base.apply(BuildOp::SetWall(east(z), Some(Panel::WINDOW)));
        }
        for x in 3..6 {
            base.apply(BuildOp::SetWall(
                Face::new(IVec3::new(x, 0, cells(6.0) - 1), Side::South),
                Some(Panel::Door),
            ));
        }
        base.apply(BuildOp::SetWall(
            Face::new(IVec3::new(4, 0, 4), Side::East),
            Some(Panel::TALL_WINDOW),
        ));
        base.apply(BuildOp::AddRoom {
            min: [0, 1, 0],
            max: [3, 1, 3],
        });
        for (style, along_x) in [(RoofStyle::Gable, true), (RoofStyle::Hip, false)] {
            base.apply(BuildOp::AddRoof(RoofSection {
                min: [0, 1, 0],
                max: [3, 1, 3],
                style,
                along_x,
                pitch: 0.7,
            }));
        }
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

        base.apply(BuildOp::SetWall(east(4), Some(Panel::WINDOW)));
        let one = base.mesh();
        assert_eq!(one.glass.triangle_count(), 2);
        // A second cell beside it adds its glass, head and sill, but takes away an
        // upright from each of the two cells where they now meet.
        base.apply(BuildOp::SetWall(east(5), Some(Panel::WINDOW)));
        let two = base.mesh();
        assert_eq!(two.glass.triangle_count(), 4);
        let added_by_first = one.structure.triangle_count() - plain.structure.triangle_count();
        let added_by_second = two.structure.triangle_count() - one.structure.triangle_count();
        assert!(added_by_second < added_by_first);

        // A door is an opening with no glass.
        base.apply(BuildOp::SetWall(east(8), Some(Panel::Door)));
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
    fn a_wall_across_a_room_is_drawn_with_its_opening() {
        let mut base = room();
        let plain = base.mesh().structure.triangle_count();
        let wall = Face::new(IVec3::new(2, 0, 2), Side::East);
        base.apply(BuildOp::SetWall(wall, Some(Panel::Solid)));
        assert!(base.mesh().structure.triangle_count() > plain);
        base.apply(BuildOp::SetWall(wall, Some(Panel::WINDOW)));
        assert_eq!(base.mesh().glass.triangle_count(), 2);
    }

    #[test]
    fn the_mesh_stays_close_to_the_cells() {
        // Trim stands a little proud of the walls.
        let mesh = room().mesh();
        for p in &mesh.structure.positions {
            assert!(p.x >= -0.3 && p.x <= 6.0 + 0.3);
            assert!(p.y >= -1e-4 && p.y <= STOREY_M + 0.4);
            assert!(p.z >= -0.3 && p.z <= 6.0 + 0.3);
        }
    }

    #[test]
    fn a_gable_roof_closes_its_ends_and_a_hip_does_not_need_to() {
        let roofed = |style| {
            let mut base = room();
            base.apply(BuildOp::AddRoof(RoofSection {
                min: [0, 0, 0],
                max: [cells(6.0) - 1, 0, cells(6.0) - 1],
                style,
                along_x: true,
                pitch: 0.7,
            }));
            base.mesh()
        };
        let highest = |mesh: &crate::BaseMesh| {
            mesh.structure
                .positions
                .iter()
                .map(|p| p.y)
                .fold(f32::MIN, f32::max)
        };
        // Six metres across: three to the ridge, at 0.7 rise a metre.
        assert!((highest(&roofed(RoofStyle::Hip)) - (STOREY_M + 2.1)).abs() < 0.05);
        // The gable's ends are walled up to the roof; the hip's walls stop at the eaves.
        let wall_quads_above_eaves = |mesh: &crate::BaseMesh| {
            mesh.structure
                .positions
                .chunks(3)
                .zip(mesh.structure.normals.chunks(3))
                .filter(|(_, n)| n[0].y.abs() < 0.01)
                .filter(|(p, _)| p.iter().any(|p| p.y > STOREY_M + 0.5))
                .count()
        };
        assert!(wall_quads_above_eaves(&roofed(RoofStyle::Gable)) > 0);
        assert_eq!(wall_quads_above_eaves(&roofed(RoofStyle::Hip)), 0);
    }

    #[test]
    fn drawing_the_same_base_twice_gives_the_same_mesh() {
        assert_eq!(room().mesh(), room().mesh());
    }
}
