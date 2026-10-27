use super::*;
use bevy::prelude::Transform;

fn world() -> (ClimateFields, Start) {
    let fields = ClimateFields::for_tests(Vec::new());
    let start = Start(Transform::from_translation(Vec3::Y * fields.sea_radius_m()));
    (fields, start)
}

fn at(day: i64, hour: f64) -> WorldTime {
    WorldTime::at(day, hour)
}

/// The noise's shift between two offsets of field 0's first octave, in metres east and south,
/// taken the short way round the lattice's period.
fn moved_m(from: &[Vec4; 12], to: &[Vec4; 12]) -> DVec2 {
    let short = |d: f32| {
        let d = f64::from(d).rem_euclid(PERIOD);
        if d > PERIOD / 2.0 { d - PERIOD } else { d }
    };
    DVec2::new(short(to[0].x - from[0].x), short(to[0].z - from[0].z)) * FIELD_M[0]
}

#[test]
fn one_clock_draws_one_sky_on_two_worlds_and_another_clock_another() {
    // Two machines' worlds, made apart from the same seed and climate.
    let (a, start) = world();
    let (b, _) = world();
    let (winds_a, winds_b) = (
        CloudWinds::of_world(&a, &start),
        CloudWinds::of_world(&b, &start),
    );
    assert_eq!(winds_a, winds_b);
    let middle = middle_for(Vec2::new(700.0, -300.0));
    let now = at(12, 15.5);
    let anchor = middle;
    assert_eq!(
        offsets(&winds_a, seconds(now), anchor),
        offsets(&winds_b, seconds(now), anchor),
    );
    let map = build_map(&a, &start, middle, now);
    assert_eq!(map, build_map(&b, &start, middle, now));
    // A minute of play on, the shapes have moved; a day on, so has the weather.
    let later = WorldTime(now.0 + 60.0 / f64::from(crate::daylight::DAY_SECONDS));
    assert_ne!(
        offsets(&winds_a, seconds(now), anchor),
        offsets(&winds_a, seconds(later), anchor),
    );
    assert_ne!(map, build_map(&a, &start, middle, at(13, 15.5)));
}

#[test]
fn the_shapes_ride_the_climates_wind_grown_and_veered_aloft() {
    let (fields, _) = world();
    let climate = fields.fields.climate();
    // North up the screen: -z, as the start's frame has it.
    let winds = CloudWinds::of(climate, Vec2::new(0.0, -1.0), true);
    let t = seconds(at(30, 9.0));
    let moved = moved_m(
        &offsets(&winds, t, Vec2::ZERO),
        &offsets(&winds, t + 10.0, Vec2::ZERO),
    );
    // The noise is taken from upwind, so it shifts against the wind.
    let velocity = -moved / 10.0;
    // At 1.5 km a wind of the climate's mean grows by (1500 / 10)^(1/7), about 2.05.
    let speed = f64::from(climate.wind.mean_m_s) * 2.046;
    assert!(
        (velocity.length() - speed).abs() < 0.01 * speed,
        "{velocity:?} against {speed}"
    );
    // From the prevailing bearing, so toward the opposite one, turned 20 degrees clockwise.
    let toward = f64::from(climate.wind.prevailing_from_deg) + 180.0 + 20.0;
    let bearing = libm::atan2(velocity.x, -velocity.y).to_degrees();
    let off = (bearing - toward).rem_euclid(360.0);
    assert!(
        off.min(360.0 - off) < 0.5,
        "bearing {bearing}, wanted {toward}"
    );
    // South of the equator it is turned the other way.
    let south = CloudWinds::of(climate, Vec2::new(0.0, -1.0), false);
    assert!(south.layer.angle_to(winds.layer).abs() > 0.6);
    // The scud low under a deck is slower than the layer, and as the ground's.
    assert!(winds.scud.length() < winds.layer.length() * 0.85);
}

#[test]
fn the_offsets_stay_exact_on_a_world_decades_old() {
    let (fields, start) = world();
    let winds = CloudWinds::of_world(&fields, &start);
    let anchor = Vec2::new(4000.0, -8000.0);
    for day in [0, 400, 20_000] {
        let t = seconds(at(day, 11.0));
        let moved = moved_m(
            &offsets(&winds, t, anchor),
            &offsets(&winds, t + 1.0, anchor),
        );
        // One second of the wind aloft, to a decimetre of a layer 1.5 km up (the offsets are
        // f32 under the lattice's period, 2.7 km a unit), however far the clock has run.
        assert!(
            (moved + winds.layer).length() < 0.1,
            "day {day}: {moved:?} against {:?}",
            -winds.layer
        );
        for o in offsets(&winds, t, anchor) {
            assert!(o.x >= 0.0 && o.x < PERIOD as f32 && o.z >= 0.0 && o.z < PERIOD as f32);
        }
    }
}

#[test]
fn moving_the_anchor_moves_nothing_drawn() {
    // The shader adds the offset to metres from the anchor; a different anchor with the same
    // point gives the same noise, so the map's middle can move without a jump.
    let (fields, start) = world();
    let winds = CloudWinds::of_world(&fields, &start);
    let t = seconds(at(3, 14.0));
    let (a, b) = (Vec2::new(0.0, 0.0), Vec2::new(4000.0, -4000.0));
    let point = Vec2::new(1234.0, 567.0);
    let (oa, ob) = (offsets(&winds, t, a), offsets(&winds, t, b));
    for (f, &metres) in FIELD_M.iter().enumerate() {
        for (o, &(frequency, _)) in OCTAVES.iter().enumerate() {
            let i = f * 4 + o;
            let seen = |anchor: Vec2, off: Vec4| {
                let p = (point - anchor).as_dvec2() / metres * frequency;
                DVec2::new(p.x + f64::from(off.x), p.y + f64::from(off.z))
            };
            let d = seen(a, oa[i]) - seen(b, ob[i]);
            let wrapped = |v: f64| {
                let v = v.rem_euclid(PERIOD);
                v.min(PERIOD - v)
            };
            assert!(
                wrapped(d.x) < 1e-3 && wrapped(d.y) < 1e-3,
                "field {f} octave {o}: {d:?}"
            );
        }
    }
}

#[test]
fn the_map_shows_a_front_tens_of_kilometres_off_while_the_eye_is_clear() {
    let (fields, start) = world();
    let middle = Vec2::ZERO;
    let cover_at = |east: f32, south: f32, time: WorldTime| {
        let point = start.at(east, south).normalize() * fields.sea_radius_m();
        fields.fields.at(fields.place(point), time).cloud
    };
    // The first hour in two months clear overhead with a front 16 km off.
    let far = [
        (16_000.0, 0.0),
        (-16_000.0, 0.0),
        (0.0, 16_000.0),
        (0.0, -16_000.0),
    ];
    let found = (0..60 * 24)
        .map(|h| at(h / 24, (h % 24) as f64))
        .find_map(|time| {
            if cover_at(0.0, 0.0, time) > 0.1 {
                return None;
            }
            far.iter()
                .find(|&&(e, s)| cover_at(e, s, time) > 0.9)
                .map(|&place| (time, place))
        });
    let Some((time, (east, south))) = found else {
        panic!("no clear hour with a front 16 km off in two months");
    };
    let started = std::time::Instant::now();
    let map = build_map(&fields, &start, middle, time);
    eprintln!(
        "weather map {}x{} built in {:.1} ms",
        MAP_TEXELS,
        MAP_TEXELS,
        started.elapsed().as_secs_f32() * 1000.0
    );
    let texel_at = |east: f32, south: f32| {
        let corner = middle - Vec2::splat(MAP_SIDE_M / 2.0);
        let t = ((Vec2::new(east, south) - corner) / MAP_TEXEL_M).floor();
        let i = (t.y as usize * MAP_TEXELS as usize + t.x as usize) * 4;
        &map[i..i + 4]
    };
    // Texels' middles sit a half texel off these points; the climate's fronts are kilometres.
    let eye = texel_at(0.0, 0.0);
    assert!(eye[0] < 64, "the eye's cover {}", eye[0]);
    let front = texel_at(east, south);
    assert!(front[0] > 200, "the front's cover {}", front[0]);
    // A front's cover closes into a deck: its kind is the stratus end, the eye's heaped.
    assert!(front[2] < eye[2]);
}

#[test]
fn the_volumes_noise_rides_the_layers_wind_exactly_and_the_anchor_moves_nothing() {
    let (fields, start) = world();
    let winds = CloudWinds::of_world(&fields, &start);
    // A share of a tile, taken the short way round.
    let short = |d: f32| {
        let d = f64::from(d).rem_euclid(1.0);
        if d > 0.5 { d - 1.0 } else { d }
    };
    let anchor = Vec2::new(4000.0, -8000.0);
    for day in [0, 400, 20_000] {
        let t = seconds(at(day, 11.0));
        let (now, then) = (
            volume_offsets(&winds, t, anchor),
            volume_offsets(&winds, t + 1.0, anchor),
        );
        for v in now.to_array() {
            assert!((0.0..1.0).contains(&v), "day {day}: {now:?}");
        }
        // The shapes and the detail both moved one second of the layer's wind, to a centimetre,
        // and the same way the 2D shapes move (the noise is taken from upwind).
        let shape = DVec2::new(short(then.x - now.x), short(then.y - now.y)) * SHAPE_TILE_M;
        let detail = DVec2::new(short(then.z - now.z), short(then.w - now.w)) * DETAIL_TILE_M;
        for moved in [shape, detail] {
            assert!(
                (moved + winds.layer).length() < 0.01,
                "day {day}: {moved:?} against {:?}",
                -winds.layer
            );
        }
        let flat = moved_m(
            &offsets(&winds, t, anchor),
            &offsets(&winds, t + 1.0, anchor),
        );
        assert!((flat - shape).length() < 0.1, "{flat:?} against {shape:?}");
    }
    // A point seen from two anchors is the same noise.
    let t = seconds(at(3, 14.0));
    let (a, b) = (Vec2::ZERO, Vec2::new(4000.0, -4000.0));
    let point = Vec2::new(1234.0, 567.0);
    let (oa, ob) = (volume_offsets(&winds, t, a), volume_offsets(&winds, t, b));
    for (tile, i) in [(SHAPE_TILE_M, 0), (DETAIL_TILE_M, 2)] {
        let seen = |anchor: Vec2, off: Vec4| {
            let p = (point - anchor).as_dvec2() / tile;
            [p.x + f64::from(off[i]), p.y + f64::from(off[i + 1])]
        };
        let (sa, sb) = (seen(a, oa), seen(b, ob));
        for k in 0..2 {
            let d = (sa[k] - sb[k]).rem_euclid(1.0);
            assert!(d.min(1.0 - d) < 1e-5, "tile {tile}: {sa:?} against {sb:?}");
        }
    }
}

#[test]
fn the_maps_middle_is_on_a_grid() {
    assert_eq!(
        middle_for(Vec2::new(1999.0, -2001.0)),
        Vec2::new(0.0, -4000.0)
    );
    assert_eq!(
        middle_for(Vec2::new(2001.0, 0.0)),
        middle_for(Vec2::new(5900.0, 1900.0))
    );
}

#[test]
fn every_kinds_layer_stands_over_the_ground_inside_the_slab_at_any_weather() {
    let ground = 1317.0;
    let (rows, slab) = layers_over_sea(ground);
    let [bases, by_rain, tops, top_by_cover, top_by_rain] = rows;
    for kind in 0..3usize {
        for (cover, rain) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
            let base = bases[kind] + by_rain[kind] * rain;
            let top = tops[kind] + top_by_cover[kind] * cover + top_by_rain[kind] * rain;
            assert!(
                base > ground && base + 200.0 < top,
                "{kind}: {base} to {top}"
            );
            assert!(
                slab.x <= base && top <= slab.y,
                "{kind}: {base} to {top} in {slab}"
            );
        }
    }
}

#[test]
fn heaped_and_rained_tops_stop_at_d544s_heights_over_the_sea() {
    // The tallest a top grows to: (deck in rain, cumulus at full cover), over the sea.
    let tallest = |ground: f32| {
        let (rows, _) = layers_over_sea(ground);
        let [_, _, tops, by_cover, by_rain] = rows;
        (tops.x + by_rain.x, tops.z + by_cover.z)
    };
    // Low ground: under both ceilings, the table's own growth (1.8 km and 3.1 km over it).
    assert_eq!(tallest(200.0), (2000.0, 3300.0));
    // The start's lake, 1.3 km up: congestus stops at 4.2 km over the sea, not 4.4.
    let (deck, cumulus) = tallest(1317.0);
    assert_eq!(deck, 1317.0 + 1800.0);
    assert!((cumulus - 4200.0).abs() < 0.5, "{cumulus}");
    // Ground at 2.5 km: nimbostratus stops at 3.8 km, and cumulus no longer piles over its
    // fair-weather top, there already at 4.2 km.
    let (deck, cumulus) = tallest(2500.0);
    assert!((deck - 3800.0).abs() < 0.5, "{deck}");
    assert_eq!(cumulus, 2500.0 + 1700.0);
}
