use super::*;
use crate::{Field, GeneratedPlanet, chunk_at, chunk_origin, chunk_size_m, may_hold_surface};
use terra_core::PlanetSpec;

const RADIUS_M: f32 = 2000.0;

fn planet() -> GeneratedPlanet {
    let spec = PlanetSpec {
        seed: Seed(11),
        radius_m: RADIUS_M,
    };
    GeneratedPlanet::new(spec, 40.0, None)
}

/// A den under the land's top: a chamber 30 m down, and a passage up to the surface.
fn den(planet: &GeneratedPlanet) -> (System, Vec3, Vec3) {
    let up = Vec3::Y;
    let surface = up * (RADIUS_M + planet.height_m(up));
    let chamber = surface - up * 30.0;
    let mouth = surface + Vec3::X * 40.0;
    let mouth = mouth.normalize() * (RADIUS_M + planet.height_m(mouth.normalize()));
    let parts = vec![
        Part::Chamber {
            middle: chamber,
            across_m: 24.0,
            high_m: 12.0,
            floor_m: 4.0,
        },
        Part::Passage {
            from: chamber + Vec3::X * 8.0,
            to: mouth,
            radius_m: 3.0,
        },
    ];
    (System::new(parts, Seed(5)), chamber, mouth)
}

#[test]
fn a_cave_is_open_inside_and_the_rock_round_it_is_as_it_was() {
    let bare = planet();
    let (system, chamber, mouth) = den(&bare);
    let carved = planet().with_caves(Caves::new(vec![system]));
    let at = |planet: &GeneratedPlanet, point: Vec3| planet.sample(point).distance;
    // The chamber's middle and the passage's are rock on the bare planet and open here.
    for open in [chamber, (chamber + Vec3::X * 8.0).lerp(mouth, 0.5)] {
        assert!(at(&bare, open) < -2.0, "the bare planet is rock at {open}");
        assert!(at(&carved, open) > 1.0, "the cave is not open at {open}");
    }
    // Below the chamber's floor it is rock, where the spheroid alone would be open.
    assert!(at(&carved, chamber - Vec3::Y * 5.0) < 0.0, "no floor");
    // Rock well away from the cave, and the air over the land, are as they were.
    for same in [
        chamber - Vec3::Y * 40.0,
        chamber + Vec3::Z * 60.0,
        chamber + Vec3::Y * 80.0,
    ] {
        assert_eq!(at(&carved, same), at(&bare, same), "changed at {same}");
    }
    // The mouth is open to the air: along the passage the ground never closes.
    for step in 0..=20 {
        let point = (chamber + Vec3::X * 8.0).lerp(mouth, step as f32 / 20.0);
        assert!(
            at(&carved, point) > 0.0,
            "the passage is shut at step {step}"
        );
    }
}

/// The streaming meshes a chunk only where the ground's distance at its middle says a
/// surface may pass (`may_hold_surface`): with caves carved in, every chunk a cave's wall
/// passes through is still one of them, and the rock far from any cave is still skipped.
#[test]
fn the_chunk_rule_finds_every_chunk_a_caves_wall_passes_through() {
    let (system, chamber, _) = den(&planet());
    let (middle, reach) = system.ball();
    let carved = planet().with_caves(Caves::new(vec![system]));
    let side = chunk_size_m(0);
    let span = (reach / side).ceil() as i32 + 1;
    let home = chunk_at(middle, 0);
    let (mut walls, mut skipped) = (0, 0);
    for x in -span..=span {
        for y in -span..=span {
            for z in -span..=span {
                let id = terra_core::ChunkId {
                    level: 0,
                    coord: home.coord + glam::IVec3::new(x, y, z),
                };
                // Whether a surface passes through it, by its corners and a grid within.
                let origin = chunk_origin(id);
                let mut signs = (false, false);
                for i in 0..=4 {
                    for j in 0..=4 {
                        for k in 0..=4 {
                            let point =
                                origin + Vec3::new(i as f32, j as f32, k as f32) * side / 4.0;
                            if carved.sample(point).distance < 0.0 {
                                signs.0 = true;
                            } else {
                                signs.1 = true;
                            }
                        }
                    }
                }
                let held = may_hold_surface(&carved, id);
                if signs.0 && signs.1 {
                    walls += 1;
                    assert!(
                        held,
                        "a wall passes through chunk {:?}, which is skipped",
                        id.coord
                    );
                } else if !held {
                    skipped += 1;
                }
            }
        }
    }
    assert!(
        walls > 8,
        "only {walls} chunks hold a wall: the test looked at no cave"
    );
    assert!(
        skipped > walls,
        "{skipped} chunks skipped of the rock and air round the cave"
    );
    // And the chamber's own chunk, deep in what was solid rock, is one that is meshed.
    assert!(may_hold_surface(&carved, chunk_at(chamber, 0)));
    assert!(!may_hold_surface(&planet(), chunk_at(chamber, 0)));
}

#[test]
fn ground_seen_coarsely_has_no_caves() {
    let (system, chamber, _) = den(&planet());
    let carved = planet().with_caves(Caves::new(vec![system]));
    let mut fine = carved.memo_for(0.5);
    let mut coarse = carved.memo_for(8.0);
    assert!(carved.distance_in(chamber, &mut fine) > 0.0);
    assert!(carved.distance_in(chamber, &mut coarse) < 0.0);
}

/// A reading, not a pass: what the carve costs a chunk's worth of asks far from any cave
/// and inside one (`cargo test -p terra_voxel caves -- --ignored --nocapture`).
#[test]
#[ignore = "a timing, read by hand"]
fn what_the_carve_costs() {
    let bare = planet();
    let (system, chamber, _) = den(&bare);
    let many: Vec<System> = (0..32)
        .map(|at| {
            let shift = Vec3::new(at as f32 * 90.0, 0.0, 0.0);
            let parts = system
                .parts()
                .iter()
                .map(|part| match *part {
                    Part::Chamber {
                        middle,
                        across_m,
                        high_m,
                        floor_m,
                    } => Part::Chamber {
                        middle: middle + shift,
                        across_m,
                        high_m,
                        floor_m,
                    },
                    Part::Passage { from, to, radius_m } => Part::Passage {
                        from: from + shift,
                        to: to + shift,
                        radius_m,
                    },
                })
                .collect();
            System::new(parts, Seed(at))
        })
        .collect();
    let carved = planet().with_caves(Caves::new(many));
    let time = |planet: &GeneratedPlanet, middle: Vec3| {
        let mut memo = planet.memo_for(0.5);
        let began = std::time::Instant::now();
        let mut sum = 0.0;
        for i in 0..33 {
            for j in 0..33 {
                for k in 0..33 {
                    let point = middle + Vec3::new(i as f32, j as f32, k as f32) * 0.5;
                    sum += planet.distance_in(point, &mut memo);
                }
            }
        }
        (began.elapsed().as_secs_f64() * 1.0e6, sum)
    };
    let far = chamber - Vec3::Z * 400.0 + Vec3::Y * 20.0;
    for (name, at) in [("far from any cave", far), ("inside a chamber", chamber)] {
        let (bare_us, _) = time(&bare, at);
        let (carved_us, _) = time(&carved, at);
        println!(
            "a chunk's 35,937 asks {name}: {bare_us:.0} us bare, {carved_us:.0} us with 32 systems"
        );
    }
}
