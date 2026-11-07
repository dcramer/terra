//! The kit's laid floor (D634): slabs, pavers, cobbles, bricks or kerbs, a scan of laid units
//! at its true size drawn straight through StandardMaterial's own maps on the layout's
//! paving mesh, so its joints run true, with the weather laid over it per pixel as a decal
//! layer would be (laid.wesl): grime packed in every joint and moss growing out of them (the
//! scan's own occlusion marks the joints), hairline cracks, the detail grain (D362), and
//! standing water, darker and near-mirror smooth (Lagarde, "Water drop 2b: dynamic rain and
//! its effects", 2013). A biome's `ground.laid` says which scan and how weathered; where moss
//! and water lie are the layout's maps by name, carried per vertex (`Place::sites` here:
//! the second UV's x and y, the colour's alpha). A ruin's court and a city's pavement are
//! the same code.

use super::biome::{Laid, Mask};
use super::ground::STACK;
use crate::scene;
use bevy::asset::embedded_asset;
use bevy::image::{ImageArrayLayout, ImageLoaderSettings};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

pub type LaidMaterial = ExtendedMaterial<StandardMaterial, LaidExt>;

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "laid.wesl");
    app.add_plugins(MaterialPlugin::<LaidMaterial>::default());
}

/// The source the shader reads for "everywhere" and "nowhere" (`source` in laid.wesl).
const ONE: f32 = 8.0;
const NONE: f32 = 9.0;

/// The floor's weathering as the shader reads it.
#[derive(Clone, Copy, Debug, Default, Reflect, ShaderType)]
pub struct Params {
    /// x the joints' grime factor, y the cracks' factor, z the grain's metres, w its strength.
    pub look: Vec4,
    /// x the moss's source, y the water's (`source`), z the moss scan's metres.
    pub masks: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct LaidExt {
    #[uniform(106)]
    pub params: Params,
    /// The game's ground stack (materials/ground), for its moss.
    #[texture(100, dimension = "2d_array")]
    #[sampler(101)]
    pub stack: Handle<Image>,
    /// The scan's own occlusion, which marks its joints.
    #[texture(102)]
    #[sampler(103)]
    pub arm: Handle<Image>,
    /// The stone detail texture (D362), grey about the middle.
    #[texture(104)]
    #[sampler(105)]
    pub detail: Handle<Image>,
}

impl MaterialExtension for LaidExt {
    fn fragment_shader() -> ShaderRef {
        "embedded://iso/kit/laid.wesl".into()
    }
}

/// The layout's paving: the names of the maps its mesh carries, in the second UV's x and y
/// and the colour's alpha (the colour's rgb is the layout's own grime on the scan).
pub struct Place<'a> {
    pub sites: [&'a str; 3],
}

/// The shader's source for a laid floor's mask, or what is wrong and the fix (D556).
fn source(mask: Option<&Mask>, place: &Place) -> Result<f32, String> {
    match mask {
        None => Ok(NONE),
        Some(Mask::Base) => Ok(ONE),
        Some(Mask::Site(name)) => place
            .sites
            .iter()
            .position(|s| s == name)
            .map(|k| k as f32)
            .ok_or_else(|| {
                format!(
                    "the layout's paving has no map {name:?}; its maps are {}. Fix: name one \
                     of those in Site(...), or Base (kit/laid.rs, D634)",
                    place.sites.join(", ")
                )
            }),
        Some(other) => Err(format!(
            "a laid floor's mask is the layout's map by name or Base, not {other:?}: the \
             kit's rule masks are baked from a land the paving does not have. Fix: give the \
             layout's paving that map (kit/laid.rs `Place`) and name it in Site(...) (D634)"
        )),
    }
}

/// A laid floor as the shader's uniform, or what is wrong with it and the fix.
pub fn params(laid: &Laid, place: &Place) -> Result<Params, String> {
    let moss = source(laid.moss.as_ref(), place).map_err(|e| format!("laid moss: {e}"))?;
    let wet = source(laid.wet.as_ref(), place).map_err(|e| format!("laid wet: {e}"))?;
    let moss_m = STACK
        .iter()
        .find(|(n, _)| *n == "moss_004")
        .map_or(1.1, |s| s.1);
    Ok(Params {
        look: Vec4::new(laid.joints, laid.cracks, laid.grain.0.max(1e-3), laid.grain.1),
        masks: Vec4::new(moss, wet, moss_m, 0.0),
    })
}

/// The floor's material: the scan through StandardMaterial, the weather over it. A bad
/// floor panics with what to fix.
pub fn material(asset: &AssetServer, laid: &Laid, place: &Place) -> LaidMaterial {
    let params = params(laid, place).unwrap_or_else(|e| panic!("{e}"));
    let linear = |file: String| -> Handle<Image> {
        asset
            .load_builder()
            .with_settings(|s: &mut ImageLoaderSettings| s.is_srgb = false)
            .load(file)
    };
    ExtendedMaterial {
        base: scene::material(asset, &laid.material, Color::WHITE),
        extension: LaidExt {
            params,
            stack: asset
                .load_builder()
                .with_settings(|s: &mut ImageLoaderSettings| {
                    s.array_layout = Some(ImageArrayLayout::RowCount {
                        rows: STACK.len() as u32,
                    });
                })
                .load("materials/ground/color.jpg"),
            arm: linear(format!("materials/{}/arm.jpg", laid.material)),
            detail: linear("materials/detail_stone/color.jpg".into()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLACE: Place = Place {
        sites: ["mossy", "puddled", "walls"],
    };

    fn laid(moss: Option<Mask>, wet: Option<Mask>) -> Laid {
        Laid {
            material: "large_grey_tiles".into(),
            metres: 3.0,
            joints: 0.3,
            cracks: 0.25,
            grain: (0.5, 0.6),
            moss,
            wet,
        }
    }

    /// A mask names the paving's map it reads, Base reads everywhere, none nowhere.
    #[test]
    fn masks_read_the_pavings_maps_by_name() {
        let p = params(
            &laid(Some(Mask::Site("walls".into())), Some(Mask::Base)),
            &PLACE,
        )
        .unwrap();
        assert_eq!(p.masks.x, 2.0);
        assert_eq!(p.masks.y, ONE);
        let p = params(&laid(None, Some(Mask::Site("puddled".into()))), &PLACE).unwrap();
        assert_eq!((p.masks.x, p.masks.y), (NONE, 1.0));
    }

    /// A map the paving lacks, or a rule mask it has no land for, fails with the fix.
    #[test]
    fn a_bad_mask_says_the_fix() {
        let e = params(&laid(Some(Mask::Site("lichen".into())), None), &PLACE).unwrap_err();
        assert!(e.contains("lichen") && e.contains("mossy") && e.contains("Fix:"), "{e}");
        let e = params(&laid(None, Some(Mask::Wet)), &PLACE).unwrap_err();
        assert!(e.contains("Wet") && e.contains("Fix:"), "{e}");
    }

    /// The ruin's floor resolves on the court's paving.
    #[test]
    fn the_ruins_floor_resolves() {
        let b = super::super::biome::Biome::load("ruin").unwrap();
        let l = b.ground.laid.expect("the ruin lays its court");
        params(&l, &PLACE).unwrap();
    }
}
