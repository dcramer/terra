//! A member cut to shape: level at a height or plumb through a point, and rings added along
//! it where it is cut, or where they are too far apart to be.

use super::*;

/// How far apart a member's rings are at most where its end is cut to a roof's slope.
pub(super) const LEAN_RING_M: f32 = 0.25;

/// A member cut level at a height: rings added where each line along it crosses the height,
/// and every point over it brought down onto it.
pub(super) fn level_cut(made: &mut Member, from: Vec3, axis: Vec3, height: f32) {
    if made.grid.iter().flatten().all(|p| p.y <= height) {
        return;
    }
    let mut places: Vec<f32> = Vec::new();
    for i in 1..made.grid.len() {
        for (p, q) in made.grid[i - 1].iter().zip(&made.grid[i]) {
            if (p.y - height) * (q.y - height) < 0.0 {
                let t = (height - p.y) / (q.y - p.y);
                places.push((p.lerp(*q, t) - from).dot(axis));
            }
        }
    }
    places.sort_by(f32::total_cmp);
    places.dedup_by(|a, b| (*a - *b).abs() < 2e-3);
    rings_at(made, from, axis, &places);
    for p in made.grid.iter_mut().flatten() {
        p.y = p.y.min(height);
    }
}

/// A member cut plumb through a point, square to a level way: rings added where each line
/// along it crosses that upright plane, and every point past it, that way, brought back onto it.
pub(super) fn plumb_cut(made: &mut Member, from: Vec3, axis: Vec3, at: Vec3, way: Vec3) {
    let past = |p: Vec3| (p - at).dot(way);
    if made.grid.iter().flatten().all(|p| past(*p) <= 0.0) {
        return;
    }
    let mut places: Vec<f32> = Vec::new();
    for i in 1..made.grid.len() {
        for (p, q) in made.grid[i - 1].iter().zip(&made.grid[i]) {
            let (a, b) = (past(*p), past(*q));
            if a * b < 0.0 {
                places.push((p.lerp(*q, a / (a - b)) - from).dot(axis));
            }
        }
    }
    places.sort_by(f32::total_cmp);
    places.dedup_by(|a, b| (*a - *b).abs() < 2e-3);
    rings_at(made, from, axis, &places);
    for p in made.grid.iter_mut().flatten() {
        *p -= way * past(*p).max(0.0);
    }
}

/// Rings added where a member's rings, along it from `from`, pass these places, each a
/// blend of the two either side.
pub(super) fn rings_at(made: &mut Member, from: Vec3, axis: Vec3, places: &[f32]) {
    for &place in places {
        let along = |ring: &[Vec3]| {
            let middle = ring.iter().copied().sum::<Vec3>() / ring.len().max(1) as f32;
            (middle - from).dot(axis)
        };
        let Some(i) = (1..made.grid.len()).find(|&i| {
            let (a, b) = (along(&made.grid[i - 1]), along(&made.grid[i]));
            a + 1e-3 < place && place < b - 1e-3
        }) else {
            continue;
        };
        let (a, b) = (along(&made.grid[i - 1]), along(&made.grid[i]));
        let t = (place - a) / (b - a);
        let ring: Vec<Vec3> = made.grid[i - 1]
            .iter()
            .zip(&made.grid[i])
            .map(|(p, q)| p.lerp(*q, t))
            .collect();
        let uv: Vec<[f32; 2]> = made.uvs[i - 1]
            .iter()
            .zip(&made.uvs[i])
            .map(|(p, q)| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t])
            .collect();
        made.grid.insert(i, ring);
        made.uvs.insert(i, uv);
        if !made.tone.is_empty() {
            let tone: Vec<f32> = made.tone[i - 1]
                .iter()
                .zip(&made.tone[i])
                .map(|(p, q)| p + (q - p) * t)
                .collect();
            made.tone.insert(i, tone);
        }
        if !made.wear.is_empty() {
            let wear: Vec<f32> = made.wear[i - 1]
                .iter()
                .zip(&made.wear[i])
                .map(|(p, q)| p + (q - p) * t)
                .collect();
            made.wear.insert(i, wear);
        }
    }
}

/// Rings put between a member's rings where they are further apart than `most`, on the
/// lines between them, so its shape is as it was but its points close enough to be cut.
pub(super) fn densify(made: &mut Member, most: f32) {
    let mut grid = Vec::with_capacity(made.grid.len());
    let mut uvs = Vec::with_capacity(made.uvs.len());
    let mut tone = Vec::with_capacity(made.tone.len());
    let mut wear = Vec::with_capacity(made.wear.len());
    for i in 0..made.grid.len() {
        if i > 0 {
            let (a, b) = (&made.grid[i - 1], &made.grid[i]);
            let gap = a[0].distance(b[0]);
            let more = (gap / most).ceil() as usize;
            for k in 1..more {
                let t = k as f32 / more as f32;
                grid.push(a.iter().zip(b).map(|(p, q)| p.lerp(*q, t)).collect());
                let (ua, ub) = (&made.uvs[i - 1], &made.uvs[i]);
                uvs.push(
                    ua.iter()
                        .zip(ub)
                        .map(|(p, q)| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t])
                        .collect(),
                );
                if !made.tone.is_empty() {
                    let (ta, tb) = (&made.tone[i - 1], &made.tone[i]);
                    tone.push(ta.iter().zip(tb).map(|(p, q)| p + (q - p) * t).collect());
                }
                if !made.wear.is_empty() {
                    let (wa, wb) = (&made.wear[i - 1], &made.wear[i]);
                    wear.push(wa.iter().zip(wb).map(|(p, q)| p + (q - p) * t).collect());
                }
            }
        }
        grid.push(made.grid[i].clone());
        uvs.push(made.uvs[i].clone());
        if !made.tone.is_empty() {
            tone.push(made.tone[i].clone());
        }
        if !made.wear.is_empty() {
            wear.push(made.wear[i].clone());
        }
    }
    (made.grid, made.uvs, made.tone, made.wear) = (grid, uvs, tone, wear);
}
