//! Turning a base's cubes into triangles.
//!
//! Walls of a kind with a kit are left to the kit, which lays its own pieces
//! (`logs.rs`); everything else is drawn here: walls of the plain kind as their cubes, a
//! floor or flat roof as its cubes with a floor's colour on top and a ceiling's under it,
//! glass as a pane up the middle of its wall, the gable over a wall up to a pitched roof,
//! and the roof itself.

use crate::{Base, CELL_M, Fill, IVec3, Kind, STOREY_CUBES, STOREY_M, storey_of, wall_layer};
use glam::Vec3;
use std::collections::BTreeMap;

// Colours are as they should appear on screen (sRGB).
const HULL: [f32; 3] = [0.93, 0.9, 0.82];
const INNER_WALL: [f32; 3] = [0.88, 0.82, 0.72];
const TOP: [f32; 3] = [0.84, 0.81, 0.74];
const ROOF: [f32; 3] = [0.74, 0.73, 0.7];
const UNDERSIDE: [f32; 3] = [0.42, 0.43, 0.47];
const FLOOR: [f32; 3] = [0.6, 0.45, 0.33];
const CEILING: [f32; 3] = [0.93, 0.91, 0.86];
const GLASS: [f32; 3] = [0.55, 0.8, 0.9];

/// How thick a pitched roof is: enough that its edge reads.
pub(crate) const ROOF_THICK_M: f32 = 0.15;
/// How far a pitched roof overhangs its walls, at its eaves and its gable ends (D175),
/// where no kit says otherwise.
const EAVES_M: f32 = 0.5;
/// How finely a pitched roof is sampled, in metres: a quarter of a cell, on which every
/// ridge, hip and valley of a roof over walls in four directions falls, or on its
/// diagonals (research/procedural-buildings.md).
const ROOF_STEP_M: f32 = CELL_M * 0.5;
/// Half the thickness of the boards behind a kit's gable.
const GABLE_BACKING_M: f32 = 0.05;

/// The colours a kit gives what it does not lay itself: its floors, roofs and the boards
/// behind its gables.
#[derive(Clone, Debug, PartialEq)]
pub struct KitLook {
    pub gable: [f32; 3],
    pub floor: [f32; 3],
    pub roof: [f32; 3],
    /// The sides and undersides of its floors, the edge and underside of its roof.
    pub side: [f32; 3],
    /// How far its roofs overhang its walls.
    pub eaves: f32,
    /// The size in metres of each of its materials' textures, for those it has.
    pub textures: BTreeMap<Material, [f32; 2]>,
}

/// What a surface is drawn in: plain colour from each triangle, or one of a kit's
/// materials, which the game gives textures (D14).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Material {
    Plain,
    /// A log's side, and a log's sawn end.
    Log,
    LogEnd,
    /// The daub between logs.
    Daub,
    /// Boards: round openings, under and along a roof's edge.
    Board,
    /// A roof's covering.
    Shakes,
    /// A footing's stone.
    Stone,
    /// A floor's planks.
    Floor,
}

/// Surfaces by what they are drawn in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Surfaces(pub BTreeMap<Material, Surface>);

impl Surfaces {
    pub fn of(&mut self, material: Material) -> &mut Surface {
        self.0.entry(material).or_default()
    }
}

/// Where a point on a flat face falls on a texture so many metres across, mapped from
/// the face's plane, upright on walls.
pub(crate) fn planar(point: Vec3, normal: Vec3, metres: [f32; 2]) -> [f32; 2] {
    let n = normal.abs();
    let (a, b) = if n.y >= n.x && n.y >= n.z {
        (point.x, point.z)
    } else if n.x >= n.z {
        (point.z, -point.y)
    } else {
        (point.x, -point.y)
    };
    [a / metres[0], b / metres[1]]
}

/// Triangles with flat faces: three entries per triangle, no shared corners.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Surface {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub colours: Vec<[f32; 3]>,
    /// Where each corner falls on its texture, if it has one.
    pub uvs: Vec<[f32; 2]>,
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
        self.uvs.extend([[0.0; 2]; 3]);
    }

    /// A triangle with a normal and a place on its texture at each corner.
    pub(crate) fn textured(
        &mut self,
        corners: [Vec3; 3],
        normals: [Vec3; 3],
        uvs: [[f32; 2]; 3],
        colour: [f32; 3],
    ) {
        self.positions.extend(corners);
        self.normals.extend(normals);
        self.colours.extend([colour; 3]);
        self.uvs.extend(uvs);
    }

    /// A flat rectangle, its texture mapped from its plane at so many metres across.
    pub(crate) fn planar_quad(
        &mut self,
        corners: [Vec3; 4],
        normal: Vec3,
        metres: [f32; 2],
        colour: [f32; 3],
    ) {
        let [a, b, c, d] = corners;
        for triangle in [[a, b, c], [a, c, d]] {
            let uvs = triangle.map(|point| planar(point, normal, metres));
            self.textured(triangle, [normal; 3], uvs, colour);
        }
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
        self.uvs.extend([[0.0; 2]; 6]);
    }
}

/// A base ready to draw, in the base's own space, in metres.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BaseMesh {
    /// Walls, floors, roofs and gables, seen from outside and from inside.
    pub structure: Surface,
    /// Window glass, to be drawn see-through and from both sides.
    pub glass: Surface,
    /// What a kit's materials draw: its roofs' covering, edges and undersides.
    pub textured: Surfaces,
}

/// The six faces of a cube: which way each faces, and its corners as seen from there,
/// on a cube from 0 to 1.
const FACES: [([i32; 3], [[f32; 3]; 4]); 6] = [
    (
        [1, 0, 0],
        [[1., 0., 1.], [1., 0., 0.], [1., 1., 0.], [1., 1., 1.]],
    ),
    (
        [-1, 0, 0],
        [[0., 0., 0.], [0., 0., 1.], [0., 1., 1.], [0., 1., 0.]],
    ),
    (
        [0, 1, 0],
        [[0., 1., 1.], [1., 1., 1.], [1., 1., 0.], [0., 1., 0.]],
    ),
    (
        [0, -1, 0],
        [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]],
    ),
    (
        [0, 0, 1],
        [[0., 0., 1.], [1., 0., 1.], [1., 1., 1.], [0., 1., 1.]],
    ),
    (
        [0, 0, -1],
        [[1., 0., 0.], [0., 0., 0.], [0., 1., 0.], [1., 1., 0.]],
    ),
];

pub(crate) fn build(base: &Base, kit: impl Fn(Kind) -> Option<KitLook>) -> BaseMesh {
    let mut mesh = BaseMesh::default();
    // What hides a face: a cube drawn as a box beside it.
    let boxed = |at: IVec3| {
        base.cube(at).is_some_and(|cube| {
            cube.fill == Fill::Solid && (!base.in_wall(at) || kit(cube.kind).is_none())
        })
    };
    for (at, cube) in base.cubes() {
        let wall = base.in_wall(at);
        let look = kit(cube.kind);
        let tile = IVec3::new(at.x, storey_of(at.y), at.z);
        match cube.fill {
            Fill::Open => continue,
            Fill::Glass => {
                pane(&mut mesh, base, at);
                continue;
            }
            Fill::Solid if wall && look.is_some() => continue,
            Fill::Solid => {}
        }
        let low = at.as_vec3() * CELL_M;
        for (out, corners) in FACES {
            let out = IVec3::from_array(out);
            if boxed(at + out) {
                continue;
            }
            let colour = if wall {
                match out.y {
                    1 => TOP,
                    -1 => HULL,
                    _ if base.contains(tile + out) => INNER_WALL,
                    _ => HULL,
                }
            } else {
                // A floor or flat roof: its top a floor where a room is over it.
                let above = IVec3::new(at.x, storey_of(at.y + 1), at.z);
                match (out.y, &look) {
                    (1, Some(look)) if base.contains(above) => look.floor,
                    (1, Some(look)) => look.roof,
                    (1, None) if base.contains(above) => FLOOR,
                    (1, None) => ROOF,
                    (-1, _) if base.contains(above - IVec3::Y) => CEILING,
                    (_, Some(look)) => look.side,
                    _ => UNDERSIDE,
                }
            };
            let corners = corners.map(|c| low + Vec3::from_array(c) * CELL_M);
            mesh.structure.quad(corners, out.as_vec3(), colour);
        }
    }
    for (tile, _) in base.walls() {
        let look = base.kind(tile).and_then(&kit);
        gable(&mut mesh, base, tile, look);
    }
    pitched_roofs(&mut mesh, base, &kit);
    mesh
}

/// A pane of glass up the middle of a cube of a wall, across the way the wall runs.
fn pane(mesh: &mut BaseMesh, base: &Base, at: IVec3) {
    let tile = IVec3::new(at.x, storey_of(at.y), at.z);
    let [low_left, low_right, ..] = crate::corners(tile, base.along(tile));
    let bottom = Vec3::Y * ((at.y - wall_layer(tile.y)) as f32 * CELL_M);
    let up = Vec3::Y * CELL_M;
    let normal = (low_right - low_left).cross(Vec3::Y).normalize();
    mesh.glass.quad(
        [
            low_left + bottom,
            low_right + bottom,
            low_right + bottom + up,
            low_left + bottom + up,
        ],
        normal,
        GLASS,
    );
}

/// The wall carried up to a pitched roof over it, as far as the roof's underside, so a
/// gable end is closed and a wall inside meets the slope: as the wall's own faces for a
/// plain wall, as boards up its middle behind a kit's own gable. In two pieces along the
/// tile, so a ridge over its middle is followed.
fn gable(mesh: &mut BaseMesh, base: &Base, tile: IVec3, look: Option<KitLook>) {
    let top_layer = wall_layer(tile.y) + STOREY_CUBES - 1;
    if base.cube(IVec3::new(tile.x, top_layer, tile.z)).is_none() {
        return;
    }
    let [origin, right_end, ..] = crate::corners(tile, base.along(tile));
    let right = right_end - origin;
    let normal = right.normalize().cross(Vec3::Y);
    let rise = |across: f32| {
        let at = origin + right * across;
        base.roof_rise(tile.y, glam::Vec2::new(at.x, at.z))
            .map(|rise| (rise - ROOF_THICK_M).max(0.0))
    };
    let (Some(start), Some(middle), Some(end)) = (rise(0.0), rise(0.5), rise(1.0)) else {
        return;
    };
    let (depth, front, back) = match look {
        Some(look) => (GABLE_BACKING_M, look.gable, look.gable),
        None => {
            let side = |out: Vec3| {
                if base.contains(tile + out.round().as_ivec3()) {
                    INNER_WALL
                } else {
                    HULL
                }
            };
            (CELL_M * 0.5, side(normal), side(-normal))
        }
    };
    let height = origin.y + STOREY_M;
    let at = |across: f32, up: f32, out: f32| (origin + right * across).with_y(up) + normal * out;
    for (left, right, rise_left, rise_right) in [(0.0, 0.5, start, middle), (0.5, 1.0, middle, end)]
    {
        if rise_left.max(rise_right) < 0.01 {
            continue;
        }
        let (top_left, top_right) = (height + rise_left, height + rise_right);
        mesh.structure.polygon(
            &[
                at(left, height, depth),
                at(right, height, depth),
                at(right, top_right, depth),
                at(left, top_left, depth),
            ],
            normal,
            front,
        );
        mesh.structure.polygon(
            &[
                at(right, height, -depth),
                at(left, height, -depth),
                at(left, top_left, -depth),
                at(right, top_right, -depth),
            ],
            -normal,
            back,
        );
    }
}

/// How a square of roof is drawn: its top's colour and its underside's, and the sizes of its
/// covering's and its boards' textures, where the kit has them.
type SquareLook = ([f32; 3], [f32; 3], Option<[f32; 2]>, Option<[f32; 2]>);

/// Every pitched roof, as its height above the walls sampled on a fine lattice: each small
/// square in four triangles from its middle, so the faces are as flat as the roof. It
/// overhangs its walls by its eaves, carried on down at its pitch, and its edge shows its
/// thickness. Its top is roof, in the kit's colour of the walls under it, where a kit's
/// covering does not lie over it; its edge and underside the kit's dark wood.
fn pitched_roofs(mesh: &mut BaseMesh, base: &Base, kit: &impl Fn(Kind) -> Option<KitLook>) {
    let per = (CELL_M / ROOF_STEP_M).round() as i32;
    let look_of =
        |section: &crate::RoofSection| base.kind(IVec3::from_array(section.min)).and_then(kit);
    let eaves_of = |section: &crate::RoofSection| look_of(section).map_or(EAVES_M, |l| l.eaves);
    let mut squares: BTreeMap<[i32; 3], SquareLook> = BTreeMap::new();
    for section in base.sections() {
        let look = look_of(section);
        let (top, under) = look
            .as_ref()
            .map_or((ROOF, CEILING), |look| (look.roof, look.side));
        let textures = look.map(|look| look.textures).unwrap_or_default();
        let colours = (
            top,
            under,
            textures.get(&Material::Shakes).copied(),
            textures.get(&Material::Board).copied(),
        );
        let out = (eaves_of(section) / ROOF_STEP_M).round() as i32;
        for x in section.min[0] * per - out..(section.max[0] + 1) * per + out {
            for z in section.min[2] * per - out..(section.max[2] + 1) * per + out {
                squares.entry([x, section.storey(), z]).or_insert(colours);
            }
        }
    }
    let height = |storey: i32, at: glam::Vec2| {
        base.sections()
            .iter()
            .filter(|section| section.storey() == storey)
            .filter_map(|section| section.rise_beyond(at, eaves_of(section)))
            .max_by(|a, b| a.total_cmp(b))
    };
    let down = Vec3::Y * ROOF_THICK_M;
    for (&[x, storey, z], &(top, under, covering, boards)) in &squares {
        let eaves = (storey + 1) as f32 * STOREY_M;
        let at = |dx: f32, dz: f32| {
            let along =
                glam::Vec2::new((x as f32 + dx) * ROOF_STEP_M, (z as f32 + dz) * ROOF_STEP_M);
            let rise = height(storey, along).unwrap_or(0.0);
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
            match covering {
                // Its covering's rows along the slope's level, their lower edges down it.
                Some(metres) => {
                    let level = Vec3::Y.cross(normal).normalize_or(Vec3::X);
                    let up = normal.cross(level);
                    let uv = |p: Vec3| [p.dot(level) / metres[0], -p.dot(up) / metres[1]];
                    mesh.textured.of(Material::Shakes).textured(
                        [middle, a, b],
                        [normal; 3],
                        [uv(middle), uv(a), uv(b)],
                        [1.0; 3],
                    );
                }
                None => mesh.structure.triangle([middle, a, b], normal, top),
            }
            let under_side = [middle - down, b - down, a - down];
            match boards {
                Some(metres) => {
                    let uvs = under_side.map(|p| planar(p, -normal, metres));
                    mesh.textured.of(Material::Board).textured(
                        under_side,
                        [-normal; 3],
                        uvs,
                        [1.0; 3],
                    );
                }
                None => mesh.structure.triangle(under_side, -normal, under),
            }
        }
        // Its edge, where no square of roof carries on.
        for (step, (a, b)) in [
            ([0, -1], (corners[1], corners[0])),
            ([1, 0], (corners[2], corners[1])),
            ([0, 1], (corners[3], corners[2])),
            ([-1, 0], (corners[0], corners[3])),
        ] {
            if squares.contains_key(&[x + step[0], storey, z + step[1]]) {
                continue;
            }
            let normal = Vec3::new(step[0] as f32, 0.0, step[1] as f32);
            match boards {
                Some(metres) => mesh.textured.of(Material::Board).planar_quad(
                    [a - down, b - down, b, a],
                    normal,
                    metres,
                    [1.0; 3],
                ),
                None => mesh
                    .structure
                    .quad([a - down, b - down, b, a], normal, under),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BuildOp, Panel, RoofSection, RoofStyle};

    fn room() -> Base {
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [5, 0, 4],
            kind: Kind::PLAIN,
        });
        base.apply(BuildOp::SetWall {
            tile: [2, 0, 4],
            panel: Some(Panel::WINDOW),
            kind: Kind::PLAIN,
        });
        base.apply(BuildOp::AddRoof(RoofSection {
            min: [0, 0, 0],
            max: [5, 0, 4],
            style: RoofStyle::Gable,
            along_x: true,
            pitch: 0.7,
        }));
        base
    }

    #[test]
    fn every_triangle_faces_the_way_its_normal_says() {
        let mesh = room().mesh(|_| None);
        assert!(mesh.structure.triangle_count() > 0 && mesh.glass.triangle_count() > 0);
        for surface in [&mesh.structure, &mesh.glass] {
            for (corners, normal) in surface.positions.chunks(3).zip(surface.normals.chunks(3)) {
                let own = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
                assert!(own.dot(normal[0]) > 0.0, "{corners:?} {normal:?}");
            }
        }
    }

    #[test]
    fn drawing_the_same_base_twice_gives_the_same_mesh() {
        assert_eq!(room().mesh(|_| None), room().mesh(|_| None));
    }

    #[test]
    fn faces_between_two_cubes_are_not_drawn() {
        let mut base = Base::default();
        base.apply(BuildOp::Fill {
            min: [0, 0, 0],
            max: [1, 0, 0],
            cube: Some(crate::Cube {
                kind: Kind::PLAIN,
                fill: Fill::Solid,
            }),
        });
        // Two cubes side by side: ten faces, not twelve.
        assert_eq!(base.mesh(|_| None).structure.triangle_count(), 20);
    }

    #[test]
    fn walls_with_a_kit_are_left_to_it_and_their_roof_takes_its_colour() {
        let look = KitLook {
            gable: [0.0; 3],
            floor: [0.0; 3],
            roof: [1.0, 0.0, 0.0],
            side: [0.0; 3],
            eaves: 0.5,
            textures: BTreeMap::new(),
        };
        let plain = room().mesh(|_| None);
        let kitted = room().mesh(|_| Some(look.clone()));
        assert!(kitted.structure.triangle_count() < plain.structure.triangle_count());
        assert!(kitted.structure.colours.contains(&[1.0, 0.0, 0.0]));
    }
}
