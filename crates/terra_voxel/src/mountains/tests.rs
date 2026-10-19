use super::*;

const RADIUS: f32 = 30_000.0;

fn range() -> Range {
    Range::new(Mountains::START, Seed(1))
}

/// Directions spread evenly over the sphere (a Fibonacci lattice).
fn directions(count: usize) -> impl Iterator<Item = Vec3> {
    (0..count).map(move |i| {
        let y = 1.0 - 2.0 * (i as f32 + 0.5) / count as f32;
        let around = i as f32 * 2.399_963;
        let flat = libm::sqrtf(1.0 - y * y);
        Vec3::new(flat * libm::cosf(around), y, flat * libm::sinf(around))
    })
}

#[test]
fn the_range_noise_spreads_as_the_share_assumes() {
    let range = range();
    let n = 20_000;
    let inside = directions(n)
        .filter(|&d| range.inside(d * RADIUS) > 0.5)
        .count() as f32
        / n as f32;
    let share = Mountains::START.share;
    assert!(
        (inside - share).abs() < 0.1,
        "{inside} of the land, not {share}"
    );
}

#[test]
fn ranges_stand_high_and_the_land_out_of_them_is_untouched() {
    let range = range();
    let mut highest: f32 = 0.0;
    for d in directions(20_000) {
        let rise = range.rise(d, RADIUS);
        if range.inside(d * RADIUS) <= 0.0 {
            assert_eq!(rise, Rise::default());
        }
        assert!(rise.height_m <= Mountains::START.highest_m());
        highest = highest.max(rise.height_m);
    }
    let m = Mountains::START;
    assert!(highest > m.base_m + m.relief_m * 0.5, "{highest}");
}

#[test]
fn gullies_only_cut_down_and_their_beds_lie_lowest() {
    let range = range();
    let (mut beds, mut ribs) = ((0.0, 0), (0.0, 0));
    for d in directions(20_000) {
        let ridged = range.ridged(d * RADIUS);
        let rise = range.rise(d, RADIUS);
        assert!(rise.height_m <= ridged + 1e-3, "{rise:?} over {ridged}");
        let cut = ridged - rise.height_m;
        // On a talus apron the ground between the cones is lowered, its gullies filled.
        if range.ridged_upper_and_foot(d * RADIUS).2 > 0.0 {
            continue;
        }
        if rise.bed > 0.7 {
            beds = (beds.0 + cut, beds.1 + 1);
        } else if rise.bed < 0.3 && ridged > 0.0 {
            ribs = (ribs.0 + cut, ribs.1 + 1);
        }
    }
    assert!(beds.1 > 0 && ribs.1 > 0);
    assert!(beds.0 / beds.1 as f32 > 3.0 * ribs.0 / ribs.1 as f32);
}

#[test]
fn the_ridged_sum_spreads_as_valleys_and_crests_assume() {
    // A quarter of the ground is valley floor, and only the highest few in a hundred
    // reach the crests.
    let range = range();
    let mut v: Vec<f32> = directions(20_000)
        .map(|d| range.ridges(d * RADIUS / 5000.0))
        .collect();
    v.sort_by(f32::total_cmp);
    let share_under = |x: f32| v.iter().filter(|&&r| r < x).count() as f32 / v.len() as f32;
    assert!(
        (0.15..0.4).contains(&share_under(VALLEY)),
        "{}",
        share_under(VALLEY)
    );
    assert!(
        (0.97..1.0).contains(&share_under(CREST)),
        "{}",
        share_under(CREST)
    );
}

#[test] // Slow: minutes.
fn the_mountains_never_jump() {
    // Ground two centimetres apart is never more than a quarter of a metre apart, as on a
    // crease steeper than any wall: no step where a cell, a set of gullies or a measuring
    // direction changes, which would stand as a ledge however close one looked.
    let range = range();
    let mut worst = (0.0, Vec3::ZERO);
    for d in directions(200_000) {
        if range.inside(d * RADIUS) <= 0.0 {
            continue;
        }
        let apart = |cm: f32| {
            let step = d.any_orthonormal_vector() * (cm * 0.005 / RADIUS);
            (range.rise((d + step).normalize(), RADIUS).height_m
                - range.rise((d - step).normalize(), RADIUS).height_m)
                .abs()
        };
        let near = apart(2.0);
        // A crease's rise shrinks with the distance, ten times over ten times closer, and
        // grows as much ten times farther; a step's stays. Both ways, since closer than a
        // centimetre a direction on a planet this size is near a single's precision (2 mm).
        let step = apart(0.2) > 0.5 * near || apart(20.0) < 2.5 * near;
        if near > 0.25 && step && near > worst.0 {
            worst = (near, d);
        }
    }
    assert!(worst.0 == 0.0, "a step of {} m at {}", worst.0, worst.1);
}

#[test]
fn walls_raise_only_the_upper_slopes_and_couloirs_cut_only_down_the_walls() {
    let m = Mountains::START;
    assert!(m.walls > 0.0 && m.couloirs_m > 0.0);
    let plain = Range::new(
        Mountains {
            walls: 0.0,
            couloirs_m: 0.0,
            ..m
        },
        Seed(1),
    );
    let uncut = Range::new(
        Mountains {
            couloirs_m: 0.0,
            ..m
        },
        Seed(1),
    );
    let walled = range();
    let (mut raised, mut deeper) = (0, 0);
    for d in directions(20_000) {
        let point = d * RADIUS;
        let (low, _) = plain.ridged_and_upper(point);
        let (high, upper) = walled.ridged_and_upper(point);
        let carved = |range: &Range, ridged: f32| ridged - range.rise(d, RADIUS).height_m;
        if upper == 0.0 {
            // The valleys, their floors and lakes, as the ridges leave them.
            assert_eq!(low, high);
            assert!((carved(&walled, high) - carved(&uncut, high)).abs() < 1e-3);
            continue;
        }
        // A wall stands its slope's upper part forward, never lower than it was.
        assert!(high >= low - 1e-3, "{high} under {low}");
        if high > low + 50.0 {
            raised += 1;
        }
        if carved(&walled, high) > carved(&uncut, high) + 10.0 {
            deeper += 1;
        }
    }
    assert!(raised > 100, "{raised} raised");
    assert!(deeper > 100, "{deeper} cut deeper");
}

#[test]
fn flat_land_in_a_range_has_no_gullies() {
    // Gullies follow a slope; where there is none, as on the valleys' floors, nothing cuts.
    let range = range();
    for d in directions(20_000) {
        let point = d * RADIUS;
        if range.inside(point) < 1.0 {
            continue;
        }
        // Level over a gully set's reach every way round, measured another way than the
        // gullies measure it: across the ground rather than along the world's axes.
        let (a, b) = (
            d.any_orthonormal_vector(),
            d.any_orthonormal_vector().cross(d),
        );
        let r = range.ridged(point);
        let reach = SLOPE_STEP_M * 2.0;
        let level = [a, -a, b, -b, a + b, a - b, -a + b, -a - b]
            .iter()
            .all(|&way| (range.ridged(point + way.normalize() * reach) - r).abs() < 0.005 * reach);
        if level {
            assert!((range.rise(d, RADIUS).height_m - r).abs() < 1e-3);
        }
    }
}

#[test] // Slow: minutes.
fn the_ground_is_smooth_within_a_voxel() {
    // How far the rise over 2 cm departs from a tenth of the rise over 20 cm: none on a
    // smooth slope. A single holds a point on a planet this size to about 2 mm, and the
    // gullies' depths follow the slope measured over 40 m, so each point's height is rough
    // by a little; rough by half a voxel, the mesh would show it. The ridges' warp, its
    // noises shifted hundreds of cells by adding to the point, stepped by up to 0.15 m.
    let range = range();
    let mut rough = Vec::new();
    for d in directions(100_000) {
        if range.inside(d * RADIUS) <= 0.0 {
            continue;
        }
        let rise = |cm: f32| {
            let step = d.any_orthonormal_vector() * (cm * 0.005 / RADIUS);
            range.rise((d + step).normalize(), RADIUS).height_m
                - range.rise((d - step).normalize(), RADIUS).height_m
        };
        rough.push((rise(2.0) - rise(20.0) / 10.0).abs());
    }
    rough.sort_by(f32::total_cmp);
    let share = |f: f32| rough[((rough.len() - 1) as f32 * f) as usize];
    assert!(share(0.99) < 0.05, "1% rough by {} m", share(0.99));
    assert!(share(0.999) < 0.1, "0.1% rough by {} m", share(0.999));
}
