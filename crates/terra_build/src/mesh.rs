//! Turning a base's shape into triangles.

use crate::{Base, DOOR_HEIGHT_M, DOOR_WIDTH_M, Face, Panel, STOREY_M, Side, TILE_M};
use glam::{IVec3, Vec3};

const HULL: [f32; 3] = [0.84, 0.85, 0.88];
const ROOF: [f32; 3] = [0.74, 0.76, 0.8];
const UNDERSIDE: [f32; 3] = [0.42, 0.43, 0.47];
const INNER_WALL: [f32; 3] = [0.82, 0.78, 0.7];
const FLOOR: [f32; 3] = [0.34, 0.33, 0.36];
const CEILING: [f32; 3] = [0.9, 0.89, 0.86];

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
    /// Walls, floors and roofs, seen from outside and from inside.
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

pub(crate) fn build(base: &Base) -> BaseMesh {
    let mut mesh = BaseMesh::default();
    for face in base.hull() {
        let frame = Frame::of(face);
        let (outside, inside) = match face.side {
            Side::Up => (ROOF, CEILING),
            Side::Down => (UNDERSIDE, FLOOR),
            _ => (HULL, INNER_WALL),
        };
        let whole = Rect {
            left: 0.0,
            right: frame.width,
            bottom: 0.0,
            top: frame.height,
        };
        let opening = match base.panel(face) {
            Panel::Solid => None,
            Panel::Window {
                left,
                right,
                sill,
                top,
            } => Some((
                Rect {
                    left,
                    right: frame.width - right,
                    bottom: sill,
                    top: frame.height - top,
                },
                true,
            )),
            Panel::Door => {
                let side = (frame.width - DOOR_WIDTH_M) * 0.5;
                Some((
                    Rect {
                        left: side,
                        right: frame.width - side,
                        bottom: 0.0,
                        top: DOOR_HEIGHT_M,
                    },
                    false,
                ))
            }
        };
        let mut solid = |rect: Rect| {
            if rect.right - rect.left > 1e-4 && rect.top - rect.bottom > 1e-4 {
                mesh.structure
                    .quad(frame.corners(rect, false), frame.normal, outside);
                mesh.structure
                    .quad(frame.corners(rect, true), -frame.normal, inside);
            }
        };
        match opening {
            None => solid(whole),
            Some((hole, glazed)) => {
                // The wall around the opening, as four strips.
                solid(Rect {
                    right: hole.left,
                    ..whole
                });
                solid(Rect {
                    left: hole.right,
                    ..whole
                });
                solid(Rect {
                    left: hole.left,
                    right: hole.right,
                    top: hole.bottom,
                    ..whole
                });
                solid(Rect {
                    left: hole.left,
                    right: hole.right,
                    bottom: hole.top,
                    ..whole
                });
                if glazed {
                    mesh.glass
                        .quad(frame.corners(hole, false), frame.normal, [0.55, 0.8, 0.9]);
                }
            }
        }
    }
    // Where one tile sits on another there is a floor between them: the ceiling of the
    // lower storey and the floor of the upper one.
    for (tile, _) in base.tiles() {
        if base.contains(tile + IVec3::Y) {
            let frame = Frame::of(Face::new(tile, Side::Up));
            let whole = Rect {
                left: 0.0,
                right: frame.width,
                bottom: 0.0,
                top: frame.height,
            };
            mesh.structure
                .quad(frame.corners(whole, false), frame.normal, FLOOR);
            mesh.structure
                .quad(frame.corners(whole, true), -frame.normal, CEILING);
        }
    }
    mesh
}

pub(crate) fn corners(face: Face) -> [Vec3; 4] {
    let frame = Frame::of(face);
    frame.corners(
        Rect {
            left: 0.0,
            right: frame.width,
            bottom: 0.0,
            top: frame.height,
        },
        false,
    )
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
        let size = Vec3::new(TILE_M, STOREY_M, TILE_M);
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
    use crate::{Base, BuildOp, Face, Panel, STOREY_M, Side, TILE_M};
    use glam::{IVec3, Vec3};

    fn room() -> Base {
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [2, 0, 2],
        });
        base
    }

    #[test]
    fn every_triangle_faces_the_way_its_normal_says() {
        let mut base = room();
        let wall = Face::new(IVec3::new(2, 0, 1), Side::East);
        base.apply(BuildOp::SetPanel(wall, Panel::WINDOW));
        base.apply(BuildOp::SetPanel(
            Face::new(IVec3::new(0, 0, 2), Side::South),
            Panel::Door,
        ));
        let mesh = base.mesh();
        for surface in [&mesh.structure, &mesh.glass] {
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
    fn outside_faces_point_away_from_the_room_and_inside_faces_into_it() {
        let mesh = room().mesh();
        let centre = Vec3::new(1.5 * TILE_M, 0.5 * STOREY_M, 1.5 * TILE_M);
        let (mut outward, mut inward) = (0, 0);
        for (corners, normals) in mesh
            .structure
            .positions
            .chunks(3)
            .zip(mesh.structure.normals.chunks(3))
        {
            let middle = (corners[0] + corners[1] + corners[2]) / 3.0;
            if (middle - centre).dot(normals[0]) > 0.0 {
                outward += 1
            } else {
                inward += 1
            }
        }
        // 30 hull faces, each drawn once from outside and once from inside, two triangles each.
        assert_eq!((outward, inward), (60, 60));
    }

    #[test]
    fn a_window_opens_the_wall_and_adds_glass() {
        let mut base = room();
        let plain = base.mesh();
        assert_eq!(plain.glass.triangle_count(), 0);

        let wall = Face::new(IVec3::new(2, 0, 1), Side::East);
        base.apply(BuildOp::SetPanel(wall, Panel::WINDOW));
        let framed = base.mesh();
        assert_eq!(framed.glass.triangle_count(), 2);
        // One solid tile (4 triangles, both sides) becomes four strips (16).
        assert_eq!(
            framed.structure.triangle_count(),
            plain.structure.triangle_count() + 12
        );

        // A full-width window has no strips at its sides.
        base.apply(BuildOp::SetPanel(wall, Panel::WIDE_WINDOW));
        assert_eq!(
            base.mesh().structure.triangle_count(),
            plain.structure.triangle_count() + 4
        );

        // A door is an opening with no glass, and nothing under it.
        base.apply(BuildOp::SetPanel(wall, Panel::Door));
        let door = base.mesh();
        assert_eq!(door.glass.triangle_count(), 0);
        assert_eq!(
            door.structure.triangle_count(),
            plain.structure.triangle_count() + 8
        );
    }

    #[test]
    fn the_mesh_fits_the_tiles() {
        let mesh = room().mesh();
        for p in &mesh.structure.positions {
            assert!(p.x >= -1e-4 && p.x <= 3.0 * TILE_M + 1e-4);
            assert!(p.y >= -1e-4 && p.y <= STOREY_M + 1e-4);
            assert!(p.z >= -1e-4 && p.z <= 3.0 * TILE_M + 1e-4);
        }
    }
}
