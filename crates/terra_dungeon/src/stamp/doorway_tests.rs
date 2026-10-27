//! The tour's look leads to the next doorway on the way: crossing a room it does not stare
//! at a blank wall while the way out is in sight (round 5's barrow film, 27-29.5 s: the west
//! wall of a 2x2 room 5-7 m down the look while the tour crossed to its south door).

use super::tests::{SEEDS, kind};
use super::*;
use crate::theme::assets;
use crate::{ModuleSet, generate};
use terra_gen::Seed;

/// A doorway is in reach this far along the way, in metres: past where the look lets it go
/// to pass through (`look::THROUGH_M`) and within where it starts to lead (`look::LEAD_M`).
const REACH_M: [f32; 2] = [3.5, 9.0];
/// A wall this near down the middle of the frame is what the frame shows.
const BLANK_M: f32 = 5.0;

/// Of one stamp's eyes with a doorway in reach and in sight: how many, how many whose look's
/// middle meets a wall within `BLANK_M`, and how many the plain look at the way 4 m on would.
fn blank(theme: &Theme, seed: u64) -> Option<[usize; 3]> {
    use super::view::STEP_M;
    let set = ModuleSet::new(theme).ok()?;
    let dungeon = generate(theme, &set, Seed(seed)).ok()?;
    let stamp = stamp(&dungeon, theme, &kind);
    let map = cubes(&stamp.ops);
    let (view, look) = (&stamp.view, &stamp.look);
    let passes = super::look::passes(view, &stamp.doorways);
    let surface = stamp.route[0].distance(stamp.route[1]);
    let lead = (super::look::AHEAD_M / STEP_M).round() as usize;
    // Whether a ray from the eye meets a wall's side (not a floor or ceiling) within `most`.
    let walled = |from: Vec3, d: Vec3, most: f32| {
        let mut was = cube_of(from);
        for k in 1..=(most / 0.05).round() as i32 {
            let c = cube_of(from + d * (k as f32 * 0.05));
            if !clear(&map, c) {
                return (c - was).y == 0;
            }
            was = c;
        }
        false
    };
    let mut out = [0; 3];
    for i in 0..view.len() {
        if (i as f32) * STEP_M < surface + 1.0 {
            continue;
        }
        let Some(&j) = passes.iter().find(|&&j| j > i) else {
            break;
        };
        let along = (j - i) as f32 * STEP_M;
        let on = view[(i + lead).min(view.len() - 1)] - view[i];
        if !(REACH_M[0]..=REACH_M[1]).contains(&along)
            || on.length() < 0.5
            || !super::look::seen(&map, view[i], view[j])
        {
            continue;
        }
        out[0] += 1;
        out[1] += walled(view[i], (look[i] - view[i]).normalize(), BLANK_M) as usize;
        out[2] += walled(view[i], on.normalize(), BLANK_M) as usize;
    }
    Some(out)
}

/// The look may meet a blank wall this much more often than the plain look at the way 4 m
/// on, a share of looks: that look neither smooths nor bounds its turn, so swings the instant
/// the way does.
const PLAIN_MORE: f32 = 0.008;

#[test]
fn crossing_a_room_the_look_leads_to_the_next_doorway_not_a_blank_wall() {
    let mut faults = Vec::new();
    for theme in Theme::all(&assets()).unwrap() {
        let seeds: Vec<u64> = (0..SEEDS).collect();
        let counts = terra_gen::par::map(&seeds, |&s| blank(&theme, s));
        let sum = |k: usize| counts.iter().flatten().map(|c| c[k]).sum::<usize>() as f32;
        let (reach, blank, plain) = (
            sum(0).max(1.0),
            sum(1) / sum(0).max(1.0),
            sum(2) / sum(0).max(1.0),
        );
        println!(
            "{}: {reach} looks with a doorway {}-{} m on in sight; the frame's middle on a wall within {BLANK_M} m in {:.2}% ({:.2}% looking at the way 4 m on)",
            theme.name,
            REACH_M[0],
            REACH_M[1],
            100.0 * blank,
            100.0 * plain
        );
        // Round 5's barrow film looked at a 2x2 room's blank west wall for 2.5 s crossing to
        // its south door: the look at the way 4 m on, smoothed and turning at most 20° a
        // metre, met a wall in 5.7/4.1/4.3% of looks with a doorway in reach (crypt, mine,
        // keep), 1.5-2.7 points over the plain look's 2.9/1.6/1.8%; led to the doorway,
        // 3.3/2.2/2.0% (round 6).
        if blank > plain + PLAIN_MORE {
            faults.push(format!(
                "{}: {blank} of looks at a blank wall, against {plain}",
                theme.name
            ));
        }
    }
    assert!(faults.is_empty(), "{faults:?}");
}
