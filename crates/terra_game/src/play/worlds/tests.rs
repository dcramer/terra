use super::*;

/// The seed the tests' worlds are made from, not the first, so a seed lost on the way shows.
const SEED: u64 = 0xDEC0_DE5E_ED00_0001;
/// The radius they are made at, neither the game's nor the test planet's, so a radius lost
/// on the way shows.
const RADIUS: f32 = 12_345.0;

/// An empty folder of its own for a test's worlds.
fn folder_for(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("terra-worlds-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn names(root: &Path) -> Vec<String> {
    list(root).into_iter().map(|w| w.meta.name).collect()
}

#[test]
fn a_new_world_never_replaces_another_and_the_last_played_comes_first() {
    let root = folder_for("list");
    make(&root, "World 1", false, SEED, RADIUS, 10).unwrap();
    make(&root, "world 1", true, SEED, RADIUS, 20).unwrap();
    make(&root, "Hill fort", false, SEED, RADIUS, 30).unwrap();
    assert_eq!(names(&root), ["Hill fort", "world 1", "World 1"]);
    let mut first = list(&root).pop().unwrap();
    first.played(40).unwrap();
    assert_eq!(names(&root), ["World 1", "Hill fort", "world 1"]);
    assert!(
        list(&root)[2].meta.creative,
        "the mode is kept with the world"
    );
    let dirs: Vec<_> = list(&root).iter().map(|w| w.dir.clone()).collect();
    assert_eq!(dirs.len(), 3, "each in its own folder");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_folder_name_is_the_name_plainly_and_never_one_taken() {
    let taken = ["hill-fort".to_string(), "Hill-Fort-2".to_string()];
    assert_eq!(folder("Hill fort", &[]), "hill-fort");
    assert_eq!(folder("  Hill   fort! ", &taken), "hill-fort-3");
    assert_eq!(folder("../..", &[]), "world");
    assert_eq!(folder("Ödland", &[]), "ödland");
}

#[test]
fn a_new_world_is_offered_the_first_name_not_taken() {
    let root = folder_for("names");
    assert_eq!(next_name(&list(&root), false), "World 1");
    make(&root, "World 1", false, SEED, RADIUS, 1).unwrap();
    make(&root, "World 3", false, SEED, RADIUS, 1).unwrap();
    assert_eq!(next_name(&list(&root), false), "World 2");
    assert_eq!(next_name(&list(&root), true), "Creative 1");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn deleting_a_world_takes_its_save_and_leaves_the_others() {
    let root = folder_for("delete");
    let doomed = make(&root, "Doomed", false, SEED, RADIUS, 1).unwrap();
    std::fs::write(doomed.save(), "(version: 2)").unwrap();
    make(&root, "Kept", false, SEED, RADIUS, 2).unwrap();
    doomed.delete().unwrap();
    assert_eq!(names(&root), ["Kept"]);
    assert!(!doomed.dir.exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn the_save_from_before_worlds_moves_untouched_into_the_first_world_once() {
    let root = folder_for("adopt");
    let old = root.with_extension("ron");
    std::fs::write(&old, "the old save").unwrap();
    let world = adopt(&root, &old, 5).unwrap().unwrap();
    assert_eq!(world.meta.name, "My world");
    assert!(!world.meta.creative);
    assert_eq!(
        std::fs::read_to_string(world.save()).unwrap(),
        "the old save"
    );
    assert!(!old.exists());
    // Never again, nor over worlds already there.
    std::fs::write(&old, "another").unwrap();
    assert!(adopt(&root, &old, 6).unwrap().is_none());
    assert_eq!(names(&root), ["My world"]);
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_file(old).unwrap();
}

#[test]
fn the_command_line_picks_the_world_the_title_screen_would() {
    let root = folder_for("pick");
    let choice = |creative: bool, world: Option<&str>| Choice {
        creative,
        world: world.map(str::to_string),
        ..Choice::default()
    };
    // Nothing yet: Continue makes the first world.
    let first = pick(&root, &choice(false, None), false, SEED, RADIUS, 1).unwrap();
    assert_eq!(
        (first.meta.name.as_str(), first.meta.creative),
        ("World 1", false)
    );
    // --creative: a new Creative world, then the same one again.
    let creative = pick(&root, &choice(true, None), false, SEED, RADIUS, 2).unwrap();
    assert_eq!(
        (creative.meta.name.as_str(), creative.meta.creative),
        ("Creative 1", true)
    );
    assert_eq!(
        pick(&root, &choice(true, None), false, SEED, RADIUS, 3)
            .unwrap()
            .dir,
        creative.dir
    );
    // Continue: the one played last, which is the Creative one now.
    assert_eq!(
        pick(&root, &choice(false, None), false, SEED, RADIUS, 4)
            .unwrap()
            .dir,
        creative.dir
    );
    // --fresh: always another.
    let fresh = pick(&root, &choice(false, None), true, SEED, RADIUS, 5).unwrap();
    assert_eq!(fresh.meta.name, "World 2");
    // --world=<name>: that one, or made.
    assert_eq!(
        pick(
            &root,
            &choice(false, Some("world 1")),
            false,
            SEED,
            RADIUS,
            6
        )
        .unwrap()
        .dir,
        first.dir
    );
    let named = pick(
        &root,
        &choice(false, Some("Hill fort")),
        false,
        SEED,
        RADIUS,
        7,
    )
    .unwrap();
    assert_eq!(named.meta.name, "Hill fort");
    assert_eq!(list(&root).len(), 4);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_world_keeps_the_seed_and_size_it_was_made_with_and_one_from_before_has_the_first() {
    let root = folder_for("seed");
    make(&root, "Seeded", false, SEED, RADIUS, 1).unwrap();
    let made = &list(&root)[0].meta;
    assert_eq!((made.seed, made.radius_m), (SEED, RADIUS));
    // Found again, it keeps its own, whatever a new world would be made from.
    let found = Choice {
        world: Some("Seeded".into()),
        ..Choice::default()
    };
    let again = pick(&root, &found, false, 7, 500.0, 2).unwrap().meta;
    assert_eq!((again.seed, again.radius_m), (SEED, RADIUS));
    // A world.ron written before worlds had seeds or sizes: the 2 km world it was made on.
    std::fs::create_dir_all(root.join("old")).unwrap();
    let before = "(name: \"Old\", creative: false, made: 1, played: 1)";
    std::fs::write(root.join("old").join(META), before).unwrap();
    let kept = list(&root);
    let old = kept.iter().find(|w| w.meta.name == "Old").unwrap();
    assert_eq!(old.meta.seed, FIRST_SEED);
    assert_eq!(old.meta.radius_m, 2_000.0);
    assert!(
        old.meta.starts.is_empty(),
        "one from before kept starts has none"
    );
    std::fs::remove_dir_all(root).unwrap();
}

/// The start a world's land found is kept in its folder the first time it is entered, read
/// back bit for bit, and taken by its land again only on a planet of its seed and radius, so
/// a world loaded again is not searched again and stands where it first did (D596).
#[test]
fn a_worlds_start_is_kept_once_and_taken_again_as_it_was() {
    use bevy::ecs::system::RunSystemOnce;
    use terra_core::{PlanetSpec, Seed};
    let root = folder_for("start");
    let world = make(&root, "Kept", false, SEED, RADIUS, 1).unwrap();
    let spec = PlanetSpec {
        seed: Seed(SEED),
        radius_m: 2_000.0,
    };
    let planet = Planet::new(spec);
    let mut app = bevy::ecs::world::World::new();
    app.insert_resource(Playing(world));
    app.insert_resource(planet.clone());
    app.run_system_once(keep_the_start).unwrap();
    app.run_system_once(keep_the_start).unwrap();
    let kept = list(&root).remove(0).meta;
    assert_eq!(kept.starts.len(), 1, "kept once: {:?}", kept.starts);
    let known = kept.known_start(spec.radius_m);
    assert_eq!(
        known.map(|known| known.direction),
        Some(planet.start_direction()),
        "read back as it was found"
    );
    assert_eq!(
        KnownStart::of(known.as_ref(), spec),
        Some(planet.start_direction())
    );
    let larger = PlanetSpec {
        radius_m: 30_000.0,
        ..spec
    };
    assert_eq!(
        KnownStart::of(known.as_ref(), larger),
        None,
        "another radius searches"
    );
    let other = PlanetSpec {
        seed: Seed(SEED + 1),
        ..spec
    };
    assert_eq!(
        KnownStart::of(known.as_ref(), other),
        None,
        "another seed searches"
    );
    let again = Planet::starting(spec, KnownStart::of(known.as_ref(), spec));
    assert_eq!(
        again.start(),
        planet.start(),
        "the land stands where it first did"
    );
    std::fs::remove_dir_all(root).unwrap();
}

/// The towns a world's land found round its start are kept in its folder once, read back bit
/// for bit and taken again only on a planet of its seed and radius; a world whose search found
/// none keeps that too, so neither searches again (D620). A site asked for is not kept.
#[test]
fn a_worlds_towns_are_kept_once_and_taken_again_as_they_were() {
    use bevy::ecs::system::RunSystemOnce;
    use terra_core::{PlanetSpec, Seed};
    let site = terra_land::town::TownSite {
        toward: Vec3::new(0.6, 0.0, 0.8),
        height_m: 412.5,
        growing_mean_c: 11.25,
        tree_line_m: 1890.0,
        relief_m: 6.5,
        slope_deg: 1.75,
        forest: 0.7,
        peak: Vec3::new(0.0, 0.6, 0.8),
        peak_m: 2510.0,
        peak_off_m: 4100.0,
        skyline_deg: 9.5,
        ringed: 0.4,
        start: Vec3::new(0.8, 0.0, 0.6),
        at_km: Vec2::new(-1.25, 2.5),
        water: Some(Vec2::new(250.0, -30.0)),
    };
    let spec = PlanetSpec {
        seed: Seed(SEED),
        radius_m: 2_000.0,
    };
    let planet = Planet::new(spec);
    for (name, towns) in [("towns", vec![site]), ("no-towns", Vec::new())] {
        let root = folder_for(name);
        let world = make(&root, "Kept", false, SEED, RADIUS, 1).unwrap();
        let mut app = bevy::ecs::world::World::new();
        app.insert_resource(Playing(world));
        app.insert_resource(planet.clone());
        app.insert_resource(Towns(towns.clone()));
        app.run_system_once(keep_the_start).unwrap();
        app.run_system_once(keep_the_start).unwrap();
        let kept = list(&root).remove(0).meta;
        assert_eq!(kept.towns.len(), 1, "kept once: {:?}", kept.towns);
        let known = kept.known_towns(spec.radius_m);
        assert_eq!(KnownTowns::of(known.as_ref(), spec), Some(&towns[..]));
        let larger = PlanetSpec {
            radius_m: 30_000.0,
            ..spec
        };
        assert_eq!(
            KnownTowns::of(known.as_ref(), larger),
            None,
            "another radius searches"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    // A site asked for (`--site town`) is the run's, not the world's.
    let root = folder_for("sited");
    let world = make(&root, "Kept", false, SEED, RADIUS, 1).unwrap();
    let mut app = bevy::ecs::world::World::new();
    app.insert_resource(Playing(world));
    app.insert_resource(planet.clone());
    app.insert_resource(Towns(vec![site]));
    app.insert_resource(Sited(terra_world::start::Found::Town(site)));
    app.run_system_once(keep_the_start).unwrap();
    assert!(list(&root).remove(0).meta.towns.is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

fn options(args: &[&str]) -> Options {
    Options::from_args(args.iter().map(|a| a.to_string())).unwrap()
}

/// What `choose` leaves the run's land at, with these flags and worlds kept in `root`.
fn chosen_radius_m(root: &Path, args: &[&str], title: bool, fresh: bool) -> f32 {
    use bevy::ecs::system::RunSystemOnce;
    let mut app = bevy::ecs::world::World::new();
    let mut options = options(args);
    options.fresh = fresh;
    app.insert_resource(options);
    app.insert_resource(Choice {
        title,
        worlds: Some(root.to_path_buf()),
        ..Choice::default()
    });
    app.run_system_once(choose).unwrap();
    app.resource::<Options>().radius_m
}

/// The game's world is D13's 30 km one (the developers, 2026-11-04): a world made from the
/// title screen or the command line is made at it unless `--radius` asks otherwise, and a
/// world kept is played at the size it was made, its save standing on it, whatever is asked.
#[test]
fn a_world_made_is_the_games_30_km_one_and_one_kept_plays_at_its_own_size() {
    let root = folder_for("size");
    assert_eq!(new_radius_m(&options(&[])), 30_000.0);
    assert_eq!(new_radius_m(&options(&["--radius", "8000"])), 8_000.0);
    // No world kept: the land behind the title screen is as New world makes it, and one
    // made from the command line is made so.
    assert_eq!(chosen_radius_m(&root, &[], true, false), 30_000.0);
    assert_eq!(chosen_radius_m(&root, &[], false, false), 30_000.0);
    assert_eq!(list(&root)[0].meta.radius_m, 30_000.0);
    std::fs::remove_dir_all(&root).unwrap();
    std::fs::create_dir_all(&root).unwrap();
    // A world kept at another size: Continue's land behind the title screen, and the world
    // played, are at its size, whatever --radius says; a new one beside it takes --radius.
    make(&root, "Small", false, SEED, RADIUS, 1).unwrap();
    assert_eq!(chosen_radius_m(&root, &[], true, false), RADIUS);
    assert_eq!(
        chosen_radius_m(&root, &["--radius", "30000"], false, false),
        RADIUS
    );
    assert_eq!(
        chosen_radius_m(&root, &["--radius", "9000"], false, true),
        9_000.0
    );
    // A run keeping no world at all stands on the test planet its tools are measured on.
    assert_eq!(options(&[]).radius_m, 2_000.0);
    std::fs::remove_dir_all(root).unwrap();
}
