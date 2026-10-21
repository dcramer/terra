//! Roles (D211, D215; research/role-pieces.md): every exposed face, edge and corner of a
//! kind's cubes classified by what is round it, as a pure function of the cubes and the
//! ground samples, worked out when a section is laid and never stored. A kit then dresses
//! each role as its file says (`kit::Dressing`): a profile swept along the edge, corner
//! stones up it, a long member, or nothing.
//!
//! **Faces.** Each face of a cube by what is beyond it: the same kind (hidden), another
//! kind, the ground, the open air, or the air in a hole (an opening's cube, carved, a door
//! or glazed).
//!
//! **Edges.** Where two of a cube's faces meet and both are open, an outside edge, named by
//! where it is: an [`Role::Arris`] up a corner, a [`Role::Jamb`] up a hole's side, a
//! [`Role::Crest`] along a top, a [`Role::Nose`] along a step's, a [`Role::Sill`] along a
//! hole's bottom, a [`Role::Head`] along its top, a [`Role::Soffit`] under an overhang.
//! Where a side face meets what the cube stands on or what stands on it: a [`Role::Foot`]
//! where it stands on the ground, or on another kind, or where the ground passes the face;
//! a [`Role::Meet`] where a thinner kind stands on it, as logs on a stone plinth. Where a
//! side face meets another of the kind's cubes' faces in a hollow: a [`Role::Border`] along
//! a floor it stands on, an [`Role::Inside`] up an inside corner.
//!
//! **Ownership.** Every edge belongs to one cube, the one whose side face it is along, so a
//! piece laid for it is in or out, planned or carved, with that cube alone. An inside
//! corner belongs to the cube whose open face looks east or west.
//!
//! **Ends.** Along a level edge, each end of a cube's edge meets the next: it runs on into
//! the same edge of the next cube, turns round the cube's own corner (mitred out), turns
//! into an inside corner (mitred back), steps where a foot follows the ground up or down, or
//! stops square.
//!
//! Distances it gives are in ticks of the grid (D208), so machines agree.

use super::{TICKS, drawn_fill};
use glam::IVec3;
use std::cell::RefCell;
use std::collections::BTreeMap;
use terra_build::kit::{Kits, Pattern};
use terra_build::{Base, Fill, Kind};

/// A way out of a cube along an axis, in `surface`'s order of faces.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Dir {
    /// +X.
    East,
    /// -X.
    West,
    Up,
    Down,
    /// +Z.
    South,
    /// -Z.
    North,
}

impl Dir {
    pub const ALL: [Dir; 6] = [
        Dir::East,
        Dir::West,
        Dir::Up,
        Dir::Down,
        Dir::South,
        Dir::North,
    ];
    /// The four level ways.
    pub const LEVEL: [Dir; 4] = [Dir::East, Dir::West, Dir::South, Dir::North];

    pub const fn axis(self) -> usize {
        match self {
            Dir::East | Dir::West => 0,
            Dir::Up | Dir::Down => 1,
            Dir::South | Dir::North => 2,
        }
    }

    pub const fn sign(self) -> i32 {
        match self {
            Dir::East | Dir::Up | Dir::South => 1,
            Dir::West | Dir::Down | Dir::North => -1,
        }
    }

    pub fn of(axis: usize, sign: i32) -> Dir {
        match (axis, sign > 0) {
            (0, true) => Dir::East,
            (0, false) => Dir::West,
            (1, true) => Dir::Up,
            (1, false) => Dir::Down,
            (_, true) => Dir::South,
            (_, false) => Dir::North,
        }
    }

    pub fn step(self) -> IVec3 {
        let mut v = IVec3::ZERO;
        v[self.axis()] = self.sign();
        v
    }

    pub fn opposite(self) -> Dir {
        Dir::of(self.axis(), -self.sign())
    }

    pub fn level(self) -> bool {
        self.axis() != 1
    }

    /// The level way a stair or slope rises toward.
    pub fn facing(facing: terra_build::Facing) -> Dir {
        match facing {
            terra_build::Facing::North => Dir::North,
            terra_build::Facing::East => Dir::East,
            terra_build::Facing::South => Dir::South,
            terra_build::Facing::West => Dir::West,
        }
    }
}

/// What is beyond a face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Beyond {
    Same,
    Other(Kind),
    Ground,
    Air,
    /// The air in a hole: an opening's cube.
    Hole,
    /// A cube of the kind cut to a shape (D193), which dresses its own edges (`shaped`).
    Cut,
}

impl Beyond {
    pub fn open(self) -> bool {
        matches!(self, Beyond::Air | Beyond::Hole)
    }
}

pub use terra_build::pieces::Role;

/// An edge of a cube with a role: between its face `side`, which it looks out of, and
/// `other`: up or down for a level edge; for an upright one, the face north or south of it
/// (its `side` east or west).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Edge {
    pub role: Role,
    pub side: Dir,
    pub other: Dir,
}

impl Edge {
    /// The axis it runs along.
    pub fn axis(self) -> usize {
        3 - self.side.axis() - self.other.axis()
    }
}

/// How one end of a cube's level edge meets what is next along it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum End {
    /// Stops: capped square.
    #[default]
    Square,
    /// Runs on into the same edge of the next cube.
    On,
    /// Turns round the cube's own outside corner: mitred out.
    Out,
    /// Turns into an inside corner: mitred back.
    Back,
    /// Turns round its own outside corner where another cube of the kind meets this one
    /// there only along the edge, corner to corner (docs/building-voxels.md, gap 3): its
    /// part in the wall mitred out to the turn, its part out of the face mitred back to the
    /// other cube's, cut at the plane through the edge halfway between the two.
    Shared,
    /// Stops where nothing of the wall runs on past it, as a lintel left over air where the
    /// course under its opening's side has come down: capped square at the cube's side,
    /// not carried on past it into a wall that is not there.
    Free,
    /// Runs on into the next cube's, which is raised or lowered: capped where it shows.
    Step,
    /// Meets a coping raking down from it, a ramp's (D231), mitred over the convex bend
    /// where the slope comes up to a level top, as a coping is at any change of direction.
    Over,
    /// Meets a coping raking up from it, a ramp's foot over a level top: mitred under the
    /// concave bend.
    Under,
}

/// One cube's edges with roles, each with how its ends meet the next, low end first along
/// its axis, and for a foot how it follows the ground.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dressed {
    pub cube: IVec3,
    pub edge: Edge,
    pub ends: [End; 2],
    /// For a foot: how far it is raised to stand clear of the ground beside it, and how far
    /// below that its foot goes on down to reach the lowest ground there, in ticks.
    pub lift: i32,
    pub sink: i32,
}

/// How far a foot is raised at a time, to step with the ground: a quarter of a cube, a
/// course of dressed stone.
pub const LIFT_STEP: i32 = TICKS / 4;

/// The neighbourhood roles are read from: a base's cubes of one kind, the kinds beside
/// which its faces still show (drawn thinner than a cube, and not running through it), the
/// kinds it carries a cap under (D232), and the ground's samples under the base.
pub struct Roles<'a> {
    base: &'a Base,
    kind: Kind,
    thin: Vec<Kind>,
    capped: Vec<Kind>,
    /// Whether the kind's foot is a plinth swept along it, which would run into a
    /// footing's own (`footing_dressed`), rather than a member laid on what it stands on.
    plinth: bool,
    /// How high its foot's plinth stands over its line, and how far under its line each
    /// trim along a top reaches, in ticks, from its pieces' profiles.
    plinth_high: Option<i32>,
    top_low: BTreeMap<Role, i32>,
    known: RefCell<BTreeMap<[i32; 3], Vec<Edge>>>,
}

impl<'a> Roles<'a> {
    /// Roles of `kind`'s cubes in a base, the kits saying how it meets the others (D232):
    /// its faces show beside a kind of long members, drawn thinner than a cube, that does
    /// not run through it, and beside a roof's cubes; it carries a cap under a kind it runs
    /// through, if its table says.
    pub fn new(base: &'a Base, kind: Kind, kits: &Kits) -> Roles<'a> {
        // A roof's cubes are thin too: a roof over them, not a cube (D241).
        let thin = kits
            .0
            .iter()
            .filter(|kit| {
                kit.pattern == Pattern::Roof
                    || (kit.pattern == Pattern::Logs && !kits.through(kit.kind(), kind))
            })
            .map(|kit| kit.kind())
            .collect();
        let capped = kits
            .0
            .iter()
            .filter(|kit| kits.carries(kind, kit.kind()) == terra_build::kit::Joint::Cap)
            .map(|kit| kit.kind())
            .collect();
        let plinth = kits.of(kind).is_some_and(|kit| {
            matches!(
                kit.dress.roles.get(&Role::Foot),
                Some(terra_build::kit::Dress::Profile { .. })
            )
        });
        // A profile's lowest and highest, in ticks, where the kit dresses a role with one.
        let span = |role: Role| {
            let kit = kits.of(kind)?;
            let Some(terra_build::kit::Dress::Profile { name, .. }) = kit.dress.roles.get(&role)
            else {
                return None;
            };
            let profile = kit.dress.made.as_deref()?.profiles.get(name)?;
            let low = profile.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
            let high = profile.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
            Some((super::ticks(low), super::ticks(high)))
        };
        let plinth_high = span(Role::Foot).map(|(_, high)| high);
        let top_low = [Role::Crest, Role::Meet, Role::Nose, Role::Sill]
            .into_iter()
            .filter_map(|role| span(role).map(|(low, _)| (role, low)))
            .collect();
        Roles {
            base,
            kind,
            thin,
            capped,
            plinth,
            plinth_high,
            top_low,
            known: RefCell::default(),
        }
    }

    /// The kind of a solid cube there: one whose drawn fill is solid.
    fn solid(&self, at: IVec3) -> Option<Kind> {
        self.base
            .cube(at)
            .filter(|cube| drawn_fill(cube) == Fill::Solid)
            .map(|cube| cube.kind)
    }

    /// A whole solid cube of the kind: one whose edges are worked out here.
    fn ours(&self, at: IVec3) -> bool {
        self.base.cube(at).is_some_and(|cube| {
            cube.kind == self.kind && drawn_fill(&cube) == Fill::Solid && !cube.shape.is_cut()
        })
    }

    /// Whether a cube is one of the kind going in, in what stands of a base
    /// (`Base::what_stands`): built only as far as it is filled, so no higher than the
    /// foot of a top edge beside it.
    fn going_in(&self, at: IVec3) -> bool {
        self.base.is_standing()
            && self
                .base
                .cube(at)
                .is_some_and(|cube| cube.kind == self.kind && !cube.is_built())
    }

    /// The ground's height under a column, in ticks of the base's grid: its sample, or
    /// where it has none (a base keeps samples only under its cubes, D206), the mean of
    /// those beside it, square on before diagonally, or the base's level.
    pub fn ground(&self, column: [i32; 2]) -> i32 {
        ground_ticks(self.base, column)
    }

    /// Whether a cell is in the ground: its middle below the ground's height there.
    fn buried(&self, at: IVec3) -> bool {
        at.y * TICKS + TICKS / 2 < self.ground([at.x, at.z])
    }

    /// What is beyond a face, in the cell `at`.
    pub fn beyond(&self, at: IVec3) -> Beyond {
        match self.base.cube(at) {
            Some(cube) if drawn_fill(&cube) == Fill::Solid => {
                // A roof's cube standing on a whole cube of the kind is the wall carried up
                // into it to the roof (D241), a gable or the wedge under an eave: the wall
                // runs on into it, with no top, cap or corner where it begins.
                if cube.kind.is_roof() && self.ours(at - IVec3::Y) {
                    return Beyond::Same;
                }
                // And one a whole cube of the kind stands on is the wall carried down into it
                // to the roof's deck (D343): no foot or plinth where it stands on the roof.
                if cube.kind.is_roof() && self.ours(at + IVec3::Y) {
                    return Beyond::Same;
                }
                if cube.kind == self.kind && cube.shape.is_cut() {
                    Beyond::Cut
                } else if cube.kind == self.kind {
                    Beyond::Same
                } else {
                    Beyond::Other(cube.kind)
                }
            }
            Some(_) => Beyond::Hole,
            None if self.buried(at) => Beyond::Ground,
            None => Beyond::Air,
        }
    }

    /// Whether a face shows beside it: open, or against a kind drawn thinner than a cube.
    fn shows(&self, beyond: Beyond) -> bool {
        match beyond {
            Beyond::Other(kind) => self.thin.contains(&kind),
            other => other.open(),
        }
    }

    /// Every edge of a cube of the kind that has a role and belongs to it; none if it is
    /// not one of the kind's solid cubes.
    pub fn edges(&self, cube: IVec3) -> Vec<Edge> {
        if let Some(found) = self.known.borrow().get(&cube.to_array()) {
            return found.clone();
        }
        let mut found = self.work_out(cube);
        // Where a wall broke, its stones stand as they broke, rough and undressed: a fall's
        // edge dressed as a laid one turned the castle film's breaks into a grid of smooth
        // quoins, a texture swap rather than a break (round 3).
        let broken = |dir: Dir| self.base.broken(cube + dir.step());
        found.retain(|edge| !broken(edge.side) && !broken(edge.other));
        self.known
            .borrow_mut()
            .insert(cube.to_array(), found.clone());
        found
    }

    fn work_out(&self, cube: IVec3) -> Vec<Edge> {
        let mut out = Vec::new();
        if !self.ours(cube) {
            return out;
        }
        let beyond = |dir: Dir| self.beyond(cube + dir.step());
        let mut push = |role: Role, side: Dir, other: Dir| out.push(Edge { role, side, other });
        // Level edges: a side face and the top or the bottom.
        for side in Dir::LEVEL {
            for vertical in [Dir::Up, Dir::Down] {
                // In what stands (D358), a cube going in has no top edge until it is full,
                // and one going in beside a cube does not reach that cube's top edge.
                let top = vertical == Dir::Up;
                if top && self.going_in(cube) {
                    continue;
                }
                let fs = match beyond(side) {
                    Beyond::Same if top && self.going_in(cube + side.step()) => Beyond::Air,
                    fs => fs,
                };
                let fv = beyond(vertical);
                let diagonal = cube + side.step() + vertical.step();
                let hole = fs == Beyond::Hole || fv == Beyond::Hole;
                if fs.open() && fv.open() && !self.ours(diagonal) {
                    let role = match (vertical, hole) {
                        (Dir::Up, true) => Role::Sill,
                        (Dir::Up, false) if self.ours(cube - side.step() + IVec3::Y) => Role::Nose,
                        (Dir::Up, false) => Role::Crest,
                        (_, true) => Role::Head,
                        (_, false) => Role::Soffit,
                    };
                    // A soffit or head is no foot, however near the ground.
                    if role != Role::Soffit || !self.on_ground(cube, side) {
                        push(role, side, vertical);
                        continue;
                    }
                }
                if fs != Beyond::Air {
                    continue;
                }
                if vertical == Dir::Up {
                    if matches!(fv, Beyond::Other(kind) if self.capped.contains(&kind)) {
                        push(Role::Meet, side, vertical);
                    }
                    continue;
                }
                if matches!(fv, Beyond::Other(_) | Beyond::Ground) || self.on_ground(cube, side) {
                    // Not where it stands on another kind's footing dressed with its own
                    // foot along this side: one plinth there, the footing's, not two
                    // stacked into one another.
                    if !self.footing_dressed(cube - IVec3::Y, side) {
                        push(Role::Foot, side, vertical);
                    }
                } else if fv == Beyond::Same
                    && [diagonal, diagonal + side.step()]
                        .iter()
                        // A floor it borders, not a roof's slope it rises out of, as a
                        // wall over a lean-to does.
                        .all(|&at| self.solid(at).is_some_and(|kind| !kind.is_roof()))
                {
                    push(Role::Border, side, vertical);
                }
            }
        }
        // Upright edges: an east or west face and a south or north one.
        for side in [Dir::East, Dir::West] {
            let fa = beyond(side);
            for other in [Dir::South, Dir::North] {
                let fb = beyond(other);
                let diagonal = cube + side.step() + other.step();
                let open = (fa.open() && self.shows(fb)) || (self.shows(fa) && fb.open());
                if open && !self.ours(diagonal) {
                    let hole = fa == Beyond::Hole || fb == Beyond::Hole;
                    push(if hole { Role::Jamb } else { Role::Arris }, side, other);
                } else if fa == Beyond::Air && fb == Beyond::Same && self.ours(diagonal) {
                    push(Role::Inside, side, other);
                }
            }
            // The inside corner the other way round: this cube's south or north face open
            // into it, and its east or west neighbour the corner's other arm, belongs to
            // that arm's cube, which looks east or west into it.
        }
        out
    }

    /// Whether a cube is another kind's, with a foot of its own along a side: open to the
    /// air there, standing on the ground, a third kind or the ground passing it.
    fn footing_dressed(&self, at: IVec3, side: Dir) -> bool {
        if !self.plinth {
            return false;
        }
        let Some(kind) = self.solid(at).filter(|&kind| kind != self.kind) else {
            return false;
        };
        self.solid(at + side.step()).is_none()
            && (self.buried(at - IVec3::Y)
                || self.solid(at - IVec3::Y).is_some_and(|under| under != kind)
                || self.on_ground(at, side))
    }

    /// Whether the ground passes a side face of a cube: the cell beside it is clear of the
    /// ground and the one below that is in it.
    fn on_ground(&self, cube: IVec3, side: Dir) -> bool {
        let beside = cube + side.step();
        !self.buried(beside) && self.buried(beside - IVec3::Y)
    }

    /// Whether a cube has an edge.
    fn has(&self, cube: IVec3, edge: Edge) -> bool {
        self.edges(cube).contains(&edge)
    }

    /// How a foot on a cube follows the ground, kept under the trim along the cube's top on
    /// a face it is on (a footing's cap where a wall stands on it): a plinth lifted clear of
    /// high ground beside a short footing ran up into its cap, the two one in the other with
    /// their ends in one plane. Lowered as far as it must be, its foot going as far down.
    fn lift(&self, cube: IVec3) -> (i32, i32) {
        let (lift, sink) = self.ground_lift(cube);
        let Some(plinth) = self.plinth_high.filter(|_| lift > 0) else {
            return (lift, sink);
        };
        let edges = self.edges(cube);
        let top = edges
            .iter()
            .filter(|e| {
                e.other == Dir::Up
                    && edges
                        .iter()
                        .any(|f| f.role == Role::Foot && f.side == e.side)
            })
            .filter_map(|e| self.top_low.get(&e.role))
            .map(|low| TICKS + low)
            .min();
        let over = top.map_or(0, |top| lift + plinth - top);
        if over <= 0 {
            return (lift, sink);
        }
        let lower = over.min(lift);
        (lift - lower, (sink - lower).max(0))
    }

    /// How a foot on a cube follows the ground: raised in steps of a course to stand
    /// clear of the highest ground beside the cube, and going on down to the lowest.
    fn ground_lift(&self, cube: IVec3) -> (i32, i32) {
        let bottom = cube.y * TICKS;
        let (mut high, mut low) = (i32::MIN, i32::MAX);
        for x in -1..=1 {
            for z in -1..=1 {
                let beside = cube + IVec3::new(x, 0, z);
                if (x, z) == (0, 0) || self.solid(beside).is_some() {
                    continue;
                }
                let ground = self.ground([beside.x, beside.z]);
                high = high.max(ground);
                low = low.min(ground);
            }
        }
        if high == i32::MIN {
            return (0, 0);
        }
        let over = (high - bottom).clamp(0, TICKS);
        let lift = (over + LIFT_STEP - 1) / LIFT_STEP * LIFT_STEP;
        (lift, (bottom + lift - low).clamp(0, 2 * TICKS))
    }

    /// A cube's edges with their ends, as laid.
    pub fn dressed(&self, cube: IVec3) -> Vec<Dressed> {
        let edges = self.edges(cube);
        // A foot follows the ground, but not where it stands on another kind, as stone
        // over a block of logs changed in it (D232), whose plinth would sink over them.
        let on_other = matches!(self.beyond(cube - IVec3::Y), Beyond::Other(_));
        let (lift, sink) = if edges.iter().any(|e| e.role == Role::Foot) && !on_other {
            self.lift(cube)
        } else {
            (0, 0)
        };
        edges
            .iter()
            .map(|&edge| {
                let ends = if edge.role.upright() {
                    [-1, 1].map(|toward| {
                        if self.has(cube + IVec3::Y * toward, edge) {
                            End::On
                        } else {
                            End::Square
                        }
                    })
                } else {
                    [-1, 1].map(|toward| self.end(cube, edge, lift, toward))
                };
                let (lift, sink) = if edge.role == Role::Foot {
                    (lift, sink)
                } else {
                    (0, 0)
                };
                Dressed {
                    cube,
                    edge,
                    ends,
                    lift,
                    sink,
                }
            })
            .collect()
    }

    /// How a level edge's end `toward` the low (-1) or high (1) end of its axis meets
    /// what is next.
    fn end(&self, cube: IVec3, edge: Edge, lift: i32, toward: i32) -> End {
        let along = Dir::of(edge.axis(), toward);
        let next = cube + along.step();
        if self.has(next, edge) {
            if edge.role == Role::Foot && self.lift(next).0 != lift {
                return End::Step;
            }
            return End::On;
        }
        if edge.role == Role::Crest
            && edge.other == Dir::Up
            && let Some(bend) = self.bend(next, edge.side, along)
        {
            return bend;
        }
        let turned = |side: Dir| Edge { side, ..edge };
        // Where a cube of the kind meets this one only along the edge, corner to corner,
        // each turns round its own outside corner there and into the other's inside one:
        // each keeps to its own side of the plane halfway between them.
        let back = self.has(next + edge.side.step(), turned(along.opposite()));
        match (self.has(cube, turned(along)), back) {
            (true, true) => End::Shared,
            (true, false) => End::Out,
            (false, true) => End::Back,
            (false, false) if self.solid(next).is_none() => End::Free,
            (false, false) => End::Square,
        }
    }

    /// Whether a level coping along `side` ending at `next`, `along` from its cube, meets a
    /// ramp's raking coping there: over the bend where the ramp rises to it (its slope's
    /// high end at this cube), under it where the ramp rises from it (the slope one over
    /// `next`, its low end here); each where the ramp's side is open, so it has its coping.
    fn bend(&self, next: IVec3, side: Dir, along: Dir) -> Option<End> {
        let slope = |at: IVec3, rises: Dir| {
            self.base.cube(at).is_some_and(|c| {
                c.kind == self.kind
                    && c.fill == Fill::Solid
                    && matches!(c.shape, terra_build::Shape::Slope(f) if Dir::facing(f) == rises)
            }) && self.beyond(at + side.step()).open()
        };
        if slope(next, along.opposite()) {
            return Some(End::Over);
        }
        let whole = self
            .base
            .cube(next)
            .is_some_and(|c| c.kind == self.kind && c.shape == terra_build::Shape::Whole);
        if whole && slope(next + IVec3::Y, along) {
            return Some(End::Under);
        }
        None
    }

    /// Every cube of the kind in `cubes`, each with its dressed edges, in order.
    pub fn all(&self, cubes: impl IntoIterator<Item = IVec3>) -> Vec<Dressed> {
        cubes
            .into_iter()
            .flat_map(|cube| self.dressed(cube))
            .collect()
    }
}

/// The ground's height under a column of a base, in ticks of its grid, as
/// `Ground::height_cm` gives it (D367).
pub fn ground_ticks(base: &Base, column: [i32; 2]) -> i32 {
    let cm = i32::from(base.ground().height_cm(column));
    (cm * TICKS).div_euclid(terra_build::ground::cube_cm())
}

/// Where a cube's edge is, in ticks: the point it starts at, low along its axis, and the
/// point it ends at.
pub fn line(cube: IVec3, edge: Edge) -> ([i32; 3], [i32; 3]) {
    let mut from = cube * TICKS;
    for dir in [edge.side, edge.other] {
        if dir.sign() > 0 {
            from[dir.axis()] += TICKS;
        }
    }
    let mut to = from;
    to[edge.axis()] += TICKS;
    (from.to_array(), to.to_array())
}

#[cfg(test)]
mod tests;
