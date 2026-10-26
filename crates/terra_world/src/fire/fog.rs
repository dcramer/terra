//! A fire's glow in the air, by docs/fire.md's light rules (Fog): a small fog volume round
//! each of the nearest few kept fires, which only their own lights shine in. In the haze's
//! fog volume, which reaches every way for 120 m, a blaze's tens of millions of lumens lit a
//! ball of air brighter than the burning house it stood in (fire sheet, round 5 and round 6);
//! a campfire's light in the haze, which is mist-thick by the ground, was the salmon ball over
//! its logs whatever this volume held, until its in-scatter there was taken at a fraction
//! (`scatter::LOCAL_SCATTER`; fire sheet A/B, 2026-10-19). A volume of its own, a few
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
/// keeps orange (fire sheet, stage 4), and at 0.004 still a ball over the campfire (review,
/// 3/10); a real fire's is a faint warm halo.
const DENSITY: f32 = 0.0012;
/// How far round a fire's light its glow reaches, for each metre of its flames' half-width,
/// and the least and most, metres.
const REACH_A_METRE: f32 = 6.0;
const REACH_M: [f32; 2] = [1.5, 4.0];
/// How much thicker in absorption and scattering, and thinner in density, a glow's air is
/// carried than it is, the same air in every product Bevy takes of them. Bevy's volumetric fog
/// (`volumetric_fog.wesl`, MIT OR Apache-2.0) lays the camera's ambient over every fog volume
/// as `exp(-ray length × (absorption + scattering))`, with no density in it: through a glow's
/// box of air nearly clear, each ray short of its back took nearly all the haze's sky light,
/// and the box's corners over the wood showed as pale grey wedges with straight edges (lake
/// film r14, r15 at 9-15 s). So carried, that ambient dies within a hundredth of a millimetre.
const AIR_SCALE: f32 = 1.0e5;
/// The glow's air's absorption and scattering, a metre at density 1.
const ABSORPTION: f32 = 0.02;
const SCATTERING: f32 = 0.6;

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
                    air(density.clone()),
                    Transform::IDENTITY,
                    Visibility::Hidden,
                ))
                .id()
        })
        .collect();
    commands.insert_resource(Volumes(volumes));
}

/// A glow's air: warmed by nothing but the light in it, thick as `show` sets it.
fn air(density: Handle<Image>) -> FogVolume {
    FogVolume {
        fog_color: Color::WHITE,
        density_factor: 0.0,
        density_texture: Some(density),
        absorption: ABSORPTION * AIR_SCALE,
        scattering: SCATTERING * AIR_SCALE,
        // Smoke and hot air scatter forward less than a misty haze: a round glow, seen from
        // every side, not a beam looking into the fire.
        scattering_asymmetry: 0.3,
        ..default()
    }
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
    shut_off: Option<Res<crate::indoor::ShutOff>>,
) {
    // In a room shut off from the sky a fire's air glows as by night (D488).
    let shut_off = shut_off.map_or(0.0, |s| s.0);
    let night = crate::indoor::fires_night(sky.map_or(0.0, |sky| sky.night), shut_off);
    for (i, &entity) in volumes.0.iter().enumerate() {
        let Ok((mut volume, mut transform, mut visibility)) = fog.get_mut(entity) else {
            continue;
        };
        let wanted = glows.0.get(i).map(|glow| (glow, density(glow, night)));
        match wanted {
            Some((glow, thick)) if thick > 0.0 => {
                transform.translation = glow.at;
                transform.scale = Vec3::splat(glow.reach * 2.0);
                if volume.density_factor != thick / AIR_SCALE {
                    volume.density_factor = thick / AIR_SCALE;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Bevy's ambient over a fog volume for a ray so long through it (`volumetric_fog.wesl`).
    fn ambient(volume: &FogVolume, ray_m: f32) -> f32 {
        libm::expf(-ray_m * (volume.absorption + volume.scattering))
    }

    #[test]
    fn a_glow_takes_none_of_the_haze_light_and_keeps_its_own() {
        let mut volume = air(Handle::default());
        let glow = Glow::of(Vec3::ZERO, 1.0, 1.0);
        volume.density_factor = density(&glow, 1.0) / AIR_SCALE;
        // A ray a millimetre into the box, at its edge, takes none of the camera's ambient.
        assert!(ambient(&volume, 0.001) < 1.0e-6);
        // The air's extinction and in-scatter, density times coefficient, are as thin as
        // before: a fire's halo of under a percent a metre at its middle.
        let extinction = volume.density_factor * (volume.absorption + volume.scattering);
        assert!((extinction - DENSITY * (ABSORPTION + SCATTERING)).abs() < 1.0e-7);
        assert!(volume.density_factor * volume.scattering > 0.0);
    }

    #[test]
    fn the_fog_shader_knows_a_glow_by_its_air_and_no_real_fog_for_one() {
        // The patched shader marches a volume short, by its point lights alone, past this
        // absorption a metre (D496); the haze's is 0.05.
        let shader = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../patches/bevy_pbr/src/volumetric_fog/volumetric_fog.wesl"
        ));
        let mark: f32 = shader
            .lines()
            .find_map(|line| line.strip_prefix("const GLOW_ABSORPTION: f32 = "))
            .and_then(|rest| rest.trim_end_matches(';').parse().ok())
            .expect("the shader's glow mark");
        assert!(air(Handle::default()).absorption > mark * 10.0);
        assert!(mark > 10.0, "a real fog absorbs well under 1 a metre");
    }
}
