//! Decks (docs/building-kits.md, construction-001's floor cases, D306): a timber kind's
//! floors and flat roofs, every cube of its kind not in a wall, laid as a carpenter frames a
//! deck, within the deck's layer of cubes: sawn boards side by side across the top, two to a
//! cube's width and as long as a tree gives, their joints staggered and over a joist; under
//! them joists across the boards, one on every line between cubes, so the outermost are the
//! deck's edge, where its underside is in the air; a rim joist across their ends along every
//! side left open, and a ledger where they meet a wall; and under the joists, at the
//! layer's foot, a bearer of the kit's heavy timber (dressed as `Role::Crest`) along every
//! line of posts under the deck, each post ending under it. A deck on posts overhangs its
//! bearers by the rest of the cube they stand in. Where a floor meets a wall or a footing,
//! its boards stop at the cube between them and the rim is left out, as the wall or footing
//! covers it; on the ground its rim goes the whole depth of the layer.
//!
//! A post's top cube with a deck beside it on its layer and nothing on it, as the hammer's
//! post a storey high under a floor laid over it leaves, is the deck's: boarded over, the
//! post ending under it ([`post_in_deck`]).

use super::roles::Role;
use super::{Layout, Piece, Place, Random, Walls, hash, ticks};
use crate::kit::{Kit, Kits, Pattern};
use crate::{Base, CELL_M, Fill, Kind, Shape};
use glam::{IVec3, Vec3};
use std::collections::BTreeSet;

/// How thick a board is, and how far its top stands over its cube's, clear of the relief of
/// a stone footing it runs onto.
const BOARD_THICK_M: f32 = 0.04;
const BOARD_PROUD_M: f32 = 0.008;
/// The gap between a rim's beams. Boards are laid tight, side by side and end to end, as
/// a floor under a room and a flat roof over one must be, their rounded edges marking the
/// seams; each edge on an even tick, so two boards' sides meet exactly, neither in the other.
const GAP_M: f32 = 0.006;

/// A length in metres to the nearest even tick, so a board's middle between two such edges
/// is on a tick.
fn even(metres: f32) -> f32 {
    let half = super::TICKS as f32 / CELL_M * 0.5;
    libm::roundf(metres * half) / half
}
/// How thick a joist is, a rim joist and a ledger.
const JOIST_WIDE_M: f32 = 0.07;
/// How deep a joist is at least, where the layer leaves little room under the boards.
const JOIST_LEAST_M: f32 = 0.1;
/// How deep each beam of a rim on the ground is at most: a deeper rim is built up of beams
/// stacked.
const RIM_COURSE_M: f32 = 0.24;
/// How far short of the end of its run of cubes a bearer stops.
const BEARER_SHORT_M: f32 = 0.02;
/// How far either way of the middle of a cube the line between its two boards falls, as a
/// fraction of the cube.
const SPLIT: f32 = 0.08;

/// The four ways along the floor.
const SIDES: [IVec3; 4] = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z];

/// Lays a deck over every cube of the kit's kind laid as one (`Walls::cell`): its floors and
/// flat roofs, and the tops of its solid masses; and over the tops of posts standing in them.
pub(super) fn decks(walls: &Walls, kit: &Kit, scope: super::Scope, layout: &mut Layout) {
    let (base, kind) = (walls.base, kit.kind());
    let is_deck = |at: IVec3| {
        (base.cube(at).is_some_and(|cube| cube.kind == kind)
            && walls.cell(at) == Some(super::Cell::Deck))
            || post_in_deck(base, walls.kits, at) == Some(kind)
    };
    // Every region with a cube in the scope, whole: found from those, not from every cube.
    let mut deck: BTreeSet<[i32; 3]> = BTreeSet::new();
    // Only its own kind's cubes and posts can be (`is_deck`, `post_in_deck`).
    let mut next: Vec<IVec3> = scope
        .cubes_where(base, None, |cube| {
            cube.kind == kind || cube.shape == Shape::Post
        })
        .into_iter()
        .filter(|&(at, _)| is_deck(at))
        .map(|(at, _)| at)
        .collect();
    deck.extend(next.iter().map(|at| at.to_array()));
    while let Some(at) = next.pop() {
        for side in SIDES {
            let beside = at + side;
            if !deck.contains(&beside.to_array()) && is_deck(beside) {
                deck.insert(beside.to_array());
                next.push(beside);
            }
        }
    }
    regions(walls, kit, &deck, CELL_M, scope, layout);
}

/// The kind a cube is laid as a deck in, if it is: a whole, solid cube of a timber kind
/// whose kit lays decks, not in a wall (`Walls::walled`), with no cube cut to a shape on it.
pub(crate) fn decked(base: &Base, kits: &Kits, at: IVec3) -> Option<Kind> {
    let cube = base.cube(at)?;
    if cube.shape != Shape::Whole || super::drawn_fill(&cube) != Fill::Solid {
        return None;
    }
    let kit = kits.of(cube.kind)?;
    if kit.pattern != Pattern::Logs || !kit.textures.contains_key(&crate::mesh::Material::Floor) {
        return None;
    }
    if base
        .cube(at + IVec3::Y)
        .is_some_and(|over| over.fill == Fill::Solid && over.shape.is_cut())
    {
        return None;
    }
    let top = !base.in_wall(at + IVec3::Y);
    let over_cut = base
        .cube(at - IVec3::Y)
        .is_some_and(|under| under.fill == Fill::Solid && under.shape.is_cut());
    let walled = base.in_wall(at) && !(top && over_cut);
    (!walled).then_some(cube.kind)
}

/// The kind of the deck a cube is the top of a post in, if it is: a post's cube, of any kind,
/// with nothing on it, beside a cube on its layer laid as a deck ([`decked`]). That deck is
/// laid over it and the post ends under it, carrying the deck's bearer, as a builder sets a
/// deck on its posts: so a post a storey high, as the hammer lays one, with a floor laid at
/// its top, stands under the floor and not up through its corner.
pub(crate) fn post_in_deck(base: &Base, kits: &Kits, at: IVec3) -> Option<Kind> {
    let cube = base.cube(at)?;
    if cube.shape != Shape::Post || cube.fill != Fill::Solid {
        return None;
    }
    if base
        .cube(at + IVec3::Y)
        .is_some_and(|over| over.fill != Fill::Open)
    {
        return None;
    }
    SIDES
        .iter()
        .filter_map(|&side| decked(base, kits, at + side))
        .min()
}

/// Lays a deck over each region of cubes joined side by side on a layer, its top so high
/// over the layer's foot: a cube's height, or half for halves.
pub(super) fn regions(
    walls: &Walls,
    kit: &Kit,
    deck: &BTreeSet<[i32; 3]>,
    height: f32,
    scope: super::Scope,
    layout: &mut Layout,
) {
    let longest = kit.longest.max(CELL_M);
    let mut seen = BTreeSet::new();
    for &start in deck {
        if seen.contains(&start) {
            continue;
        }
        // The cubes joined side by side on its layer: one deck.
        let mut region = BTreeSet::from([start]);
        let mut next = vec![IVec3::from_array(start)];
        seen.insert(start);
        while let Some(at) = next.pop() {
            for side in SIDES {
                let beside = (at + side).to_array();
                if deck.contains(&beside) && seen.insert(beside) {
                    region.insert(beside);
                    next.push(at + side);
                }
            }
        }
        // Only a deck that reaches the scope is laid, but all of it.
        let cells = region.iter().map(|&at| IVec3::from_array(at));
        let low = cells.clone().reduce(IVec3::min).unwrap_or_default();
        let high = cells.reduce(IVec3::max).unwrap_or_default();
        if !scope.reaches(low, high) {
            continue;
        }
        let deck = Deck {
            walls,
            kit,
            region: &region,
            longest,
            height,
        };
        deck.lay(layout);
    }
}

/// What holds a cube of a deck up from underneath.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Under {
    /// Nothing: it is framed, joists under its boards.
    Air,
    /// A post, or it is a post's top: framed, and a bearer over the post.
    Post,
    /// A cube or the ground: it rests on that, unframed.
    Solid,
}

/// How one side of a deck's cube is finished.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Edge {
    /// Closed by what is beyond: nothing.
    Closed,
    /// Open: a rim, or the edge joist where the joists run along it.
    Open,
    /// A wall beyond, framed into: a ledger on it under the joists' ends.
    Wall,
}

/// One deck's region of cubes on a layer, and what it is laid by.
struct Deck<'a> {
    walls: &'a Walls<'a>,
    kit: &'a Kit,
    region: &'a BTreeSet<[i32; 3]>,
    longest: f32,
    height: f32,
}

impl Deck<'_> {
    fn has(&self, at: IVec3) -> bool {
        self.region.contains(&at.to_array())
    }

    fn under(&self, at: IVec3) -> Under {
        let base = self.walls.base;
        let post = |at: IVec3| {
            base.cube(at)
                .is_some_and(|cube| cube.fill == Fill::Solid && cube.shape == Shape::Post)
        };
        if post(at) || post(at - IVec3::Y) {
            Under::Post
        } else if base.cube(at - IVec3::Y).is_none() && !base.grounded(at) {
            Under::Air
        } else {
            Under::Solid
        }
    }

    fn framed(&self, at: IVec3) -> bool {
        self.has(at) && self.under(at) != Under::Solid
    }

    /// How a cube's side is finished.
    fn edge(&self, at: IVec3, side: IVec3) -> Edge {
        let beyond = at + side;
        if self.has(beyond) {
            return Edge::Closed;
        }
        let base = self.walls.base;
        match base.cube(beyond) {
            None => Edge::Open,
            Some(cube) if cube.fill == Fill::Open => Edge::Open,
            // A flight's cubes are drawn open, its stringers bearing on the rim.
            Some(_) if self.walls.stair(beyond) => Edge::Open,
            Some(cube) if cube.shape == Shape::Post => Edge::Closed,
            Some(cube)
                if cube.fill == Fill::Solid && !cube.kind.is_roof() && base.in_wall(beyond) =>
            {
                Edge::Wall
            }
            Some(_) => Edge::Closed,
        }
    }

    /// How far into a wall's cube beside it its face is, where the deck meets it: a wall of
    /// the deck's own logs is set in from its cube's face by the logs' round.
    fn wall_face(&self, beyond: IVec3) -> f32 {
        let ours = self
            .walls
            .base
            .cube(beyond)
            .is_some_and(|cube| cube.kind == self.kit.kind());
        if ours {
            ((CELL_M - self.kit.diameter) * 0.5).max(0.0)
        } else {
            0.0
        }
    }

    fn lay(&self, layout: &mut Layout) {
        let cells: Vec<IVec3> = self
            .region
            .iter()
            .map(|&at| IVec3::from_array(at))
            .collect();
        let low = cells.iter().copied().reduce(IVec3::min).unwrap_or_default();
        let high = cells.iter().copied().reduce(IVec3::max).unwrap_or_default();
        let y = low.y;
        let foot = y as f32 * CELL_M;
        let top = foot + self.height;
        // Bearers only in a deck a cube deep: a half's is too low to frame.
        let whole = self.height >= CELL_M - 1e-3;
        let posts: Vec<IVec3> = if whole {
            cells
                .iter()
                .copied()
                .filter(|&at| self.under(at) == Under::Post)
                .collect()
        } else {
            Vec::new()
        };
        // Boards and bearers along one way, joists across them: the bearers along the lines
        // of posts, the fewer of the two ways; else the joists square to the walls they
        // meet; else the boards along the longer way.
        let lines = |axis: IVec3| -> BTreeSet<i32> {
            posts
                .iter()
                .map(|at| at.dot(IVec3::ONE - axis - IVec3::Y))
                .collect()
        };
        let walled = |normal: IVec3| {
            cells
                .iter()
                .flat_map(|&at| [normal, -normal].map(move |side| (at, side)))
                .filter(|&(at, side)| self.edge(at, side) == Edge::Wall)
                .count()
        };
        let along_x = match lines(IVec3::X).len().cmp(&lines(IVec3::Z).len()) {
            std::cmp::Ordering::Less => true,
            std::cmp::Ordering::Greater => false,
            std::cmp::Ordering::Equal => match walled(IVec3::X).cmp(&walled(IVec3::Z)) {
                std::cmp::Ordering::Greater => false,
                std::cmp::Ordering::Less => true,
                std::cmp::Ordering::Equal => high.x - low.x >= high.z - low.z,
            },
        };
        let (along, across) = if along_x {
            (IVec3::X, IVec3::Z)
        } else {
            (IVec3::Z, IVec3::X)
        };
        let cell = |a: i32, b: i32| along * a + across * b + IVec3::Y * y;
        let (a_low, b_low) = (low.dot(along), low.dot(across));
        let (a_high, b_high) = (high.dot(along), high.dot(across));
        let point = |a: f32, b: f32, height: f32| {
            along.as_vec3() * a + across.as_vec3() * b + Vec3::Y * height
        };
        self.boards(along, across, (low, high), top, layout);
        // The bearers' depth, and the joists' over them up to the boards.
        let (_, bearer_high) = crate::timber::section(self.kit, Some(Role::Crest));
        let joist_foot = if whole { foot + bearer_high } else { foot };
        let joist_deep = (top - BOARD_THICK_M - joist_foot).max(JOIST_LEAST_M);
        let joist_foot = top - BOARD_THICK_M - joist_deep;
        // Bearers: along each line of posts, the length of each run of the deck over one.
        for line in lines(along) {
            for (first, last) in runs(a_low..=a_high, |a| self.has(cell(a, line))) {
                if !(first..=last).any(|a| self.under(cell(a, line)) == Under::Post) {
                    continue;
                }
                let middle = (line as f32 + 0.5) * CELL_M;
                let height = foot + bearer_high * 0.5;
                let mut bearer = super::runs::log(
                    point(first as f32 * CELL_M + BEARER_SHORT_M, middle, height),
                    point((last + 1) as f32 * CELL_M - BEARER_SHORT_M, middle, height),
                    super::Split::Whole,
                    hash(&[0xbea2, line as u32, first as u32, y as u32, along_x as u32]),
                    None,
                );
                super::runs::set_role(&mut bearer, Some(Role::Crest));
                layout.pieces.push(bearer);
            }
        }
        // Joists on every line between cubes across the boards, under the framed cubes
        // either side of it: the outermost flush with the deck's edge, the rest centred on
        // the line; each from end to end of its run, stopping inside a rim and running on
        // onto a ledger.
        let joist_middle = joist_foot + joist_deep * 0.5;
        for k in a_low..=a_high + 1 {
            let state = |b: i32| (self.framed(cell(k - 1, b)), self.framed(cell(k, b)));
            let mut found: Vec<(i32, i32, (bool, bool))> = Vec::new();
            for b in b_low..=b_high {
                let here = state(b);
                if here == (false, false) {
                    continue;
                }
                match found.last_mut() {
                    Some((_, last, was)) if *last + 1 == b && *was == here => *last = b,
                    _ => found.push((b, b, here)),
                }
            }
            for (first, last, (before, after)) in found {
                let shift = match (before, after) {
                    (true, true) => 0.0,
                    (false, true) => JOIST_WIDE_M * 0.5,
                    _ => -JOIST_WIDE_M * 0.5,
                };
                let a = k as f32 * CELL_M + shift;
                let owner = |b: i32| if after { cell(k, b) } else { cell(k - 1, b) };
                let reach = |b: i32, side: IVec3| match self.edge(owner(b), side) {
                    Edge::Open => -JOIST_WIDE_M,
                    Edge::Wall => self.wall_face(owner(b) + side) - JOIST_WIDE_M,
                    Edge::Closed => 0.0,
                };
                let from = first as f32 * CELL_M - reach(first, -across);
                let to = (last + 1) as f32 * CELL_M + reach(last, across);
                layout.pieces.push(Piece::Plank {
                    from: Place::near(point(a, from, joist_middle)),
                    to: Place::near(point(a, to, joist_middle)),
                    wide: ticks(JOIST_WIDE_M),
                    high: ticks(joist_deep),
                    seed: hash(&[0x1015, k as u32, first as u32, y as u32]),
                });
            }
        }
        // Rims along its open sides: across the joists' ends a rim joist as deep as they
        // are, and where it rests on something the whole depth of the layer, in beams a
        // course deep, stacked; ledgers on the walls the joists run into.
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Rim {
            Joist,
            Ledger,
            Full,
        }
        for side in SIDES {
            let runs_along = if side.dot(along) != 0 { across } else { along };
            let joist_ends = side.dot(across) != 0;
            let rim = |at: IVec3| -> Option<Rim> {
                if !self.has(at) {
                    return None;
                }
                match (self.edge(at, side), self.framed(at)) {
                    (Edge::Open, false) => Some(Rim::Full),
                    (Edge::Open, true) if joist_ends => Some(Rim::Joist),
                    (Edge::Wall, true) if joist_ends => Some(Rim::Ledger),
                    _ => None,
                }
            };
            let lines: BTreeSet<i32> = cells.iter().map(|at| at.dot(side.abs())).collect();
            let first = cells.iter().map(|at| at.dot(runs_along)).min().unwrap_or(0);
            let last = cells.iter().map(|at| at.dot(runs_along)).max().unwrap_or(0);
            let outward = side.dot(side.abs()) as f32;
            for line in lines {
                let at_of = |k: i32| side.abs() * line + runs_along * k + IVec3::Y * y;
                let mut found: Vec<(i32, i32, Rim)> = Vec::new();
                for k in first..=last {
                    let Some(here) = rim(at_of(k)) else {
                        continue;
                    };
                    match found.last_mut() {
                        Some((_, end, was)) if *end + 1 == k && *was == here => *end = k,
                        _ => found.push((k, k, here)),
                    }
                }
                for (from, to, kind) in found {
                    // The cube's face on that side, and the rim's middle set in from it, or
                    // out from it onto the wall's face for a ledger.
                    let face = (line as f32 + 0.5 + outward * 0.5) * CELL_M;
                    let set = match kind {
                        Rim::Ledger => self.wall_face(at_of(from) + side) - JOIST_WIDE_M * 0.5,
                        _ => -JOIST_WIDE_M * 0.5,
                    };
                    let across_at = face + outward * set;
                    // A rim joist as deep as the joists, flush with them; a rim on the
                    // ground in beams with a gap between.
                    let courses: Vec<(f32, f32)> = match kind {
                        Rim::Joist | Rim::Ledger => vec![(joist_middle, joist_deep + GAP_M)],
                        Rim::Full => {
                            let beam_high = self.height - BOARD_THICK_M;
                            let beams = ((beam_high / RIM_COURSE_M).round() as i32).max(1);
                            let deep = beam_high / beams as f32;
                            (0..beams)
                                .map(|beam| (foot + deep * (beam as f32 + 0.5), deep))
                                .collect()
                        }
                    };
                    for (beam, (height, deep)) in courses.into_iter().enumerate() {
                        let on = |k: f32| {
                            side.abs().as_vec3() * across_at
                                + runs_along.as_vec3() * k
                                + Vec3::Y * height
                        };
                        layout.pieces.push(Piece::Plank {
                            from: Place::near(on(from as f32 * CELL_M)),
                            to: Place::near(on((to + 1) as f32 * CELL_M)),
                            wide: ticks(JOIST_WIDE_M),
                            high: ticks(deep - GAP_M),
                            seed: hash(&[
                                0x41b,
                                line as u32,
                                from as u32,
                                side.x as u32,
                                side.z as u32,
                                beam as u32,
                            ]),
                        });
                    }
                }
            }
        }
    }

    /// Its boards: two across each row of cubes along `along`, along every run of it whose
    /// top is open, running on under a wall beside it as far as the wall's members' faces.
    fn boards(
        &self,
        along: IVec3,
        across: IVec3,
        (low, high): (IVec3, IVec3),
        top: f32,
        layout: &mut Layout,
    ) {
        let base = self.walls.base;
        let region = self.region;
        let y = low.y;
        let along_x = along == IVec3::X;
        let cell = |a: i32, b: i32| along * a + across * b + IVec3::Y * y;
        let (a_low, b_low) = (low.dot(along), low.dot(across));
        let (a_high, b_high) = (high.dot(along), high.dot(across));
        // Boards run on under a wall as far as its members' faces, where it stands on
        // something else beside the deck, as a footing.
        let under_wall = even(((CELL_M - self.kit.diameter) * 0.5).max(0.0));
        // Covered by what stands on it, but for a roof's cube, whose slope leaves the deck
        // under it showing, as a dormer's eaves over a storey's ceiling do.
        let covered = |at: IVec3| {
            base.cube(at + IVec3::Y)
                .is_some_and(|c| c.fill != Fill::Open && !c.kind.is_roof())
        };
        // A cube beside the deck under a wall: the boards run on over it, under the wall.
        let wall_foot = |at: IVec3| {
            !region.contains(&at.to_array())
                && base.cube(at).is_some_and(|c| c.fill == Fill::Solid)
                && base.in_wall(at + IVec3::Y)
        };
        // How far past a cube beside the deck its boards run: under a wall standing on it,
        // or up to the face of a wall beside the deck on its layer, over the ledger there,
        // so no slot is left along the wall between its logs and the boards.
        let reach = |at: IVec3| {
            if wall_foot(at) {
                under_wall
            } else if !region.contains(&at.to_array())
                && base.in_wall(at)
                && base.cube(at).is_some_and(|c| c.fill == Fill::Solid)
            {
                even(self.wall_face(at))
            } else {
                0.0
            }
        };
        let board_y = top + BOARD_PROUD_M - BOARD_THICK_M * 0.5;
        let point = |a: f32, b: f32, height: f32| {
            along.as_vec3() * a + across.as_vec3() * b + Vec3::Y * height
        };
        for b in b_low..=b_high {
            let mut row = Random(hash(&[0xdec, b as u32, y as u32, along_x as u32]));
            let split = even((0.5 + SPLIT * row.signed()) * CELL_M) / CELL_M;
            let runs = runs(a_low..=a_high, |a| {
                region.contains(&cell(a, b).to_array()) && !covered(cell(a, b))
            });
            for (board, (from, to)) in [(0.0, split), (split, 1.0)].into_iter().enumerate() {
                for &(first, last) in &runs {
                    // The outer board of a row along a wall runs on under it.
                    let beside = if board == 0 { b - 1 } else { b + 1 };
                    let wider = (first..=last)
                        .map(|a| reach(cell(a, beside)))
                        .fold(f32::MAX, f32::min);
                    let (from_m, to_m) = if board == 0 {
                        (
                            b as f32 * CELL_M + from * CELL_M - wider,
                            (b as f32 + to) * CELL_M,
                        )
                    } else {
                        ((b as f32 + from) * CELL_M, (b as f32 + to) * CELL_M + wider)
                    };
                    let middle = (from_m + to_m) * 0.5;
                    let wide = to_m - from_m;
                    let (start_out, end_out) =
                        (reach(cell(first - 1, b)), reach(cell(last + 1, b)));
                    // Joints over a joist, staggered from the boards beside it.
                    let seed = hash(&[0xb0a, b as u32, board as u32, y as u32, first as u32]);
                    let mut lengths = Random(seed);
                    let mut at = first;
                    let mut piece = 0u32;
                    while at <= last {
                        let most = ((self.longest / CELL_M) as i32).max(1);
                        let span = if piece == 0 {
                            1 + (lengths.unit() * most as f32) as i32
                        } else {
                            most - (lengths.unit() * 2.0) as i32
                        }
                        .clamp(1, most);
                        let end = (at + span - 1).min(last);
                        let start_m =
                            at as f32 * CELL_M - if at == first { start_out } else { 0.0 };
                        let end_m =
                            (end + 1) as f32 * CELL_M + if end == last { end_out } else { 0.0 };
                        layout.pieces.push(Piece::Plank {
                            from: Place::near(point(start_m, middle, board_y)),
                            to: Place::near(point(end_m, middle, board_y)),
                            wide: ticks(wide),
                            high: ticks(BOARD_THICK_M),
                            seed: hash(&[seed, piece]),
                        });
                        at = end + 1;
                        piece += 1;
                    }
                }
            }
        }
    }
}

/// The runs of a range where `on` holds, as their first and last.
fn runs(range: std::ops::RangeInclusive<i32>, on: impl Fn(i32) -> bool) -> Vec<(i32, i32)> {
    let mut found: Vec<(i32, i32)> = Vec::new();
    for k in range {
        if !on(k) {
            continue;
        }
        match found.last_mut() {
            Some((_, last)) if *last + 1 == k => *last = k,
            _ => found.push((k, k)),
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::super::lay;
    use super::super::tests::{cabin, kit, textured};
    use super::*;
    use crate::{BuildOp, Kind};

    fn planks(layout: &Layout) -> Vec<(Vec3, Vec3, i32, i32)> {
        layout
            .pieces
            .iter()
            .filter_map(|piece| match *piece {
                Piece::Plank {
                    from,
                    to,
                    wide,
                    high,
                    ..
                } => Some((from.metres(), to.metres(), wide, high)),
                _ => None,
            })
            .collect()
    }

    /// How much of each cube's top a deck's boards cover, by cube.
    fn boarded(layout: &Layout) -> std::collections::BTreeMap<[i32; 3], f32> {
        let mut cover = std::collections::BTreeMap::new();
        for (from, to, wide, high) in planks(layout) {
            let (wide, high_m) = (super::super::metres(wide), super::super::metres(high));
            if high != ticks(BOARD_THICK_M) {
                continue;
            }
            let along = (to - from).normalize();
            let across = Vec3::Y.cross(along).abs();
            let y = ((from.y + high_m) / CELL_M).floor() as i32 - 1;
            let steps = (from.distance(to) / 0.05).round() as i32;
            for s in 0..steps {
                let p = from + along * (0.025 + s as f32 * 0.05);
                for c in [-0.45, -0.15, 0.15, 0.45] {
                    let q = p + across * (wide * c);
                    let at = (q / CELL_M).floor().as_ivec3().with_y(y);
                    *cover.entry(at.to_array()).or_insert(0.0) += 0.05 * wide * 0.25;
                }
            }
        }
        cover
    }

    #[test]
    fn a_floor_is_boarded_over_every_open_cube_and_nowhere_else() {
        let base = cabin();
        let layout = lay(&base, &textured());
        let cover = boarded(&layout);
        let deck: BTreeSet<[i32; 3]> = base
            .cubes()
            .filter(|(at, _)| !base.in_wall(*at))
            .filter(|(at, _)| base.cube(*at + IVec3::Y).is_none())
            .map(|(at, _)| at.to_array())
            .collect();
        // Over every cube of floor and flat roof not under a wall, its top boarded whole,
        // the boards laid tight; and over the walls beside a deck on its layer only as far as
        // their logs' faces, so no slot runs along a wall; nowhere else.
        let whole = CELL_M * CELL_M;
        for at in &deck {
            let area = cover.get(at).copied().unwrap_or(0.0);
            assert!(area > whole * 0.98 && area < whole * 1.03, "{at:?}: {area}");
        }
        let face = (CELL_M - kit().diameter) * 0.5;
        for (at, area) in &cover {
            if deck.contains(at) {
                continue;
            }
            let at = IVec3::from_array(*at);
            assert!(base.in_wall(at), "boarded off the deck at {at}");
            assert!(*area <= CELL_M * (face + 0.01), "{at}: {area}");
        }
        // No board longer than a tree gives.
        assert!(
            planks(&layout)
                .iter()
                .all(|(from, to, ..)| from.distance(*to) <= kit().longest + 1e-4)
        );
    }

    #[test]
    fn a_floor_inside_stone_footings_runs_on_under_its_log_walls_to_their_faces() {
        let mut base = cabin();
        base.lay_footings(Kind(2), |_| 0);
        let layout = lay(&base, &textured());
        let boards: Vec<(Vec3, Vec3, f32)> = planks(&layout)
            .into_iter()
            .filter(|p| p.3 == ticks(BOARD_THICK_M))
            .map(|(from, to, wide, _)| (from, to, super::super::metres(wide)))
            .collect();
        // Inside the walls, 3 m by 2, and under the logs as far as their faces.
        let reach = (CELL_M - kit().diameter) * 0.5;
        let (low, high) = boards.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(low, high), &(from, to, wide)| {
                let across = Vec3::Y.cross((to - from).normalize()).abs() * wide * 0.5;
                (
                    low.min(from - across).min(to - across),
                    high.max(from + across).max(to + across),
                )
            },
        );
        assert!((low.x - (CELL_M - reach)).abs() < 0.01, "{low}");
        assert!((high.x - (7.0 * CELL_M + reach)).abs() < 0.01, "{high}");
        assert!((low.z - (CELL_M - reach)).abs() < 0.01, "{low}");
        assert!((high.z - (5.0 * CELL_M + reach)).abs() < 0.01, "{high}");
    }

    #[test]
    fn a_deck_out_in_the_open_has_rims_round_it_and_joists_under_it_in_the_air() {
        let mut base = Base::default();
        // A deck three metres by two, a storey up.
        base.apply(BuildOp::SetFloor {
            min: [0, 1, 0],
            max: [5, 1, 3],
            laid: true,
            kind: kit().kind(),
        });
        let layout = lay(&base, &textured());
        let all = planks(&layout);
        let framing: Vec<_> = all.iter().filter(|p| p.2 == ticks(JOIST_WIDE_M)).collect();
        // Boards along its length, so joists across it on every line between cubes, the
        // outermost its east and west edges: seven, each the deck's width less a rim at
        // either end; and a rim joist across their ends along its north and south sides.
        let (joists, rims): (Vec<&&(Vec3, Vec3, i32, i32)>, Vec<_>) = framing
            .iter()
            .partition(|&&&(from, to, ..)| (from.x - to.x).abs() < 1e-3);
        assert_eq!(joists.len(), 7, "{joists:?}");
        for (from, to, ..) in &joists {
            assert!((from.distance(*to) - (2.0 - 2.0 * JOIST_WIDE_M)).abs() < 3e-3);
        }
        assert_eq!(rims.len(), 2, "{rims:?}");
        assert!(
            rims.iter()
                .all(|(from, to, ..)| (from.distance(*to) - 3.0).abs() < 1e-3)
        );
        // Rims and joists as deep, together, with the boards on them; the same at the
        // edges as in the middle.
        assert!(framing.iter().all(|p| p.3 == framing[0].3));
        // Everything within its cubes.
        for (from, to, wide, high) in &all {
            let piece = Piece::Plank {
                from: Place::near(*from),
                to: Place::near(*to),
                wide: *wide,
                high: *high,
                seed: 0,
            };
            let (min, max) = piece.bounds().unwrap();
            assert!(min.cmpge(Vec3::new(0.0, 2.5, 0.0) - 2e-3).all(), "{min}");
            // But for the boards standing a little proud.
            let top = 3.0 + BOARD_PROUD_M;
            assert!(max.cmple(Vec3::new(3.0, top, 2.0) + 2e-3).all(), "{max}");
        }
    }
}
