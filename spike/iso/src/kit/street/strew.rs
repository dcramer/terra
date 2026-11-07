//! The street's scatter (kit/street.rs): its floor as the scatter's ground, the biome's
//! models and plant clumps strewn over it, and the weather's floor.

use super::{SITES, Street};
use crate::kit::biome::Biome;
use crate::kit::contact::Grounded;
use bevy::prelude::*;

/// How far down the street the scatter is strewn each side of the focus (m): the frame and
/// a margin.
const STREWN: f32 = 18.0;

/// The street's floor height at `p`: the road at 0, the pavements the kerb's rise up.
fn floor(st: &Street, p: Vec2) -> f32 {
    if p.y.abs() <= st.edges().0 {
        0.0
    } else {
        st.kerb.1
    }
}

/// The street as the scatter's ground (kit/scatter.rs): its maps (`SITES`) and floor, kept
/// off each prop's foot, over the frame and a margin each side of the focus.
pub(super) fn with_ground<R>(st: &Street, f: impl FnOnce(&crate::kit::scatter::Ground) -> R) -> R {
    let (_, _, wall) = st.edges();
    let site = |k: usize, p: Vec2| [st.gutter(p), st.puddled(p), st.walls(p)][k];
    let height = |p: Vec2| floor(st, p);
    let feet: Vec<Vec2> = st
        .props
        .iter()
        .flat_map(|p| p.at.iter().map(|x| st.spot(p.side, p.line, *x, p.out).xz()))
        .collect();
    let keep = |p: Vec2| feet.iter().any(|f| f.distance(p) < 0.7);
    f(&crate::kit::scatter::Ground {
        sites: &SITES,
        site: &site,
        height: &height,
        water: None,
        rules: None,
        keep: &keep,
        span: (
            Vec2::new(st.focus.0 - STREWN, -wall),
            Vec2::new(st.focus.0 + STREWN, wall),
        ),
    })
}

/// The biome's scatter over the street (kit/scatter.rs): its models, and its plant sets'
/// clumps as cards on the street's floor (uc-wasteland j3: the street drew no plant set, so
/// the wasteland's dry grass came out nowhere, silently).
#[allow(clippy::too_many_arguments)]
pub(super) fn strew(
    mut commands: Commands,
    asset: Res<AssetServer>,
    biome: Res<Biome>,
    mut budget: ResMut<crate::kit::scatter::Budget>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut foliage: ResMut<Assets<crate::foliage::FoliageMaterial>>,
    mut mats: ResMut<crate::foliage::Mats>,
) {
    let Some(st) = biome.street.as_ref() else {
        return;
    };
    let (_, _, wall) = st.edges();
    // The weather's floor (kit/weather.rs): the road and the pavements, no standing water.
    commands.insert_resource(crate::kit::weather::Floor::sample(
        Vec2::new(st.focus.0 - STREWN, -wall),
        Vec2::new(st.focus.0 + STREWN, wall),
        0.25,
        None,
        |p| floor(st, p),
    ));
    let (strewn, clumps) = with_ground(st, |g| {
        let strewn = crate::kit::scatter::spawn(
            &mut commands,
            &asset,
            &mut budget,
            &biome.refs,
            &biome.scatter,
            g,
        );
        let clumps = crate::kit::scatter::plants(&mut budget, &biome.refs, &biome.scatter, g);
        (strewn, clumps)
    });
    // Each piece of litter on its own contact shadow and stain (kit/contact.rs).
    for (e, at) in strewn {
        let y = floor(st, at.xz());
        commands.entity(e).insert(Grounded { floor: y });
    }
    // The cards stand on the land the foliage reads (its heights, and the wind's breath
    // reads it every frame): the street's floor on the terrain's grid.
    let land = crate::terrain::Land::from_shape(|p| floor(st, p));
    crate::kit::scatter::draw_plants(
        &mut commands,
        &asset,
        &mut budget,
        &land,
        &mut meshes,
        &mut foliage,
        &mut mats,
        &clumps,
    );
    commands.insert_resource(land);
}
