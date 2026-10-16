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
use std::collections::{BTreeMap, BTreeSet};

/// How thick a board is, and how far its top stands over its cube's, clear of the relief of
/// a stone footing it runs onto, and of the flats of a stone wall's border band lying on the
/// deck at its foot (9 and 12 mm up).
const BOARD_THICK_M: f32 = 0.04;
const BOARD_PROUD_M: f32 = 0.007;
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
/// The narrowest board laid to fill under a wall beside one cube of a run: narrower, a
/// board is no board, and what it would fill is under the wall's members' rounding.
const FILLER_LEAST_M: f32 = 0.008;

/// A strip under a deck's boards' edge beside a round-log wall, a plain board without a
/// plank's wave: how far its top is under the deck's cube's top, along a wall across x and
/// across z, in ticks; clear of an end board's top 5 mm under it, a crest log's flat top
/// 7 mm under, a log's far box 1 cm under, and the boards' wavy undersides 3 cm under.
const STRIP_UNDER: [i32; 2] = [6, 8];
/// How thick a strip under a deck's boards' edge is, in ticks.
const STRIP_THICK: i32 = 4;
/// How much further than the boards a strip under their edge must reach to be laid.
const STRIP_LEAST_M: f32 = 0.004;
/// How far short of its cube's face a strip under a deck's boards stops where it ends.
const STRIP_END_IN_M: f32 = 0.0031;
/// How far back under the deck's own boards a strip under their edge starts.
const STRIP_BACK_M: f32 = 0.0137;

/// How far in from its cubes' face a deck's outer board stands at its open side.
const BOARD_EDGE_IN_M: f32 = 0.003;

/// How far short of a deck's open edge a board run on under a wall beside it stops.
const FILLER_SHORT_M: f32 = 0.006;

/// How far a deck's boards run on under the narrowest member of a wall beside them.
const BOARD_UNDER_MEMBER_M: f32 = 0.006;

/// How thick a joist is, a rim joist and a ledger.
const JOIST_WIDE_M: f32 = 0.07;
/// How far a rim run on into a wall of its kind is set in from the rim's face: behind the
/// wall's sawn ends, flush or a hair short, out of their planes.
const RUN_ON_IN_M: f32 = 0.009;
/// How many ticks deeper at top and foot a rim run on into a wall is than the rim: past a
/// board's wave (3 mm) and its ends' taper (6% of its depth, 7 mm), never in their planes.
const RUN_ON_DEEPER: i32 = 6;
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

/// How far in from its cube's face a timber kind's members stand at their narrowest, in
/// metres: its seed's thinnest beam, a round log's round, or, in a column standing `alone`,
/// a post; none for a kind whose members are not long ones, which fills its cube.
pub(crate) fn member_face(kit: &Kit, alone: bool) -> Option<f32> {
    if kit.pattern != Pattern::Logs {
        return None;
    }
    // A wall of round logs: where its round stands back from its cube's face, the boards
    // under it out of sight however far its logs bulge.
    if !alone && kit.timber.profile == crate::kit::Profile::Round {
        return Some(((CELL_M - kit.diameter) * 0.5).max(0.0));
    }
    let roles: &[Option<Role>] = if alone {
        &[None, Some(Role::Post)]
    } else {
        &[None]
    };
    let narrowest = roles
        .iter()
        .map(|&role| crate::timber::section(kit, role).0 * 0.5)
        .fold(f32::MAX, f32::min)
        * (1.0 - kit.timber.variation.thickness);
    Some((CELL_M * 0.5 - narrowest).max(0.0))
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

    /// How far into a wall's cube beside the deck its boards run: under its members, a
    /// little past where they stand back from its cube's face at their narrowest
    /// ([`member_face`]: a post up a column alone, as a lookout's corner, or its seed's
    /// thinnest log or beam), so no slot is left at the boards' level between their ends and
    /// the members, whatever is under them; a wall of a kind that fills its cube, at its
    /// cube's face.
    fn board_reach(&self, beyond: IVec3) -> f32 {
        let base = self.walls.base;
        let alone = SIDES
            .iter()
            .all(|&side| base.cube(beyond + side).is_none() || self.has(beyond + side));
        let Some(kit) = base
            .cube(beyond)
            .and_then(|cube| self.walls.kits.of(cube.kind))
        else {
            return 0.0;
        };
        member_face(kit, alone).map_or(0.0, |face| (face + BOARD_UNDER_MEMBER_M).min(CELL_M * 0.5))
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
                // How far past its end cube it runs, onto a ledger or short of a rim: the
                // nearer of the two cubes either side of its line, where both are framed, so
                // it never runs past the ledger of the one into it, as where a wall of another
                // kind meets the deck beside a wall of its own.
                let reach_of = |at: IVec3, side: IVec3| match self.edge(at, side) {
                    Edge::Open => -JOIST_WIDE_M,
                    Edge::Wall => self.wall_face(at + side) - JOIST_WIDE_M,
                    Edge::Closed => 0.0,
                };
                let reach = |b: i32, side: IVec3| {
                    [(before, cell(k - 1, b)), (after, cell(k, b))]
                        .into_iter()
                        .filter(|&(framed, _)| framed)
                        .map(|(_, at)| reach_of(at, side))
                        .fold(f32::MAX, f32::min)
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
        let rim_on = |at: IVec3, side: IVec3| -> Option<Rim> {
            if !self.has(at) {
                return None;
            }
            match (self.edge(at, side), self.framed(at)) {
                (Edge::Open, false) => Some(Rim::Full),
                (Edge::Open, true) if side.dot(across) != 0 => Some(Rim::Joist),
                (Edge::Wall, true) if side.dot(across) != 0 => Some(Rim::Ledger),
                _ => None,
            }
        };
        for side in SIDES {
            let runs_along = if side.dot(along) != 0 { across } else { along };
            let rim = |at: IVec3| rim_on(at, side);
            let lines: BTreeSet<i32> = cells.iter().map(|at| at.dot(side.abs())).collect();
            let first = cells.iter().map(|at| at.dot(runs_along)).min().unwrap_or(0);
            let last = cells.iter().map(|at| at.dot(runs_along)).max().unwrap_or(0);
            let outward = side.dot(side.abs()) as f32;
            for line in lines {
                let at_of = |k: i32| side.abs() * line + runs_along * k + IVec3::Y * y;
                // A run a kind of rim, and for a ledger, the face of the wall it is on: a
                // ledger on a wall of ours set in by its logs' round, on another kind's at
                // its face, each where the joists running onto it end.
                let mut found: Vec<(i32, i32, (Rim, i32))> = Vec::new();
                for k in first..=last {
                    let Some(rim) = rim(at_of(k)) else {
                        continue;
                    };
                    let here = (rim, ticks(self.wall_face(at_of(k) + side)));
                    match found.last_mut() {
                        Some((_, end, was)) if *end + 1 == k && *was == here => *end = k,
                        _ => found.push((k, k, here)),
                    }
                }
                for (from, to, (kind, _)) in found {
                    // At a corner, the rim along the joists' ends runs through and the one
                    // across them butts against its inside face, as a carpenter frames a
                    // box: the two never lie in one another.
                    let butt = |k: i32, end: IVec3| {
                        if runs_along == across && rim_on(at_of(k), end).is_some() {
                            JOIST_WIDE_M
                        } else {
                            0.0
                        }
                    };
                    let (start_in, end_in) = (butt(from, -runs_along), butt(to, runs_along));
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
                    // A rim, on what it rests on or across joists' ends, ending at a wall of the
                    // deck's own kind rising from the cube beyond, as a pier from the end of a
                    // thick wall's decked top or a lookout's corner post: that wall's members are narrower than its cube, so the rim runs on
                    // into it to its middle, under them, as one board a little deeper than all
                    // its beams, set in a little from the rim's face, behind the wall's sawn
                    // ends, so no slot is left between the two; on a little into the rim, its
                    // top and foot past the rim's however its boards wave and taper, so none of
                    // its faces is in the plane of one of the rim's.
                    if kind != Rim::Ledger {
                        let low = courses
                            .iter()
                            .map(|&(height, deep)| height - (deep - GAP_M) * 0.5)
                            .fold(f32::MAX, f32::min);
                        let high = courses
                            .iter()
                            .map(|&(height, deep)| height + (deep - GAP_M) * 0.5)
                            .fold(f32::MIN, f32::max);
                        let ends = [
                            (from - 1, from as f32 * CELL_M + start_in),
                            (to + 1, (to + 1) as f32 * CELL_M - end_in),
                        ];
                        for (end, (k, rim_end)) in ends.into_iter().enumerate() {
                            if self.walls.cell(at_of(k)) != Some(super::Cell::Wall) {
                                continue;
                            }
                            let middle = (k as f32 + 0.5) * CELL_M;
                            let into = (rim_end - middle).signum() * RUN_ON_IN_M * 2.0;
                            let on = |k: f32| {
                                side.abs().as_vec3() * (across_at - outward * RUN_ON_IN_M)
                                    + runs_along.as_vec3() * k
                                    + Vec3::Y * (low + high) * 0.5
                            };
                            layout.pieces.push(Piece::Plank {
                                from: Place::near(on(middle)),
                                to: Place::near(on(rim_end + into)),
                                wide: ticks(JOIST_WIDE_M),
                                high: ticks(high - low) + 2 * RUN_ON_DEEPER,
                                seed: hash(&[
                                    0x41c,
                                    line as u32,
                                    from as u32,
                                    side.x as u32,
                                    side.z as u32,
                                    end as u32,
                                ]),
                            });
                        }
                    }
                    for (beam, (height, deep)) in courses.into_iter().enumerate() {
                        let on = |k: f32| {
                            side.abs().as_vec3() * across_at
                                + runs_along.as_vec3() * k
                                + Vec3::Y * height
                        };
                        layout.pieces.push(Piece::Plank {
                            from: Place::near(on(from as f32 * CELL_M + start_in)),
                            to: Place::near(on((to + 1) as f32 * CELL_M - end_in)),
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
        let under_wall = |at: IVec3| {
            even(
                self.board_reach(at)
                    .max(((CELL_M - self.kit.diameter) * 0.5).max(0.0)),
            )
        };
        // Covered by what stands on it, but for a roof's cube, whose slope leaves the deck
        // under it showing, as a dormer's eaves over a storey's ceiling do, and a wall of the
        // deck's own members standing on it, as a lookout's rail: they are narrower than its
        // cube, so the boards run on under them, which stand on the boards.
        let covered = |at: IVec3| {
            base.cube(at + IVec3::Y).is_some_and(|c| {
                c.fill != Fill::Open
                    && !c.kind.is_roof()
                    && !(c.kind == self.kit.kind() && base.in_wall(at + IVec3::Y))
            })
        };
        // A cube beside the deck under a wall: the boards run on over it, under the wall.
        // A wall of the deck's own logs, whose face is set in from its cube's by their round:
        // another kind's stands at its cube's face, which the boards stop at.
        let wall_foot = |at: IVec3| {
            !region.contains(&at.to_array())
                && base.cube(at).is_some_and(|c| c.fill == Fill::Solid)
                && base.in_wall(at + IVec3::Y)
                && base
                    .cube(at + IVec3::Y)
                    .is_some_and(|c| c.kind == self.kit.kind())
        };
        // How far past a cube beside the deck its boards run: under a wall standing on it,
        // or up to the face of a wall beside the deck on its layer, over the ledger there,
        // so no slot is left along the wall between its logs and the boards.
        let reach = |at: IVec3| {
            // Not over a flight's top step beside it, whose tread is the deck's edge there.
            if self.walls.stair(at) {
                return 0.0;
            }
            if wall_foot(at) {
                under_wall(at)
            } else if !region.contains(&at.to_array())
                && base.in_wall(at)
                && base.cube(at).is_some_and(|c| c.fill == Fill::Solid)
            {
                even(self.board_reach(at))
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
                    // At the deck's open side, nothing it runs on under beside it, a hair in from its
                    // cubes' face, out of the plane of the sawn ends of what stands on its
                    // edge there, as a rail's logs.
                    let open_side = (first..=last).all(|a| {
                        let at = cell(a, beside);
                        !region.contains(&at.to_array()) && reach(at) < 1e-3
                    });
                    let wider = if open_side { -BOARD_EDGE_IN_M } else { wider };
                    let (from_m, to_m) = if board == 0 {
                        (
                            b as f32 * CELL_M + from * CELL_M - wider,
                            (b as f32 + to) * CELL_M,
                        )
                    } else {
                        ((b as f32 + from) * CELL_M, (b as f32 + to) * CELL_M + wider)
                    };
                    // Where a cube beside the run lets the boards further under it than the
                    // whole run's least, as a post rising beside one cube of a deck's edge,
                    // a board as long as that cube fills the rest of the way to it.
                    for a in first..=last {
                        let into = reach(cell(a, beside));
                        let extra = into - wider;
                        if extra < FILLER_LEAST_M {
                            continue;
                        }
                        // Short of a row of the deck on that side running on into the same
                        // cube from beside it, so the two never lie in one another.
                        let other = |k: i32| {
                            let at = cell(k, beside);
                            region.contains(&at.to_array()) && !covered(at)
                        };
                        // And a hair short of the deck's edge where nothing is past it, out
                        // of the plane of the sawn ends of what it runs in under.
                        let edge = |k: i32| {
                            !region.contains(&cell(k, b).to_array()) && reach(cell(k, b)) < 1e-3
                        };
                        let start = a as f32 * CELL_M
                            + if other(a - 1) {
                                into
                            } else if a == first && edge(a - 1) {
                                FILLER_SHORT_M
                            } else {
                                0.0
                            };
                        let end = (a + 1) as f32 * CELL_M
                            - if other(a + 1) {
                                into
                            } else if a == last && edge(a + 1) {
                                FILLER_SHORT_M
                            } else {
                                0.0
                            };
                        if end - start < 0.02 {
                            continue;
                        }
                        let (near, far) = if board == 0 {
                            (from_m - extra, from_m)
                        } else {
                            (to_m, to_m + extra)
                        };
                        layout.pieces.push(Piece::Plank {
                            from: Place::near(point(start, (near + far) * 0.5, board_y)),
                            to: Place::near(point(end, (near + far) * 0.5, board_y)),
                            wide: ticks(far - near),
                            high: ticks(BOARD_THICK_M),
                            seed: hash(&[0xf11, b as u32, a as u32, board as u32, y as u32]),
                        });
                    }
                    let middle = (from_m + to_m) * 0.5;
                    let wide = to_m - from_m;
                    // Run on under a wall at either end; where it stops at the deck's open
                    // edge, widened in under a wall of flat-faced beams beside it, whose sawn
                    // ends there are as square as its own, a hair short, out of their plane.
                    let widened_flat = (first..=last).any(|a| {
                        base.cube(cell(a, beside))
                            .and_then(|cube| self.walls.kits.of(cube.kind))
                            .is_some_and(|kit| kit.timber.profile == crate::kit::Profile::Hewn)
                    });
                    let out = |k: i32| {
                        let at = cell(k, b);
                        let past = reach(at);
                        if past < 1e-3
                            && wider > 1e-3
                            && widened_flat
                            && !region.contains(&at.to_array())
                        {
                            -FILLER_SHORT_M
                        } else {
                            past
                        }
                    };
                    let (start_out, end_out) = (out(first - 1), out(last + 1));
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
        // Under the boards' edge beside a wall of round logs: the boards run on under the
        // logs' round to where a log of the kit's thickness stands back, but a log bowed away
        // from the deck stands back further, and down past the boards' end there, between the
        // course over the deck and the one under it, the room under the deck shows. A strip
        // a little under the boards, from under their edge to past a log bowed its furthest.
        // One strip along each run of such cells, carried on into a wall's cube at a corner
        // (under the strip along the other way) and stopped short of anything else; a plain
        // board, without a plank's wave, out of the planes of everything round it.
        let mut edges: BTreeMap<([i32; 3], [i32; 3]), f32> = BTreeMap::new();
        for &at in region.iter() {
            let at = IVec3::from_array(at);
            if covered(at) || at.y != y {
                continue;
            }
            for side in SIDES {
                let beyond = at + side;
                if region.contains(&beyond.to_array()) || self.walls.stair(beyond) {
                    continue;
                }
                let reached = reach(beyond);
                let depth = self.strip_depth(beyond);
                if reached > 1e-3 && depth >= reached + STRIP_LEAST_M {
                    edges.insert((side.to_array(), at.to_array()), depth);
                }
            }
        }
        for (&(side, at), &depth) in &edges {
            let (side, at) = (IVec3::from_array(side), IVec3::from_array(at));
            let along = IVec3::new(side.z.abs(), 0, side.x.abs());
            let same = |cell: IVec3| {
                edges
                    .get(&(side.to_array(), cell.to_array()))
                    .is_some_and(|&d| (d - depth).abs() < 1e-4)
            };
            if same(at - along) {
                continue;
            }
            let mut last = at;
            while same(last + along) {
                last += along;
            }
            // Past an end: on into a wall's cube there, else a hair short of its cube's face.
            let past = |cell: IVec3| {
                if !region.contains(&cell.to_array()) && base.cube(cell).is_some() {
                    depth * 0.5
                } else {
                    -STRIP_END_IN_M
                }
            };
            let n = side.as_vec3();
            let length = along.as_vec3();
            let centre = |cell: IVec3| (cell.as_vec3() + Vec3::splat(0.5)) * CELL_M;
            let start = centre(at) - length * (CELL_M * 0.5 + past(at - along));
            let end = centre(last) + length * (CELL_M * 0.5 + past(last + along));
            let under = super::metres(STRIP_UNDER[usize::from(side.x == 0)]);
            // Its inner side under the deck's own boards, out of the plane of its cubes' faces.
            let (inner, outer) = (
                n * (CELL_M * 0.5 - STRIP_BACK_M),
                n * (CELL_M * 0.5 + depth),
            );
            let low = (start + inner).with_y(top - under - super::metres(STRIP_THICK));
            let high = (end + outer).with_y(top - under);
            layout.pieces.push(Piece::Board {
                min: Place::near(low.min(high)),
                max: Place::near(low.max(high)),
            });
        }
    }

    /// How far a strip under a deck's boards runs into a round-log wall's cube beside it: past
    /// where its logs stand back when bowed their furthest away, and a little more.
    fn strip_depth(&self, beyond: IVec3) -> f32 {
        let base = self.walls.base;
        let Some(kit) = base
            .cube(beyond)
            .and_then(|cube| self.walls.kits.of(cube.kind))
            .filter(|kit| {
                kit.pattern == Pattern::Logs && kit.timber.profile == crate::kit::Profile::Round
            })
        else {
            return 0.0;
        };
        member_face(kit, false).map_or(0.0, |face| {
            (face + kit.timber.bend + BOARD_UNDER_MEMBER_M).min(CELL_M * 0.5)
        })
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
                // Sixteen places across it, each standing for a sixteenth of its width.
                for k in 0..16 {
                    let c = (k as f32 + 0.5) / 16.0 - 0.5;
                    let q = p + across * (wide * c);
                    let at = (q / CELL_M).floor().as_ivec3().with_y(y);
                    *cover.entry(at.to_array()).or_insert(0.0) += 0.05 * wide / 16.0;
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
            .filter(|(at, _)| {
                base.cube(*at + IVec3::Y)
                    .is_none_or(|over| over.kind == kit().kind() && base.in_wall(*at + IVec3::Y))
            })
            .map(|(at, _)| at.to_array())
            .collect();
        // Over every cube of floor and flat roof, its top boarded whole, the boards laid
        // tight, under a wall of its own logs standing on it too, which are narrower than
        // its cube; and over the walls beside a deck on its layer only as far as their logs'
        // faces, so no slot runs along a wall; nowhere else.
        let whole = CELL_M * CELL_M;
        for at in &deck {
            let area = cover.get(at).copied().unwrap_or(0.0);
            assert!(area > whole * 0.98 && area < whole * 1.03, "{at:?}: {area}");
        }
        let face = member_face(&kit(), true).expect("a timber kind") + BOARD_UNDER_MEMBER_M;
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
    fn a_lookouts_deck_is_closed_under_its_rail_and_beside_its_corner_posts() {
        // A deck on posts at its corners with a rail of its own beams along one edge, the
        // posts running up into the rail's corners, as the village's lookout: the rail's
        // beams are narrower than its cubes, so the deck's boards run on under them, and the
        // deck's rims, ending at the corner posts, run on into them. Lines up through the
        // deck under the rail, and in through its side beside a corner post, all meet
        // something; slots 4 to 12 cm wide were open there.
        let hewn = crate::kit::tests::shipped("hewn.ron");
        let kits = Kits(vec![hewn.clone()]);
        let cube = |shape: Shape| {
            Some(crate::Cube {
                shape,
                ..crate::Cube::new(hewn.kind(), Fill::Solid)
            })
        };
        let mut base = Base::default();
        let mut fill = |min: [i32; 3], max: [i32; 3], shape: Shape| {
            base.apply(BuildOp::Fill {
                min,
                max,
                cube: cube(shape),
            })
        };
        for [x, z] in [[0, 0], [5, 0], [0, 3], [5, 3]] {
            fill([x, 0, z], [x, 0, z], Shape::Post);
        }
        fill([0, 1, 0], [5, 1, 3], Shape::Whole);
        fill([0, 2, 0], [5, 2, 0], Shape::Whole);
        let lines = crate::inspect::Lines::of(&base, &kits);
        // Up through the deck under the rail, its cubes but the corners'.
        for i in 0..=200 {
            let x = 0.55 + i as f32 * 0.0095;
            for j in 0..=50 {
                let z = 0.0 + j as f32 * 0.01;
                assert!(
                    lines.meets(Vec3::new(x, 0.3, z), Vec3::Y, 2.0),
                    "open under the rail at ({x:.3}, {z:.3})"
                );
            }
        }
        // In through the deck's side beside the corner post, under its boards, as closed as
        // the rim beside it.
        let heights: Vec<f32> = (0..=45)
            .map(|j| 0.5 + j as f32 * 0.01)
            .filter(|&y| lines.meets(Vec3::new(1.25, y, -0.3), Vec3::Z, 0.6))
            .collect();
        assert!(heights.len() > 5, "the deck's rim");
        for i in 0..=14 {
            let x = 0.36 + i as f32 * 0.01;
            for &y in &heights {
                assert!(
                    lines.meets(Vec3::new(x, y, -0.3), Vec3::Z, 0.6),
                    "open beside the corner post at ({x:.2}, {y:.2})"
                );
            }
        }
    }

    #[test]
    fn a_decks_boards_stop_at_a_flights_top_step_beside_it() {
        // Stairs up beside a deck to its level, as the village's outside stair to an upper
        // door: the deck's boards run on under a wall beside them, but not over the
        // flight's top step, whose tread is the deck's edge there; run on over it, its
        // boards lay in the plane of the tread's end.
        let hewn = crate::kit::tests::shipped("hewn.ron");
        let mut base = Base::default();
        base.apply(BuildOp::SetFloor {
            min: [0, 1, 0],
            max: [3, 1, 3],
            laid: true,
            kind: hewn.kind(),
        });
        // A flight up from the south to the deck's south edge.
        for (z, y) in [(5, 0), (4, 1)] {
            base.apply(BuildOp::Fill {
                min: [1, y, z],
                max: [1, y, z],
                cube: Some(crate::Cube {
                    shape: Shape::Stair(crate::Facing::North),
                    ..crate::Cube::new(hewn.kind(), Fill::Solid)
                }),
            });
        }
        let layout = lay(&base, &hewn);
        let boards: Vec<_> = planks(&layout)
            .into_iter()
            .filter(|p| p.3 == ticks(BOARD_THICK_M))
            .collect();
        assert!(!boards.is_empty());
        for (from, to, wide, _) in &boards {
            // A board's reach across, along z: none past the deck's south face, z = 2.
            let half = super::super::metres(*wide) * 0.5;
            let south = from.z.max(to.z)
                + if (from.x - to.x).abs() > 1e-3 {
                    half
                } else {
                    0.0
                };
            assert!(south <= 2.0 + 1e-3, "a board reaching z = {south}");
        }
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
