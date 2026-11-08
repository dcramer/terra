use super::*;
use crate::{ground, start_climate};
use terra_core::{PlanetSpec, Seed};
use terra_voxel::{Caves, Field};

fn land(seed: u64) -> Land {
    Land::new(
        ground::world(PlanetSpec {
            seed: Seed(seed),
            radius_m: 30_000.0,
        }),
        start_climate().unwrap(),
    )
}

/// On eight worlds: a den is found from the seed, the same each time, with no fault; carved
/// from the ground it is open from the air over its mouth all the way to its chamber, with
/// room over the floor for a 4 m body, and the worlds' dens differ.
#[test]
fn a_den_is_found_from_the_seed_open_from_its_mouth_to_a_chamber_a_troll_fits() {
    let seeds: Vec<u64> = (2..10).collect();
    let found = terra_gen::par::map(&seeds, |&seed| {
        let land = land(seed);
        // The planet's own start (the seed's searched one takes seconds; the sweep holds it).
        let start = crate::PLANET_START;
        let den = find(&land, start).unwrap_or_else(|| panic!("seed {seed} has no den"));
        assert_eq!(
            faults(&land, start, &den),
            Vec::<String>::new(),
            "seed {seed}"
        );
        assert_eq!(find(&land, start).as_ref(), Some(&den), "seed {seed}");
        let carved = land
            .ground
            .clone()
            .with_caves(Caves::new(vec![den.system(&land)]));
        let open = |point: Vec3| carved.sample(point).distance > 0.0;
        // Along the passages' middles, from over the ground outside to the chamber.
        for part in &den.parts {
            if let Part::Passage { from, to, .. } = *part {
                for step in 0..=10 {
                    let point = from.lerp(to, step as f32 / 10.0);
                    assert!(open(point), "seed {seed}: the way in is shut at {point}");
                }
            }
        }
        // Over the floor's middle: open for a troll's height and more, rock under it, and
        // the bare land solid there.
        let up = den.floor.normalize();
        for high in [1.5, 2.5, 4.5] {
            assert!(
                open(den.floor + up * high),
                "seed {seed}: no room {high} m up"
            );
        }
        assert!(!open(den.floor - up * 2.0), "seed {seed}: no floor");
        assert!(land.ground.sample(den.floor + up * 2.5).distance < -COVER_M);
        // A clear floor 12 m across (caves.md): open 6 m out from its middle, a man's height up.
        let out = up.any_orthonormal_vector();
        for way in [out, -out, up.cross(out), -up.cross(out)] {
            assert!(
                open(den.floor + way * 6.0 + up * 1.8),
                "seed {seed}: the floor is narrow"
            );
        }
        (
            den.layout,
            (den.mouth - start * den.mouth.length()).length() as i32,
        )
    });
    let places: std::collections::HashSet<i32> = found.iter().map(|(_, at)| *at).collect();
    assert!(
        places.len() >= 6,
        "eight worlds' dens are in {} places",
        places.len()
    );
    let layouts: std::collections::HashSet<_> = found.iter().map(|(layout, _)| *layout).collect();
    assert!(
        layouts.len() >= 4,
        "eight worlds' dens have {} layouts",
        layouts.len()
    );
}

/// The planted faults (D632): a den moved under thin ground, one at the start, and one off
/// the land searched are each named.
#[test]
fn a_den_under_thin_rock_too_near_or_out_of_reach_is_a_fault() {
    let land = land(2);
    let start = crate::PLANET_START;
    let den = find(&land, start).expect("a den");
    // Lifted 8 m: its roof is thin or gone.
    let up = den.floor.normalize();
    let lift = |part: &Part| match *part {
        Part::Passage { from, to, radius_m } => Part::Passage {
            from: from + up * 8.0,
            to: to + up * 8.0,
            radius_m,
        },
        Part::Chamber {
            middle,
            across_m,
            high_m,
            floor_m,
        } => Part::Chamber {
            middle: middle + up * 8.0,
            across_m,
            high_m,
            floor_m,
        },
    };
    let lifted = Den {
        parts: den.parts.iter().map(lift).collect(),
        ..den.clone()
    };
    let thin = faults(&land, start, &lifted);
    assert!(thin[0].contains("of rock over it"), "{thin:?}");
    // Its mouth at the start itself.
    let near = Den {
        mouth: on_ground(&land, start),
        ..den.clone()
    };
    assert!(faults(&land, start, &near)[0].contains("from the start"));
    // Its mouth off the land searched.
    let off = (start + start.any_orthonormal_vector() * 0.2).normalize();
    let far = Den {
        mouth: on_ground(&land, off),
        ..den
    };
    assert!(faults(&land, start, &far)[0].contains("off the land searched"));
}

#[test]
fn the_27_layouts_are_each_picked() {
    let layouts: std::collections::HashSet<_> = (0..27).map(Layout::of).collect();
    assert_eq!(layouts.len(), 27);
}

#[test]
#[ignore = "a reading: why a seed has no den"]
fn why_no_den() {
    let land = land(9);
    let start = crate::PLANET_START;
    let (grid, cell) = grid(&land, start);
    let n = grid.region.cells;
    let reached = grid.reached(cell, n);
    let gate = crate::gate::find(&land, start);
    let mut why = std::collections::BTreeMap::<String, usize>::new();
    let (mut may, mut slopes, mut over) = (0, Vec::new(), Vec::new());
    for k in 0..n * n {
        let at = (k % n, k / n);
        match wrong_mouth(&land, &grid, &reached, gate, at) {
            Some(reason) => {
                *why.entry(reason.split(' ').take(4).collect::<Vec<_>>().join(" "))
                    .or_default() += 1
            }
            None => {
                may += 1;
                slopes.push(grid.region.slope_deg(at.0, at.1) as i32);
                over.push(
                    (grid.region.heights[k] - (land.sea_radius_m() - land.radius_m())) as i32,
                );
                let mouth = on_ground(&land, grid.region.direction(at.0, at.1));
                let den = shape(mouth, grid.region.east, Layout::of(0), 1.0);
                let reason = wrong_shape(&land, &den).unwrap_or_else(|| "fine".into());
                *why.entry(format!(
                    "shape: {}",
                    reason.split(' ').take(3).collect::<Vec<_>>().join(" ")
                ))
                .or_default() += 1;
            }
        }
    }
    slopes.sort();
    over.sort();
    println!(
        "WHY {may} may; slopes {:?}..{:?}; over sea {:?}..{:?}; {why:?}",
        slopes.first(),
        slopes.last(),
        over.first(),
        over.last()
    );
}
