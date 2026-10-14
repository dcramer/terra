//! A sky: the inside of a large sphere around the camera, drawn by
//! `assets/shaders/sky.wesl`, which reloads when saved.
//!
//! An experiment that wants one adds [`SkyPlugin`] and a [`Sky`] saying what it looks like.
//! The sun's place in it follows the sandbox's tweaks.

use crate::air::AirPicture;
use crate::planet::MainCamera;
use crate::planet::{Planet, Tweaks};
use bevy::{
    camera::visibility::NoFrustumCulling, light::NotShadowCaster, prelude::*, reflect::TypePath,
    render::render_resource::AsBindGroup, shader::ShaderRef,
};

/// What the sky looks like. Change it while running and the sky follows.
#[derive(Resource, Clone, Debug)]
pub struct Sky {
    pub horizon: Color,
    pub zenith: Color,
    /// The colour of the glow around the sun.
    pub glow: Color,
    /// How much of the sky is cloud, from none to overcast.
    pub cloudiness: f32,
    /// The camera is exposed for sunlit ground, which is far brighter than a colour on a
    /// screen; this lifts the sky to sit with it.
    pub brightness: f32,
    /// How far into the night it is, from 0 by day to 1 in full dark: the stars and the
    /// moon, which stands opposite the sun.
    pub night: f32,
    /// Over the atmosphere, how much of this sky's own colours to draw: none by day, all
    /// once the sun is down. The atmosphere's sky goes dark as soon as the sun sets, where a
    /// real one glows for a while; the dusk and the night are drawn in the colours chosen
    /// for them.
    pub twilight: f32,
    /// How the stars have turned with the sky: about which axis, and how far, in radians.
    pub spin_axis: Vec3,
    pub spin: f32,
    /// Whether the sky's colour comes from Bevy's atmosphere (`air`), so that this draws
    /// only the clouds, the stars and the moon over it, lit as the sun lights the world.
    pub over_air: bool,
    /// Land beyond what the world draws, below the horizon, if a world asks for it (D273).
    pub far_land: Option<FarLand>,
}

/// Land beyond the world's edge, drawn in the sky below the horizon as shipped games draw
/// the hills past their maps (D273): ridges one behind another, each paler in the haze than
/// the one before, the farthest on the horizon. The stand-in planet is so small that from a
/// height its ground curves away well below the horizon of the air, which is Earth's
/// (`air`), and the air's own ground showed there as a dark band. Over the air only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FarLand {
    /// What covers it, seen from afar: its forest's canopy, as an albedo.
    pub canopy: Color,
    /// How high its farthest ridges rise over the horizon, in degrees.
    pub hills: f32,
}

impl Sky {
    /// A sky for daytime: no stars, and the sky not turned.
    pub fn by_day(
        horizon: Color,
        zenith: Color,
        glow: Color,
        cloudiness: f32,
        brightness: f32,
    ) -> Self {
        Self {
            horizon,
            zenith,
            glow,
            cloudiness,
            brightness,
            night: 0.0,
            twilight: 0.0,
            spin_axis: Vec3::Y,
            spin: 0.0,
            over_air: false,
            far_land: None,
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct SkyMaterial {
    #[uniform(0)]
    horizon: LinearRgba,
    #[uniform(0)]
    zenith: LinearRgba,
    #[uniform(0)]
    glow: LinearRgba,
    #[uniform(0)]
    sun_direction: Vec3,
    #[uniform(0)]
    radius_m: f32,
    #[uniform(0)]
    cloudiness: f32,
    #[uniform(0)]
    brightness: f32,
    #[uniform(0)]
    night: f32,
    #[uniform(0)]
    spin_axis: Vec3,
    #[uniform(0)]
    spin: f32,
    /// The sun's (or the moon's) colour times its illuminance in lux, which lights the
    /// clouds when they are drawn over the atmosphere.
    #[uniform(0)]
    sunlight: Vec3,
    #[uniform(0)]
    over_air: u32,
    #[uniform(0)]
    twilight: f32,
    /// The far land's canopy, and in w how high its farthest ridges rise, in radians; w 0
    /// for none.
    #[uniform(0)]
    far_land: Vec4,
    /// The air all round the camera, for the far land's haze.
    #[texture(1, dimension = "cube")]
    #[sampler(2)]
    air: Option<Handle<Image>>,
}

impl Material for SkyMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/sky.wesl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        // Over the atmosphere, clouds cover it and stars add to it.
        if self.over_air != 0 {
            AlphaMode::Premultiplied
        } else {
            AlphaMode::Opaque
        }
    }
}

impl SkyMaterial {
    fn new(sky: &Sky, tweaks: &Tweaks, planet: &Planet, air: Option<&AirPicture>) -> Self {
        let air = air.map(|picture| picture.0.clone());
        let far_land = match (sky.far_land, sky.over_air && air.is_some()) {
            (Some(land), true) => {
                let c = land.canopy.to_linear();
                Vec4::new(c.red, c.green, c.blue, land.hills.to_radians().max(1e-4))
            }
            _ => Vec4::ZERO,
        };
        SkyMaterial {
            far_land,
            air,
            horizon: sky.horizon.into(),
            zenith: sky.zenith.into(),
            glow: sky.glow.into(),
            sun_direction: tweaks.sun_direction.normalize_or(Vec3::Y),
            radius_m: planet.radius_m(),
            cloudiness: sky.cloudiness,
            brightness: sky.brightness,
            night: sky.night,
            spin_axis: sky.spin_axis.normalize_or(Vec3::Y),
            spin: sky.spin,
            sunlight: Vec3::from_slice(&tweaks.sun_color.to_linear().to_f32_array()[..3])
                * tweaks.sun_illuminance,
            over_air: sky.over_air as u32,
            twilight: sky.twilight,
        }
    }
}

#[derive(Component)]
struct SkyDome;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<SkyMaterial>::default())
            .add_systems(Startup, spawn)
            .add_systems(
                Update,
                (
                    update.run_if(
                        resource_changed::<Sky>
                            .or_eager(resource_changed::<Tweaks>)
                            .or_eager(resource_added::<AirPicture>),
                    ),
                    keep_around_camera,
                ),
            );
    }
}

fn spawn(
    mut commands: Commands,
    sky: Res<Sky>,
    tweaks: Res<Tweaks>,
    planet: Res<Planet>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SkyMaterial>>,
) {
    let dome = Sphere::new(1.0)
        .mesh()
        .ico(6)
        .unwrap()
        .with_inverted_winding()
        .unwrap();
    commands.spawn((
        Name::new("Sky"),
        SkyDome,
        Mesh3d(meshes.add(dome)),
        MeshMaterial3d(materials.add(SkyMaterial::new(&sky, &tweaks, &planet, None))),
        Transform::from_scale(Vec3::splat(planet.radius_m() * 20.0)),
        NotShadowCaster,
        NoFrustumCulling,
    ));
}

fn update(
    sky: Res<Sky>,
    tweaks: Res<Tweaks>,
    planet: Res<Planet>,
    air: Option<Res<AirPicture>>,
    mut materials: ResMut<Assets<SkyMaterial>>,
) {
    for (_, material) in materials.iter_mut() {
        *material = SkyMaterial::new(&sky, &tweaks, &planet, air.as_deref());
    }
}

fn keep_around_camera(
    camera: Single<&Transform, (With<MainCamera>, Without<SkyDome>)>,
    mut sky: Single<&mut Transform, With<SkyDome>>,
) {
    sky.translation = camera.translation;
}
