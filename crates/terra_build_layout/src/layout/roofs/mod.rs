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
//!
//! The roof's surface and its squares are here; its lines ([`lines`]), frame ([`frame`]),
//! covering ([`covering`]), flashing ([`flashing`]) and what stands on it ([`standing`]) each
//! have a module of their own.

mod covering;
mod flashing;
mod frame;
mod lines;
mod standing;

use super::{Piece, Place, Scope};
use flashing::{flashing, flashing_butted, flashing_wide};
use frame::rafters;
use glam::{IVec3, Vec2, Vec3};
use lines::{NEAR_SCOPE, crease_met, edge_met, join, ridges_a_cube_wide};
use std::collections::{BTreeMap, BTreeSet};
use terra_build::kit::{Kit, Kits};
use terra_build::{Base, CELL_M, Cube, Fill, Kind, Shape};
use terra_gen::hash;
use terra_gen::hash::QuickMap;

pub use covering::TUCK_M;
pub use flashing::is_flashing;
pub use standing::{carried_cut, gable_cut};
pub(crate) use standing::{curbs, gables, underside};

/// How finely a roof is sampled, in metres: half a cube.
pub const STEP_M: f32 = CELL_M * 0.5;
/// How far a roof of a kind with no kit is carried on past its edges.
pub const EAVES_M: f32 = 0.5;
pub use terra_build::pieces::{ROOF_STAGES, RoofPart, stage_of};

/// Which roof cube a piece of roof goes up with, and in which of its stages: 1 its frame,
/// 2 its battens, 3 its covering, 4 its trims.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RoofCell {
    pub cube: IVec3,
    pub stage: u8,
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

/// Squares by their place, in a table over the places they cover: where each place's start
/// in `squares`, the next place's its end.
#[derive(Clone, Debug)]
struct PlaceTable {
    low: [i32; 2],
    size: [i32; 2],
    starts: Vec<u32>,
    squares: Vec<usize>,
}

impl PlaceTable {
    /// The most places a table is made over: past it a roof's squares are looked up by hash.
    const MOST: i64 = 1 << 20;

    fn of(by_place: &QuickMap<[i32; 2], Vec<usize>>) -> Option<PlaceTable> {
        let (low, high) = by_place
            .keys()
            .fold(([i32::MAX; 2], [i32::MIN; 2]), |(l, h), p| {
                (
                    [l[0].min(p[0]), l[1].min(p[1])],
                    [h[0].max(p[0]), h[1].max(p[1])],
                )
            });
        if by_place.is_empty()
            || i64::from(high[0] - low[0] + 1) * i64::from(high[1] - low[1] + 1) > Self::MOST
        {
            return None;
        }
        let size = [high[0] - low[0] + 1, high[1] - low[1] + 1];
        let cells = (size[0] * size[1]) as usize;
        let mut counts = vec![0u32; cells + 1];
        for place in by_place.keys() {
            let at = ((place[1] - low[1]) * size[0] + place[0] - low[0]) as usize;
            counts[at + 1] = by_place[place].len() as u32;
        }
        for at in 1..=cells {
            counts[at] += counts[at - 1];
        }
        let mut squares = vec![0usize; counts[cells] as usize];
        for (place, on) in by_place {
            let at = ((place[1] - low[1]) * size[0] + place[0] - low[0]) as usize;
            let start = counts[at] as usize;
            squares[start..start + on.len()].copy_from_slice(on);
        }
        Some(PlaceTable {
            low,
            size,
            starts: counts,
            squares,
        })
    }

    fn at(&self, place: [i32; 2]) -> &[usize] {
        let (x, z) = (place[0] - self.low[0], place[1] - self.low[1]);
        if x < 0 || z < 0 || x >= self.size[0] || z >= self.size[1] {
            return &[];
        }
        let at = (z * self.size[0] + x) as usize;
        &self.squares[self.starts[at] as usize..self.starts[at + 1] as usize]
    }
}

/// The cube standing on a roof's cube, if one does: something not a roof's, in, with its
/// foot on the roof cube's top (not an upper half or an upside-down shape hung over it). It
/// is carried down into the roof's cube to the roof's deck, and the roof is flashed round
/// its foot (docs/building-plan.md, where roofs and walls meet).
pub fn standing_on(base: &Base, roof: IVec3) -> Option<Cube> {
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
    /// The same, in a table over the squares' places, where it is not too big: a point's
    /// squares looked up by where it is, as every piece laid on a roof asks of many points.
    table: Option<PlaceTable>,
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
pub fn remembered<T>(run: impl FnOnce() -> T) -> T {
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
    pub fn whole(base: &Base, kits: &Kits, near: Scope) -> std::rc::Rc<Roofs> {
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

    /// The lowest any roof comes over a box east and south, in metres, eaves too: no
    /// height a point in it asks of them ([`Roofs::top_over`]) is under it. `None` where no
    /// roof is over any of it.
    pub fn lowest_over(&self, low: Vec2, high: Vec2) -> Option<f32> {
        let (from, to) = (
            (low / STEP_M).floor().as_ivec2(),
            (high / STEP_M).floor().as_ivec2(),
        );
        let mut lowest: Option<f32> = None;
        for x in from.x..=to.x {
            for z in from.y..=to.y {
                for &q in self.by_place.get(&[x, z]).map_or(&[][..], Vec::as_slice) {
                    let square = &self.squares[q];
                    for &h in square.corners.iter().chain([&square.middle]) {
                        lowest = Some(lowest.map_or(h, |l: f32| l.min(h)));
                    }
                }
            }
        }
        lowest
    }

    /// The roofs of these roof cubes, sorted.
    fn of(base: &Base, kits: &Kits, cubes: &[(IVec3, Cube)]) -> Roofs {
        let mut roofs = Roofs {
            squares: Vec::new(),
            by_place: QuickMap::default(),
            table: None,
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
        roofs.carry_eaves(base, kits);
        roofs.index();
        roofs.link(base);
        roofs.group();
        roofs
    }

    /// Carries the roof on past every open edge by its kit's eaves.
    fn carry_eaves(&mut self, base: &Base, kits: &Kits) {
        let roofs = &mut *self;
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
            // A point carried on past the square, in half cubes, on the planes of its cube.
            let height = |x: f32, z: f32| {
                // A point in half cubes, in the owning cube's fractions.
                let (u, v) = (
                    x * 0.5 - square.cube.x as f32,
                    z * 0.5 - square.cube.z as f32,
                );
                (square.cube.y as f32 + plane(shape, u, v)) * CELL_M
            };
            let highest = square.corners.iter().fold(f32::MIN, |a, &b| a.max(b));
            // Whether a place carried on into, a step this way from the square, is the
            // roof's own: its cubes there, near it.
            let roofed = |[dx, dz]: [i32; 2]| {
                let place = [square.place[0] + dx, square.place[1] + dz];
                let middle = height(place[0] as f32 + 0.5, place[1] as f32 + 0.5);
                roofs.by_place.get(&place).is_some_and(|on| {
                    on.iter()
                        .any(|&r| (roofs.squares[r].middle - middle).abs() < 1.0)
                })
            };
            let mut out: Vec<[i32; 2]> = Vec::new();
            for side in 0..4 {
                if !open[side] {
                    continue;
                }
                let (step, _, _) = SIDES[side];
                for k in 1..=n {
                    out.push([step[0] * k, step[1] * k]);
                }
                // Round the corner, where the next side is open too: as a hip's crease runs
                // down off the roof there, or one plane on round it, but never a valley out
                // past an outside corner, nor a crease rising out past the roof, as round a
                // lone corner cube laid by hand, whose two ramps both run on past it
                // (docs/building-voxels.md, gap 11).
                let next = (side + 1) % 4;
                if open[next] {
                    let (turn, _, _) = SIDES[next];
                    // Nor round it from a side open over a lower roof of its own, where cubes
                    // step down: the eaves straight out there are that roof's own squares,
                    // and carried round past its edge they stood a step over its own eaves
                    // (gap 11).
                    let round: Vec<[i32; 2]> = (1..=n)
                        .flat_map(|a| {
                            (1..=n).map(move |b| {
                                ([step[0] * a, step[1] * a], [turn[0] * b, turn[1] * b])
                            })
                        })
                        .filter(|&(along, across)| !roofed(along) && !roofed(across))
                        .map(|(along, across)| [along[0] + across[0], along[1] + across[1]])
                        .collect();
                    let creased = round.iter().any(|&[dx, dz]| {
                        let place = [square.place[0] + dx, square.place[1] + dz];
                        let corners = CORNERS
                            .map(|[cx, cz]| height((place[0] + cx) as f32, (place[1] + cz) as f32));
                        let middle = height(place[0] as f32 + 0.5, place[1] as f32 + 0.5);
                        let mean = corners.iter().sum::<f32>() * 0.25;
                        let rises = corners.iter().any(|&h| h > highest + 1e-4);
                        middle < mean - 1e-4 || (rises && middle > mean + 1e-4)
                    });
                    if !creased {
                        out.extend(round);
                    }
                }
            }
            for [dx, dz] in out {
                let place = [square.place[0] + dx, square.place[1] + dz];
                let corners =
                    CORNERS.map(|[cx, cz]| height((place[0] + cx) as f32, (place[1] + cz) as f32));
                let middle = height(place[0] as f32 + 0.5, place[1] as f32 + 0.5);
                // Not where the roof's cubes are, near it, nor into a wall or anything else
                // beside the roof.
                if roofed([dx, dz]) {
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
    }

    fn index(&mut self) {
        self.by_place.clear();
        for (q, square) in self.squares.iter().enumerate() {
            self.by_place.entry(square.place).or_default().push(q);
        }
        self.table = PlaceTable::of(&self.by_place);
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
                    // Or a wall running on past a level edge of the roof's own cubes, its top
                    // where the edge is, as a garden wall runs on from a room's corner under
                    // its eaves: the eave is against it, flashed, not carried on over it
                    // (docs/building-voxels.md, gap 18).
                    let level = (a.y - b.y).abs() < 1e-4;
                    let under = ((probe - Vec3::Y * 0.1) / CELL_M).floor().as_ivec3();
                    let walled = real
                        && level
                        && under.y + 1 == cube.y
                        && base.cube(under).is_some_and(|c| {
                            !c.kind.is_roof() && c.fill == Fill::Solid && c.shape == Shape::Whole
                        });
                    if base.cube(cube).is_some_and(|c| c.fill != Fill::Open) || hole || walled {
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
        match &self.table {
            Some(table) => table.at(place),
            None => self.by_place.get(&place).map_or(&[], Vec::as_slice),
        }
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

    /// The top of an eave (a level edge with nothing beyond) a point is out past, within
    /// `reach` of it level and no lower than `above`: where a roof steps down, its eave at
    /// the step's top stands over the roof below, and what hangs from it (its fascia) is
    /// that roof's edge, not over the roof below.
    pub fn eave_over(&self, x: f32, z: f32, reach: f32, above: f32) -> Option<f32> {
        let p = Vec2::new(x, z);
        let near = [
            [0.0, 0.0],
            [reach, 0.0],
            [-reach, 0.0],
            [0.0, reach],
            [0.0, -reach],
        ];
        near.iter()
            .flat_map(|[dx, dz]| self.over(x + dx, z + dz))
            .flat_map(|&q| (0..4).map(move |side| (q, side)))
            .filter(|&(q, side)| self.beside[q][side] == Beside::Open)
            .filter_map(|(q, side)| {
                let (a, b) = (self.point(q, side), self.point(q, (side + 1) % 4));
                if (a.y - b.y).abs() > 1e-4 || a.y < above {
                    return None;
                }
                let (step, _, _) = SIDES[side];
                let out = Vec2::new(step[0] as f32, step[1] as f32);
                let (a2, b2) = (Vec2::new(a.x, a.z), Vec2::new(b.x, b.z));
                let along = (b2 - a2).normalize_or_zero();
                let (past, on) = ((p - a2).dot(out), (p - a2).dot(along));
                let inside =
                    (-1e-3..=reach).contains(&past) && (-1e-3..=STEP_M + 1e-3).contains(&on);
                inside.then_some(a.y)
            })
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
    pub fn fingerprint(&self) -> u64 {
        *self.fingerprinted.get_or_init(|| self.fingerprint_of())
    }

    fn fingerprint_of(&self) -> u64 {
        use std::hash::Hasher;
        let mut hash = terra_gen::hash::Fnv1a::default();
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

/// How many cubes across a hole in a roof may be and still be one, the roof going on past
/// it: a smoke hole or a skylight. Wider, the roof's edges are its own, eaves and verges.
const HOLE_ACROSS: i32 = 2;

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
pub fn planar_normal(from: Vec3, to: Vec3, beside: Vec3) -> Vec3 {
    let n = (to - from).cross(beside - from).normalize_or(Vec3::Y);
    if n.y < 0.0 { -n } else { n }
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
pub fn lay(base: &Base, kit: &Kit, kits: &Kits, scope: Scope, pieces: &mut Vec<Piece>) {
    if kit.pattern != terra_build::kit::Pattern::Roof {
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
    let (segments, hidden) = lines::found(&roofs, &ours);
    let joined = ridges_a_cube_wide(join(segments));
    let hidden = join(hidden);
    let r = &kit.roof;
    let (rafter, batten) = (r.rafter * 0.5, r.batten.1);
    let beam_half = frame::beam_half(kit, rafter);
    // The beams under the creases, under what stands on the roof too, then every long
    // piece in its own stage.
    let all: Vec<&Segment> = joined.iter().chain(&hidden).collect();
    let every: Vec<Segment> = all.iter().map(|s| (*s).clone()).collect();
    for (index, segment) in all.iter().enumerate() {
        let segment: &Segment = segment;
        let shown = index < joined.len();
        let cell = roofs.cell(segment.square, segment.part.stage());
        let seed = hash::of(&[
            segment.from.0[0] as u32,
            segment.from.0[2] as u32,
            segment.part as u32,
        ]);
        if matches!(
            segment.part,
            RoofPart::Ridge | RoofPart::Hip | RoofPart::Valley
        ) {
            let under = frame::Under {
                down: batten + rafter,
                beam_half,
            };
            frame::crease_beams(base, &roofs, segment, &every, seed, under, pieces);
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
    let battens = covering::lay(base, kit, &roofs, &ours, pieces);
    frame::battens(base, kit, &roofs, battens, pieces);
}

#[cfg(test)]
mod tests;
