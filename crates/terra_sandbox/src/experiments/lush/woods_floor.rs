//! The woods' floor (D619): on a world with land, what stands on the floor round the woods'
//! grown trees is the forest walk's, laid by its own rules (`floor::lay`) on the floor the
//! woods laid round their trees (`terra_world::cover::forest::WoodsFloor`): rocks and
//! boulders, fallen logs and stumps, and the undergrowth in drifts, ferns, bracken, herbs,
//! hazel, bilberry, holly, saplings and the rest, only among the trees and off the water,
//! as thickly as the forest walk's, within `REACH_M` of the woods' middle, and past it round
//! the camera as the woods' trees stream in (`stream`, D622). The ground cover is then grown
//! from that floor, where the woods' grass, leaves, needles and moss lie.

mod stream;

use super::FloorLooks;
use super::floor::{self, Area, Lay, MossyScans, Variants};
use super::shade::NearShadows;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use terra_world::cover::forest::{LayingFloor, WoodsFloor};
use terra_world::flora::Undergrown;
use terra_world::flora_kinds::Flora;
use terra_world::grown::Cards;
use terra_world::instance::InWorld;
use terra_world::planet::Planet;
use terra_world::start::{Sited, Start};

/// The floor is laid this far round the woods' middle on loading, the forest walk's own
/// reach: past it, it streams in round the camera (`stream`).
const REACH_M: f32 = 220.0;
/// None nearer the start than the woods' own clearing round it.
const CLEAR_M: f32 = 14.0;
/// Where the stump and the log lie by the base's door, metres east and south of the start,
/// as if the clearing had been cut.
const STUMP_AT: Vec2 = Vec2::new(1.0, 11.0);
const LOG_AT: Vec2 = Vec2::new(-2.5, 12.5);

/// Whether the woods' floor is laid only among the trees on the floor's map: where the first
/// realm's trees stand, which are the ones it holds. A realm that grows none of them
/// (`trees: 0`, the far realm's, D642) has its forest in its own parts, which stand on all
/// its dry level ground and are not on the map: laid only among trees its floor was bare
/// (no plant on a row of pieces the first realm lays 2,071 on under oaks, `stream_tests`;
/// the first realm's floor has 43,058 within 220 m of seed 3's start), so it is laid
/// wherever the ground is dry.
pub(super) fn among_its_trees(planet: &Planet) -> bool {
    planet.trees() > 0.0
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(InWorld), lay_woods_floor.in_set(LayingFloor));
    stream::plugin(app);
}

#[allow(clippy::too_many_arguments)]
fn lay_woods_floor(
    mut commands: Commands,
    planet: Res<Planet>,
    start: Res<Start>,
    sited: Option<Res<Sited>>,
    (woods, looks, flora, cards, kept_clear): (
        Option<ResMut<WoodsFloor>>,
        Option<Res<FloorLooks>>,
        Option<Res<Flora>>,
        Option<Res<Cards>>,
        Option<Res<terra_world::flora::KeptClear>>,
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
    let seed = planet.seed().derive("woods floor");
    let east = start.rotation * Vec3::X;
    let south = start.rotation * Vec3::Z;
    let middle = sited
        .and_then(|sited| sited.woods_middle(&planet))
        .map(|at| {
            let local = start.local(at);
            Vec2::new(local.x, local.z)
        })
        .unwrap_or(Vec2::ZERO);
    // The world's flora sows none of its old undergrowth where this lays its own: among the
    // trees everywhere, as the stream lays it past `REACH_M` round the camera, past where the
    // flora's understory reaches.
    commands.insert_resource(Undergrown {
        middle,
        reach_m: f32::INFINITY,
    });
    let area = Area {
        middle,
        from_m: CLEAR_M,
        to_m: REACH_M,
        inner_m: CLEAR_M,
        outer_m: REACH_M,
        among_trees: among_its_trees(&planet),
        off_water: true,
        square: None,
        skip_within_m: 0.0,
    };
    let mut lay = Lay {
        commands: &mut commands,
        planet: &planet,
        kept_clear: kept_clear.as_deref(),
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
        seed,
        foliage: &mut foliage,
        shrinking: HashMap::default(),
        shade: NearShadows::new(start.translation, east, south),
        sun: None,
    };
    let kit = floor::lay(&mut lay, map, &area);
    // A stump and a log of the floor's own wood by the door, in the start's clearing.
    floor::by_the_door(&mut lay, map, STUMP_AT, LOG_AT);
    let frame = (start.translation, east, south);
    commands.insert_resource(stream::FloorStream::new(kit, seed, middle, frame));
    info!(
        "Woods: their floor laid within {REACH_M:.0} m of {:.0},{:.0} in {:.1} s",
        middle.x,
        middle.y,
        began.elapsed().as_secs_f32()
    );
}
