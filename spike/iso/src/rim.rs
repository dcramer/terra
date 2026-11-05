//! A rim on the people and beasts: their glTF materials are swapped, as their scenes come
//! in, for the same material with a fresnel term added to the emission, so a figure's edge
//! catches a thin light of the hour's sky and it reads against the grass (as Pillars of
//! Eternity and Pathfinder light their figures). A light could not do it alone: Bevy
//! culls a light by the view, not the mesh, so a back light also whitens every grass blade.
//! The same material shades what stands under the oaks: their crowns hide the sky, so the
//! sky's fill on a beast, a fern or a rock there is dimmed as the ground's is (the canopy
//! map, scene::canopy drawn over the land), not lit as if in the open.

use crate::party::Actor;
use crate::scene::Hour;
use bevy::asset::embedded_asset;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

pub type RimMaterial = ExtendedMaterial<StandardMaterial, RimExt>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct RimExt {
    /// The rim's colour as it reaches the screen (the glTF materials' emission is not
    /// scaled by the exposure); alpha is the fresnel's power.
    #[uniform(100)]
    pub rim: LinearRgba,
    /// How much sky reaches each point of the land, 1 in the open (scene::canopy), over
    /// `SPAN` metres about the middle.
    #[texture(101)]
    #[sampler(102)]
    pub canopy: Handle<Image>,
}

/// The canopy map's width in metres, about the world's middle.
const SPAN: f32 = 128.0;
const TEXELS: u32 = 256;

/// The canopy map every shaded material reads.
#[derive(Resource)]
struct Canopy(Handle<Image>);

/// A model shaded by the canopy without a rim (a fern, a rock, a log in the wood).
#[derive(Component)]
pub struct Shade;

impl MaterialExtension for RimExt {
    fn fragment_shader() -> ShaderRef {
        "embedded://iso/shaders/rim.wesl".into()
    }
}

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "shaders/rim.wesl");
    app.add_plugins(MaterialPlugin::<RimMaterial>::default())
        .init_resource::<Swapped>()
        .add_systems(Startup, canopy_map)
        .add_systems(Update, (swap, tint).chain());
}

fn canopy_map(
    mut commands: Commands,
    land: Res<crate::terrain::Land>,
    mut images: ResMut<Assets<Image>>,
) {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::ImageSampler;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let trees = crate::forest::tree_spots(&land);
    let mut data = Vec::with_capacity((TEXELS * TEXELS) as usize);
    for y in 0..TEXELS {
        for x in 0..TEXELS {
            let p = (Vec2::new(x as f32, y as f32) + 0.5) / TEXELS as f32 - 0.5;
            let c = crate::scene::canopy(p * SPAN, &trees);
            data.push((c.clamp(0.0, 1.0) * 255.0) as u8);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: TEXELS,
            height: TEXELS,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::R8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    commands.insert_resource(Canopy(images.add(image)));
}

/// A colour the model's own is multiplied by (a pale Tripo coat brought to the beast's).
#[derive(Component, Clone, Copy)]
pub struct Tint(pub Vec3);

#[derive(Resource, Default)]
struct Swapped(HashMap<(AssetId<StandardMaterial>, [u32; 3], bool), Handle<RimMaterial>>);

/// The rim for the hour: faint by day, the sky's blue at dusk and by night.
fn rim_for(hour: Hour) -> LinearRgba {
    match hour {
        Hour::Day => LinearRgba::new(0.10, 0.10, 0.09, 3.6),
        Hour::Dusk => LinearRgba::new(0.08, 0.09, 0.13, 3.4),
        Hour::Night => LinearRgba::new(0.07, 0.10, 0.22, 3.2),
    }
}

#[allow(clippy::too_many_arguments)]
fn swap(
    mut commands: Commands,
    added: Query<
        (Entity, &MeshMaterial3d<StandardMaterial>),
        Added<MeshMaterial3d<StandardMaterial>>,
    >,
    parents: Query<&ChildOf>,
    actors: Query<Option<&Tint>, With<Actor>>,
    shaded: Query<(), With<Shade>>,
    standard: Res<Assets<StandardMaterial>>,
    mut rims: ResMut<Assets<RimMaterial>>,
    mut swapped: ResMut<Swapped>,
    hour: Option<Res<Hour>>,
    canopy: Res<Canopy>,
) {
    let rim = rim_for(hour.map_or(Hour::Dusk, |h| *h));
    for (e, mat) in &added {
        let (tint, rimmed) = match parents.iter_ancestors(e).find_map(|a| actors.get(a).ok()) {
            Some(tint) => (tint.map_or(Vec3::ONE, |t| t.0), true),
            None if parents.iter_ancestors(e).any(|a| shaded.contains(a)) => (Vec3::ONE, false),
            None => continue,
        };
        let key = (mat.id(), tint.to_array().map(f32::to_bits), rimmed);
        let handle = match swapped.0.get(&key) {
            Some(h) => h.clone(),
            None => {
                let Some(base) = standard.get(&mat.0) else {
                    continue;
                };
                let mut base = base.clone();
                let c = base.base_color.to_linear();
                base.base_color =
                    LinearRgba::new(c.red * tint.x, c.green * tint.y, c.blue * tint.z, c.alpha)
                        .into();
                let h = rims.add(ExtendedMaterial {
                    base,
                    extension: RimExt {
                        rim: if rimmed { rim } else { LinearRgba::NONE },
                        canopy: canopy.0.clone(),
                    },
                });
                swapped.0.insert(key, h.clone());
                h
            }
        };
        commands
            .entity(e)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert(MeshMaterial3d(handle));
    }
}

fn tint(hour: Option<Res<Hour>>, swapped: Res<Swapped>, mut rims: ResMut<Assets<RimMaterial>>) {
    let Some(hour) = hour else { return };
    if !hour.is_changed() {
        return;
    }
    let rim = rim_for(*hour);
    for ((.., rimmed), h) in &swapped.0 {
        if !rimmed {
            continue;
        }
        if let Some(mut m) = rims.get_mut(h) {
            m.extension.rim = rim;
        }
    }
}
