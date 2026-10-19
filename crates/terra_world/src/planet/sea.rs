//! The sea's material: the sky it reflects, and its waves following the camera.

use super::{MainCamera, Planet, Sea, Tweaks, WaterMaterial};
use crate::sky::Sky;
use bevy::{
    pbr::MaterialExtension, prelude::*, render::render_resource::AsBindGroup, shader::ShaderRef,
};

/// What the water shader is told. Field order must match the struct in water.wesl.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct WaterExtension {
    /// The sky at the horizon and overhead, which is what the water reflects.
    #[uniform(100)]
    horizon: LinearRgba,
    #[uniform(100)]
    zenith: LinearRgba,
    #[uniform(100)]
    sky_brightness: f32,
    /// The wind over the water (`waves::over_water`).
    #[uniform(100)]
    wind: Vec4,
    /// The waves' frame, travelling with the camera (`waves::Travelling`): its origin, east and
    /// north.
    #[uniform(100)]
    origin: Vec4,
    #[uniform(100)]
    east: Vec4,
    #[uniform(100)]
    north: Vec4,
    /// A point fixed in the world near the start, what the gusts are laid out from.
    #[uniform(100)]
    anchor: Vec4,
    /// The waves' trains, their phases where the frame is.
    #[uniform(100)]
    trains: [Vec4; crate::waves::TRAINS],
}

impl MaterialExtension for WaterExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/water.wesl".into()
    }
}

impl WaterExtension {
    pub(super) fn new(sky: Option<&Sky>, anchor: Vec3) -> Self {
        let mut water = WaterExtension {
            horizon: LinearRgba::BLACK,
            zenith: LinearRgba::BLACK,
            sky_brightness: 0.0,
            wind: Vec4::ZERO,
            origin: anchor.extend(0.0),
            east: Vec4::ZERO,
            north: Vec4::ZERO,
            anchor: anchor.extend(0.0),
            trains: crate::waves::trains(),
        };
        water.reflect(sky);
        water
    }

    /// The sky it reflects; with none, it reflects nothing.
    pub(super) fn reflect(&mut self, sky: Option<&Sky>) {
        (self.horizon, self.zenith, self.sky_brightness) = sky
            .map_or((LinearRgba::BLACK, LinearRgba::BLACK, 0.0), |sky| {
                (sky.horizon.into(), sky.zenith.into(), sky.brightness)
            });
    }
}

/// The sea's waves follow the camera: their frame moved to the sea's surface under it, and the
/// wind over them, written to its material only when either has changed.
pub(super) fn sea_waves(
    tweaks: Res<Tweaks>,
    planet: Res<Planet>,
    wind: Option<Res<crate::wind::WorldWind>>,
    camera: Option<Single<&GlobalTransform, With<MainCamera>>>,
    sea: Single<&MeshMaterial3d<WaterMaterial>, With<Sea>>,
    mut water_materials: ResMut<Assets<WaterMaterial>>,
    mut frame: Local<Option<crate::waves::Travelling>>,
) {
    let Some(camera) = camera else {
        return;
    };
    let up = camera.translation().normalize_or(Vec3::Y);
    let under = up * (planet.radius_m() + tweaks.sea_level_m);
    let trains = crate::waves::trains();
    let frame = frame.get_or_insert_with(|| {
        let east = Vec3::Y.cross(up).try_normalize().unwrap_or(Vec3::X);
        crate::waves::Travelling::new(under, east)
    });
    let moved = frame.origin().distance(under) > 0.001;
    let wind = crate::waves::over_water(wind.as_deref());
    let Some(material) = water_materials.get(&sea.0) else {
        return;
    };
    if !moved && material.extension.wind == wind && material.extension.east != Vec4::ZERO {
        return;
    }
    frame.follow(under, &trains);
    if let Some(mut material) = water_materials.get_mut(&sea.0) {
        let water = &mut material.extension;
        water.wind = wind;
        water.origin = frame.origin().extend(0.0);
        water.east = frame.east().extend(0.0);
        water.north = frame.north().extend(0.0);
        water.trains = frame.trains(&trains);
    }
}
