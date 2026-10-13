//! Roofs (docs/building-plan.md, docs/building-kits.md): a base's pitched roofs laid as
//! pieces, from the height field their sections make over each storey.
//!
//! **The field.** A storey's roof is, at each point, the highest of the sections over it
//! (`RoofSection::rise`), so a gable meeting a hip makes its own valley and L, T and U plans
//! need nothing more. Each section is carried on down past its edges by its kit's eaves,
//! but only where no other section is: a wing's eaves stop where another wing's roof begins,
//! so a ridge dies into the roof it meets instead of standing out of it.
//!
//! **The lattice.** The field is sampled every half cube, each square in four triangles
//! from its middle; every ridge, hip and valley over walls on the grid falls on a square's
//! side or diagonal, so the triangles are as flat as the roof (research/procedural-
//! buildings.md). Where two triangles meet at an angle is a crease: a ridge (level and
//! convex), a hip (sloping and convex) or a valley (concave). Where the roof stops is an
//! edge: an eave (level), a verge (sloping), or flashing where a wall stands up through it.
//!
//! **The pieces**, in the order a roof goes up (`Roof::stages`): its frame (rafters up the
//! slopes at every cube, a beam under every ridge, hip and valley), its battens and its
//! covering a square at a time, then the boards along its eaves and verges, the caps over
//! its ridges and hips, its valleys' linings and the flashing against walls. Each is placed
//! in the grid (D208) and knows the roof cell whose stage it waits for, so a server can lay
//! them without any art.

use super::{Piece, Place, Scope, hash, ticks};
use crate::kit::{Kit, Kits};
use crate::mesh::{QuickMap, ROOF_THICK_M};
use crate::{Base, CELL_M, Fill, Kind, RoofId, RoofSection, STOREY_CUBES, STOREY_M};
use glam::{IVec3, Vec2, Vec3};
use std::collections::BTreeMap;

/// How finely a roof is sampled, in metres: half a cube.
pub const STEP_M: f32 = CELL_M * 0.5;
/// How far a roof overhangs its walls where no kit says (D175).
pub const EAVES_M: f32 = 0.5;

/// Which roof cell a piece of roof goes up with, and in which of its stages: 1 its frame,
/// 2 its battens, 3 its covering and trims.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoofCell {
    pub roof: RoofId,
    /// Counted in cubes east and south, within the roof's section.
    pub cell: [i32; 2],
    pub stage: u8,
}

/// What a long piece of roof is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RoofPart {
    /// The frame: a rafter up a slope, or the beam under a ridge, hip or valley.
    Rafter,
    /// A batten across the rafters under the foot of a course.
    Batten,
    /// The board along an eave, over the rafters' ends: the fascia.
    Fascia,
    /// The board along a verge, a gable's sloping edge: the bargeboard.
    Verge,
    /// The cap over a ridge.
    Ridge,
    /// The cap over a hip.
    Hip,
    /// The lining of a valley.
    Valley,
    /// Mortar where the roof meets a wall standing up through it.
    Flashing,
}

impl RoofPart {
    /// The stage of its roof cell it goes up in.
    pub fn stage(self) -> u8 {
        match self {
            RoofPart::Rafter => 1,
            RoofPart::Batten => 2,
            _ => 3,
        }
    }
}

/// A storey's roof as heights: every section over its walls, with its eaves.
pub struct Field {
    /// How high its walls' tops are, in metres.
    pub top: f32,
    sections: Vec<Owner>,
}

/// A section of a roof, and what it is built in.
#[derive(Clone, Copy, Debug)]
struct Owner {
    id: RoofId,
    section: RoofSection,
    eaves: f32,
    kind: Kind,
}

impl Field {
    /// Every storey's roof in a base, by storey, each section's eaves its kit's.
    pub fn all(base: &Base, kits: &Kits) -> BTreeMap<i32, Field> {
        let mut fields: BTreeMap<i32, Field> = BTreeMap::new();
        for (id, roof) in base.roofs() {
            let section = roof.section;
            let kind = base.roof_kind(roof).unwrap_or(Kind::PLAIN);
            let eaves = kits.of(kind).map_or(EAVES_M, |kit| kit.eaves);
            let storey = section.storey();
            fields
                .entry(storey)
                .or_insert_with(|| Field {
                    top: (storey + 1) as f32 * STOREY_M,
                    sections: Vec::new(),
                })
                .sections
                .push(Owner {
                    id,
                    section,
                    eaves,
                    kind,
                });
        }
        fields
    }

    /// How far above the walls' tops the roof is at a point, east and south in metres, and
    /// which of its sections makes it there.
    fn at(&self, at: Vec2) -> Option<(f32, usize)> {
        let inside = |section: &RoofSection| {
            let (low, high) = section.bounds();
            at.cmpge(low - 1e-4).all() && at.cmple(high + 1e-4).all()
        };
        let within = self.sections.iter().any(|owner| inside(&owner.section));
        let mut best: Option<(f32, usize)> = None;
        for (index, owner) in self.sections.iter().enumerate() {
            let rise = if inside(&owner.section) {
                owner.section.rise_beyond(at, 0.0)
            } else if !within {
                owner.section.rise_beyond(at, owner.eaves)
            } else {
                None
            };
            if let Some(rise) = rise
                && best.is_none_or(|(highest, _)| rise > highest + 1e-6)
            {
                best = Some((rise, index));
            }
        }
        best
    }

    /// Whether any of it, its eaves too, is over the columns of a scope.
    pub(crate) fn reaches(&self, scope: Scope) -> bool {
        self.sections.iter().any(|owner| {
            let (low, high) = owner.section.bounds();
            let reach = owner.eaves.max(0.0) + CELL_M;
            let (low, high) = (
                ((low - reach) / CELL_M).floor().as_ivec2(),
                ((high + reach) / CELL_M).ceil().as_ivec2(),
            );
            // As high as it could rise over its widest span, its walls' tops below.
            let span = (owner.section.bounds().1 - owner.section.bounds().0).max_element();
            let (bottom, top) = (
                ((self.top - 1.0) / CELL_M).floor() as i32,
                ((self.top + owner.section.pitch * span + 1.0) / CELL_M).ceil() as i32,
            );
            low.x <= scope.high.x
                && high.x >= scope.low.x
                && low.y <= scope.high.z
                && high.y >= scope.low.z
                && bottom <= scope.high.y
                && top >= scope.low.y
        })
    }

    /// How far above the walls' tops the roof's top is at a point, if it is over it.
    pub fn rise(&self, at: Vec2) -> Option<f32> {
        self.at(at).map(|(rise, _)| rise)
    }

    /// The height of the roof's underside at a point, in metres, if it is over it.
    pub fn under(&self, at: Vec2) -> Option<f32> {
        self.rise(at).map(|rise| self.top + rise - ROOF_THICK_M)
    }
}

/// The four sides of a square, as the steps to its neighbour across each and the corners
/// along each, from the square's own corner: north, east, south, west.
const SIDES: [([i32; 2], [i32; 2], [i32; 2]); 4] = [
    ([0, -1], [0, 0], [1, 0]),
    ([1, 0], [1, 0], [1, 1]),
    ([0, 1], [1, 1], [0, 1]),
    ([-1, 0], [0, 1], [0, 0]),
];

/// A square of the lattice under a roof.
#[derive(Clone, Copy, Debug)]
struct Square {
    /// The section highest over its middle.
    owner: usize,
    /// Whether a wall stands up through it, so it has no covering.
    blocked: bool,
}

/// A storey's roof sampled on the lattice.
struct Lattice<'a> {
    field: &'a Field,
    /// Each corner's rise, by its place on the lattice.
    nodes: QuickMap<[i32; 2], f32>,
    middles: QuickMap<[i32; 2], f32>,
    /// The squares, in order, and the same to look up quickly.
    squares: BTreeMap<[i32; 2], Square>,
    found: QuickMap<[i32; 2], Square>,
}

impl<'a> Lattice<'a> {
    fn new(base: &Base, field: &'a Field) -> Lattice<'a> {
        let mut lattice = Lattice {
            field,
            nodes: QuickMap::default(),
            middles: QuickMap::default(),
            squares: BTreeMap::new(),
            found: QuickMap::default(),
        };
        for owner in &field.sections {
            let (low, high) = owner.section.bounds();
            let reach = owner.eaves.max(0.0);
            let from = ((low - reach) / STEP_M).floor().as_ivec2();
            let to = ((high + reach) / STEP_M).ceil().as_ivec2();
            for i in from.x..to.x {
                for j in from.y..to.y {
                    if lattice.found.contains_key(&[i, j]) {
                        continue;
                    }
                    let middle = (Vec2::new(i as f32, j as f32) + 0.5) * STEP_M;
                    let Some((rise, owner)) = field.at(middle) else {
                        continue;
                    };
                    // Each corner's rise, worked out once for the squares that share it.
                    let mut corners = true;
                    for [di, dj] in [[0, 0], [1, 0], [0, 1], [1, 1]] {
                        let node = [i + di, j + dj];
                        if lattice.nodes.contains_key(&node) {
                            continue;
                        }
                        match field.rise(Vec2::new(node[0] as f32, node[1] as f32) * STEP_M) {
                            Some(rise) => {
                                lattice.nodes.insert(node, rise);
                            }
                            None => corners = false,
                        }
                    }
                    if !corners {
                        continue;
                    }
                    // A wall standing up through the roof here.
                    let cube = IVec3::new(
                        (middle.x / CELL_M).floor() as i32,
                        ((field.top + rise + 0.05) / CELL_M).floor() as i32,
                        (middle.y / CELL_M).floor() as i32,
                    );
                    let blocked = base
                        .cube(cube)
                        .is_some_and(|c| c.fill != Fill::Open && base.in_wall(cube));
                    lattice.middles.insert([i, j], rise);
                    lattice.squares.insert([i, j], Square { owner, blocked });
                    lattice.found.insert([i, j], Square { owner, blocked });
                }
            }
        }
        lattice
    }

    fn point(&self, node: [i32; 2]) -> Vec3 {
        Vec3::new(
            node[0] as f32 * STEP_M,
            self.field.top + self.nodes[&node],
            node[1] as f32 * STEP_M,
        )
    }

    fn middle(&self, square: [i32; 2]) -> Vec3 {
        Vec3::new(
            (square[0] as f32 + 0.5) * STEP_M,
            self.field.top + self.middles[&square],
            (square[1] as f32 + 0.5) * STEP_M,
        )
    }

    /// A square with covering on it: under the roof, and no wall through it.
    fn covered(&self, square: [i32; 2]) -> Option<Square> {
        self.found.get(&square).copied().filter(|s| !s.blocked)
    }

    /// One of a square's four triangles, on a side: its middle and the side's two corners.
    fn triangle(&self, square: [i32; 2], side: usize) -> [Vec3; 3] {
        let (_, a, b) = SIDES[side];
        let at = |d: [i32; 2]| self.point([square[0] + d[0], square[1] + d[1]]);
        [self.middle(square), at(a), at(b)]
    }

    /// The cell of a roof's section a square goes up with: the one it is over, or the
    /// nearest, for eaves.
    fn cell(&self, square: [i32; 2], stage: u8) -> RoofCell {
        let owner = &self.field.sections[self.found[&square].owner];
        let (min, max) = (owner.section.min, owner.section.max);
        RoofCell {
            roof: owner.id,
            cell: [
                square[0].div_euclid(2).clamp(min[0], max[0]),
                square[1].div_euclid(2).clamp(min[2], max[2]),
            ],
            stage,
        }
    }
}

/// A triangle's normal, facing up.
fn normal(t: &[Vec3; 3]) -> Vec3 {
    let n = (t[2] - t[0]).cross(t[1] - t[0]).normalize_or(Vec3::Y);
    if n.y < 0.0 { -n } else { n }
}

/// What two triangles sharing an edge from `p` to `q` make of it, if they meet at an angle.
fn crease(first: &[Vec3; 3], second: &[Vec3; 3], p: Vec3, q: Vec3) -> Option<RoofPart> {
    let (n1, n2) = (normal(first), normal(second));
    if n1.dot(n2) > 1.0 - 1e-5 {
        return None;
    }
    let apex = second
        .iter()
        .copied()
        .max_by(|a, b| {
            let off = |x: Vec3| x.distance(p).min(x.distance(q));
            off(*a).total_cmp(&off(*b))
        })
        .unwrap_or(second[0]);
    let convex = (apex - p).dot(n1) < 0.0;
    let level = (p.y - q.y).abs() < 1e-4;
    Some(match (convex, level) {
        (true, true) => RoofPart::Ridge,
        (true, false) => RoofPart::Hip,
        (false, _) => RoofPart::Valley,
    })
}

/// A line of roof before it is joined to those it runs on from.
#[derive(Clone, Debug)]
struct Segment {
    part: RoofPart,
    from: Place,
    to: Place,
    sides: [Place; 2],
    square: [i32; 2],
}

/// Lays the pitched roofs of a base built in a kit's kind: every piece of each square of
/// them the kit owns, in the order they go up.
pub(crate) fn lay(base: &Base, kit: &Kit, kits: &Kits, scope: Scope, pieces: &mut Vec<Piece>) {
    for field in Field::all(base, kits).values() {
        // A roof is laid whole, so its long pieces join as they do over the whole base,
        // but only where it reaches the scope.
        if !field.reaches(scope) || !field.sections.iter().any(|o| o.kind == kit.kind()) {
            continue;
        }
        let lattice = Lattice::new(base, field);
        let ours = |square: [i32; 2]| {
            lattice
                .found
                .get(&square)
                .is_some_and(|s| field.sections[s.owner].kind == kit.kind())
        };
        if !lattice.squares.keys().any(|&square| ours(square)) {
            continue;
        }
        let mut segments: Vec<Segment> = Vec::new();
        for (&square, here) in &lattice.squares {
            if here.blocked || !ours(square) {
                continue;
            }
            let triangles: [[Vec3; 3]; 4] = [0, 1, 2, 3].map(|side| lattice.triangle(square, side));
            let centroid = |t: &[Vec3; 3]| Place::near((t[0] + t[1] + t[2]) / 3.0);
            // Its diagonals, between its own triangles.
            for side in 0..4 {
                let (a, b) = (&triangles[side], &triangles[(side + 1) % 4]);
                let (p, q) = (a[0], a[2]);
                if let Some(part) = crease(a, b, p, q) {
                    segments.push(Segment {
                        part,
                        from: Place::near(p),
                        to: Place::near(q),
                        sides: [centroid(a), centroid(b)],
                        square,
                    });
                }
            }
            // Its sides: shared with the square beyond, or an edge of the roof.
            for (side, triangle) in triangles.iter().enumerate() {
                let (step, _, _) = SIDES[side];
                let beyond = [square[0] + step[0], square[1] + step[1]];
                let (p, q) = (triangle[1], triangle[2]);
                let part = match lattice.found.get(&beyond) {
                    Some(there) if !there.blocked => {
                        // Each shared side once, from the square to its west or north.
                        if side == 0 || side == 3 {
                            continue;
                        }
                        let other = lattice.triangle(beyond, (side + 2) % 4);
                        let Some(part) = crease(triangle, &other, p, q) else {
                            continue;
                        };
                        segments.push(Segment {
                            part,
                            from: Place::near(p),
                            to: Place::near(q),
                            sides: [centroid(triangle), centroid(&other)],
                            square,
                        });
                        continue;
                    }
                    Some(_) => RoofPart::Flashing,
                    None if (p.y - q.y).abs() < 1e-4 => RoofPart::Fascia,
                    None => RoofPart::Verge,
                };
                let out = Vec3::new(step[0] as f32, 0.0, step[1] as f32) * STEP_M;
                segments.push(Segment {
                    part,
                    from: Place::near(p),
                    to: Place::near(q),
                    sides: [centroid(triangle), Place::near((p + q) * 0.5 + out)],
                    square,
                });
            }
        }
        let joined = join(segments);
        let r = &kit.roof;
        let (rafter, batten) = (r.rafter.1 * 0.5, r.batten.1);
        // The beams under the creases, then every long piece in its own stage.
        for segment in &joined {
            let cell = lattice.cell(segment.square, segment.part.stage());
            let seed = hash(&[
                segment.from.0[0] as u32,
                segment.from.0[2] as u32,
                segment.part as u32,
            ]);
            if matches!(
                segment.part,
                RoofPart::Ridge | RoofPart::Hip | RoofPart::Valley
            ) {
                let (a, b) = (segment.sides[0].metres(), segment.sides[1].metres());
                let (from, to) = (segment.from.metres(), segment.to.metres());
                let down = (planar_normal(from, to, a) + planar_normal(from, to, b))
                    .normalize_or(Vec3::Y)
                    * (batten + rafter);
                pieces.push(Piece::Roof {
                    part: RoofPart::Rafter,
                    from: Place::near(from - down),
                    to: Place::near(to - down),
                    sides: segment.sides,
                    seed: hash(&[seed, 1]),
                    cell: lattice.cell(segment.square, 1),
                });
            }
            pieces.push(Piece::Roof {
                part: segment.part,
                from: segment.from,
                to: segment.to,
                sides: segment.sides,
                seed,
                cell,
            });
        }
        rafters(&lattice, &ours, batten + rafter, pieces);
        // Battens and covering, a square at a time.
        let texture = kit
            .textures
            .get(&crate::Material::Covering)
            .map_or(1.0, |texture| texture.metres[1]);
        let exposure = r.exposure(texture);
        let mut battens: Vec<Segment> = Vec::new();
        for (&square, here) in &lattice.squares {
            if here.blocked || !ours(square) {
                continue;
            }
            let owner = &field.sections[here.owner];
            let pitch = owner.section.pitch;
            // The courses' lower edges: the first at the eaves, then a course's rise up.
            let eave = field.top - owner.eaves * pitch;
            let rise = exposure * pitch / (1.0 + pitch * pitch).sqrt();
            let corners = [[0, 0], [1, 0], [1, 1], [0, 1]]
                .map(|[di, dj]| Place::near(lattice.point([square[0] + di, square[1] + dj])));
            let open = [0, 1, 2, 3].map(|side| {
                let (step, _, _) = SIDES[side];
                lattice
                    .covered([square[0] + step[0], square[1] + step[1]])
                    .is_none()
            });
            let courses = (ticks(eave), ticks(rise).max(1));
            pieces.push(Piece::Covering {
                corners,
                middle: Place::near(lattice.middle(square)),
                courses,
                open,
                cell: lattice.cell(square, 3),
            });
            // A batten under the foot of every course across the square, each joined to
            // the next square's into one along the face.
            let (base, step) = (super::metres(courses.0), super::metres(courses.1));
            for side in 0..4 {
                let t = lattice.triangle(square, side);
                let n = normal(&t);
                let upslope = n.cross(Vec3::Y.cross(n).normalize_or(Vec3::X));
                let low = t.iter().map(|p| p.y).fold(f32::MAX, f32::min);
                let high = t.iter().map(|p| p.y).fold(f32::MIN, f32::max);
                let first = ((low - base) / step).ceil() as i32;
                let last = ((high - base) / step).floor() as i32;
                for course in first..=last {
                    let Some((p, q)) = level_cut(&t, base + course as f32 * step) else {
                        continue;
                    };
                    if p.distance(q) < 1e-3 {
                        continue;
                    }
                    let beside = Place::near((p + q) * 0.5 + upslope * 0.1);
                    battens.push(Segment {
                        part: RoofPart::Batten,
                        from: Place::near(p),
                        to: Place::near(q),
                        sides: [beside, beside],
                        square,
                    });
                }
            }
        }
        for segment in join(battens) {
            pieces.push(Piece::Roof {
                part: RoofPart::Batten,
                from: segment.from,
                to: segment.to,
                sides: segment.sides,
                seed: 0,
                cell: lattice.cell(segment.square, 2),
            });
        }
    }
}

/// Where a level plane crosses a triangle, if it does: the two ends of the line.
fn level_cut(t: &[Vec3; 3], height: f32) -> Option<(Vec3, Vec3)> {
    let mut found = [Vec3::ZERO; 3];
    let mut count = 0;
    for i in 0..3 {
        let (a, b) = (t[i], t[(i + 1) % 3]);
        let point = if (a.y - height) * (b.y - height) < 0.0 {
            a + (b - a) * ((height - a.y) / (b.y - a.y))
        } else if (a.y - height).abs() < 1e-6 {
            a
        } else {
            continue;
        };
        // Points that follow one another the same are one.
        if count > 0 && found[count - 1].distance(point) < 1e-6 {
            continue;
        }
        found[count] = point;
        count += 1;
    }
    (count >= 2).then(|| (found[0], found[count - 1]))
}

/// The normal of the plane through a line and a point beside it, facing up.
pub(crate) fn planar_normal(from: Vec3, to: Vec3, beside: Vec3) -> Vec3 {
    let n = (to - from).cross(beside - from).normalize_or(Vec3::Y);
    if n.y < 0.0 { -n } else { n }
}

/// Lines of roof in a line joined end to end: each ridge, hip, valley, eave, verge and
/// flashing one piece from end to end.
fn join(segments: Vec<Segment>) -> Vec<Segment> {
    // By what each is and the line it is on in plan, then along it. Heights are to the
    // nearest tick, so a line's slope is compared, not its direction in ticks.
    type Line = (RoofPart, [i64; 2], i64);
    let mut lines: BTreeMap<Line, Vec<(i64, i64, Segment)>> = BTreeMap::new();
    for mut segment in segments {
        let plan = |p: Place| glam::I64Vec2::new(i64::from(p.0[0]), i64::from(p.0[2]));
        let mut d = plan(segment.to) - plan(segment.from);
        let g = i64::from(gcd(d.x.abs() as i32, d.y.abs() as i32).max(1));
        d /= g;
        if d.x < 0 || (d.x == 0 && d.y < 0) {
            d = -d;
            std::mem::swap(&mut segment.from, &mut segment.to);
        }
        let (a, b) = (plan(segment.from), plan(segment.to));
        let key = (segment.part, d.to_array(), a.perp_dot(d));
        lines
            .entry(key)
            .or_default()
            .push((a.dot(d), b.dot(d), segment));
    }
    let slope = |s: &Segment| {
        let (a, b) = (s.from.metres(), s.to.metres());
        (b.y - a.y) / Vec2::new(b.x - a.x, b.z - a.z).length().max(1e-6)
    };
    let mut out = Vec::new();
    for (_, mut on_line) in lines {
        on_line.sort_by_key(|(start, end, _)| (*start, *end));
        let mut current: Option<(i64, Segment)> = None;
        for (start, end, segment) in on_line {
            match &mut current {
                Some((reach, joined))
                    if start <= *reach && (slope(joined) - slope(&segment)).abs() < 0.02 =>
                {
                    if end > *reach {
                        *reach = end;
                        joined.to = segment.to;
                    }
                }
                _ => {
                    if let Some((_, done)) = current.take() {
                        out.push(done);
                    }
                    current = Some((end, segment));
                }
            }
        }
        if let Some((_, done)) = current {
            out.push(done);
        }
    }
    out
}

fn gcd(a: i32, b: i32) -> i32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// The rafters: one up every slope at every cube, from its foot at the eaves to the ridge,
/// hip or valley it meets, under the battens.
fn rafters(
    lattice: &Lattice,
    ours: &dyn Fn([i32; 2]) -> bool,
    below: f32,
    pieces: &mut Vec<Piece>,
) {
    // Lines north and south at every cube, for slopes falling north or south; lines east
    // and west, for slopes falling east or west.
    let (mut low, mut high) = ([i32::MAX; 2], [i32::MIN; 2]);
    for square in lattice.squares.keys() {
        for axis in 0..2 {
            low[axis] = low[axis].min(square[axis]);
            high[axis] = high[axis].max(square[axis] + 1);
        }
    }
    if low[0] > high[0] {
        return;
    }
    for across in 0..2 {
        let along = 1 - across;
        let first = low[across] + low[across].rem_euclid(2);
        for line in (first..=high[across]).step_by(2) {
            // The run of squares along the line whose triangles beside it fall along it,
            // one way: (its first square, its last, which way it falls).
            let mut run: Option<([i32; 2], [i32; 2], bool)> = None;
            let mut done: Vec<([i32; 2], [i32; 2])> = Vec::new();
            for at in low[along]..=high[along] {
                // The squares either side of the line here, and their triangles against it.
                let mut sides = Vec::new();
                for before in [true, false] {
                    let mut square = [0; 2];
                    square[across] = if before { line - 1 } else { line };
                    square[along] = at;
                    if lattice.covered(square).is_none() || !ours(square) {
                        continue;
                    }
                    // The side of the square on the line.
                    let side = match (across, before) {
                        (0, true) => 1,
                        (0, false) => 3,
                        (_, true) => 2,
                        (_, false) => 0,
                    };
                    sides.push((square, normal(&lattice.triangle(square, side))));
                }
                let falls: Vec<Option<bool>> = sides
                    .iter()
                    .map(|(_, n)| {
                        let (a, c) = (
                            n[if along == 0 { 0 } else { 2 }],
                            n[if across == 0 { 0 } else { 2 }],
                        );
                        (a.abs() > c.abs() + 1e-4 && n.y < 1.0 - 1e-4).then_some(a > 0.0)
                    })
                    .collect();
                let way = match falls.as_slice() {
                    [Some(a), Some(b)] if a == b => Some(*a),
                    _ => None,
                };
                let square = sides.first().map(|(square, _)| *square);
                match (way, &mut run, square) {
                    (Some(way), Some((_, last, running)), Some(square)) if *running == way => {
                        *last = square;
                    }
                    (Some(way), _, Some(square)) => {
                        if let Some((a, b, _)) = run.take() {
                            done.push((a, b));
                        }
                        run = Some((square, square, way));
                    }
                    _ => {
                        if let Some((a, b, _)) = run.take() {
                            done.push((a, b));
                        }
                    }
                }
            }
            if let Some((a, b, _)) = run.take() {
                done.push((a, b));
            }
            for (a, b) in done {
                // From the first square's near corner on the line to the last's far one.
                let node = |square: [i32; 2], end: i32| {
                    let mut node = [0; 2];
                    node[across] = line;
                    node[along] = square[along] + end;
                    node
                };
                let (from, to) = (lattice.point(node(a, 0)), lattice.point(node(b, 1)));
                let face = lattice.triangle(
                    a,
                    match (across, a[across] < line) {
                        (0, true) => 1,
                        (0, false) => 3,
                        (_, true) => 2,
                        (_, false) => 0,
                    },
                );
                let down = normal(&face) * below;
                let centre = (face[0] + face[1] + face[2]) / 3.0;
                pieces.push(Piece::Roof {
                    part: RoofPart::Rafter,
                    from: Place::near(from - down),
                    to: Place::near(to - down),
                    sides: [Place::near(centre), Place::near(centre)],
                    seed: hash(&[line as u32, a[along] as u32, across as u32, 3]),
                    cell: lattice.cell(a, 1),
                });
            }
        }
    }
}

/// The cubes a block kind's gable fills: over each of its walls' tops a roof covers, up to
/// the roof's underside, each with the underside's height over it, so its surface is cut
/// to the slope. Over a gable end they rise to the ridge; along the eaves they close the
/// wedge between the wall and the roof.
pub(crate) fn gables(base: &Base, kit: &Kit, kits: &Kits, scope: Scope, pieces: &mut Vec<Piece>) {
    let fields = Field::all(base, kits);
    if fields.is_empty() {
        return;
    }
    for (at, cube) in scope.cubes(base, None) {
        if cube.kind != kit.kind() || cube.fill == Fill::Open || !base.in_wall(at) {
            continue;
        }
        let storey = crate::storey_of(at.y);
        if at.y != crate::wall_layer(storey) + STOREY_CUBES - 1 {
            continue;
        }
        let Some(field) = fields.get(&storey) else {
            continue;
        };
        let mut above = at + IVec3::Y;
        while base.cube(above).is_none() {
            let bottom = above.y as f32 * CELL_M;
            let under: Option<Vec<i32>> = (0..9)
                .map(|n| {
                    let corner = Vec2::new(
                        (above.x as f32 + (n % 3) as f32 * 0.5) * CELL_M,
                        (above.z as f32 + (n / 3) as f32 * 0.5) * CELL_M,
                    );
                    field.under(corner).map(ticks)
                })
                .collect();
            let Some(under) = under else {
                break;
            };
            let highest = under.iter().copied().max().unwrap_or(i32::MIN);
            if super::metres(highest) < bottom + 0.02 {
                break;
            }
            pieces.push(Piece::Gable {
                cube: above,
                under: [
                    [under[0], under[1], under[2]],
                    [under[3], under[4], under[5]],
                    [under[6], under[7], under[8]],
                ],
            });
            above += IVec3::Y;
        }
    }
}

/// How high a gable's cube may go at a point of it, in metres: the roof's underside over the
/// point, from the heights over the cube's top in its piece (`Piece::Gable`). Everything of
/// the cube over it is brought down to it, so a face cut away entirely comes to nothing.
pub fn gable_cut(cube: IVec3, under: &[[i32; 3]; 3], at: Vec3) -> f32 {
    let x = ((at.x / CELL_M - cube.x as f32) * 2.0).clamp(0.0, 2.0);
    let z = ((at.z / CELL_M - cube.z as f32) * 2.0).clamp(0.0, 2.0);
    let (i, k) = ((x as usize).min(1), (z as usize).min(1));
    let (tx, tz) = (x - i as f32, z - k as f32);
    let h = |row: usize, column: usize| super::metres(under[row][column]);
    let north = h(k, i) * (1.0 - tx) + h(k, i + 1) * tx;
    let south = h(k + 1, i) * (1.0 - tx) + h(k + 1, i + 1) * tx;
    north * (1.0 - tz) + south * tz
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::tests::kit;
    use crate::{BuildOp, RoofStyle};

    fn roofed(rooms: &[([i32; 3], [i32; 3])], roofs: &[RoofSection]) -> Base {
        let mut base = Base::default();
        for &(min, max) in rooms {
            base.apply(BuildOp::AddRoom {
                min,
                max,
                kind: kit().kind(),
            });
        }
        for &section in roofs {
            base.apply(BuildOp::AddRoof(section));
        }
        base
    }

    fn section(min: [i32; 3], max: [i32; 3], style: RoofStyle, along_x: bool) -> RoofSection {
        RoofSection {
            min,
            max,
            style,
            along_x,
            pitch: 0.7,
        }
    }

    fn laid(base: &Base) -> Vec<Piece> {
        let kit = kit();
        let mut pieces = Vec::new();
        lay(
            base,
            &kit,
            &Kits(vec![kit.clone()]),
            Scope::ALL,
            &mut pieces,
        );
        pieces
    }

    fn parts(pieces: &[Piece], part: RoofPart) -> Vec<(Vec3, Vec3)> {
        pieces
            .iter()
            .filter_map(|p| match *p {
                Piece::Roof {
                    part: this,
                    from,
                    to,
                    ..
                } if this == part => Some((from.metres(), to.metres())),
                _ => None,
            })
            .collect()
    }

    fn close(a: Vec3, b: Vec3) -> bool {
        a.distance(b) < 0.01
    }

    #[test]
    fn a_gable_roof_has_one_ridge_two_eaves_and_four_verges() {
        // Five metres by four, the ridge along X two metres in, rising 1.4 m.
        let base = roofed(
            &[([0, 0, 0], [9, 0, 7])],
            &[section([0, 0, 0], [9, 0, 7], RoofStyle::Gable, true)],
        );
        let pieces = laid(&base);
        let ridges = parts(&pieces, RoofPart::Ridge);
        assert_eq!(ridges.len(), 1, "{ridges:?}");
        let (a, b) = ridges[0];
        let top = STOREY_M + 2.0 * 0.7;
        // From verge to verge, over the gables' overhang.
        assert!(
            close(a, Vec3::new(-0.5, top, 2.0)) && close(b, Vec3::new(5.5, top, 2.0)),
            "{a} {b}"
        );
        assert_eq!(parts(&pieces, RoofPart::Fascia).len(), 2);
        assert_eq!(parts(&pieces, RoofPart::Verge).len(), 4);
        assert!(parts(&pieces, RoofPart::Hip).is_empty());
        assert!(parts(&pieces, RoofPart::Valley).is_empty());
        // The eaves overhang by half a metre, carried on down at the pitch.
        for (a, b) in parts(&pieces, RoofPart::Fascia) {
            assert!(
                (a.y - (STOREY_M - 0.35)).abs() < 0.01 && a.y == b.y,
                "{a} {b}"
            );
            assert!((a.z + 0.5).abs() < 0.01 || (a.z - 4.5).abs() < 0.01, "{a}");
        }
        // A covering and battens over every square, eaves and all, each with its stage.
        let squares = pieces
            .iter()
            .filter(|p| matches!(p, Piece::Covering { .. }))
            .count();
        assert_eq!(squares, 24 * 20);
        for piece in &pieces {
            let (stage, want) = match piece {
                Piece::Covering { cell, .. } => (cell.stage, 3),
                Piece::Roof { part, cell, .. } => (cell.stage, part.stage()),
                _ => continue,
            };
            assert_eq!(stage, want, "{piece:?}");
        }
    }

    #[test]
    fn a_hip_roof_has_four_hips_and_a_short_ridge() {
        let base = roofed(
            &[([0, 0, 0], [9, 0, 7])],
            &[section([0, 0, 0], [9, 0, 7], RoofStyle::Hip, true)],
        );
        let pieces = laid(&base);
        let hips = parts(&pieces, RoofPart::Hip);
        assert_eq!(hips.len(), 4, "{hips:?}");
        let ridges = parts(&pieces, RoofPart::Ridge);
        assert_eq!(ridges.len(), 1);
        let (a, b) = ridges[0];
        // A metre long: the five metres less two metres' run at each end.
        assert!((a.distance(b) - 1.0).abs() < 0.01, "{a} {b}");
        assert!(parts(&pieces, RoofPart::Verge).is_empty());
        assert_eq!(parts(&pieces, RoofPart::Fascia).len(), 4);
        // Hips run from the eaves' corners up to the ridge's ends.
        for (a, b) in hips {
            let (low, high) = if a.y < b.y { (a, b) } else { (b, a) };
            assert!((low.y - (STOREY_M - 0.35)).abs() < 0.01, "{low}");
            assert!(
                close(high, ridges[0].0) || close(high, ridges[0].1),
                "{high}"
            );
        }
    }

    #[test]
    fn an_l_has_a_valley_where_its_wings_meet_and_no_eaves_inside_the_other() {
        // Two wings four metres wide, one along X and one along Z, meeting in a corner.
        let base = roofed(
            &[([0, 0, 0], [11, 0, 7]), ([0, 0, 8], [7, 0, 15])],
            &[
                section([0, 0, 0], [11, 0, 7], RoofStyle::Gable, true),
                section([0, 0, 4], [7, 0, 15], RoofStyle::Gable, false),
            ],
        );
        let pieces = laid(&base);
        let valleys = parts(&pieces, RoofPart::Valley);
        assert!(!valleys.is_empty());
        // The valley runs from the inside corner of the L's eaves up to where the ridges meet.
        let top = STOREY_M + 2.0 * 0.7;
        let meet = Vec3::new(2.0, top, 2.0);
        assert!(
            valleys
                .iter()
                .any(|&(a, b)| close(a, meet) || close(b, meet)),
            "{valleys:?}"
        );
        // Nothing of either wing's roof stands above the other's: every covering square's
        // middle is on the field.
        let fields = Field::all(&base, &Kits(vec![kit()]));
        let field = &fields[&0];
        for piece in &pieces {
            if let Piece::Covering { middle, .. } = piece {
                let m = middle.metres();
                let rise = field.rise(Vec2::new(m.x, m.z)).unwrap();
                assert!((m.y - field.top - rise).abs() < 0.01, "{m}");
            }
        }
        // Two ridges, each from its gable's verge to where they meet.
        let ridges = parts(&pieces, RoofPart::Ridge);
        assert_eq!(ridges.len(), 2, "{ridges:?}");
    }

    #[test]
    fn a_lean_to_has_no_ridge_and_a_board_along_its_high_edge() {
        let base = roofed(
            &[([0, 0, 0], [9, 0, 7])],
            &[RoofSection {
                pitch: 0.4,
                ..section([0, 0, 0], [9, 0, 7], RoofStyle::LeanTo, true)
            }],
        );
        let pieces = laid(&base);
        assert!(parts(&pieces, RoofPart::Ridge).is_empty());
        assert_eq!(parts(&pieces, RoofPart::Fascia).len(), 2);
        assert_eq!(parts(&pieces, RoofPart::Verge).len(), 2);
    }

    #[test]
    fn a_roof_against_a_wall_standing_up_through_it_is_flashed() {
        // A lean-to rising south to a wall two storeys high.
        let mut base = roofed(&[([0, 0, 0], [9, 0, 5])], &[]);
        for x in -2..12 {
            for storey in 0..2 {
                base.apply(BuildOp::SetWall {
                    tile: [x, storey, 6],
                    panel: Some(crate::Panel::Solid),
                    kind: kit().kind(),
                });
            }
        }
        base.apply(BuildOp::AddRoof(RoofSection {
            pitch: 0.4,
            ..section([0, 0, 0], [9, 0, 5], RoofStyle::LeanTo, true)
        }));
        let pieces = laid(&base);
        let flashing = parts(&pieces, RoofPart::Flashing);
        // One fillet all along the wall's face, the eaves too.
        assert_eq!(flashing.len(), 1, "{flashing:?}");
        let (a, b) = flashing[0];
        assert!(
            (a.z - 3.0).abs() < 0.01 && (b.z - 3.0).abs() < 0.01,
            "{a} {b}"
        );
        assert!((a.distance(b) - 6.0).abs() < 0.01, "{a} {b}");
        // No covering inside the wall.
        for piece in &pieces {
            if let Piece::Covering { middle, .. } = piece {
                assert!(middle.metres().z < 3.0, "{middle:?}");
            }
        }
    }

    #[test]
    fn a_stone_gable_rises_to_the_roof_and_no_further() {
        let mut stone = kit();
        stone.id = 2;
        stone.pattern = crate::kit::Pattern::Masonry;
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [9, 0, 7],
            kind: stone.kind(),
        });
        base.apply(BuildOp::AddRoof(section(
            [0, 0, 0],
            [9, 0, 7],
            RoofStyle::Gable,
            true,
        )));
        let kits = Kits(vec![stone.clone()]);
        let mut pieces = Vec::new();
        gables(&base, &stone, &kits, Scope::ALL, &mut pieces);
        let cubes: Vec<IVec3> = pieces
            .iter()
            .filter_map(|p| match *p {
                Piece::Gable { cube, .. } => Some(cube),
                _ => None,
            })
            .collect();
        // Over the west wall's middle, up to the ridge 1.4 m over the walls, less the
        // roof's thickness: three cubes.
        let middle: Vec<i32> = cubes
            .iter()
            .filter(|c| c.x == 0 && c.z == 4)
            .map(|c| c.y)
            .collect();
        assert_eq!(middle, vec![6, 7, 8], "{cubes:?}");
        for piece in &pieces {
            if let Piece::Gable { cube, under } = piece {
                assert!(base.cube(*cube).is_none());
                assert!(
                    super::super::metres(*under.iter().flatten().max().unwrap())
                        > cube.y as f32 * CELL_M
                );
            }
        }
    }
}
