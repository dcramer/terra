//! Each face's map drawn: the masks' layers (grime, wear, damp) the same either way, the
//! motif layers (crack, loss, moss, lichen, streak, soot) either cut from noise by the masks
//! or drawn from the symbols wave function collapse solved, then the maps packed into one
//! atlas with a lookup from every cube's face.

use crate::faces::{FACES, Plane};
use crate::masks::{Context, Masks, Wants, aged};
use crate::motifs::LAYERS as MOTIFS;
use crate::wfc::Model;
use crate::{CELL_M, LAYERS, Mode, Settings, Stats, Stuff, TILE, Weathered};
use glam::{IVec3, Vec2, Vec3};
use std::collections::HashMap;
use std::sync::OnceLock;
use terra_gen::Noise;
use terra_gen::smoothstep;
use terra_gen::{Seed, hash};

/// The motif layers' models, learned once.
fn models() -> &'static [Model; 4] {
    static MODELS: OnceLock<[Model; 4]> = OnceLock::new();
    MODELS.get_or_init(|| std::array::from_fn(|i| MOTIFS[i].model()))
}

/// A motif layer's symbols as solved over a plane, rows from the top down.
pub struct Solved {
    pub symbols: Vec<char>,
    pub w: usize,
    pub h: usize,
    pub per_cell: usize,
}

impl Solved {
    pub fn at(&self, x: i32, y: i32) -> char {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return '.';
        }
        self.symbols[y as usize * self.w + x as usize]
    }

    fn is(&self, x: i32, y: i32, c: char) -> bool {
        self.at(x, y) == c
    }
}

/// The fewest pixels a crack keeps: a 3 by 3 pattern rules out a lone pixel but not two or
/// three where two crack ends meet, which read as dirt, not a crack (6 cm), so they go.
pub const MIN_CRACK: usize = 4;

/// The 8-connected groups of `c` in a grid, each its cells.
pub fn groups(grid: &[char], w: usize, h: usize, c: char) -> Vec<Vec<usize>> {
    let mut seen = vec![false; w * h];
    let mut out = Vec::new();
    for start in 0..w * h {
        if grid[start] != c || seen[start] {
            continue;
        }
        let (mut stack, mut cells) = (vec![start], Vec::new());
        seen[start] = true;
        while let Some(i) = stack.pop() {
            cells.push(i);
            let (x, y) = ((i % w) as i32, (i / w) as i32);
            for (dx, dy) in (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))) {
                let (nx, ny) = (x + dx, y + dy);
                if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                    let j = ny as usize * w + nx as usize;
                    if grid[j] == c && !seen[j] {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
        }
        out.push(cells);
    }
    out
}

/// Clears the groups of `c` smaller than `min`.
pub fn prune(grid: &mut [char], w: usize, h: usize, c: char, min: usize) {
    for cells in groups(grid, w, h, c) {
        if cells.len() < min {
            cells.iter().for_each(|&i| grid[i] = '.');
        }
    }
}

/// A plane's faces' maps, in the order of its cubes, and how its solves went.
pub struct Drawn {
    pub maps: Vec<[Vec<u8>; LAYERS]>,
    pub stats: Stats,
}

/// Solves a plane's motif layer `index`.
pub fn solve(
    index: usize,
    plane: &Plane,
    contexts: &HashMap<[i32; 2], Context>,
    settings: &Settings,
    stats: &mut Stats,
) -> Solved {
    let layer = &MOTIFS[index];
    let model = &models()[index];
    let pc = layer.per_cell;
    let size = plane.size();
    let (w, h) = (size[0] as usize * pc, size[1] as usize * pc);
    let cube_of = |x: usize, y: usize| {
        let place = [
            plane.min[0] + (x / pc) as i32,
            plane.max[1] - (y / pc) as i32,
        ];
        let fu = ((x % pc) as f32 + 0.5) / pc as f32;
        let fv = 1.0 - ((y % pc) as f32 + 0.5) / pc as f32;
        contexts.get(&place).map(|c| (c, fu, fv))
    };
    let present = |x: usize, y: usize| cube_of(x, y).is_some();
    let weigh = |x: usize, y: usize| {
        let (c, fu, fv) = cube_of(x, y).expect("present");
        layer.weights(model, &Wants::of(&c.at(fu, fv), c.stuff, settings.age))
    };
    let depth = plane.cubes[0].0.dot(FACES[plane.face].normal);
    let seed = hash::of(&[settings.seed, plane.face as u32, depth as u32, index as u32]);
    let (out, tries) = model.solve((w, h), &present, &weigh, seed, settings.tries);
    stats.solves += 1;
    stats.tries += tries as usize;
    stats.cells += (0..w * h).filter(|&i| present(i % w, i / w)).count();
    if out.is_some() {
        stats.solved += 1;
        if tries == 1 {
            stats.first_try += 1;
        }
    }
    let mut symbols = match out {
        Some(out) => out.iter().map(|&s| model.symbols[s as usize]).collect(),
        None => vec!['.'; w * h],
    };
    if index == 0 {
        prune(&mut symbols, w, h, 'c', MIN_CRACK);
    }
    Solved {
        symbols,
        w,
        h,
        per_cell: pc,
    }
}

/// Draws every face of a plane.
pub fn plane(
    stuff: &dyn Fn(IVec3) -> Stuff,
    plane: &Plane,
    ground: i32,
    settings: &Settings,
) -> Drawn {
    let face = &FACES[plane.face];
    let contexts: HashMap<[i32; 2], Context> = plane
        .cubes
        .iter()
        .map(|(at, place)| (*place, Context::of(stuff, face, *at, ground)))
        .collect();
    let mut stats = Stats {
        faces: plane.cubes.len(),
        planes: 1,
        ..Stats::default()
    };
    let solved: Option<[Solved; 4]> = (settings.mode == Mode::Wfc)
        .then(|| std::array::from_fn(|i| solve(i, plane, &contexts, settings, &mut stats)));
    let depth = plane.cubes[0].0.dot(face.normal);
    let noise = Noise::new(Seed(u64::from(hash::of(&[
        settings.seed,
        plane.face as u32,
        depth as u32,
    ]))));
    let maps = plane
        .cubes
        .iter()
        .map(|(_, place)| {
            let context = &contexts[place];
            let mut maps: [Vec<u8>; LAYERS] = std::array::from_fn(|_| vec![0; TILE * TILE * 4]);
            for row in 0..TILE {
                for col in 0..TILE {
                    let fu = (col as f32 + 0.5) / TILE as f32;
                    let fv = 1.0 - (row as f32 + 0.5) / TILE as f32;
                    let m = context.at(fu, fv);
                    let wants = Wants::of(&m, context.stuff, settings.age);
                    let p = Vec2::new(
                        (place[0] as f32 + fu) * CELL_M,
                        (place[1] as f32 + fv) * CELL_M,
                    );
                    let n = |scale: Vec2, k: f32| {
                        0.5 + 0.5 * noise.sample(Vec3::new(p.x * scale.x, p.y * scale.y, k))
                    };
                    let motifs = match &solved {
                        None => by_masks(&wants, &n, p),
                        Some(solved) => {
                            let grid = Vec2::new(
                                (place[0] - plane.min[0]) as f32 + fu,
                                (plane.max[1] - place[1]) as f32 + 1.0 - fv,
                            );
                            by_motifs(solved, grid, &n, settings.seed)
                        }
                    };
                    let [grime, wear, damp] = masked(&m, settings.age, &n);
                    let at = (row * TILE + col) * 4;
                    let texels = [
                        motifs[0], motifs[1], motifs[2], motifs[3], grime, wear, motifs[4], damp,
                        motifs[5], m.cavity, m.edge, 1.0,
                    ];
                    for (k, v) in texels.iter().enumerate() {
                        maps[k / 4][at + k % 4] = (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                    }
                }
            }
            maps
        })
        .collect();
    Drawn { maps, stats }
}

/// The masks' own layers, the same in either way: grime in crevices, under eaves and
/// splashed up from the ground; wear on outside edges; damp at the foot.
fn masked(m: &Masks, age: f32, n: &dyn Fn(Vec2, f32) -> f32) -> [f32; 3] {
    let grime = aged(age, 15.0)
        * (0.15 + 0.6 * m.cavity + 0.3 * m.sheltered * (1.0 - m.up) + 0.35 * m.ground)
        * (0.7 + 0.6 * n(Vec2::splat(1.7), 3.0));
    let wear = aged(age, 20.0) * m.edge * (0.5 + n(Vec2::splat(7.0), 5.0));
    let damp = aged(age, 10.0) * m.ground * m.ground * (0.7 + 0.6 * n(Vec2::new(2.0, 3.0), 7.0));
    [grime, wear, damp]
}

/// The motif layers as classic procedural weathering draws them: each its mask cut by noise
/// at a threshold. Crack, loss, moss, streak, lichen, soot.
fn by_masks(w: &Wants, n: &dyn Fn(Vec2, f32) -> f32, p: Vec2) -> [f32; 6] {
    // Ridged noise for cracks, its ridges thin wandering lines.
    let ridge = 1.0 - (2.0 * n(Vec2::splat(2.6), 13.0) - 1.0).abs();
    let crack = smoothstep(0.95, 0.99, ridge) * smoothstep(0.05, 0.5, w.crack);
    let t = n(Vec2::splat(2.2), 17.0) * 0.55 + w.loss * 0.75;
    let loss = if t > 0.92 {
        1.0
    } else if t > 0.78 {
        0.5
    } else {
        0.0
    };
    let moss = smoothstep(
        0.45,
        0.6,
        w.moss * 0.85 + (n(Vec2::splat(3.0), 19.0) - 0.5) * 0.7,
    );
    // Lichen in spots, a few centimetres across.
    let cell = (p * 18.0).floor();
    let spot = hash::unit(hash::of(&[cell.x as i32 as u32, cell.y as i32 as u32, 23]));
    let inside = ((p * 18.0).fract() - 0.5).length();
    let lichen = if spot < w.lichen * 0.35 {
        1.0 - smoothstep(0.25, 0.45, inside)
    } else {
        0.0
    };
    let streak = w.streak * smoothstep(0.55, 0.8, n(Vec2::new(14.0, 0.8), 29.0));
    let soot = w.soot * smoothstep(0.3, 0.75, n(Vec2::new(3.0, 1.2), 31.0));
    [crack, loss, moss, streak, lichen, soot]
}

/// The motif layers drawn from the symbols solved, at a point `grid` in cubes across and
/// down the plane from its top left. Crack, loss, moss, streak, lichen, soot.
fn by_motifs(
    solved: &[Solved; 4],
    grid: Vec2,
    n: &dyn Fn(Vec2, f32) -> f32,
    seed: u32,
) -> [f32; 6] {
    let [cracks, loss, growth, flow] = solved;
    let crack = crack_at(cracks, grid * cracks.per_cell as f32, seed, n);
    // Lost stones and spalls: the symbol under the point, its edge wandering.
    let g = grid * loss.per_cell as f32
        + Vec2::new(
            n(Vec2::splat(9.0), 37.0) - 0.5,
            n(Vec2::splat(9.0), 38.0) - 0.5,
        ) * 0.6;
    let lost = match loss.at(g.x.floor() as i32, g.y.floor() as i32) {
        'x' => 1.0,
        's' => 0.5,
        _ => 0.0,
    };
    // Moss as a soft field, never cut out: the material blends it by its own height, so a
    // weak field fills the joints and a strong one covers the stones (build.wesl).
    let g = grid * growth.per_cell as f32;
    let moss = smoothstep(
        0.2,
        0.8,
        field(growth, g, 'm') + (n(Vec2::splat(5.0), 41.0) - 0.5) * 0.45,
    );
    let lichen = spots(growth, g, seed);
    let g = grid * flow.per_cell as f32;
    let streak = streak_at(flow, g, seed) * (0.6 + 0.4 * n(Vec2::new(4.0, 1.0), 43.0));
    let soot = smoothstep(
        0.3,
        0.7,
        field(flow, g, 'k') + (n(Vec2::new(3.0, 1.5), 47.0) - 0.5) * 0.5,
    );
    [crack, lost, moss, streak, lichen, soot]
}

/// A symbol's share round a point, between the four pixels' middles nearest it.
fn field(s: &Solved, g: Vec2, c: char) -> f32 {
    let q = g - Vec2::splat(0.5);
    let (x, y) = (q.x.floor() as i32, q.y.floor() as i32);
    let f = q - q.floor();
    let v = |dx: i32, dy: i32| if s.is(x + dx, y + dy, c) { 1.0 } else { 0.0 };
    let top = v(0, 0) * (1.0 - f.x) + v(1, 0) * f.x;
    let bottom = v(0, 1) * (1.0 - f.x) + v(1, 1) * f.x;
    top * (1.0 - f.y) + bottom * f.y
}

/// A pixel's middle, moved a little by its hash so a crack does not run on the grid.
fn middle(x: i32, y: i32, seed: u32) -> Vec2 {
    let j = |k: u32| hash::unit(hash::of(&[x as u32, y as u32, seed, k])) - 0.5;
    Vec2::new(x as f32 + 0.5 + 0.55 * j(1), y as f32 + 0.5 + 0.55 * j(2))
}

/// How far a point is from a segment.
fn to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

/// A crack's darkness at a point `g` in crack pixels: its pixels' middles joined to their
/// neighbours', a line a quarter of a pixel wide, a little wider here and there.
fn crack_at(s: &Solved, g: Vec2, seed: u32, n: &dyn Fn(Vec2, f32) -> f32) -> f32 {
    let (ix, iy) = (g.x.floor() as i32, g.y.floor() as i32);
    let mut best = f32::MAX;
    for y in iy - 1..=iy + 1 {
        for x in ix - 1..=ix + 1 {
            if !s.is(x, y, 'c') {
                continue;
            }
            let a = middle(x, y, seed);
            let mut lone = true;
            for (dx, dy) in [
                (1, 0),
                (0, 1),
                (-1, 0),
                (0, -1),
                (1, 1),
                (1, -1),
                (-1, 1),
                (-1, -1),
            ] {
                if s.is(x + dx, y + dy, 'c') {
                    lone = false;
                    best = best.min(to_segment(g, a, middle(x + dx, y + dy, seed)));
                }
            }
            if lone {
                best = best.min((g - a).length());
            }
        }
    }
    let width = 0.16 + 0.14 * n(Vec2::splat(6.0), 53.0);
    1.0 - smoothstep(width * 0.5, width, best)
}

/// Lichen: a few spots in each pixel of it, each its own size.
fn spots(s: &Solved, g: Vec2, seed: u32) -> f32 {
    let (x, y) = (g.x.floor() as i32, g.y.floor() as i32);
    if !s.is(x, y, 'l') {
        return 0.0;
    }
    let mut out: f32 = 0.0;
    for k in 0..3u32 {
        let h = |i: u32| hash::unit(hash::of(&[x as u32, y as u32, seed, 60 + k * 3 + i]));
        let at = Vec2::new(x as f32 + 0.2 + 0.6 * h(0), y as f32 + 0.2 + 0.6 * h(1));
        let r = 0.08 + 0.14 * h(2);
        out = out.max(1.0 - smoothstep(r * 0.6, r, (g - at).length()));
    }
    out
}

/// A streak at a point: a narrow line down the pixel's middle, darkest at its top and
/// fading to its foot.
fn streak_at(s: &Solved, g: Vec2, seed: u32) -> f32 {
    let (x, y) = (g.x.floor() as i32, g.y.floor() as i32);
    let mut out: f32 = 0.0;
    for cx in x - 1..=x + 1 {
        if !s.is(cx, y, 'w') {
            continue;
        }
        let up = (1..).take_while(|k| s.is(cx, y - k, 'w')).count() as f32;
        let down = (1..).take_while(|k| s.is(cx, y + k, 'w')).count() as f32;
        let along = (up + (g.y - y as f32)) / (up + down + 1.0);
        let centre = cx as f32 + 0.5 + 0.4 * (hash::unit(hash::of(&[cx as u32, seed, 71])) - 0.5);
        let across = (g.x - centre).abs() / (0.18 + 0.12 * along);
        out = out.max(libm::expf(-across * across) * (1.0 - 0.75 * along));
    }
    out
}

/// Packs the planes' maps into one atlas, a lookup beside it.
pub fn pack(planes: &[Plane], drawn: Vec<Drawn>, low: IVec3, high: IVec3) -> Weathered {
    let dims = high - low + IVec3::ONE;
    let slots: usize = planes.iter().map(|p| p.cubes.len()).sum();
    let per_row = (libm::ceilf(libm::sqrtf(slots as f32)) as usize).max(1);
    let side = per_row * TILE;
    let mut atlas: [Vec<u8>; LAYERS] = std::array::from_fn(|_| vec![0; side * side * 4]);
    let across = dims.x as usize * 6;
    let mut lookup = vec![0u32; across * (dims.y * dims.z) as usize];
    let mut stats = Stats::default();
    let mut slot = 0;
    for (plane, drawn) in planes.iter().zip(drawn) {
        let s = drawn.stats;
        stats.faces += s.faces;
        stats.planes += s.planes;
        stats.solves += s.solves;
        stats.first_try += s.first_try;
        stats.solved += s.solved;
        stats.tries += s.tries;
        stats.cells += s.cells;
        for ((at, _), maps) in plane.cubes.iter().zip(drawn.maps) {
            let (sx, sy) = (slot % per_row, slot / per_row);
            for (layer, map) in atlas.iter_mut().zip(&maps) {
                for row in 0..TILE {
                    let to = ((sy * TILE + row) * side + sx * TILE) * 4;
                    layer[to..to + TILE * 4]
                        .copy_from_slice(&map[row * TILE * 4..(row + 1) * TILE * 4]);
                }
            }
            let rel = *at - low;
            lookup[(rel.z * dims.y + rel.y) as usize * across + rel.x as usize * 6 + plane.face] =
                slot as u32 + 1;
            slot += 1;
        }
    }
    Weathered {
        low,
        dims,
        lookup,
        per_row: per_row as u32,
        slots: slot as u32,
        atlas,
        stats,
    }
}
