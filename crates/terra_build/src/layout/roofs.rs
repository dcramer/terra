//! Roofs (docs/building-kits.md, D226, D241): a roof kind's cubes laid as a roof, each
//! cube's look from its place among its neighbours, as a wall's edges are dressed by their
//! roles (D215).
//!
//! **The surface.** Every roof cube with nothing over it is the roof there: its top, a
//! slope's ramp, a corner's two, a whole cube's flat, sampled every half cube in four
//! triangles a square from the square's middle, so every ridge, hip and valley of cubes cut
//! to slopes falls on a square's side or diagonal. A square carries on into the square
//! beside it where their shared corners are at the same heights, whichever cube or layer
//! it is in: so a roof climbs a layer a cube, and two roofs at different heights in one
//! column, a storey's over another's, are apart.
//!
//! **What each edge is.** Where two squares meet at an angle is a crease: a ridge (level
//! and convex), a hip (sloping and convex) or a valley (concave). Where no square carries
//! on, the roof stops: an eave (level), a verge (sloping), or flashing where something
//! stands beyond it, a wall or a higher roof. Past an eave or a verge the covering is
//! carried on by the kit's `eaves`, drawn only (D192 as amended): down the slope at an
//! eave, level past a verge, round a hip's corner and meeting the next in a valley at an
//! inside corner, each square of it the plane of the cube it runs on from.
//!
//! **The pieces**, in the order a roof cube goes up, a stage a step of its fill
//! ([`stage_of`]): its frame (rafters up the slopes at every cube, a beam under every
//! ridge, hip and valley), its battens, its covering, then its trims, the boards along its
//! eaves and verges, the caps over its ridges and hips, its valleys' linings and the
//! flashing against walls. Each is placed in the grid (D208) and knows the roof cube whose
//! stage it waits for, so a server lays them without any art.

use super::{Piece, Place, Scope, hash, ticks};
use crate::kit::{Kit, Kits};
use crate::mesh::QuickMap;
use crate::{Base, CELL_M, Cube, Fill, Kind, Shape};
use glam::{IVec3, Vec2, Vec3};
use std::collections::{BTreeMap, BTreeSet};

/// How finely a roof is sampled, in metres: half a cube.
pub const STEP_M: f32 = CELL_M * 0.5;
/// How far a roof of a kind with no kit is carried on past its edges.
pub const EAVES_M: f32 = 0.5;
/// How many stages a roof cube goes up in: its frame, its battens, its covering and its
/// trims, a step of its fill each (`kit.cost.steps`).
pub const ROOF_STAGES: u8 = 4;

/// How far a roof cube is up, by its fill: nothing, then its frame, battens, covering and
/// trims, a quarter of the cube each.
pub fn stage_of(cube: &Cube) -> u8 {
    let level = u16::from(cube.level());
    (level * u16::from(ROOF_STAGES)).div_ceil(16) as u8
}

/// Which roof cube a piece of roof goes up with, and in which of its stages: 1 its frame,
/// 2 its battens, 3 its covering, 4 its trims.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RoofCell {
    pub cube: IVec3,
    pub stage: u8,
}

/// What a long piece of roof is.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum RoofPart {
    /// The frame: a rafter up a slope, or the beam under a ridge, hip or valley.
    Rafter,
    /// A batten across the rafters under the foot of a course.
    Batten,
    /// Along an eave: a fascia, or thatch's cut butt.
    Eave,
    /// The board along a verge, a gable's sloping edge: the bargeboard.
    Verge,
    /// The cap over a ridge.
    Ridge,
    /// The cap over a hip.
    Hip,
    /// The lining of a valley.
    Valley,
    /// Mortar where the roof meets a wall standing up beside it.
    Flashing,
}

impl RoofPart {
    /// The stage of its roof cube it goes up in.
    pub fn stage(self) -> u8 {
        match self {
            RoofPart::Rafter => 1,
            RoofPart::Batten => 2,
            _ => 4,
        }
    }
}

/// The stage a square of covering goes up in.
const COVERING: u8 = 3;

/// A roof cube's top over a point of its footprint, as a fraction of the cube: a planed
/// shape's top, else the cube's own.
fn surface(shape: Shape, u: f32, v: f32) -> f32 {
    if shape.planed_top() {
        shape.top(u, v)
    } else {
        1.0
    }
}

/// A roof cube's top carried on past its footprint, as the plane or planes it is cut to.
fn plane(shape: Shape, u: f32, v: f32) -> f32 {
    match shape {
        Shape::Half => 0.5,
        Shape::Slope(f) => f.toward(u, v),
        Shape::OuterCorner(f) => f.toward(u, v).min(f.turned(1).toward(u, v)),
        Shape::InnerCorner(f) => f.toward(u, v).max(f.turned(1).toward(u, v)),
        _ => 1.0,
    }
}

/// The four sides of a square, north, east, south and west: the step to the square beyond
/// each, and the corners along it, from the square's own north-west corner.
const SIDES: [([i32; 2], [i32; 2], [i32; 2]); 4] = [
    ([0, -1], [0, 0], [1, 0]),
    ([1, 0], [1, 0], [1, 1]),
    ([0, 1], [1, 1], [0, 1]),
    ([-1, 0], [0, 1], [0, 0]),
];

/// A square's corners, north-west, north-east, south-east and south-west, from its own
/// north-west.
const CORNERS: [[i32; 2]; 4] = [[0, 0], [1, 0], [1, 1], [0, 1]];

/// A square of a roof, half a cube across.
#[derive(Clone, Copy, Debug)]
struct Square {
    /// Where it is, in half cubes east and south.
    place: [i32; 2],
    /// The roof cube it is drawn for: the one it is over, or, past an edge, the one whose
    /// eaves it carries on.
    cube: IVec3,
    kind: Kind,
    /// Its corners' heights, as [`CORNERS`], and its middle's, in metres.
    corners: [f32; 4],
    middle: f32,
}

/// What is beyond a side of a square.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Beside {
    /// A square the roof carries on into.
    Square(usize),
    /// Nothing: an eave or a verge.
    Open,
    /// A wall, or a roof standing higher: flashing.
    Against,
}

/// A base's roofs near a place: every square of their surface and of their eaves, and what
/// is beyond each side of each.
pub struct Roofs {
    squares: Vec<Square>,
    by_place: QuickMap<[i32; 2], Vec<usize>>,
    beside: Vec<[Beside; 4]>,
    /// Each square's roof: which squares carry on into one another.
    roof: Vec<usize>,
    /// Each roof's lowest and highest point, in metres.
    span: Vec<(f32, f32)>,
    /// How far each square is from the nearest open edge, in squares.
    inside: Vec<u16>,
}

impl Roofs {
    /// The roofs of a base's roof cubes within `near` and every roof cube joined to them,
    /// whole, so every piece of them is laid as laying the whole base would.
    pub(crate) fn whole(base: &Base, kits: &Kits, near: Scope) -> Roofs {
        let mut found: BTreeSet<[i32; 3]> = BTreeSet::new();
        let mut frontier: Vec<IVec3> = near
            .cubes(base, None)
            .into_iter()
            .filter(|(_, cube)| cube.kind.is_roof())
            .map(|(at, _)| at)
            .collect();
        found.extend(frontier.iter().map(|at| at.to_array()));
        while let Some(at) = frontier.pop() {
            for x in -1..=1 {
                for y in -1..=1 {
                    for z in -1..=1 {
                        let next = at + IVec3::new(x, y, z);
                        if base.cube(next).is_some_and(|cube| cube.kind.is_roof())
                            && found.insert(next.to_array())
                        {
                            frontier.push(next);
                        }
                    }
                }
            }
        }
        let cubes: Vec<(IVec3, Cube)> = found
            .into_iter()
            .filter_map(|at| {
                let at = IVec3::from_array(at);
                base.cube(at).map(|cube| (at, cube))
            })
            .collect();
        Roofs::of(base, kits, &cubes)
    }

    /// Whether there is no roof at all.
    pub fn is_empty(&self) -> bool {
        self.squares.is_empty()
    }

    /// The roofs of these roof cubes, sorted.
    fn of(base: &Base, kits: &Kits, cubes: &[(IVec3, Cube)]) -> Roofs {
        let mut roofs = Roofs {
            squares: Vec::new(),
            by_place: QuickMap::default(),
            beside: Vec::new(),
            roof: Vec::new(),
            span: Vec::new(),
            inside: Vec::new(),
        };
        // The top of every roof cube with nothing over it.
        for &(at, cube) in cubes {
            if cube.fill == Fill::Open || base.cube(at + IVec3::Y).is_some() {
                continue;
            }
            for sub in [[0, 0], [1, 0], [0, 1], [1, 1]] {
                let height = |du: f32, dv: f32| {
                    let (u, v) = ((sub[0] as f32 + du) * 0.5, (sub[1] as f32 + dv) * 0.5);
                    (at.y as f32 + surface(cube.shape, u, v)) * CELL_M
                };
                roofs.squares.push(Square {
                    place: [at.x * 2 + sub[0], at.z * 2 + sub[1]],
                    cube: at,
                    kind: cube.kind,
                    corners: CORNERS.map(|[dx, dz]| height(dx as f32, dz as f32)),
                    middle: height(0.5, 0.5),
                });
            }
        }
        roofs.index();
        roofs.link(base);
        // Carried on past every open edge by its kit's eaves, each square of it on the
        // planes of the cube it runs on from; where two run on into the same square, the
        // higher, so eaves meeting round an inside corner make a valley.
        let real = roofs.squares.len();
        let mut past: BTreeMap<[i32; 2], Square> = BTreeMap::new();
        for q in 0..real {
            let square = roofs.squares[q];
            let reach = kits.of(square.kind).map_or(EAVES_M, |kit| kit.eaves);
            let n = libm::roundf(reach / STEP_M) as i32;
            if n <= 0 {
                continue;
            }
            let open: [bool; 4] = roofs.beside[q].map(|b| b == Beside::Open);
            let shape = base.cube(square.cube).map_or(Shape::Whole, |c| c.shape);
            let mut out: Vec<[i32; 2]> = Vec::new();
            for side in 0..4 {
                if !open[side] {
                    continue;
                }
                let (step, _, _) = SIDES[side];
                for k in 1..=n {
                    out.push([step[0] * k, step[1] * k]);
                }
                // Round the corner, where the next side is open too.
                let next = (side + 1) % 4;
                if open[next] {
                    let (turn, _, _) = SIDES[next];
                    for a in 1..=n {
                        for b in 1..=n {
                            out.push([step[0] * a + turn[0] * b, step[1] * a + turn[1] * b]);
                        }
                    }
                }
            }
            for [dx, dz] in out {
                let place = [square.place[0] + dx, square.place[1] + dz];
                let height = |x: f32, z: f32| {
                    // A point in half cubes, in the owning cube's fractions.
                    let (u, v) = (
                        x * 0.5 - square.cube.x as f32,
                        z * 0.5 - square.cube.z as f32,
                    );
                    (square.cube.y as f32 + plane(shape, u, v)) * CELL_M
                };
                let corners =
                    CORNERS.map(|[cx, cz]| height((place[0] + cx) as f32, (place[1] + cz) as f32));
                let middle = height(place[0] as f32 + 0.5, place[1] as f32 + 0.5);
                // Not where the roof's cubes are, near it, nor into a wall or anything else
                // beside the roof.
                let roofed = roofs.by_place.get(&place).is_some_and(|on| {
                    on.iter()
                        .any(|&r| (roofs.squares[r].middle - middle).abs() < 1.0)
                });
                if roofed {
                    continue;
                }
                let inside = IVec3::new(
                    libm::floorf((place[0] as f32 + 0.5) * 0.5) as i32,
                    libm::floorf(middle / CELL_M + 0.05) as i32,
                    libm::floorf((place[1] as f32 + 0.5) * 0.5) as i32,
                );
                if base.cube(inside).is_some_and(|c| c.fill != Fill::Open) {
                    continue;
                }
                let candidate = Square {
                    place,
                    cube: square.cube,
                    kind: square.kind,
                    corners,
                    middle,
                };
                past.entry(place)
                    .and_modify(|there| {
                        if candidate.middle > there.middle + 1e-4 {
                            there.cube = candidate.cube;
                            there.kind = candidate.kind;
                        }
                        for k in 0..4 {
                            there.corners[k] = there.corners[k].max(candidate.corners[k]);
                        }
                        there.middle = there.middle.max(candidate.middle);
                    })
                    .or_insert(candidate);
            }
        }
        roofs.squares.extend(past.into_values());
        roofs.index();
        roofs.link(base);
        roofs.group();
        roofs
    }

    fn index(&mut self) {
        self.by_place.clear();
        for (q, square) in self.squares.iter().enumerate() {
            self.by_place.entry(square.place).or_default().push(q);
        }
    }

    /// What is beyond each side of each square.
    fn link(&mut self, base: &Base) {
        self.beside = (0..self.squares.len())
            .map(|q| {
                [0, 1, 2, 3].map(|side| {
                    let square = &self.squares[q];
                    let (step, _, _) = SIDES[side];
                    let place = [square.place[0] + step[0], square.place[1] + step[1]];
                    let along = |r: &usize| {
                        let other = &self.squares[*r];
                        (square.corners[side] - other.corners[(side + 3) % 4]).abs() < 1e-3
                            && (square.corners[(side + 1) % 4] - other.corners[(side + 2) % 4])
                                .abs()
                                < 1e-3
                    };
                    if let Some(r) = self
                        .by_place
                        .get(&place)
                        .and_then(|on| on.iter().copied().find(|r| along(r)))
                    {
                        return Beside::Square(r);
                    }
                    // Just past the edge and a little over it: something there stands up
                    // beside the roof.
                    let (a, b) = (self.point(q, side), self.point(q, (side + 1) % 4));
                    let out = Vec3::new(step[0] as f32, 0.0, step[1] as f32);
                    let probe = (a + b) * 0.5 + out * 0.05 + Vec3::Y * 0.05;
                    let cube = (probe / CELL_M).floor().as_ivec3();
                    if base.cube(cube).is_some_and(|c| c.fill != Fill::Open) {
                        Beside::Against
                    } else {
                        Beside::Open
                    }
                })
            })
            .collect();
    }

    /// Groups the squares into roofs, each its height and how far each square is from its
    /// open edges.
    fn group(&mut self) {
        let count = self.squares.len();
        let mut parent: Vec<usize> = (0..count).collect();
        fn root(parent: &mut [usize], mut q: usize) -> usize {
            while parent[q] != q {
                parent[q] = parent[parent[q]];
                q = parent[q];
            }
            q
        }
        for q in 0..count {
            for b in self.beside[q] {
                if let Beside::Square(r) = b {
                    let (a, b) = (root(&mut parent, q), root(&mut parent, r));
                    if a != b {
                        parent[a.max(b)] = a.min(b);
                    }
                }
            }
        }
        self.roof = (0..count).map(|q| root(&mut parent, q)).collect();
        self.span = vec![(f32::MAX, f32::MIN); count];
        for q in 0..count {
            let r = self.roof[q];
            let square = &self.squares[q];
            for h in square.corners.iter().chain([&square.middle]) {
                self.span[r] = (self.span[r].0.min(*h), self.span[r].1.max(*h));
            }
        }
        // From the squares on an open edge in.
        self.inside = vec![u16::MAX; count];
        let mut frontier = std::collections::VecDeque::new();
        for q in 0..count {
            if self.beside[q].contains(&Beside::Open) {
                self.inside[q] = 0;
                frontier.push_back(q);
            }
        }
        while let Some(q) = frontier.pop_front() {
            for b in self.beside[q] {
                if let Beside::Square(r) = b
                    && self.inside[r] == u16::MAX
                {
                    self.inside[r] = self.inside[q] + 1;
                    frontier.push_back(r);
                }
            }
        }
    }

    fn point(&self, q: usize, corner: usize) -> Vec3 {
        let square = &self.squares[q];
        let [dx, dz] = CORNERS[corner];
        Vec3::new(
            (square.place[0] + dx) as f32 * STEP_M,
            square.corners[corner],
            (square.place[1] + dz) as f32 * STEP_M,
        )
    }

    fn middle(&self, q: usize) -> Vec3 {
        let square = &self.squares[q];
        Vec3::new(
            (square.place[0] as f32 + 0.5) * STEP_M,
            square.middle,
            (square.place[1] as f32 + 0.5) * STEP_M,
        )
    }

    /// One of a square's four triangles, on a side: its middle and the side's two corners.
    fn triangle(&self, q: usize, side: usize) -> [Vec3; 3] {
        [
            self.middle(q),
            self.point(q, side),
            self.point(q, (side + 1) % 4),
        ]
    }

    /// The height of a square's top over a point in it.
    fn height(&self, q: usize, x: f32, z: f32) -> f32 {
        let square = &self.squares[q];
        let (fx, fz) = (
            x / STEP_M - square.place[0] as f32 - 0.5,
            z / STEP_M - square.place[1] as f32 - 0.5,
        );
        let side = if fz.abs() >= fx.abs() {
            if fz < 0.0 { 0 } else { 2 }
        } else if fx > 0.0 {
            1
        } else {
            3
        };
        let [m, a, b] = self.triangle(q, side);
        let p = Vec2::new(x, z);
        let (m2, a2, b2) = (
            Vec2::new(m.x, m.z),
            Vec2::new(a.x, a.z),
            Vec2::new(b.x, b.z),
        );
        let area = (a2 - m2).perp_dot(b2 - m2);
        if area.abs() < 1e-9 {
            return m.y;
        }
        let wa = (p - m2).perp_dot(b2 - m2) / area;
        let wb = (a2 - m2).perp_dot(p - m2) / area;
        m.y + (a.y - m.y) * wa + (b.y - m.y) * wb
    }

    /// The squares over a point, east and south in metres.
    fn over(&self, x: f32, z: f32) -> &[usize] {
        let place = [
            libm::floorf(x / STEP_M) as i32,
            libm::floorf(z / STEP_M) as i32,
        ];
        self.by_place.get(&place).map_or(&[], Vec::as_slice)
    }

    /// The top of the lowest roof over a point that is no lower than `above`, eaves too.
    pub fn top_over(&self, x: f32, z: f32, above: f32) -> Option<f32> {
        self.over(x, z)
            .iter()
            .map(|&q| self.height(q, x, z))
            .filter(|&h| h >= above - 1e-3)
            .min_by(|a, b| a.total_cmp(b))
    }

    /// Where a wall carried up under the lowest roof over a point, whose top is no lower
    /// than `above`, is cut: the roof's deck, the plane its covering is laid on and the
    /// boards under it are drawn on, so a gable closes to the roof with no gap under it.
    pub fn under_over(&self, x: f32, z: f32, above: f32) -> Option<f32> {
        self.top_over(x, z, above)
    }

    /// Where a point is on the roof whose top is nearest it: how far up from its lowest to
    /// its highest, as a fraction, and how far in from an open edge, in metres.
    pub fn place_of(&self, p: Vec3) -> Option<(f32, f32)> {
        self.over(p.x, p.z)
            .iter()
            .map(|&q| (q, (self.height(q, p.x, p.z) - p.y).abs()))
            .filter(|(_, off)| *off < 0.6)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(q, _)| {
                let (low, high) = self.span[self.roof[q]];
                let up = (self.height(q, p.x, p.z) - low) / (high - low).max(0.1);
                (up, f32::from(self.inside[q].min(64)) * STEP_M)
            })
    }

    /// The cell of a square's cube a piece goes up with.
    fn cell(&self, q: usize, stage: u8) -> RoofCell {
        RoofCell {
            cube: self.squares[q].cube,
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
    square: usize,
}

/// Lays a roof kind's roofs: every piece of each square of them of the kit's kind, in the
/// order they go up.
pub(crate) fn lay(base: &Base, kit: &Kit, kits: &Kits, scope: Scope, pieces: &mut Vec<Piece>) {
    if kit.pattern != crate::kit::Pattern::Roof {
        return;
    }
    // Only the roofs with a cube of the kind near the scope, whole, so their long pieces
    // join as they do over the whole base.
    let mine = scope
        .cubes(base, None)
        .iter()
        .any(|(_, cube)| cube.kind == kit.kind());
    if !mine {
        return;
    }
    let roofs = Roofs::whole(base, kits, scope);
    let ours = |q: usize| roofs.squares[q].kind == kit.kind();
    let count = roofs.squares.len();
    let mut segments: Vec<Segment> = Vec::new();
    for q in (0..count).filter(|&q| ours(q)) {
        let triangles: [[Vec3; 3]; 4] = [0, 1, 2, 3].map(|side| roofs.triangle(q, side));
        let centroid = |t: &[Vec3; 3]| Place::near((t[0] + t[1] + t[2]) / 3.0);
        // Its diagonals, between its own triangles.
        for side in 0..4 {
            let (a, b) = (&triangles[side], &triangles[(side + 1) % 4]);
            let (p, r) = (a[0], a[2]);
            if let Some(part) = crease(a, b, p, r) {
                segments.push(Segment {
                    part,
                    from: Place::near(p),
                    to: Place::near(r),
                    sides: [centroid(a), centroid(b)],
                    square: q,
                });
            }
        }
        // Its sides: shared with the square beyond, or an edge of the roof.
        for (side, triangle) in triangles.iter().enumerate() {
            let (step, _, _) = SIDES[side];
            let (p, r) = (triangle[1], triangle[2]);
            let part = match roofs.beside[q][side] {
                Beside::Square(other) => {
                    // Each shared side once, from the square to its west or north.
                    if side == 0 || side == 3 {
                        continue;
                    }
                    let theirs = roofs.triangle(other, (side + 2) % 4);
                    let Some(part) = crease(triangle, &theirs, p, r) else {
                        continue;
                    };
                    segments.push(Segment {
                        part,
                        from: Place::near(p),
                        to: Place::near(r),
                        sides: [centroid(triangle), centroid(&theirs)],
                        square: q,
                    });
                    continue;
                }
                Beside::Against => RoofPart::Flashing,
                Beside::Open if (p.y - r.y).abs() < 1e-4 => RoofPart::Eave,
                Beside::Open => RoofPart::Verge,
            };
            let out = Vec3::new(step[0] as f32, 0.0, step[1] as f32) * STEP_M;
            segments.push(Segment {
                part,
                from: Place::near(p),
                to: Place::near(r),
                sides: [centroid(triangle), Place::near((p + r) * 0.5 + out)],
                square: q,
            });
        }
    }
    let joined = ridges_a_cube_wide(join(segments));
    let r = &kit.roof;
    let (rafter, batten) = (r.rafter * 0.5, r.batten.1);
    // The beams under the creases, then every long piece in its own stage.
    for segment in &joined {
        let cell = roofs.cell(segment.square, segment.part.stage());
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
            // A ridge's beam bears on the gables' inner faces, inside the roof; a hip's
            // and a valley's run on down to the eaves as the rafters do.
            let level = segment.part == RoofPart::Ridge;
            for (from, to) in framed(base, from - down, to - down, level) {
                pieces.push(Piece::Roof {
                    part: RoofPart::Rafter,
                    from: Place::near(from),
                    to: Place::near(to),
                    sides: segment.sides,
                    seed: hash(&[seed, 1]),
                    cell: roofs.cell(segment.square, 1),
                });
            }
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
    rafters(&roofs, &ours, batten + rafter, pieces);
    // Battens and covering, a square at a time, the courses' lower edges level lines a
    // course's rise apart up from each roof's lowest edge.
    let texture = kit
        .textures
        .get(&crate::Material::Covering)
        .map_or(1.0, |texture| texture.metres[1]);
    let exposure = r.exposure(texture);
    // Every slope rises a cube a cube: a course's rise is its exposure up a 45° slope.
    let rise = exposure * std::f32::consts::FRAC_1_SQRT_2;
    let mut battens: Vec<Segment> = Vec::new();
    for q in (0..count).filter(|&q| ours(q)) {
        let eave = roofs.span[roofs.roof[q]].0;
        let corners = [0, 1, 2, 3].map(|k| Place::near(roofs.point(q, k)));
        let open = roofs.beside[q].map(|b| !matches!(b, Beside::Square(_)));
        let courses = (ticks(eave), ticks(rise).max(1));
        pieces.push(Piece::Covering {
            corners,
            middle: Place::near(roofs.middle(q)),
            courses,
            open,
            cell: roofs.cell(q, COVERING),
        });
        // A batten under the foot of every course across the square, each joined to the
        // next square's into one along the face.
        let (base_m, step) = (super::metres(courses.0), super::metres(courses.1));
        for side in 0..4 {
            let t = roofs.triangle(q, side);
            let n = normal(&t);
            let upslope = n.cross(Vec3::Y.cross(n).normalize_or(Vec3::X));
            let low = t.iter().map(|p| p.y).fold(f32::MAX, f32::min);
            let high = t.iter().map(|p| p.y).fold(f32::MIN, f32::max);
            let first = libm::ceilf((low - base_m) / step) as i32;
            let last = libm::floorf((high - base_m) / step) as i32;
            for course in first..=last {
                let Some((p, r)) = level_cut(&t, base_m + course as f32 * step) else {
                    continue;
                };
                if p.distance(r) < 1e-3 {
                    continue;
                }
                let beside = Place::near((p + r) * 0.5 + upslope * 0.1);
                battens.push(Segment {
                    part: RoofPart::Batten,
                    from: Place::near(p),
                    to: Place::near(r),
                    sides: [beside, beside],
                    square: q,
                });
            }
        }
    }
    // Each over the roof's own cubes, from wall to wall: none out under the eaves and
    // verges, where the boards under the covering are all that shows, nor through a gable.
    for segment in join(battens) {
        for (from, to) in framed(base, segment.from.metres(), segment.to.metres(), true) {
            pieces.push(Piece::Roof {
                part: RoofPart::Batten,
                from: Place::near(from),
                to: Place::near(to),
                sides: segment.sides,
                seed: 0,
                cell: roofs.cell(segment.square, 2),
            });
        }
    }
}

/// How far apart a frame's piece is tried along it, in metres, for where it may run.
const FRAME_STEP_M: f32 = 0.02;
/// The shortest piece of frame left once cut, in metres.
const FRAME_SHORTEST_M: f32 = 0.1;

/// Where a piece of a roof's frame from `from` to `to` may run, cut into the pieces between:
/// never inside a wall, a gable carried up into the roof included, so it stops at the
/// wall's face and its end is never seen through it; and a `level` one (a batten, a
/// ridge's beam) only over the roof's own cubes, so it stops inside the roof rather than
/// running on out under a verge to end in the air. Rafters and the beams under hips and
/// valleys, which fall to the eaves, run on out over them as rafters' tails do.
fn framed(base: &Base, from: Vec3, to: Vec3, level: bool) -> Vec<(Vec3, Vec3)> {
    let length = from.distance(to);
    if length < 1e-4 {
        return Vec::new();
    }
    let inside = |p: Vec3| {
        let at = (p / CELL_M).floor().as_ivec3();
        let wall = |at: IVec3| {
            base.cube(at).is_some_and(|c| {
                !c.kind.is_roof() && super::drawn_fill(&c) == Fill::Solid && !c.shape.is_cut()
            })
        };
        let here = base.cube(at);
        // A wall's cube, or a roof's standing on one: the wall carried up into it.
        let in_wall = wall(at) || (here.is_some_and(|c| c.kind.is_roof()) && wall(at - IVec3::Y));
        let over_roof = !level
            || (-1..=1).any(|dy| {
                base.cube(at + IVec3::Y * dy)
                    .is_some_and(|c| c.kind.is_roof() && c.fill != Fill::Open)
            });
        !in_wall && over_roof
    };
    let steps = libm::ceilf(length / FRAME_STEP_M).max(1.0) as usize;
    let at = |t: f32| from.lerp(to, t);
    let t = |i: usize| i as f32 / steps as f32;
    // Where it passes from inside to out between two tries, to a millimetre or so.
    let edge = |mut good: f32, mut bad: f32| {
        for _ in 0..6 {
            let middle = (good + bad) * 0.5;
            if inside(at(middle)) {
                good = middle;
            } else {
                bad = middle;
            }
        }
        good
    };
    let mut out = Vec::new();
    let mut start: Option<f32> = None;
    for i in 0..=steps {
        let ok = inside(at(t(i)));
        match (ok, start) {
            (true, None) => start = Some(if i == 0 { 0.0 } else { edge(t(i), t(i - 1)) }),
            (false, Some(s)) => {
                out.push((at(s), at(edge(t(i - 1), t(i)))));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((at(s), to));
    }
    out.retain(|(a, b)| a.distance(*b) >= FRAME_SHORTEST_M);
    out
}

/// A ridge a cube wide, the flat top of whole cubes along a roof's middle where its span
/// is odd, capped as one: its two ridges, level, as long as each other, a cube apart and at
/// one height, are one cap along its middle as wide as it, over the slopes either side.
fn ridges_a_cube_wide(mut segments: Vec<Segment>) -> Vec<Segment> {
    let wide = super::TICKS;
    let mut gone = vec![false; segments.len()];
    let mut merged = Vec::new();
    for i in 0..segments.len() {
        let a = &segments[i];
        if gone[i] || a.part != RoofPart::Ridge || a.from.0[1] != a.to.0[1] {
            continue;
        }
        for j in i + 1..segments.len() {
            let b = &segments[j];
            if gone[j] || b.part != RoofPart::Ridge || b.from.0[1] != a.from.0[1] {
                continue;
            }
            let across = IVec3::from_array(b.from.0) - IVec3::from_array(a.from.0);
            let along = IVec3::from_array(a.to.0) - IVec3::from_array(a.from.0);
            let same = IVec3::from_array(b.to.0) - IVec3::from_array(b.from.0) == along;
            let square = across.y == 0
                && across.x * along.x + across.z * along.z == 0
                && across.x.abs() + across.z.abs() == wide;
            if !(same && square) {
                continue;
            }
            // The side of each away from the other.
            let away = |s: &Segment, from: IVec3| {
                *s.sides
                    .iter()
                    .max_by_key(|p| {
                        let d = IVec3::from_array(p.0) - from;
                        (d.x * across.x + d.z * across.z).abs()
                    })
                    .expect("two sides")
            };
            let middle =
                |p: Place, q: Place| Place([0, 1, 2].map(|k| (p.0[k] + q.0[k]).div_euclid(2)));
            merged.push(Segment {
                part: RoofPart::Ridge,
                from: middle(a.from, b.from),
                to: middle(a.to, b.to),
                sides: [
                    away(a, IVec3::from_array(b.from.0)),
                    away(b, IVec3::from_array(a.from.0)),
                ],
                square: a.square,
            });
            gone[i] = true;
            gone[j] = true;
            break;
        }
    }
    let mut out: Vec<Segment> = segments
        .drain(..)
        .zip(gone)
        .filter(|(_, gone)| !gone)
        .map(|(segment, _)| segment)
        .collect();
    out.extend(merged);
    out
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
    // By what each is, the line it is on in plan and its height where that line crosses
    // the plan's origin, then along it. Heights are to the nearest tick, so a line's slope
    // is compared, not its direction in ticks.
    type Line = (RoofPart, [i64; 2], i64, i64);
    let slope = |s: &Segment| {
        let (a, b) = (s.from.metres(), s.to.metres());
        (b.y - a.y) / Vec2::new(b.x - a.x, b.z - a.z).length().max(1e-6)
    };
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
        // Its height where it would cross the line's start, to the centimetre.
        let along =
            a.as_vec2().dot(d.as_vec2().normalize_or_zero()) * (CELL_M / super::TICKS as f32);
        let level =
            libm::roundf((segment.from.metres().y - slope(&segment) * along) * 100.0) as i64;
        let key = (segment.part, d.to_array(), a.perp_dot(d), level);
        lines
            .entry(key)
            .or_default()
            .push((a.dot(d), b.dot(d), segment));
    }
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

/// The rafters: one up every slope along every cube's edge, from its foot at the eaves to
/// the ridge, hip or valley it meets, under the battens.
fn rafters(roofs: &Roofs, ours: &dyn Fn(usize) -> bool, below: f32, pieces: &mut Vec<Piece>) {
    let mut segments = Vec::new();
    for q in (0..roofs.squares.len()).filter(|&q| ours(q)) {
        let place = roofs.squares[q].place;
        // Its east side on a cube's edge, a line north and south; its south side, east and
        // west.
        for (side, on_edge, along) in [
            (1, (place[0] + 1).rem_euclid(2) == 0, 2),
            (2, (place[1] + 1).rem_euclid(2) == 0, 0),
        ] {
            let Beside::Square(other) = roofs.beside[q][side] else {
                continue;
            };
            if !on_edge || !ours(other) {
                continue;
            }
            let across = 2 - along;
            // None along a verge: where a square either side is carried out past the edge
            // of its cube across the slope, the line is a gable's face or out beyond it,
            // where a rafter would lie on the wall or hang in the air under the verge.
            let past = |q: usize| {
                let square = &roofs.squares[q];
                let (place, cube) = if across == 0 {
                    (square.place[0], square.cube.x)
                } else {
                    (square.place[1], square.cube.z)
                };
                place.div_euclid(2) != cube
            };
            if past(q) || past(other) {
                continue;
            }
            let mine = roofs.triangle(q, side);
            let theirs = roofs.triangle(other, (side + 2) % 4);
            // Each falls along the line, the same way.
            let fall = |n: Vec3| {
                (n[along].abs() > n[across].abs() + 1e-4 && n.y < 1.0 - 1e-4)
                    .then_some(n[along] > 0.0)
            };
            let n = normal(&mine);
            match (fall(n), fall(normal(&theirs))) {
                (Some(a), Some(b)) if a == b => {}
                _ => continue,
            }
            let down = n * below;
            let centre = Place::near((mine[0] + mine[1] + mine[2]) / 3.0);
            segments.push(Segment {
                part: RoofPart::Rafter,
                from: Place::near(mine[1] - down),
                to: Place::near(mine[2] - down),
                sides: [centre, centre],
                square: q,
            });
        }
    }
    for segment in join(segments) {
        let seed = hash(&[
            segment.from.0[0] as u32,
            segment.from.0[2] as u32,
            segment.from.0[1] as u32,
            3,
        ]);
        pieces.push(Piece::Roof {
            part: RoofPart::Rafter,
            from: segment.from,
            to: segment.to,
            sides: segment.sides,
            seed,
            cell: roofs.cell(segment.square, 1),
        });
    }
}

/// A block kind's cubes carried up into the roof over its walls (`Piece::Gable`): every
/// roof cube standing on one of its whole cubes, its surface cut at the roof's underside, as
/// a gable end rises to its verges and a wall's top closes the wedge under its eaves.
pub(crate) fn gables(base: &Base, kit: &Kit, scope: Scope, pieces: &mut Vec<Piece>) {
    for (at, cube) in scope.cubes(base, None) {
        if !cube.kind.is_roof() || cube.fill == Fill::Open {
            continue;
        }
        let under_it = base.cube(at - IVec3::Y);
        if !under_it.is_some_and(|c| {
            c.kind == kit.kind() && super::drawn_fill(&c) == Fill::Solid && !c.shape.is_cut()
        }) {
            continue;
        }
        if let Some(under) = underside(at, cube) {
            pieces.push(Piece::Gable { cube: at, under });
        }
    }
}

/// The underside of a roof cube's roof over its corners, the middles of its sides and its
/// middle, west to east and north to south, in ticks, if any of it is over the cube's
/// bottom: what a wall carried up into it is cut at.
pub(crate) fn underside(at: IVec3, cube: Cube) -> Option<[[i32; 3]; 3]> {
    // Up to the roof's deck, the plane its covering lies on: the boards under the roof, its
    // eaves and its verges are drawn on it, so the wall meets them in one straight line.
    let under = |u: f32, v: f32| ticks((at.y as f32 + surface(cube.shape, u, v)) * CELL_M);
    let rows = [0.0, 0.5, 1.0].map(|v| [0.0, 0.5, 1.0].map(|u| under(u, v)));
    let highest = rows.iter().flatten().copied().max().unwrap_or(i32::MIN);
    (super::metres(highest) > at.y as f32 * CELL_M + 0.02).then_some(rows)
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
    use crate::kit::Pattern;
    use crate::roofer::{self, Style};
    use crate::{BuildOp, STOREY_M};

    const ROOF: Kind = Kind::FIRST_ROOF;

    fn roof_kit() -> Kit {
        Kit {
            id: ROOF.0,
            pattern: Pattern::Roof,
            ..Kit::default()
        }
    }

    /// A room of plain walls over a footprint of rectangles, roofed by the roofer in a
    /// style.
    fn roofed(rooms: &[([i32; 3], [i32; 3])], style: Style) -> Base {
        let mut base = Base::default();
        let mut footprint = BTreeSet::new();
        for &(min, max) in rooms {
            base.apply(BuildOp::AddRoom {
                min,
                max,
                kind: Kind::PLAIN,
            });
            for x in min[0]..=max[0] {
                for z in min[2]..=max[2] {
                    footprint.insert([x, z]);
                }
            }
        }
        // The flat roof the room tool lays, off, the walls' top layer left under the roof,
        // as "add a roof" does.
        for &[x, z] in &footprint {
            if base.filling(IVec3::new(x, 0, z)).is_none() {
                base.apply(BuildOp::Fill {
                    min: [x, 5, z],
                    max: [x, 5, z],
                    cube: None,
                });
            }
        }
        let plan = roofer::over(&footprint, 6, style, None);
        for (at, shape) in plan.roof {
            base.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: Some(Cube {
                    shape,
                    ..Cube::new(ROOF, Fill::Solid)
                }),
            });
        }
        for at in plan.gables {
            base.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: Some(Cube::new(Kind::PLAIN, Fill::Solid)),
            });
        }
        base
    }

    fn laid(base: &Base) -> Vec<Piece> {
        let kit = roof_kit();
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
    fn a_gable_of_cubes_has_one_ridge_two_eaves_and_four_verges() {
        // Five metres by four, the ridge along X two metres in, rising two metres.
        let base = roofed(&[([0, 0, 0], [9, 0, 7])], Style::Gable);
        let pieces = laid(&base);
        let ridges = parts(&pieces, RoofPart::Ridge);
        assert_eq!(ridges.len(), 1, "{ridges:?}");
        let (a, b) = ridges[0];
        let top = STOREY_M + 2.0;
        // From verge to verge, over the gables' overhang.
        assert!(
            close(a, Vec3::new(-0.5, top, 2.0)) && close(b, Vec3::new(5.5, top, 2.0)),
            "{a} {b}"
        );
        assert_eq!(parts(&pieces, RoofPart::Eave).len(), 2);
        assert_eq!(parts(&pieces, RoofPart::Verge).len(), 4);
        assert!(parts(&pieces, RoofPart::Hip).is_empty());
        assert!(parts(&pieces, RoofPart::Valley).is_empty());
        // The eaves overhang by half a metre, carried on down the slope.
        for (a, b) in parts(&pieces, RoofPart::Eave) {
            assert!(
                (a.y - (STOREY_M - 0.5)).abs() < 0.01 && a.y == b.y,
                "{a} {b}"
            );
            assert!((a.z + 0.5).abs() < 0.01 || (a.z - 4.5).abs() < 0.01, "{a}");
        }
        // A covering over every square, eaves and all, each with its stage.
        let squares = pieces
            .iter()
            .filter(|p| matches!(p, Piece::Covering { .. }))
            .count();
        assert_eq!(squares, 24 * 20);
        for piece in &pieces {
            let (stage, want) = match piece {
                Piece::Covering { cell, .. } => (cell.stage, COVERING),
                Piece::Roof { part, cell, .. } => (cell.stage, part.stage()),
                _ => continue,
            };
            assert_eq!(stage, want, "{piece:?}");
            // Every piece waits on a roof cube.
            let cube = piece.roof_cell().unwrap().cube;
            assert!(base.cube(cube).is_some_and(|c| c.kind == ROOF), "{piece:?}");
        }
        // Rafters along every cube's edge up each slope, nine a side from one gable's inner
        // face to the other's, none on a gable or out under a verge, and a beam under the ridge.
        assert_eq!(parts(&pieces, RoofPart::Rafter).len(), 2 * 9 + 1);
    }

    #[test]
    fn a_hip_of_cubes_has_four_hips_and_a_short_ridge() {
        let base = roofed(&[([0, 0, 0], [9, 0, 7])], Style::Hip);
        let pieces = laid(&base);
        let hips = parts(&pieces, RoofPart::Hip);
        assert_eq!(hips.len(), 4, "{hips:?}");
        let ridges = parts(&pieces, RoofPart::Ridge);
        assert_eq!(ridges.len(), 1);
        let (a, b) = ridges[0];
        // A metre long: the five metres less two metres' run at each end.
        assert!((a.distance(b) - 1.0).abs() < 0.01, "{a} {b}");
        assert!(parts(&pieces, RoofPart::Verge).is_empty());
        assert_eq!(parts(&pieces, RoofPart::Eave).len(), 4);
        // Hips run from the eaves' corners, past the walls, up to the ridge's ends.
        for (a, b) in hips {
            let (low, high) = if a.y < b.y { (a, b) } else { (b, a) };
            assert!((low.y - (STOREY_M - 0.5)).abs() < 0.01, "{low}");
            assert!(
                close(high, ridges[0].0) || close(high, ridges[0].1),
                "{high}"
            );
        }
    }

    #[test]
    fn an_l_of_cubes_has_a_valley_where_its_wings_meet_and_eaves_that_meet_in_it() {
        let base = roofed(
            &[([0, 0, 0], [11, 0, 7]), ([0, 0, 7], [7, 0, 15])],
            Style::Hip,
        );
        let pieces = laid(&base);
        let valleys = parts(&pieces, RoofPart::Valley);
        assert_eq!(valleys.len(), 1, "{valleys:?}");
        // From the inside corner of the L's eaves, past the walls, up to the hips.
        let (a, b) = valleys[0];
        let low = if a.y < b.y { a } else { b };
        assert!(close(low, Vec3::new(4.5, STOREY_M - 0.5, 4.5)), "{low}");
        // Every eave is one board, the inside corner's two meeting in the valley.
        assert_eq!(parts(&pieces, RoofPart::Eave).len(), 6);
    }

    #[test]
    fn an_odd_span_is_capped_by_one_ridge_down_its_flat_middle() {
        let base = roofed(&[([0, 0, 0], [9, 0, 8])], Style::Gable);
        let pieces = laid(&base);
        let ridges = parts(&pieces, RoofPart::Ridge);
        assert_eq!(ridges.len(), 1, "{ridges:?}");
        assert!((ridges[0].0.z - 2.25).abs() < 0.01, "{ridges:?}");
    }

    #[test]
    fn a_roof_against_a_taller_wall_is_flashed_and_its_cubes_hold_by_their_ramp() {
        // A lean-to rising south to a wall two storeys high.
        let mut base = roofed(&[([0, 0, 0], [9, 0, 5])], Style::LeanTo);
        // Its own gables and high side off, and the tall wall where its high side was.
        base.apply(BuildOp::Fill {
            min: [-2, 0, 6],
            max: [11, 13, 6],
            cube: Some(Cube::new(Kind::PLAIN, Fill::Solid)),
        });
        let pieces = laid(&base);
        let flashing = parts(&pieces, RoofPart::Flashing);
        assert_eq!(flashing.len(), 1, "{flashing:?}");
        let (a, b) = flashing[0];
        assert!(
            (a.z - 3.0).abs() < 0.01 && (b.z - 3.0).abs() < 0.01,
            "{a} {b}"
        );
        assert!(base.floating().is_empty());
    }

    #[test]
    fn a_storey_built_over_a_roof_stops_it_and_a_second_roof_over_it_is_apart() {
        let mut base = roofed(&[([0, 0, 0], [9, 0, 7])], Style::Gable);
        // A second roof two storeys up over the same footprint, by itself.
        let upper = roofer::over(
            &(0..10).flat_map(|x| (0..8).map(move |z| [x, z])).collect(),
            18,
            Style::Hip,
            None,
        );
        for (at, shape) in upper.roof {
            base.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: Some(Cube {
                    shape,
                    ..Cube::new(ROOF, Fill::Solid)
                }),
            });
        }
        let pieces = laid(&base);
        assert_eq!(parts(&pieces, RoofPart::Hip).len(), 4);
        assert_eq!(parts(&pieces, RoofPart::Ridge).len(), 2);
    }

    #[test]
    fn a_stone_gable_rises_into_the_roof_cubes_over_it() {
        let stone = Kit {
            id: 2,
            pattern: Pattern::Masonry,
            ..Kit::default()
        };
        let mut base = roofed(&[([0, 0, 0], [9, 0, 7])], Style::Gable);
        // Its walls of stone, the gables too.
        let walls: Vec<IVec3> = base
            .cubes()
            .filter(|(_, c)| c.kind == Kind::PLAIN)
            .map(|(at, _)| at)
            .collect();
        for at in walls {
            base.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: Some(Cube::new(stone.kind(), Fill::Solid)),
            });
        }
        let mut pieces = Vec::new();
        gables(&base, &stone, Scope::ALL, &mut pieces);
        // Over the west gable's middle, the roof cube at its top, cut at the underside.
        let tops: Vec<IVec3> = pieces
            .iter()
            .filter_map(|p| match *p {
                Piece::Gable { cube, .. } => Some(cube),
                _ => None,
            })
            .collect();
        assert!(tops.contains(&IVec3::new(0, 9, 3)), "{tops:?}");
        for piece in &pieces {
            if let Piece::Gable { cube, under } = piece {
                assert!(base.cube(*cube).is_some_and(|c| c.kind.is_roof()));
                let highest = super::super::metres(*under.iter().flatten().max().unwrap());
                assert!(
                    highest > cube.y as f32 * CELL_M
                        && highest <= (cube.y + 1) as f32 * CELL_M + 1e-3
                );
            }
        }
        // Cut along the roof's deck, the plane its covering lies on, with no gap under it:
        // up the west gable's face the eaves are a storey up, rising a metre a metre to the
        // ridge two metres in.
        let deck = |z: f32| STOREY_M + z.min(4.0 - z);
        let mut tried = 0;
        for piece in &pieces {
            let Piece::Gable { cube, under } = piece else {
                continue;
            };
            if cube.x != 0 {
                continue;
            }
            for k in 0..=10 {
                let z = (cube.z as f32 + k as f32 / 10.0) * CELL_M;
                let want = deck(z);
                // Only where the deck crosses this cube, not the one over or under it.
                if want < cube.y as f32 * CELL_M || want > (cube.y + 1) as f32 * CELL_M {
                    continue;
                }
                let cut = gable_cut(*cube, under, Vec3::new(0.0, 0.0, z));
                assert!(
                    (cut - want).abs() < 0.005,
                    "{cube} at {z}: cut at {cut}, deck {want}"
                );
                tried += 1;
            }
        }
        assert!(tried > 20, "{tried}");
    }

    /// Every point along a piece of a roof's frame, a rafter, a beam or a batten.
    fn frame_points(pieces: &[Piece]) -> Vec<(Vec3, bool)> {
        pieces
            .iter()
            .filter_map(|p| match *p {
                Piece::Roof {
                    part: RoofPart::Rafter | RoofPart::Batten,
                    from,
                    to,
                    ..
                } => Some((from.metres(), to.metres())),
                _ => None,
            })
            .flat_map(|(a, b)| {
                let level = (a.y - b.y).abs() < 1e-3;
                (0..=20).map(move |i| (a.lerp(b, i as f32 / 20.0), level))
            })
            .collect()
    }

    #[test]
    fn no_piece_of_a_frame_crosses_a_gable_or_ends_in_the_air_past_it() {
        // Five metres by four, the gables' outer faces at x = 0 and 5, their inner faces
        // half a metre in.
        let base = roofed(&[([0, 0, 0], [9, 0, 7])], Style::Gable);
        let points = frame_points(&laid(&base));
        assert!(points.len() > 100);
        for (p, _) in points {
            assert!(
                (0.5 - 1e-3..=4.5 + 1e-3).contains(&p.x),
                "a piece of frame at {p}, in a gable or out past it"
            );
        }
    }

    #[test]
    fn a_level_piece_of_frame_runs_only_over_the_rooms_of_any_plan() {
        // An L, a T and a U of gables: no batten or ridge beam in a wall, gable or not, nor
        // out past the walls under an eave or a verge.
        for shape in ["L", "T", "U"] {
            let (rooms, _) = crate::roof_plan(shape, Style::Gable, Style::Gable);
            let rooms: Vec<([i32; 3], [i32; 3])> = rooms.to_vec();
            let base = roofed(&rooms, Style::Gable);
            // In the plan, and not in its outer walls: every column round it in the plan too.
            let planned = |x: i32, z: i32| {
                rooms.iter().any(|(min, max)| {
                    (min[0]..=max[0]).contains(&x) && (min[2]..=max[2]).contains(&z)
                })
            };
            let inside =
                |x: i32, z: i32| (-1..=1).all(|dx| (-1..=1).all(|dz| planned(x + dx, z + dz)));
            let points = frame_points(&laid(&base));
            assert!(points.iter().any(|(_, level)| *level), "{shape}");
            for (p, level) in points {
                let (x, z) = (
                    libm::floorf(p.x / CELL_M) as i32,
                    libm::floorf(p.z / CELL_M) as i32,
                );
                // On a cube's edge, either side will do.
                let near = |a: f32| (a / CELL_M - libm::roundf(a / CELL_M)).abs() < 1e-3;
                let xs = if near(p.x) { vec![x - 1, x] } else { vec![x] };
                let zs = if near(p.z) { vec![z - 1, z] } else { vec![z] };
                let over_a_room = xs.iter().any(|&x| zs.iter().any(|&z| inside(x, z)));
                assert!(
                    !level || over_a_room,
                    "{shape}: a level piece of frame at {p}"
                );
            }
        }
    }

    #[test]
    fn a_point_under_a_roof_finds_its_underside() {
        let base = roofed(&[([0, 0, 0], [9, 0, 7])], Style::Gable);
        let kit = roof_kit();
        let roofs = Roofs::whole(&base, &Kits(vec![kit]), Scope::ALL);
        // A metre in from the eave, a metre up the slope.
        let top = roofs.top_over(2.0, 1.0, 0.0).unwrap();
        assert!((top - (STOREY_M + 1.0)).abs() < 1e-3, "{top}");
        // Past the eave, the overhang.
        let eave = roofs.top_over(2.0, -0.25, 0.0).unwrap();
        assert!((eave - (STOREY_M - 0.25)).abs() < 1e-3, "{eave}");
        let (up, inside) = roofs.place_of(Vec3::new(2.0, 4.99, 2.0 - 0.01)).unwrap();
        assert!(up > 0.9 && inside > 1.0, "{up} {inside}");
    }
}
