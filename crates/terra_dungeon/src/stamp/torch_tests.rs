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

/// Each stair cell's middle 1 m over its step, from how `shell::steps` lays the steps (a cube
/// up each cube from two cubes in, so the middle of the `i`th cell stands `8 i + 2` cubes up).
fn stair_points(dungeon: &Dungeon) -> Vec<Vec<Vec3>> {
    dungeon
        .layout
        .stairs
        .iter()
        .map(|stair| {
            // The two cells of steps; the cells above are the well and the landing.
            let cells = stair.cells.iter().take(2).enumerate();
            cells
                .map(|(i, &at)| {
                    let step = (CELL as usize * i) as f32 + MID - 2.0 + 0.5;
                    Frame::new(at, 0).point(MID, step + 2.0, MID)
                })
                .collect()
        })
        .collect()
}

#[test]
fn every_stair_is_lit() {
    for theme in Theme::all(&assets()).unwrap() {
        let set = ModuleSet::new(&theme).unwrap();
        let seeds: Vec<u64> = (0..SEEDS).collect();
        let counts = terra_gen::par::map(&seeds, |&s| {
            let dungeon = generate(&theme, &set, Seed(s)).ok()?;
            let stamp = stamp(&dungeon, &theme, &kind);
            let torches: Vec<Vec3> = stamp.lights.iter().map(|l| l.at).collect();
            let stairs = stair_points(&dungeon);
            let dark = stairs
                .iter()
                .filter(|points| {
                    points
                        .iter()
                        .any(|p| torches.iter().all(|t| t.distance(*p) > LIT_M))
                })
                .count();
            Some((stairs.len(), dark))
        });
        let counts: Vec<_> = counts.into_iter().flatten().collect();
        let stairs: usize = counts.iter().map(|c| c.0).sum();
        let dark: usize = counts.iter().map(|c| c.1).sum();
        println!(
            "{}: {stairs} stairs in {} dungeons, {dark} with a step past {LIT_M} m of a torch",
            theme.name,
            counts.len()
        );
        assert_eq!(dark, 0, "{}: {dark} of {stairs} stairs dark", theme.name);
    }
}

/// Whether a point stands in a room's cells, from its cells' boxes: a cell's floor to a
/// level up.
fn in_room(rect: crate::place::Rect, p: Vec3) -> bool {
    rect.cells().any(|at| {
        let frame = Frame::new(at, 0);
        let (a, b) = (
            frame.point(0.0, 0.0, 0.0),
            frame.point(CELL as f32, LEVEL as f32, CELL as f32),
        );
        p.cmpge(a.min(b)).all() && p.cmplt(a.max(b)).all()
    })
}

/// A big room is lit from two places at least, its torches, braziers and fire bowls: the light
/// cap cut the second torch of the crypt's hall and vault (barrow round 13).
#[test]
fn a_big_room_has_two_torches() {
    let mut failed = Vec::new();
    for theme in Theme::all(&assets()).unwrap() {
        let set = ModuleSet::new(&theme).unwrap();
        let seeds: Vec<u64> = (0..SEEDS).collect();
        let counts = terra_gen::par::map(&seeds, |&s| {
            let dungeon = generate(&theme, &set, Seed(s)).ok()?;
            let stamp = stamp(&dungeon, &theme, &kind);
            let torches: Vec<Vec3> = stamp.lights.iter().map(|l| l.at).collect();
            let layout = &dungeon.layout;
            let big: Vec<usize> = (0..layout.rooms.len())
                .filter(|&r| layout.rooms[r].size.element_product() >= runin::BIG_ROOM_CELLS)
                .filter(|&r| dungeon.graph.rooms[r].role != crate::theme::Role::Rest)
                .collect();
            let dim: Vec<usize> = big
                .iter()
                .copied()
                .filter(|&r| {
                    torches
                        .iter()
                        .filter(|&&t| in_room(layout.rooms[r], t))
                        .count()
                        < 2
                })
                .collect();
            for &r in &dim {
                let rect = layout.rooms[r];
                let n = torches.iter().filter(|&&t| in_room(rect, t)).count();
                println!(
                    "{} seed {s}: room {r}, {:?}, {:?}, {n} lights",
                    theme.name, rect.size, dungeon.graph.rooms[r].role
                );
            }
            Some((big.len(), dim.len()))
        });
        let counts: Vec<_> = counts.into_iter().flatten().collect();
        let big: usize = counts.iter().map(|c| c.0).sum();
        let dim: usize = counts.iter().map(|c| c.1).sum();
        println!(
            "{}: {big} rooms of {} cells or more in {} dungeons, {dim} with under two torches",
            theme.name,
            runin::BIG_ROOM_CELLS,
            counts.len()
        );
        if dim > 0 {
            failed.push(format!("{}: {dim} of {big}", theme.name));
        }
    }
    assert!(failed.is_empty(), "big rooms with one torch: {failed:?}");
}
