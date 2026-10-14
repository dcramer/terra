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
use crate::kit::{Kits, Pattern};
use crate::{Base, Fill, Kind};
use glam::IVec3;
use std::cell::RefCell;
use std::collections::BTreeMap;

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
    pub fn facing(facing: crate::Facing) -> Dir {
        match facing {
            crate::Facing::North => Dir::North,
            crate::Facing::East => Dir::East,
            crate::Facing::South => Dir::South,
            crate::Facing::West => Dir::West,
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

/// What an edge of a cube is, by where it is (the module's comment).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Role {
    Arris,
    Jamb,
    Inside,
    Crest,
    Nose,
    Sill,
    Head,
    Soffit,
    Foot,
    Border,
    Meet,
    /// Not an edge: a post's shaft, up a column of cubes cut as posts (D193), as the kit
    /// dresses it: a member of its timber, or a pillar of its stone.
    Post,
}

impl Role {
    pub const ALL: [Role; 12] = [
        Role::Arris,
        Role::Jamb,
        Role::Inside,
        Role::Crest,
        Role::Nose,
        Role::Sill,
        Role::Head,
        Role::Soffit,
        Role::Foot,
        Role::Border,
        Role::Meet,
        Role::Post,
    ];

    /// Whether it runs up and down, rather than level.
    pub fn upright(self) -> bool {
        matches!(self, Role::Arris | Role::Jamb | Role::Inside)
    }
}

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
    /// Runs on into the next cube's, which is raised or lowered: capped where it shows.
    Step,
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
            .filter(|kit| kits.carries(kind, kit.kind()) == crate::kit::Joint::Cap)
            .map(|kit| kit.kind())
            .collect();
        Roles {
            base,
            kind,
            thin,
            capped,
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
        let found = self.work_out(cube);
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
            let fs = beyond(side);
            for vertical in [Dir::Up, Dir::Down] {
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
                    push(Role::Foot, side, vertical);
                } else if fv == Beyond::Same
                    && self.solid(diagonal).is_some()
                    && self.solid(diagonal + side.step()).is_some()
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

    /// How a foot on a cube follows the ground: raised in steps of a course to stand
    /// clear of the highest ground beside the cube, and going on down to the lowest.
    fn lift(&self, cube: IVec3) -> (i32, i32) {
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
        let turned = |side: Dir| Edge { side, ..edge };
        if self.has(cube, turned(along)) {
            return End::Out;
        }
        if self.has(next + edge.side.step(), turned(along.opposite())) {
            return End::Back;
        }
        End::Square
    }

    /// Every cube of the kind in `cubes`, each with its dressed edges, in order.
    pub fn all(&self, cubes: impl IntoIterator<Item = IVec3>) -> Vec<Dressed> {
        cubes
            .into_iter()
            .flat_map(|cube| self.dressed(cube))
            .collect()
    }
}

/// The ground's height under a column of a base, in ticks of its grid: its sample, or
/// where it has none (a base keeps samples only under its cubes, D206), the mean of those
/// beside it, square on before diagonally, or the base's level.
pub fn ground_ticks(base: &Base, column: [i32; 2]) -> i32 {
    let ground = base.ground();
    let [x, z] = column;
    let mean = |offsets: &[[i32; 2]]| {
        let found: Vec<i32> = offsets
            .iter()
            .filter_map(|[dx, dz]| ground.sample([x + dx, z + dz]).map(i32::from))
            .collect();
        (!found.is_empty()).then(|| found.iter().sum::<i32>().div_euclid(found.len() as i32))
    };
    let cm = ground
        .sample(column)
        .map(i32::from)
        .or_else(|| mean(&[[1, 0], [-1, 0], [0, 1], [0, -1]]))
        .or_else(|| mean(&[[1, 1], [-1, 1], [1, -1], [-1, -1]]))
        .unwrap_or(i32::from(crate::ground::LEVEL_CM));
    (cm * TICKS).div_euclid(crate::ground::cube_cm())
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
mod tests {
    use super::*;
    use crate::kit::Kit;
    use crate::{BuildOp, Cube};

    const STONE: Kind = Kind(2);
    const LOGS: Kind = Kind(1);

    fn kits() -> Kits {
        Kits(vec![
            Kit::default(),
            Kit {
                id: 2,
                pattern: Pattern::Masonry,
                ..Kit::default()
            },
        ])
    }

    fn fill(base: &mut Base, min: [i32; 3], max: [i32; 3], kind: Kind) {
        base.apply(BuildOp::Fill {
            min,
            max,
            cube: Some(Cube::new(kind, Fill::Solid)),
        });
    }

    fn base(boxes: &[([i32; 3], [i32; 3])]) -> Base {
        let mut base = Base::default();
        for &(min, max) in boxes {
            fill(&mut base, min, max, STONE);
        }
        base
    }

    /// Every dressed edge of the kind's cubes.
    fn all(base: &Base) -> Vec<Dressed> {
        let kits = kits();
        let roles = Roles::new(base, STONE, &kits);
        roles.all(base.cubes().map(|(at, _)| at))
    }

    fn count(dressed: &[Dressed], role: Role) -> usize {
        dressed.iter().filter(|d| d.edge.role == role).count()
    }

    fn at(dressed: &[Dressed], cube: [i32; 3], role: Role) -> Vec<&Dressed> {
        dressed
            .iter()
            .filter(|d| d.cube == IVec3::from_array(cube) && d.edge.role == role)
            .collect()
    }

    #[test]
    fn a_wall_top_under_a_roofs_cubes_is_no_crest_and_one_open_to_the_sky_is() {
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [7, 0, 5],
            kind: STONE,
        });
        // Open to the sky: its walls' tops are crests.
        base.apply(BuildOp::SetRoof {
            min: [1, 0, 1],
            max: [6, 0, 4],
            laid: false,
            kind: STONE,
        });
        assert!(count(&all(&base), Role::Crest) > 0);
        // Under a gable of roof cubes, none: the roof is over them, and a coping there
        // would stand up through it.
        let footprint = (0..8).flat_map(|x| (0..6).map(move |z| [x, z])).collect();
        let plan = crate::roofer::over(&footprint, 6, crate::roofer::Style::Gable, None);
        for (at, shape) in plan.roof {
            base.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: Some(Cube {
                    shape,
                    ..Cube::new(Kind::FIRST_ROOF, Fill::Solid)
                }),
            });
        }
        for at in plan.gables {
            fill(&mut base, at.to_array(), at.to_array(), STONE);
        }
        assert_eq!(count(&all(&base), Role::Crest), 0);
    }

    #[test]
    fn a_gable_carried_up_into_its_roof_has_no_edge_at_its_steps() {
        // A gable of stone under roof cubes of a roof kit: the wall runs on into the roof
        // cubes standing on it, so the steps the gable's cubes climb in have no corner up
        // them, no cap or top along them, nothing to stand out under the roof.
        let mut kits = kits();
        kits.0.push(Kit {
            id: Kind::FIRST_ROOF.0,
            pattern: Pattern::Roof,
            ..Kit::default()
        });
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [7, 0, 5],
            kind: STONE,
        });
        let footprint = (0..8).flat_map(|x| (0..6).map(move |z| [x, z])).collect();
        let plan = crate::roofer::over(&footprint, 6, crate::roofer::Style::Gable, None);
        for (at, shape) in plan.roof {
            base.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: Some(Cube {
                    shape,
                    ..Cube::new(Kind::FIRST_ROOF, Fill::Solid)
                }),
            });
        }
        for at in plan.gables {
            fill(&mut base, at.to_array(), at.to_array(), STONE);
        }
        let roles = Roles::new(&base, STONE, &kits);
        let gable: Vec<IVec3> = base
            .cubes()
            .filter(|(at, c)| c.kind == STONE && at.y >= 6)
            .map(|(at, _)| at)
            .collect();
        assert!(gable.len() > 10);
        for at in gable {
            for edge in roles.edges(at) {
                assert!(
                    edge.other != Dir::Up && edge.role != Role::Arris,
                    "{at}: {edge:?} on a gable under its roof"
                );
            }
        }
    }

    #[test]
    fn a_free_wall_has_arrises_up_its_ends_crests_along_its_top_and_a_foot() {
        // Four cubes long, three high, one thick, standing on the ground at the base's level.
        let base = base(&[([0, 0, 0], [3, 2, 0])]);
        let d = all(&base);
        // Four uprights at its ends, each cube of them.
        assert_eq!(count(&d, Role::Arris), 4 * 3);
        // Along the top, both faces and both ends.
        assert_eq!(count(&d, Role::Crest), 4 * 2 + 2);
        // Along the foot likewise: it stands on the ground.
        assert_eq!(count(&d, Role::Foot), 4 * 2 + 2);
        for role in [
            Role::Soffit,
            Role::Jamb,
            Role::Inside,
            Role::Border,
            Role::Nose,
        ] {
            assert_eq!(count(&d, role), 0, "{role:?}");
        }
        // The crest along the south face runs on between cubes and turns out round the
        // corners.
        let south = |x: i32| {
            at(&d, [x, 2, 0], Role::Crest)
                .into_iter()
                .find(|e| e.edge.side == Dir::South)
                .unwrap()
                .ends
        };
        assert_eq!(south(0), [End::Out, End::On]);
        assert_eq!(south(1), [End::On, End::On]);
        assert_eq!(south(3), [End::On, End::Out]);
        // Each edge belongs to one cube only.
        let mut seen = std::collections::BTreeSet::new();
        for e in &d {
            let line = line(e.cube, e.edge);
            assert!(seen.insert((line, e.edge.side, e.edge.other)), "{e:?}");
        }
    }

    #[test]
    fn an_l_has_an_inside_corner_whose_crests_mitre_back_into_it() {
        // An L: along x from the corner, and along z from it.
        let base = base(&[([0, 0, 0], [3, 1, 0]), ([0, 0, 1], [0, 1, 3])]);
        let d = all(&base);
        // The inside corner, up the line at x = 1, z = 1, belongs to the cube looking east
        // into it.
        let inside: Vec<_> = d.iter().filter(|e| e.edge.role == Role::Inside).collect();
        assert_eq!(inside.len(), 2, "{inside:?}");
        assert!(
            inside
                .iter()
                .all(|e| e.cube.x == 0 && e.cube.z == 1 && e.edge.side == Dir::East)
        );
        // The crest along the inner face of the x arm stops mitred back at the corner.
        let crest = at(&d, [1, 1, 0], Role::Crest)
            .into_iter()
            .find(|e| e.edge.side == Dir::South)
            .unwrap();
        assert_eq!(crest.ends[0], End::Back);
        // The outer corner's crests mitre out.
        let outer = at(&d, [0, 1, 0], Role::Crest);
        assert!(
            outer
                .iter()
                .any(|e| e.edge.side == Dir::North && e.ends == [End::Out, End::On])
        );
    }

    #[test]
    fn a_hole_has_jambs_a_sill_and_a_head() {
        let mut base = base(&[([0, 0, 0], [5, 5, 0])]);
        // A window carved out of it: an opening's cubes.
        base.apply(BuildOp::Fill {
            min: [2, 2, 0],
            max: [3, 3, 0],
            cube: Some(Cube::new(STONE, Fill::Open)),
        });
        let d = all(&base);
        // Jambs up both sides of it, both faces, two cubes high.
        assert_eq!(count(&d, Role::Jamb), 2 * 2 * 2);
        // A sill on both faces under it, a head over it, each two cubes long.
        assert_eq!(count(&d, Role::Sill), 2 * 2);
        assert_eq!(count(&d, Role::Head), 2 * 2);
        let sill = at(&d, [2, 1, 0], Role::Sill);
        assert!(sill.iter().all(|e| e.edge.other == Dir::Up));
        // The sill runs on into the next cube's and stops square at the jambs.
        assert!(sill.iter().all(|e| e.ends == [End::Square, End::On]));
    }

    #[test]
    fn an_overhang_has_soffits_and_steps_have_noses() {
        // A pillar under a slab overhanging it.
        let overhang = base(&[([0, 0, 0], [0, 3, 0]), ([-2, 4, -1], [2, 4, 1])]);
        let d = all(&overhang);
        assert!(count(&d, Role::Soffit) >= 4 * 2);
        // A flight of three steps, each a cube higher, rising north.
        let mut steps = Base::default();
        for step in 0..3 {
            fill(&mut steps, [0, 0, -step], [2, step, -step], STONE);
        }
        let d = all(&steps);
        // Each step's south edge is a nose but the top one's, which nothing rises behind.
        assert_eq!(count(&d, Role::Nose), 2 * 3, "{:?}", d);
        assert!(
            d.iter()
                .filter(|e| e.edge.role == Role::Nose)
                .all(|e| e.edge.side == Dir::South)
        );
        // A riser's foot on the step under it is no floor border: nothing runs on beyond.
        assert_eq!(count(&d, Role::Border), 0);
    }

    #[test]
    fn a_wall_on_a_floor_has_a_border_inside_and_a_foot_outside() {
        // A floor three cubes deep, a wall standing on its north edge.
        let base = base(&[([0, 0, 0], [3, 0, 2]), ([0, 1, 0], [3, 2, 0])]);
        let d = all(&base);
        // Along the wall's south face, where the floor runs on south of it.
        let border: Vec<_> = d.iter().filter(|e| e.edge.role == Role::Border).collect();
        assert_eq!(border.len(), 4, "{border:?}");
        assert!(
            border
                .iter()
                .all(|e| e.cube.y == 1 && e.edge.side == Dir::South)
        );
    }

    #[test]
    fn logs_on_stone_meet_it_with_a_cap_and_stone_quoined_beside_logs() {
        // A stone plinth a cube high, a log wall on it, the stone running on east past it.
        let mut base = base(&[([0, 0, 0], [5, 0, 0])]);
        fill(&mut base, [0, 1, 0], [3, 3, 0], LOGS);
        fill(&mut base, [4, 1, 0], [5, 3, 0], STONE);
        let d = all(&base);
        // The plinth's top under the logs meets them on both faces.
        assert_eq!(at(&d, [1, 0, 0], Role::Meet).len(), 2);
        // The stone beside the logs is quoined up its end against them: its faces show
        // past the thinner logs.
        assert_eq!(at(&d, [4, 2, 0], Role::Arris).len(), 2);
        // Seen from the logs, they stand on another kind: a foot.
        let kits = kits();
        let logs = Roles::new(&base, LOGS, &kits);
        let foot = logs
            .dressed(IVec3::new(1, 1, 0))
            .into_iter()
            .filter(|e| e.edge.role == Role::Foot)
            .count();
        assert_eq!(foot, 2);
    }

    #[test]
    fn stone_over_logs_changed_into_a_wall_has_a_foot_that_does_not_sink_over_them() {
        // A stone wall with a block of logs two wide and two high changed into it, a
        // metre up: the stone over the logs stands on them, its foot level with them.
        let mut base = base(&[([0, 0, 0], [9, 5, 0])]);
        fill(&mut base, [4, 2, 0], [5, 3, 0], LOGS);
        let d = all(&base);
        let feet = at(&d, [4, 4, 0], Role::Foot);
        assert_eq!(feet.len(), 2);
        assert!(feet.iter().all(|foot| foot.sink == 0 && foot.lift == 0));
        // Its sides quoined, the stone running through, and capped under them.
        assert_eq!(at(&d, [3, 2, 0], Role::Arris).len(), 2);
        assert_eq!(at(&d, [4, 1, 0], Role::Meet).len(), 2);
    }

    #[test]
    fn how_kinds_meet_is_the_kits_table_not_their_pattern() {
        use crate::kit::{Joint, Meet, Meets};
        let mut base = base(&[([0, 0, 0], [5, 0, 0])]);
        fill(&mut base, [0, 1, 0], [3, 3, 0], LOGS);
        fill(&mut base, [4, 1, 0], [5, 3, 0], STONE);
        // The defaults: stone runs through logs and caps them, logs end with a board.
        let kits = kits();
        assert!(kits.through(STONE, LOGS) && !kits.through(LOGS, STONE));
        assert_eq!(kits.carries(STONE, LOGS), Joint::Cap);
        assert_eq!(kits.end(LOGS, STONE), Joint::Board);
        // A stone whose table lets logs run through it: no quoins up the joint, no cap, and
        // the logs' table read the other way round.
        let mut table = Meets::of(Pattern::Masonry);
        table.kinds.insert(
            LOGS.0,
            Meet {
                through: Some(false),
                ..Meet::default()
            },
        );
        let mut changed = kits.clone();
        changed.0[1].meets = Some(table);
        assert!(changed.through(LOGS, STONE));
        assert_eq!(changed.end(LOGS, STONE), Joint::None);
        let roles = Roles::new(&base, STONE, &changed);
        let d = roles.all(base.cubes().map(|(at, _)| at));
        assert!(at(&d, [1, 0, 0], Role::Meet).is_empty());
        assert!(at(&d, [4, 2, 0], Role::Arris).is_empty());
        // A heavier timber, by weight alone, runs through stone the same way.
        let mut heavy = kits.clone();
        heavy.0[0].meets = Some(Meets {
            weight: 4,
            ..Meets::of(Pattern::Logs)
        });
        assert!(heavy.through(LOGS, STONE));
    }

    #[test]
    fn a_foot_on_a_slope_steps_with_the_ground() {
        // A wall along x on footings down into ground rising a quarter cube a cube east.
        let mut base = Base::default();
        let cm = crate::ground::cube_cm();
        for x in 0..8 {
            fill(&mut base, [x, -1, 0], [x, 3, 0], STONE);
        }
        // Sampled once the cubes are there: a base keeps samples only under its cubes.
        for x in -1..9 {
            base.sample_ground((-1..=1).map(|z| crate::Sample {
                column: [x, z],
                height_cm: (x * cm / 4) as i16,
            }));
        }

        let d = all(&base);
        let feet: Vec<_> = d
            .iter()
            .filter(|e| e.edge.role == Role::Foot && e.edge.side == Dir::South)
            .collect();
        // A foot on every cube along it: the ground passes each.
        assert_eq!(feet.len(), 8, "{feet:?}");
        // Raised by whole steps, never down into the ground, and going on down to it.
        let ground = |x: i32| x * TICKS / 4;
        for e in &feet {
            let x = e.cube.x;
            let edge = e.cube.y * TICKS + e.lift;
            assert_eq!(e.lift % LIFT_STEP, 0);
            assert!(edge >= ground(x + 1).min(e.cube.y * TICKS + TICKS), "{e:?}");
            assert!(edge - e.sink <= ground(x - 1).max(ground(0)), "{e:?}");
        }
        // Where the lift changes, the ends step.
        assert!(feet.iter().any(|e| e.ends.contains(&End::Step)));
        // Nothing is dressed under the ground.
        assert!(d.iter().all(|e| e.edge.role == Role::Foot || e.cube.y >= 0));
    }

    #[test]
    fn roles_are_the_same_worked_out_twice_and_only_for_the_kind() {
        let base = base(&[([0, 0, 0], [3, 2, 0])]);
        assert_eq!(all(&base), all(&base));
        let kits = kits();
        let logs = Roles::new(&base, LOGS, &kits);
        assert!(logs.dressed(IVec3::ZERO).is_empty());
    }
}
