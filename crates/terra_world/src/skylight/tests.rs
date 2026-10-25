use super::*;

/// The light from a way, the hue worked out for it alone.
fn toward(c: &Colours, d: Vec3) -> Vec3 {
    c.light(&c.lit(), d, c.hue(d.y.max(0.0)))
}

fn lobes(c: &Colours) -> ([Vec3; 6], [Vec3; 6]) {
    c.painting(&c.lit()).lobes()
}

/// A sun so high, in degrees, in the +X half of the sky.
fn sun_at(degrees: f32) -> Vec3 {
    let r = degrees.to_radians();
    Vec3::new(libm::cosf(r), libm::sinf(r), 0.0)
}

fn colours() -> Colours {
    Colours {
        low: 0.0,
        day: 1.0,
        shade: Vec3::new(0.4, 0.5, 1.0),
        horizon: Vec3::new(0.9, 0.85, 0.8),
        zenith: Vec3::new(0.1, 0.25, 0.7),
        ground: Vec3::new(0.3, 0.35, 0.1),
        overcast: 0.0,
        sun: sun_at(40.0),
        glow: srgb_linear(1.0, 0.74, 0.46),
    }
}

/// The irradiance over pi toward a side from the light of every way, by fine quadrature:
/// even in height (even in area) and round.
fn quadrature(c: &Colours, n: Vec3, above: bool) -> Vec3 {
    let lit = c.lit();
    let rows = 240;
    let mut sum = Vec3::ZERO;
    for i in 0..rows {
        let y = 1.0 - 2.0 * (i as f32 + 0.5) / rows as f32;
        if (y >= 0.0) != above {
            continue;
        }
        let across = (1.0 - y * y).max(0.0).sqrt();
        let hue = c.hue(y.max(0.0));
        for j in 0..rows * 2 {
            let round = (j as f32 + 0.5) / (rows * 2) as f32 * std::f32::consts::TAU;
            let d = Vec3::new(across * libm::cosf(round), y, across * libm::sinf(round));
            sum += c.light(&lit, d, hue) * d.dot(n).max(0.0);
        }
    }
    // Each way stands for 4 pi / (2 rows^2) of the sphere; over pi.
    sum * 4.0 / (2 * rows * rows) as f32
}

const SIDES: [Vec3; 6] = [
    Vec3::X,
    Vec3::NEG_X,
    Vec3::Y,
    Vec3::NEG_Y,
    Vec3::Z,
    Vec3::NEG_Z,
];

#[test]
fn the_harmonics_diffuse_light_is_the_exact_irradiance_within_a_few_percent() {
    // Three bands of harmonics against the cosine-weighted sum of the light from every way,
    // for a high sun, a low one, one set and an overcast sky; every side and the ways
    // between. Ramamoorthi and Hanrahan bound three bands' error for any light at 9% of the
    // irradiance and 3% on average; here it is 4% at most (facing up and down by a set sun,
    // the bright horizon's ring) and 1% on average.
    let (mut worst, mut sum, mut count) = (0.0f32, 0.0f32, 0);
    for (sun, overcast, low) in [
        (70.0, 0.0, 0.0),
        (20.0, 0.0, 0.1),
        (6.0, 0.0, 0.9),
        (-4.0, 0.0, 1.0),
        (30.0, 1.0, 0.0),
    ] {
        let mut c = colours();
        c.sun = sun_at(sun);
        c.overcast = overcast;
        c.low = low;
        let painting = c.painting(&c.lit());
        let all = painting.sky.plus(&painting.ground);
        let ways = SIDES.into_iter().chain([
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(-1.0, 0.4, 1.0),
            Vec3::new(0.3, -1.0, -0.6),
        ]);
        let exact: Vec<(Vec3, Vec3, Vec3)> = ways
            .map(|n| {
                let n = n.normalize();
                (n, quadrature(&c, n, true), quadrature(&c, n, false))
            })
            .collect();
        // Measured against the brightest side's light, as Ramamoorthi and Hanrahan bound it.
        let scale = exact
            .iter()
            .map(|(_, sky, ground)| (*sky + *ground).max_element())
            .fold(0.0, f32::max);
        for (n, sky, ground) in &exact {
            let error = (all.diffuse(*n) - (*sky + *ground)).abs().max_element() / scale;
            worst = worst.max(error);
            sum += error;
            count += 1;
        }
        // The sky's part and the ground's apart, as the irradiance volume reads them.
        let (sky, ground) = painting.lobes();
        for (k, (n, want_sky, want_ground)) in exact.iter().take(6).enumerate() {
            let error = |got: Vec3, want: Vec3| (got - want).abs().max_element() / scale;
            assert!(
                error(sky[k], *want_sky) < 0.05,
                "{n}: sky {} {want_sky}",
                sky[k]
            );
            assert!(
                error(ground[k], *want_ground) < 0.05,
                "{n}: ground {} {want_ground}",
                ground[k]
            );
        }
    }
    let mean = sum / count as f32;
    assert!(worst < 0.05 && mean < 0.02, "worst {worst} mean {mean}");
    eprintln!("harmonics: worst {worst}, mean {mean}");
}

#[test]
fn a_side_facing_the_sun_takes_more_sky_light_and_warmer_than_one_facing_away() {
    // D490: by day a clear sky is brightest round the sun, and that light is the sun's glow.
    let warmth = |c: Vec3| c.x / c.z;
    for sun in [60.0, 25.0, 8.0] {
        let mut c = colours();
        c.sun = sun_at(sun);
        let (sky, _) = lobes(&c);
        let (toward_sun, away) = (sky[0], sky[1]);
        assert!(
            luma(toward_sun) > 1.5 * luma(away),
            "sun {sun}: {toward_sun} {away}"
        );
        assert!(
            warmth(toward_sun) > warmth(away) * 1.05,
            "sun {sun}: {toward_sun} {away}"
        );
        // The sides across the sun's way take alike.
        assert!(
            (luma(sky[4]) - luma(sky[5])).abs() < 0.01 * luma(sky[4]),
            "{} {}",
            sky[4],
            sky[5]
        );
    }
    // Under a full deck, by night, the sky is the same all round.
    for (overcast, day) in [(1.0, 1.0), (0.0, 0.0)] {
        let mut c = colours();
        c.overcast = overcast;
        c.day = day;
        let (sky, _) = lobes(&c);
        assert!(
            (luma(sky[0]) - luma(sky[1])).abs() < 0.02 * luma(sky[0]),
            "{} {}",
            sky[0],
            sky[1]
        );
    }
}

#[test]
fn open_ground_takes_the_light_the_even_sky_gave_it_wherever_the_sun_is() {
    // The CIE's clear sky is scaled so what faces up takes what the old sky, the same all
    // round, gave it: the shade's brightness was tuned to that (D392).
    let mut even = colours();
    even.day = 0.0;
    let want = luma(lobes(&even).0[2]);
    for sun in [80.0, 40.0, 10.0, 2.0] {
        let mut c = colours();
        c.sun = sun_at(sun);
        let up = luma(lobes(&c).0[2]);
        assert!((up - want).abs() < 0.05 * want, "sun {sun}: {up} {want}");
    }
}

#[test]
fn the_painting_is_what_each_way_gives_its_heights_hue_worked_out_once() {
    // A low sun under some cloud, so every branch of `light` is taken.
    let mut c = colours();
    c.low = 0.4;
    c.overcast = 0.3;
    c.sun = sun_at(12.0);
    let cube = c.painting(&c.lit()).cube;
    let mut at = 0;
    for face in 0..6 {
        for y in 0..FACE {
            for x in 0..FACE {
                let light = toward(&c, face_direction(face, x, y, FACE).normalize());
                for channel in [light.x, light.y, light.z, 1.0] {
                    assert_eq!(
                        cube[at..at + 4],
                        channel.to_le_bytes(),
                        "face {face} {x},{y}"
                    );
                    at += 4;
                }
            }
        }
    }
}

#[test]
fn the_diffuse_map_is_the_harmonics_toward_each_texel_in_half_floats() {
    let c = colours();
    let painting = c.painting(&c.lit());
    let data = painting.diffuse();
    assert_eq!(data.len(), (DIFFUSE * DIFFUSE * 6 * 8) as usize);
    let all = painting.sky.plus(&painting.ground);
    // The texel by the middle of the top face.
    let texel = (2 * DIFFUSE + DIFFUSE / 2) * DIFFUSE + DIFFUSE / 2;
    let at = texel as usize * 8;
    let half =
        |i: usize| half::f16::from_le_bytes([data[at + i * 2], data[at + i * 2 + 1]]).to_f32();
    let want = all.diffuse(face_direction(2, DIFFUSE / 2, DIFFUSE / 2, DIFFUSE).normalize());
    let got = Vec3::new(half(0), half(1), half(2));
    assert!(
        (got - want).abs().max_element() < 2e-3 * want.max_element(),
        "{got} {want}"
    );
    assert_eq!(half(3), 1.0);
}

#[test]
fn under_an_overcast_deck_what_faces_up_is_lit_over_what_faces_out() {
    let lit = |overcast: f32| {
        let mut c = colours();
        c.overcast = overcast;
        // The ground under snow, lit by the sky over it, as `paint` sets it.
        c.ground = SNOW_ALBEDO * c.overhead(&c.lit());
        let (sky, ground) = lobes(&c);
        // A side across the sun's way, neither toward it nor away.
        let side = luma(sky[4] + ground[4]);
        (luma(sky[2] + ground[2]), side, luma(sky[3] + ground[3]))
    };
    let (up, side, down) = lit(1.0);
    let (clear_up, clear_side, clear_down) = lit(0.0);
    // An overcast sky is three times brighter overhead than at its horizon, so a side,
    // which sees half the sky, mostly its low part, and half the snow, takes a sixth less
    // than what faces up (CIE: a wall takes 0.4 of the open ground's sky light); a clear
    // sky, brightest low, lit them nearly alike.
    assert!(up > 1.15 * side, "up {up} side {side}");
    assert!(
        up / side > clear_up / clear_side + 0.1,
        "{clear_up} {clear_side}"
    );
    assert!(up / down > clear_up / clear_down, "{clear_up} {clear_down}");
    // Open ground takes about the light it took under a clear sky.
    assert!((up - clear_up).abs() < 0.1 * clear_up, "{up} {clear_up}");
}

#[test]
fn under_a_closed_canopy_nothing_mirrors_the_sky_and_in_the_open_all_of_it() {
    assert_eq!(sky_reflected(0.0), 0.0);
    assert_eq!(sky_reflected(1.0), 1.0);
    assert!(sky_reflected(0.3) < sky_reflected(0.6));
    // Small changes as the eye walks are not followed.
    assert_eq!(sky_reflected(0.401), sky_reflected(0.405));
}

#[test]
fn the_sky_is_blue_above_and_the_ground_below() {
    let c = colours();
    let above = toward(&c, Vec3::Y);
    assert!(above.z > above.x, "{above}");
    assert_eq!(toward(&c, -Vec3::Y), c.ground);
}

#[test]
fn a_clear_sky_is_brighter_at_the_horizon() {
    // Away from the sun as well as toward it.
    let c = colours();
    for d in [Vec3::X, -Vec3::X, Vec3::Z] {
        assert!(luma(toward(&c, d)) > luma(toward(&c, Vec3::Y)), "{d}");
    }
}

#[test]
fn each_side_takes_a_cosine_weighted_share_of_what_it_faces() {
    let c = colours();
    let (sky, ground) = lobes(&c);
    // Facing up sees only sky; facing down only ground.
    assert!(luma(ground[2]) < 0.02 * luma(sky[2]) && luma(sky[3]) < 0.02 * luma(ground[3]));
    // Facing down sees only the ground, which is one colour: all of it, and a little of the
    // sky's bright horizon its first nine degrees blend from, which the bands ring with.
    assert!(ground[3].distance(c.ground) < 0.05, "{}", ground[3]);
    // A side sees half sky and half ground.
    let side = sky[4] + ground[4];
    let half = (c.ground + toward(&c, Vec3::Z)) * 0.5;
    assert!(luma(side) > luma(half) * 0.7 && luma(side) < luma(half) * 1.3);
}

#[test]
fn the_cube_has_the_sky_on_its_top_face_and_the_ground_on_its_bottom() {
    let c = colours();
    let cube = c.painting(&c.lit()).cube;
    // The pixel by the middle of a face, faces in the order +X, -X, +Y, -Y, +Z, -Z.
    let middle = |face: u32| {
        let pixel = (face * FACE + FACE / 2) * FACE + FACE / 2;
        let at = pixel as usize * 16;
        let float =
            |i: usize| f32::from_le_bytes(cube[at + i * 4..at + i * 4 + 4].try_into().unwrap());
        (Vec3::new(float(0), float(1), float(2)), float(3))
    };
    let (top, opaque) = middle(2);
    assert_eq!(opaque, 1.0);
    let way = face_direction(2, FACE / 2, FACE / 2, FACE).normalize();
    assert!(
        top.distance(toward(&c, way)) < 1e-6 && top.z > top.x,
        "{top}"
    );
    let (bottom, _) = middle(3);
    assert!(bottom.distance(c.ground) < 0.01, "{bottom}");
}

#[test]
fn daylight_of_6500_kelvin_is_srgb_white_and_the_locus_runs_amber_to_blue() {
    // sRGB's white is D65, CIE daylight of 6,504 K.
    let white = daylight(6_504.0);
    assert!((white - Vec3::ONE).abs().max_element() < 0.03, "{white}");
    let warm = daylight(4_000.0);
    let cold = daylight(15_000.0);
    assert!(warm.x > warm.y && warm.y > warm.z, "{warm}");
    assert!(cold.z > cold.y && cold.y > cold.x, "{cold}");
}

#[test]
fn a_low_sun_keeps_the_shade_violet_and_the_ground_lit_by_the_sky_is_not_green() {
    // The settlement's dusk: an orange horizon, a violet shade key, grass on the ground.
    let mut c = Colours {
        low: 1.0,
        day: 1.0,
        shade: srgb_linear(0.76, 0.68, 0.92),
        horizon: srgb_linear(1.0, 0.8, 0.62),
        zenith: srgb_linear(0.35, 0.45, 0.75),
        ground: Vec3::ZERO,
        overcast: 0.0,
        sun: sun_at(2.0),
        glow: srgb_linear(1.0, 0.6, 0.32),
    };
    let sky = c.overhead(&c.lit());
    // Violet: red and blue both over green, as the keys give it; on the locus green sat
    // between them and the shade read grey.
    assert!(sky.y < sky.x && sky.y < sky.z, "{sky}");
    // Grass lit by that sky gives back a grey, not its own green.
    let grass = Vec3::new(0.05, 0.09, 0.03);
    c.ground = grass * sky;
    let lit = c.ground / luma(c.ground);
    let unlit = grass / luma(grass);
    assert!(lit.y < unlit.y, "{lit} {unlit}");
    // By day it stays on the locus.
    c.low = 0.0;
    let day = c.overhead(&c.lit());
    assert!(day.y >= day.x.min(day.z), "{day}");
}

#[test]
fn a_colour_on_the_locus_is_never_magenta_and_keeps_its_brightness() {
    let lilac = Vec3::new(0.45, 0.4, 0.8);
    let on = on_daylight_locus(lilac);
    // Daylight's green lies between its red and its blue; lilac's is under both.
    assert!(on.y >= on.x.min(on.z), "{on}");
    assert!((luma(on) - luma(lilac)).abs() < 1e-3, "{on}");
    // As cool as it was: blue over red kept within the locus's reach.
    assert!(on.z > on.x, "{on}");
}

fn srgb_linear(r: f32, g: f32, b: f32) -> Vec3 {
    Vec3::from_slice(&Color::srgb(r, g, b).to_linear().to_f32_array()[..3])
}

#[test]
fn above_twilight_an_orange_and_lilac_sky_lights_the_shade_daylight_not_lilac() {
    // An orange horizon, a blue zenith and a lilac shade colour, with the sun above
    // twilight and an orange glow round it; every way into the sky, toward the sun and away,
    // and the light toward each side is daylight, never magenta.
    for sun in [25.0, 40.0, 70.0] {
        let c = Colours {
            low: 0.0,
            day: 1.0,
            shade: srgb_linear(0.68, 0.66, 0.9),
            horizon: srgb_linear(0.95, 0.55, 0.38),
            zenith: srgb_linear(0.16, 0.24, 0.52),
            ground: Vec3::new(0.1, 0.1, 0.08),
            overcast: 0.0,
            sun: sun_at(sun),
            glow: srgb_linear(1.0, 0.6, 0.32),
        };
        let painting = c.painting(&c.lit());
        for texel in painting.cube.chunks(16) {
            let float = |i: usize| f32::from_le_bytes(texel[i * 4..i * 4 + 4].try_into().unwrap());
            let light = Vec3::new(float(0), float(1), float(2));
            assert!(light.y >= light.x.min(light.z) * 0.99, "sun {sun}: {light}");
        }
        for side in painting.lobes().0 {
            assert!(side.y >= side.x.min(side.z) * 0.99, "sun {sun}: {side}");
        }
    }
}
