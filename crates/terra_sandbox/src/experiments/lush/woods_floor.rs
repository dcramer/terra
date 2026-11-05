//! The woods' floor (D619): on a world with land, what stands on the floor round the woods'
//! grown trees is the forest walk's, laid by its own rules (`floor::lay`) on the floor the
//! woods laid round their trees (`terra_world::cover::forest::WoodsFloor`): rocks and
//! boulders, fallen logs and stumps, and the undergrowth in drifts, ferns, bracken, herbs,
//! hazel, bilberry, holly, saplings and the rest, only among the trees and off the water,
//! as thickly as the forest walk's, within `REACH_M` of the woods' middle. The ground cover
//! is then grown from that floor, where the woods' grass, leaves, needles and moss lie.

use super::FloorLooks;
use super::floor::{self, Area, Lay, MossyScans, Variants};
use super::shade::NearShadows;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use terra_world::cover::forest::{LayingFloor, WoodsFloor};
use terra_world::flora_kinds::Flora;
use terra_world::grown::Cards;
use terra_world::instance::InWorld;
use terra_world::planet::Planet;
use terra_world::start::{Sited, Start};

/// The floor is laid this far round the woods' middle, the forest walk's own reach: past it
/// the grown trees stand on the woods' ground cover alone.
const REACH_M: f32 = 220.0;
/// None nearer the start than the woods' own clearing round it.
const CLEAR_M: f32 = 14.0;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(InWorld), lay_woods_floor.in_set(LayingFloor));
}

#[allow(clippy::too_many_arguments)]
fn lay_woods_floor(
    mut commands: Commands,
    planet: Res<Planet>,
    start: Res<Start>,
    sited: Option<Res<Sited>>,
    (woods, looks, flora, cards): (
        Option<ResMut<WoodsFloor>>,
        Option<Res<FloorLooks>>,
        Option<Res<Flora>>,
        Option<Res<Cards>>,
    ),
    assets: Res<AssetServer>,
    (mut meshes, mut plain, mut foliage): (
        ResMut<Assets<Mesh>>,
        ResMut<Assets<StandardMaterial>>,
        ResMut<Assets<super::Foliage>>,
    ),
    (mut mossy, mut mossy_scans, mut variants): (
        ResMut<Assets<floor::Mossy>>,
        ResMut<MossyScans>,
        ResMut<Variants>,
    ),
) {
    let (Some(mut woods), Some(looks), Some(flora), Some(cards)) = (woods, looks, flora, cards)
    else {
        return;
    };
    let Some(map) = woods.0.as_mut() else {
        return;
    };
    let began = std::time::Instant::now();
    let east = start.rotation * Vec3::X;
    let south = start.rotation * Vec3::Z;
    let middle = sited
        .and_then(|sited| sited.woods_middle(&planet))
        .map(|at| {
            let local = start.local(at);
            Vec2::new(local.x, local.z)
        })
        .unwrap_or(Vec2::ZERO);
    let area = Area {
        middle,
        from_m: CLEAR_M,
        to_m: REACH_M,
        inner_m: CLEAR_M,
        outer_m: REACH_M,
        among_trees: true,
    };
    floor::lay(
        &mut Lay {
            commands: &mut commands,
            planet: &planet,
            start: start.translation,
            east,
            south,
            assets: &assets,
            meshes: &mut meshes,
            plain: &mut plain,
            mossy: &mut mossy,
            mossy_scans: &mut mossy_scans,
            flora: &flora,
            cards: &cards,
            variants: &mut variants,
            bark: looks.bark.clone(),
            pine_bark: looks.pine_bark.clone(),
            spruce_bark: looks.spruce_bark.clone(),
            atlas: looks.floor_cards.clone(),
            thicket: looks.thicket_cards.clone(),
            seed: planet.seed().derive("woods floor"),
            foliage: &mut foliage,
            shrinking: HashMap::default(),
            shade: NearShadows::new(start.translation, east, south),
            sun: None,
        },
        map,
        &area,
    );
    info!(
        "Woods: their floor laid within {REACH_M:.0} m of {:.0},{:.0} in {:.1} s",
        middle.x,
        middle.y,
        began.elapsed().as_secs_f32()
    );
}
