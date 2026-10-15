//! A texture set's cells (D305): the stones of a wall, the shingles of a covering, found
//! once as the set is made resident, so damage breaks a material by its own structure: a
//! wall's mortar opens and whole stones fall, a covering loses whole shingles. The low
//! places of its height map are the joints (mortar, the gaps under and between shingles);
//! what they part are its cells, each found by a flood over the map, as the islands of an
//! image are labelled, and given a number of its own. Kept beside the four maps, a byte a
//! texel at a quarter of their size: 0 a joint, any other a cell's number.

/// How many texels a side the cells are kept at.
pub const CELLS: u32 = 512;

/// The least a cell is, in texels at [`CELLS`]: a speck parted off by a joint is joint.
const LEAST: usize = 20;

/// The cells of a height map `side` square (RGBA, its height in red), the lowest `share`
/// of its texels being joints, tiling: [`CELLS`] square, a byte a texel.
pub fn cells(height: &[u8], side: u32, share: f32) -> Vec<u8> {
    let (side, n) = (side as usize, CELLS as usize);
    let step = (side / n).max(1);
    // Down to the cells' size, each the least of the texels under it, so a joint a texel
    // wide is kept.
    let mut low = vec![255u8; n * n];
    for y in 0..n {
        for x in 0..n {
            let mut least = 255u8;
            for j in 0..step {
                for i in 0..step {
                    let at = ((y * step + j) % side * side + (x * step + i) % side) * 4;
                    least = least.min(height.get(at).copied().unwrap_or(255));
                }
            }
            low[y * n + x] = least;
        }
    }
    // The height under which the lowest `share` of them lie.
    let mut sorted = low.clone();
    sorted.sort_unstable();
    let cut = sorted[((share.clamp(0.0, 0.95)) * (sorted.len() - 1) as f32) as usize];
    let solid: Vec<bool> = if share <= 0.0 {
        vec![true; n * n]
    } else {
        low.iter().map(|&h| h > cut).collect()
    };
    // Each island of solid texels, wrapping round, a number of its own.
    let mut out = vec![0u8; n * n];
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
        stack.push(start);
        seen[start] = true;
        while let Some(at) = stack.pop() {
            members.push(at);
            let (x, y) = (at % n, at / n);
            for (dx, dy) in [(1, 0), (n - 1, 0), (0, 1), (0, n - 1)] {
                let next = (y + dy) % n * n + (x + dx) % n;
                if !seen[next] && solid[next] {
                    seen[next] = true;
                    stack.push(next);
                }
            }
        }
        if members.len() < LEAST {
            continue;
        }
        // A number from 8 to 255, the same every run, scattered so neighbours differ.
        let mut h = count.wrapping_mul(0x9E37_79B1);
        h ^= h >> 15;
        h = h.wrapping_mul(0x2C1B_3C6D);
        h ^= h >> 12;
        let value = 8 + (h % 248) as u8;
        for &at in &members {
            out[at] = value;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stones_parted_by_low_joints_are_cells_of_their_own_and_the_joints_none() {
        // Four raised squares, each a stone, in a grid of low mortar lines.
        let side = 1024u32;
        let mut height = vec![0u8; (side * side * 4) as usize];
        for y in 0..side {
            for x in 0..side {
                let joint = x % 256 < 24 || y % 256 < 24;
                height[((y * side + x) * 4) as usize] = if joint { 40 } else { 220 };
            }
        }
        let made = cells(&height, side, 0.1);
        let n = CELLS as usize;
        let at = |x: usize, y: usize| made[y * n + x];
        // The joints are none; each stone one number throughout, and its neighbours others.
        assert_eq!(at(2, 2), 0);
        let stones = [at(64, 64), at(192, 64), at(64, 192), at(192, 192)];
        assert!(stones.iter().all(|&v| v > 0), "{stones:?}");
        assert_eq!(at(64, 64), at(100, 100));
        let distinct: std::collections::BTreeSet<u8> = stones.into_iter().collect();
        assert!(distinct.len() >= 3, "{stones:?}");
    }
}
