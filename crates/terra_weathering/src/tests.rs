use super::*;
use crate::masks::{Context, Wants};
use crate::motifs::{BANNED_BELOW, LAYERS as MOTIFS};
use crate::wfc::{MAX_SYMBOLS, Model};

/// A grid's symbols as chars, rows from the top.
fn solve(
    model: &Model,
    (w, h): (usize, usize),
    weigh: &dyn Fn(usize, usize) -> [f32; MAX_SYMBOLS],
    seed: u32,
) -> Option<Vec<char>> {
    let (out, _) = model.solve((w, h), &|_, _| true, weigh, seed, 10);
    out.map(|o| o.iter().map(|&s| model.symbols[s as usize]).collect())
}

/// Every symbol weighed 1, but `c` weighed `k`.
fn all_but(model: &Model, c: char, k: f32) -> [f32; MAX_SYMBOLS] {
    std::array::from_fn(|i| {
        if model.symbols.get(i) == Some(&c) {
            k
        } else {
            1.0
        }
    })
}

/// The sizes of the 8-connected groups of `c` in a grid.
fn groups(grid: &[char], w: usize, h: usize, c: char) -> Vec<usize> {
    raster::groups(grid, w, h, c).iter().map(Vec::len).collect()
}

#[test]
fn cracks_run_on_as_lines_never_as_specks() {
    let model = MOTIFS[0].model();
    let (w, h) = (48, 48);
    let (mut solved, mut kept) = (0, 0);
    for seed in 0..6 {
        let mut grid =
            solve(&model, (w, h), &|_, _| all_but(&model, 'c', 0.5), seed).expect("solved");
        solved += groups(&grid, w, h, 'c').iter().sum::<usize>();
        raster::prune(&mut grid, w, h, 'c', raster::MIN_CRACK);
        let sizes = groups(&grid, w, h, 'c');
        assert!(
            sizes.iter().all(|&s| s >= raster::MIN_CRACK),
            "a speck kept: {sizes:?}"
        );
        kept += sizes.iter().sum::<usize>();
    }
    eprintln!(
        "cracks: {kept} of {solved} solved pixels in lines of {} or more",
        raster::MIN_CRACK
    );
    assert!(kept > 50, "too few cracks to judge: {kept} pixels");
    // The solve makes lines, the pruning only trims where two crack ends met.
    assert!(
        kept * 3 >= solved,
        "only {kept} of {solved} crack pixels in lines"
    );
}

#[test]
fn moss_grows_only_where_the_mask_allows() {
    let model = MOTIFS[2].model();
    let (w, h) = (40, 30);
    let banned = |x: usize| x >= w / 2;
    let weigh = |x: usize, _y: usize| all_but(&model, 'm', if banned(x) { 0.0 } else { 3.0 });
    for seed in 0..4 {
        let grid = solve(&model, (w, h), &weigh, seed).expect("solved");
        let moss: Vec<usize> = (0..w * h).filter(|&i| grid[i] == 'm').collect();
        assert!(!moss.is_empty(), "no moss where it is allowed");
        assert!(
            moss.iter().all(|&i| !banned(i % w)),
            "moss where its mask is 0"
        );
    }
}

#[test]
fn moss_spreads_in_patches_not_noise() {
    // Moss everywhere allowed: its pixels should mostly sit in large groups, as a patch's
    // do, where a mask cut by white noise would scatter them.
    let model = MOTIFS[2].model();
    let (w, h) = (40, 40);
    let grid = solve(&model, (w, h), &|_, _| all_but(&model, 'm', 1.0), 3).expect("solved");
    let sizes = groups(&grid, w, h, 'm');
    let total: usize = sizes.iter().sum();
    let big: usize = sizes.iter().filter(|&&s| s >= 4).sum();
    assert!(
        total > 20 && big * 10 >= total * 8,
        "{big} of {total} moss pixels in patches of 4 or more"
    );
}

#[test]
fn every_layer_solves_nearly_always_with_bans() {
    // Stripes where each motif is banned, as masks make them: the solver still finishes.
    let (w, h) = (36, 36);
    let mut solved = 0;
    let mut runs = 0;
    for layer in &MOTIFS {
        let model = layer.model();
        for seed in 0..5 {
            let weigh = |x: usize, y: usize| {
                std::array::from_fn(|i| match model.symbols.get(i) {
                    Some('.') | None => 1.0,
                    Some(_) => {
                        if (x / 6 + y / 9).is_multiple_of(2) {
                            0.0
                        } else {
                            1.0
                        }
                    }
                })
            };
            runs += 1;
            solved += usize::from(solve(&model, (w, h), &weigh, seed).is_some());
        }
    }
    assert!(solved * 20 >= runs * 19, "{solved} of {runs} solved");
}

/// A stone room 8 by 6 cubes and 6 high, a window in its west wall two cubes up and a
/// roof over it.
fn house(at: IVec3) -> Stuff {
    let wall = (at.x == 0 || at.x == 7 || at.z == 0 || at.z == 5)
        && (0..=7).contains(&at.x)
        && (0..=5).contains(&at.z);
    let window = at.x == 0 && (2..=3).contains(&at.z) && (2..=3).contains(&at.y);
    if at.y == 6 && (0..=7).contains(&at.x) && (0..=5).contains(&at.z) {
        Stuff::Roof
    } else if wall && (0..=5).contains(&at.y) && !window {
        Stuff::Stone
    } else {
        Stuff::Air
    }
}

#[test]
fn the_masks_follow_the_shape() {
    let west = &FACES[1];
    let at = |x: i32, y: i32, z: i32, fu: f32, fv: f32| {
        Context::of(&house, west, IVec3::new(x, y, z), 0).at(fu, fv)
    };
    // Under the window's sill streaks run; beside it, none.
    assert!(at(0, 1, 2, 0.5, 0.5).below > 0.5);
    assert_eq!(at(0, 1, 4, 0.5, 0.5).below, 0.0);
    // Over the window's head, soot and loosened stones.
    assert!(at(0, 4, 2, 0.5, 0.2).above > 0.5);
    assert_eq!(at(0, 4, 4, 0.5, 0.2).above, 0.0);
    // The wall's foot is near the ground, its top is not.
    assert!(at(0, 0, 4, 0.5, 0.1).ground > 0.8);
    assert_eq!(at(0, 5, 4, 0.5, 0.5).ground, 0.0);
    // At the house's corner (z = 0, the face's back edge along u) the edge is worn; in the
    // middle of the face it is not.
    assert!(at(0, 1, 0, 0.02, 0.5).edge > 0.8);
    assert_eq!(at(0, 1, 1, 0.5, 0.5).edge, 0.0);
    // The window's jamb is an outside edge too.
    assert!(at(0, 2, 1, 0.98, 0.5).edge > 0.8);
    // The roof's top looks up; under the roof the wall is sheltered.
    let roof = Context::of(&house, &FACES[2], IVec3::new(3, 6, 3), 0).at(0.5, 0.5);
    assert_eq!(roof.up, 1.0);
    assert!(
        at(0, 1, 1, 0.5, 0.5).sheltered < 0.5,
        "nothing stands over the air in front of the wall"
    );
}

#[test]
fn a_young_building_has_no_cracks_and_an_old_one_may() {
    let m = Context::of(&house, &FACES[1], IVec3::new(0, 4, 2), 0).at(0.5, 0.3);
    assert!(Wants::of(&m, Stuff::Stone, 1.0).crack < BANNED_BELOW);
    assert!(Wants::of(&m, Stuff::Stone, 60.0).crack > 0.5);
    // Timber never cracks or loses stones.
    let timber = Wants::of(&m, Stuff::Timber, 60.0);
    assert_eq!((timber.crack, timber.loss), (0.0, 0.0));
}

#[test]
fn a_house_weathered_has_a_map_for_every_exposed_face() {
    let (low, high) = (IVec3::new(-1, 0, -1), IVec3::new(8, 7, 6));
    for mode in [Mode::Masks, Mode::Wfc] {
        let out = weather(
            &house,
            low,
            high,
            &Settings {
                mode,
                age: 50.0,
                seed: 7,
                tries: 10,
            },
        );
        // Counted another way: each solid cube's faces with air in front, but undersides.
        let mut exposed = 0;
        for z in low.z..=high.z {
            for y in low.y..=high.y {
                for x in low.x..=high.x {
                    let at = IVec3::new(x, y, z);
                    if house(at).solid() {
                        exposed += FACES
                            .iter()
                            .filter(|f| f.normal.y >= 0 && !house(at + f.normal).solid())
                            .count();
                    }
                }
            }
        }
        assert_eq!(out.slots as usize, exposed);
        let mut slots: Vec<u32> = out.lookup.iter().copied().filter(|&s| s > 0).collect();
        slots.sort_unstable();
        slots.dedup();
        assert_eq!(slots.len(), exposed, "each face its own map");
        assert_eq!(out.atlas[0].len(), out.side() * out.side() * 4);
        if mode == Mode::Wfc {
            assert_eq!(out.stats.solved, out.stats.solves, "{:?}", out.stats);
        }
    }
}
