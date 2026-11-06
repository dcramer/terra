//! The flags' material (flags.wesl): the scanned Large Grey Tiles with grime and moss in
//! their joints, cracks, the detail grain and wet puddles laid over them per pixel, weighted
//! by the paving mesh's second UV (ground.rs: x how mossy, y how wet).

use crate::scene::material;
use bevy::asset::embedded_asset;
use bevy::image::{ImageArrayLayout, ImageLoaderSettings};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

pub type FlagsMaterial = ExtendedMaterial<StandardMaterial, FlagsExt>;

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "flags.wesl");
    app.add_plugins(MaterialPlugin::<FlagsMaterial>::default());
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct FlagsExt {
    /// The game's ground stack (materials/ground), for its moss.
    #[texture(100, dimension = "2d_array")]
    #[sampler(101)]
    pub stack: Handle<Image>,
    /// The flags' own occlusion, which marks their joints.
    #[texture(102)]
    #[sampler(103)]
    pub arm: Handle<Image>,
    /// The stone detail texture (D362), grey about the middle.
    #[texture(104)]
    #[sampler(105)]
    pub detail: Handle<Image>,
}

impl MaterialExtension for FlagsExt {
    fn fragment_shader() -> ShaderRef {
        "embedded://iso/ruin/flags.wesl".into()
    }
}

pub fn flags(asset: &AssetServer) -> FlagsMaterial {
    let linear = |file: &'static str| -> Handle<Image> {
        asset
            .load_builder()
            .with_settings(|s: &mut ImageLoaderSettings| s.is_srgb = false)
            .load(file)
    };
    ExtendedMaterial {
        base: material(asset, "large_grey_tiles", Color::WHITE),
        extension: FlagsExt {
            stack: asset
                .load_builder()
                .with_settings(|s: &mut ImageLoaderSettings| {
                    s.array_layout = Some(ImageArrayLayout::RowCount { rows: 12 });
                })
                .load("materials/ground/color.jpg"),
            arm: linear("materials/large_grey_tiles/arm.jpg"),
            detail: linear("materials/detail_stone/color.jpg"),
        },
    }
}
