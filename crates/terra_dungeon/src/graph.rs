//! Stage 1, the mission graph (D473): which rooms there are, what each is for, how they are
//! linked, and what is in them, before any of it has a place. Dormans' mission graphs as
//! Unexplored builds them: a main path from the entrance to the goal with a locked gate before
//! the goal, a loop off the main path that the key rooms hang from, a shortcut back, a secret,
//! a rest and treasure with what is left of the room budget.

use crate::theme::{Role, Theme};
use terra_gen::{Rng, Seed};

/// How two rooms are joined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    /// An open way, an arch.
    Corridor,
    /// A door that opens either way.
    Door,
    /// A door that opens when every key of its lock is held.
    Locked(u8),
    /// Passable from `a` to `b` only: a door barred on `b`'s side.
    OneWay,
    /// A hidden way, passable once found.
    Secret,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    pub a: usize,
    pub b: usize,
    pub link: Link,
}

impl Edge {
    /// The ways it can be walked, from and to.
    pub fn ways(&self) -> impl Iterator<Item = (usize, usize)> {
        let back = (self.link != Link::OneWay).then_some((self.b, self.a));
        std::iter::once((self.a, self.b)).chain(back)
    }
}

/// A group of one species.
#[derive(Clone, Debug, PartialEq)]
pub struct Mobs {
    pub species: String,
    pub count: u32,
}

/// A stack of one item.
#[derive(Clone, Debug, PartialEq)]
pub struct Loot {
    pub item: String,
    pub count: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Room {
    pub role: Role,
    /// The room it was hung from; placement puts it beside that one.
    pub parent: Option<usize>,
    /// Rooms from the entrance, walking any way that can be walked.
    pub depth: u32,
    /// 0 at the entrance to 1 at the goal.
    pub danger: f32,
    /// The lock this room holds a key to.
    pub key: Option<u8>,
    pub mobs: Vec<Mobs>,
    pub loot: Vec<Loot>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Graph {
    pub rooms: Vec<Room>,
    pub edges: Vec<Edge>,
    /// The main path, entrance to goal.
    pub main: Vec<usize>,
}

/// Rooms every graph has: the main path's five, a loop room, the shortcut, the rest, the
/// secret and a treasure, and the key rooms.
pub fn least_rooms(keys: u32) -> u32 {
    10 + keys
}

/// The graph of a theme's dungeon from a seed.
pub fn generate(theme: &Theme, seed: Seed) -> Graph {
    let mut rng = seed.derive("graph").rng();
    let keys = theme.keys.max(1);
    let least = least_rooms(keys);
    let (low, high) = (theme.rooms.0.max(least), theme.rooms.1.max(least));
    let size = low + rng.below(high - low + 1);
    let mut spare = size - least;
    let more_main = rng.below(spare.min(2) + 1);
    spare -= more_main;
    let more_loop = if spare > 0 { rng.below(2) } else { 0 };
    spare -= more_loop;

    let mut b = Builder::default();
    // The main path: entrance, passages, the gate into the guard room, the goal.
    let mut main = vec![b.add(Role::Entrance, None, Link::Door)];
    for _ in 0..2 + more_main {
        let link = open(&mut rng);
        main.push(b.add(Role::Passage, main.last().copied(), link));
    }
    let gate = main.len() - 1;
    main.push(b.add(Role::Guard, Some(main[gate]), Link::Locked(0)));
    main.push(b.add(Role::Goal, main.last().copied(), Link::Door));

    // The loop: one or two rooms from main[i] back to main[j], no shorter than the main path
    // between them, so depth along the main path keeps rising.
    let rooms = 1 + more_loop as usize;
    let from = rng.below(gate as u32 - 1) as usize;
    let last = (from + rooms + 1).min(gate);
    let to = from + 2 + rng.below((last - from - 1) as u32) as usize;
    let mut branch = Vec::new();
    for k in 0..rooms {
        let parent = if k == 0 { main[from] } else { branch[k - 1] };
        let link = open(&mut rng);
        branch.push(b.add(Role::Branch, Some(parent), link));
    }
    let link = open(&mut rng);
    b.link(*branch.last().unwrap(), main[to], link);

    // The keys hang off the loop, so the loop must be walked.
    for _ in 0..keys {
        let host = branch[rng.below(branch.len() as u32) as usize];
        let room = b.add(Role::Puzzle, Some(host), Link::Door);
        b.rooms[room].key = Some(0);
    }
    // The shortcut: from beside the gate back to the first passage, one way.
    let shortcut = b.add(Role::Shortcut, Some(main[gate]), Link::Door);
    b.link(shortcut, main[1], Link::OneWay);
    b.add(Role::Rest, Some(main[gate.div_ceil(2)]), Link::Door);
    let host = b.host(&mut rng);
    b.add(Role::Secret, Some(host), Link::Secret);
    for _ in 0..1 + spare {
        let host = b.host(&mut rng);
        let link = open(&mut rng);
        b.add(Role::Treasure, Some(host), link);
    }

    let mut graph = Graph {
        rooms: b.rooms,
        edges: b.edges,
        main,
    };
    graph.measure_danger();
    graph.populate(theme, &mut rng);
    graph
}

fn open(rng: &mut Rng) -> Link {
    if rng.unit() < 0.5 {
        Link::Corridor
    } else {
        Link::Door
    }
}

#[derive(Default)]
struct Builder {
    rooms: Vec<Room>,
    edges: Vec<Edge>,
}

impl Builder {
    fn add(&mut self, role: Role, parent: Option<usize>, link: Link) -> usize {
        let room = self.rooms.len();
        self.rooms.push(Room {
            role,
            parent,
            depth: 0,
            danger: 0.0,
            key: None,
            mobs: Vec::new(),
            loot: Vec::new(),
        });
        if let Some(parent) = parent {
            self.link(parent, room, link);
        }
        room
    }

    fn link(&mut self, a: usize, b: usize, link: Link) {
        self.edges.push(Edge { a, b, link });
    }

    /// A room to hang a dead end from: one on a path or a treasure, with the fewest links, so
    /// no room is crowded with doors.
    fn host(&self, rng: &mut Rng) -> usize {
        let hosts = [Role::Passage, Role::Branch, Role::Guard, Role::Treasure];
        let links = |room: usize| {
            self.edges
                .iter()
                .filter(|e| e.a == room || e.b == room)
                .count()
        };
        let candidates: Vec<usize> = (0..self.rooms.len())
            .filter(|&r| hosts.contains(&self.rooms[r].role))
            .collect();
        let fewest = candidates.iter().map(|&r| links(r)).min().unwrap_or(0);
        let least: Vec<usize> = candidates
            .into_iter()
            .filter(|&r| links(r) <= fewest + 1)
            .collect();
        least[rng.below(least.len() as u32) as usize]
    }
}

impl Graph {
    pub fn entrance(&self) -> usize {
        self.main[0]
    }

    pub fn goal(&self) -> usize {
        *self.main.last().unwrap()
    }

    /// The edges touching a room.
    pub fn edges_of(&self, room: usize) -> impl Iterator<Item = (usize, &Edge)> {
        self.edges
            .iter()
            .enumerate()
            .filter(move |(_, e)| e.a == room || e.b == room)
    }

    /// Depth by breadth-first search over every way that can be walked (locks open), danger
    /// from depth: 0 at the entrance and the rest room, 1 at the goal, the guard a step up but
    /// below the goal.
    fn measure_danger(&mut self) {
        let mut depth = vec![u32::MAX; self.rooms.len()];
        depth[self.entrance()] = 0;
        let mut queue = std::collections::VecDeque::from([self.entrance()]);
        while let Some(room) = queue.pop_front() {
            for edge in &self.edges {
                for (from, to) in edge.ways() {
                    if from == room && depth[to] == u32::MAX {
                        depth[to] = depth[room] + 1;
                        queue.push_back(to);
                    }
                }
            }
        }
        let deepest = depth
            .iter()
            .copied()
            .filter(|&d| d != u32::MAX)
            .max()
            .unwrap_or(1)
            .max(1);
        for (room, depth) in self.rooms.iter_mut().zip(depth) {
            room.depth = depth;
            room.danger = match room.role {
                Role::Entrance | Role::Rest => 0.0,
                Role::Goal => 1.0,
                Role::Guard => (depth as f32 / deepest as f32 + 0.2).min(0.95),
                _ => depth as f32 / deepest as f32,
            };
        }
    }

    /// Mobs from the theme's bestiary and loot from its tables, by role and danger.
    fn populate(&mut self, theme: &Theme, rng: &mut Rng) {
        for room in &mut self.rooms {
            let (role, danger) = (room.role, room.danger);
            let groups = match role {
                Role::Entrance | Role::Rest | Role::Secret => 0,
                Role::Puzzle | Role::Goal => 1,
                Role::Guard => 2,
                _ => u32::from(rng.unit() < 0.3 + 0.6 * danger),
            };
            for _ in 0..groups {
                let fits = |m: &&crate::theme::Mob| m.roles.contains(&role) && m.danger <= danger;
                let found: Vec<_> = theme.bestiary.iter().filter(fits).collect();
                if let Some(mob) = pick(rng, &found, |m| m.weight) {
                    let count = mob.group.0 + rng.below(mob.group.1 - mob.group.0 + 1);
                    room.mobs.push(Mobs {
                        species: mob.species.clone(),
                        count,
                    });
                }
            }
            let stacks = match role {
                Role::Treasure => 2 + rng.below(2),
                Role::Secret => 2,
                Role::Goal => 3,
                Role::Guard | Role::Rest => 1,
                Role::Entrance | Role::Puzzle => 0,
                _ => u32::from(rng.unit() < 0.3),
            };
            for _ in 0..stacks {
                let fits = |l: &&crate::theme::Loot| l.roles.contains(&role) && l.danger <= danger;
                let found: Vec<_> = theme.loot.iter().filter(fits).collect();
                if let Some(loot) = pick(rng, &found, |l| l.weight) {
                    let count = loot.count.0 + rng.below(loot.count.1 - loot.count.0 + 1);
                    room.loot.push(Loot {
                        item: loot.item.clone(),
                        count,
                    });
                }
            }
        }
    }

    /// The rooms reached from the entrance, picking up keys on the way and opening a lock once
    /// all its keys are held. `shut` keeps one lock shut whatever is held; `secrets` says
    /// whether hidden ways are found.
    pub fn reach(&self, shut: Option<u8>, secrets: bool) -> Vec<bool> {
        let needed = |lock: u8| self.rooms.iter().filter(|r| r.key == Some(lock)).count();
        let mut reached = vec![false; self.rooms.len()];
        reached[self.entrance()] = true;
        let mut held = vec![0usize; 256];
        loop {
            let mut changed = false;
            for edge in &self.edges {
                let passable = match edge.link {
                    Link::Locked(lock) => shut != Some(lock) && held[lock as usize] >= needed(lock),
                    Link::Secret => secrets,
                    _ => true,
                };
                for (from, to) in edge.ways() {
                    if passable && reached[from] && !reached[to] {
                        reached[to] = true;
                        changed = true;
                        if let Some(lock) = self.rooms[to].key {
                            held[lock as usize] += 1;
                        }
                    }
                }
            }
            if !changed {
                return reached;
            }
        }
    }

    /// The goal and every key are reached, and every lock's keys are reached with that lock
    /// shut: no key lies behind its own lock.
    pub fn solvable(&self) -> bool {
        let reached = self.reach(None, true);
        let keys = || {
            self.rooms
                .iter()
                .enumerate()
                .filter_map(|(i, r)| r.key.map(|k| (i, k)))
        };
        reached[self.goal()]
            && keys().all(|(room, _)| reached[room])
            && keys().all(|(room, lock)| self.reach(Some(lock), true)[room])
    }
}

/// A weighted pick.
pub fn pick<'a, T>(rng: &mut Rng, from: &[&'a T], weight: impl Fn(&T) -> f32) -> Option<&'a T> {
    let total: f32 = from.iter().map(|t| weight(t)).sum();
    if from.is_empty() || total <= 0.0 {
        return None;
    }
    let mut left = rng.unit() * total;
    for t in from {
        left -= weight(t);
        if left < 0.0 {
            return Some(*t);
        }
    }
    from.last().copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::assets;

    fn graphs() -> Vec<(Theme, Vec<Graph>)> {
        let themes = Theme::all(&assets()).unwrap();
        themes
            .into_iter()
            .map(|theme| {
                let seeds: Vec<u64> = (0..1000).collect();
                let graphs = terra_gen::par::map(&seeds, |&s| generate(&theme, Seed(s)));
                (theme, graphs)
            })
            .collect()
    }

    #[test]
    fn every_graph_is_solvable_and_shaped_by_its_theme() {
        for (theme, graphs) in graphs() {
            for (seed, g) in graphs.iter().enumerate() {
                let at = format!("{} seed {seed}", theme.name);
                assert!(g.solvable(), "{at}: not solvable");
                assert!(
                    g.reach(None, false)[g.goal()],
                    "{at}: the goal needs a secret"
                );
                let n = g.rooms.len() as u32;
                assert!(n >= theme.rooms.0 && n <= theme.rooms.1, "{at}: {n} rooms");
                let count = |role| g.rooms.iter().filter(|r| r.role == role).count();
                assert_eq!(count(Role::Entrance), 1, "{at}");
                assert_eq!(count(Role::Goal), 1, "{at}");
                assert_eq!(count(Role::Guard), 1, "{at}");
                assert_eq!(count(Role::Puzzle), theme.keys as usize, "{at}");
                assert!(count(Role::Treasure) >= 1, "{at}");
                assert!(g.edges.len() >= g.rooms.len(), "{at}: no loop");
                assert!((5..=7).contains(&g.main.len()), "{at}");
            }
        }
    }

    #[test]
    fn keys_come_before_their_lock() {
        for (theme, graphs) in graphs() {
            for (seed, g) in graphs.iter().enumerate() {
                for (room, r) in g.rooms.iter().enumerate() {
                    if let Some(lock) = r.key {
                        let shut = g.reach(Some(lock), true);
                        assert!(
                            shut[room],
                            "{} seed {seed}: key {room} behind its lock",
                            theme.name
                        );
                    }
                }
                // And the lock matters: with it shut, the goal is out of reach.
                assert!(
                    !g.reach(Some(0), true)[g.goal()],
                    "{} seed {seed}",
                    theme.name
                );
            }
        }
    }

    #[test]
    fn danger_rises_toward_the_goal() {
        for (theme, graphs) in graphs() {
            for (seed, g) in graphs.iter().enumerate() {
                let path: Vec<f32> = g.main.iter().map(|&r| g.rooms[r].danger).collect();
                assert!(
                    path.windows(2).all(|w| w[0] < w[1]),
                    "{} seed {seed}: {path:?}",
                    theme.name
                );
                assert_eq!(*path.last().unwrap(), 1.0);
            }
        }
    }

    #[test]
    fn rooms_hold_the_theme_s_mobs_and_loot() {
        let mut mobs = 0;
        let mut loot = 0;
        for (theme, graphs) in graphs() {
            for g in &graphs {
                for r in &g.rooms {
                    for m in &r.mobs {
                        assert!(
                            theme
                                .bestiary
                                .iter()
                                .any(|b| b.species == m.species && b.roles.contains(&r.role))
                        );
                        mobs += m.count;
                    }
                    for l in &r.loot {
                        assert!(
                            theme
                                .loot
                                .iter()
                                .any(|t| t.item == l.item && t.roles.contains(&r.role))
                        );
                        loot += l.count;
                    }
                    if matches!(r.role, Role::Entrance | Role::Rest) {
                        assert!(
                            r.mobs.is_empty(),
                            "{}: mobs at the {:?}",
                            theme.name,
                            r.role
                        );
                    }
                }
                assert!(!g.rooms[g.goal()].mobs.is_empty() && !g.rooms[g.goal()].loot.is_empty());
            }
        }
        assert!(mobs > 0 && loot > 0);
    }

    #[test]
    fn a_seed_makes_one_graph() {
        let theme = &Theme::all(&assets()).unwrap()[0];
        assert_eq!(generate(theme, Seed(7)), generate(theme, Seed(7)));
        assert_ne!(generate(theme, Seed(7)), generate(theme, Seed(8)));
    }
}
