use super::*;
use std::path::{Path, PathBuf};
use terra_world::choice::Choice;

fn folder_for(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("terra-title-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Presses Enter on the item, wherever it is on the page.
fn choose(menu: &mut Menu, item: Item) -> Option<Act> {
    menu.at = menu.items().iter().position(|&i| i == item).unwrap();
    menu.choose()
}

/// What the game does with a choice of world, as the title screen's systems do.
fn world_of(root: &Path, act: Act) -> worlds::World {
    match act {
        Act::Play(world) => world,
        Act::Make {
            name,
            creative,
            seed,
        } => worlds::make(root, &name, creative, seed, 30_000.0, 100).unwrap(),
        other => panic!("not a world: {other:?}"),
    }
}

#[test]
fn the_title_screens_choices_play_the_worlds_the_flags_do() {
    let root = folder_for("flags");
    worlds::make(&root, "Old", false, 1, 30_000.0, 1).unwrap();
    worlds::make(&root, "Last", false, 1, 30_000.0, 2).unwrap();
    // Continue: the world `cargo xtask play continue` (no flag) plays.
    let mut menu = Menu::new(worlds::list(&root));
    let continued = world_of(&root, choose(&mut menu, Item::Continue).unwrap());
    let flagged = worlds::pick(&root, &Choice::default(), false, 1, 30_000.0, 3).unwrap();
    assert_eq!(
        (continued.dir, continued.meta.creative),
        (flagged.dir, false)
    );
    // New world, Survival: another world, as `--fresh`, never one replaced.
    let mut menu = Menu::new(worlds::list(&root));
    assert_eq!(choose(&mut menu, Item::NewWorld), None);
    let survival = world_of(&root, choose(&mut menu, Item::Create).unwrap());
    assert_eq!(
        (survival.meta.name.as_str(), survival.meta.creative),
        ("World 1", false)
    );
    // New world, Creative: a creative world, the one `--creative` then plays.
    let mut menu = Menu::new(worlds::list(&root));
    choose(&mut menu, Item::NewWorld);
    assert_eq!(choose(&mut menu, Item::Mode), None);
    let creative = world_of(&root, choose(&mut menu, Item::Create).unwrap());
    assert_eq!(
        (creative.meta.name.as_str(), creative.meta.creative),
        ("Creative 1", true)
    );
    let flagged = worlds::pick(
        &root,
        &Choice {
            creative: true,
            ..Choice::default()
        },
        false,
        1,
        30_000.0,
        200,
    );
    assert_eq!(flagged.unwrap().dir, creative.dir);
    // The yard, Settings and Quit are the game's to do.
    let mut menu = Menu::new(worlds::list(&root));
    assert_eq!(choose(&mut menu, Item::Yard), Some(Act::Yard));
    assert_eq!(choose(&mut menu, Item::Settings), Some(Act::Settings));
    assert_eq!(choose(&mut menu, Item::Quit), Some(Act::Quit));
    assert_eq!(worlds::list(&root).len(), 4, "nothing was replaced");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn continue_and_the_list_cannot_be_chosen_with_no_world() {
    let mut menu = Menu::new(Vec::new());
    assert_eq!(menu.picked(), Some(Item::NewWorld));
    assert_eq!(choose(&mut menu, Item::Continue), None);
    assert_eq!(menu.page, Page::Main);
    menu.at = 2;
    menu.step(false);
    assert_eq!(
        menu.picked(),
        Some(Item::Quit),
        "stepping up passes over both, wrapping"
    );
}

#[test]
fn deleting_a_world_asks_first_and_back_keeps_it() {
    let root = folder_for("delete");
    worlds::make(&root, "Keep me", false, 1, 30_000.0, 1).unwrap();
    let mut menu = Menu::new(worlds::list(&root));
    choose(&mut menu, Item::Worlds);
    menu.ask_delete(0);
    assert_eq!(menu.page, Page::Delete(0));
    assert_eq!(menu.picked(), Some(Item::Back), "No is picked first");
    assert_eq!(menu.choose(), None);
    assert_eq!(menu.page, Page::Worlds);
    menu.ask_delete(0);
    let Some(Act::Delete(world)) = choose(&mut menu, Item::ConfirmDelete) else {
        panic!("Yes deletes");
    };
    assert_eq!(world.meta.name, "Keep me");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn the_name_is_typed_and_follows_the_mode_until_typed() {
    let mut menu = Menu::new(Vec::new());
    choose(&mut menu, Item::NewWorld);
    assert_eq!(menu.name, "World 1");
    menu.turn();
    assert_eq!(menu.name, "Creative 1");
    menu.at = 0;
    for _ in 0..10 {
        menu.erase();
    }
    assert!(!menu.enabled(Item::Create), "no name, no world");
    menu.type_text("Hill fort\n");
    menu.turn();
    assert_eq!((menu.name.as_str(), menu.creative), ("Hill fort", false));
}

/// What `worlds::choose` puts behind the title screen with the worlds in `root`: the land's
/// seed, and the dice New world rolls from.
fn behind(root: &Path) -> (Option<u64>, worlds::Dice) {
    use bevy::ecs::system::RunSystemOnce;
    let mut app = bevy::ecs::world::World::new();
    app.insert_resource(crate::shell::Options::from_args(std::iter::empty()).unwrap());
    app.insert_resource(Choice {
        title: true,
        worlds: Some(root.to_path_buf()),
        ..Choice::default()
    });
    app.run_system_once(worlds::choose).unwrap();
    let seed = app.resource::<crate::shell::Options>().seed;
    (seed, *app.resource::<worlds::Dice>())
}

/// A first launch loads its world once (D621): with no world kept, the land behind the title
/// screen is the world New world first offers, so Create plays on it, as `act` plays a world
/// of the land's seed and size without crossing; with a world kept, it is Continue's.
#[test]
fn with_no_world_kept_new_world_first_offers_the_land_behind_the_title() {
    let root = folder_for("first-launch");
    let (seed, dice) = behind(&root);
    let mut menu = Menu::new(worlds::list(&root));
    menu.dice = dice.0;
    choose(&mut menu, Item::NewWorld);
    assert_eq!(menu.seed(), seed);
    let Some(Act::Make { seed: made, .. }) = choose(&mut menu, Item::Create) else {
        panic!("Create makes a world");
    };
    assert_eq!(Some(made), seed);
    // Another roll offers another world, which crosses to its own land.
    choose(&mut menu, Item::Seed);
    assert_ne!(menu.seed(), seed);
    // A world kept: Continue's land behind, whatever the dice.
    worlds::make(&root, "Kept", false, 5, 30_000.0, 1).unwrap();
    assert_eq!(behind(&root).0, Some(5));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn new_world_rolls_a_seed_kept_in_its_world_and_takes_one_typed() {
    let root = folder_for("seeds");
    let mut menu = Menu::new(Vec::new());
    menu.dice = 7;
    choose(&mut menu, Item::NewWorld);
    let rolled = menu.seed().expect("a seed rolled");
    // Each roll another: Enter on the seed, staying on the page.
    assert_eq!(choose(&mut menu, Item::Seed), None);
    assert_ne!(menu.seed().unwrap(), rolled);
    assert_eq!(menu.page, Page::New);
    // Typed over a rolled seed, digits alone, as far as a 64-bit number goes.
    menu.type_text("4x2");
    assert_eq!(menu.seed(), Some(42));
    menu.type_text(&"9".repeat(30));
    assert!(menu.seed.len() <= 20, "{}", menu.seed);
    assert!(menu.seed().is_some(), "{}", menu.seed);
    for _ in 0..30 {
        menu.erase();
    }
    assert!(!menu.enabled(Item::Create), "no seed, no world");
    menu.type_text("1234");
    // Kept in the world made, which the list then shows.
    let made = world_of(&root, choose(&mut menu, Item::Create).unwrap());
    assert_eq!(made.meta.seed, 1234);
    assert_eq!(worlds::list(&root)[0].meta.seed, 1234);
    // A rolled seed goes whole at the first Backspace.
    menu.back();
    choose(&mut menu, Item::NewWorld);
    menu.at = menu.items().iter().position(|&i| i == Item::Seed).unwrap();
    menu.erase();
    assert_eq!(menu.seed, "");
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn hosting_needs_a_world_and_a_name_and_joining_a_name_and_an_address() {
    let root = folder_for("host");
    let menu = Menu::new(Vec::new());
    assert!(!menu.enabled(Item::Host), "no world to host");
    let world = worlds::make(&root, "Home", false, 7, 30_000.0, 1).unwrap();
    let mut menu = Menu::new(vec![world.clone()]);
    assert_eq!(choose(&mut menu, Item::Host), None);
    assert_eq!(menu.page, Page::Host);
    // With no name the world cannot be hosted; typed, it can.
    assert_eq!(choose(&mut menu, Item::HostWorld(0)), None);
    menu.at = 0;
    menu.type_text("Ada");
    assert_eq!(
        choose(&mut menu, Item::HostWorld(0)),
        Some(Act::Host(world))
    );

    menu.back();
    assert_eq!(choose(&mut menu, Item::Join), None);
    assert_eq!(menu.picked(), Some(Item::Address));
    assert!(!menu.enabled(Item::Connect), "no address yet");
    // An address takes what one is written in, no spaces.
    menu.type_text(" 203.0.113.7 :4/7");
    assert_eq!(menu.address, "203.0.113.7:47");
    menu.erase();
    menu.erase();
    menu.erase();
    let asked = choose(&mut menu, Item::Connect);
    assert_eq!(
        asked,
        Some(Act::Join {
            address: "203.0.113.7".into()
        })
    );
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn while_a_host_is_yet_to_answer_only_back_is_chosen_and_it_gives_up() {
    let mut menu = Menu::new(Vec::new());
    (menu.player, menu.address) = ("Ada".into(), "203.0.113.7".into());
    choose(&mut menu, Item::Join);
    menu.joining = Some("Asking the host".into());
    assert_eq!(choose(&mut menu, Item::Connect), None, "asked once");
    menu.at = 1;
    menu.type_text("9");
    assert_eq!(menu.address, "203.0.113.7", "nothing is typed meanwhile");
    assert_eq!(choose(&mut menu, Item::Back), Some(Act::Cancel));
    assert_eq!((menu.page, &menu.joining), (Page::Join, &None), "and stays");
    // What the screen told is gone once the player next chooses.
    menu.notice = Some("No answer".into());
    choose(&mut menu, Item::Address);
    assert_eq!(menu.notice, None);
}

/// A name is no longer than the wire carries, by bytes, never cut inside a letter.
#[test]
fn a_players_name_is_held_to_what_the_wire_carries() {
    let mut menu = Menu::new(Vec::new());
    choose(&mut menu, Item::Join);
    menu.at = 0;
    menu.type_text(&"é".repeat(40));
    assert_eq!(menu.player.len(), PLAYER_BYTES);
    assert_eq!(menu.player.chars().count(), 16);
    assert_eq!(PLAYER_BYTES, terra_net::proto::MOST_NAME);
}

#[test]
fn the_characters_page_turns_each_feature_and_back_returns_to_it() {
    let mut menu = Menu::new(Vec::new());
    assert_eq!(choose(&mut menu, Item::Character), None);
    assert_eq!(menu.page, Page::Character);
    let began = menu.features.clone();
    for feature in Feature::ALL {
        let before = menu.features.clone();
        assert_eq!(choose(&mut menu, Item::Feature(feature)), None);
        assert_ne!(menu.features, before, "{feature:?} did not turn");
    }
    assert_ne!(menu.features.hair, began.hair);
    menu.back();
    assert_eq!(
        (menu.page, menu.picked()),
        (Page::Main, Some(Item::Character))
    );
}
