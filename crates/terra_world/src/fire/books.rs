//! The flames' flipbooks (docs/fire.md, "The flipbook format" and "The colour ramp"): the sets
//! the baker writes (`tools/fire/bake/`), each a flame map (R heat, G cover, B soot, A
//! distortion, 8x8 frames), its half-size motion and a sidecar, stacked as the layers of two
//! array textures so one draw reads every set; the one colour ramp every fire's heat is
//! coloured through (`tools/fire/ramp.ts`); and which set each kind of fire is drawn from.

use bevy::{
    asset::RenderAssetUsages,
    image::{CompressedImageFormats, ImageSampler, ImageType},
    prelude::*,
    render::render_resource::{
        Extent3d, TextureDimension, TextureViewDescriptor, TextureViewDimension,
    },
};
use serde::Deserialize;

/// The sets the cards are drawn from, a layer each in this order (`fire.wesl`'s `SET_*`):
/// a hearth's flames (`campfire`), a burning building's (`tongue`) and a torch's, a hearth
/// under `fire.wesl`'s `TORCH_M` tall (docs/fire.md, "The kinds"). The breath's puffs, its fireball and
/// an explosion have sets baked too (`breath-puff`, `fireball`, `explosion`), but are drawn as
/// the breath's marched volume until their composites (stages 7 and 8) put sprites on them;
/// they are left out of the array until then, as each set is 16 MB of the GPU's.
pub const SETS: [&str; 3] = ["campfire", "tongue", "torch"];

/// The frames a set has, and how many across its sheet, as `fire.wesl` reads them.
const FRAMES: u32 = 64;
const COLUMNS: u32 = 8;

/// A set's sidecar, as `tools/fire/bake/pack.ts` writes it.
#[derive(Deserialize, Debug)]
pub struct Sidecar {
    pub frames: u32,
    pub columns: u32,
    pub rows: u32,
    /// Texels a frame across.
    pub frame: u32,
    /// Frames a second, as baked.
    pub fps: f32,
    /// How wide and tall a frame stands in the world, as baked.
    pub metres: f32,
    /// How high up its frame the flame's tips reach at most, 0 to 1.
    pub reach: f32,
    pub motion: Motion,
    pub baked: Baked,
}

#[derive(Deserialize, Debug)]
pub struct Motion {
    pub frame: u32,
    /// The flow a texel's full swing stands for, in frame widths a frame.
    pub scale: f32,
}

#[derive(Deserialize, Debug)]
pub struct Baked {
    /// The warp error from one frame to the next along the vectors, against none: under 1
    /// the vectors bring frames nearer, over 1 they take them apart (the torch's, 1.39).
    pub flow_gain: f32,
}

impl Sidecar {
    /// What `fire.wesl` is told of the set: its rate, its size in metres, its motion's scale,
    /// and whether its frames are blended along its vectors (1) or crossfaded (0), the vectors
    /// used only where they bring a frame nearer the next.
    pub fn told(&self) -> Vec4 {
        let along = if self.baked.flow_gain < 1.0 { 1.0 } else { 0.0 };
        Vec4::new(self.fps, self.metres, self.motion.scale, along)
    }
}

/// The colour ramp's sidecar (`tools/fire/ramp.ts`).
#[derive(Deserialize, Debug)]
pub struct RampSidecar {
    pub rows: Vec<String>,
    /// What the texture's RGB is multiplied by to give the ramp's own.
    pub scale: f32,
}

/// The flipbooks as the fire's material reads them.
pub struct Books {
    pub flames: Image,
    pub motions: Image,
    pub ramp: Image,
    /// A set's [`Sidecar::told`], in the order of [`SETS`].
    pub sets: [Vec4; 3],
    /// x the ramp's scale.
    pub ramp_scale: Vec4,
    /// Each set's [`Sidecar::reach`], x y z in the order of [`SETS`].
    pub reach: Vec4,
}

fn file(path: &str) -> std::path::PathBuf {
    crate::data::assets().join("textures/fire").join(path)
}

/// Reads a set's sidecar. A mistake in one is a mistake in the game, so it stops it.
pub fn sidecar(set: &str) -> Sidecar {
    let path = file(&format!("{set}.json"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|problem| panic!("Could not read {}: {problem}", path.display()));
    let read: Sidecar = serde_json::from_str(&text)
        .unwrap_or_else(|problem| panic!("{}: {problem}", path.display()));
    assert!(
        read.frames == FRAMES
            && read.columns == COLUMNS
            && read.rows * read.columns == read.frames
            && read.motion.frame * 2 == read.frame,
        "{}: fire.wesl reads {FRAMES} frames, {COLUMNS} across, the motion at half size",
        path.display()
    );
    read
}

/// A picture under textures/fire, its values as stored (`srgb` false) or decoded from sRGB.
fn picture(path: &str, srgb: bool) -> Image {
    let at = file(path);
    let bytes = std::fs::read(&at)
        .unwrap_or_else(|problem| panic!("Could not read {}: {problem}", at.display()));
    Image::from_buffer(
        &bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        srgb,
        ImageSampler::linear(),
        RenderAssetUsages::RENDER_WORLD,
    )
    .unwrap_or_else(|problem| panic!("{}: {problem}", at.display()))
}

/// Pictures of one size stacked as the layers of an array texture.
fn stack(layers: Vec<Image>, sampler: ImageSampler) -> Image {
    let size = layers[0].texture_descriptor.size;
    let format = layers[0].texture_descriptor.format;
    let mut data = Vec::new();
    for layer in &layers {
        assert!(
            layer.texture_descriptor.size == size && layer.texture_descriptor.format == format,
            "every fire set's maps are the same size"
        );
        data.extend_from_slice(layer.data.as_deref().unwrap_or_default());
    }
    let mut image = Image::new(
        Extent3d {
            depth_or_array_layers: layers.len() as u32,
            ..size
        },
        TextureDimension::D2,
        data,
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = sampler;
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..default()
    });
    image
}

/// Reads every set, its motion and sidecar, and the ramp: at startup, once, as the cards
/// cannot draw without them.
pub fn read() -> Books {
    let sidecars = SETS.map(sidecar);
    let flames = stack(
        SETS.iter()
            .map(|set| picture(&format!("{set}.png"), false))
            .collect(),
        ImageSampler::linear(),
    );
    // Sampled unfiltered (SideFX Labs' advice): `fire.wesl` loads its texels.
    let motions = stack(
        SETS.iter()
            .map(|set| picture(&format!("{set}.motion.png"), false))
            .collect(),
        ImageSampler::nearest(),
    );
    let path = file("ramp.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|problem| panic!("Could not read {}: {problem}", path.display()));
    let ramp: RampSidecar = serde_json::from_str(&text)
        .unwrap_or_else(|problem| panic!("{}: {problem}", path.display()));
    assert_eq!(
        ramp.rows.first().map(String::as_str),
        Some("wood"),
        "fire.wesl reads wood from the ramp's first row"
    );
    Books {
        flames,
        motions,
        ramp: picture("ramp.png", true),
        sets: sidecars.each_ref().map(|s| s.told()),
        reach: Vec4::new(sidecars[0].reach, sidecars[1].reach, sidecars[2].reach, 0.0),
        ramp_scale: Vec4::new(ramp.scale, ramp.rows.len() as f32, 0.0, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baked(flow_gain: f32) -> Sidecar {
        Sidecar {
            frames: FRAMES,
            columns: COLUMNS,
            rows: FRAMES / COLUMNS,
            frame: 256,
            fps: 24.0,
            metres: 2.0,
            reach: 0.7,
            motion: Motion {
                frame: 128,
                scale: 0.03,
            },
            baked: Baked { flow_gain },
        }
    }

    #[test]
    fn frames_are_blended_along_vectors_only_where_they_help() {
        assert_eq!(baked(0.85).told().w, 1.0);
        assert_eq!(baked(1.39).told().w, 0.0);
    }

    #[test]
    fn the_shader_names_each_set_by_its_layer() {
        let shader =
            std::fs::read_to_string(crate::data::assets().join("shaders/fire.wesl")).unwrap();
        for (layer, set) in SETS.iter().enumerate() {
            let name = set.to_uppercase();
            assert!(
                shader.contains(&format!("const SET_{name}: i32 = {layer};")),
                "{set}"
            );
        }
    }

    #[test]
    fn every_drawn_set_has_a_sidecar_the_shader_can_read() {
        for set in SETS {
            let read = sidecar(set);
            assert!(
                read.fps > 0.0
                    && read.metres > 0.0
                    && read.motion.scale > 0.0
                    && read.reach > 0.2
                    && read.reach < 1.0,
                "{set}"
            );
            assert!(
                file(&format!("{set}.png")).exists() && file(&format!("{set}.motion.png")).exists(),
                "{set}"
            );
        }
    }
}
