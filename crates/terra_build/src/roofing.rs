//! Roofs drawn from their pieces (`layout::roofs`), on the one material system (D251, D258,
//! docs/building-plan.md#one-material-system-d251-d258): the covering is the roof cubes'
//! sloped surface in the covering's material, and every line the roof's neighbours make
//! (ridges, hips, valleys, eaves, verges, flashing) and its frame (rafters, beams, battens)
//! are the kit's hand-made modules laid along them (`modules`). Pure data, so it runs
//! anywhere.
//!
//! **The covering.** Each half-cube square of the roof is lifted off the roof by the
//! covering's thickness and pushed out by its material's height, read across each row of
//! its texture (`Relief::row`), so the courses of shakes or slates the texture shows stand
//! out as steps and thatch swells and dips as its texture does. Its texture's rows are laid
//! along level lines a course's rise apart from the roof's eaves up (the kit's `rows` and
//! `phase`), so courses run on round hips and through valleys, each shifted along so no two
//! repeat; the square is cut along those lines so each step falls on its own. Where the
//! covering stops it is closed down to the roof, plumb along an eave, so thatch shows its
//! full thickness cut clean, and square to the roof along a verge, where the verge's module
//! wraps it.

use crate::kit::{Kit, Part};
use crate::layout::roofs::{RoofPart, planar_normal};
use crate::layout::{Piece, Place, hash, metres, unit};
use crate::mesh::{Material, Surface, Surfaces, planar};
use crate::modules::{self, Line, Look, Seat};
use crate::relief::Relief;
use glam::Vec3;

/// The size in metres of a kit's texture for a material, or a metre square if it has none.
fn size(kit: &Kit, material: Material) -> [f32; 2] {
    kit.textures
        .get(&material)
        .map_or([1.0, 1.0], |texture| texture.metres)
}

/// Draws a piece of roof, each part in its material; `face` is its covering's height map,
/// once it has been read.
pub fn draw(piece: &Piece, kit: &Kit, face: Option<&Relief>, surfaces: &mut Surfaces) {
    draw_as(piece, kit, face, false, surfaces);
}

/// Draws a piece of roof as [`draw`] does, or, `plain`, as it is seen far off or as a
/// ghost: its covering a sheet over the roof, its modules at their far budget.
pub fn draw_as(
    piece: &Piece,
    kit: &Kit,
    face: Option<&Relief>,
    plain: bool,
    surfaces: &mut Surfaces,
) {
    match *piece {
        Piece::Covering {
            corners,
            middle,
            courses,
            open,
            ..
        } => {
            let square = Square {
                corners: corners.map(Place::metres),
                middle: middle.metres(),
                base: metres(courses.0),
                rise: metres(courses.1).max(0.01),
                open,
            };
            if plain {
                square.sheet(kit, surfaces.of(Material::Covering));
            } else {
                square.cover(kit, face, surfaces);
            }
        }
        Piece::Roof {
            part,
            from,
            to,
            sides,
            seed,
            ..
        } => {
            let Some(spec) = kit.roof.parts.get(&part) else {
                return;
            };
            let Some(module) = kit
                .dress
                .made
                .as_deref()
                .and_then(|pieces| pieces.modules.get(&spec.module))
            else {
                return;
            };
            let (from, to) = (from.metres(), to.metres());
            let line = Line {
                from,
                to,
                seat: seat(part, spec.on_roof, from, to, sides.map(Place::metres)),
                across: None,
                capped: [true; 2],
                seed,
                blend: module.blend,
            };
            modules::lay(
                module,
                &line,
                &look(kit, spec),
                plain || kit.dress.far,
                surfaces,
            );
        }
        _ => {}
    }
}

/// How a part's module looks: its material and its ends', their textures' sizes, its tint.
fn look(kit: &Kit, part: &Part) -> Look {
    Look {
        material: part.material,
        ends: part.ends,
        metres: size(kit, part.material),
        end_metres: part.ends.map_or([1.0, 1.0], |m| size(kit, m)),
        tint: part.tint,
        shade: 0.14,
    }
}

/// How a part of a roof sits on its line, `sides` a point on the roof either side of it,
/// or for an edge, on the roof and beyond.
fn seat(part: RoofPart, on_roof: bool, from: Vec3, to: Vec3, sides: [Vec3; 2]) -> Seat {
    let normals = sides.map(|side| planar_normal(from, to, side));
    let into = sides.map(|side| across(from, to, side));
    match part {
        // Draped over both slopes.
        RoofPart::Ridge | RoofPart::Hip | RoofPart::Valley => Seat::Crease { up: normals, into },
        // On the roof, out past its edge, or toward the wall it meets.
        RoofPart::Verge | RoofPart::Flashing => Seat::Plane {
            up: normals[0],
            out: -into[0],
        },
        RoofPart::Eave if on_roof => Seat::Plane {
            up: normals[0],
            out: -into[0],
        },
        // Plumb, out level past the eave, as a fascia hangs.
        RoofPart::Eave => Seat::Plane {
            up: Vec3::Y,
            out: -into[0].with_y(0.0).normalize_or(Vec3::X),
        },
        // Across the slope toward the side of the course over it.
        RoofPart::Batten => Seat::Plane {
            up: normals[0],
            out: into[0],
        },
        // Along its middle, square to the roof over it.
        RoofPart::Rafter => {
            let up = (normals[0] + normals[1]).normalize_or(Vec3::Y);
            let along = (to - from).normalize_or(Vec3::X);
            Seat::Plane {
                up,
                out: up.cross(along).normalize_or(Vec3::Z),
            }
        }
    }
}

/// The way across a line on the roof toward a point on one side of it, in the plane of the
/// line and the point, square to the line.
fn across(from: Vec3, to: Vec3, side: Vec3) -> Vec3 {
    let along = (to - from).normalize_or(Vec3::X);
    (side - from).reject_from(along).normalize_or(Vec3::X)
}

/// A square of roof: its corners and middle on the roof's top, and its courses.
struct Square {
    corners: [Vec3; 4],
    middle: Vec3,
    /// The first course's lower edge, and each course's rise, in metres.
    base: f32,
    rise: f32,
    open: [bool; 4],
}

/// A flat part of a square, wound counter-clockwise from above, and the square's sides it
/// lies along: the whole square where it is flat, else each of its four triangles.
type Facet = (Vec<Vec3>, Vec<usize>);

/// Where each course is cut with no height map to say, as fractions of it up from its lower
/// edge: at its edge, just over it and through its middle.
const CUTS: [f32; 3] = [0.0, 0.07, 0.45];
/// How tall a course must rise to be cut where it is lowest as well as highest, in metres.
const SWELL_M: f32 = 0.15;

/// Where each course is cut, as fractions of it up from its lower edge: at its edge, and
/// where its texture's rows are lowest and highest, so the step between them, each butt
/// standing out over the course under it, falls on its own and the swell between is followed
/// (`Relief::row`).
fn cuts(relief: Option<&Relief>, phase: f32, rows: f32, rise: f32) -> Vec<f32> {
    // A short course, a slate's, at its edge and its highest only.
    let short = rise < SWELL_M;
    let Some(relief) = relief else {
        return if short {
            vec![0.0, 0.45]
        } else {
            CUTS.to_vec()
        };
    };
    let at = |f: f32| relief.row(phase - f / rows);
    let samples: Vec<(f32, f32)> = (1..16)
        .map(|k| k as f32 / 16.0)
        .map(|f| (f, at(f)))
        .collect();
    let low = samples.iter().copied().min_by(|a, b| a.1.total_cmp(&b.1));
    let high = samples.iter().copied().max_by(|a, b| a.1.total_cmp(&b.1));
    let mut out = vec![0.0];
    let wanted = if short { vec![high] } else { vec![low, high] };
    for (f, _) in wanted.into_iter().flatten() {
        if out.iter().all(|&g: &f32| (g - f).abs() > 0.05) {
            out.push(f);
        }
    }
    out.sort_by(f32::total_cmp);
    out
}

impl Square {
    fn facets(&self) -> Vec<Facet> {
        let triangles: Vec<Facet> = (0..4)
            .map(|side| {
                let (a, b) = (self.corners[side], self.corners[(side + 1) % 4]);
                (vec![self.middle, b, a], vec![side])
            })
            .collect();
        let n = up(&triangles[0].0);
        if triangles.iter().all(|(t, _)| up(t).dot(n) > 1.0 - 1e-5) {
            let mut quad = self.corners.to_vec();
            if (quad[1] - quad[0]).cross(quad[2] - quad[0]).y < 0.0 {
                quad.reverse();
            }
            return vec![(quad, vec![0, 1, 2, 3])];
        }
        triangles
    }

    /// How far up its courses a height is, in courses from the first's lower edge.
    fn at(&self, y: f32) -> f32 {
        (y - self.base) / self.rise
    }

    /// The covering as a flat sheet over the roof, for far off and a ghost.
    fn sheet(&self, kit: &Kit, out: &mut Surface) {
        let r = &kit.roof;
        let texture = size(kit, Material::Covering);
        for (polygon, _) in self.facets() {
            let n = up(&polygon);
            let lifted: Vec<Vec3> = polygon
                .iter()
                .map(|p| *p + n * (r.thick + r.relief * 0.5))
                .collect();
            for i in 1..lifted.len() - 1 {
                let corners = [lifted[0], lifted[i], lifted[i + 1]];
                out.textured(
                    corners,
                    [n; 3],
                    corners.map(|p| planar(p, n, texture)),
                    r.tint,
                );
            }
        }
    }

    /// The covering: the square lifted by the covering's thickness and its relief, its
    /// edges closed down to the roof where no covering carries on, its underside boarded.
    fn cover(&self, kit: &Kit, face: Option<&Relief>, surfaces: &mut Surfaces) {
        let r = &kit.roof;
        let texture = size(kit, Material::Covering);
        let rows = r.rows.max(0.1);
        // The texture's row at a height, and how far the covering stands out there.
        let v = |y: f32| r.phase - self.at(y) / rows;
        let height = |y: f32| face.map_or(0.5, |relief| relief.row(v(y)));
        let lift = |y: f32| r.thick + r.relief * height(y);
        let cuts = cuts(face, r.phase, rows, self.rise);
        for (polygon, sides) in self.facets() {
            let n = up(&polygon);
            let level = Vec3::Y.cross(n).normalize_or(Vec3::X);
            let upslope = n.cross(level);
            // How far the surface leans by the relief's slope up the roof.
            let lean = |y: f32| {
                if upslope.y < 1e-3 || face.is_none() {
                    return 0.0;
                }
                let dy = self.rise * 0.02;
                (lift(y + dy) - lift(y - dy)) / (2.0 * dy / upslope.y)
            };
            // The square's sides no covering carries on past: its level ones plumb.
            let edges: Vec<(Vec3, Vec3, bool)> = sides
                .iter()
                .filter(|&&side| self.open[side])
                .map(|&side| {
                    let (a, b) = (self.corners[side], self.corners[(side + 1) % 4]);
                    (a, b, (a.y - b.y).abs() < 1e-4 && n.y < 1.0 - 1e-4)
                })
                .collect();
            let on = |x: Vec3, (a, b, _): &(Vec3, Vec3, bool)| {
                (x - *a).cross(*b - *a).length() < 1e-4 * (*b - *a).length().max(1e-3)
            };
            // A point lifted onto the covering's top: plumb over it along an eave.
            let top = |p: Vec3| {
                let off = lift(p.y);
                if edges.iter().any(|e| e.2 && on(p, e)) {
                    p + Vec3::Y * (off / n.y.max(0.2))
                } else {
                    p + n * off
                }
            };
            let normal = |p: Vec3| (n - upslope * lean(p.y)).normalize_or(n);
            let colour = r.tint;
            let covering = surfaces.of(Material::Covering);
            for band in slices(&polygon, self, &cuts) {
                let course = libm::floorf(
                    self.at(band.iter().map(|p| p.y).sum::<f32>() / band.len() as f32),
                ) as i32;
                let shift = unit(hash(&[course as u32, 77])) * 7.0;
                let uv = |p: Vec3| [p.dot(level) / texture[0] + shift, v(p.y)];
                let placed: Vec<(Vec3, Vec3, [f32; 2])> =
                    band.iter().map(|&p| (top(p), normal(p), uv(p))).collect();
                for i in 1..placed.len().saturating_sub(1) {
                    let [a, b, c] = [placed[0], placed[i], placed[i + 1]];
                    let face = (b.0 - a.0).cross(c.0 - a.0);
                    if face.length() < 1e-9 {
                        continue;
                    }
                    let (b, c) = if face.dot(n) >= 0.0 { (b, c) } else { (c, b) };
                    covering.textured([a.0, b.0, c.0], [a.1, b.1, c.1], [a.2, b.2, c.2], colour);
                }
                // Where no covering carries on past a side, its edge, down to the roof.
                for edge in &edges {
                    let middle = polygon.iter().sum::<Vec3>() / polygon.len() as f32;
                    let outward = (edge.0 - middle)
                        .reject_from(edge.1 - edge.0)
                        .normalize_or(Vec3::X);
                    for (i, &p) in band.iter().enumerate() {
                        let q = band[(i + 1) % band.len()];
                        if !on(p, edge) || !on(q, edge) || p.distance(q) < 1e-5 {
                            continue;
                        }
                        let quad = [p, q, placed[(i + 1) % placed.len()].0, placed[i].0];
                        side(covering, quad, outward, texture, colour);
                    }
                }
            }
            // Its underside, boarded, seen from inside.
            let board = size(kit, Material::Board);
            let under = surfaces.of(Material::Board);
            for i in 1..polygon.len() - 1 {
                let corners = [polygon[0], polygon[i + 1], polygon[i]];
                under.textured(
                    corners,
                    [-n; 3],
                    corners.map(|p| planar(p, -n, board)),
                    [0.8; 3],
                );
            }
        }
    }
}

/// A flat polygon's normal, facing up.
fn up(polygon: &[Vec3]) -> Vec3 {
    let n = (polygon[1] - polygon[0])
        .cross(polygon[2] - polygon[0])
        .normalize_or(Vec3::Y);
    if n.y < 0.0 { -n } else { n }
}

/// A convex polygon split by a level plane: what is under it and what is over it.
fn split(polygon: &[Vec3], height: f32) -> (Vec<Vec3>, Vec<Vec3>) {
    let (mut under, mut over) = (Vec::new(), Vec::new());
    for i in 0..polygon.len() {
        let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
        if a.y <= height {
            under.push(a);
        }
        if a.y >= height {
            over.push(a);
        }
        if (a.y - height) * (b.y - height) < 0.0 {
            let s = (height - a.y) / (b.y - a.y);
            let p = (a + (b - a) * s).with_y(height);
            under.push(p);
            over.push(p);
        }
    }
    (under, over)
}

/// A flat polygon cut along each course's cuts ([`cuts`]) into the bands between them.
fn slices(polygon: &[Vec3], square: &Square, cuts: &[f32]) -> Vec<Vec<Vec3>> {
    let low = polygon.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    let high = polygon.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    if high - low < 1e-4 {
        return vec![polygon.to_vec()];
    }
    let first = libm::floorf(square.at(low)) as i32;
    let last = libm::ceilf(square.at(high)) as i32;
    let mut out = Vec::new();
    let mut rest = polygon.to_vec();
    for course in first..=last {
        for &f in cuts {
            let height = square.base + (course as f32 + f) * square.rise;
            if height <= low + 1e-5 || height >= high - 1e-5 {
                continue;
            }
            let (under, over) = split(&rest, height);
            if under.len() >= 3 {
                out.push(under);
            }
            rest = over;
            if rest.len() < 3 {
                return out;
            }
        }
    }
    out.push(rest);
    out
}

/// A side, two triangles from four corners in order, each wound to look the outward way
/// and lit flat, its texture mapped from its plane: a cut edge's corners need not lie in one
/// plane where one of them is cut plumb.
fn side(out: &mut Surface, quad: [Vec3; 4], outward: Vec3, metres: [f32; 2], colour: [f32; 3]) {
    for [i, j, k] in [[0, 1, 2], [0, 2, 3]] {
        let corners = [quad[i], quad[j], quad[k]];
        let face = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
        if face.length() < 1e-9 {
            continue;
        }
        let (corners, normal) = if face.dot(outward) >= 0.0 {
            (corners, face.normalize())
        } else {
            ([corners[0], corners[2], corners[1]], -face.normalize())
        };
        out.textured(
            corners,
            [normal; 3],
            corners.map(|p| planar(p, normal, metres)),
            colour,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::roofs;
    use crate::roofer::{self, Style};
    use crate::{Base, BuildOp, Cube, Fill, Kind, STOREY_M};

    /// A shipped roof kit, its pieces read, numbered as the first roof kind.
    fn shipped(name: &str) -> Kit {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let text = std::fs::read_to_string(assets.join("data/kits").join(name)).unwrap();
        let mut kit: Kit = ron::from_str(&text).unwrap();
        kit.dress
            .read(&|file| std::fs::read_to_string(assets.join(file)).map_err(|e| e.to_string()))
            .unwrap();
        kit.id = Kind::FIRST_ROOF.0;
        kit
    }

    /// Five metres by four of plain walls under a roof of the first roof kind.
    fn cabin(style: Style) -> Base {
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [9, 0, 7],
            kind: Kind::PLAIN,
        });
        let footprint = (0..10).flat_map(|x| (0..8).map(move |z| [x, z])).collect();
        for (at, shape) in roofer::over(&footprint, 6, style, None).roof {
            base.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: Some(Cube {
                    shape,
                    ..Cube::new(Kind::FIRST_ROOF, Fill::Solid)
                }),
            });
        }
        base
    }

    fn drawn(kit: &Kit, base: &Base, only: impl Fn(&Piece) -> bool) -> Surfaces {
        let mut pieces = Vec::new();
        roofs::lay(
            base,
            kit,
            &crate::kit::Kits(vec![kit.clone()]),
            crate::layout::Scope::ALL,
            &mut pieces,
        );
        let mut surfaces = Surfaces::default();
        for piece in pieces.iter().filter(|p| only(p)) {
            draw(piece, kit, None, &mut surfaces);
        }
        surfaces
    }

    #[test]
    fn every_part_of_a_shipped_roof_is_drawn_from_its_pieces_facing_out() {
        for name in ["shakes.ron", "slate.ron", "thatch.ron"] {
            let kit = shipped(name);
            for style in [Style::Gable, Style::Hip] {
                let surfaces = drawn(&kit, &cabin(style), |_| true);
                for (material, surface) in &surfaces.0 {
                    assert!(
                        surface.triangle_count() > 0,
                        "{name} {style:?} {material:?}"
                    );
                    for (corners, normals) in
                        surface.positions.chunks(3).zip(surface.normals.chunks(3))
                    {
                        // Wound the way its corners' normals look together: a modelled
                        // piece is shaded smooth across small creases, so one corner may
                        // lean past its face.
                        let own = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
                        let together = normals[0] + normals[1] + normals[2];
                        assert!(
                            normals.iter().all(|n| n.is_finite()) && own.dot(together) >= 0.0,
                            "{name} {style:?} {material:?}: {corners:?} against {normals:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_ridge_capping_sits_down_on_the_covering_over_the_ridge() {
        // A gable along X, its ridge two metres up over z = 2: the capping covers the crease,
        // its underside down on the covering either side, never standing clear of it with
        // the sky under it.
        for name in ["shakes.ron", "slate.ron", "thatch.ron"] {
            let kit = shipped(name);
            let ridge = |p: &Piece| {
                matches!(
                    p,
                    Piece::Roof {
                        part: RoofPart::Ridge,
                        ..
                    }
                )
            };
            let surfaces = drawn(&kit, &cabin(Style::Gable), ridge);
            let points: Vec<Vec3> = surfaces
                .0
                .values()
                .flat_map(|s| s.positions.clone())
                .collect();
            assert!(!points.is_empty(), "{name}");
            let top = STOREY_M + 2.0;
            let r = &kit.roof;
            // How far over the roof's top a point is, square to the slope it is over.
            let over =
                |p: &Vec3| (p.y - (top - (p.z - 2.0).abs())) * std::f32::consts::FRAC_1_SQRT_2;
            // Its lowest point within the slopes stays within the covering's thickness.
            let lowest = points
                .iter()
                .filter(|p| p.x > 0.0 && p.x < 5.0)
                .map(over)
                .fold(f32::MAX, f32::min);
            assert!(lowest < r.thick + r.relief + 0.01, "{name}: {lowest}");
            // And it stands over the crease, above the covering there.
            let crest = points
                .iter()
                .filter(|p| (p.z - 2.0).abs() < 0.05)
                .map(|p| p.y)
                .fold(f32::MIN, f32::max);
            assert!(
                crest > top + r.thick * std::f32::consts::SQRT_2,
                "{name}: {crest}"
            );
        }
    }

    #[test]
    fn an_eave_is_cut_plumb_through_the_coverings_thickness() {
        let kit = shipped("thatch.ron");
        let covering = |p: &Piece| matches!(p, Piece::Covering { .. });
        let surfaces = drawn(&kit, &cabin(Style::Gable), covering);
        let surface = &surfaces.0[&Material::Covering];
        // The eaves overhang half a metre, to z = -0.5 and 4.5: their cut faces look level,
        // out from the roof, and are as tall as the covering is thick and more.
        let mut found = 0;
        for (corners, normals) in surface.positions.chunks(3).zip(surface.normals.chunks(3)) {
            let at_eave = corners
                .iter()
                .all(|p| (p.z + 0.5).abs() < 0.01 || (p.z - 4.5).abs() < 0.01);
            if !at_eave {
                continue;
            }
            found += 1;
            assert!(normals[0].y.abs() < 1e-3, "{corners:?} {normals:?}");
        }
        assert!(found > 0);
        let tall = surface
            .positions
            .iter()
            .filter(|p| (p.z + 0.5).abs() < 0.01 && p.x > 1.0 && p.x < 4.0)
            .map(|p| p.y)
            .fold((f32::MAX, f32::MIN), |(a, b), y| (a.min(y), b.max(y)));
        assert!(tall.1 - tall.0 >= kit.roof.thick, "{tall:?}");
    }

    #[test]
    fn a_courses_butt_stands_out_by_its_textures_rows() {
        // One square on a slope rising north, its courses 20 cm of rise apart, its texture's
        // rows high in its top half and low in its bottom half.
        let square = Square {
            corners: [
                Vec3::new(0.0, 0.25, 0.0),
                Vec3::new(0.25, 0.25, 0.0),
                Vec3::new(0.25, 0.0, 0.25),
                Vec3::new(0.0, 0.0, 0.25),
            ],
            middle: Vec3::new(0.125, 0.125, 0.125),
            base: 0.0,
            rise: 0.2,
            open: [false; 4],
        };
        let kit = Kit {
            roof: crate::kit::Roofing {
                rows: 1.0,
                phase: 0.0,
                thick: 0.02,
                relief: 0.03,
                ..Default::default()
            },
            ..Kit::default()
        };
        let rows: Vec<u8> = (0..64)
            .flat_map(|j| [if j < 32 { 255 } else { 0 }; 4])
            .collect();
        let relief = Relief::new(4, 64, rows).unwrap();
        let mut surfaces = Surfaces::default();
        square.cover(&kit, Some(&relief), &mut surfaces);
        let covering = &surfaces.0[&Material::Covering];
        // Every point over the roof by its thickness and at most its relief more, and both
        // the thinnest and the thickest found.
        let (low, high) = covering
            .positions
            .iter()
            .map(|p| (p.y - (0.25 - p.z)) * std::f32::consts::FRAC_1_SQRT_2)
            .fold((f32::MAX, f32::MIN), |(a, b), o| (a.min(o), b.max(o)));
        assert!(low > 0.02 - 1e-3 && high < 0.05 + 1e-3, "{low} {high}");
        assert!(low < 0.025 && high > 0.045, "{low} {high}");
    }
}
