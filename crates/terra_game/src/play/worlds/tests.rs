use super::*;

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
    make(&root, "World 1", false, 10).unwrap();
    make(&root, "world 1", true, 20).unwrap();
    make(&root, "Hill fort", false, 30).unwrap();
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
    make(&root, "World 1", false, 1).unwrap();
    make(&root, "World 3", false, 1).unwrap();
    assert_eq!(next_name(&list(&root), false), "World 2");
    assert_eq!(next_name(&list(&root), true), "Creative 1");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn deleting_a_world_takes_its_save_and_leaves_the_others() {
    let root = folder_for("delete");
    let doomed = make(&root, "Doomed", false, 1).unwrap();
    std::fs::write(doomed.save(), "(version: 2)").unwrap();
    make(&root, "Kept", false, 2).unwrap();
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
    let first = pick(&root, &choice(false, None), false, 1).unwrap();
    assert_eq!(
        (first.meta.name.as_str(), first.meta.creative),
        ("World 1", false)
    );
    // --creative: a new Creative world, then the same one again.
    let creative = pick(&root, &choice(true, None), false, 2).unwrap();
    assert_eq!(
        (creative.meta.name.as_str(), creative.meta.creative),
        ("Creative 1", true)
    );
    assert_eq!(
        pick(&root, &choice(true, None), false, 3).unwrap().dir,
        creative.dir
    );
    // Continue: the one played last, which is the Creative one now.
    assert_eq!(
        pick(&root, &choice(false, None), false, 4).unwrap().dir,
        creative.dir
    );
    // --fresh: always another.
    let fresh = pick(&root, &choice(false, None), true, 5).unwrap();
    assert_eq!(fresh.meta.name, "World 2");
    // --world=<name>: that one, or made.
    assert_eq!(
        pick(&root, &choice(false, Some("world 1")), false, 6)
            .unwrap()
            .dir,
        first.dir
    );
    let named = pick(&root, &choice(false, Some("Hill fort")), false, 7).unwrap();
    assert_eq!(named.meta.name, "Hill fort");
    assert_eq!(list(&root).len(), 4);
    std::fs::remove_dir_all(root).unwrap();
}
