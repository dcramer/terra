//! The planet's tweaks applied: the sun, the sea, and the ground's and waters' materials
//! follow the tweaks, the sky, the land's maps and the quality whenever one of them changes.

use super::*;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        // Every condition is evaluated, so one on a world resource asks whether it is there:
        // none is while the world is generated (D623).
        apply_tweaks
            .run_if(
                resource_exists_and_changed::<Tweaks>
                    .or_eager(resource_exists_and_changed::<Sky>)
                    .or_eager(resource_exists_and_changed::<GroundCoverMap>)
                    .or_eager(resource_exists_and_changed::<crate::climate::ClimateMaps>)
                    .or_eager(resource_changed::<crate::quality::Quality>)
                    .or_eager(resource_exists_and_changed::<crate::horizon::HorizonPicture>),
            )
            .run_if(in_world),
    );
}

fn apply_tweaks(
    tweaks: Res<Tweaks>,
    planet: Res<Planet>,
    sky: Option<Res<Sky>>,
    mut sun: Single<(&mut DirectionalLight, &mut Transform), (With<Sun>, Without<Sea>)>,
    mut sea: Single<
        (
            &mut Transform,
            &mut Visibility,
            &MeshMaterial3d<WaterMaterial>,
        ),
        With<Sea>,
    >,
    mut ground_materials: ResMut<Assets<GroundMaterial>>,
    mut water_materials: ResMut<Assets<WaterMaterial>>,
    textures: Res<GroundTextures>,
    cover: Res<GroundCoverMap>,
    climate: Res<crate::climate::ClimateMaps>,
    quality: Res<crate::quality::Quality>,
    land: Option<Res<LandMap>>,
    horizon: Option<Res<crate::horizon::HorizonPicture>>,
    open: Option<Res<crate::skylight::OpenSky>>,
) {
    let toward_sun = tweaks
        .light_direction
        .try_normalize()
        .unwrap_or(tweaks.sun_direction.normalize_or(Vec3::Y));
    sun.0.color = tweaks.sun_color;
    sun.0.illuminance = tweaks.sun_illuminance;
    sun.0.soft_shadow_size = (tweaks.light_size > 0.0).then_some(tweaks.light_size);
    *sun.1 = Transform::from_translation(toward_sun).looking_at(Vec3::ZERO, Vec3::Y);

    sea.0.scale = Vec3::splat(planet.radius_m() + tweaks.sea_level_m);
    *sea.1 = if tweaks.sea {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };

    // Only its tweaks and the sky's colour change the ground: a changed material is prepared again.
    let land = land.as_deref().cloned().unwrap_or_default();
    let no_horizon = crate::horizon::HorizonPicture::default();
    let horizon = horizon.as_deref().unwrap_or(&no_horizon);
    let mut ground = GroundExtension::new(
        &planet, &tweaks, &textures, &cover, &climate, *quality, &land, horizon,
    );
    ground.snow_light = crate::skylight::snow_light(sky.as_deref(), open.as_deref());
    let stale: Vec<_> = ground_materials
        .iter()
        .filter(|(_, material)| material.extension != ground)
        .map(|(id, _)| id)
        .collect();
    for id in stale {
        if let Some(mut material) = ground_materials.get_mut(id) {
            material.extension = ground.clone();
        }
    }
    // Every water reflects the sky; the sea's colour is the tweaks', a lake's its own.
    let sea_material = sea.2.id();
    for (id, material) in water_materials.iter_mut() {
        if id == sea_material {
            material.base.base_color = tweaks.sea_shallow;
        }
        material.extension.reflect(sky.as_deref());
    }
}
