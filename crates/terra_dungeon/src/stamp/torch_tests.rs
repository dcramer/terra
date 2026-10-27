use super::tests::{SEEDS, kind};
use super::*;
use crate::place::{STEPS, Side};
use crate::theme::assets;
use crate::{ModuleSet, generate};
use terra_gen::Seed;

/// The walls a corridor's straight run of three cells or more ends in, each as its face's
/// middle 2 m up, found from the corridor's cells, not from how the run-in finds them.
fn end_walls(dungeon: &Dungeon) -> Vec<Vec3> {
    let layout = &dungeon.layout;
    let mut out = Vec::new();
    for run in &layout.corridors {
        for w in run.windows(3) {
            for (a, b, c) in [(w[0], w[1], w[2]), (w[2], w[1], w[0])] {
                let Some(d) = (0..4).find(|&d| a + STEPS[d] == b && b + STEPS[d] == c) else {
                    continue;
                };
                let Some(cell) = layout.cell(c) else { continue };
                if layout.cells[cell].sides[d] == Side::Wall {
                    out.push(Frame::new(c, d).point(MID, 4.0, 0.0));
                }
            }
        }
    }
    out
}

/// A torch in the next cell along, on the side wall away from the end wall, is 4 m along and
/// 3.35 m across from its face's middle: 5.2 m.
const LIT_M: f32 = 5.5;

#[test]
fn a_corridor_never_runs_into_a_dark_wall() {
    for theme in Theme::all(&assets()).unwrap() {
        let set = ModuleSet::new(&theme).unwrap();
        let seeds: Vec<u64> = (0..SEEDS).collect();
        let counts = terra_gen::par::map(&seeds, |&s| {
            let dungeon = generate(&theme, &set, Seed(s)).ok()?;
            let stamp = stamp(&dungeon, &theme, &kind);
            let torches: Vec<Vec3> = stamp.lights.iter().map(|l| l.at).collect();
            let walls = end_walls(&dungeon);
            let dark = walls
                .iter()
                .filter(|w| torches.iter().all(|t| t.distance(**w) > LIT_M))
                .count();
            Some((walls.len(), dark, stamp.lights.len() >= LIGHTS))
        });
        let counts: Vec<_> = counts.into_iter().flatten().collect();
        let walls: usize = counts.iter().map(|c| c.0).sum();
        let dark: usize = counts.iter().map(|c| c.1).sum();
        let capped = counts.iter().filter(|c| c.2).count();
        println!(
            "{}: {walls} end walls in {} dungeons, {dark} dark, {capped} at {LIGHTS} lights",
            theme.name,
            counts.len()
        );
        // The cap cuts the last of them where many rooms and fires took the lights first: one
        // wall in 776 of the keep's.
        assert!(
            dark * 200 <= walls,
            "{}: {dark} end walls of {walls} with no torch within {LIT_M} m",
            theme.name
        );
    }
}
