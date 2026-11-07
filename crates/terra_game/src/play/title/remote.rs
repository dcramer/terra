//! The title screen by the remote protocol (D69, tools/sandbox), so a scripted run can see
//! which screen is up and what it offers, and leave a world as Quit to title does. Choices
//! are made with the keys, as a player makes them (`press down enter`).
//!
//! - `title.state`: the screen up, the land's seed, the world played, and while the title
//!   screen is up its page, its choices, the one picked, its worlds and what it tells.
//! - `title.leave`: the world played is left for the title screen.

use super::super::worlds::Playing;
use super::{Leaving, Title};
use bevy::prelude::*;
use bevy::remote::BrpResult;
use serde_json::{Value, json};
use terra_world::instance::WorldSpec;
use terra_world::loading::Screen;
use terra_world::remote::{method, refused};

pub fn plugin(app: &mut App) {
    method(app, "title.state", state);
    method(app, "title.leave", leave);
}

fn state(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    let screen = *world.resource::<State<Screen>>().get();
    let menu = world
        .get_resource::<Title>()
        .filter(|_| screen == Screen::Title)
        .map(|title| &title.0);
    Ok(json!({
        "screen": format!("{screen:?}"),
        "seed": world.get_resource::<WorldSpec>().map(|spec| spec.seed.to_string()),
        "playing": world.get_resource::<Playing>().map(|playing| playing.0.meta.name.clone()),
        "page": menu.map(|menu| format!("{:?}", menu.page)),
        "choices": menu.map(|menu| menu.items().iter().map(|item| format!("{item:?}")).collect::<Vec<_>>()),
        "picked": menu.and_then(|menu| menu.picked()).map(|item| format!("{item:?}")),
        "worlds": menu.map(|menu| menu.worlds.iter().map(|world| json!({
            "name": world.meta.name,
            "seed": world.meta.seed.to_string(),
        })).collect::<Vec<_>>()),
        "notice": menu.and_then(|menu| menu.notice.clone()),
    }))
}

fn leave(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    if *world.resource::<State<Screen>>().get() != Screen::Playing {
        return Err(refused(
            "No world is being played: the title screen is up, or the game is loading.",
        ));
    }
    world.insert_resource(Leaving::default());
    Ok(json!("leaving for the title screen"))
}
