//! The base laid out at each world's start, whether or not anything is built there: the
//! base, its site and its footprint are the world's (D469); the kits' look is every world's.

use super::{Build, BuildLook, DrivenByPlayer, Site, SiteFootprint, example};
use bevy::prelude::*;
use terra_world::instance::{Building, InWorld, InstanceApp, WorldScoped};
use terra_world::planet::Planet;
use terra_world::start::Start;

pub(super) fn plugin(app: &mut App) {
    app.world_resource::<Site>().add_systems(
        OnEnter(InWorld),
        (
            lay_out.in_set(Building::Place),
            super::keep_footprint
                .in_set(Building::View)
                .before(terra_world::flora::FloraSystems::Plant),
        ),
    );
}

fn lay_out(
    mut commands: Commands,
    planet: Res<Planet>,
    start: Res<Start>,
    look: Res<BuildLook>,
    driven: Option<Res<DrivenByPlayer>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let mut build = Build::new(&mut commands, &look, **start, "Base");
    // The player's base is built by plans, drawn as far as each cube is in.
    build.by_cubes = driven.is_some();
    build.by_plans = driven.is_some();
    let operations = if driven.is_some() {
        Vec::new()
    } else {
        example(look.laying)
    };
    build.replace_with(&operations, &mut commands, &planet, &mut meshes);
    commands.spawn((
        Name::new("Base footprint"),
        SiteFootprint,
        build.footprint.clone(),
        WorldScoped,
    ));
    commands.insert_resource(Site {
        build,
        camera_placed: false,
    });
}
