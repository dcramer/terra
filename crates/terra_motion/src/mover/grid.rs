//! The bodies' broadphase: a uniform grid of bodies by place, hashed (Ericson, Real-Time
//! Collision Detection, 7.1 "Uniform grids" and 7.1.6 "Hashed storage"), so a body's sweep
//! and contacts and the pushes' pairs look only at bodies in nearby cells, not at every other
//! (Jolt's `CharacterVsCharacterCollisionSimple` loops over every character, "a very
//! inefficient way", and leaves a crowd's broadphase to the game). Each body is one point in
//! one cell, moved when it moves; a box's bodies (their points in it) come back as indices sorted ascending, ID
//! order, so whatever the cells hold and however they were filled the answers are the same
//! (D138). Callers keep their own exact tests: the grid only leaves out bodies too far for
//! those tests to pass.
//!
//! A body's tick asks about the bodies around it many times over; it gathers them once for a
//! box around it ([`Grid::gather`], into a [`Gathered`] of its own, so bodies stepped on
//! several threads share one grid) and each ask that fits in that box takes the gathered list.

use glam::DVec3;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// A cell's side, m: a man's whole reach in a tick's queries spans a few cells.
pub const CELL_M: f64 = 2.0;

/// The most cells a box is looked up by before every body is taken instead.
const MOST_CELLS: i64 = 512;

type Key = (i64, i64, i64);

/// A cell's key hashed by SplitMix64's finaliser, a few multiplies (std's SipHash is made to
/// resist chosen keys, which a grid's are not).
#[derive(Default)]
struct Mix(u64);

impl Hasher for Mix {
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(u64::from(b));
        }
    }

    fn write_u64(&mut self, v: u64) {
        self.0 = terra_gen::hash::splitmix64(self.0 ^ v);
    }

    fn write_i64(&mut self, v: i64) {
        self.write_u64(v as u64);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

/// Bodies by place: each an index (its place in the caller's ID-ordered list) at a point.
#[derive(Default)]
pub struct Grid {
    cells: HashMap<Key, Vec<u32>, BuildHasherDefault<Mix>>,
    keys: Vec<Key>,
    points: Vec<DVec3>,
    /// The farthest any body's surface lies from its point, m.
    pub extent: f32,
    /// A cell's side, m.
    cell: f64,
}

/// The bodies of a box gathered for many asks, and the box: one a thread.
#[derive(Default)]
pub struct Gathered {
    list: Vec<u32>,
    around: Option<(DVec3, DVec3)>,
}

impl Grid {
    /// The grid of points, each its index's body, none of whose surfaces lies farther than
    /// `extent` from its point.
    pub fn of(points: impl Iterator<Item = DVec3>, extent: f32) -> Self {
        Grid::of_cells(points, extent, CELL_M)
    }

    /// [`Grid::of`] in cells of a side, m: about the reach of the boxes it will be asked of.
    pub fn of_cells(points: impl Iterator<Item = DVec3>, extent: f32, cell: f64) -> Self {
        let mut grid = Grid {
            extent,
            cell,
            ..Grid::default()
        };
        for (i, at) in points.enumerate() {
            let k = grid.key(at);
            grid.keys.push(k);
            grid.points.push(at);
            grid.cells.entry(k).or_default().push(i as u32);
        }
        grid
    }

    fn key(&self, at: DVec3) -> Key {
        let c = (at / self.cell).floor();
        (c.x as i64, c.y as i64, c.z as i64)
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// A body's point moved. A [`Gathered`] list gathered before holds the bodies whose
    /// points were in its box then: gather again.
    pub fn moved(&mut self, index: usize, to: DVec3) {
        self.points[index] = to;
        let k = self.key(to);
        let was = self.keys[index];
        if k == was {
            return;
        }
        if let Some(cell) = self.cells.get_mut(&was) {
            cell.retain(|&i| i as usize != index);
            if cell.is_empty() {
                self.cells.remove(&was);
            }
        }
        self.cells.entry(k).or_default().push(index as u32);
        self.keys[index] = k;
    }

    /// Whether a body's point lies in a box.
    pub fn inside(&self, index: u32, lo: DVec3, hi: DVec3) -> bool {
        let at = self.points[index as usize];
        at.cmpge(lo).all() && at.cmple(hi).all()
    }

    /// Every body whose point lies in a box, ascending, into `out`.
    pub fn near(&self, lo: DVec3, hi: DVec3, out: &mut Vec<u32>) {
        out.clear();
        let (a, b) = (self.key(lo), self.key(hi));
        let span = |a: i64, b: i64| (b - a + 1).max(0);
        let cells = span(a.0, b.0)
            .saturating_mul(span(a.1, b.1))
            .saturating_mul(span(a.2, b.2));
        if cells > MOST_CELLS.min(self.cells.len() as i64) || !lo.is_finite() || !hi.is_finite() {
            out.extend((0..self.keys.len() as u32).filter(|&i| self.inside(i, lo, hi)));
            return;
        }
        for x in a.0..=b.0 {
            for y in a.1..=b.1 {
                for z in a.2..=b.2 {
                    if let Some(cell) = self.cells.get(&(x, y, z)) {
                        out.extend(cell.iter().filter(|&&i| self.inside(i, lo, hi)));
                    }
                }
            }
        }
        out.sort_unstable();
    }

    /// Gathers the bodies of a box for the asks to come ([`Gathered::within`]).
    pub fn gather(&self, lo: DVec3, hi: DVec3, into: &mut Gathered) {
        self.near(lo, hi, &mut into.list);
        into.around = Some((lo, hi));
    }
}

impl Gathered {
    /// The gathered bodies, if a box lies within the gathered one: those in it and maybe
    /// more ([`Grid::inside`] tells), ascending.
    pub fn within(&self, lo: DVec3, hi: DVec3) -> Option<&[u32]> {
        let (a, b) = self.around?;
        (lo.cmpge(a).all() && hi.cmple(b).all()).then_some(&self.list[..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_gen::hash::{of, unit};

    fn by_scan(points: &[DVec3], lo: DVec3, hi: DVec3) -> Vec<u32> {
        (0..points.len() as u32)
            .filter(|&i| {
                let p = points[i as usize];
                p.cmpge(lo).all() && p.cmple(hi).all()
            })
            .collect()
    }

    /// A point in a box of a side, from an index.
    fn scatter(i: u32, side: f64) -> DVec3 {
        let h = |s: u32| f64::from(unit(of(&[i, s])));
        DVec3::new(h(1), h(2), h(3)) * side - side * 0.5
    }

    #[test]
    fn a_box_holds_every_body_a_scan_finds_there_in_id_order() {
        let mut points: Vec<DVec3> = (0..400).map(|i| scatter(i, 30.0)).collect();
        let mut grid = Grid::of(points.iter().copied(), 0.0);
        let (mut out, mut gathered) = (Vec::new(), Gathered::default());
        for round in 0..3u32 {
            for q in 0..50u32 {
                let lo = scatter(q + 1000 * (round + 1), 30.0);
                let hi = lo + DVec3::splat(f64::from(q % 7) + 0.5);
                grid.near(lo, hi, &mut out);
                let found = by_scan(&points, lo, hi);
                assert!(out.windows(2).all(|w| w[0] < w[1]), "ascending");
                assert_eq!(out, found, "{q}");
                // A smaller box inside a gathered one is answered from it, and holds as much.
                grid.gather(lo - 1.0, hi + 1.0, &mut gathered);
                let inner = gathered.within(lo, hi).expect("inside");
                assert!(found.iter().all(|i| inner.contains(i)));
                assert!(gathered.within(lo - 2.0, hi).is_none());
            }
            // Move every third body somewhere else and look again.
            for i in (0..points.len()).step_by(3) {
                points[i] = scatter(i as u32 + 7000 * (round + 1), 30.0);
                grid.moved(i, points[i]);
            }
        }
    }

    #[test]
    fn a_box_too_big_to_look_up_gives_every_body() {
        let points: Vec<DVec3> = (0..10).map(|i| scatter(i, 1000.0)).collect();
        let grid = Grid::of(points.iter().copied(), 0.0);
        let mut out = Vec::new();
        grid.near(DVec3::splat(-1.0e6), DVec3::splat(1.0e6), &mut out);
        assert_eq!(out, (0..10).collect::<Vec<u32>>());
        grid.near(
            DVec3::splat(-1.0e6),
            DVec3::new(1.0e6, 1.0e6, 0.0),
            &mut out,
        );
        assert_eq!(
            out,
            by_scan(&points, DVec3::splat(-1.0e6), DVec3::new(1.0e6, 1.0e6, 0.0))
        );
    }
}
