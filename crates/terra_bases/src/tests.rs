//! The base on screen alone, as the sandbox's `build` experiment runs it: on the shell, the
//! planet and the start, without the world's layer (`terra_world::world`, D198).

use super::*;
use terra_core::{PlanetSpec, Seed};
use terra_world::shell::Options;

/// An app with what the base stands on, as the shell gives it (input, the base's materials),
/// and nothing of the world's layer above the planet: none of its resources (the scene's
/// `Indoors`, the quality, the open sky) are there.
fn alone() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        bevy::gizmos::GizmoPlugin,
    ))
    .init_asset::<Mesh>()
    .init_asset::<StandardMaterial>()
    .init_asset::<Image>()
    .init_asset::<arrays::BuildMaterial>()
    .init_asset::<draw::GhostMaterial>()
    .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
    .init_resource::<ButtonInput<KeyCode>>();
    let planet = Planet::new(PlanetSpec {
        seed: Seed(1),
        radius_m: 2000.0,
    });
    app.insert_resource(Options::from_args(std::iter::empty()).unwrap())
        .insert_resource(Start(Transform::from_translation(planet.start())))
        .insert_resource(planet);
    plugin(&mut app);
    app.world_mut()
        .spawn((MainCamera, Transform::default(), GlobalTransform::default()));
    app
}

/// The base runs its frames without the world's layer: a system asking for a resource only
/// that layer makes (the scene's `Indoors`, which `light::inside` tells) would panic here.
#[test]
fn the_base_runs_without_the_world_layer() {
    let mut app = alone();
    for _ in 0..3 {
        app.update();
    }
    assert!(app.world().contains_resource::<Site>());
    assert!(
        !app.world()
            .contains_resource::<terra_world::scene::Indoors>()
    );
}
