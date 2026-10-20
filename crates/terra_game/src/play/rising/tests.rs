use super::*;

fn lapse() -> Timelapse {
    Timelapse::parse(
        "(seconds: 40.0, hours: (7.0, 17.0), first: 2.0, last: 26.0, takes: (6.0, 12.0), buildings: Some(3), common: (10.0, 30.0))",
    )
    .unwrap()
}

#[test]
fn the_nearest_buildings_go_up_first_and_the_biggest_take_longest() {
    let times = schedule(
        &lapse(),
        &[30.0, 5.0, 50.0, 12.0],
        &[400, 1600, 900, 100],
        4,
    );
    // Three of four, the furthest left for later.
    assert!(times[2].is_none());
    let (a, b, d) = (times[0].unwrap(), times[1].unwrap(), times[3].unwrap());
    assert!(b.0 < d.0 && d.0 < a.0, "{times:?}");
    assert_eq!(b.0, 2.0);
    assert_eq!(a.0, 26.0);
    assert!(b.1 > a.1 && a.1 > d.1, "{times:?}");
    assert_eq!(b.1, 12.0);
}

#[test]
fn a_walled_settlement_puts_up_its_wall_after_its_houses_and_its_castle_last() {
    let mut lapse = lapse();
    lapse.buildings = None;
    lapse.town = Some(Grown {
        buildings: (2, 2),
        streets: 1.0,
        walls: Some((28.0, 34.0)),
    });
    // Two houses, then a piece of wall nearer the square than the far one, and the castle.
    let times = schedule(
        &lapse,
        &[5.0, 60.0, 40.0, 70.0, 90.0],
        &[400, 900, 200, 200, 9000],
        2,
    );
    let start = |i: usize| times[i].expect("every building goes up").0;
    assert!(start(0) < start(1) && start(1) <= lapse.last);
    assert!(
        start(2) >= 28.0 && start(2) < start(3),
        "the wall from the square out after the houses"
    );
    assert_eq!(start(4), 34.0, "the castle last");
    assert!((2..4).all(|i| start(i) < start(4)));
}

#[test]
fn a_settlement_growing_starts_few_houses_at_first_and_more_later() {
    let mut lapse = lapse();
    lapse.buildings = None;
    lapse.growth = 0.6;
    let distances: Vec<f32> = (0..20).map(|i| i as f32).collect();
    let times = schedule(&lapse, &distances, &[100; 20], 20);
    let starts: Vec<f32> = times.iter().map(|t| t.unwrap().0).collect();
    // The gap after the first house is wider than the gaps near the end.
    assert!(
        starts[1] - starts[0] > 3.0 * (starts[19] - starts[18]),
        "{starts:?}"
    );
    assert_eq!(starts[0], lapse.first);
    assert!((starts[19] - lapse.last).abs() < 1e-4);
}

#[test]
fn a_time_lapse_without_people_shows_nobody_however_built() {
    let mut rising = Rising::new(lapse(), 0);
    rising.up = vec![true, false];
    assert!(rising.shows(true, Some(1)) && rising.shows(false, Some(0)));
    assert!(!rising.shows(false, Some(1)));
    assert!(rising.wildlife());
    rising.lapse = Timelapse::parse(
        "(seconds: 40.0, hours: (7.0, 17.0), first: 2.0, last: 26.0, takes: (6.0, 12.0), common: (10.0, 30.0), people: false)",
    )
    .unwrap();
    for (builder, home) in [(true, Some(1)), (false, Some(0)), (false, None)] {
        assert!(!rising.shows(builder, home), "{builder} {home:?}");
    }
    assert!(
        !rising.wildlife(),
        "no wild animals in a time-lapse of the buildings alone"
    );
}

#[test]
fn a_time_lapse_without_people_keeps_the_wild_animals_away_and_one_with_them_does_not() {
    use bevy::ecs::system::RunSystemOnce;
    let without = Timelapse::parse(
        "(seconds: 40.0, hours: (7.0, 17.0), first: 2.0, last: 26.0, takes: (6.0, 12.0), common: (10.0, 30.0), people: false)",
    )
    .unwrap();
    for (lapse, wild) in [(lapse(), true), (without, false)] {
        let mut world = World::new();
        let mut rising = Some(Rising::new(lapse, 0));
        world
            .run_system_once(move |mut commands: Commands| {
                start(&mut commands, rising.take().unwrap());
            })
            .unwrap();
        assert!(world.contains_resource::<Rising>());
        let withheld = world.contains_resource::<crate::play::wildlife::Withheld>();
        assert_eq!(withheld, !wild, "wild animals about: {wild}");
    }
}

#[test]
fn the_clock_runs_from_its_first_hour_to_its_last() {
    let lapse = lapse();
    assert_eq!(lapse.hour(-1.0), 7.0);
    assert_eq!(lapse.hour(20.0), 12.0);
    assert_eq!(lapse.hour(99.0), 17.0);
}

#[test]
fn every_time_lapse_file_reads() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/data/timelapse");
    let mut read = 0;
    for entry in std::fs::read_dir(dir).expect("the time-lapses") {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        Timelapse::parse(&text).unwrap_or_else(|why| panic!("{}: {why}", path.display()));
        read += 1;
    }
    assert!(read > 0);
}
