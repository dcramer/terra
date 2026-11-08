//! The world's den as a scripted run reads it (D591, D643, D69): `den.state` says where its
//! mouth opens and its chamber's floor is, in the terms `--at` and the log give places in
//! (metres east, up and south of the start), which way its passage goes in, and its layout.

use crate::planet::Planet;
use crate::start::Start;
use bevy::prelude::*;

/// How far along its passage a den's mouth is open to the sky or thinly roofed
/// (`terra_land::cave`'s mouth), and how far either side of it nothing is left growing.
const MOUTH_M: f32 = 22.0;
const CLEAR_M: f32 = 5.5;

/// Made afresh in each world: its den's mouth has been cleared.
#[derive(Resource, Default)]
struct Cleared(bool);

pub fn plugin(app: &mut App) {
    use crate::instance::InstanceApp;
    app.init_world_resource::<Cleared>()
        .add_systems(Update, clear_mouth.run_if(crate::instance::in_world));
    crate::remote::method(app, "den.state", state);
}

/// Once in each world entered: nothing grows in the den's mouth, where the ground the
/// plants were laid on is cut away: kept clear at once, for what is laid after this frame,
/// and asked of what already grows, as a gate's ground is.
fn clear_mouth(
    mut cleared: ResMut<Cleared>,
    planet: Res<Planet>,
    mut kept: Option<ResMut<crate::flora::KeptClear>>,
    mut clear: MessageWriter<crate::flora::ClearAround>,
) {
    if std::mem::replace(&mut cleared.0, true) {
        return;
    }
    let Some(den) = planet.den() else { return };
    let mut along = 0.0;
    while along <= MOUTH_M {
        let at = den.mouth + den.heading * along;
        if let Some(kept) = kept.as_mut() {
            kept.keep(at, CLEAR_M);
        }
        clear.write(crate::flora::ClearAround(at, CLEAR_M));
        along += CLEAR_M;
    }
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
            "room_across_m": den.parts.iter().find_map(|part| match part {
                terra_voxel::caves::Part::Chamber { across_m, .. } => Some(*across_m),
                _ => None,
            }),
        }
    }))
}
