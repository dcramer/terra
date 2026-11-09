//! The debug menu's page for someone playing (D81; the backtick key, its "Play" tab): going
//! to the places a game is tested at (the start, the gate, the den, the village), the gate
//! (bearing a mark, opening it, stepping through), the hour of the day, taking no harm,
//! putting a creature ahead, and giving items.
//! Every button makes a remote method's call (`terra_world::remote::call`), the one a
//! scripted check makes, so the menu and the agents' tools are one list; the page keeps only
//! what was typed and what the last call answered.

use super::claim::Village;
use super::inventory::Items;
use super::marks::Kinds;
use super::species::Bestiary;
use crate::remote::call;
use crate::start::Start;
use bevy::prelude::*;
use serde_json::{Value, json};
use terra_world::debug_menu::{DebugMenu, Page, ui as egui};

/// What the page keeps between frames.
#[derive(Resource, Default)]
struct PlayPage {
    /// What is typed to find an item by.
    find: String,
    /// What the last button's call answered, or why it was refused.
    said: String,
    /// The places as they were last listed, for a script to read.
    places: Vec<(&'static str, [f32; 2])>,
}

/// How far to the side of a place the player is put, so as not to stand in what is there.
const BESIDE_M: f32 = 4.0;
/// The items listed at once: the rest are found by typing.
const LISTED: usize = 14;

pub(super) fn plugin(app: &mut App) {
    let Some(mut menu) = app.world_mut().get_resource_mut::<DebugMenu>() else {
        return;
    };
    menu.add(Page {
        title: "Play",
        show,
        describe: |world| {
            let page = world.resource::<PlayPage>();
            json!({ "said": page.said, "places": page.places })
        },
    });
    app.init_resource::<PlayPage>();
}

/// One call made for a button, and what it answered kept for the page to say.
fn press(world: &mut World, method: &str, params: Value) {
    let said = match call(world, method, Some(params)) {
        Ok(answer) => format!("{method}: {answer}"),
        Err(why) => format!("{method} refused: {why}"),
    };
    info!("Debug menu: {said}");
    world.resource_mut::<PlayPage>().said = said;
}

/// A place read from a method's answer: the east and south of a point three numbers long.
fn point(answer: &Value) -> Option<[f32; 2]> {
    let at = answer.as_array()?;
    Some([at.first()?.as_f64()? as f32, at.get(2)?.as_f64()? as f32])
}

/// The places to go to, by name, each east and south of the start in metres as `play.warp`
/// takes it; a place this world has none of is left out.
fn places(world: &mut World) -> Vec<(&'static str, [f32; 2])> {
    let mut places = vec![("The start", [0.0, 0.0])];
    let gate = call(world, "portal.state", None).ok();
    if let Some(at) = gate.as_ref().and_then(|gate| point(&gate["at"])) {
        places.push(("The gate", [at[0] + BESIDE_M, at[1]]));
    }
    let den = call(world, "den.state", None).ok();
    if let Some(at) = den.as_ref().and_then(|den| point(&den["den"]["mouth"])) {
        places.push(("The den's mouth", at));
    }
    if let Some(at) = den.as_ref().and_then(|den| point(&den["den"]["floor"])) {
        places.push(("The troll's chamber", at));
    }
    let village = (world.get_resource::<Village>().map(|village| village.0))
        .zip(world.get_resource::<Start>())
        .map(|(middle, start)| start.local(middle));
    if let Some(at) = village {
        places.push(("The village", [at.x, at.z]));
    }
    places
}

fn show(ui: &mut egui::Ui, world: &mut World) {
    ui.heading("Go to");
    let places = places(world);
    world.resource_mut::<PlayPage>().places.clone_from(&places);
    ui.horizontal_wrapped(|ui| {
        for (name, at) in places {
            if ui.button(name).clicked() {
                press(world, "play.warp", json!({ "at": at }));
            }
        }
    });
    ui.separator();
    ui.heading("The gate");
    let marks: Vec<(String, String)> = (world.get_resource::<Kinds>())
        .map(|kinds| {
            kinds
                .all()
                .iter()
                .map(|kind| (kind.key.clone(), kind.name.clone()))
                .collect()
        })
        .unwrap_or_default();
    ui.horizontal_wrapped(|ui| {
        for (key, name) in marks {
            if ui.button(format!("Bear {name}")).clicked() {
                press(world, "marks.give", json!({ "mark": key }));
            }
        }
        if ui.button("Open the nearest gate").clicked() {
            press(world, "portal.open", json!({}));
        }
        if ui.button("Step through it").clicked() {
            press(world, "portal.step", json!({}));
        }
    });
    ui.separator();
    ui.heading("Hour of the day");
    ui.horizontal_wrapped(|ui| {
        for (name, hour) in [
            ("Dawn", 6.0),
            ("Noon", 12.0),
            ("Dusk", 19.0),
            ("Midnight", 0.0),
        ] {
            if ui.button(name).clicked() {
                press(world, "play.hour", json!({ "hour": hour }));
            }
        }
    });
    ui.separator();
    ui.heading("The player");
    let unharmed = world.resource::<super::probe::Unharmed>().0;
    let harm = if unharmed {
        "Takes no harm (press to take harm again)"
    } else {
        "Take no harm"
    };
    if ui.button(harm).clicked() {
        press(world, "play.unharmed", json!({}));
    }
    ui.separator();
    let mut find = std::mem::take(&mut world.resource_mut::<PlayPage>().find);
    ui.horizontal(|ui| {
        ui.label("Find a creature or an item");
        ui.text_edit_singleline(&mut find);
    });
    let wanted = find.to_lowercase();
    world.resource_mut::<PlayPage>().find = find;
    ui.heading("Put a creature ahead");
    let kinds: Vec<String> = (world.get_resource::<Bestiary>())
        .map(|bestiary| {
            (bestiary.all().map(|kind| bestiary.name(kind).to_string()))
                .filter(|name| name.to_lowercase().contains(&wanted))
                .collect()
        })
        .unwrap_or_default();
    ui.horizontal_wrapped(|ui| {
        for kind in kinds.iter().take(LISTED) {
            if ui.button(kind).clicked() {
                press(world, "wildlife.put", json!({ "kind": kind }));
            }
        }
    });
    if kinds.len() > LISTED {
        ui.label(format!(
            "and {} more: type to find one",
            kinds.len() - LISTED
        ));
    }
    ui.separator();
    ui.heading("Give");
    let names: Vec<String> = (world.get_resource::<Items>())
        .map(|items| {
            (items.kinds().map(|kind| items.name(kind).to_string()))
                .filter(|name| name.to_lowercase().contains(&wanted))
                .collect()
        })
        .unwrap_or_default();
    for name in names.iter().take(LISTED) {
        ui.horizontal(|ui| {
            for count in [1, 10] {
                if ui.button(format!("+{count}")).clicked() {
                    press(world, "play.give", json!({ "item": name, "count": count }));
                }
            }
            ui.label(name);
        });
    }
    if names.len() > LISTED {
        ui.label(format!(
            "and {} more: type to find one",
            names.len() - LISTED
        ));
    }
    ui.separator();
    ui.label(world.resource::<PlayPage>().said.as_str());
}
