//! Stage 3, dressing by wave function collapse (D474): each placed cell takes a module from its
//! theme's set (a module turned to each facing is a variant). A module's side takes sockets:
//! a wall, door or stair side must take what placement put there; an open side, into the same
//! room or corridor, must share an interior socket with the module across it. Open floor,
//! rubble and water are built in; a theme names its own (a pillar row's `aisle`, an alcove
//! run, the two beats of a wall's rhythm, a runner or a rail), and a socket only a few modules
//! take is a rule (D483): a pillar row runs on to a wall, a rail to a door or its buffer, a
//! pool meets dry floor only at a shore's steps. `any` is every socket but water and rubble,
//! for the features a run may end against. Cells placement fixes (the goal's dais, the key rooms' altars, the rest
//! room's fire, the stairs) take their module in any facing that fits.
//!
//! The solver is the simple tiled model (Gumin's WaveFunctionCollapse, as Townscaper and Bad
//! North use it): collapse the cell with the fewest variants left, by weight; propagate
//! arc-consistency to its neighbours; on a contradiction or a dressing that blocks a cell off,
//! start again from a derived seed, up to `RETRIES` times, then fall back to the theme's plain
//! floors, which always solve.

use crate::graph::Graph;
use crate::place::{Layout, STEPS, Side, Space, opposite};
use crate::theme::{self, Role, Theme};
use terra_gen::{Rng, Seed};

pub const OPEN: u32 = 1;
pub const WALL: u32 = 2;
pub const DOOR: u32 = 4;
pub const STAIR: u32 = 8;
pub const RUBBLE: u32 = 16;
pub const WATER: u32 = 32;
/// Sockets in all, the built-in six and a theme's own.
pub const SOCKETS: usize = 32;
/// Solves tried before the plain floors.
pub const RETRIES: u32 = 8;

/// The built-in sockets, by bit.
pub const BUILT_IN: [&str; 6] = ["open", "wall", "door", "stair", "rubble", "water"];

/// Every socket a theme's modules name: the built-in six, then its own in the order first met.
fn sockets_of(theme: &Theme) -> Result<Vec<String>, String> {
    let mut names: Vec<String> = BUILT_IN.map(String::from).to_vec();
    for m in &theme.modules {
        for name in m.sides.iter().flat_map(|s| s.split('|')).map(str::trim) {
            if name != "any" && !names.iter().any(|n| n == name) {
                names.push(name.to_string());
            }
        }
    }
    if names.len() > SOCKETS {
        return Err(format!(
            "{}: {} sockets, at most {SOCKETS}",
            theme.name,
            names.len()
        ));
    }
    Ok(names)
}

/// A module turned to one facing.
#[derive(Clone, Debug)]
pub struct Variant {
    pub module: usize,
    /// Quarter turns clockwise: its north side faces east after one.
    pub turn: u8,
    /// North, east, south, west, after turning.
    pub sides: [u32; 4],
    pub weight: f32,
}

/// A theme's variants, with the masks the solver works with: up to 128 variants, a bit each.
pub struct ModuleSet {
    pub variants: Vec<Variant>,
    /// By side and socket: the variants whose side takes it.
    fits: [[u128; SOCKETS]; 4],
    /// The sockets an open side shares: open floor, rubble, water and the theme's own.
    pub interior: u32,
    /// The theme's sockets, by bit.
    pub sockets: Vec<String>,
    /// The free variants a room cell, and a corridor cell, may take.
    room: u128,
    corridor: u128,
    /// The plain floors.
    safe: u128,
    /// By module: its variants.
    of: Vec<u128>,
    blocks: Vec<bool>,
}

impl ModuleSet {
    pub fn new(theme: &Theme) -> Result<ModuleSet, String> {
        let sockets = sockets_of(theme)?;
        let all = ((1u64 << sockets.len()) - 1) as u32;
        let any = all & !(WATER | RUBBLE);
        let mut variants: Vec<Variant> = Vec::new();
        for (module, m) in theme.modules.iter().enumerate() {
            if m.sides.len() != 4 {
                return Err(format!("{}: {} sides, not 4", m.name, m.sides.len()));
            }
            let mut sides = [0u32; 4];
            for (d, text) in m.sides.iter().enumerate() {
                for name in text.split('|').map(str::trim) {
                    sides[d] |= match sockets.iter().position(|n| n == name) {
                        Some(bit) => 1 << bit,
                        None => any,
                    };
                }
            }
            let mut own: Vec<Variant> = Vec::new();
            for turn in 0..if m.rotates { 4 } else { 1 } {
                let turned: [u32; 4] = std::array::from_fn(|d| sides[(d + 4 - turn) % 4]);
                if !own.iter().any(|v| v.sides == turned) {
                    own.push(Variant {
                        module,
                        turn: turn as u8,
                        sides: turned,
                        weight: 0.0,
                    });
                }
            }
            let share = m.weight / own.len() as f32;
            variants.extend(own.into_iter().map(|v| Variant { weight: share, ..v }));
        }
        if variants.len() > 128 {
            return Err(format!(
                "{}: {} variants, at most 128",
                theme.name,
                variants.len()
            ));
        }
        let mut set = ModuleSet {
            fits: [[0; SOCKETS]; 4],
            interior: all & !(WALL | DOOR | STAIR),
            sockets,
            room: 0,
            corridor: 0,
            safe: 0,
            of: vec![0; theme.modules.len()],
            blocks: theme.modules.iter().map(|m| m.blocks).collect(),
            variants,
        };
        for (v, variant) in set.variants.iter().enumerate() {
            let bit = 1u128 << v;
            let m = &theme.modules[variant.module];
            for d in 0..4 {
                for (socket, fits) in set.fits[d].iter_mut().enumerate() {
                    if variant.sides[d] & (1 << socket) != 0 {
                        *fits |= bit;
                    }
                }
            }
            set.of[variant.module] |= bit;
            if !m.fixed && m.spaces.contains(&theme::Space::Room) {
                set.room |= bit;
            }
            if !m.fixed && m.spaces.contains(&theme::Space::Corridor) {
                set.corridor |= bit;
            }
            if m.safe {
                set.safe |= bit;
            }
        }
        Ok(set)
    }

    pub fn blocks(&self, variant: usize) -> bool {
        self.blocks[self.variants[variant].module]
    }

    /// Under water: a side takes water and none a dry floor (a shore, with steps, is dry).
    pub fn wet(&self, variant: usize) -> bool {
        let sides = &self.variants[variant].sides;
        let dry = self.interior & !WATER;
        sides.iter().any(|s| s & WATER != 0) && sides.iter().all(|s| s & dry == 0)
    }

    /// The variants whose side `d` takes one of the sockets of `mask`.
    pub fn fit(&self, d: usize, mask: u32) -> u128 {
        bits(mask as u128).fold(0, |u, socket| u | self.fits[d][socket])
    }

    /// The interior sockets some variant of `domain` offers on side `d`.
    fn offered(&self, domain: u128, d: usize) -> u32 {
        bits(domain).fold(0, |u, v| u | (self.variants[v].sides[d] & self.interior))
    }
}

/// Every cell's variant, and how it was found.
#[derive(Clone, Debug)]
pub struct Dressing {
    /// By cell: an index into `ModuleSet::variants`.
    pub picks: Vec<usize>,
    /// Solves tried, the last the one kept (or failed before the fallback).
    pub tries: u32,
    /// Fell back to the plain floors.
    pub fallback: bool,
    /// Cells placement fixed, with the module each holds.
    pub fixed: Vec<(usize, usize)>,
}

/// Dresses a placed dungeon.
pub fn dress(
    layout: &Layout,
    graph: &Graph,
    theme: &Theme,
    set: &ModuleSet,
    seed: Seed,
) -> Dressing {
    let mut rng = seed.derive("features").rng();
    let fixed = features(layout, graph, theme, &mut rng);
    let neighbours: Vec<[Option<usize>; 4]> = layout
        .cells
        .iter()
        .map(|c| {
            std::array::from_fn(|d| {
                (c.sides[d] == Side::Open)
                    .then(|| layout.cell(c.at + STEPS[d]))
                    .flatten()
            })
        })
        .collect();
    let unary = |free: u128, room: u128, corridor: u128| -> Vec<u128> {
        layout
            .cells
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let mut domain = match c.space {
                    Space::Room(_) => room & free,
                    Space::Corridor(_) => corridor & free,
                    Space::Stair(_) => set.of[theme.module(&theme.features.stair).unwrap_or(0)],
                };
                if let Some(&(_, module)) = fixed.iter().find(|(cell, _)| *cell == i) {
                    domain = set.of[module];
                }
                for d in 0..4 {
                    let need = match c.sides[d] {
                        Side::Wall => WALL,
                        Side::Door(_) => DOOR,
                        Side::Way => STAIR,
                        Side::Open => set.interior,
                    };
                    domain &= set.fit(d, need);
                }
                domain
            })
            .collect()
    };
    let walkable = |picks: &[usize]| {
        layout
            .reached(|c| !set.blocks(picks[c]))
            .iter()
            .zip(picks)
            .all(|(&r, &v)| r || set.blocks(v))
    };
    let base = unary(u128::MAX, set.room, set.corridor);
    for tries in 1..=RETRIES {
        let mut rng = seed.derive("dress").index(tries as u64).rng();
        if let Some(picks) = solve(&base, &neighbours, set, &mut rng)
            && walkable(&picks)
        {
            return Dressing {
                picks,
                tries,
                fallback: false,
                fixed,
            };
        }
    }
    let plain = unary(set.safe, set.room, set.corridor);
    let picks = solve(&plain, &neighbours, set, &mut rng)
        .unwrap_or_else(|| panic!("{}: the plain floors do not fit every cell", theme.name));
    Dressing {
        picks,
        tries: RETRIES,
        fallback: true,
        fixed,
    }
}

/// The cells placement fixes, and their modules: in the goal room, each key room and the rest
/// room, the cell with the most open sides and no door, ties by chance.
fn features(layout: &Layout, graph: &Graph, theme: &Theme, rng: &mut Rng) -> Vec<(usize, usize)> {
    let f = &theme.features;
    let mut fixed = Vec::new();
    for (room, r) in graph.rooms.iter().enumerate() {
        let name = match r.role {
            Role::Goal => &f.goal,
            Role::Puzzle => &f.key,
            Role::Rest => &f.rest,
            _ => continue,
        };
        let Some(module) = theme.module(name) else {
            continue;
        };
        let best = layout.rooms[room]
            .cells()
            .filter_map(|at| layout.cell(at))
            .map(|cell| {
                let sides = &layout.cells[cell].sides;
                let open = sides.iter().filter(|s| **s == Side::Open).count() as f32;
                let doors = sides
                    .iter()
                    .filter(|s| matches!(s, Side::Door(_) | Side::Way))
                    .count() as f32;
                (2.0 * open - 3.0 * doors + rng.unit(), cell)
            })
            .max_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, cell)) = best {
            fixed.push((cell, module));
        }
    }
    fixed
}

fn bits(mut mask: u128) -> impl Iterator<Item = usize> {
    std::iter::from_fn(move || {
        (mask != 0).then(|| {
            let bit = mask.trailing_zeros() as usize;
            mask &= mask - 1;
            bit
        })
    })
}

/// One solve: every cell's variant, or none on a contradiction.
fn solve(
    base: &[u128],
    neighbours: &[[Option<usize>; 4]],
    set: &ModuleSet,
    rng: &mut Rng,
) -> Option<Vec<usize>> {
    let mut domain = base.to_vec();
    if domain.contains(&0) || !propagate(&mut domain, neighbours, set, (0..base.len()).collect()) {
        return None;
    }
    loop {
        let (mut pick, mut fewest, mut ties) = (None, u32::MAX, 0);
        for (cell, d) in domain.iter().enumerate() {
            let left = d.count_ones();
            if left > 1 && left <= fewest {
                ties = if left < fewest { 1 } else { ties + 1 };
                fewest = left;
                if ties == 1 || rng.below(ties) == 0 {
                    pick = Some(cell);
                }
            }
        }
        let Some(cell) = pick else { break };
        let total: f32 = bits(domain[cell]).map(|v| set.variants[v].weight).sum();
        let mut left = rng.unit() * total;
        let mut chosen = bits(domain[cell]).last().unwrap_or(0);
        for v in bits(domain[cell]) {
            left -= set.variants[v].weight;
            if left < 0.0 {
                chosen = v;
                break;
            }
        }
        domain[cell] = 1 << chosen;
        if !propagate(&mut domain, neighbours, set, vec![cell]) {
            return None;
        }
    }
    Some(domain.iter().map(|d| d.trailing_zeros() as usize).collect())
}

/// Arc consistency over open sides: a variant stays only while the cell across has one that
/// shares an interior socket with it.
fn propagate(
    domain: &mut [u128],
    neighbours: &[[Option<usize>; 4]],
    set: &ModuleSet,
    mut stack: Vec<usize>,
) -> bool {
    while let Some(cell) = stack.pop() {
        for (d, next) in neighbours[cell].iter().enumerate() {
            let Some(next) = *next else {
                continue;
            };
            let kept = domain[next] & set.fit(opposite(d), set.offered(domain[cell], d));
            if kept != domain[next] {
                if kept == 0 {
                    return false;
                }
                domain[next] = kept;
                stack.push(next);
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::generate;
    use crate::place::place;
    use crate::theme::assets;

    #[test]
    fn dressing_fills_every_cell_keeps_every_way_and_floods_whole_spaces() {
        for theme in Theme::all(&assets()).unwrap() {
            let set = ModuleSet::new(&theme).unwrap();
            let seeds: Vec<u64> = (0..1000).collect();
            let results = terra_gen::par::map(&seeds, |&s| {
                let graph = generate(&theme, Seed(s));
                let layout = place(&graph, &theme, Seed(s)).ok()?;
                let dressing = dress(&layout, &graph, &theme, &set, Seed(s));
                let at = format!("{} seed {s}", theme.name);
                assert_eq!(dressing.picks.len(), layout.cells.len(), "{at}");
                for (cell, &v) in layout.cells.iter().zip(&dressing.picks) {
                    let variant = &set.variants[v];
                    for (d, side) in cell.sides.iter().enumerate() {
                        let need = match side {
                            Side::Wall => WALL,
                            Side::Door(_) => DOOR,
                            Side::Way => STAIR,
                            Side::Open => set.interior,
                        };
                        assert!(
                            variant.sides[d] & need != 0,
                            "{at}: {} at {}",
                            theme.modules[variant.module].name,
                            cell.at
                        );
                        if cell.sides[d] == Side::Open {
                            let next = layout.cell(cell.at + STEPS[d]).unwrap();
                            let across = &set.variants[dressing.picks[next]];
                            assert!(
                                variant.sides[d] & across.sides[opposite(d)] & set.interior != 0,
                                "{at}: {} and {} share no socket at {}",
                                theme.modules[variant.module].name,
                                theme.modules[across.module].name,
                                cell.at
                            );
                            assert!(
                                !set.wet(v) || across.sides[opposite(d)] & WATER != 0,
                                "{at}: water ends without a shore at {}",
                                cell.at
                            );
                        }
                    }
                }
                let reached = layout.reached(|c| !set.blocks(dressing.picks[c]));
                for (c, &v) in dressing.picks.iter().enumerate() {
                    assert!(
                        reached[c] || set.blocks(v),
                        "{at}: {} cut off",
                        layout.cells[c].at
                    );
                }
                for &(cell, module) in &dressing.fixed {
                    assert_eq!(
                        set.variants[dressing.picks[cell]].module, module,
                        "{at}: a feature moved"
                    );
                    assert!(reached[cell], "{at}: a feature out of reach");
                }
                Some((dressing.tries, dressing.fallback))
            });
            let dressed: Vec<(u32, bool)> = results.into_iter().flatten().collect();
            let solved = dressed.iter().filter(|(_, f)| !f).count() as f32 / dressed.len() as f32;
            let tries = dressed.iter().map(|(t, _)| *t).sum::<u32>() as f32 / dressed.len() as f32;
            println!(
                "{}: {:.1}% solved without the fallback, {tries:.2} tries each",
                theme.name,
                solved * 100.0
            );
            assert!(
                solved >= 0.97,
                "{}: only {:.1}% solved",
                theme.name,
                solved * 100.0
            );
        }
    }

    #[test]
    fn modules_turn() {
        let theme = Theme::all(&assets()).unwrap().remove(0);
        let set = ModuleSet::new(&theme).unwrap();
        let walled: Vec<&Variant> = set
            .variants
            .iter()
            .filter(|v| v.sides.iter().all(|s| *s != 0))
            .collect();
        assert!(set.variants.len() > theme.modules.len(), "turned variants");
        for v in walled.iter().filter(|v| v.turn == 1) {
            let first = set
                .variants
                .iter()
                .find(|w| w.module == v.module && w.turn == 0)
                .unwrap();
            assert_eq!(v.sides[1], first.sides[0], "north turns to east");
        }
    }
}
