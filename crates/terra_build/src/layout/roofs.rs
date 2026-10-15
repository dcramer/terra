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
    /// Where the roof rises to something standing on or beside it along a level line: an
    /// apron, as under a chimney or along a lean-to's head.
    Flashing,
    /// Where the roof meets something standing on or beside it along a sloping line: step
    /// flashing over soakers, up a chimney's or a dormer cheek's sides.
    Step,
    /// Where the roof falls to something standing on or beside it along a level line: a
    /// back gutter, above a chimney.
    Gutter,
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
    /// Whether something not a roof's stands on it ([`standing_on`]): the covering runs on
    /// under it, but no cap or edge of the roof is drawn there and its foot is flashed.
    covered: bool,
}

/// The cube standing on a roof's cube, if one does: something not a roof's, in, with its
/// foot on the roof cube's top (not an upper half or an upside-down shape hung over it). It
/// is carried down into the roof's cube to the roof's deck, and the roof is flashed round
/// its foot (docs/building-plan.md, where roofs and walls meet).
pub(crate) fn standing_on(base: &Base, roof: IVec3) -> Option<Cube> {
    base.cube(roof + IVec3::Y).filter(|over| {
        !over.kind.is_roof()
            && over.level() > 0
            && super::drawn_fill(over) == Fill::Solid
            && !over.shape.planed_under()
            && !matches!(over.shape, Shape::InvertedStair(_))
    })
}

/// Whether a roof cube is part of a roof's surface: in it, and with no roof cube over it
/// that has anything of it in, which is the roof there instead.
fn on_top(base: &Base, at: IVec3, cube: &Cube) -> bool {
    cube.fill != Fill::Open
        && !base
            .cube(at + IVec3::Y)
            .is_some_and(|over| over.kind.is_roof() && over.fill != Fill::Open && over.level() > 0)
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
    /// Its fingerprint, once asked for.
    fingerprinted: std::cell::OnceCell<u64>,
}

thread_local! {
    /// The roofs worked out while laying is [`remembered`], by the base and the roof cubes.
    static MEMO: std::cell::RefCell<Option<QuickMap<u64, std::rc::Rc<Roofs>>>> =
        const { std::cell::RefCell::new(None) };
}

/// Runs a laying of one base, every roof it asks for ([`Roofs::whole`]) worked out once,
/// and forgotten after.
pub(crate) fn remembered<T>(run: impl FnOnce() -> T) -> T {
    let outer = MEMO.with(|memo| {
        let mut memo = memo.borrow_mut();
        let outer = memo.is_some();
        if !outer {
            *memo = Some(QuickMap::default());
        }
        outer
    });
    let out = run();
    if !outer {
        MEMO.with(|memo| *memo.borrow_mut() = None);
    }
    out
}

impl Roofs {
    /// The roofs of a base's roof cubes within `near` and every roof cube joined to them,
    /// whole, so every piece of them is laid as laying the whole base would. Worked out once
    /// for the same roofs while laying is [`remembered`]: every roof kit's layout, and every
    /// log kit's cut under them, asks for the same.
    pub(crate) fn whole(base: &Base, kits: &Kits, near: Scope) -> std::rc::Rc<Roofs> {
        let mut found: BTreeSet<[i32; 3]> = BTreeSet::new();
        let mut frontier: Vec<IVec3> = near
            .cubes_where(base, None, |cube| cube.kind.is_roof())
            .into_iter()
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
        let key = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            (base as *const Base as usize).hash(&mut hasher);
            (kits as *const Kits as usize).hash(&mut hasher);
            found.hash(&mut hasher);
            hasher.finish()
        };
        if let Some(known) = MEMO.with(|memo| memo.borrow().as_ref()?.get(&key).cloned()) {
            return known;
        }
        let cubes: Vec<(IVec3, Cube)> = found
            .into_iter()
            .filter_map(|at| {
                let at = IVec3::from_array(at);
                base.cube(at).map(|cube| (at, cube))
            })
            .collect();
        let roofs = std::rc::Rc::new(Roofs::of(base, kits, &cubes));
        MEMO.with(|memo| {
            if let Some(memo) = memo.borrow_mut().as_mut() {
                memo.insert(key, roofs.clone());
            }
        });
        roofs
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
            fingerprinted: std::cell::OnceCell::new(),
        };
        // The top of every roof cube with no roof over it, whatever else stands on it: the
        // roof runs on under a wall standing on it, as a real roof's deck does under a
        // chimney's or a dormer's cheek, so no wall's shape can open it.
        for &(at, cube) in cubes {
            if !on_top(base, at, &cube) {
                continue;
            }
            let covered = standing_on(base, at).is_some();
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
                    covered,
                });
            }
        }
        roofs.index();
        roofs.link(base);
        // Carried on past every open edge by its kit's eaves, each square of it on the
        // planes of the cube it runs on from; where two run on into the same square, the
        // higher, so eaves meeting round an inside corner make a valley.
        let real = roofs.squares.len();
        // By place, each roof's apart: a storey's roof's eaves over a roof under it, a
        // storey up, are not that one's.
        let mut past: BTreeMap<[i32; 2], Vec<Square>> = BTreeMap::new();
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
                    covered: false,
                };
                let here = past.entry(place).or_default();
                match here
                    .iter_mut()
                    .find(|there| (there.middle - candidate.middle).abs() < 1.0)
                {
                    Some(there) => {
                        if candidate.middle > there.middle + 1e-4 {
                            there.cube = candidate.cube;
                            there.kind = candidate.kind;
                        }
                        for k in 0..4 {
                            there.corners[k] = there.corners[k].max(candidate.corners[k]);
                        }
                        there.middle = there.middle.max(candidate.middle);
                    }
                    None => here.push(candidate),
                }
            }
        }
        roofs.squares.extend(past.into_values().flatten());
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
                    // Or a hole in the roof, the roof going on beyond it (docs/building-voxels.md,
                    // gap 5): its edge flashed round, as at something standing beside the
                    // roof, and no eaves carried into it. Not past an eave's own squares.
                    let real = [square.place[0].div_euclid(2), square.place[1].div_euclid(2)]
                        == [square.cube.x, square.cube.z];
                    let across = IVec3::new(step[0], 0, step[1]);
                    let hole = real
                        && (1..=HOLE_ACROSS).any(|k| {
                            (-k - 1..=k + 1).any(|dy| {
                                base.cube(cube + across * k + IVec3::Y * dy)
                                    .is_some_and(|c| c.kind.is_roof() && c.fill != Fill::Open)
                            })
                        });
                    if base.cube(cube).is_some_and(|c| c.fill != Fill::Open) || hole {
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

    /// The top of the highest roof under a point that is no higher than `below`, eaves too:
    /// the roof a piece over a point lies on, where one roof's eaves pass over another's.
    pub fn top_under(&self, x: f32, z: f32, below: f32) -> Option<f32> {
        self.over(x, z)
            .iter()
            .map(|&q| self.height(q, x, z))
            .filter(|&h| h <= below + 1e-3)
            .max_by(|a, b| a.total_cmp(b))
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

    /// A number for everything a point's place on the roofs depends on ([`Roofs::place_of`]):
    /// every square, how far in from an edge each is, and each roof's span.
    pub(crate) fn fingerprint(&self) -> u64 {
        *self.fingerprinted.get_or_init(|| self.fingerprint_of())
    }

    fn fingerprint_of(&self) -> u64 {
        use std::hash::Hasher;
        let mut hash = crate::mesh::Corners::default();
        for (q, square) in self.squares.iter().enumerate() {
            hash.write_i32(square.place[0]);
            hash.write_i32(square.place[1]);
            for corner in square.corners {
                hash.write_u32(corner.to_bits());
            }
            hash.write_u32(square.middle.to_bits());
            hash.write_u16(self.inside[q]);
            hash.write_usize(self.roof[q]);
        }
        for (low, high) in &self.span {
            hash.write_u32(low.to_bits());
            hash.write_u32(high.to_bits());
        }
        hash.finish()
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

/// Which flashing a line where the covering meets something standing is, by the line and
/// the roof's triangle on its open side: a step up a sloping line, else an apron where the
/// roof rises to it and a back gutter where it falls to it.
fn flashing(open: &[Vec3; 3]) -> RoofPart {
    let [middle, p, r] = *open;
    if (p.y - r.y).abs() > 1e-4 {
        RoofPart::Step
    } else if middle.y > p.y + 1e-4 {
        RoofPart::Gutter
    } else {
        RoofPart::Flashing
    }
}

/// Whether a part is flashing of any sort, where the covering meets something standing.
pub fn is_flashing(part: RoofPart) -> bool {
    matches!(part, RoofPart::Flashing | RoofPart::Step | RoofPart::Gutter)
}

/// A line's ends, a flashing's pulled back where it meets another in an inside corner, both
/// lying on the roof toward one another: the one more along the north and south butts the
/// other's side, short of the corner by its width, so the two are not laid one in the other.
/// Round an outside corner each lies on its own side and stops at the corner.
fn flashing_butted(segment: &Segment, all: &[Segment], kit: &Kit) -> (Place, Place) {
    let (from, to) = (segment.from.metres(), segment.to.metres());
    if !is_flashing(segment.part) {
        return (segment.from, segment.to);
    }
    let along = to - from;
    let plan = along.with_y(0.0);
    if plan.length() < 1e-4 || plan.z.abs() <= plan.x.abs() {
        return (segment.from, segment.to);
    }
    let wide = kit
        .roof
        .parts
        .get(&segment.part)
        .or_else(|| kit.roof.parts.get(&RoofPart::Flashing))
        .and_then(|part| kit.dress.made.as_deref()?.modules.get(&part.module))
        .map_or(0.2, |module| module.size()[1]);
    // The way onto the roof from the line, level.
    let onto = (segment.sides[0].metres() - from)
        .reject_from(along)
        .with_y(0.0)
        .normalize_or_zero();
    let butted = |end: Vec3, other_end: Vec3| -> Vec3 {
        let meets = all.iter().any(|other| {
            if std::ptr::eq(other, segment) || !is_flashing(other.part) {
                return false;
            }
            let (a, b) = (other.from.metres(), other.to.metres());
            let way = (b - a).with_y(0.0).normalize_or_zero();
            if way.dot(plan.normalize_or_zero()).abs() > 0.5 {
                return false;
            }
            // It runs from this end out over this one's roof.
            let away = if a.distance(end) < 0.01 {
                way
            } else if b.distance(end) < 0.01 {
                -way
            } else {
                return false;
            };
            away.dot(onto) > 0.5
        });
        if meets {
            end + (other_end - end).normalize_or_zero() * (wide * along.length() / plan.length())
        } else {
            end
        }
    };
    (Place::near(butted(from, to)), Place::near(butted(to, from)))
}

/// How wide a flashing's board may lie across the roof from its line, in ticks, where the
/// roof folds away under it within its modelled width (over a hip or a ridge beside what
/// it flashes), so it lies on the roof rather than standing off it: as far as the roof keeps
/// to the plane it lies on. None where the whole width lies on it.
fn flashing_wide(roofs: &Roofs, segment: &Segment, kit: &Kit) -> Option<i32> {
    let wide = kit
        .roof
        .parts
        .get(&segment.part)
        .or_else(|| kit.roof.parts.get(&RoofPart::Flashing))
        .and_then(|part| kit.dress.made.as_deref()?.modules.get(&part.module))
        .map(|module| module.size()[1])?;
    let (from, to) = (segment.from.metres(), segment.to.metres());
    let side = segment.sides[0].metres();
    let n = planar_normal(from, to, side);
    let onto = (side - from)
        .reject_from(to - from)
        .with_y(0.0)
        .normalize_or_zero();
    if onto == Vec3::ZERO || n.y < 0.2 {
        return None;
    }
    // The roof's plane at a point level from a point of the line.
    let plane = |p: Vec3, q: Vec3| p.y - (n.x * (q.x - p.x) + n.z * (q.z - p.z)) / n.y;
    let mut reach = wide;
    for t in [0.1, 0.5, 0.9] {
        let p = from.lerp(to, t);
        let mut d = FLASHING_STEP_M;
        while d <= wide {
            let q = p + onto * d;
            let falls = roofs
                .top_over(q.x, q.z, p.y - CELL_M)
                .is_none_or(|top| top < plane(p, q) - FLASHING_LIFT_M);
            if falls {
                reach = reach.min(d - FLASHING_STEP_M);
                break;
            }
            d += FLASHING_STEP_M;
        }
    }
    (reach < wide - 1e-4).then(|| ticks(reach.max(FLASHING_NARROWEST_M)))
}

/// How finely a flashing's board is fitted to where the roof folds away under it.
const FLASHING_STEP_M: f32 = 0.02;
/// How far the roof may fall away under a flashing's board before it stands off it.
const FLASHING_LIFT_M: f32 = 0.005;
/// The narrowest a flashing's board is laid.
const FLASHING_NARROWEST_M: f32 = 0.04;

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
    let mine = !scope
        .cubes_where(base, None, |cube| cube.kind == kit.kind())
        .is_empty();
    if !mine {
        return;
    }
    let roofs = Roofs::whole(base, kits, scope);
    // Only the squares near the scope: a long piece is joined only within a section
    // (`join`), so none further off is part of one laid in it.
    let near = |q: usize| {
        let at = roofs.squares[q].cube;
        at.x >= scope.low.x - NEAR_SCOPE
            && at.x <= scope.high.x + NEAR_SCOPE
            && at.z >= scope.low.z - NEAR_SCOPE
            && at.z <= scope.high.z + NEAR_SCOPE
    };
    let ours = |q: usize| roofs.squares[q].kind == kit.kind() && near(q);
    let count = roofs.squares.len();
    let mut segments: Vec<Segment> = Vec::new();
    // Creases under something standing on the roof: the frame under them runs on, as it
    // carries what stands there, but their caps do not.
    let mut hidden: Vec<Segment> = Vec::new();
    for q in (0..count).filter(|&q| ours(q)) {
        let triangles: [[Vec3; 3]; 4] = [0, 1, 2, 3].map(|side| roofs.triangle(q, side));
        let centroid = |t: &[Vec3; 3]| Place::near((t[0] + t[1] + t[2]) / 3.0);
        let covered = roofs.squares[q].covered;
        // Its diagonals, between its own triangles.
        for side in 0..4 {
            let (a, b) = (&triangles[side], &triangles[(side + 1) % 4]);
            let (p, r) = (a[0], a[2]);
            if let Some(part) = crease(a, b, p, r) {
                let segment = Segment {
                    part,
                    from: Place::near(p),
                    to: Place::near(r),
                    sides: [centroid(a), centroid(b)],
                    square: q,
                };
                if covered {
                    hidden.push(segment);
                } else {
                    segments.push(segment);
                }
            }
        }
        // Its sides: shared with the square beyond, or an edge of the roof.
        for (side, triangle) in triangles.iter().enumerate() {
            let (step, _, _) = SIDES[side];
            let out = Vec3::new(step[0] as f32, 0.0, step[1] as f32) * STEP_M;
            let (p, r) = (triangle[1], triangle[2]);
            let part = match roofs.beside[q][side] {
                Beside::Square(other) => {
                    // Each shared side once, from the square to its west or north.
                    if side == 0 || side == 3 {
                        continue;
                    }
                    let theirs = roofs.triangle(other, (side + 2) % 4);
                    let crease = crease(triangle, &theirs, p, r).map(|part| Segment {
                        part,
                        from: Place::near(p),
                        to: Place::near(r),
                        sides: [centroid(triangle), centroid(&theirs)],
                        square: q,
                    });
                    match (covered, roofs.squares[other].covered) {
                        (false, false) => segments.extend(crease),
                        (true, true) => hidden.extend(crease),
                        // Where something stands on one and not the other, the covering
                        // runs on under its foot, flashed along it from the open side.
                        (mine, _) => {
                            hidden.extend(crease);
                            let (open, toward, square) = if mine {
                                (theirs, -out, other)
                            } else {
                                (*triangle, out, q)
                            };
                            segments.push(Segment {
                                part: flashing(&open),
                                from: Place::near(p),
                                to: Place::near(r),
                                sides: [centroid(&open), Place::near((p + r) * 0.5 + toward)],
                                square,
                            });
                        }
                    }
                    continue;
                }
                // Under what stands on it, against what stands beside it: nothing shows.
                Beside::Against if covered => continue,
                Beside::Against => flashing(triangle),
                Beside::Open if (p.y - r.y).abs() < 1e-4 => RoofPart::Eave,
                Beside::Open => RoofPart::Verge,
            };
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
    let hidden = join(hidden);
    let r = &kit.roof;
    let (rafter, batten) = (r.rafter * 0.5, r.batten.1);
    // How far a beam drawn from the kit's rafter module reaches from its line: as it is
    // modelled, which may be wider than the rafter's laid size.
    let beam_half = kit
        .roof
        .parts
        .get(&RoofPart::Rafter)
        .and_then(|part| kit.dress.made.as_deref()?.modules.get(&part.module))
        .map_or(rafter, |module| {
            let [high, wide] = module.size();
            rafter.max(high.max(wide) * 0.5)
        });
    // The beams under the creases, under what stands on the roof too, then every long
    // piece in its own stage.
    let all: Vec<&Segment> = joined.iter().chain(&hidden).collect();
    let every: Vec<Segment> = all.iter().map(|s| (*s).clone()).collect();
    for (index, segment) in all.iter().enumerate() {
        let segment: &Segment = segment;
        let shown = index < joined.len();
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
            let beams = framed(base, from - down, to - down, level);
            let last = beams.len().saturating_sub(1);
            // Its ends a little short of the line's, where the battens and rafters along it
            // end too, so its sawn ends are not in their planes; and where its crease meets
            // another, as a wing's ridge meets the main one at a T, short of that one's beam
            // by half its width, so it butts its side rather than running into it.
            let met = [segment.from, segment.to].map(|end| crease_met(segment, end, &every));
            let short = met.map(|met| BEAM_SHORT_M + if met { beam_half } else { 0.0 });
            for (i, (mut from, mut to)) in beams.into_iter().enumerate() {
                let along = (to - from).normalize_or_zero();
                let level = Vec3::new(along.x, 0.0, along.z).length().max(0.2);
                // Where its crease meets another, its end so far back in plan from where
                // they meet, measured level from that point, whatever the beam's drop under the
                // roof carried it toward it; elsewhere only drawn short of its end.
                let back = |end: Vec3, meet: Vec3, way: Vec3, short: f32, met: bool| {
                    if !met {
                        return end + way * (short / level);
                    }
                    let level_way = Vec3::new(way.x, 0.0, way.z).normalize_or_zero();
                    let now = (end - meet).with_y(0.0).dot(level_way);
                    end + way * ((short - now).max(0.0) / level)
                };
                if i == 0 {
                    from = back(from, segment.from.metres(), along, short[0], met[0]);
                }
                if i == last {
                    to = back(to, segment.to.metres(), -along, short[1], met[1]);
                }
                pieces.push(Piece::Roof {
                    part: RoofPart::Rafter,
                    from: Place::near(from),
                    to: Place::near(to),
                    sides: segment.sides,
                    seed: hash(&[seed, 1]),
                    cell: roofs.cell(segment.square, 1),
                    capped: [true; 2],
                    met: [false; 2],
                    wide: None,
                });
            }
        }
        if !shown {
            continue;
        }
        let (from, to) = flashing_butted(segment, &joined, kit);
        let wide = is_flashing(segment.part)
            .then(|| flashing_wide(&roofs, segment, kit))
            .flatten();
        pieces.push(Piece::Roof {
            part: segment.part,
            from,
            to,
            sides: segment.sides,
            seed,
            cell,
            capped: [segment.from, segment.to].map(|end| !edge_met(segment, end, &joined)),
            met: [segment.from, segment.to].map(|end| crease_met(segment, end, &joined)),
            wide,
        });
    }
    rafters(&roofs, &ours, batten + rafter, rafter, pieces);
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
        let mut open = roofs.beside[q].map(|b| !matches!(b, Beside::Square(_)));
        let courses = (ticks(eave), ticks(rise).max(1));
        // Tucked into whatever stands beside it, so a wall or a post thinner than its cube
        // meets covering rather than a slot down past its face.
        if !roofs.squares[q].covered {
            for (side, open) in open.iter_mut().enumerate() {
                if roofs.beside[q][side] != Beside::Against {
                    continue;
                }
                if let Some(strip) = tuck(base, kit, &roofs, q, side, courses) {
                    *open = false;
                    pieces.push(strip);
                }
            }
        }
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
        // How far up the slope a batten reaches, as it is modelled: across it from its line,
        // and how far it hangs under the roof's deck, whose plane leans back toward the crest.
        let (reach_up, reach_down) = kit
            .roof
            .parts
            .get(&RoofPart::Batten)
            .and_then(|part| kit.dress.made.as_deref()?.modules.get(&part.module))
            .map_or((r.batten.0, 0.0), |module| {
                module
                    .near
                    .iter()
                    .flat_map(|mesh| &mesh.positions)
                    .fold((r.batten.0, 0.0f32), |(up, down), p| {
                        (up.max(p[2]), down.max(-p[1]))
                    })
            });
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
                // None at a crest, where the roof falls away within the batten's reach up the
                // slope from it: the far slope's own top batten would cross it there. Past
                // that reach the roof falls on the way on, as it never does on its own plane
                // nor past a hip, beyond which it runs level the way this face rises.
                let middle = (p + r) * 0.5;
                let way = Vec3::new(upslope.x, 0.0, upslope.z).normalize_or_zero();
                let reach = (upslope * reach_up - n * reach_down).dot(way);
                let top = |past: f32| {
                    let at = middle + way * (reach + past);
                    roofs.top_over(at.x, at.z, middle.y - CELL_M)
                };
                match (top(0.01), top(0.02)) {
                    (Some(near), Some(far)) if far > near - CREST_FALL_M => {}
                    _ => continue,
                }
                let beside = Place::near(middle + upslope * 0.1);
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
    // Capped where it stops, not at a joint with the next of its line, where the two run on
    // as one and a cap on each stood in the other.
    let joined = join(battens);
    for segment in &joined {
        let joint = |end: Place| {
            joined.iter().any(|other| {
                !std::ptr::eq(other, segment)
                    && (other.from == end || other.to == end)
                    && (other.to.metres() - other.from.metres())
                        .normalize_or_zero()
                        .dot((segment.to.metres() - segment.from.metres()).normalize_or_zero())
                        .abs()
                        > 0.99
            })
        };
        let (start, end) = (segment.from.metres(), segment.to.metres());
        // Its ends at a hip in by its width: laid square across the face, its upper edge
        // reached past the hip, where the next face's batten at the same height lies.
        let along = (end - start).normalize_or_zero();
        let at_hip = |p: Vec3, out: Vec3| {
            let beyond = p + out * 0.05;
            roofs
                .top_over(beyond.x, beyond.z, p.y - CELL_M)
                .is_some_and(|h| h < p.y - 0.01)
        };
        let wide = kit.roof.batten.0;
        let start = if at_hip(start, -along) {
            start + along * wide
        } else {
            start
        };
        let end = if at_hip(end, along) {
            end - along * wide
        } else {
            end
        };
        if start.distance(end) < 1e-3 || (end - start).dot(along) <= 0.0 {
            continue;
        }
        for (from, to) in framed(base, start, end, true) {
            let capped = [
                !(from.distance(start) < 1e-3 && joint(segment.from)),
                !(to.distance(end) < 1e-3 && joint(segment.to)),
            ];
            pieces.push(Piece::Roof {
                part: RoofPart::Batten,
                from: Place::near(from),
                to: Place::near(to),
                sides: segment.sides,
                seed: 0,
                cell: roofs.cell(segment.square, 2),
                capped,
                met: [false; 2],
                wide: None,
            });
        }
    }
}

/// How far the covering is tucked into what stands beside the roof, in metres: past any
/// timber wall's or post's face in its cube (a log wall's is 10 cm in, a post's 11), short
/// of the middle of a cube, so two roofs either side of a wall a cube thick never meet in it.
pub const TUCK_M: f32 = 0.15;

/// How many cubes across a hole in a roof may be and still be one, the roof going on past
/// it: a smoke hole or a skylight. Wider, the roof's edges are its own, eaves and verges.
const HOLE_ACROSS: i32 = 2;

/// Whether a cube stands up beside a roof for its covering to be tucked into: something not
/// a roof's, solid and in, filling its column's height (whole, or a post).
fn stands_beside(base: &Base, at: IVec3) -> bool {
    base.cube(at).is_some_and(|cube| {
        !cube.kind.is_roof()
            && cube.level() > 0
            && super::drawn_fill(&cube) == Fill::Solid
            && matches!(cube.shape, Shape::Whole | Shape::Post)
    })
}

/// A strip of a square's covering carried on past a side something stands beside
/// ([`TUCK_M`]), on the plane of the square's triangle there, as a roofer runs the covering
/// up under a wall's flashing: only where what stands there is as high as the strip and its
/// covering all along it. Where what stands there turns away at an end of the side (an
/// outside corner) the strip's end is mitred back, and where it turns toward the roof (an
/// inside corner) carried on into the corner, mitred, so the strips of two sides meet on
/// the diagonal, neither in the other.
fn tuck(
    base: &Base,
    kit: &Kit,
    roofs: &Roofs,
    q: usize,
    side: usize,
    courses: (i32, i32),
) -> Option<Piece> {
    let t = roofs.triangle(q, side);
    let (a, b) = (t[1], t[2]);
    let (step, _, _) = SIDES[side];
    let out = Vec3::new(step[0] as f32, 0.0, step[1] as f32);
    let n = normal(&t);
    let along = (b - a).with_y(0.0).normalize_or_zero();
    let cell_of = |p: Vec3| (p / CELL_M).floor().as_ivec3();
    let solid_at = |p: Vec3| stands_beside(base, cell_of(p));
    // Each end: carried on (1) into an inside corner, mitred back (-1) round an outside
    // one, or square (0) where the roof and what stands beside it run on together.
    let end = |p: Vec3, away: Vec3| -> f32 {
        let beyond = p + away * 0.05 + Vec3::Y * 0.02;
        if !solid_at(beyond + out * 0.05) {
            -1.0
        } else if solid_at(beyond - out * 0.05) && solid_at(beyond + away * TUCK_M) {
            1.0
        } else {
            0.0
        }
    };
    let (ka, kb) = (end(a, -along), end(b, along));
    // A far corner: out by the tuck, and along by it into a corner or back from one, on
    // the plane.
    let far = |p: Vec3, k: f32, way: Vec3| {
        let d = out * TUCK_M + way * (k * TUCK_M);
        p + d + Vec3::Y * (-(n.x * d.x + n.z * d.z) / n.y.max(0.2))
    };
    let (a2, b2) = (far(a, ka, -along), far(b, kb, along));
    let corners = [a, b, b2, a2];
    // As high and as low as what it is tucked into goes, all over it.
    let r = &kit.roof;
    let high = corners.iter().map(|p| p.y).fold(f32::MIN, f32::max) + r.thick + r.relief + 0.02;
    let low = corners.iter().map(|p| p.y).fold(f32::MAX, f32::min) - 0.02;
    let centre = corners.iter().sum::<Vec3>() / 4.0;
    for p in [
        a + out * 0.02 + along * 0.02,
        b + out * 0.02 - along * 0.02,
        a2.lerp(centre, 0.1),
        b2.lerp(centre, 0.1),
    ] {
        let column = cell_of(p);
        for y in libm::floorf(low / CELL_M) as i32..=libm::floorf(high / CELL_M) as i32 {
            if !stands_beside(base, IVec3::new(column.x, y, column.z)) {
                return None;
            }
        }
    }
    let middle = centre;
    Some(Piece::Covering {
        corners: corners.map(Place::near),
        middle: Place::near(middle),
        courses,
        // Its edges all inside what it is tucked into, where none is seen, so none is closed
        // down to the roof and far off its sheet is lifted plumb all over, as the square's
        // side it runs on from is, and as the next strip's is where two meet in a corner.
        open: [false; 4],
        cell: roofs.cell(q, COVERING),
    })
}

/// Whether a crease's end meets another crease, at its end or along it, as a wing's ridge
/// meets the main ridge at a T, or a hip the ridge's end.
fn crease_met(segment: &Segment, end: Place, all: &[Segment]) -> bool {
    let crease =
        |part: RoofPart| matches!(part, RoofPart::Ridge | RoofPart::Hip | RoofPart::Valley);
    let p = end.metres();
    all.iter().any(|other| {
        if std::ptr::eq(other, segment) || !crease(other.part) {
            return false;
        }
        let (a, b) = (other.from.metres(), other.to.metres());
        // Not the next piece of its own line, jointed to it.
        let along = (segment.to.metres() - segment.from.metres()).normalize_or_zero();
        if along.dot((b - a).normalize_or_zero()).abs() > 0.99 {
            return false;
        }
        let t = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-9)).clamp(0.0, 1.0);
        (a + (b - a) * t).distance(p) < 0.01
    })
}

/// Whether a line's end meets another line of the roof there, and so takes no cap: a verge's
/// top under its ridge's cap, two verges at an apex, whose own module covers the joint; a
/// hip's or a valley's foot at the eaves, where it is mitred to them (`roofing`) rather
/// than capped square across the line and standing out past them. An eave meets nothing it
/// should stop at.
fn edge_met(segment: &Segment, end: Place, all: &[Segment]) -> bool {
    let near = |a: Place, b: Place| (0..3).all(|i| (a.0[i] - b.0[i]).abs() <= super::TICKS / 16);
    // A crease's end at something standing on the roof dies into it under the flashing,
    // uncapped: a cap there stood a knob against the wall's face.
    let crease = matches!(
        segment.part,
        RoofPart::Ridge | RoofPart::Hip | RoofPart::Valley
    );
    if crease
        && all.iter().any(|other| {
            let (a, b) = (other.from.metres(), other.to.metres());
            let p = end.metres();
            let t = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-9)).clamp(0.0, 1.0);
            is_flashing(other.part) && (a + (b - a) * t).distance(p) < 0.01
        })
    {
        return true;
    }
    if matches!(segment.part, RoofPart::Hip | RoofPart::Valley) {
        return all.iter().any(|other| {
            other.part == RoofPart::Eave && (near(other.from, end) || near(other.to, end))
        });
    }
    if segment.part != RoofPart::Verge {
        return false;
    }
    all.iter().any(|other| {
        !std::ptr::eq(other, segment)
            && matches!(
                other.part,
                RoofPart::Ridge | RoofPart::Hip | RoofPart::Verge
            )
            && (near(other.from, end) || near(other.to, end))
    })
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
    // Whether a cell may hold it, asked of the base once a cell: the tries come a few
    // centimetres apart, many to a cell.
    let known: std::cell::RefCell<Option<(IVec3, bool)>> = std::cell::RefCell::new(None);
    let inside_cell = |at: IVec3| {
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
    let inside = |p: Vec3| {
        let at = (p / CELL_M).floor().as_ivec3();
        if let Some((cell, ok)) = *known.borrow()
            && cell == at
        {
            return ok;
        }
        let ok = inside_cell(at);
        *known.borrow_mut() = Some((at, ok));
        ok
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
/// How far round a layout's scope a roof's squares are worked on, in cubes: past the reach
/// of a square's pieces, its eaves included.
const NEAR_SCOPE: i32 = crate::cubes::SECTION + 4;

/// Segments joined end to end along each line into long pieces, as a builder runs a board
/// or a ridge's cap on, jointed where a line passes a section's edge but for within two
/// metres of its ends, so a long piece is the same whichever part of the roof is laid and an edit
/// reaches no further than a section along it (D281).
fn join(segments: Vec<Segment>) -> Vec<Segment> {
    // By what each is, the line it is on in plan and its height where that line crosses
    // the plan's origin, then along it. Heights are to the nearest tick, so a line's slope
    // is compared, not its direction in ticks.
    type Line = (RoofPart, [i64; 2], i64, i64);
    // A run of a line's segments end to end: how far it reaches, the run as one, by whose
    // slope the next is joined, and its segments with where each starts and ends along it.
    type Run = (i64, Segment, Vec<(i64, i64, Segment)>);
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
    // Each line's runs of segments end to end, then each run jointed.
    let mut out = Vec::new();
    for ((_, d, _, _), mut on_line) in lines {
        // A line is measured along it in ticks times its direction's length.
        let clear =
            (JOINT_CLEAR as f32 * glam::Vec2::new(d[0] as f32, d[1] as f32).length()) as i64;
        on_line.sort_by_key(|(start, end, _)| (*start, *end));
        let mut runs: Vec<Run> = Vec::new();
        for (start, end, segment) in on_line {
            match runs.last_mut() {
                Some((reach, joined, run))
                    if start <= *reach && (slope(joined) - slope(&segment)).abs() < 0.02 =>
                {
                    if end > *reach {
                        *reach = end;
                        joined.to = segment.to;
                    }
                    run.push((start, end, segment));
                }
                _ => runs.push((end, segment.clone(), vec![(start, end, segment)])),
            }
        }
        for (reach, _, run) in runs {
            let first = run[0].0;
            let mut current: Option<(i64, Segment)> = None;
            let mut last_start = first;
            for (start, end, segment) in run {
                let crossed = start.div_euclid(JOINT_EVERY) > last_start.div_euclid(JOINT_EVERY)
                    && start - first >= clear
                    && reach - start >= clear;
                last_start = start;
                match &mut current {
                    Some((joined_reach, joined)) if !crossed => {
                        if end > *joined_reach {
                            *joined_reach = end;
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
    }
    out
}

/// Where a long piece of a roof is jointed, along its line: at every section's edge, in the
/// units a line is measured in (ticks along it, across both axes for a diagonal), but for
/// within two metres, in ticks, of its ends, so no short piece is left at an end.
const JOINT_EVERY: i64 = (crate::cubes::SECTION * super::TICKS) as i64;
const JOINT_CLEAR: i64 = 4 * super::TICKS as i64;

fn gcd(a: i32, b: i32) -> i32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// How much shorter at each end than its crease the beam under a ridge, a hip or a valley is.
const BEAM_SHORT_M: f32 = 0.006;

/// How far the roof falls over a centimetre on past a batten's reach up the slope, where a
/// crest is within that reach: more than rounding, less than the centimetre a 45-degree roof
/// falls there.
const CREST_FALL_M: f32 = 0.002;

/// The rafters: one up every slope along every cube's edge, from its foot at the eaves to
/// the ridge, hip or valley it meets, under the battens.
fn rafters(
    roofs: &Roofs,
    ours: &dyn Fn(usize) -> bool,
    below: f32,
    half: f32,
    pieces: &mut Vec<Piece>,
) {
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
        // Its top end, where the roof stops past it, as a lean-to's high edge does: laid
        // under the roof, square to it, it stood out past the edge by as much as it is deep,
        // so it stops short by as much.
        let (mut from, mut to) = (segment.from.metres(), segment.to.metres());
        let axis = (to - from).normalize_or(Vec3::Y);
        let (top, step) = if to.y >= from.y {
            (&mut to, axis)
        } else {
            (&mut from, -axis)
        };
        let level = Vec3::new(step.x, 0.0, step.z).normalize_or_zero();
        let beyond = *top + level * STEP_M * 0.5;
        let carries_on = roofs
            .top_over(beyond.x, beyond.z, top.y - CELL_M)
            .is_some_and(|h| h < top.y + below + CELL_M);
        // Where it carries on over a ridge or a hip, laid under the roof square to it, it
        // reached past the ridge's plumb line by as much as it is under the roof and as
        // deep again, through the rafter of the slope beyond in the same plane: it stops
        // where its foot's edge meets that line, against the beam under the crease.
        let (from, to) = if level == Vec3::ZERO {
            (segment.from, segment.to)
        } else if carries_on {
            let rise = step.y / Vec3::new(step.x, 0.0, step.z).length().max(1e-4);
            *top -= step * ((below + half) * rise.abs());
            (Place::near(from), Place::near(to))
        } else {
            *top -= step * (below * 2.0);
            (Place::near(from), Place::near(to))
        };
        pieces.push(Piece::Roof {
            part: RoofPart::Rafter,
            from,
            to,
            sides: segment.sides,
            seed,
            cell: roofs.cell(segment.square, 1),
            capped: [true; 2],
            met: [false; 2],
            wide: None,
        });
    }
}

/// A block kind's cubes carried into the roof cubes they meet (`Piece::Gable`), each to the
/// roof's deck: up into every roof cube standing on one of its whole cubes, as a gable end
/// rises to its verges and a wall's top closes the wedge under its eaves, and down into
/// every roof cube one of its cubes stands on, as a chimney or a dormer's cheek stands on
/// the roof (docs/building-plan.md, where roofs and walls meet).
pub(crate) fn gables(base: &Base, kit: &Kit, scope: Scope, pieces: &mut Vec<Piece>) {
    for (at, cube) in scope.cubes_where(base, None, |cube| cube.kind.is_roof()) {
        if !cube.kind.is_roof() || cube.fill == Fill::Open {
            continue;
        }
        let below = base.cube(at - IVec3::Y).is_some_and(|c| {
            c.kind == kit.kind() && super::drawn_fill(&c) == Fill::Solid && !c.shape.is_cut()
        });
        let above = on_top(base, at, &cube)
            && standing_on(base, at).is_some_and(|over| over.kind == kit.kind());
        let under = deck(at, cube);
        let (low, high) = under
            .iter()
            .flatten()
            .fold((i32::MAX, i32::MIN), |(l, h), &t| (l.min(t), h.max(t)));
        let up = below && super::metres(high) > at.y as f32 * CELL_M + 0.02;
        let down = above && super::metres(low) < (at.y + 1) as f32 * CELL_M - 0.02;
        let carried = match (up, down) {
            (true, true) => super::Carried::Through,
            (true, false) => super::Carried::Up,
            (false, true) => super::Carried::Down,
            (false, false) => continue,
        };
        pieces.push(Piece::Gable {
            cube: at,
            under,
            carried,
        });
    }
}

/// Where a block kind's cube carried into a roof cube is cut over a point, lowest and
/// highest, in metres: under the deck for one carried up, over it for one carried down,
/// nowhere for one the walls over and under it fill.
pub fn carried_cut(
    carried: super::Carried,
    cube: IVec3,
    under: &[[i32; 3]; 3],
    at: Vec3,
) -> [f32; 2] {
    match carried {
        super::Carried::Up => [f32::MIN, gable_cut(cube, under, at)],
        super::Carried::Down => [gable_cut(cube, under, at), f32::MAX],
        super::Carried::Through => [f32::MIN, f32::MAX],
    }
}

/// A roof cube's deck, the plane its covering lies on, over its corners, the middles of its
/// sides and its middle, west to east and north to south, in ticks: the boards under the
/// roof, its eaves and its verges are drawn on it, so a wall carried to it meets them in
/// one straight line.
pub(crate) fn deck(at: IVec3, cube: Cube) -> [[i32; 3]; 3] {
    let under = |u: f32, v: f32| ticks((at.y as f32 + surface(cube.shape, u, v)) * CELL_M);
    [0.0, 0.5, 1.0].map(|v| [0.0, 0.5, 1.0].map(|u| under(u, v)))
}

/// A roof cube's deck ([`deck`]), if any of it is over the cube's bottom: what a wall
/// carried up into it is cut at.
pub(crate) fn underside(at: IVec3, cube: Cube) -> Option<[[i32; 3]; 3]> {
    let rows = deck(at, cube);
    let highest = rows.iter().flatten().copied().max().unwrap_or(i32::MIN);
    (super::metres(highest) > at.y as f32 * CELL_M + 0.02).then_some(rows)
}

/// How far in from its cube's side a curb under a wall of logs keeps: to the logs' widest,
/// less a hair, as eave blocking's face is (`runs`).
const CURB_IN_M: f32 = 0.004;
/// How far in from its cube's side a curb keeps where a wall's logs end at that side.
const CURB_END_IN_M: f32 = 0.008;
/// How far in from its cube's sides a curb under a shape of a timber kind keeps: behind the
/// boards it is clad in.
const CURB_SHAPE_IN_M: f32 = 0.02;

/// A timber kind's walls, posts and shapes standing on roof cubes, each carried down into
/// its roof cube by a curb of its boards cut to the roof's deck (`Piece::Curb`), as a
/// dormer's cheek or a post stands on a sole on the roof: across a wall's run as wide as its
/// logs, to its cube's side where the wall runs on, as narrow as a post under a post, and
/// the whole cube under a shape.
pub(crate) fn curbs(base: &Base, kit: &Kit, scope: Scope, pieces: &mut Vec<Piece>) {
    let same = |at: IVec3| {
        base.cube(at).is_some_and(|c| {
            c.kind == kit.kind() && c.level() > 0 && super::drawn_fill(&c) == Fill::Solid
        })
    };
    // Half a member's widest across, less a hair: an upright's section stands either way.
    let half_of = |role: Option<super::Role>| {
        let (wide, high) = crate::timber::section(kit, role);
        wide.max(high) * 0.5 - CURB_IN_M
    };
    // A wall's foot on the roof is its sill where the kit dresses one, wider than its logs.
    let foot = kit
        .dress
        .member(super::Role::Foot)
        .map_or(f32::MIN, |_| half_of(Some(super::Role::Foot)));
    // A column of whole cubes alone is an upright of the wall's members (`runs::posts`), a
    // post cube the kit's post.
    // A kit that dresses its arrises with uprights stands one at each corner of a column.
    let column = if kit.dress.member(super::Role::Arris).is_some() {
        CELL_M * 0.5 - CURB_IN_M
    } else {
        half_of(None)
    };
    let (wall, post) = (half_of(None).max(foot), half_of(Some(super::Role::Post)));
    for (at, cube) in scope.cubes_where(base, None, |cube| cube.kind.is_roof()) {
        if !cube.kind.is_roof() || !on_top(base, at, &cube) {
            continue;
        }
        let Some(over) = standing_on(base, at).filter(|over| over.kind == kit.kind()) else {
            continue;
        };
        let under = deck(at, cube);
        let lowest = under.iter().flatten().copied().min().unwrap_or(i32::MAX);
        let top = (at.y + 1) as f32 * CELL_M;
        if super::metres(lowest) >= top - 0.02 {
            continue;
        }
        let standing = at + IVec3::Y;
        let dirs = [IVec3::NEG_X, IVec3::X, IVec3::NEG_Z, IVec3::Z];
        let runs_x = same(standing + IVec3::X) || same(standing - IVec3::X);
        let runs_z = same(standing + IVec3::Z) || same(standing - IVec3::Z);
        let through_x = same(standing + IVec3::X) && same(standing - IVec3::X);
        let through_z = same(standing + IVec3::Z) && same(standing - IVec3::Z);
        let alone = (over.shape == Shape::Post)
            .then_some(post)
            .or((over.shape == Shape::Whole && !runs_x && !runs_z).then_some(column));
        // How far in from each side, west, east, north and south: a wall's faces across its
        // run where it runs on through the cube both ways; the whole cube at its end, where
        // its logs are sawn flush and a post or a corner's upright stands, and round a corner
        // or a T, where they run through to its outer faces.
        let inset = dirs.map(|d| {
            let across = if d.x != 0 {
                through_z && !runs_x
            } else {
                through_x && !runs_z
            };
            if let Some(half) = alone {
                CELL_M * 0.5 - half
            } else if over.shape.is_cut() {
                // Behind a shape's cladding, which is laid on its cube's faces.
                CURB_SHAPE_IN_M
            } else if across {
                CELL_M * 0.5 - wall
            } else if same(standing + d) {
                0.0
            } else {
                // Out of the planes of the logs' ends sawn flush with the cube's side, and of
                // the daub a hair in from them.
                CURB_END_IN_M
            }
        });
        let low = at.as_vec3() * CELL_M;
        let (x0, x1) = (low.x + inset[0], low.x + CELL_M - inset[1]);
        let (z0, z1) = (low.z + inset[2], low.z + CELL_M - inset[3]);
        pieces.push(Piece::Curb {
            min: Place([ticks(x0), lowest, ticks(z0)]),
            max: Place([ticks(x1), ticks(top), ticks(z1)]),
            under,
        });
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
    // Each quarter in two triangles split along its diagonal through the cube's middle,
    // where a corner's hip or valley runs, so the cut follows the crease exactly: across it,
    // a valley's cut stood up through the roof and a hip's fell short of it.
    let h = |dx: usize, dz: usize| super::metres(under[k + dz][i + dx]);
    let (h00, h10, h01, h11) = (h(0, 0), h(1, 0), h(0, 1), h(1, 1));
    if (i == 0) == (k == 0) {
        // The diagonal from (0, 0) to (1, 1).
        if tx >= tz {
            h00 + tx * (h10 - h00) + tz * (h11 - h10)
        } else {
            h00 + tz * (h01 - h00) + tx * (h11 - h01)
        }
    } else if tx + tz <= 1.0 {
        // The diagonal from (1, 0) to (0, 1).
        h00 + tx * (h10 - h00) + tz * (h01 - h00)
    } else {
        h11 + (1.0 - tx) * (h01 - h11) + (1.0 - tz) * (h10 - h11)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::Pattern;
    use crate::roofer::{self, Style};
    use crate::{BuildOp, STOREY_M};

    const ROOF: Kind = Kind::FIRST_ROOF;

    #[test]
    fn a_gable_cut_follows_a_valley_across_its_cube_and_never_stands_over_it() {
        // An inner corner's underside over a cube: level at 3.5 m along its north and west,
        // falling to 3 m at its south-east corner, the valley on the diagonal between.
        let m = |metres: f32| super::super::ticks(metres);
        let under = [
            [m(3.5), m(3.5), m(3.5)],
            [m(3.5), m(3.25), m(3.25)],
            [m(3.5), m(3.25), m(3.0)],
        ];
        // The roof itself: the higher of the two slopes meeting in the valley.
        let roof = |x: f32, z: f32| 3.5 - 0.5 * x.min(z);
        for i in 0..=20 {
            for k in 0..=20 {
                let (x, z) = (i as f32 / 20.0, k as f32 / 20.0);
                let cut = gable_cut(IVec3::ZERO, &under, Vec3::new(x * CELL_M, 0.0, z * CELL_M));
                assert!(
                    (cut - roof(x, z)).abs() < 2e-3,
                    "{x} {z}: {cut} for {}",
                    roof(x, z)
                );
            }
        }
    }

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
    fn battens_either_side_of_a_hip_stop_short_of_one_another() {
        // A tower's pyramid: four faces, four hips, each face's battens level with the next
        // face's. Laid square across its face, each stops in from the hip by its width, so
        // its upper edge does not reach over the hip into the next face's at its height.
        let base = roofed(&[([0, 0, 0], [7, 0, 7])], Style::Hip);
        let pieces = laid(&base);
        let wide = roof_kit().roof.batten.0;
        let battens = parts(&pieces, RoofPart::Batten);
        assert!(battens.len() > 8);
        // The hips run from the corners to the apex at the middle, 45° in plan: a point is
        // on a face, not past its hips, while it is no nearer the middle across the face than
        // along the hip.
        let middle = Vec3::new(2.0, 0.0, 2.0);
        for (from, to) in &battens {
            let along = (*to - *from).normalize();
            let across = Vec3::Y.cross(along).normalize();
            for end in [*from, *to] {
                // Its corner up the slope from the end, toward the middle.
                let toward = (middle - end).with_y(0.0);
                let up = if across.dot(toward) > 0.0 {
                    across
                } else {
                    -across
                };
                let corner = end + up * wide;
                let d = (corner - middle).with_y(0.0);
                let (a, c) = (d.dot(along).abs(), d.dot(up).abs());
                assert!(
                    a <= c + 1e-3,
                    "{from}..{to}: its corner {corner} past a hip"
                );
            }
        }
    }

    #[test]
    fn a_batten_jointed_to_the_next_of_its_line_takes_no_cap_there() {
        // Twenty metres long: the battens jointed at sections' edges, each two at a joint
        // running on as one, so neither is capped there; capped at the gables.
        let base = roofed(&[([0, 0, 0], [39, 0, 7])], Style::Gable);
        let pieces = laid(&base);
        let battens: Vec<(Place, Place, [bool; 2])> = pieces
            .iter()
            .filter_map(|p| match *p {
                Piece::Roof {
                    part: RoofPart::Batten,
                    from,
                    to,
                    capped,
                    ..
                } => Some((from, to, capped)),
                _ => None,
            })
            .collect();
        let mut joints = 0;
        for &(from, to, capped) in &battens {
            for (end, at) in [from, to].into_iter().enumerate() {
                let joined = battens
                    .iter()
                    .any(|&(f, t, _)| (f, t) != (from, to) && (f == at || t == at));
                assert_eq!(capped[end], !joined, "{from:?}..{to:?} at {at:?}");
                joints += usize::from(joined);
            }
        }
        assert!(joints >= 4, "{joints}");
    }

    #[test]
    fn a_wings_ridge_beam_butts_the_main_ones_side() {
        // A T: the wing's ridge meets the main ridge along it, the beams under them at one
        // height, the wing's ending at the main one's side, not in it.
        let (_, ops) = crate::roof_rows(&[Kind::PLAIN], false, &|_| ROOF)
            .into_iter()
            .find(|(row, _)| row == "T, gable roof")
            .expect("a T under a gable");
        let mut base = Base::default();
        for op in ops {
            base.apply(op);
        }
        let pieces = laid(&base);
        let half = roof_kit().roof.rafter * 0.5;
        let beams: Vec<(Vec3, Vec3)> = parts(&pieces, RoofPart::Rafter)
            .into_iter()
            .filter(|(a, b)| (a.y - b.y).abs() < 1e-3)
            .collect();
        let along_x = beams.iter().find(|(a, b)| (a.z - b.z).abs() < 1e-3);
        let along_z = beams.iter().find(|(a, b)| (a.x - b.x).abs() < 1e-3);
        let (Some(main), Some(wing)) = (along_x, along_z) else {
            panic!("a beam under each ridge: {beams:?}");
        };
        for end in [wing.0, wing.1] {
            let off = (end.z - main.0.z).abs();
            assert!(
                off > half,
                "the wing's beam ends at {end}, in the main one at {main:?}"
            );
        }
    }

    #[test]
    fn a_rafter_stops_at_the_ridges_plumb_line_clear_of_its_pair() {
        // Rafters either side of a ridge, along the same cube's edge, lie in one plane: one
        // reaching past the ridge's plumb line would lie in the other.
        let base = roofed(&[([0, 0, 0], [9, 0, 7])], Style::Gable);
        let pieces = laid(&base);
        let (a, _) = parts(&pieces, RoofPart::Ridge)[0];
        let half = roof_kit().roof.rafter * 0.5;
        let mut looked = 0;
        for (from, to) in parts(&pieces, RoofPart::Rafter) {
            if (from.z - to.z).abs() < 1e-3 {
                continue;
            }
            let (top, foot) = if from.y > to.y {
                (from, to)
            } else {
                (to, from)
            };
            let side = (foot.z - a.z).signum();
            // Its edge under its top end: square to it, down, in its plane.
            let axis = (top - foot).normalize();
            let down = Vec3::X.cross(axis) * -side.signum();
            let down = if down.y > 0.0 { -down } else { down };
            let corner = top + down * half;
            assert!(
                (corner.z - a.z) * side > -1e-3,
                "{from} {to}: its foot's edge at {corner}, past the ridge at {a}"
            );
            looked += 1;
        }
        assert!(looked > 10);
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
            if let Piece::Gable { cube, under, .. } = piece {
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
            let Piece::Gable { cube, under, .. } = piece else {
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

    /// How many edge pieces lie along each open side of each square of covering.
    fn closings(base: &Base) -> Vec<(Vec3, Vec3, usize)> {
        let pieces = laid(base);
        let edges: Vec<(Vec3, Vec3)> = pieces
            .iter()
            .filter_map(|p| match *p {
                Piece::Roof { part, from, to, .. }
                    if !matches!(
                        part,
                        RoofPart::Rafter
                            | RoofPart::Batten
                            | RoofPart::Ridge
                            | RoofPart::Hip
                            | RoofPart::Valley
                    ) =>
                {
                    Some((from.metres(), to.metres()))
                }
                _ => None,
            })
            .collect();
        // Whether a point lies on a piece's line, within a centimetre.
        let on = |p: Vec3, (a, b): (Vec3, Vec3)| {
            let t = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-9)).clamp(0.0, 1.0);
            (a + (b - a) * t).distance(p) < 0.01
        };
        let mut out = Vec::new();
        for piece in &pieces {
            let Piece::Covering { corners, open, .. } = piece else {
                continue;
            };
            for side in 0..4 {
                if !open[side] {
                    continue;
                }
                let (p, r) = (corners[side].metres(), corners[(side + 1) % 4].metres());
                let n = edges
                    .iter()
                    .filter(|&&line| on(p, line) && on(r, line) && on((p + r) * 0.5, line))
                    .count();
                out.push((p, r, n));
            }
        }
        out
    }

    #[test]
    fn every_open_side_of_a_roofs_covering_is_closed_by_one_edge_piece() {
        // docs/building-voxels.md, invariant 3: every side of a square of covering that does
        // not carry on into the next is closed by exactly one edge piece, an eave, a verge or
        // the flashing against what stands there, over every plan, style and edge a roof has.
        let rect = [([0, 0, 0], [9, 0, 7])];
        let l = [([0, 0, 0], [11, 0, 7]), ([0, 0, 7], [7, 0, 15])];
        let t = [([0, 0, 0], [13, 0, 7]), ([3, 0, 7], [10, 0, 15])];
        let mut bases = Vec::new();
        for style in [Style::Gable, Style::Hip, Style::LeanTo] {
            bases.push((format!("rectangle {style:?}"), roofed(&rect, style)));
        }
        for style in [Style::Gable, Style::Hip] {
            bases.push((format!("L {style:?}"), roofed(&l, style)));
            bases.push((format!("T {style:?}"), roofed(&t, style)));
        }
        // A lean-to rising south to a wall two storeys high, and a gable whose verge is
        // against a taller wall beyond its end: their edges against the walls are flashed.
        let tall = |base: &mut Base, min: [i32; 3], max: [i32; 3]| {
            base.apply(BuildOp::Fill {
                min,
                max,
                cube: Some(Cube::new(Kind::PLAIN, Fill::Solid)),
            });
        };
        let mut against = roofed(&[([0, 0, 0], [9, 0, 5])], Style::LeanTo);
        tall(&mut against, [-2, 0, 6], [11, 13, 6]);
        bases.push(("lean-to against a wall".into(), against));
        let mut verge = roofed(&rect, Style::Gable);
        tall(&mut verge, [-1, 0, -1], [-1, 13, 8]);
        bases.push(("verge against a wall".into(), verge));
        for (name, base) in &bases {
            let sides = closings(base);
            assert!(sides.len() > 20, "{name}: {}", sides.len());
            for (p, r, n) in sides {
                assert_eq!(
                    n, 1,
                    "{name}: the covering's side {p} to {r} closed {n} times"
                );
            }
        }
    }
    /// The highest roof cube in a column.
    fn top_of(base: &Base, x: i32, z: i32) -> IVec3 {
        let y = base
            .column(x, z)
            .filter(|&y| {
                base.cube(IVec3::new(x, y, z))
                    .is_some_and(|c| c.kind.is_roof())
            })
            .max()
            .expect("a roof over the column");
        IVec3::new(x, y, z)
    }

    fn put(base: &mut Base, at: IVec3, cube: Cube) {
        base.apply(BuildOp::Fill {
            min: at.to_array(),
            max: at.to_array(),
            cube: Some(cube),
        });
    }

    /// Every flashing piece: what it is and its ends.
    fn flashings(pieces: &[Piece]) -> Vec<(RoofPart, Vec3, Vec3)> {
        pieces
            .iter()
            .filter_map(|p| match *p {
                Piece::Roof { part, from, to, .. } if is_flashing(part) => {
                    Some((part, from.metres(), to.metres()))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_wall_standing_on_a_slope_leaves_the_roof_whole_under_it_and_is_flashed_round() {
        // D343 as amended: a stack of wall standing on the north slope's middle. The roof
        // is drawn the same under it, covering and frame, and its foot is flashed: an apron
        // below it, where the roof rises to it, steps up its sides, a back gutter above it,
        // where the roof falls to it.
        let mut base = roofed(&[([0, 0, 0], [9, 0, 7])], Style::Gable);
        let roof = top_of(&base, 4, 1);
        for y in 1..=2 {
            put(
                &mut base,
                roof + IVec3::Y * y,
                Cube::new(Kind::PLAIN, Fill::Solid),
            );
        }
        let pieces = laid(&base);
        let under = pieces
            .iter()
            .filter(|p| matches!(p, Piece::Covering { cell, .. } if cell.cube == roof))
            .count();
        assert_eq!(under, 4, "the covering runs on under the wall");
        let (x0, x1, z0, z1) = (2.0, 2.5, 0.5, 1.0);
        let rafters_under = parts(&pieces, RoofPart::Rafter)
            .iter()
            .filter(|(a, b)| {
                let mid = (*a + *b) * 0.5;
                (x0 - 0.01..=x1 + 0.01).contains(&mid.x) && (0.0..=1.5).contains(&mid.z)
            })
            .count();
        assert!(rafters_under > 0, "the frame carries the wall");
        let flashed = flashings(&pieces);
        let along = |want: RoofPart, on: &dyn Fn(Vec3, Vec3) -> bool| {
            flashed.iter().any(|&(part, a, b)| part == want && on(a, b))
        };
        assert!(
            along(RoofPart::Flashing, &|a, b| (a.z - z0).abs() < 1e-3
                && (b.z - z0).abs() < 1e-3),
            "an apron below: {flashed:?}"
        );
        assert!(
            along(RoofPart::Gutter, &|a, b| (a.z - z1).abs() < 1e-3
                && (b.z - z1).abs() < 1e-3),
            "a back gutter above: {flashed:?}"
        );
        for x in [x0, x1] {
            assert!(
                along(RoofPart::Step, &|a, b| (a.x - x).abs() < 1e-3
                    && (b.x - x).abs() < 1e-3),
                "steps up its side at {x}: {flashed:?}"
            );
        }
    }

    #[test]
    fn the_covering_is_tucked_into_a_wall_beside_the_roof() {
        // A lean-to rising south to a wall two storeys high: its covering runs on past its
        // high edge into the wall, under its own boards, so a wall thinner than its cube
        // leaves no slot down past its face.
        let mut base = roofed(&[([0, 0, 0], [9, 0, 5])], Style::LeanTo);
        base.apply(BuildOp::Fill {
            min: [-2, 0, 6],
            max: [11, 13, 6],
            cube: Some(Cube::new(Kind::PLAIN, Fill::Solid)),
        });
        let pieces = laid(&base);
        let face = 3.0;
        let tucked: Vec<[Vec3; 4]> = pieces
            .iter()
            .filter_map(|p| match *p {
                Piece::Covering { corners, open, .. } if open == [false; 4] => {
                    Some(corners.map(Place::metres))
                }
                _ => None,
            })
            .filter(|c| c.iter().any(|p| p.z > face + 0.01))
            .collect();
        assert!(!tucked.is_empty(), "no covering tucked into the wall");
        let mut along = 0.0;
        for corners in &tucked {
            let deepest = corners.iter().map(|p| p.z).fold(f32::MIN, f32::max);
            assert!(
                (deepest - (face + TUCK_M)).abs() < 0.01,
                "tucked to {deepest} into the wall at {face}"
            );
            let xs = corners.iter().map(|p| p.x);
            along += xs.clone().fold(f32::MIN, f32::max) - xs.fold(f32::MAX, f32::min);
        }
        // Along the whole edge, from verge to verge.
        assert!(along > 4.4, "{along}");
    }

    #[test]
    fn a_hole_in_a_roof_is_flashed_round_and_takes_no_eaves() {
        // Gap 5 of docs/building-voxels.md: a cube taken out of a slope, as a smoke hole.
        let mut base = roofed(&[([0, 0, 0], [9, 0, 7])], Style::Gable);
        let hole = top_of(&base, 4, 1);
        base.apply(BuildOp::Fill {
            min: hole.to_array(),
            max: hole.to_array(),
            cube: None,
        });
        let pieces = laid(&base);
        let inside = |p: Vec3| (2.0..=2.5).contains(&p.x) && (0.5..=1.0).contains(&p.z);
        for part in [RoofPart::Eave, RoofPart::Verge] {
            assert!(
                parts(&pieces, part)
                    .iter()
                    .all(|(a, b)| !inside((*a + *b) * 0.5)),
                "{part:?} round the hole"
            );
        }
        assert!(
            pieces.iter().all(|p| match p {
                Piece::Covering { middle, .. } => !inside(middle.metres()),
                _ => true,
            }),
            "covering carried on into the hole: {:?}",
            pieces
                .iter()
                .filter(|p| match p {
                    Piece::Covering { middle, .. } => inside(middle.metres()),
                    _ => false,
                })
                .collect::<Vec<_>>()
        );
        let round = flashings(&pieces)
            .iter()
            .filter(|(_, a, b)| inside(*a) && inside(*b))
            .count();
        assert!(round >= 4, "flashed on every side: {round}");
    }

    #[test]
    fn a_wall_standing_on_a_roof_is_carried_down_to_its_deck() {
        // R2: stone standing on a slope is the stone's cube cut at the roof's deck from
        // below; a timber kind's foot a curb of its boards on the deck.
        let stone = Kit {
            id: 2,
            pattern: Pattern::Masonry,
            ..Kit::default()
        };
        let timber = Kit {
            id: 3,
            pattern: Pattern::Logs,
            ..Kit::default()
        };
        for kit in [&stone, &timber] {
            let mut base = roofed(&[([0, 0, 0], [9, 0, 7])], Style::Gable);
            let roof = top_of(&base, 4, 1);
            put(
                &mut base,
                roof + IVec3::Y,
                Cube::new(kit.kind(), Fill::Solid),
            );
            let cube = base.cube(roof).unwrap();
            let mut pieces = Vec::new();
            if kit.pattern == Pattern::Masonry {
                gables(&base, kit, Scope::ALL, &mut pieces);
                assert!(
                    pieces.iter().any(|p| matches!(
                        p,
                        Piece::Gable { cube: at, under, carried: super::super::Carried::Down }
                            if *at == roof && *under == deck(roof, cube)
                    )),
                    "{pieces:?}"
                );
            } else {
                curbs(&base, kit, Scope::ALL, &mut pieces);
                let Some(Piece::Curb { min, max, under }) = pieces.first() else {
                    panic!("no curb: {pieces:?}");
                };
                assert_eq!(*under, deck(roof, cube));
                // Up to the foot of what stands on it.
                assert_eq!(max.0[1], (roof.y + 1) * super::super::TICKS);
                assert!(min.0[1] < max.0[1]);
            }
        }
    }

    #[test]
    fn flashing_in_an_inside_corner_butts_and_does_not_lie_in_the_other() {
        // A lean-to in the corner of two taller walls, south and east: the flashing along
        // the east wall, running north and south, stops short of the corner by the south
        // one's width, so the two boards are not laid one in the other.
        let mut base = roofed(&[([0, 0, 0], [9, 0, 5])], Style::LeanTo);
        base.apply(BuildOp::Fill {
            min: [-2, 0, 6],
            max: [11, 13, 6],
            cube: Some(Cube::new(Kind::PLAIN, Fill::Solid)),
        });
        base.apply(BuildOp::Fill {
            min: [10, 0, -2],
            max: [10, 13, 5],
            cube: Some(Cube::new(Kind::PLAIN, Fill::Solid)),
        });
        let flashed = flashings(&laid(&base));
        let east: Vec<_> = flashed
            .iter()
            .filter(|(_, a, b)| (a.x - 5.0).abs() < 1e-3 && (b.x - 5.0).abs() < 1e-3)
            .collect();
        assert!(!east.is_empty(), "{flashed:?}");
        let reach = east
            .iter()
            .map(|(_, a, b)| a.z.max(b.z))
            .fold(f32::MIN, f32::max);
        assert!(
            (reach - (3.0 - 0.2)).abs() < 0.01,
            "the east flashing reaches {reach}: {east:?}"
        );
    }
}
