//! Volumetric clouds (docs/sky.md stage 2, D515), on the high and ultra presets: the clouds
//! marched as volumes at half resolution, as Horizon's Nubis does (Schneider 2015, 2017), from
//! the same weather map and the same drift as the sky's 2D layer, so both presets put their
//! clouds in one place.
//!
//! This is the main world's half: the 3D noise (`noise3d`), the half-resolution picture the
//! march writes, what the march is told each frame ([`CloudVolume`]), the mark on the camera it
//! marches for, and the veil, an inverted sphere round the camera that lays the picture over
//! the scene before every other see-through thing. The march itself is `render`'s, a compute
//! pass after the depth prepass, so it stops at the ground (`assets/shaders/cloud_volume.wesl`).

use super::clouds::{self, CloudWinds};
use super::moving::Clouds;
use super::noise3d;
use crate::air::AirPicture;
use crate::daylight::Clock;
use crate::planet::{MainCamera, Planet, Tweaks};
use crate::quality::Quality;
use crate::start::Start;
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    light::NotShadowCaster,
    mesh::MeshVertexBufferLayoutRef,
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    reflect::TypePath,
    render::{
        RenderApp,
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_resource::{
            AsBindGroup, CompareFunction, Extent3d, RenderPipelineDescriptor, ShaderType,
            SpecializedMeshPipelineError, TextureDimension, TextureFormat, TextureUsages,
        },
    },
    shader::ShaderRef,
};
use terra_climate::WorldTime;
use terra_gen::tune::{self, Tunable};

mod render;

/// What the march is told each frame, laid out as `cloud_volume.wesl`'s `Params`.
#[derive(Clone, Copy, Default, ShaderType)]
pub struct Params {
    /// From the world to metres east, up and south of the weather map's middle.
    pub to_map: Mat4,
    /// The 2D shapes' offsets (`clouds::offsets`, field 0's four octaves): where the columns of
    /// cloud stand, as the sky's 2D layer draws them.
    pub shapes: [Vec4; 4],
    /// The 3D noise's offsets (`clouds::volume_offsets`).
    pub volume: Vec4,
    /// Whether there is a weather map, the blend to the next step's, the map's units a metre
    /// and the scene's fair-weather cloud (as `SkyMaterial::map`).
    pub map: Vec4,
    /// Without a map: the sky's cloudiness, its rain and its kind.
    pub fallback: Vec4,
    /// Toward the sun in the world, and in w the 2D shapes' noise units a metre.
    pub sun: Vec4,
    /// The sun's colour times its illuminance, lux, and in w the frame, for the jitter.
    pub sunlight: Vec4,
    /// The tunables the march reads: [`HEAPED_COVER`], [`BODY_LOW`].
    pub tuned: Vec4,
    /// The kinds' layers over the sea (`clouds::layers_over_sea`): base, base by rain, top, top
    /// by cover, top by rain.
    pub layers: [Vec4; 5],
    /// The shells the layers are, round the air's sea under the eye (docs/sky.md#heights): the
    /// planet's sea radius and the air's sea radius in the world's metres, and the slab's
    /// bottom and top over the sea.
    pub shell: Vec4,
}

/// The most a heaped column covers, so the 3D noise cuts it into heaps (Nubis's coverage
/// remap); 0.7 let the whole body through as flat-sided slabs, 0.25 cut nearly every heap.
pub static HEAPED_COVER: Tunable = Tunable::new(
    "sky.cloud.heaped_cover",
    0.5,
    0.0..=1.0,
    "the most a heaped column covers: less cuts the heaps smaller, more joins them into slabs",
);

/// Where the body's remapped noise starts, stretched from it to span 0 to 1, so a coverage cuts
/// it gradually rather than all or nothing (the remap leaves it between about 0.7 and 0.95).
pub static BODY_LOW: Tunable = Tunable::new(
    "sky.cloud.body_low",
    0.6,
    0.0..=0.95,
    "where the clouds' body noise is stretched from: higher billows harder, 0 is no stretch",
);

/// Everything the march reads, handed to the render world when it changes.
#[derive(Resource, Clone, ExtractResource)]
#[extract_app(RenderApp)]
pub struct CloudVolume {
    pub params: Params,
    pub noise: Handle<Image>,
    pub maps: Option<[Handle<Image>; 2]>,
    pub air: Option<Handle<Image>>,
    /// The half-resolution picture: rgb the light the clouds send the eye (exposed and
    /// premultiplied), a their opacity; zero where there is no cloud.
    pub target: Handle<Image>,
}

/// The camera the clouds are marched for.
#[derive(Component, Clone, Copy, ExtractComponent)]
#[extract_app(RenderApp)]
pub struct CloudVolumeView;

/// Lays the march's picture over the scene (`cloud_veil.wesl`).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct VeilMaterial {
    #[texture(0)]
    #[sampler(1)]
    picture: Handle<Image>,
}

impl Material for VeilMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/cloud_veil.wesl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Premultiplied
    }

    /// After the sky's dome (-1e6), before every other see-through thing: the clouds stand
    /// behind the smoke and the water, and over the dome's stars and far land.
    fn depth_bias(&self) -> f32 {
        -5.0e5
    }

    /// Over everything opaque: the march has already stopped at the ground.
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_compare = Some(CompareFunction::Always);
            depth.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}

#[derive(Component)]
struct Veil;

pub(super) struct VolumePlugin;

impl Plugin for VolumePlugin {
    fn build(&self, app: &mut App) {
        tune::register(&[&HEAPED_COVER, &BODY_LOW]);
        app.add_plugins((
            MaterialPlugin::<VeilMaterial>::default(),
            ExtractResourcePlugin::<CloudVolume>::default(),
            ExtractComponentPlugin::<CloudVolumeView>::default(),
            render::RenderPlugin,
        ))
        .add_systems(Startup, bake)
        .add_systems(Update, keep_veil_around)
        .add_systems(
            PostUpdate,
            tell.after(bevy::transform::TransformSystems::Propagate),
        );
    }
}

/// The noise, baked once on every core.
fn bake(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let noise = images.add(noise_image());
    commands.insert_resource(Noise(noise));
}

/// The baked noise as a 3D image with its mips. Built uninitialised and given its data after:
/// `Image::new` asserts the data is the top level alone (in debug builds only), and the bake
/// carries every level.
fn noise_image() -> Image {
    let side = noise3d::SIDE;
    let mut image = Image::new_uninit(
        Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: side,
        },
        TextureDimension::D3,
        // Amounts, not colours.
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(noise3d::bake());
    image.texture_descriptor.mip_level_count = noise3d::LEVELS;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    image
}

#[derive(Resource)]
struct Noise(Handle<Image>);

/// A picture of half the view's size for the march to write.
fn target(size: UVec2) -> Image {
    let mut image = Image::new_target_texture(
        size.x.max(1),
        size.y.max(1),
        TextureFormat::Rgba16Float,
        None,
    );
    image.asset_usage = RenderAssetUsages::RENDER_WORLD;
    image.texture_descriptor.usage =
        TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    image.sampler = ImageSampler::linear();
    image
}

/// Tells the march where the clouds are this frame, keeps its picture the view's half size,
/// and marks the camera and shows the veil on high and ultra only.
#[allow(clippy::too_many_arguments)]
fn tell(
    mut commands: Commands,
    (quality, noise, clouds, tweaks, mut sky): (
        Option<Res<Quality>>,
        Option<Res<Noise>>,
        Res<Clouds>,
        Res<Tweaks>,
        ResMut<super::Sky>,
    ),
    (clock, fields, start, air, planet, air_scale): (
        Option<Res<Clock>>,
        Option<Res<crate::climate::ClimateFields>>,
        Option<Res<Start>>,
        Option<Res<AirPicture>>,
        Res<Planet>,
        Option<Res<crate::air::AirScale>>,
    ),
    camera: Option<Single<(Entity, &Camera, Has<CloudVolumeView>), With<MainCamera>>>,
    volume: Option<ResMut<CloudVolume>>,
    mut veil: Query<(&mut Visibility, &MeshMaterial3d<VeilMaterial>), With<Veil>>,
    (mut images, mut meshes, mut veils, mut frame): (
        ResMut<Assets<Image>>,
        ResMut<Assets<Mesh>>,
        ResMut<Assets<VeilMaterial>>,
        Local<u32>,
    ),
) {
    let (Some(noise), Some(camera)) = (noise, camera) else {
        return;
    };
    let (entity, view, marked) = *camera;
    // Over the air only, and only where the start gives the air's flat frame.
    let on = quality.is_none_or(|q| q.volume_clouds()) && sky.over_air && start.is_some();
    if !on {
        if marked {
            commands.entity(entity).remove::<CloudVolumeView>();
            // So the dome draws its 2D clouds again.
            sky.set_changed();
        }
        for (mut shown, _) in &mut veil {
            shown.set_if_neq(Visibility::Hidden);
        }
        return;
    }
    if !marked {
        commands.entity(entity).insert(CloudVolumeView);
        sky.set_changed();
    }
    let size = (view.physical_viewport_size().unwrap_or(UVec2::ONE) / 2).max(UVec2::ONE);
    // A new picture when the view's size changes, so the veil's material is prepared again.
    let target = match volume.as_ref().map(|v| v.target.clone()) {
        Some(handle) if images.get(&handle).is_some_and(|i| i.size() == size) => handle,
        _ => images.add(target(size)),
    };
    match veil.single_mut() {
        Ok((mut shown, material)) => {
            shown.set_if_neq(Visibility::Inherited);
            if let Some(m) = veils.get(&material.0)
                && m.picture != target
            {
                veils.get_mut(&material.0).unwrap().picture = target.clone();
            }
        }
        Err(_) => {
            let sphere = Sphere::new(1.0)
                .mesh()
                .ico(3)
                .unwrap()
                .with_inverted_winding()
                .unwrap();
            commands.spawn((
                Name::new("Cloud veil"),
                Veil,
                Mesh3d(meshes.add(sphere)),
                MeshMaterial3d(veils.add(VeilMaterial {
                    picture: target.clone(),
                })),
                Transform::from_scale(Vec3::splat(planet.radius_m() * 10.0)),
                NotShadowCaster,
                NoFrustumCulling,
            ));
        }
    }
    let time = clock.map_or(WorldTime(0.0), |clock| clock.time());
    let winds = match (&fields, &start) {
        (Some(fields), Some(start)) => CloudWinds::of_world(fields, start),
        _ => CloudWinds::default(),
    };
    // Anchored at the map's middle, as the 2D shapes' offsets are.
    let anchor = clouds.middle;
    let sun = tweaks.sun_direction.normalize_or(Vec3::Y);
    let sunlight = Vec3::from_slice(&tweaks.sun_color.to_linear().to_f32_array()[..3])
        * tweaks.sun_illuminance;
    *frame = frame.wrapping_add(1);
    // Low layers stand over the low ground the start is on, its height over the sea.
    let ground = start
        .as_ref()
        .map_or(0.0, |s| s.0.translation.length() - planet.sea_radius_m());
    let (layers, slab) = clouds::layers_over_sea(ground);
    let air_scale = air_scale.map_or(crate::air::AirScale(crate::air::HAZE_SCALE), |s| *s);
    let params = Params {
        to_map: clouds.to_map,
        shapes: [
            clouds.offsets[0],
            clouds.offsets[1],
            clouds.offsets[2],
            clouds.offsets[3],
        ],
        volume: clouds::volume_offsets(&winds, clouds::seconds(time), anchor),
        map: Vec4::new(
            if clouds.maps.is_some() { 1.0 } else { 0.0 },
            clouds.blend,
            1.0 / clouds::MAP_SIDE_M,
            sky.fair,
        ),
        fallback: Vec4::new(sky.cloudiness, sky.deck.y, 1.0 - sky.deck.x, 0.0),
        sun: sun.extend((1.0 / clouds::FIELD_M[0]) as f32),
        sunlight: sunlight.extend((*frame % 1024) as f32),
        tuned: Vec4::new(HEAPED_COVER.get(), BODY_LOW.get(), 0.0, 0.0),
        layers,
        shell: Vec4::new(
            planet.sea_radius_m(),
            crate::air::sea_sphere_radius_m(air_scale),
            slab.x,
            slab.y,
        ),
    };
    commands.insert_resource(CloudVolume {
        params,
        noise: noise.0.clone(),
        maps: clouds.maps.clone(),
        air: air.map(|a| a.0.clone()),
        target,
    });
}

/// Keeps the veil round the camera, as the sky's dome is.
fn keep_veil_around(
    camera: Single<&Transform, (With<MainCamera>, Without<Veil>)>,
    mut veil: Query<&mut Transform, With<Veil>>,
) {
    for mut t in &mut veil {
        t.translation = camera.translation;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The image holds every mip level the descriptor names, as the GPU upload reads them.
    #[test]
    fn noise_image_holds_every_level() {
        let image = noise_image();
        let side = noise3d::SIDE as usize;
        let levels: usize = (0..noise3d::LEVELS).map(|l| (side >> l).pow(3) * 4).sum();
        assert_eq!(image.data.as_ref().map(Vec::len), Some(levels));
        assert_eq!(image.texture_descriptor.mip_level_count, noise3d::LEVELS);
    }
}
