//! A material's cells (D305): the stones of a wall, the shingles of a covering, found from
//! its height map, so damage breaks a material by its own structure: a wall's mortar opens
//! and whole stones fall out, a covering loses whole shingles. The low places of its height
//! map are the joints (mortar, the gaps under and between shingles); what they part are its
//! cells, each found by a flood over the map, as the islands of an image are labelled, and
//! given a number of its own. Four bytes a texel, [`CELLS`] square: the cell's number (0 a
//! joint, any other from 8 to 255), the way to its middle in texels across and down from
//! 128, rounded so every texel of a cell finds the very same middle, and how far the texel
//! is from its cell's edge (`distances`). A cell gives way whole, by how much was lost at
//! its middle, read once there.
//!
//! Worked out the same from the same picture wherever it is read: on the CPU
//! (`relief::Relief::cells`), where a wall's surface is pushed back where a stone has gone,
//! its points marked sunk, which the texture arrays' shader draws as gone; and on the GPU
//! beside the material's maps (`terra_game`'s texture arrays), for a covering's shingles
//! and every edge's shape.

/// How many texels a side the cells are kept at.
pub const CELLS: u32 = 512;

/// Where a laid stone falls out of its wall (D305): from this much lost at its middle, the
/// weakest first and the last at the two summed, each stone at its cell's share between.
/// The surface reads them (`fallen`), marking where it is sunk for the shader to draw.
pub const FALLS: [f32; 2] = [0.55, 0.45];

/// The least a cell is, in texels at [`CELLS`]: a speck parted off by a joint is joint.
const LEAST: usize = 20;

/// The cells of a height map `width` by `height`, `stride` bytes a texel with its height in
/// the first, the lowest `share` of its texels being joints, tiling.
pub fn label(data: &[u8], width: usize, height: usize, stride: usize, share: f32) -> Vec<u8> {
    let n = CELLS as usize;
    let (width, height, stride) = (width.max(1), height.max(1), stride.max(1));
    // Down to the cells' size, each the least of the texels under it, so a joint a texel
    // wide is kept.
    let mut low = vec![255u8; n * n];
    for y in 0..n {
        let (y0, y1) = (
            y * height / n,
            ((y + 1) * height / n).max(y * height / n + 1),
        );
        for x in 0..n {
            let (x0, x1) = (x * width / n, ((x + 1) * width / n).max(x * width / n + 1));
            let mut least = 255u8;
            for j in y0..y1 {
                for i in x0..x1 {
                    let at = ((j % height) * width + (i % width)) * stride;
                    least = least.min(data.get(at).copied().unwrap_or(255));
                }
            }
            low[y * n + x] = least;
        }
    }
    // The height under which the lowest `share` of them lie.
    let mut sorted = low.clone();
    sorted.sort_unstable();
    let cut = sorted[(share.clamp(0.0, 0.95) * (sorted.len() - 1) as f32) as usize];
    let above: Vec<bool> = low.iter().map(|&h| h > cut).collect();
    // A texel beside a joint is joint too, so two stones or shingles a thin gap apart, or
    // touching at a corner, are two.
    let solid: Vec<bool> = if share <= 0.0 {
        vec![true; n * n]
    } else {
        (0..n * n)
            .map(|at| {
                let (x, y) = (at % n, at / n);
                above[at]
                    && [(1, 0), (n - 1, 0), (0, 1), (0, n - 1)]
                        .iter()
                        .all(|&(dx, dy)| above[(y + dy) % n * n + (x + dx) % n])
            })
            .collect()
    };
    // Each island of solid texels, wrapping round, a number of its own, and each of its
    // texels the way to its middle (found unwrapped, so an island across an edge is whole).
    let mut out = vec![0u8; n * n * 4];
    for texel in out.chunks_exact_mut(4) {
        texel.copy_from_slice(&[0, 128, 128, 255]);
    }
    let mut seen = vec![false; n * n];
    let mut count = 0u32;
    let mut stack = Vec::new();
    let mut members = Vec::new();
    for start in 0..n * n {
        if seen[start] || !solid[start] {
            continue;
        }
        count += 1;
        members.clear();
        stack.push((start, (start % n) as i64, (start / n) as i64));
        seen[start] = true;
        while let Some((at, ux, uy)) = stack.pop() {
            members.push((at, ux, uy));
            let (x, y) = (at % n, at / n);
            for (dx, dy, sx, sy) in [
                (1, 0, 1, 0),
                (n - 1, 0, -1, 0),
                (0, 1, 0, 1),
                (0, n - 1, 0, -1),
            ] {
                let next = (y + dy) % n * n + (x + dx) % n;
                if !seen[next] && solid[next] {
                    seen[next] = true;
                    stack.push((next, ux + sx, uy + sy));
                }
            }
        }
        if members.len() < LEAST {
            continue;
        }
        // A number from 8 to 255, the same every run, scattered so neighbours differ.
        let h = crate::layout::mix3(0, count, 0, 0);
        let value = 8 + (h % 248) as u8;
        let count_f = members.len() as f32;
        let (mx, my) = members.iter().fold((0.0f32, 0.0f32), |(a, b), &(_, x, y)| {
            (a + x as f32 / count_f, b + y as f32 / count_f)
        });
        for &(at, ux, uy) in &members {
            let to = |d: f32| (128.0 + d.round()).clamp(1.0, 255.0) as u8;
            out[at * 4..at * 4 + 4].copy_from_slice(&[
                value,
                to(mx - ux as f32),
                to(my - uy as f32),
                255,
            ]);
        }
    }
    distances(&mut out, n);
    out
}

/// How far each texel is from the edge of its cell or joint, in texels, in the fourth
/// channel at [`DISTANCE_SCALE`] a texel: a distance field, as fonts and decals are drawn
/// from (Valve's signed distance fields), so an edge read between texels runs smooth and
/// straight however near it is seen, where the texels' numbers alone would step. Two
/// sweeps of a chamfer transform (Borgefors), wrapping.
fn distances(out: &mut [u8], n: usize) {
    let id = |at: usize| out[at * 4];
    let mut far: Vec<f32> = (0..n * n)
        .map(|at| {
            let (x, y) = (at % n, at / n);
            let edge = [(1, 0), (n - 1, 0), (0, 1), (0, n - 1)]
                .iter()
                .any(|&(dx, dy)| id((y + dy) % n * n + (x + dx) % n) != id(at));
            if edge { 0.5 } else { f32::MAX }
        })
        .collect();
    const DIAGONAL: f32 = std::f32::consts::SQRT_2;
    let forward = [
        (n - 1, n - 1, DIAGONAL),
        (0, n - 1, 1.0),
        (1, n - 1, DIAGONAL),
        (n - 1, 0, 1.0),
    ];
    let backward = [
        (1, 1, DIAGONAL),
        (0, 1, 1.0),
        (n - 1, 1, DIAGONAL),
        (1, 0, 1.0),
    ];
    for _ in 0..2 {
        for (steps, reversed) in [(forward, false), (backward, true)] {
            for k in 0..n * n {
                let at = if reversed { n * n - 1 - k } else { k };
                let (x, y) = (at % n, at / n);
                for &(dx, dy, cost) in &steps {
                    let from = (y + dy) % n * n + (x + dx) % n;
                    if id(from) == id(at) && far[from] + cost < far[at] {
                        far[at] = far[from] + cost;
                    }
                }
            }
        }
    }
    for (at, distance) in far.into_iter().enumerate() {
        out[at * 4 + 3] = (distance.min(255.0 / DISTANCE_SCALE) * DISTANCE_SCALE) as u8;
    }
}

/// How many steps a texel the cells' distances are kept in (`distances`): to 15.9 texels.
pub const DISTANCE_SCALE: f32 = 16.0;

/// How far a laid stone's hollow reaches round it into the mortar, in texels of the cells:
/// the mortar round a stone that has gone goes with it, up to the stones beside, so no fin
/// of mortar is left standing between two stones gone.
pub const MORTAR_REACH: f32 = 12.0;

/// How far inside a stone left standing a point is, in texels of the cells, read between
/// the four texels round it from their distance fields: below 0 in a joint, or in a stone
/// that has given way (`given`'s rule), so the edge of what stands is smooth.
pub fn standing(
    cells: &[u8],
    uv: [f32; 2],
    lost: &dyn Fn([f32; 2]) -> f32,
    begin: f32,
    span: f32,
) -> f32 {
    let n = CELLS as f32;
    let (px, py) = (uv[0] * n - 0.5, uv[1] * n - 0.5);
    let (ox, oy) = (px.floor(), py.floor());
    let (fx, fy) = (px - ox, py - oy);
    let mut out = 0.0;
    for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
        let w = (if dx > 0.0 { fx } else { 1.0 - fx }) * (if dy > 0.0 { fy } else { 1.0 - fy });
        let tap = [(ox + dx + 0.5) / n, (oy + dy + 0.5) / n];
        let t = texel(cells, tap);
        let far = f32::from(t[3]) / DISTANCE_SCALE;
        let cell = f32::from(t[0]) / 255.0;
        let stands = cell >= 0.01 && lost(middle(cells, tap)) < begin + span * cell;
        out += w * if stands { far } else { -far };
    }
    out
}

/// Whether a point of a laid wall has gone (D305): out of what stands (`standing`), and in
/// a stone that has given way or the mortar within [`MORTAR_REACH`] of one, each stone judged once,
/// at its middle (`middle`).
pub fn fallen(cells: &[u8], uv: [f32; 2], lost: &dyn Fn([f32; 2]) -> f32) -> bool {
    let [begin, span] = FALLS;
    if standing(cells, uv, lost, begin, span) >= 0.0 {
        return false;
    }
    if given(cells, uv, lost, begin, span).1 >= 0.5 {
        return true;
    }
    [0.5, 1.0].into_iter().any(|share| {
        let r = MORTAR_REACH * share / CELLS as f32;
        (0..8).any(|k| {
            let a = k as f32 * std::f32::consts::TAU / 8.0 + 0.4;
            let near = [uv[0] + libm::cosf(a) * r, uv[1] + libm::sinf(a) * r];
            given(cells, near, lost, begin, span).1 >= 0.5
        })
    })
}

/// One of a picture's cells: its middle on the texture and how far its edge is from it at
/// its nearest, both as a share of the texture.
pub type Stone = ([f32; 2], f32);

/// Its largest cells, at most `most` of them, largest first, none less than `least` texels
/// from its middle to its nearest edge: each its middle on the texture and how far from
/// it its edge is at its nearest, both as a share of the texture. What a dressed block cut
/// from the material is drawn from (`dress::corner`), each the inside of one of its own
/// stones.
pub fn largest(cells: &[u8], most: usize, least: f32) -> Vec<Stone> {
    let n = CELLS as usize;
    let mut by_middle: std::collections::BTreeMap<[i32; 2], f32> =
        std::collections::BTreeMap::new();
    for at in 0..n * n {
        let t = &cells[at * 4..at * 4 + 4];
        if t[0] == 0 {
            continue;
        }
        let far = f32::from(t[3]) / DISTANCE_SCALE;
        if far < least {
            continue;
        }
        let (x, y) = ((at % n) as i32, (at / n) as i32);
        let middle = [x + i32::from(t[1]) - 128, y + i32::from(t[2]) - 128];
        let best = by_middle.entry(middle).or_insert(0.0);
        *best = best.max(far);
    }
    let mut out: Vec<Stone> = by_middle
        .into_iter()
        .map(|([x, y], far)| {
            let n = n as f32;
            ([(x as f32 + 0.5) / n, (y as f32 + 0.5) / n], far / n)
        })
        .collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0[0].total_cmp(&b.0[0])));
    out.truncate(most);
    out
}

/// The texel of the cells where a texture is at a point, repeating (as the arrays' shader
/// reads it, unfiltered).
fn texel(cells: &[u8], uv: [f32; 2]) -> &[u8] {
    let n = CELLS as i64;
    let texel = |v: f32| ((v - v.floor()) * n as f32).floor() as i64 % n;
    let (x, y) = (texel(uv[0]), texel(uv[1]));
    let at = ((y * n + x) * 4) as usize;
    &cells[at..at + 4]
}

/// A cell's number where a texture is at a point, 0 to 1.
pub fn at(cells: &[u8], uv: [f32; 2]) -> f32 {
    f32::from(texel(cells, uv)[0]) / 255.0
}

/// Where on the texture the middle of the cell at a point is.
pub fn middle(cells: &[u8], uv: [f32; 2]) -> [f32; 2] {
    let t = texel(cells, uv);
    let n = CELLS as f32;
    let centre = |v: f32| ((v * n).floor() + 0.5) / n;
    [
        centre(uv[0]) + (f32::from(t[1]) - 128.0) / n,
        centre(uv[1]) + (f32::from(t[2]) - 128.0) / n,
    ]
}

/// How much of the cells' texels round a point, weighed by how near each is, is a cell that
/// has given way, a cell giving way when what is lost at its middle (`lost`, given where on
/// the texture its middle is) passes `begin` and its number's share of `span` more; and how
/// much is joint: as the arrays' shader's `given`.
pub fn given(
    cells: &[u8],
    uv: [f32; 2],
    lost: &dyn Fn([f32; 2]) -> f32,
    begin: f32,
    span: f32,
) -> (f32, f32) {
    let n = CELLS as f32;
    let (px, py) = (uv[0] * n - 0.5, uv[1] * n - 0.5);
    let (ox, oy) = (px.floor(), py.floor());
    let (fx, fy) = (px - ox, py - oy);
    let (mut joint, mut gone) = (0.0, 0.0);
    for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
        let w = (if dx > 0.0 { fx } else { 1.0 - fx }) * (if dy > 0.0 { fy } else { 1.0 - fy });
        let tap = [(ox + dx + 0.5) / n, (oy + dy + 0.5) / n];
        let cell = at(cells, tap);
        if cell < 0.01 {
            joint += w;
        } else if lost(middle(cells, tap)) >= begin + span * cell {
            gone += w;
        }
    }
    (joint, gone)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stones_parted_by_low_joints_are_cells_of_their_own_and_the_joints_none() {
        // Four raised squares, each a stone, in a grid of low mortar lines.
        let side = 1024usize;
        let mut height = vec![0u8; side * side];
        for y in 0..side {
            for x in 0..side {
                let joint = x % 256 < 24 || y % 256 < 24;
                height[y * side + x] = if joint { 40 } else { 220 };
            }
        }
        let made = label(&height, side, side, 1, 0.1);
        let n = CELLS as usize;
        let cell = |x: usize, y: usize| made[(y * n + x) * 4];
        // The joints are none; each stone one number throughout, and its neighbours others.
        assert_eq!(cell(2, 2), 0);
        let stones = [cell(64, 64), cell(192, 64), cell(64, 192), cell(192, 192)];
        assert!(stones.iter().all(|&v| v > 0), "{stones:?}");
        assert_eq!(cell(64, 64), cell(100, 100));
        let distinct: std::collections::BTreeSet<u8> = stones.into_iter().collect();
        assert!(distinct.len() >= 3, "{stones:?}");
        // The same picture read four bytes a texel gives the same cells.
        let rgba: Vec<u8> = height.iter().flat_map(|&h| [h, 0, 0, 255]).collect();
        assert_eq!(label(&rgba, side, side, 4, 0.1), made);
        // A stone gives way whole once enough is lost for its number, and not before.
        let uv = [64.5 / n as f32, 64.5 / n as f32];
        let number = at(&made, uv);
        assert_eq!(
            given(&made, uv, &|_| 0.5 + 0.5 * number - 0.01, 0.5, 0.5).1,
            0.0
        );
        assert_eq!(
            given(&made, uv, &|_| 0.5 + 0.5 * number + 0.01, 0.5, 0.5).1,
            1.0
        );
        // Every texel of a stone knows its middle: the stone's, at (70, 70) texels of
        // the cells for a stone from 12 to 127 across and down (a stone a quarter of the
        // picture, its joint 12 texels wide, the cells half the picture's size).
        let middle_of = |x: usize, y: usize| {
            middle(
                &made,
                [(x as f32 + 0.5) / n as f32, (y as f32 + 0.5) / n as f32],
            )
            .map(|v| v * n as f32)
        };
        // The very same from every one of its texels, so a stone is judged whole by one
        // reading, never split across it.
        let first = middle_of(20, 20);
        assert!(
            (first[0] - 70.0).abs() < 1.0 && (first[1] - 70.0).abs() < 1.0,
            "{first:?}"
        );
        for (x, y) in [(64, 64), (120, 100), (20, 115), (115, 20)] {
            assert_eq!(middle_of(x, y), first, "from ({x}, {y})");
        }
    }

    #[test]
    fn a_stones_edge_read_between_texels_runs_straight_through_them() {
        // One raised square, a stone, its edges at texels 128 and 384 of the cells.
        let side = 512usize;
        let height: Vec<u8> = (0..side * side)
            .map(|i| {
                let (x, y) = (i % side, i / side);
                if (128..384).contains(&x) && (128..384).contains(&y) {
                    220
                } else {
                    30
                }
            })
            .collect();
        let made = label(&height, side, side, 1, 0.5);
        let n = CELLS as f32;
        let whole = |_: [f32; 2]| 0.0;
        // Deep inside, as far as is kept.
        let deep = standing(&made, [0.5, 0.5], &whole, 0.5, 0.5);
        assert!(deep > 15.0, "{deep}");
        // Across its west edge the reading rises steadily, half a texel at a time.
        let across: Vec<f32> = (0..9)
            .map(|k| standing(&made, [(126.0 + k as f32 * 0.5) / n, 0.5], &whole, 0.5, 0.5))
            .collect();
        for pair in across.windows(2) {
            assert!(pair[1] > pair[0], "{across:?}");
        }
        // And crosses into the stone at the same place on every row: a straight edge.
        let crossing = |v: f32| {
            (0..60)
                .map(|k| standing(&made, [(124.0 + k as f32 * 0.1) / n, v], &whole, 0.5, 0.5))
                .position(|s| s >= 0.0)
        };
        assert!(crossing(0.3).is_some());
        assert_eq!(crossing(0.3), crossing(0.61));
        // Gone, it stands nowhere.
        let gone = |_: [f32; 2]| 1.0;
        assert!(standing(&made, [0.5, 0.5], &gone, 0.5, 0.5) < 0.0);
        assert!(fallen(&made, [0.5, 0.5], &gone) && !fallen(&made, [0.5, 0.5], &whole));
    }
}
