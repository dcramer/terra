//! How far round a player each kind is put out: further than it is drawn, so one ahead is
//! there before the camera can see it, as a shipped game keeps its ambient animals spawned
//! out of view ahead of time, on the far side of the lake or round the bend (Red Dead's
//! ambient population, Unreal's MassEntity spawning beyond the cull distance). Put out
//! nearer, inside its drawn distance, it could only be put where no camera looks
//! (`unseen::put`), and a lake in view was left without its ducks the whole lake film
//! (round 23). A cell is looked into for a kind once, as it first comes within that kind's
//! reach, so the far-seen kinds are put out early and the rest as before.

use super::{CELL_M, POPULATE_M};
use bevy::prelude::*;

/// One put out this far past where it is drawn is out of every picture: its fade's margin
/// (`appear::whole`), and a cell across, as far as a spot in it lies from its middle.
const PAST_DRAWN_M: f32 = 4.0 + CELL_M;

/// How near a cell's middle comes to a player before a kind drawn so far off is put out in
/// it: as near as every kind is (`POPULATE_M`), or further for a kind seen further.
pub(super) fn reach_m(seen_m: f32) -> f32 {
    POPULATE_M.max(seen_m + PAST_DRAWN_M)
}

/// Whether a cell is looked into for a kind of this reach now, the nearest player so far
/// from its middle, having been looked into last with one so far (infinitely far before it
/// was looked into at all; below nothing, never again): it is within the kind's reach now
/// and was not then.
pub(super) fn due(reach_m: f32, now_m: f32, before_m: f32) -> bool {
    now_m <= reach_m && reach_m < before_m
}

/// The cells round a cell within so many of it (at most `NEAREST_MOST`), nearest first, so
/// the nearest are put out first when a look leaves some for the next.
pub(super) fn nearest_first(reach: i32) -> &'static [IVec3] {
    use std::sync::OnceLock;
    static OFFSETS: OnceLock<Vec<IVec3>> = OnceLock::new();
    let offsets = OFFSETS.get_or_init(|| {
        let most = NEAREST_MOST;
        let mut offsets: Vec<IVec3> = (-most..=most)
            .flat_map(|x| {
                (-most..=most).flat_map(move |y| (-most..=most).map(move |z| IVec3::new(x, y, z)))
            })
            .collect();
        offsets.sort_by_key(|offset| offset.length_squared());
        offsets
    });
    // A cell's middle within `reach` cells of a player anywhere in its own cell is at most one
    // more off.
    let reach = (reach + 1).min(NEAREST_MOST);
    &offsets[..offsets.partition_point(|offset| offset.length_squared() <= reach * reach)]
}

/// The furthest cells are looked into, in cells: past the furthest-seen kind's reach.
const NEAREST_MOST: i32 = 9;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_is_put_out_once_in_a_cell_ahead_before_any_spot_of_it_is_drawn() {
        // A player coming on toward a cell at a run, looked at every quarter second, over
        // kinds drawn from a few tens of metres (a finch) to across a lake (a heron).
        for seen_m in [35.0, 90.0, 120.0, 160.0, 200.0] {
            let reach = reach_m(seen_m);
            let mut before = f32::INFINITY;
            let mut looked = Vec::new();
            let mut now = 400.0;
            while now > 0.0 {
                if due(reach, now, before) {
                    looked.push(now);
                }
                if [reach, seen_m].iter().any(|r| due(*r, now, before)) {
                    before = now;
                }
                now -= 4.5 * 0.25;
            }
            assert_eq!(
                looked.len(),
                1,
                "drawn {seen_m} m: looked into at {looked:?}"
            );
            // Every spot of the cell, as far as a cell's corner from its middle, is past
            // where the kind is drawn and its fade.
            let nearest_spot = looked[0] - CELL_M * 3f32.sqrt() / 2.0;
            assert!(
                nearest_spot > seen_m + 4.0,
                "drawn {seen_m} m, put out from {nearest_spot} m"
            );
            assert!(looked[0] >= POPULATE_M - 1.2, "nearer than every kind is");
        }
    }

    #[test]
    fn a_cell_of_air_or_rock_is_never_looked_into_again() {
        for reach in [POPULATE_M, 180.0, 240.0] {
            assert!(!due(reach, 10.0, -1.0));
        }
    }
}
