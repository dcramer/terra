use super::*;

/// The seed the tests' worlds are made from, not the first, so a seed lost on the way shows.
const SEED: u64 = 0xDEC0_DE5E_ED00_0001;

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
    make(&root, "World 1", false, SEED, 10).unwrap();
    make(&root, "world 1", true, SEED, 20).unwrap();
    make(&root, "Hill fort", false, SEED, 30).unwrap();
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
    make(&root, "World 1", false, SEED, 1).unwrap();
    make(&root, "World 3", false, SEED, 1).unwrap();
    assert_eq!(next_name(&list(&root), false), "World 2");
    assert_eq!(next_name(&list(&root), true), "Creative 1");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn deleting_a_world_takes_its_save_and_leaves_the_others() {
    let root = folder_for("delete");
    let doomed = make(&root, "Doomed", false, SEED, 1).unwrap();
    std::fs::write(doomed.save(), "(version: 2)").unwrap();
    make(&root, "Kept", false, SEED, 2).unwrap();
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
    let first = pick(&root, &choice(false, None), false, SEED, 1).unwrap();
    assert_eq!(
        (first.meta.name.as_str(), first.meta.creative),
        ("World 1", false)
    );
    // --creative: a new Creative world, then the same one again.
    let creative = pick(&root, &choice(true, None), false, SEED, 2).unwrap();
    assert_eq!(
        (creative.meta.name.as_str(), creative.meta.creative),
        ("Creative 1", true)
    );
    assert_eq!(
        pick(&root, &choice(true, None), false, SEED, 3)
            .unwrap()
            .dir,
        creative.dir
    );
    // Continue: the one played last, which is the Creative one now.
    assert_eq!(
        pick(&root, &choice(false, None), false, SEED, 4)
            .unwrap()
            .dir,
        creative.dir
    );
    // --fresh: always another.
    let fresh = pick(&root, &choice(false, None), true, SEED, 5).unwrap();
    assert_eq!(fresh.meta.name, "World 2");
    // --world=<name>: that one, or made.
    assert_eq!(
        pick(&root, &choice(false, Some("world 1")), false, SEED, 6)
            .unwrap()
            .dir,
        first.dir
    );
    let named = pick(&root, &choice(false, Some("Hill fort")), false, SEED, 7).unwrap();
    assert_eq!(named.meta.name, "Hill fort");
    assert_eq!(list(&root).len(), 4);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_world_keeps_the_seed_it_was_made_from_and_one_from_before_seeds_has_the_first() {
    let root = folder_for("seed");
    make(&root, "Seeded", false, SEED, 1).unwrap();
    assert_eq!(list(&root)[0].meta.seed, SEED);
    // Found again, it keeps its own, whatever a new world would be made from.
    let found = Choice {
        world: Some("Seeded".into()),
        ..Choice::default()
    };
    assert_eq!(pick(&root, &found, false, 7, 2).unwrap().meta.seed, SEED);
    // A world.ron written before worlds had seeds.
    std::fs::create_dir_all(root.join("old")).unwrap();
    let before = "(name: \"Old\", creative: false, made: 1, played: 1)";
    std::fs::write(root.join("old").join(META), before).unwrap();
    let kept = list(&root);
    let old = kept.iter().find(|w| w.meta.name == "Old").unwrap();
    assert_eq!(old.meta.seed, FIRST_SEED);
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
    let world = make(&root, "Kept", false, SEED, 1).unwrap();
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
