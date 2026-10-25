use super::*;
use crate::scene::SUN_DIRECTION;

fn path() -> SkyPath {
    SkyPath::over(Vec3::new(0.1, 1.0, 0.2), climate().declination(0))
}

#[test]
fn the_mist_is_drawn_as_thick_round_a_walker_and_thinner_for_an_eye_above_it() {
    // A walker, and a rider, stand in the mist as it was tuned.
    assert_eq!(above_mist(1.7), 1.0);
    assert_eq!(above_mist(3.0), 1.0);
    // From a bird's-eye height the haze between the eye and the ground is thinner, the
    // higher the thinner, and never gone.
    let (low, high, higher) = (above_mist(12.0), above_mist(25.0), above_mist(60.0));
    assert!(
        low < 1.0 && high < low && higher <= high,
        "{low} {high} {higher}"
    );
    assert!(higher > 0.2, "{higher}");
}

#[test]
fn the_eye_opens_up_in_the_open_of_a_far_seeing_world_and_nowhere_else() {
    let key = 11.8;
    // The lush forest's world keeps the key in the open, by day and by night, and any world's
    // forest by day.
    assert_eq!(open_exposure(key, 1.0, false, 0.0, 1.0), key);
    assert_eq!(open_exposure(key, 1.0, false, 1.0, 1.0), key);
    assert_eq!(open_exposure(key, 0.2, true, 0.0, 1.0), key);
    assert_eq!(open_exposure(key, 0.2, false, 0.0, 1.0), key);
    // By night under the crowns the eye opens, on every world, and the more the darker.
    for far_seeing in [false, true] {
        let wood = open_exposure(key, 0.2, far_seeing, 1.0, 1.0);
        assert!((key - wood - WOODED_NIGHT_EV).abs() < 1e-4, "{wood}");
        assert!(open_exposure(key, 0.2, far_seeing, 0.5, 1.0) > wood);
    }
    // Open moonlit land on a far-seeing world opens up too, less than by day, so a
    // town's street and a lake's shore at night are not silhouettes.
    let moonlit = open_exposure(key, 1.0, true, 1.0, 1.0);
    assert!(moonlit < key - 0.5, "{moonlit}");
    assert!(
        moonlit > open_exposure(key, 1.0, true, 0.0, 1.0),
        "{moonlit}"
    );
    // Out on a mountain lake's shore by day, brighter: a lower EV.
    let open = open_exposure(key, 1.0, true, 0.0, 1.0);
    assert!(open < key - 1.0, "{open}");
    // And smoothly between, as one walks out of the trees.
    let mut last = key;
    for k in 0..=20 {
        let now = open_exposure(key, k as f32 / 20.0, true, 0.0, 1.0);
        assert!(now <= last && last - now < 0.3, "{last} then {now}");
        last = now;
    }
}

#[test]
fn by_night_a_closed_room_keeps_the_key_while_the_eye_opens_under_the_trees() {
    let key = 11.8;
    // Both hide the sky (`open` low), at midnight: under the crowns, outdoors, the eye opens
    // the whole canopy's stops; in a closed room (D485, `IndoorSky` 0) it keeps the key, the
    // room's dark meant and its fires setting the eye.
    for far_seeing in [false, true] {
        let wood = open_exposure(key, 0.1, far_seeing, 1.0, 1.0);
        assert!((key - wood - WOODED_NIGHT_EV).abs() < 1e-4, "{wood}");
        assert_eq!(open_exposure(key, 0.1, far_seeing, 1.0, 0.0), key);
        assert_eq!(wooded_night_ev(0.1, 1.0, 0.0), 0.0);
        // A room open to the sky through its door opens the eye less than the wood, and
        // smoothly as the eye steps out.
        let mut last = key;
        for k in 0..=20 {
            let now = open_exposure(key, 0.1, far_seeing, 1.0, k as f32 / 20.0);
            assert!(now <= last && last - now < 0.3, "{last} then {now}");
            last = now;
        }
    }
}

#[test]
fn out_of_the_trees_the_haze_is_thinner_and_takes_the_horizons_colour() {
    let horizon = Vec3::new(0.9, 0.86, 0.8);
    let small = crate::air::AirScale(crate::air::HAZE_SCALE);
    let (under, near) = haze_for(horizon, 0.1, small);
    let (out, far) = haze_for(horizon, 1.0, small);
    assert_eq!(near, super::super::scene::HAZE_VISIBLE_M);
    assert!(far > near * 8.0, "{far} against {near}");
    // Under the canopy green and dim; out of it the horizon's own hue.
    assert!(under.y > under.x && under.y > under.z, "{under}");
    let hue = |c: Vec3| c / c.max_element();
    assert!(
        (hue(out) - hue(horizon)).abs().max_element() < 1e-4,
        "{out}"
    );
    // Between, it changes smoothly as the eye walks out.
    let mut last = near;
    for k in 1..=20 {
        let (_, now) = haze_for(horizon, k as f32 / 20.0, small);
        assert!(now >= last && now < last * 1.6, "{last} then {now}");
        last = now;
    }
}

#[test]
fn the_sun_rises_stands_high_at_noon_and_sets() {
    let path = path();
    assert!(path.height(MORNING) > 10.0 && path.height(MORNING) < 30.0);
    assert!(
        path.height(9.0) > path.height(MORNING),
        "the morning sun climbs"
    );
    // At noon the sun is as far from overhead as the latitude, less its declination.
    let noon = path.height(12.0);
    let highest = 90.0 - LATITUDE.to_degrees() + path.declination.to_degrees();
    assert!((noon - highest).abs() < 0.5, "noon {noon}, not {highest}");
    assert!(path.height(0.0) < -30.0, "midnight is dark");
    // As long up as the sunrise equation says: cos h = -tan latitude tan declination,
    // with h the hour angle from sunrise to noon. In summer, longer than the night.
    let sunrise = libm::acosf(-libm::tanf(LATITUDE) * libm::tanf(path.declination));
    let day_tenths = sunrise / TAU * 2.0 * 240.0;
    let up = (0..240)
        .filter(|i| path.height(*i as f32 / 10.0) > 0.0)
        .count();
    assert!(
        (up as f32 - day_tenths).abs() <= 2.0,
        "{up} tenths of an hour of day, not {day_tenths}"
    );
    assert!(up > 120);
    // East in the morning, west in the evening.
    assert!(path.sun(7.0).dot(path.east) > 0.5);
    assert!(path.sun(17.0).dot(path.east) < -0.5);
}

#[test]
fn the_morning_sun_stands_where_the_art_put_it() {
    let path = path();
    let toward = SUN_DIRECTION.reject_from(path.up).normalize();
    let sun = path.sun(MORNING).reject_from(path.up).normalize();
    assert!(sun.dot(toward) > 0.999);
}

#[test]
fn the_light_is_the_art_at_twenty_degrees_and_dark_at_night() {
    let keys = keys();
    let (_, art) = keys.iter().find(|(height, _)| *height == 20.0).unwrap();
    let (_, deepest) = keys[0];
    let close = |a: f32, b: f32| (a - b).abs() <= 1e-3 * b.abs().max(1.0);
    // At each key the light is the key's, and past the last ones it stays.
    for (height, light) in keys {
        let at = light_at(height);
        assert!(close(at.illuminance, light.illuminance), "{height}");
        assert!(close(at.ambient, light.ambient), "{height}");
        assert!(at.zenith.distance(light.zenith) < 1e-4, "{height}");
    }
    let night = light_at(-90.0);
    assert_eq!(night, deepest);
    assert_eq!(night.night, 1.0);
    assert!(night.illuminance < art.illuminance && night.ambient < art.ambient);
    // Most of the night's light is the moon's, so it lights the night from one side and
    // casts shadows; the sky's light under it is a glow (D310).
    assert!(night.illuminance > night.ambient);
    // Halfway between two keys, the light is halfway between theirs.
    for pair in keys.windows(2) {
        let ((low, a), (high, b)) = (pair[0], pair[1]);
        let between = light_at((low + high) / 2.0);
        let halfway = (a.illuminance + b.illuminance) / 2.0;
        assert!(close(between.illuminance, halfway), "{low} to {high}");
        assert!(close(between.sky, (a.sky + b.sky) / 2.0), "{low} to {high}");
    }
}

#[test]
fn a_low_sun_greys_the_land_warms_the_horizon_and_the_shade() {
    let art = light_at(20.0);
    // From the sun's last 15 degrees to the horizon, the land in the midtones keeps a
    // fifth to a quarter less of its colour, and the horizon and the shade lean warm.
    for height in [0.0, 4.0, 8.0, 12.0] {
        let low = light_at(height);
        assert!(low.colour <= 0.85 && low.colour >= 0.7, "{height}");
        assert!(low.horizon.x - low.horizon.z > 0.3, "{height}: horizon");
        let warmth = |c: Vec3| c.x / c.z;
        assert!(
            warmth(low.shade) > warmth(art.shade) + 0.05,
            "{height}: shade"
        );
    }
    assert_eq!(art.colour, 1.0);
    assert_eq!(light_at(60.0).colour, 1.0);
}

#[test]
fn the_moon_rises_from_nothing_as_the_sun_sets() {
    // Not till the dusk has gone, so the afterglow is not lit from the far side (D310).
    assert_eq!(moonlight(-1.0), 0.0);
    assert_eq!(moonlight(-8.0), 0.0);
    assert!(moonlight(-10.0) < moonlight(-13.0));
    assert_eq!(moonlight(-14.0), light_at(-14.0).illuminance);
    assert_eq!(moonlight(-18.0), light_at(-18.0).illuminance);
}

#[test]
fn the_moon_stands_high_on_the_far_side_of_the_sky_from_the_sun() {
    let path = SkyPath::over(Vec3::Y, climate().declination(0));
    for hour in [21.0, 23.5, 1.0, 3.0] {
        let moon = path.moon(hour);
        let up = libm::asinf(moon.dot(Vec3::Y));
        assert!((up - MOON_HEIGHT).abs() < 1e-3, "{hour}: {up}");
        let sun = path.sun(hour).reject_from(Vec3::Y);
        assert!(
            moon.reject_from(Vec3::Y).dot(sun) < 0.0,
            "{hour}: on the sun's side"
        );
    }
}

#[test]
fn a_place_east_of_the_start_keeps_a_later_hour_by_its_own_sun() {
    let path = SkyPath::over(Vec3::Y, climate().declination(160));
    // A place 15 degrees round the pole to the east: its sky an hour on from the start's.
    let east = Quat::from_axis_angle(path.pole(), 15f32.to_radians()) * Vec3::Y;
    assert!(east.dot(path.east) > 0.0, "the turn goes east");
    let there = path.seen_from(east);
    for hour in [6.0, 9.0, 13.0, 17.5] {
        assert!(
            (path.local_hour(hour) - hour).abs() < 1e-4,
            "the start keeps the clock"
        );
        let local = there.local_hour(hour);
        assert!((local - (hour + 1.0)).abs() < 1e-3, "{hour}: {local}");
        // And its sun stands as the start's does at that hour.
        assert!(
            (there.height(hour) - path.height(hour + 1.0)).abs() < 0.05,
            "{hour}"
        );
    }
}

#[test]
fn the_clock_goes_round() {
    let mut clock = Clock { day: 3, hour: 23.9 };
    clock.advance(DAY_SECONDS / 24.0 * 0.2);
    assert!((clock.hour - 0.1).abs() < 1e-3);
    assert_eq!(clock.day, 4);
    assert_eq!(clock.time().day(), 4);
    assert!((clock.time().hour() - 0.1).abs() < 1e-3);
}

fn climate() -> WorldClimate {
    WorldClimate(crate::data::read(WorldClimate::FILE))
}

#[test]
fn the_clock_starts_on_the_sun_the_art_was_made_for_and_then_follows_the_year() {
    // The art was made under a sun 10° north of the equator (D315).
    let climate = climate();
    let first = climate.declination(0);
    assert!(
        (first.to_degrees() - 10.0).abs() < 0.1,
        "{}",
        first.to_degrees()
    );
    // Through the year the noon sun climbs higher than that, at midsummer, and lower,
    // at midwinter, and the morning sun stays where the art put it on the first day.
    let year = climate.0.year;
    let after = |phase: f64| {
        let days = ((phase - year.start).rem_euclid(1.0) * year.days) as i64;
        path().on(climate.declination(days)).height(12.0)
    };
    let first_noon = path().height(12.0);
    assert!(after(0.5) > first_noon + 10.0, "{}", after(0.5));
    assert!(after(0.0) < first_noon - 25.0, "{}", after(0.0));
}

#[test]
fn the_sun_warms_as_it_sinks_toward_a_black_bodys_colour() {
    // Below the art's 20 degrees the sun's light loses green and more blue the lower it
    // stands, so a meadow it lights turns olive rather than staying lawn green (settlement
    // r21: green 0.94 at 12 degrees left the end of the dusk film reading as afternoon).
    let mut last = light_at(20.0).light;
    for height in [16.0, 12.0, 8.0, 4.0, 0.0] {
        let light = light_at(height).light;
        assert!(
            light.y < last.y && light.z < last.z,
            "{height}: {light} after {last}"
        );
        assert!(light.z < light.y, "{height}: blue under green");
        last = light;
    }
    // A sun 12 degrees up is about 3,800 K: green near three quarters of red.
    let twelve = light_at(12.0).light;
    assert!(twelve.y / twelve.x < 0.82, "{twelve}");
}

#[test]
fn under_a_deck_the_light_comes_from_high_overhead_and_under_a_clear_sky_from_the_sun() {
    let up = Vec3::Y;
    let sun = Vec3::new(1.0, 0.25, 0.0).normalize();
    // A clear sky: the sun's own beam, from the sun.
    let clear = crate::weather::Under::sky(0.0, 0.0);
    let (direct, toward) = keyed(clear, 0.0, sun, up);
    assert_eq!(direct, clear.direct);
    assert!(toward.dot(sun) > 0.999);
    // A full deck in falling snow: the beam is all but gone, yet a key stays, from far higher
    // than the sun stands, and still on the sun's side.
    let snowfall = crate::weather::Under::sky(0.95, 0.8);
    let (direct, toward) = keyed(snowfall, crate::weather::deck_cover(0.95), sun, up);
    assert!(
        direct > 3.0 * snowfall.direct,
        "{direct} {}",
        snowfall.direct
    );
    assert!(
        direct < snowfall.total,
        "the key is a share of the deck's light"
    );
    assert!(toward.dot(up) > 0.8 && toward.x > 0.0, "{toward}");
}

#[test]
fn the_sun_casts_crisp_shadows_and_a_deck_soft_ones() {
    assert_eq!(
        light_size(0.0),
        0.0,
        "a clear sky keeps the sun's crisp edges"
    );
    let deck = light_size(crate::weather::deck_cover(0.95));
    assert!(deck > 10.0, "a full deck's key is a wide source: {deck}");
    assert!(light_size(crate::weather::deck_cover(0.5)) < deck);
}

#[test]
fn the_shade_keeps_its_colour_through_civil_twilight_and_loses_it_toward_nautical() {
    // How much of the midtones' colour the shade keeps with the sun so many degrees under.
    let shade = |height: f32| {
        let mut grade = bevy::render::view::ColorGrading::default();
        fade_colour(&mut grade, height, 1.0, crate::weather::Grade::NEUTRAL);
        grade.shadows.saturation / grade.midtones.saturation
    };
    // At sunset the shaded ground is some 25 cd/m², wholly in colour (photopic).
    assert!(shade(0.0) > 0.95, "sunset {}", shade(0.0));
    // Mid-way through civil twilight (about 0.2 cd/m², mid-mesopic) it keeps most of it.
    assert!(shade(-5.0) > 0.6, "civil dusk {}", shade(-5.0));
    // Into nautical twilight it has lost it, as the rods see.
    assert!(shade(-12.0) < 0.15, "nautical {}", shade(-12.0));
    // And it never gains colour as the light goes.
    let mut last = f32::MAX;
    for step in 0..=24 {
        let now = shade(-(step as f32) * 0.5);
        assert!(now <= last + 1e-5, "{} deg", -(step as f32) * 0.5);
        last = now;
    }
}

#[test]
fn a_closed_rooms_air_takes_none_of_the_skys_colour_and_the_open_air_all_of_it() {
    let haze = Color::srgba(0.55, 0.62, 0.5, 0.4);
    let shut = indoor_air(haze, 0.0).to_linear();
    assert_eq!((shut.red, shut.green, shut.blue), (0.0, 0.0, 0.0));
    // The alpha is how far the sun's glow reaches in the fog, not a brightness.
    assert!((shut.alpha - 0.4).abs() < 1e-6);
    let open = indoor_air(haze, 1.0).to_linear();
    assert!((open.green - haze.to_linear().green).abs() < 1e-6);
    // Half the sky's light is half the light, not half the sRGB value.
    let half = indoor_air(haze, 0.5).to_linear();
    assert!((half.green * 2.0 - open.green).abs() < 1e-6);
}
