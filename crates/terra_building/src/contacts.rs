//! `build.contacts {within?}`: why the player stands where they do at the base, for a
//! headless test to reproduce a stop seen in the game (D69). The player's body as the mover
//! keeps it, its feet and what its last tick touched, and the base's boxes near its feet
//! (`terra_build_layout::solids`, kits and leaves as the walk has them), all in the base's own
//! space, in metres.

use bevy::{
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
};
use serde::Deserialize;
use serde_json::{Value, json};
use terra_bases::Site;
use terra_walker::movement::Movers;
use terra_walker::{LocalPlayer, Player};
use terra_world::remote::{method, refused};

pub fn plugin(app: &mut App) {
    method(app, "build.contacts", contacts);
}

#[derive(Deserialize, Default)]
struct Near {
    within: Option<f32>,
}

fn contacts(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Near { within } = match params {
        Some(_) => parse_some(params)?,
        None => Near::default(),
    };
    let within = within.unwrap_or(1.0);
    let player = world
        .query_filtered::<Entity, (With<Player>, With<LocalPlayer>)>()
        .iter(world)
        .next()
        .ok_or_else(|| refused("no player yet"))?;
    let body = world
        .resource::<Movers>()
        .body(player)
        .ok_or_else(|| refused("the mover has no body for the player"))?;
    let site = world.resource::<Site>();
    let back = site.frame.compute_affine().inverse();
    let local = |at: Vec3| back.transform_point3(at);
    let turn = |v: Vec3| back.transform_vector3(v).normalize_or_zero();
    let v = |v: Vec3| [v.x, v.y, v.z];
    let feet = local(body.feet);
    let touched: Vec<Value> = body
        .contacts()
        .map(|(at, normal, met)| {
            json!({"at": v(local(at.as_vec3())), "normal": v(turn(normal)), "met": met})
        })
        .collect();
    let slabs: Vec<Value> =
        terra_build_layout::solids::near(site.walkable(), feet, within, site.kits.as_deref())
            .into_iter()
            .map(|slab| {
                let corner = |s: f32| {
                    slab.centre
                        + s * slab
                            .axes
                            .iter()
                            .zip(slab.half.to_array())
                            .map(|(a, h)| *a * h)
                            .sum::<Vec3>()
                };
                json!({
                    "centre": v(slab.centre),
                    "half": v(slab.half),
                    "axes": slab.axes.map(v),
                    "low": v(corner(-1.0)),
                    "high": v(corner(1.0)),
                })
            })
            .collect();
    Ok(json!({
        "feet": v(feet),
        "velocity": v(back.transform_vector3(body.velocity)),
        "ground": format!("{:?}", body.ground),
        "stepped": body.stepped,
        "contacts": touched,
        "slabs": slabs,
    }))
}
