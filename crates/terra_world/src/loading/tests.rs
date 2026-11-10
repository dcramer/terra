use super::*;

#[test]
fn counts_everything_alike() {
    let mut progress = Progress::default();
    progress.report("Loading", 1, 1);
    progress.report("Loading", 0, 2);
    progress.report("Preparing shaders", 3, 3);
    assert_eq!(progress.coming(), 4);
    assert_eq!(progress.fraction(), 0.6);
    assert_eq!(progress.status(), "Preparing shaders");
    let mut compiling = Progress::default();
    compiling.report("Loading", 0, 4);
    compiling.report("Preparing shaders", 1, 5);
    assert_eq!(compiling.status(), "Preparing shaders");
    assert_eq!(compiling.fraction(), 0.9);
    let nothing = Progress::default();
    assert_eq!(nothing.coming(), 0);
    assert_eq!(nothing.fraction(), 1.0);
    assert_eq!(nothing.status(), "Ready");
}

#[test]
fn reports_of_one_thing_add_up() {
    let mut progress = Progress::default();
    progress.report("Loading", 2, 1);
    progress.report("Preparing shaders", 4, 0);
    progress.report("Loading", 3, 0);
    // Five loading in all hold back more than four shaders.
    assert_eq!(progress.status(), "Loading");
    assert_eq!(
        progress.summary(),
        "Loading 1 of 6, Preparing shaders 0 of 4"
    );
}

/// The loading screen as `cover` puts it up, with no frame yet watched.
fn screen() -> World {
    screen_for(&[])
}

/// The screen for a run given these options.
fn screen_for(args: &[&str]) -> World {
    let mut world = World::new();
    world.insert_resource(Loading::default());
    world.insert_resource(Progress::default());
    world.insert_resource(Drawables::default());
    let mut real = Time::<Real>::default();
    real.update_with_duration(Duration::ZERO);
    world.insert_resource(real);
    world.insert_resource(Options::from_args(args.iter().map(|a| a.to_string())).unwrap());
    world.insert_resource(NextState::<Screen>::default());
    world.spawn((Bar, Node::default()));
    world.spawn((Status, Text::new("Loading")));
    world
}

/// One frame of so many seconds, with so much said to be coming and done. Whether the
/// game has been told to start, and how far the bar is.
fn frame(world: &mut World, coming: usize, done: usize, seconds: f32) -> (bool, Val) {
    world
        .resource_mut::<Time<Real>>()
        .update_with_duration(Duration::from_secs_f32(seconds));
    world
        .resource_mut::<Progress>()
        .report("Loading", coming, done);
    world.run_system_cached(watch).unwrap();
    let started = matches!(
        world.resource::<NextState<Screen>>(),
        NextState::Pending(Screen::Playing)
    );
    let bar = world
        .query_filtered::<&Node, With<Bar>>()
        .single(world)
        .unwrap()
        .width;
    (started, bar)
}

/// The screen `cover` puts up, at the start or over a window gone dark for a crossing.
fn covered(crossing: bool) -> World {
    let mut world = screen();
    if crossing {
        let to = WorldSpec {
            key: crate::instance::InstanceKey(7),
            kind: "tundra".into(),
            seed: 7,
        };
        cross_in_the_dark(&mut world.commands(), to);
    }
    world.flush();
    world.run_system_cached(cover).unwrap();
    world.flush();
    world
}

/// The visibility of the screen's title, and of its ring, bar and status.
fn shown(world: &mut World) -> (Visibility, Vec<Visibility>) {
    let mut title =
        world.query_filtered::<&Visibility, (With<Text>, Without<Status>, With<Fades>)>();
    let title = *title.single(world).unwrap();
    let mut ring = world.query_filtered::<&ChildOf, With<Spinner>>();
    let ring = ring.iter(world).next().unwrap().parent();
    let mut bar = world.query_filtered::<&ChildOf, (With<Bar>, With<Fades>)>();
    let bar = bar.single(world).unwrap().parent();
    let mut status = world.query_filtered::<Entity, (With<Status>, With<Fades>)>();
    let status = status.single(world).unwrap();
    let parts = [ring, bar, status]
        .map(|part| *world.get::<Visibility>(part).unwrap())
        .to_vec();
    (title, parts)
}

/// A frame of the screen's own of so many seconds.
fn wait(world: &mut World, seconds: f32) {
    world
        .resource_mut::<Time<Real>>()
        .update_with_duration(Duration::from_secs_f32(seconds));
    world.run_system_cached(show_late).unwrap();
    world.flush();
}

/// The start shows the title, the ring, the bar and what it waits for at once. A crossing
/// (D501) is the dark alone while it is short, and shows all but the title once it is not.
#[test]
fn a_crossing_shows_what_it_waits_for_only_once_it_is_long() {
    let mut start = covered(false);
    let all = vec![Visibility::Inherited; 3];
    assert_eq!(shown(&mut start), (Visibility::Inherited, all.clone()));
    wait(&mut start, 5.0);
    assert_eq!(shown(&mut start), (Visibility::Inherited, all.clone()));

    let mut crossing = covered(true);
    let none = vec![Visibility::Hidden; 3];
    assert_eq!(shown(&mut crossing), (Visibility::Hidden, none.clone()));
    wait(&mut crossing, SHOW_AFTER_S * 0.5);
    assert_eq!(shown(&mut crossing), (Visibility::Hidden, none), "short");
    wait(&mut crossing, SHOW_AFTER_S * 0.6);
    assert_eq!(shown(&mut crossing), (Visibility::Hidden, all), "long");
}

/// The ring turns: one dot is brightest, and which one goes round with the clock.
#[test]
fn the_ring_turns_with_the_real_clock() {
    let brightest = |turns: f32| {
        (0..DOTS)
            .max_by(|a, b| dot_alpha(*a, turns).total_cmp(&dot_alpha(*b, turns)))
            .unwrap()
    };
    assert_eq!(brightest(0.0), 0);
    assert_eq!(brightest(0.31), 3);
    assert_eq!(brightest(1.95), 9);
    assert!((0..DOTS).all(|dot| (0.12..=1.0).contains(&dot_alpha(dot, 0.31))));
    // Through the system: a frame later the dots' colours have changed.
    let mut world = covered(false);
    let alphas = |world: &mut World| -> Vec<f32> {
        world.run_system_cached(spin).unwrap();
        let mut dots = world.query::<(&Spinner, &BackgroundColor)>();
        let mut dots: Vec<_> = dots
            .iter(world)
            .map(|(dot, c)| (dot.0, c.0.alpha()))
            .collect();
        dots.sort_by_key(|dot| dot.0);
        dots.into_iter().map(|dot| dot.1).collect()
    };
    let before = alphas(&mut world);
    assert_eq!(before.len(), DOTS);
    world
        .resource_mut::<Time<Real>>()
        .update_with_duration(Duration::from_secs_f32(0.25));
    assert_ne!(before, alphas(&mut world));
}

/// The window goes dark, then the world is crossed, once.
#[test]
fn the_window_goes_dark_then_crosses() {
    use crate::instance::{InstanceKey, Travel};
    let mut world = screen_for(&["--shot", "x.png"]);
    world.insert_resource(NextState::<Travel>::default());
    let to = WorldSpec {
        key: InstanceKey(7),
        kind: "tundra".into(),
        seed: 7,
    };
    cross_in_the_dark(&mut world.commands(), to.clone());
    world.flush();
    world.run_system_cached(darken).unwrap();
    world.flush();
    assert_eq!(world.get_resource::<WorldSpec>(), Some(&to));
    world.remove_resource::<WorldSpec>();
    world.run_system_cached(darken).unwrap();
    world.flush();
    assert!(!world.contains_resource::<WorldSpec>(), "crossed once");
}

#[test]
fn the_game_starts_once_nothing_has_come_for_some_frames_in_a_row() {
    let mut world = screen();
    assert!(!frame(&mut world, 2, 0, 0.1).0);
    for _ in 1..QUIET_FRAMES {
        assert!(!frame(&mut world, 0, 2, 0.1).0);
    }
    // Something more comes just before: the quiet starts again.
    assert!(!frame(&mut world, 1, 2, 0.1).0);
    for _ in 1..QUIET_FRAMES {
        assert!(!frame(&mut world, 0, 3, 0.1).0);
    }
    assert!(frame(&mut world, 0, 3, 0.1).0);
}

#[test]
fn a_later_load_is_timed_from_when_it_began_not_from_the_start() {
    let mut world = screen();
    // The first load ends after 100 s of frames, and the title screen waits.
    for _ in 0..=QUIET_FRAMES {
        frame(&mut world, 0, 1, 10.0);
    }
    let startup = world.resource::<Time<Real>>().startup();
    let first = Duration::from_secs(100);
    let took = |world: &World, now| {
        world
            .resource::<Loading>()
            .took(world.resource::<Time<Real>>(), now)
    };
    assert!(world.resource::<Loading>().ended_once);
    assert_eq!(
        took(&world, startup + first),
        first,
        "the first from the start"
    );
    // A world is chosen: the screen goes up again, and loads for 3 s.
    world.run_system_cached(cover).unwrap();
    let began = world
        .resource::<Loading>()
        .began
        .expect("timed from the choice");
    let three = Duration::from_secs(3);
    assert_eq!(took(&world, began + three), three);
}

/// A crossing is timed from leaving the last world: the next world's first frames,
/// before its screen goes up, are part of the wait as the player sees it. A later load
/// chosen on the title screen is timed from its screen again.
#[test]
fn a_crossing_is_timed_from_leaving_not_from_the_screen() {
    let mut world = screen();
    for _ in 0..=QUIET_FRAMES {
        frame(&mut world, 0, 1, 10.0);
    }
    world.run_system_cached(leaving).unwrap();
    let left = world
        .resource::<Loading>()
        .began
        .expect("timed from leaving");
    // The next world's first frames take a while before the screen goes up.
    std::thread::sleep(Duration::from_millis(2));
    world.run_system_cached(cover).unwrap();
    assert_eq!(world.resource::<Loading>().began, Some(left));
    // The screen's own bar and words away, so the frames watch this test's.
    let put_up: Vec<Entity> = world
        .query_filtered::<Entity, With<Cover>>()
        .iter(&world)
        .collect();
    for entity in put_up {
        world.despawn(entity);
    }
    for _ in 0..=QUIET_FRAMES {
        frame(&mut world, 0, 1, 0.1);
    }
    assert!(
        !world.resource::<Loading>().timed,
        "the next load timed afresh"
    );
    world.run_system_cached(cover).unwrap();
    assert!(world.resource::<Loading>().began > Some(left));
}

#[test]
fn the_bar_never_goes_back() {
    let mut world = screen();
    assert_eq!(frame(&mut world, 1, 3, 0.1).1, percent(75.0));
    // More was found to be coming: the bar waits rather than falling back.
    assert_eq!(frame(&mut world, 3, 1, 0.1).1, percent(75.0));
    assert_eq!(frame(&mut world, 0, 4, 0.1).1, percent(100.0));
}

#[test]
fn the_game_starts_anyway_when_something_never_loads() {
    let mut world = screen();
    let half = GIVE_UP.as_secs_f32() / 2.0;
    assert!(!frame(&mut world, 1, 0, half).0);
    assert!(!frame(&mut world, 1, 0, half).0);
    assert!(frame(&mut world, 1, 0, 0.1).0);
}

/// A scripted run's pictures wait for everything far longer than a player would, so a
/// busy machine does not film pines with no trunks; it still starts in the end.
#[test]
fn a_scripted_run_waits_far_longer_for_what_never_loads() {
    let mut world = screen_for(&["--shot", "out.png"]);
    let step = GIVE_UP.as_secs_f32();
    assert!(!frame(&mut world, 1, 0, step).0);
    assert!(
        !frame(&mut world, 1, 0, step).0,
        "gave up at the player's cap"
    );
    let rest = GIVE_UP_SCRIPTED.as_secs_f32() - 2.0 * step;
    assert!(!frame(&mut world, 1, 0, rest - 1.0).0);
    assert!(frame(&mut world, 1, 0, 2.0).0, "never gave up");
}

/// What cannot be drawn yet is named, by the nearest name it hangs from, and counted.
#[test]
fn what_is_waiting_is_named() {
    let mut world = World::new();
    let tree = world.spawn(Name::new("Pine")).id();
    let bark = world.spawn(ChildOf(tree)).id();
    let bark2 = world.spawn(ChildOf(tree)).id();
    let chunk = world.spawn(Name::new("Chunk")).id();
    let drawables = Drawables {
        waiting: 4,
        ready: 10,
        missing: vec![
            (bark, "its material"),
            (bark2, "its material"),
            (chunk, "its mesh"),
        ],
    };
    let said = world
        .run_system_cached_with(
            |In(d): In<Drawables>, names: Query<(Option<&Name>, Option<&ChildOf>)>| {
                waiting_for(&d, &names)
            },
            drawables,
        )
        .unwrap();
    assert_eq!(
        said,
        "4 things in view: part of Pine (its material) x2, Chunk (its mesh) x1 and 1 more"
    );
}
