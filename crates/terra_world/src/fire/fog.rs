//! A fire's glow in the air, by docs/fire.md's light rules (Fog): a small fog volume round
//! each of the nearest few kept fires, which only their own lights shine in. In the haze's
//! fog volume, which reaches every way for 120 m, a blaze's tens of millions of lumens lit a
//! ball of air brighter than the burning house it stood in (fire sheet, round 5 and round 6);
//! the haze's air is thin and its box so big that Bevy's light through it (`exp(-density x
//! the box's radius)`) leaves a campfire's glow in it unseen. A volume of its own, a few
//! metres across and densest at the flames, glows round the fire and nowhere else, as
//! RDR2's and Unreal's local fog volumes do round a fire (research/fire, `aaa-vfx.md`).
//!
//! By day the glow is gone, as a fire's in the open by day is: the box would only be air the
//! sun lights white. It thickens as the night comes (`Sky::night`).

use super::light::LIGHTS;
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    light::FogVolume,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

/// How many kept fires glow in the air at once: as many as cast shadows on the high preset,
/// the nearest.
pub(super) const FOGS: usize = 4;
/// How thick the air round a fire is at its flames, at full night: at 0.015 the glow was a
/// salmon ball over the logs brighter than its flames, its air lit past where the tonemapper
/// keeps orange (fire sheet, stage 4); a real fire's is a faint warm halo.
const DENSITY: f32 = 0.004;
/// How far round a fire's light its glow reaches, for each metre of its flames' half-width,
/// and the least and most, metres.
const REACH_A_METRE: f32 = 6.0;
const REACH_M: [f32; 2] = [1.5, 4.0];

/// A fire's glow to show: where its light hangs, how far round it its air glows, and how
/// strongly (its light's ease with distance).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Glow {
    pub at: Vec3,
    pub reach: f32,
    pub strength: f32,
}

impl Glow {
    /// The glow round a kept fire's light of so wide a flame (its half-width, metres).
    pub fn of(at: Vec3, radius: f32, strength: f32) -> Glow {
        Glow {
            at,
            reach: (radius * REACH_A_METRE).clamp(REACH_M[0], REACH_M[1]),
            strength,
        }
    }
}

/// The glows the lights want this frame, nearest kept fires first; set by `light`.
#[derive(Resource, Default, Debug)]
pub(super) struct Glows(pub Vec<Glow>);

/// The fog volumes, one a glow.
#[derive(Resource)]
struct Volumes(Vec<Entity>);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Glows>()
        .add_systems(Startup, setup)
        .add_systems(PostUpdate, show.after(super::light::light));
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let density = images.add(falloff());
    let volumes = (0..FOGS.min(LIGHTS))
        .map(|i| {
            commands
                .spawn((
                    Name::new(format!("Fire: glow {i}")),
                    FogVolume {
                        // Air, warmed by nothing but the light in it.
                        fog_color: Color::WHITE,
                        density_factor: 0.0,
                        density_texture: Some(density.clone()),
                        absorption: 0.02,
                        scattering: 0.6,
                        // Smoke and hot air scatter forward less than a misty haze: a round
                        // glow, seen from every side, not a beam looking into the fire.
                        scattering_asymmetry: 0.3,
                        ..default()
                    },
                    Transform::IDENTITY,
                    Visibility::Hidden,
                ))
                .id()
        })
        .collect();
    commands.insert_resource(Volumes(volumes));
}

/// A ball of air in its box, densest at its middle and none at its edge, so no box shows,
/// falling off as a cube so the halo has no edge of its own.
fn falloff() -> Image {
    const N: u32 = 16;
    let mut data = Vec::with_capacity((N * N * N) as usize);
    for z in 0..N {
        for y in 0..N {
            for x in 0..N {
                let p = (Vec3::new(x as f32, y as f32, z as f32) + 0.5) / N as f32 * 2.0 - 1.0;
                let t = (1.0 - p.length()).clamp(0.0, 1.0);
                data.push((t * t * t * 255.0).round() as u8);
            }
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: N,
            height: N,
            depth_or_array_layers: N,
        },
        TextureDimension::D3,
        data,
        TextureFormat::R8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    let mut sampler = ImageSamplerDescriptor::linear();
    sampler.set_address_mode(ImageAddressMode::ClampToEdge);
    image.sampler = ImageSampler::Descriptor(sampler);
    image
}

/// How thick a glow's air is at its middle: none by day, all of `DENSITY` by full night.
pub(super) fn density(glow: &Glow, night: f32) -> f32 {
    DENSITY * glow.strength.clamp(0.0, 1.0) * night.clamp(0.0, 1.0)
}

fn show(
    glows: Res<Glows>,
    sky: Option<Res<crate::sky::Sky>>,
    volumes: Res<Volumes>,
    mut fog: Query<(&mut FogVolume, &mut Transform, &mut Visibility)>,
) {
    let night = sky.map_or(0.0, |sky| sky.night);
    for (i, &entity) in volumes.0.iter().enumerate() {
        let Ok((mut volume, mut transform, mut visibility)) = fog.get_mut(entity) else {
            continue;
        };
        let wanted = glows.0.get(i).map(|glow| (glow, density(glow, night)));
        match wanted {
            Some((glow, thick)) if thick > 0.0 => {
                transform.translation = glow.at;
                transform.scale = Vec3::splat(glow.reach * 2.0);
                if volume.density_factor != thick {
                    volume.density_factor = thick;
                }
                if *visibility != Visibility::Inherited {
                    *visibility = Visibility::Inherited;
                }
            }
            _ => {
                if *visibility != Visibility::Hidden {
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }
}
