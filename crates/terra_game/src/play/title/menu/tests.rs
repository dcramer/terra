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
        Act::Make { name, creative } => worlds::make(root, &name, creative, 100).unwrap(),
        other => panic!("not a world: {other:?}"),
    }
}

#[test]
fn the_title_screens_choices_play_the_worlds_the_flags_do() {
    let root = folder_for("flags");
    worlds::make(&root, "Old", false, 1).unwrap();
    worlds::make(&root, "Last", false, 2).unwrap();
    // Continue: the world `cargo xtask play continue` (no flag) plays.
    let mut menu = Menu::new(worlds::list(&root));
    let continued = world_of(&root, choose(&mut menu, Item::Continue).unwrap());
    let flagged = worlds::pick(&root, &Choice::default(), false, 3).unwrap();
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
    worlds::make(&root, "Keep me", false, 1).unwrap();
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
