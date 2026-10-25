use super::*;
use terra_climate::{Climate, Date, Year};

fn climate() -> WorldClimate {
    WorldClimate(terra_world::data::read::<Climate>(WorldClimate::FILE))
}

fn year() -> Year {
    climate().0.year
}

/// A walk's app with its keys and the clock they move, and nothing to draw.
fn app(day: i64, hour: f32) -> App {
    let mut app = App::new();
    app.insert_resource(ButtonInput::<KeyCode>::default())
        .insert_resource(Clock { day, hour })
        .insert_resource(climate())
        .init_resource::<Walk>()
        .add_systems(
            Update,
            (
                season.run_if(input_just_pressed(SEASON)),
                time_of_day.run_if(input_just_pressed(TIME_OF_DAY)),
                earlier.run_if(input_just_pressed(EARLIER)),
                later.run_if(input_just_pressed(LATER)),
                toggle.run_if(input_just_pressed(PANEL)),
            ),
        );
    app
}

fn press(app: &mut App, key: KeyCode) -> Clock {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release(key);
    keys.clear();
    *app.world().resource::<Clock>()
}

#[test]
fn the_season_key_flips_high_summer_and_a_january_snowy_spell_keeping_the_hour() {
    let year = year();
    let summer = Date::parse(rules::SUMMER).unwrap().day(&year);
    let mut app = app(summer, 12.0);
    let winter = press(&mut app, SEASON);
    assert_eq!(Date::calendar(winter.day, &year), rules::WINTER);
    assert_eq!(rules::season(&year, winter.day), "winter");
    // Seasons by the months, as a forecast names them.
    for (date, season) in [
        ("12-01", "winter"),
        ("02-28", "winter"),
        ("03-01", "spring"),
        ("06-01", "summer"),
        ("08-31", "summer"),
        ("09-01", "autumn"),
        ("11-30", "autumn"),
    ] {
        let day = Date::parse(date).unwrap().day(&year);
        assert_eq!(rules::season(&year, day), season, "{date}");
    }
    assert_eq!(winter.hour, 12.0);
    let back = press(&mut app, SEASON);
    assert_eq!(back.day, summer);
    assert_eq!(rules::season(&year, back.day), "summer");
    // Autumn and spring go to the nearer extreme's other side as a person expects.
    assert_eq!(
        rules::flipped(&year, Date::parse("10-15").unwrap().day(&year)),
        summer
    );
    assert_eq!(
        rules::flipped(&year, Date::parse("05-01").unwrap().day(&year)),
        winter.day
    );
}

#[test]
fn the_time_key_steps_dawn_noon_dusk_night_and_round_on_the_same_day() {
    let year = year();
    let day = Date::parse(rules::WINTER).unwrap().day(&year);
    let mut app = app(day, 12.0);
    let declination = climate().declination(day);
    let height = |hour: f32| terra_climate::sun_height(LATITUDE, declination, hour).to_degrees();
    let dusk = press(&mut app, TIME_OF_DAY);
    assert!(
        dusk.hour > 12.0 && (height(dusk.hour) - 2.0).abs() < 0.05,
        "{dusk:?}"
    );
    let night = press(&mut app, TIME_OF_DAY);
    assert_eq!(night.hour, 23.0);
    let dawn = press(&mut app, TIME_OF_DAY);
    assert!(
        dawn.hour < 12.0 && (height(dawn.hour) - 4.0).abs() < 0.05,
        "{dawn:?}"
    );
    let noon = press(&mut app, TIME_OF_DAY);
    assert_eq!(noon.hour, 12.0);
    assert!([dusk, night, dawn, noon].iter().all(|c| c.day == day));
    // A winter's day is shorter than a summer's: dusk comes earlier.
    let summer = rules::times_of_day(
        LATITUDE,
        climate().declination(Date::parse(rules::SUMMER).unwrap().day(&year)),
    );
    assert!(summer[2].1 > dusk.hour + 1.5, "{summer:?} {dusk:?}");
}

#[test]
fn the_hour_keys_step_an_hour_through_midnight() {
    let mut app = app(10, 23.5);
    let on = press(&mut app, LATER);
    assert_eq!((on.day, (on.hour * 100.0).round() / 100.0), (11, 0.5));
    let back = press(&mut app, EARLIER);
    assert_eq!((back.day, (back.hour * 100.0).round() / 100.0), (10, 23.5));
}

#[test]
fn the_panel_key_hides_and_shows_it_and_the_panel_lists_the_keys_and_the_moment() {
    let mut app = app(0, 12.0);
    assert!(app.world().resource::<Walk>().panel);
    press(&mut app, PANEL);
    assert!(!app.world().resource::<Walk>().panel);
    press(&mut app, PANEL);
    let walk = app.world().resource::<Walk>();
    assert!(walk.panel);
    let shown = panel::text_of(walk, "winter 01-15 (day 143) 17:02 snowfall", 0.85);
    for wanted in [
        "F6", "F7", "F8", "J L", "P ", "O ", "01-15", "17:02", "snowfall", "85%",
    ] {
        assert!(shown.contains(wanted), "{wanted} not in {shown}");
    }
}

fn names() -> Vec<String> {
    ["overcast", "rain", "snowfall"].map(String::from).to_vec()
}

#[test]
fn a_weather_is_called_by_the_last_named_it_is_or_clear_or_cloudy() {
    let names = names();
    assert_eq!(rules::called(&names, |i| i <= 1, 1.0, 1.0), "rain");
    assert_eq!(rules::called(&names, |_| false, 0.2, 0.0), "clear");
    assert_eq!(rules::called(&names, |_| false, 0.7, 0.0), "cloudy");
}

#[test]
fn the_weather_key_goes_to_the_next_weather_on_the_nearest_day_passing_over_the_rest() {
    let cycle: Vec<String> = std::iter::once("clear".to_string())
        .chain(names())
        .collect();
    let days: Vec<(i64, String)> = rules::nearest_days()
        .map(|on| {
            let called = match on {
                -2 | 3 => "overcast",
                5 => "snowfall",
                _ => "clear",
            };
            (on, called.to_string())
        })
        .collect();
    // The nearer of two overcast days, back as readily as on.
    let next = |now: &str| rules::next_weather(&cycle, now, &days);
    assert_eq!(next("clear"), Some(("overcast".into(), -2, vec![])));
    // No rain in reach: on to the snowfall, saying the rain was passed over.
    assert_eq!(
        next("overcast"),
        Some(("snowfall".into(), 5, vec!["rain".into()]))
    );
    // Round the cycle from the last: today is clear.
    assert_eq!(next("snowfall"), Some(("clear".into(), 0, vec![])));
    // Cloudy is not in the cycle: from it, the cycle starts again.
    assert_eq!(next("cloudy"), Some(("overcast".into(), -2, vec![])));
    let same: Vec<(i64, String)> = rules::nearest_days()
        .map(|on| (on, "clear".into()))
        .collect();
    assert_eq!(rules::next_weather(&cycle, "clear", &same), None);
    assert_eq!(same.len() as i64, 2 * rules::LOOKS_DAYS + 1);
}

#[test]
fn feedback_is_numbered_on_from_the_highest_in_its_folder() {
    assert_eq!(rules::next_number([].into_iter()), 1);
    let names = [
        "1-20261024-120000.png",
        "3-20261024-120500.json",
        "notes.txt",
        "2-x.png",
    ];
    assert_eq!(rules::next_number(names.into_iter()), 4);
}

#[test]
fn the_walk_stands_for_play_and_the_lush_forest_and_a_runs_own_options_win() {
    let args = ["--experiment", "forest-walk", "--time", "17"]
        .map(String::from)
        .to_vec();
    let expanded = crate::experiments::expanded(args);
    let options = terra_world::shell::Options::from_args(expanded.into_iter()).unwrap();
    assert_eq!(options.experiments, ["play", "lush", "forest-walk"]);
    assert_eq!(options.time, Some(17.0));
    assert!(options.yard());
    assert_eq!(rules::clock_text(17.05), "17:03");
}
