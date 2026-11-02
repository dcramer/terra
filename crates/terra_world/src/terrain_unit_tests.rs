use super::*;

/// How long the lake's load takes by stage, on the CPU alone: the land, choosing the
/// first view's chunks, shaping and colouring them, each on every core as the game does.
/// A measure, not a check: `cargo xtask remote-render --cpu -- cargo test --release -p
/// terra_world --lib lake_load_by_stage -- --ignored --nocapture`.
#[test]
#[ignore]
fn lake_load_by_stage() {
    let args = [
        "--experiment",
        "play",
        "--radius",
        "30000",
        "--site",
        "lake@-15.34,1.68",
        "--date",
        "06-10",
        "--time",
        "17",
    ];
    let options = crate::shell::Options::from_args(args.iter().map(|a| a.to_string())).unwrap();
    let spec = crate::planet::entering::first_world(&options);
    let at = std::time::Instant::now();
    let (planet, start, _) = crate::planet::entering::land_of(&spec, &options);
    eprintln!("land worked out in {:.2} s", at.elapsed().as_secs_f32());
    let viewer = start.0.translation + start.0.translation.normalize() * 1.7;
    let at = std::time::Instant::now();
    let wanted = wanted(&planet, viewer, 1.0);
    eprintln!(
        "chose {} chunks in {:.2} s",
        wanted.len(),
        at.elapsed().as_secs_f32()
    );
    let chunks: Vec<(ChunkId, Sides)> = wanted.iter().map(|(&id, &s)| (id, s)).collect();
    let at = std::time::Instant::now();
    let shaped = terra_gen::par::map(&chunks, |&(id, sides)| {
        (id, mesh_chunk(&**planet.voxels(), id, sides))
    });
    eprintln!("shaped in {:.2} s", at.elapsed().as_secs_f32());
    let at = std::time::Instant::now();
    let coloured = terra_gen::par::map(&shaped, |(id, chunk)| {
        to_mesh(&planet, *id, chunk.clone()).is_some()
    });
    eprintln!(
        "coloured {} in {:.2} s",
        coloured.len(),
        at.elapsed().as_secs_f32()
    );
}

#[test]
fn a_couloirs_key_deepens_into_its_bed_not_across_the_flank() {
    // On a wall the snow's key is the bed's depth: the flank a quarter of the way down keys a
    // quarter of what the bottom does, where `SNOW_BED`'s fade already counts it near whole,
    // so a threshold on the key lays the snow in the bottom alone and the ribs stand bare.
    let flank = couloir_key(0.0, 0.25, 1.0);
    let bottom = couloir_key(0.0, 1.0, 1.0);
    assert!((bottom - 1.0).abs() < 1e-6 && (flank - 0.25).abs() < 1e-6);
    assert!(smoothstep(SNOW_BED.0, SNOW_BED.1, 0.25) > 0.4);
    // None on a rib, none where the slope above gathers nothing, none on level ground.
    assert_eq!(couloir_key(0.0, 0.0, 1.0), 0.0);
    assert_eq!(couloir_key(0.0, 1.0, 0.0), 0.0);
    assert_eq!(couloir_key(1.0, 1.0, 1.0), 0.0);
}

#[test]
fn a_gully_bed_keeps_snow_under_the_open_faces_reach() {
    // A foothill wall too low for the open feed: its gully's bed holds snow, its rib none.
    assert!(fed_lying(0.0, 0.3, 0.2, 1.0) > 0.2);
    assert_eq!(fed_lying(0.0, 0.3, 0.2, 0.0), 0.0);
    // Half down into the bed, half the gully's depth.
    assert!((fed_lying(0.0, 0.4, 0.0, 0.5) - 0.2).abs() < 1e-6);
}

#[test]
fn a_low_walls_benches_hold_snow_by_their_gullies_not_along_them() {
    // A stepped foothill wall facing +z, its gullies 20 m wide every 120 m along x, cut
    // down through every bench; two benches 60 m apart in height. Along each bench's
    // contour (one height) its open feed is laid as `lying` lays it.
    let bed = |at: Vec3| {
        let off = (at.x.rem_euclid(120.0) - 60.0).abs();
        (1.0 - off / 10.0).clamp(0.0, 1.0)
    };
    let fed = 0.5;
    let along = |height: f32| -> Vec<bool> {
        (0..480)
            .map(|x| {
                let point = Vec3::new(x as f32, height, 0.0);
                let mut bed = bed;
                let here = smoothstep(SNOW_BED.0, SNOW_BED.1, bed(point));
                let near = here.max(gully_by(point, Vec3::Y, &mut bed));
                fed_lying(fed * bench_keep(1.0, near), 0.0, 1.0, here) > fed * 0.5
            })
            .collect()
    };
    let (upper, lower) = (along(60.0), along(0.0));
    // Not a band: a share of each bench is snow, in runs no longer than a gully's fan.
    for bench in [&upper, &lower] {
        let share = bench.iter().filter(|&&s| s).count() as f32 / bench.len() as f32;
        assert!(share > 0.15 && share < 0.7, "{share} of the bench snowed");
        let longest = bench.split(|&s| !s).map(<[bool]>::len).max().unwrap_or(0);
        assert!(longest < 100, "a run of {longest} m along the bench");
    }
    // Lined up down the fall line: the benches' snow lies under the same gullies.
    let both = upper.iter().zip(&lower).filter(|(a, b)| **a && **b).count();
    let either = upper.iter().zip(&lower).filter(|(a, b)| **a || **b).count();
    assert!(
        both * 10 >= either * 9,
        "{both} of {either} metres lined up"
    );
    // Over the low walls' reach a bench keeps its whole feed (D425's horns unchanged).
    assert_eq!(bench_keep(0.0, 0.0), 1.0);
}

#[test]
fn open_faces_and_ledges_keep_their_own_feed() {
    // Off a gully's bed a wall lies as the open feed lays it, however deep the gully feed:
    // the horns stay two thirds rock (D425).
    for level in [0.0, 0.3, 1.0] {
        assert_eq!(fed_lying(0.25, 2.0, level, 0.0), 0.25 * level);
    }
    // In a bed never less than the open feed gives it.
    assert!(fed_lying(0.5, 0.1, 0.0, 1.0) >= 0.5);
    // The gully feed is from higher on the same snow line, so never shallower where both lie.
    const { assert!(FED_GULLY_M > FED_M) };
}

#[test]
fn a_low_wall_is_fed_deeper_yet_snow_still_deepens_upward() {
    // The foothills fed as round 24's whole range, the horns as D425's two thirds rock.
    assert_eq!(fed_from_m(800.0), FED_GULLY_M);
    assert_eq!(fed_from_m(3200.0), FED_M);
    // The height a wall is fed snow as rises with its own, so no snow line runs backwards
    // (the r3 review): every metre higher is fed from higher.
    let mut last = f32::MIN;
    for step in 0..=500 {
        let height = step as f32 * 10.0;
        let fed_as = height + fed_from_m(height);
        assert!(
            fed_as > last,
            "fed as {fed_as} m at {height} m, under {last}"
        );
        last = fed_as;
    }
}
