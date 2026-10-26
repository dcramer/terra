//! Stage 2, placement (D473): the graph laid on a grid of cells, 4 m square and a level (6 m)
//! high (D527). Rooms are rectangles of cells set beside the room they hang from, in the graph's
//! order, on a level their danger asks for; corridors are routed between them by A*
//! (`route`), with a stair where they change level. Every cell knows its space and what each
//! of its four sides is: a wall, open into the same space, a door or a stair's way.
//!
//! Cells are `IVec3`: x east, y the level counted down from 0 (the first below the surface),
//! z south. The entrance is a 2 by 2 room at the origin with a stair up to the surface north
//! of it.

use crate::graph::Graph;
use crate::route;
use crate::theme::{Role, Theme};
use glam::{IVec2, IVec3, Vec2};
use std::collections::{HashMap, VecDeque};
use terra_gen::{Rng, Seed};

/// North, east, south, west.
pub const STEPS: [IVec3; 4] = [
    IVec3::new(0, 0, -1),
    IVec3::new(1, 0, 0),
    IVec3::new(0, 0, 1),
    IVec3::new(-1, 0, 0),
];

pub fn opposite(side: usize) -> usize {
    (side + 2) % 4
}

/// Tries at the whole placement, each from its own seed, before a graph is given up on.
pub const ATTEMPTS: u64 = 12;
/// Tries at a site for each room.
const SITES: u32 = 30;

/// What a cell is part of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Space {
    Room(usize),
    /// A corridor, by the graph's edge it joins.
    Corridor(usize),
    /// A stair, by its index in `Layout::stairs`.
    Stair(usize),
}

/// What one side of a cell is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Wall,
    /// Into the same space.
    Open,
    /// A doorway between a room and a corridor, by the graph's edge.
    Door(usize),
    /// Between a corridor (or the entrance) and a stair.
    Way,
}

#[derive(Clone, Debug)]
pub struct Cell {
    pub at: IVec3,
    pub space: Space,
    pub sides: [Side; 4],
}

/// A room's cells: `size` across (x) and along (z) from `min`, on level `min.y`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub min: IVec3,
    pub size: IVec2,
}

impl Rect {
    pub fn cells(self) -> impl Iterator<Item = IVec3> {
        (0..self.size.y)
            .flat_map(move |z| (0..self.size.x).map(move |x| self.min + IVec3::new(x, 0, z)))
    }

    pub fn contains(self, at: IVec3) -> bool {
        let d = at - self.min;
        d.y == 0 && d.x >= 0 && d.z >= 0 && d.x < self.size.x && d.z < self.size.y
    }

    /// The middle, in cells, across and along.
    pub fn centre(self) -> Vec2 {
        Vec2::new(self.min.x as f32, self.min.z as f32) + self.size.as_vec2() * 0.5
    }

    /// Cells from the rectangle to `at` across and along, 0 inside.
    pub fn reach(self, at: IVec3) -> i32 {
        let max = self.min + IVec3::new(self.size.x - 1, 0, self.size.y - 1);
        let dx = (self.min.x - at.x).max(at.x - max.x).max(0);
        let dz = (self.min.z - at.z).max(at.z - max.z).max(0);
        dx + dz
    }
}

/// A straight stair of two cells a level: it rises along `up` from `at` (on the lower level)
/// over `at + up`; the cells above are its well and its landing. Walked in from `at - up` on
/// the lower level, out to `at + 2 up` on the upper. The entrance's stair (`edge` none) rises
/// to the surface and has no upper cells.
#[derive(Clone, Debug)]
pub struct Stair {
    pub edge: Option<usize>,
    pub at: IVec3,
    pub up: usize,
    pub cells: Vec<IVec3>,
}

#[derive(Clone, Debug)]
pub struct Layout {
    /// By room.
    pub rooms: Vec<Rect>,
    /// By edge: the corridor's cells from room `a` to room `b`, stairs left out.
    pub corridors: Vec<Vec<IVec3>>,
    pub stairs: Vec<Stair>,
    pub cells: Vec<Cell>,
    /// Whole placements tried.
    pub attempts: u32,
    pub levels: i32,
    index: HashMap<IVec3, usize>,
    /// Cells joined across levels: a stair's top to its landing.
    vertical: Vec<(usize, usize)>,
}

/// A layout being built.
pub(crate) struct Plan {
    pub levels: i32,
    pub occupied: HashMap<IVec3, Space>,
    pub rooms: Vec<Rect>,
    pub corridors: Vec<Vec<IVec3>>,
    pub stairs: Vec<Stair>,
    joins: HashMap<[[i32; 3]; 2], Side>,
}

fn key(a: IVec3, b: IVec3) -> [[i32; 3]; 2] {
    let (a, b) = (a.to_array(), b.to_array());
    if a < b { [a, b] } else { [b, a] }
}

/// The graph laid on the grid, or why it could not be.
pub fn place(graph: &Graph, theme: &Theme, seed: Seed) -> Result<Layout, String> {
    let mut why = String::new();
    for attempt in 0..ATTEMPTS {
        match place_once(graph, theme, seed.derive("place").index(attempt)) {
            Ok(mut layout) => {
                layout.attempts = attempt as u32 + 1;
                return Ok(layout);
            }
            Err(e) => why = e,
        }
    }
    Err(why)
}

fn place_once(graph: &Graph, theme: &Theme, seed: Seed) -> Result<Layout, String> {
    let mut rng = seed.rng();
    let mut plan = Plan {
        levels: theme.levels.max(1) as i32,
        occupied: HashMap::new(),
        rooms: Vec::new(),
        corridors: vec![Vec::new(); graph.edges.len()],
        stairs: Vec::new(),
        joins: HashMap::new(),
    };
    assert_eq!(
        graph.entrance(),
        0,
        "rooms are placed in order, from the entrance"
    );
    plan.add_room(Rect {
        min: IVec3::ZERO,
        size: IVec2::splat(2),
    });
    // The stair up to the surface, north of the entrance.
    let surface = vec![IVec3::new(0, 0, -1), IVec3::new(0, 0, -2)];
    for &at in &surface {
        plan.occupied.insert(at, Space::Stair(0));
    }
    plan.join(IVec3::ZERO, surface[0], Side::Way);
    plan.join(surface[0], surface[1], Side::Open);
    plan.stairs.push(Stair {
        edge: None,
        at: surface[0],
        up: 0,
        cells: surface,
    });

    for room in 1..graph.rooms.len() {
        let rect = plan
            .site(graph, room, &mut rng)
            .ok_or_else(|| format!("no site for room {room}"))?;
        plan.add_room(rect);
    }
    let mut order: Vec<usize> = (0..graph.edges.len()).collect();
    let span = |e: usize| {
        let (a, b) = (plan.rooms[graph.edges[e].a], plan.rooms[graph.edges[e].b]);
        (a.centre() - b.centre()).abs().element_sum() as i32 + 4 * (a.min.y - b.min.y).abs()
    };
    order.sort_by_key(|&e| span(e));
    for e in order {
        route::route(&mut plan, graph, e).ok_or_else(|| format!("no route for edge {e}"))?;
    }
    Ok(plan.finish())
}

fn size_of(role: Role, rng: &mut Rng) -> IVec2 {
    match role {
        Role::Entrance => IVec2::splat(2),
        Role::Goal => IVec2::splat(4),
        Role::Guard | Role::Puzzle => IVec2::splat(3),
        Role::Rest if rng.unit() < 0.5 => IVec2::new(2, 3),
        Role::Rest => IVec2::new(3, 2),
        _ => IVec2::new(2 + rng.below(2) as i32, 2 + rng.below(2) as i32),
    }
}

impl Plan {
    fn add_room(&mut self, rect: Rect) {
        let room = self.rooms.len();
        for at in rect.cells() {
            self.occupied.insert(at, Space::Room(room));
        }
        self.rooms.push(rect);
    }

    pub fn free(&self, at: IVec3) -> bool {
        at.y >= 0 && at.y < self.levels && !self.occupied.contains_key(&at)
    }

    pub fn join(&mut self, a: IVec3, b: IVec3, side: Side) {
        self.joins.insert(key(a, b), side);
    }

    /// A site for a room beside its parent: the best of a number of tries by closeness to the
    /// rooms already placed it links to, a cell's margin from everything else.
    fn site(&self, graph: &Graph, room: usize, rng: &mut Rng) -> Option<Rect> {
        let r = &graph.rooms[room];
        let parent = self.rooms[r.parent?];
        let size = size_of(r.role, rng);
        let target = ((r.danger * self.levels as f32) as i32).min(self.levels - 1);
        let level = target
            .clamp(parent.min.y - 1, parent.min.y + 1)
            .clamp(0, self.levels - 1);
        let linked: Vec<usize> = graph
            .edges_of(room)
            .map(|(_, e)| if e.a == room { e.b } else { e.a })
            .filter(|&other| other < room)
            .collect();
        let mut best: Option<(f32, Rect)> = None;
        for _ in 0..SITES {
            let side = rng.below(4) as usize;
            let gap = if level == parent.min.y {
                2 + rng.below(3)
            } else {
                5 + rng.below(3)
            } as i32;
            let mut min = beside(parent, size, side, gap, rng);
            min.y = level;
            let rect = Rect { min, size };
            if !self.clear(rect) {
                continue;
            }
            let score = linked
                .iter()
                .map(|&o| {
                    let other = self.rooms[o];
                    (other.centre() - rect.centre()).abs().element_sum()
                        + 4.0 * (other.min.y - level).abs() as f32
                })
                .sum::<f32>()
                + rng.unit();
            if best.is_none_or(|(s, _)| score < s) {
                best = Some((score, rect));
            }
        }
        best.map(|(_, rect)| rect)
    }

    /// Nothing in the rectangle or a cell around it.
    fn clear(&self, rect: Rect) -> bool {
        (-1..=rect.size.y).all(|z| {
            (-1..=rect.size.x).all(|x| {
                !self
                    .occupied
                    .contains_key(&(rect.min + IVec3::new(x, 0, z)))
            })
        })
    }

    fn finish(self) -> Layout {
        let mut cells = Vec::new();
        for (room, rect) in self.rooms.iter().enumerate() {
            cells.extend(rect.cells().map(|at| (at, Space::Room(room))));
        }
        for (edge, run) in self.corridors.iter().enumerate() {
            cells.extend(run.iter().map(|&at| (at, Space::Corridor(edge))));
        }
        for (stair, s) in self.stairs.iter().enumerate() {
            cells.extend(s.cells.iter().map(|&at| (at, Space::Stair(stair))));
        }
        let index: HashMap<IVec3, usize> = cells
            .iter()
            .enumerate()
            .map(|(i, &(at, _))| (at, i))
            .collect();
        let cells: Vec<Cell> = cells
            .iter()
            .map(|&(at, space)| {
                let sides = std::array::from_fn(|d| {
                    let next = at + STEPS[d];
                    if let Some(&side) = self.joins.get(&key(at, next)) {
                        side
                    } else if matches!(space, Space::Room(_))
                        && self.occupied.get(&next) == Some(&space)
                    {
                        Side::Open
                    } else {
                        Side::Wall
                    }
                });
                Cell { at, space, sides }
            })
            .collect();
        let vertical = self
            .stairs
            .iter()
            .filter(|s| s.edge.is_some())
            .map(|s| {
                let top = s.at + STEPS[s.up];
                (index[&top], index[&(top - IVec3::Y)])
            })
            .collect();
        Layout {
            rooms: self.rooms,
            corridors: self.corridors,
            stairs: self.stairs,
            cells,
            attempts: 0,
            levels: self.levels,
            index,
            vertical,
        }
    }
}

/// Where a room of `size` goes on `side` of `parent`, `gap` cells off, overlapping it by at
/// least a cell across, so a corridor can run straight.
fn beside(parent: Rect, size: IVec2, side: usize, gap: i32, rng: &mut Rng) -> IVec3 {
    let mut slide =
        |own: i32, theirs: i32| -(own - 1) + rng.below((own + theirs - 1) as u32) as i32;
    let p = parent.min;
    match side {
        0 => IVec3::new(p.x + slide(size.x, parent.size.x), 0, p.z - gap - size.y),
        1 => IVec3::new(
            p.x + parent.size.x + gap,
            0,
            p.z + slide(size.y, parent.size.y),
        ),
        2 => IVec3::new(
            p.x + slide(size.x, parent.size.x),
            0,
            p.z + parent.size.y + gap,
        ),
        _ => IVec3::new(p.x - gap - size.x, 0, p.z + slide(size.y, parent.size.y)),
    }
}

impl Layout {
    pub fn cell(&self, at: IVec3) -> Option<usize> {
        self.index.get(&at).copied()
    }

    /// The cells one step from a cell: through every side that is not a wall, and up or down
    /// a stair.
    pub fn neighbours(&self, cell: usize) -> impl Iterator<Item = usize> + '_ {
        let c = &self.cells[cell];
        let level = (0..4)
            .filter(move |&d| c.sides[d] != Side::Wall)
            .filter_map(move |d| self.cell(c.at + STEPS[d]));
        let across = self.vertical.iter().filter_map(move |&(a, b)| {
            if a == cell {
                Some(b)
            } else if b == cell {
                Some(a)
            } else {
                None
            }
        });
        level.chain(across)
    }

    /// The cells reached from the entrance through cells `walkable` allows.
    pub fn reached(&self, walkable: impl Fn(usize) -> bool) -> Vec<bool> {
        let mut seen = vec![false; self.cells.len()];
        let start = self.cell(IVec3::ZERO).expect("the entrance");
        seen[start] = true;
        let mut queue = VecDeque::from([start]);
        while let Some(cell) = queue.pop_front() {
            for next in self.neighbours(cell) {
                if !seen[next] && walkable(next) {
                    seen[next] = true;
                    queue.push_back(next);
                }
            }
        }
        seen
    }

    /// Lowest and highest cell.
    pub fn bounds(&self) -> (IVec3, IVec3) {
        let min = self.cells.iter().fold(IVec3::MAX, |m, c| m.min(c.at));
        let max = self.cells.iter().fold(IVec3::MIN, |m, c| m.max(c.at));
        (min, max)
    }

    /// The layout's promises: sides agree from both cells, cells add up to rooms, corridors
    /// and stairs (nothing overlaps), only what the graph joins is joined, and each edge's
    /// rooms are joined by its own cells.
    pub fn verify(&self, graph: &Graph) -> Result<(), String> {
        let area: usize = self
            .rooms
            .iter()
            .map(|r| (r.size.x * r.size.y) as usize)
            .sum::<usize>()
            + self.corridors.iter().map(Vec::len).sum::<usize>()
            + self.stairs.iter().map(|s| s.cells.len()).sum::<usize>();
        if area != self.cells.len() || self.index.len() != self.cells.len() {
            return Err(format!(
                "{} cells for an area of {area}: something overlaps",
                self.cells.len()
            ));
        }
        for cell in &self.cells {
            for (d, &side) in cell.sides.iter().enumerate() {
                let Some(next) = self.cell(cell.at + STEPS[d]).map(|n| &self.cells[n]) else {
                    if side != Side::Wall {
                        return Err(format!("{} opens onto nothing", cell.at));
                    }
                    continue;
                };
                if next.sides[opposite(d)] != side {
                    return Err(format!("{} and {} disagree", cell.at, next.at));
                }
                self.allowed(graph, cell.space, next.space, side)
                    .map_err(|e| format!("{} to {}: {e}", cell.at, next.at))?;
            }
        }
        for (e, edge) in graph.edges.iter().enumerate() {
            let own = |space: Space| match space {
                Space::Room(r) => r == edge.a || r == edge.b,
                Space::Corridor(c) => c == e,
                Space::Stair(s) => self.stairs[s].edge == Some(e),
            };
            let mut seen = vec![false; self.cells.len()];
            let start = self.cell(self.rooms[edge.a].min).unwrap();
            seen[start] = true;
            let mut queue = VecDeque::from([start]);
            while let Some(cell) = queue.pop_front() {
                for next in self.neighbours(cell) {
                    if !seen[next] && own(self.cells[next].space) {
                        seen[next] = true;
                        queue.push_back(next);
                    }
                }
            }
            if !self.rooms[edge.b].cells().any(|at| seen[self.index[&at]]) {
                return Err(format!(
                    "edge {e} ({} to {}) is not walkable",
                    edge.a, edge.b
                ));
            }
        }
        Ok(())
    }

    /// Whether a side of `side`'s kind may join these two spaces.
    fn allowed(&self, graph: &Graph, a: Space, b: Space, side: Side) -> Result<(), String> {
        let ends = |e: usize, r: usize| graph.edges[e].a == r || graph.edges[e].b == r;
        let ok = match (side, a, b) {
            (Side::Wall, _, _) => true,
            (Side::Open, a, b) => a == b,
            (Side::Door(e), Space::Room(r), Space::Corridor(c))
            | (Side::Door(e), Space::Corridor(c), Space::Room(r)) => c == e && ends(e, r),
            (Side::Way, Space::Corridor(c), Space::Stair(s))
            | (Side::Way, Space::Stair(s), Space::Corridor(c)) => self.stairs[s].edge == Some(c),
            (Side::Way, Space::Room(r), Space::Stair(s))
            | (Side::Way, Space::Stair(s), Space::Room(r)) => {
                r == graph.entrance() && self.stairs[s].edge.is_none()
            }
            _ => false,
        };
        if ok {
            Ok(())
        } else {
            Err(format!("{side:?} joins {a:?} and {b:?}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::generate;
    use crate::theme::assets;

    #[test]
    fn every_placement_keeps_its_promises() {
        for theme in Theme::all(&assets()).unwrap() {
            let seeds: Vec<u64> = (0..1000).collect();
            let results = terra_gen::par::map(&seeds, |&s| {
                let graph = generate(&theme, Seed(s));
                place(&graph, &theme, Seed(s)).map(|layout| {
                    layout
                        .verify(&graph)
                        .unwrap_or_else(|e| panic!("{} seed {s}: {e}", theme.name));
                    let reached = layout.reached(|_| true);
                    assert!(
                        reached.iter().all(|&r| r),
                        "{} seed {s}: a cell out of reach",
                        theme.name
                    );
                    let levels = layout.cells.iter().map(|c| c.at.y).max().unwrap();
                    assert!(levels < theme.levels as i32);
                    layout.attempts
                })
            });
            let placed = results.iter().filter(|r| r.is_ok()).count();
            let tries: u32 = results.iter().filter_map(|r| r.as_ref().ok()).sum();
            println!(
                "{}: {placed} of 1000 placed, {:.2} tries each",
                theme.name,
                tries as f32 / placed.max(1) as f32
            );
            assert!(
                placed >= 990,
                "{}: only {placed} of 1000 placed",
                theme.name
            );
        }
    }

    #[test]
    fn deeper_rooms_go_down() {
        let theme = Theme::all(&assets())
            .unwrap()
            .into_iter()
            .find(|t| t.levels >= 2)
            .unwrap();
        let mut below = 0;
        for s in 0..50 {
            let graph = generate(&theme, Seed(s));
            if let Ok(layout) = place(&graph, &theme, Seed(s)) {
                below += usize::from(layout.rooms[graph.goal()].min.y > 0);
                assert!(!layout.stairs.iter().skip(1).any(|st| st.cells.len() != 4));
            }
        }
        assert!(
            below >= 40,
            "the goal was below the first level in {below} of 50"
        );
    }
}
