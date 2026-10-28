//! A mine's timber stood down its drifts (D559, docs/dungeons.md, the hewn mine's plan): a set
//! every `Timber::every` metres down each straight drift, as a hard-rock mine timbered its
//! levels, in step across a drift's cells (the first half a step in from a cell's end), so
//! the lagging over them roofs it whole; none where a door's leaves swing, so a leaf never
//! passes through a post. A drift's turns, crossings and stairs are not timbered yet.

use super::{CELL, CUBE, Frame, MID, Piece, Stamp};
use crate::Dungeon;
use crate::place::{Side, Space};
use crate::theme::Timber;
use glam::Vec3;

/// A door's leaf, 1.5 m, swings so far into its cell; no set stands nearer the doorway.
const LEAF_M: f32 = 1.6;

pub(super) fn lay(dungeon: &Dungeon, timber: &Timber, out: &mut Stamp) {
    let cell_m = CELL as f32 * CUBE;
    let sets = (cell_m / timber.every).round().max(1.0) as usize;
    let step = cell_m / sets as f32;
    for cell in &dungeon.layout.cells {
        if !matches!(cell.space, Space::Corridor(_)) {
            continue;
        }
        let wall = |d: usize| cell.sides[d] == Side::Wall;
        // The drift along the set's Z: north and south (its front facing south), or east and
        // west (facing east); the sides a drift's ends are on.
        let (facing, ends) = if wall(1) && wall(3) {
            (2, [0, 2])
        } else if wall(0) && wall(2) {
            (1, [3, 1])
        } else {
            continue;
        };
        let door = |d: usize| matches!(cell.sides[d], Side::Door(_));
        let f = Frame::new(cell.at, 0);
        for k in 0..sets {
            let along = (k as f32 + 0.5) * step;
            if (door(ends[0]) && along < LEAF_M) || (door(ends[1]) && cell_m - along < LEAF_M) {
                continue;
            }
            let along = along / CUBE;
            let at = if facing == 2 {
                f.point(MID, 0.0, along)
            } else {
                f.point(along, 0.0, MID)
            };
            out.pieces.push(Piece {
                model: timber.set.clone(),
                at,
                facing,
                scale: Vec3::ONE,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::kind;
    use super::super::{CELL, CUBE, stamp};
    use crate::place::{Side, Space};
    use crate::theme::{Theme, assets};
    use crate::{ModuleSet, generate};
    use terra_gen::Seed;

    /// Down every straight drift of a timbered theme, a set every `every` metres, in step
    /// across its cells, none within a door's swing; nothing in an untimbered theme.
    #[test]
    fn every_straight_drift_is_timbered_in_step() {
        for theme in Theme::all(&assets()).unwrap() {
            let set = ModuleSet::new(&theme).unwrap();
            let (mut drifts, mut sets) = (0, 0);
            for s in 0..40 {
                let Ok(dungeon) = generate(&theme, &set, Seed(s)) else {
                    continue;
                };
                let laid = stamp(&dungeon, &theme, &kind);
                let model = theme.timber.as_ref().map(|t| t.set.as_str());
                let posts: Vec<_> = (laid.pieces.iter())
                    .filter(|p| Some(p.model.as_str()) == model)
                    .collect();
                let Some(timber) = &theme.timber else {
                    assert!(
                        posts.is_empty(),
                        "{}: sets in an untimbered theme",
                        theme.name
                    );
                    continue;
                };
                let cell_m = CELL as f32 * CUBE;
                for cell in &dungeon.layout.cells {
                    let wall = |d: usize| cell.sides[d] == Side::Wall;
                    let ns = wall(1) && wall(3);
                    if !matches!(cell.space, Space::Corridor(_)) || !(ns || wall(0) && wall(2)) {
                        continue;
                    }
                    drifts += 1;
                    let min = (cell.at.x as f32 * cell_m, cell.at.z as f32 * cell_m);
                    let here: Vec<f32> = (posts.iter())
                        .filter(|p| p.at.x > min.0 && p.at.x < min.0 + cell_m)
                        .filter(|p| p.at.z > min.1 && p.at.z < min.1 + cell_m)
                        .filter(|p| (p.at.y - laid_floor(cell.at.y)).abs() < 0.01)
                        .map(|p| if ns { p.at.z - min.1 } else { p.at.x - min.0 })
                        .collect();
                    sets += here.len();
                    let doors = (cell.sides.iter()).any(|s| matches!(s, Side::Door(_)));
                    let want = (cell_m / timber.every).round() as usize;
                    assert!(
                        here.len() == want || doors && here.len() < want,
                        "{} seed {s}: {} sets in a drift cell, want {want}",
                        theme.name,
                        here.len()
                    );
                    for along in here {
                        // In step: half a step in from the cell's end, a step apart.
                        let k = along / timber.every - 0.5;
                        assert!(
                            (k - k.round()).abs() < 1e-3,
                            "{}: a set off its step",
                            theme.name
                        );
                    }
                }
            }
            if theme.timber.is_some() {
                assert!(
                    sets * 2 > drifts,
                    "{}: {sets} sets in {drifts} drift cells",
                    theme.name
                );
            }
        }
    }

    /// A level's floor top in metres.
    fn laid_floor(level: i32) -> f32 {
        super::super::floor_of(level) as f32 * CUBE
    }
}
