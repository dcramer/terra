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

/// How high a gabled house's wall stands in its column `z`: 4 cubes at the eaves (z = 0 and
/// 9), stepping up a cube every two toward the ridge.
fn gable_top(z: i32) -> i32 {
    3 + z.min(9 - z).max(0) / 2
}

/// A stone house 8 by 10 cubes, its eave walls (z = 0 and 9) 4 high, its gable walls (x = 0
/// and 7) stepping up under a roof that overhangs every wall by a cube (verges and eaves), a
/// window in the west gable two cubes up.
fn gabled(at: IVec3) -> Stuff {
    let (x, y, z) = (at.x, at.y, at.z);
    if (-1..=8).contains(&x) && (-1..=10).contains(&z) && y == gable_top(z.clamp(0, 9)) + 1 {
        return Stuff::Roof;
    }
    let walls =
        (x == 0 || x == 7 || z == 0 || z == 9) && (0..=7).contains(&x) && (0..=9).contains(&z);
    let window = x == 0 && (4..=5).contains(&z) && (1..=2).contains(&y);
    if walls && (0..=gable_top(z)).contains(&y) && !window {
        Stuff::Stone
    } else {
        Stuff::Air
    }
}

#[test]
fn streaks_start_at_a_sill_or_a_level_wall_head_never_under_a_gable_verge() {
    let below = |face: usize, at: IVec3, fv: f32| {
        Context::of(&gabled, &FACES[face], at, 0).at(0.5, fv).below
    };
    // The west gable (face -x, sheltered by its verge): under its window's sill, streaks; up its
    // stepped head, under the verge, none.
    assert!(below(1, IVec3::new(0, 0, 4), 0.5) > 0.2);
    for z in 0..=9 {
        for y in 3..=gable_top(z) {
            for fv in [0.1, 0.5, 0.9] {
                let b = below(1, IVec3::new(0, y, z), fv);
                assert_eq!(
                    b, 0.0,
                    "a streak under the gable's verge at z {z}, y {y}: {b}"
                );
            }
        }
    }
    // Beside the window, with no sill over it, none.
    assert_eq!(below(1, IVec3::new(0, 1, 2), 0.5), 0.0);
    // The eave wall (face -z) ends level under the roof, so streaks run down from its head,
    // along all of it.
    for x in 0..=7 {
        assert!(
            below(5, IVec3::new(x, 3, 0), 0.5) > 0.0,
            "none under the eave at x {x}"
        );
    }
}

#[test]
fn a_streak_runs_down_from_its_source_never_from_mid_wall() {
    // A 3 by 8 grid, streaks allowed from row 2 down: a run begun at row 4 is carried up to
    // row 2; one under soot is cleared; one already at the top is kept as it is.
    let (w, h) = (3, 8);
    let rows = ["...", "...", "..w", ".k.", "ww.", "ww.", ".w.", "..."];
    let mut grid: Vec<char> = rows.concat().chars().collect();
    raster::from_the_source(&mut grid, w, h, &|_, y| y >= 2);
    let column = |x: usize| (0..h).map(|y| grid[y * w + x]).collect::<String>();
    assert_eq!(column(0), "..wwww..");
    assert_eq!(column(1), "...k....");
    assert_eq!(column(2), "..w.....");
}

/// The streak layer's texels along a row of cubes of one face, left to right, `row` texels
/// down from their top.
fn streak_row(out: &Weathered, face: usize, cubes: &[IVec3], row: usize) -> Vec<u8> {
    let across = out.dims.x as usize * 6;
    let per_row = out.per_row as usize;
    let mut texels = Vec::new();
    for at in cubes {
        let rel = *at - out.low;
        let slot =
            out.lookup[(rel.z * out.dims.y + rel.y) as usize * across + rel.x as usize * 6 + face];
        assert!(slot > 0, "no map for {at} face {face}");
        let s = slot as usize - 1;
        let (sx, sy) = (s % per_row, s / per_row);
        for col in 0..TILE {
            let i = ((sy * TILE + row) * out.side() + sx * TILE + col) * 4 + 3;
            texels.push(out.atlas[0][i]);
        }
    }
    texels
}

/// The widths of the bands along a row of texels, where each is over half the row's peak.
fn bands(texels: &[u8]) -> Vec<usize> {
    let peak = u32::from(*texels.iter().max().unwrap_or(&0));
    let mut out = Vec::new();
    let mut run = 0;
    for &t in texels.iter().chain([&0]) {
        if u32::from(t) * 2 > peak {
            run += 1;
        } else if run > 0 {
            out.push(run);
            run = 0;
        }
    }
    out
}

#[test]
fn streaks_are_soft_bands_under_the_eave_and_sill_and_none_under_the_verge() {
    let (low, high) = (IVec3::new(-1, 0, -1), IVec3::new(8, 8, 10));
    let eave: Vec<Vec<IVec3>> = (0..=3)
        .map(|y| (0..=7).map(|x| IVec3::new(x, y, 0)).collect())
        .collect();
    let sill: Vec<IVec3> = (0..=9).map(|z| IVec3::new(0, 0, z)).collect();
    let (mut widths, mut under_verge) = (Vec::new(), 0u32);
    for seed in 0..4 {
        let settings = Settings {
            mode: Mode::Wfc,
            age: 50.0,
            seed,
            tries: 10,
        };
        let out = weather(&gabled, low, high, &settings);
        // The eave wall's rows (face -z, along x) and the gable's under its window.
        let rows = eave.iter().map(|c| (5, c)).chain([(1, &sill)]);
        for (face, cubes) in rows {
            for row in 0..TILE {
                let texels = streak_row(&out, face, cubes, row);
                if texels.iter().any(|&t| t >= 64) {
                    widths.extend(bands(&texels));
                }
            }
        }
        // Nothing on the gable over the window's head, under the verge.
        for z in 0..=9 {
            for y in 3..=gable_top(z) {
                under_verge += (0..TILE)
                    .flat_map(|row| streak_row(&out, 1, &[IVec3::new(0, y, z)], row))
                    .map(u32::from)
                    .sum::<u32>();
            }
        }
    }
    assert_eq!(under_verge, 0, "streaks under the gable's verge");
    assert!(
        widths.len() > 20,
        "too few streaks to judge: {}",
        widths.len()
    );
    widths.sort_unstable();
    let median = widths[widths.len() / 2];
    eprintln!(
        "streaks: median {median} texels (1.6 cm) across, {} bands",
        widths.len()
    );
    // About 10 cm or more across: a band, not a hairline.
    assert!(median >= 6, "streaks a median {median} texels across");
}
