//! Turning a base's shape into triangles.
//!
//! The shape is tiles; what makes it read as a building is the trim. Every wall gets a
//! coloured band at its foot, a post where it turns a corner, a seam where it meets the
//! next panel, and a parapet where the roof begins. Openings get frames, windows a sill,
//! doors a canopy with a light under it, and every ceiling a light panel.

use crate::{Base, DOOR_HEIGHT_M, DOOR_WIDTH_M, Face, Panel, STOREY_M, Side, TILE_M};
use glam::{IVec3, Vec3};

// Colours are as they should appear on screen (sRGB).
const HULL: [f32; 3] = [0.93, 0.9, 0.82];
const POST: [f32; 3] = [0.84, 0.81, 0.74];
const BAND: [f32; 3] = [0.9, 0.46, 0.17];
const DARK: [f32; 3] = [0.17, 0.18, 0.23];
const PARAPET: [f32; 3] = [0.33, 0.34, 0.4];
const ROOF: [f32; 3] = [0.74, 0.73, 0.7];
const SOLAR: [f32; 3] = [0.09, 0.13, 0.3];
const UNDERSIDE: [f32; 3] = [0.42, 0.43, 0.47];
const INNER_WALL: [f32; 3] = [0.88, 0.82, 0.72];
const FLOOR: [f32; 3] = [0.6, 0.45, 0.33];
const FLOOR_ALTERNATE: [f32; 3] = [0.54, 0.4, 0.3];
const CEILING: [f32; 3] = [0.93, 0.91, 0.86];
const GLASS: [f32; 3] = [0.55, 0.8, 0.9];
const LIGHT: [f32; 3] = [1.0, 0.86, 0.62];

/// Height of the coloured band at the foot of every outside wall.
const BAND_HEIGHT_M: f32 = 0.5;
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
    /// Light panels, to be drawn glowing.
    pub lights: Surface,
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
        let whole = frame.whole();
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
            if !rect.is_empty() {
                mesh.structure
                    .quad(frame.corners(rect, false), frame.normal, outside);
                mesh.structure
                    .quad(frame.corners(rect, true), -frame.normal, inside);
            }
        };
        match opening {
            None => solid(whole),
            Some((hole, _)) => {
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
            }
        }
        match face.side {
            Side::Up => roof_fittings(&mut mesh, &frame, face),
            Side::Down => {}
            _ => wall_trim(&mut mesh, base, &frame, face, opening),
        }
        if face.side == Side::Up {
            ceiling_light(&mut mesh, &frame);
        }
    }
    // Interior walls: a slab with some thickness, so it reads as a wall from both sides,
    // with the same openings an outside wall can have.
    for (face, panel) in base.partitions() {
        let frame = Frame::of(face);
        let whole = frame.whole();
        let opening = match panel {
            Panel::Solid => None,
            Panel::Window {
                left,
                right,
                sill,
                top,
            } => Some(Rect {
                left,
                right: frame.width - right,
                bottom: sill,
                top: frame.height - top,
            }),
            Panel::Door => {
                let side = (frame.width - DOOR_WIDTH_M) * 0.5;
                Some(Rect {
                    left: side,
                    right: frame.width - side,
                    bottom: 0.0,
                    top: DOOR_HEIGHT_M,
                })
            }
        };
        let strips = match opening {
            None => vec![whole],
            Some(hole) => vec![
                Rect {
                    right: hole.left,
                    ..whole
                },
                Rect {
                    left: hole.right,
                    ..whole
                },
                Rect {
                    left: hole.left,
                    right: hole.right,
                    top: hole.bottom,
                    ..whole
                },
                Rect {
                    left: hole.left,
                    right: hole.right,
                    bottom: hole.top,
                    ..whole
                },
            ],
        };
        for strip in strips.into_iter().filter(|strip| !strip.is_empty()) {
            frame.block(
                &mut mesh.structure,
                strip,
                -PARTITION_HALF_M,
                PARTITION_HALF_M,
                INNER_WALL,
            );
        }
        if let (Some(hole), Panel::Window { .. }) = (opening, panel) {
            mesh.glass
                .quad(frame.corners(hole, false), frame.normal, GLASS);
        }
    }
    // Where one tile sits on another there is a floor between them: the ceiling of the
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
            ceiling_light(&mut mesh, &frame);
        }
    }
    mesh
}

/// Floors are laid in two shades, tile by tile, so a room's size can be read at a glance.
fn floor_colour(tile: IVec3) -> [f32; 3] {
    if (tile.x + tile.z).rem_euclid(2) == 0 {
        FLOOR
    } else {
        FLOOR_ALTERNATE
    }
}

/// Everything fixed to the outside of one wall tile, and the frame of its opening.
fn wall_trim(
    mesh: &mut BaseMesh,
    base: &Base,
    frame: &Frame,
    face: Face,
    opening: Option<(Rect, bool)>,
) {
    let (width, height) = (frame.width, frame.height);
    let along = frame.right.as_ivec3();
    let continues = |step: IVec3| base.is_hull(Face::new(face.tile() + step, face.side));
    let door = opening.filter(|(_, glazed)| !glazed).map(|(hole, _)| hole);

    // The coloured band at the wall's foot, with a dark line over it, broken by a door.
    let mut footing = |left: f32, right: f32| {
        // Starting a hair above the floor, so that on an upper storey its underside is
        // not exactly where the ceiling below is.
        let band = Rect {
            left,
            right,
            bottom: 0.004,
            top: BAND_HEIGHT_M,
        };
        if !band.is_empty() {
            frame.block(&mut mesh.structure, band, 0.0, 0.05, BAND);
            let line = Rect {
                bottom: BAND_HEIGHT_M,
                top: BAND_HEIGHT_M + 0.05,
                ..band
            };
            frame.block(&mut mesh.structure, line, 0.0, 0.065, DARK);
        }
    };
    match door {
        Some(hole) => {
            footing(0.0, hole.left - 0.08);
            footing(hole.right + 0.08, width);
        }
        None => footing(0.0, width),
    }

    // A post where the wall turns a corner, a seam where it carries on.
    for (edge, step) in [(0.0, -along), (width, along)] {
        if !continues(step) {
            let post = Rect {
                left: edge - 0.1,
                right: edge + 0.1,
                bottom: 0.004,
                top: height,
            };
            frame.block(&mut mesh.structure, post, 0.0, 0.09, POST);
        } else if edge > 0.0 {
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
        // In two pieces, so that the part below the roof stays outside the room.
        let fascia = Rect {
            left: -0.12,
            right: width + 0.12,
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

    match opening {
        Some((hole, true)) => {
            mesh.glass
                .quad(frame.corners(hole, false), frame.normal, GLASS);
            let member = 0.06;
            let sides = [
                Rect {
                    right: hole.left + member,
                    ..hole
                },
                Rect {
                    left: hole.right - member,
                    ..hole
                },
                Rect {
                    top: hole.bottom + member,
                    ..hole
                },
                Rect {
                    bottom: hole.top - member,
                    ..hole
                },
            ];
            for side in sides {
                frame.block(&mut mesh.structure, side, -0.09, 0.07, DARK);
            }
            // A wide pane is divided down the middle.
            if hole.right - hole.left > 2.2 {
                let middle = (hole.left + hole.right) * 0.5;
                let mullion = Rect {
                    left: middle - 0.03,
                    right: middle + 0.03,
                    ..hole
                };
                frame.block(&mut mesh.structure, mullion, -0.06, 0.05, DARK);
            }
            let sill = Rect {
                left: (hole.left - 0.06).max(0.0),
                right: (hole.right + 0.06).min(width),
                bottom: hole.bottom - 0.06,
                top: hole.bottom,
            };
            frame.block(&mut mesh.structure, sill, -0.12, 0.15, POST);
        }
        Some((hole, false)) => {
            let member = 0.09;
            let surround = [
                Rect {
                    left: hole.left - member,
                    right: hole.left,
                    ..hole
                },
                Rect {
                    left: hole.right,
                    right: hole.right + member,
                    ..hole
                },
                Rect {
                    left: hole.left - member,
                    right: hole.right + member,
                    bottom: hole.top,
                    top: hole.top + member,
                },
            ];
            for side in surround {
                frame.block(&mut mesh.structure, side, -0.1, 0.1, DARK);
            }
            // A canopy over the door, with a light under it.
            let canopy = Rect {
                left: hole.left - 0.35,
                right: hole.right + 0.35,
                bottom: hole.top + 0.16,
                top: hole.top + 0.26,
            };
            frame.block(&mut mesh.structure, canopy, 0.0, 1.0, PARAPET);
            let lamp = Rect {
                left: hole.left + 0.1,
                right: hole.right - 0.1,
                bottom: hole.top + 0.12,
                top: hole.top + 0.16,
            };
            frame.block(&mut mesh.lights, lamp, 0.2, 0.8, LIGHT);
        }
        None => {}
    }
}

/// A light panel in the ceiling under an upward face.
fn ceiling_light(mesh: &mut BaseMesh, frame: &Frame) {
    let panel = Rect {
        left: frame.width * 0.5 - 0.55,
        right: frame.width * 0.5 + 0.55,
        bottom: frame.height * 0.5 - 0.55,
        top: frame.height * 0.5 + 0.55,
    };
    frame.block(&mut mesh.lights, panel, -0.05, -0.001, LIGHT);
}

/// What stands on a roof tile. Which tiles get what is fixed by where they are, so a
/// base looks the same every time it is drawn.
fn roof_fittings(mesh: &mut BaseMesh, frame: &Frame, face: Face) {
    let [x, y, z] = face.tile;
    let pick =
        (x.wrapping_mul(73_856_093) ^ y.wrapping_mul(19_349_663) ^ z.wrapping_mul(83_492_791))
            .rem_euclid(7);
    match pick {
        // A solar panel on short feet.
        0 | 3 | 5 => {
            let panel = Rect {
                left: 0.35,
                right: frame.width - 0.35,
                bottom: 0.45,
                top: frame.height - 0.45,
            };
            frame.block(&mut mesh.structure, panel, 0.2, 0.26, SOLAR);
            let frame_under = Rect {
                left: panel.left - 0.04,
                right: panel.right + 0.04,
                bottom: panel.bottom - 0.04,
                top: panel.top + 0.04,
            };
            frame.block(&mut mesh.structure, frame_under, 0.16, 0.2, DARK);
            for (left, bottom) in [(0.6, 0.7), (frame.width - 0.75, frame.height - 0.85)] {
                let foot = Rect {
                    left,
                    right: left + 0.15,
                    bottom,
                    top: bottom + 0.15,
                };
                frame.block(&mut mesh.structure, foot, 0.0, 0.16, DARK);
            }
        }
        // A vent.
        1 => {
            let body = Rect {
                left: 0.9,
                right: 1.8,
                bottom: 1.0,
                top: 1.7,
            };
            frame.block(&mut mesh.structure, body, 0.0, 0.5, POST);
            let cap = Rect {
                left: 0.82,
                right: 1.88,
                bottom: 0.92,
                top: 1.78,
            };
            frame.block(&mut mesh.structure, cap, 0.5, 0.58, DARK);
        }
        _ => {}
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
    use crate::{Base, BuildOp, Face, Panel, STOREY_M, Side, TILE_M};
    use glam::IVec3;

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
        base.apply(BuildOp::SetPanel(wall, Panel::WIDE_WINDOW));
        base.apply(BuildOp::SetPanel(
            Face::new(IVec3::new(0, 0, 2), Side::South),
            Panel::Door,
        ));
        base.apply(BuildOp::AddRoom {
            min: [0, 1, 0],
            max: [1, 1, 1],
        });
        let mesh = base.mesh();
        for surface in [&mesh.structure, &mesh.glass, &mesh.lights] {
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
    fn a_window_opens_the_wall_and_adds_glass() {
        let mut base = room();
        let plain = base.mesh();
        assert_eq!(plain.glass.triangle_count(), 0);

        let wall = Face::new(IVec3::new(2, 0, 1), Side::East);
        base.apply(BuildOp::SetPanel(wall, Panel::WINDOW));
        let framed = base.mesh();
        assert_eq!(framed.glass.triangle_count(), 2);
        assert!(framed.structure.triangle_count() > plain.structure.triangle_count());

        // A door is an opening with no glass, and a light over it.
        base.apply(BuildOp::SetPanel(wall, Panel::Door));
        let door = base.mesh();
        assert_eq!(door.glass.triangle_count(), 0);
        assert!(door.lights.triangle_count() > plain.lights.triangle_count());
    }

    #[test]
    fn every_ceiling_has_a_light() {
        // Nine tiles, one panel each, twelve triangles a panel.
        assert_eq!(room().mesh().lights.triangle_count(), 9 * 12);
    }

    #[test]
    fn the_mesh_stays_close_to_the_tiles() {
        // Trim stands a little proud of the walls, and a canopy further.
        let mesh = room().mesh();
        for p in &mesh.structure.positions {
            assert!(p.x >= -0.2 && p.x <= 3.0 * TILE_M + 0.2);
            assert!(p.y >= -1e-4 && p.y <= STOREY_M + 0.7);
            assert!(p.z >= -0.2 && p.z <= 3.0 * TILE_M + 0.2);
        }
    }

    #[test]
    fn an_interior_wall_is_drawn_with_its_opening() {
        let mut base = room();
        let plain = base.mesh().structure.triangle_count();
        let wall = Face::new(IVec3::new(0, 0, 1), Side::East);
        base.apply(BuildOp::SetPartition(wall, Some(Panel::Solid)));
        // One slab: six sides, two triangles each.
        assert_eq!(base.mesh().structure.triangle_count(), plain + 12);
        base.apply(BuildOp::SetPartition(wall, Some(Panel::Door)));
        // A doorway leaves three slabs: one each side and one over it.
        assert_eq!(base.mesh().structure.triangle_count(), plain + 36);
        base.apply(BuildOp::SetPartition(wall, Some(Panel::WINDOW)));
        assert_eq!(base.mesh().glass.triangle_count(), 2);
    }

    #[test]
    fn drawing_the_same_base_twice_gives_the_same_mesh() {
        assert_eq!(room().mesh(), room().mesh());
    }
}
