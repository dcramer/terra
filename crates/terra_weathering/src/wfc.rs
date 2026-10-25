//! Wave function collapse, the overlapping model (Maxim Gumin's, MIT, as his paper and
//! `OverlappingModel.cs` describe it): every `n` by `n` window of a few example patches is a
//! pattern, counted; an output grid's cells each take a pattern whose windows agree with
//! their neighbours', the least certain cell chosen first by its entropy, a contradiction
//! started again with another seed. Here each cell's symbols are also weighed, or banned at
//! a weight of 0, by the caller (the masks), so a motif goes only where it may.

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use terra_gen::hash;

/// The most patterns a model may have.
pub const MAX_PATTERNS: usize = 256;
/// The most symbols.
pub const MAX_SYMBOLS: usize = 4;

/// A set of patterns, a bit each.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Bits([u64; MAX_PATTERNS / 64]);

impl Bits {
    fn has(&self, i: usize) -> bool {
        self.0[i / 64] >> (i % 64) & 1 == 1
    }
    fn set(&mut self, i: usize) {
        self.0[i / 64] |= 1 << (i % 64);
    }
    fn and(self, o: Bits) -> Bits {
        Bits(std::array::from_fn(|w| self.0[w] & o.0[w]))
    }
    fn or(&mut self, o: &Bits) {
        for (a, b) in self.0.iter_mut().zip(o.0) {
            *a |= b;
        }
    }
    fn count(&self) -> u32 {
        self.0.iter().map(|w| w.count_ones()).sum()
    }
    fn ones(&self) -> impl Iterator<Item = usize> + '_ {
        (0..MAX_PATTERNS).filter(|&i| self.has(i))
    }
}

/// The four neighbours: east, south (a row down), west, north.
const DIRS: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

/// Patterns learned from examples, and which may stand beside which.
#[derive(Clone, Debug)]
pub struct Model {
    pub n: usize,
    /// The symbols, in the order their indices are.
    pub symbols: Vec<char>,
    /// Each pattern's symbols, row by row, as indices.
    pub patterns: Vec<Vec<u8>>,
    /// How often each was seen.
    pub counts: Vec<f32>,
    /// For each direction and pattern, the patterns that may stand next to it that way.
    compatible: [Vec<Bits>; 4],
    /// Every pattern, and for each direction every pattern that may stand next to any.
    all: Bits,
    reach: [Bits; 4],
}

impl Model {
    /// Learns `n` by `n` patterns from example patches, each rows of symbols top to bottom,
    /// and each mirrored left to right too where `mirror`.
    pub fn learn(examples: &[&str], n: usize, mirror: bool) -> Model {
        let mut symbols: Vec<char> = Vec::new();
        let mut patterns: Vec<Vec<u8>> = Vec::new();
        let mut counts: Vec<f32> = Vec::new();
        for example in examples {
            let rows: Vec<Vec<char>> = example
                .lines()
                .map(str::trim)
                .filter(|r| !r.is_empty())
                .map(|r| r.chars().collect())
                .collect();
            let mut grids = vec![rows.clone()];
            if mirror {
                grids.push(
                    rows.iter()
                        .map(|r| r.iter().rev().copied().collect())
                        .collect(),
                );
            }
            for grid in grids {
                let (w, h) = (grid[0].len(), grid.len());
                for y in 0..=h.saturating_sub(n) {
                    for x in 0..=w.saturating_sub(n) {
                        let mut p = Vec::with_capacity(n * n);
                        for dy in 0..n {
                            for dx in 0..n {
                                let c = grid[y + dy][x + dx];
                                let i = symbols.iter().position(|&s| s == c).unwrap_or_else(|| {
                                    symbols.push(c);
                                    symbols.len() - 1
                                });
                                p.push(i as u8);
                            }
                        }
                        match patterns.iter().position(|q| *q == p) {
                            Some(i) => counts[i] += 1.0,
                            None => {
                                patterns.push(p);
                                counts.push(1.0);
                            }
                        }
                    }
                }
            }
        }
        assert!(
            patterns.len() <= MAX_PATTERNS,
            "{} patterns, at most {MAX_PATTERNS}",
            patterns.len()
        );
        assert!(
            symbols.len() <= MAX_SYMBOLS,
            "{} symbols, at most {MAX_SYMBOLS}",
            symbols.len()
        );
        let agrees = |a: &[u8], b: &[u8], dx: i32, dy: i32| {
            let n = n as i32;
            (0..n).all(|y| {
                (0..n).all(|x| {
                    let (bx, by) = (x - dx, y - dy);
                    !(0..n).contains(&bx)
                        || !(0..n).contains(&by)
                        || a[(y * n + x) as usize] == b[(by * n + bx) as usize]
                })
            })
        };
        let compatible: [Vec<Bits>; 4] = std::array::from_fn(|d| {
            let (dx, dy) = DIRS[d];
            patterns
                .iter()
                .map(|a| {
                    let mut bits = Bits::default();
                    for (j, b) in patterns.iter().enumerate() {
                        if agrees(a, b, dx, dy) {
                            bits.set(j);
                        }
                    }
                    bits
                })
                .collect()
        });
        let mut all = Bits::default();
        (0..patterns.len()).for_each(|i| all.set(i));
        let reach = std::array::from_fn(|d| {
            let mut bits = Bits::default();
            compatible[d].iter().for_each(|c: &Bits| bits.or(c));
            bits
        });
        Model {
            n,
            symbols,
            patterns,
            counts,
            compatible,
            all,
            reach,
        }
    }

    /// The index of a symbol.
    pub fn symbol(&self, c: char) -> Option<usize> {
        self.symbols.iter().position(|&s| s == c)
    }

    /// Solves a grid `w` by `h`: `present` says which cells are in it (the rest neither take
    /// a pattern nor hold their neighbours to anything), `weigh` each symbol's weight at a
    /// cell (0 bans it there), which weighs the patterns whose first symbol it is. Gives each
    /// cell's symbol (0 where absent) and the tries it took, or `None` past `tries`.
    pub fn solve(
        &self,
        (w, h): (usize, usize),
        present: &dyn Fn(usize, usize) -> bool,
        weigh: &dyn Fn(usize, usize) -> [f32; MAX_SYMBOLS],
        seed: u32,
        tries: u32,
    ) -> (Option<Vec<u8>>, u32) {
        let here: Vec<bool> = (0..w * h).map(|i| present(i % w, i / w)).collect();
        let weights: Vec<[f32; MAX_SYMBOLS]> = (0..w * h)
            .map(|i| {
                if here[i] {
                    weigh(i % w, i / w)
                } else {
                    [0.0; MAX_SYMBOLS]
                }
            })
            .collect();
        for attempt in 0..tries.max(1) {
            let mut run = Run::new(self, w, h, &here, &weights, hash::of(&[seed, attempt]));
            if run.go() {
                let out = (0..w * h)
                    .map(|i| match run.domain[i].ones().next() {
                        Some(p) if here[i] => self.patterns[p][0],
                        _ => 0,
                    })
                    .collect();
                return (Some(out), attempt + 1);
            }
        }
        (None, tries.max(1))
    }
}

/// A cell waiting in the queue, the least entropy first.
#[derive(PartialEq)]
struct Waiting(f32, usize);

impl Eq for Waiting {}
impl PartialOrd for Waiting {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Waiting {
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.total_cmp(&self.0).then(o.1.cmp(&self.1))
    }
}

/// One attempt at a solve.
struct Run<'a> {
    model: &'a Model,
    w: usize,
    h: usize,
    here: &'a [bool],
    weights: &'a [[f32; MAX_SYMBOLS]],
    domain: Vec<Bits>,
    seed: u32,
    draws: u32,
}

impl<'a> Run<'a> {
    fn new(
        model: &'a Model,
        w: usize,
        h: usize,
        here: &'a [bool],
        weights: &'a [[f32; MAX_SYMBOLS]],
        seed: u32,
    ) -> Self {
        let domain = (0..w * h)
            .map(|i| {
                let mut bits = Bits::default();
                if here[i] {
                    for (p, pattern) in model.patterns.iter().enumerate() {
                        if weights[i][pattern[0] as usize] > 0.0 {
                            bits.set(p);
                        }
                    }
                }
                bits
            })
            .collect();
        Run {
            model,
            w,
            h,
            here,
            weights,
            domain,
            seed,
            draws: 0,
        }
    }

    fn random(&mut self) -> f32 {
        self.draws += 1;
        hash::unit(hash::of(&[self.seed, self.draws]))
    }

    fn weight(&self, cell: usize, p: usize) -> f32 {
        self.model.counts[p] * self.weights[cell][self.model.patterns[p][0] as usize]
    }

    /// Its entropy, a little noise added so ties break at random, or `None` once decided.
    fn entropy(&mut self, cell: usize) -> Option<f32> {
        if self.domain[cell].count() <= 1 {
            return None;
        }
        let (mut sum, mut sum_log) = (0.0, 0.0);
        for p in self.domain[cell].ones() {
            let wt = self.weight(cell, p);
            sum += wt;
            sum_log += wt * libm::logf(wt);
        }
        Some(libm::logf(sum) - sum_log / sum + 1e-4 * self.random())
    }

    /// Runs to the end: false on a contradiction.
    fn go(&mut self) -> bool {
        let cells: Vec<usize> = (0..self.w * self.h).filter(|&i| self.here[i]).collect();
        if cells.iter().any(|&i| self.domain[i].count() == 0) || !self.propagate(cells.clone()) {
            return false;
        }
        let mut queue = BinaryHeap::new();
        for &i in &cells {
            if let Some(e) = self.entropy(i) {
                queue.push(Waiting(e, i));
            }
        }
        while let Some(Waiting(_, cell)) = queue.pop() {
            if self.domain[cell].count() <= 1 {
                continue;
            }
            // Choose one of its patterns by weight.
            let options: Vec<(usize, f32)> = self.domain[cell]
                .ones()
                .map(|p| (p, self.weight(cell, p)))
                .collect();
            let total: f32 = options.iter().map(|o| o.1).sum();
            let mut pick = self.random() * total;
            let mut chosen = options[options.len() - 1].0;
            for (p, wt) in &options {
                if pick < *wt {
                    chosen = *p;
                    break;
                }
                pick -= wt;
            }
            let mut one = Bits::default();
            one.set(chosen);
            self.domain[cell] = one;
            let mut changed = Vec::new();
            if !self.propagate_from(vec![cell], &mut changed) {
                return false;
            }
            for c in changed {
                if let Some(e) = self.entropy(c) {
                    queue.push(Waiting(e, c));
                }
            }
        }
        true
    }

    fn propagate(&mut self, start: Vec<usize>) -> bool {
        let mut changed = Vec::new();
        self.propagate_from(start, &mut changed)
    }

    /// Narrows the neighbours of changed cells until nothing changes: false if a cell is
    /// left with no pattern. The cells it narrowed are added to `changed`.
    fn propagate_from(&mut self, mut stack: Vec<usize>, changed: &mut Vec<usize>) -> bool {
        while let Some(cell) = stack.pop() {
            let (x, y) = ((cell % self.w) as i32, (cell / self.w) as i32);
            let own = self.domain[cell];
            let full = own == self.model.all;
            for (d, (dx, dy)) in DIRS.iter().enumerate() {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= self.w as i32 || ny >= self.h as i32 {
                    continue;
                }
                let next = ny as usize * self.w + nx as usize;
                if !self.here[next] {
                    continue;
                }
                let mut allowed = Bits::default();
                if full {
                    allowed = self.model.reach[d];
                } else {
                    for p in own.ones() {
                        allowed.or(&self.model.compatible[d][p]);
                    }
                }
                let narrowed = self.domain[next].and(allowed);
                if narrowed != self.domain[next] {
                    if narrowed.count() == 0 {
                        return false;
                    }
                    self.domain[next] = narrowed;
                    stack.push(next);
                    changed.push(next);
                }
            }
        }
        true
    }
}
