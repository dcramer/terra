//! The world's den as a scripted run reads it (D591, D643, D69): `den.state` says where its
//! mouth opens and its chamber's floor is, in the terms `--at` and the log give places in
//! (metres east, up and south of the start), which way its passage goes in, and its layout.

use crate::planet::Planet;
use crate::start::Start;
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    crate::remote::method(app, "den.state", state);
}

fn state(
    In(_): In<Option<serde_json::Value>>,
    planet: Option<Res<Planet>>,
    start: Option<Res<Start>>,
) -> bevy::remote::BrpResult {
    let (Some(planet), Some(start)) = (planet, start) else {
        return Err(crate::remote::refused("No world is made yet."));
    };
    let Some(den) = planet.den() else {
        return Ok(serde_json::json!({ "den": null }));
    };
    let local = |point: Vec3| start.local(point).to_array();
    Ok(serde_json::json!({
        "den": {
            "mouth": local(den.mouth),
            "floor": local(den.floor),
            "ahead": local(den.mouth + den.heading * 10.0),
            "layout": { "way": den.layout.way, "room": den.layout.room, "alcoves": den.layout.alcoves },
            "parts": den.parts.len(),
        }
    }))
}
