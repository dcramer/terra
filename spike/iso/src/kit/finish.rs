//! A model's finish, regraded as it loads from its biome's file (`finishes`): a glTF's own
//! materials as Tripo and the packs made them can be wrong for the scene's light. The
//! marsh's judge (round 261107-0607): "dead trees ... painted red-brown bark (cartoon beside
//! the Gord and TQ2 frames); troll and white ghost plastic/chrome sheen; stones' orange
//! blotches read as rust". Tripo's metal-roughness map carries metal and gloss a bark or a
//! hide has not, and its baked base colour is painted, saturated and lit.
//!
//! So each model whose path starts with one of a finish's `models` has its materials'
//! metal scaled, its roughness floored, its specular set, and its base colour graded
//! (desaturated, its warm hues more, darkened, tinted, its highlights capped), as a
//! look-dev artist's material instance overrides a master's parameters (Unreal's material
//! instances, Unity's material variants). The textures are graded in place, once, on the
//! CPU: each glTF's images are its own, so every copy of the model takes the grade and no
//! other model does. Nothing is graded unless a biome's file asks.
//!
//! Graded in the frame the glTF's materials and images are added, before the render world
//! first uploads them (iso-kit j61): Bevy 0.20 re-uploads an image changed later by
//! `write_texture` of its top mip alone (bevy_render `texture/gpu_image.rs`
//! `prepare_asset`, reusing the texture when the descriptor is the same), so a grade after
//! the upload changed only mip 0, which a model 70 px tall at the iso camera never samples
//! (iso-kit j60 graded after the upload). The glTF loader adds each material's
//! `StandardMaterial` (`<label>/std`) and its images in one go (bevy_asset
//! `process_asset_load`), so its `Added` event is the moment.

use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use serde::Deserialize;

/// One finish: the models it regrades and how (each field's default leaves it as is).
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Finish {
    /// Asset paths the models' files start with (`"models/iso/dead-tree-"`), or one
    /// material of a model by its glTF label (`"models/iso/city/sedan.glb#Material0/"`,
    /// the slash so `Material1` is not `Material10`).
    pub models: Vec<String>,
    /// The roughness floor, 0 to 1: no texel smoother than it.
    #[serde(default)]
    pub rough: f32,
    /// The metal's scale: 0 makes every texel a dielectric.
    #[serde(default = "one")]
    pub metal: f32,
    /// Bevy's `reflectance` (0.5 is glTF's 4% dielectric specular), or the model's own.
    #[serde(default)]
    pub reflectance: Option<f32>,
    /// A clear coat over the material, its strength and perceptual roughness (glTF's
    /// KHR_materials_clearcoat, Bevy's `clearcoat`): a car's lacquer, a dielectric 4%
    /// mirror over the paint whose own specular is rougher, so the street's signs mirror
    /// in it sharp and untinted as they do on a real car's roof (uc-reflect j5; Bevy's
    /// clearcoat example, the Filament clear coat model).
    #[serde(default)]
    pub clearcoat: Option<(f32, f32)>,
    /// The base colour's share taken to its own grey (linear luminance), 0 to 1.
    #[serde(default)]
    pub desaturate: f32,
    /// More of it on warm texels (red over green: orange lichen, red-brown paint), at full
    /// warmth; green moss and grey stone keep their colour.
    #[serde(default)]
    pub warm: f32,
    /// The base colour's scale.
    #[serde(default = "one")]
    pub value: f32,
    /// A colour the graded base colour is multiplied by (linear).
    #[serde(default = "white")]
    pub tint: (f32, f32, f32),
    /// The brightest a texel's linear luminance may be after the grade (1: no cap): Tripo's
    /// painted highlights, a dead tree's pale caps.
    #[serde(default = "one")]
    pub ceiling: f32,
}

fn one() -> f32 {
    1.0
}

fn white() -> (f32, f32, f32) {
    (1.0, 1.0, 1.0)
}

/// The images already graded (two materials of a glTF may share one).
#[derive(Resource, Default)]
struct Graded(HashSet<AssetId<Image>>);

pub fn plugin(app: &mut App) {
    // After the asset events are written and before the frame's extraction uploads them.
    app.init_resource::<Graded>().add_systems(
        PostUpdate,
        finish.after(bevy::asset::AssetEventSystems),
    );
}

/// The finish a model's path takes, if any.
fn finish_for(finishes: &[Finish], path: &str) -> Option<usize> {
    let path = path.replace('\\', "/");
    finishes
        .iter()
        .position(|f| f.models.iter().any(|m| path.starts_with(m.as_str())))
}

/// Each material a glTF adds whose file a finish names, finished in the frame it is added:
/// changed untracked, as its `Added` event (and its images') already sends it to the
/// render world this frame, after this system.
fn finish(
    biome: Option<Res<super::biome::Biome>>,
    mut events: MessageReader<AssetEvent<StandardMaterial>>,
    asset: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut graded: ResMut<Graded>,
) {
    let Some(biome) = biome else { return };
    for event in events.read() {
        let AssetEvent::Added { id } = event else {
            continue;
        };
        let Some(path) = asset.get_path(*id) else {
            continue;
        };
        // The path with its label (`sedan.glb#Material0/std`), so a finish may name one
        // material of a model as well as the model.
        let Some(i) = finish_for(&biome.finishes, &path.to_string()) else {
            continue;
        };
        let Some(m) = materials.get_mut_untracked(*id) else {
            continue;
        };
        let n = apply(&biome.finishes[i], m, &mut images, &mut graded.0);
        info!("kit/finish.rs: {path}: finish {i}, {n} textures graded");
    }
}

/// `f` on one material: its factors, and its base colour and metal-roughness images graded
/// in place, each once; the number of images graded.
fn apply(
    f: &Finish,
    m: &mut StandardMaterial,
    images: &mut Assets<Image>,
    done: &mut HashSet<AssetId<Image>>,
) -> usize {
    m.metallic *= f.metal;
    if m.metallic_roughness_texture.is_none() {
        m.perceptual_roughness = m.perceptual_roughness.max(f.rough);
    }
    if let Some(r) = f.reflectance {
        m.reflectance = r;
    }
    if let Some((coat, rough)) = f.clearcoat {
        m.clearcoat = coat;
        m.clearcoat_perceptual_roughness = rough;
    }
    let lut = Lut::new();
    let mut n = 0;
    if let Some(h) = &m.base_color_texture
        && done.insert(h.id())
        && each_texel(images, h.id(), TextureFormat::Rgba8UnormSrgb, |px| {
            let c = colour(&lut, [px[0], px[1], px[2]], f);
            px[..3].copy_from_slice(&c);
        })
    {
        n += 1;
    }
    if let Some(h) = &m.metallic_roughness_texture
        && done.insert(h.id())
        && each_texel(images, h.id(), TextureFormat::Rgba8Unorm, |px| {
            let [g, b] = sheen([px[1], px[2]], f);
            px[1] = g;
            px[2] = b;
        })
    {
        n += 1;
    }
    n
}

/// `f` on every RGBA8 texel of every mip of an image, if it is in `format` (a glTF's colour
/// is sRGB, its metal-roughness linear) and its texels are still in the main world.
fn each_texel(
    images: &mut Assets<Image>,
    id: AssetId<Image>,
    format: TextureFormat,
    f: impl Fn(&mut [u8]),
) -> bool {
    let Some(img) = images.get_mut_untracked(id) else {
        warn!(
            "kit/finish.rs: a finished material's texture {id:?} is not loaded with it; left \
             ungraded. A finish (D634) grades a glTF's embedded images as the glTF loads: \
             embed the model's textures (a .glb), or drop it from the biome's `finishes`"
        );
        return false;
    };
    if img.texture_descriptor.format != format {
        warn!(
            "kit/finish.rs: a texture is {:?}, not {format:?}; left ungraded. A finish \
             (D634) grades a glTF's RGBA8 textures only: drop the model from the biome's \
             `finishes`, or teach each_texel the format",
            img.texture_descriptor.format
        );
        return false;
    }
    let Some(data) = img.data.as_mut() else {
        warn!(
            "kit/finish.rs: texture {id:?} has no texels in the main world (uploaded and \
             dropped); left ungraded. Load the model's materials with MAIN_WORLD usage"
        );
        return false;
    };
    data.chunks_exact_mut(4).for_each(f);
    true
}

/// sRGB bytes to linear and back, by table.
struct Lut {
    linear: [f32; 256],
}

impl Lut {
    fn new() -> Self {
        Self {
            linear: std::array::from_fn(|i| Srgba::gamma_function(i as f32 / 255.0)),
        }
    }

    /// The sRGB byte nearest a linear value.
    fn srgb(&self, v: f32) -> u8 {
        let i = self.linear.partition_point(|&l| l < v).min(255);
        if i > 0 && v - self.linear[i - 1] < self.linear[i] - v {
            (i - 1) as u8
        } else {
            i as u8
        }
    }
}

/// A base colour texel graded by `f`.
fn colour(lut: &Lut, px: [u8; 3], f: &Finish) -> [u8; 3] {
    let [r, g, b] = px.map(|v| lut.linear[v as usize]);
    let l = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let warmth = (2.0 * (r - g) / (r + 1e-4)).clamp(0.0, 1.0);
    let d = (f.desaturate + f.warm * warmth).clamp(0.0, 1.0);
    let (t, c) = ([f.tint.0, f.tint.1, f.tint.2], [r, g, b]);
    let mut c: [f32; 3] = std::array::from_fn(|k| (c[k] + (l - c[k]) * d) * f.value * t[k]);
    let l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    if l > f.ceiling {
        c = c.map(|v| v * f.ceiling / l);
    }
    c.map(|v| lut.srgb(v))
}

/// A metal-roughness texel's roughness (green) and metal (blue) by `f`.
fn sheen([g, b]: [u8; 2], f: &Finish) -> [u8; 2] {
    let floor = (f.rough.clamp(0.0, 1.0) * 255.0).round() as u8;
    let metal = (b as f32 * f.metal.max(0.0)).round().min(255.0) as u8;
    [g.max(floor), metal]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finish() -> Finish {
        crate::kit::parse::<Finish>("(models: [\"models/x\"])", "test", "finish.rs").unwrap()
    }

    /// A finish that asks nothing leaves every texel as it was (so a model a biome names
    /// changes only by what its finish sets).
    #[test]
    fn a_bare_finish_changes_nothing() {
        let (f, lut) = (finish(), Lut::new());
        for px in [
            [0, 0, 0],
            [255, 255, 255],
            [200, 90, 30],
            [40, 120, 50],
            [7, 8, 9],
        ] {
            assert_eq!(colour(&lut, px, &f), px);
        }
        assert_eq!(sheen([40, 200], &f), [40, 200]);
    }

    /// The warm grade takes the orange lichen toward grey and leaves the green moss and
    /// the grey stone as they were.
    #[test]
    fn warm_greys_orange_and_keeps_green() {
        let f = Finish {
            warm: 0.8,
            ..finish()
        };
        let lut = Lut::new();
        let spread = |c: [u8; 3]| c.iter().max().unwrap() - c.iter().min().unwrap();
        let orange = [200, 110, 40];
        assert!(spread(colour(&lut, orange, &f)) < spread(orange) / 3);
        for keep in [[60, 110, 50], [120, 120, 118]] {
            let c = colour(&lut, keep, &f);
            assert!(
                c.iter().zip(keep).all(|(a, b)| a.abs_diff(b) <= 1),
                "{keep:?} -> {c:?}"
            );
        }
    }

    /// Darkened and capped: no texel's luminance above the ceiling, a dark one darkened by
    /// the value alone.
    #[test]
    fn value_and_ceiling_darken() {
        let f = Finish {
            value: 0.5,
            ceiling: 0.2,
            ..finish()
        };
        let lut = Lut::new();
        let lum = |c: [u8; 3]| {
            let [r, g, b] = c.map(|v| lut.linear[v as usize]);
            0.2126 * r + 0.7152 * g + 0.0722 * b
        };
        assert!(lum(colour(&lut, [250, 245, 240], &f)) <= 0.21);
        let dark = [60, 50, 40];
        assert!((lum(colour(&lut, dark, &f)) / lum(dark) - 0.5).abs() < 0.05);
    }

    /// The sheen: no texel smoother than the floor, a rougher one kept, metal scaled.
    #[test]
    fn sheen_floors_roughness_and_drops_metal() {
        let f = Finish {
            rough: 0.8,
            metal: 0.0,
            ..finish()
        };
        assert_eq!(sheen([30, 255], &f), [204, 0]);
        assert_eq!(sheen([250, 10], &f), [250, 0]);
    }

    /// A material's finish reaches its images: the base colour darkened by the value once,
    /// though two of the glTF's materials share it, the metal-roughness made dielectric and
    /// rough, the factors set.
    #[test]
    fn a_finish_grades_a_materials_images_once() {
        use bevy::asset::RenderAssetUsages;
        use bevy::render::render_resource::{Extent3d, TextureDimension};
        let f = Finish {
            value: 0.5,
            rough: 0.8,
            metal: 0.0,
            reflectance: Some(0.3),
            ..finish()
        };
        let size = Extent3d {
            width: 2,
            height: 2,
            depth_or_array_layers: 1,
        };
        let image = |px: &[u8], format| {
            Image::new_fill(
                size,
                TextureDimension::D2,
                px,
                format,
                RenderAssetUsages::default(),
            )
        };
        let mut images = Assets::<Image>::default();
        let base = images.add(image(&[180, 120, 90, 255], TextureFormat::Rgba8UnormSrgb));
        let rm = images.add(image(&[0, 40, 255, 255], TextureFormat::Rgba8Unorm));
        let material = || StandardMaterial {
            base_color_texture: Some(base.clone()),
            metallic_roughness_texture: Some(rm.clone()),
            metallic: 1.0,
            ..default()
        };
        let (mut a, mut b, mut done) = (material(), material(), HashSet::default());
        assert_eq!(apply(&f, &mut a, &mut images, &mut done), 2);
        assert_eq!(apply(&f, &mut b, &mut images, &mut done), 0);
        let lut = Lut::new();
        let lum = |c: &[u8]| {
            let [r, g, b] = [c[0], c[1], c[2]].map(|v| lut.linear[v as usize]);
            0.2126 * r + 0.7152 * g + 0.0722 * b
        };
        let texels = images.get(&base).unwrap().data.clone().unwrap();
        let ratio = lum(&texels[..4]) / lum(&[180, 120, 90]);
        assert!((ratio - 0.5).abs() < 0.03, "base colour scaled by {ratio}, not 0.5");
        assert_eq!(&images.get(&rm).unwrap().data.as_ref().unwrap()[..4], &[0, 204, 0, 255]);
        assert_eq!((a.metallic, a.reflectance), (0.0, 0.3));
    }

    /// A model takes the first finish whose prefix its path starts with.
    #[test]
    fn a_model_takes_its_finish_by_path() {
        let a = Finish {
            models: vec!["models/iso/dead-tree-".into()],
            ..finish()
        };
        let b = Finish {
            models: vec!["models/iso/ogre".into(), "models/iso/treant".into()],
            ..finish()
        };
        let fs = [a, b];
        assert_eq!(finish_for(&fs, "models/iso/dead-tree-b.glb"), Some(0));
        assert_eq!(finish_for(&fs, "models/iso/treant-a.glb"), Some(1));
        assert_eq!(finish_for(&fs, "models\\iso\\ogre.glb"), Some(1));
        assert_eq!(finish_for(&fs, "models/iso/knight.glb"), None);
        // A material's own path, as the glTF loader labels it, takes its model's finish.
        assert_eq!(
            finish_for(&fs, "models/iso/dead-tree-b.glb#Material3/std"),
            Some(0)
        );
    }

    /// A finish naming one material of a model takes that material alone.
    #[test]
    fn a_finish_may_name_one_material() {
        let coat = Finish {
            models: vec!["models/iso/city/sedan.glb#Material0/".into()],
            clearcoat: Some((1.0, 0.04)),
            ..finish()
        };
        let fs = [coat];
        assert_eq!(
            finish_for(&fs, "models/iso/city/sedan.glb#Material0/std"),
            Some(0)
        );
        assert_eq!(
            finish_for(&fs, "models/iso/city/sedan.glb#Material10/std"),
            None
        );
        assert_eq!(
            finish_for(&fs, "models/iso/city/sedan.glb#Material2/std"),
            None
        );
        let mut m = StandardMaterial::default();
        let mut images = Assets::<Image>::default();
        apply(&fs[0], &mut m, &mut images, &mut HashSet::default());
        assert_eq!(
            (m.clearcoat, m.clearcoat_perceptual_roughness),
            (1.0, 0.04)
        );
    }

    /// Every finish in every biome names models that are there, so a typo in a path is
    /// caught here, not by a still that did not change.
    #[test]
    fn every_finish_names_a_model_on_disk() {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        for e in std::fs::read_dir(crate::kit::dir()).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_none_or(|x| x != "ron") {
                continue;
            }
            let biome: crate::kit::biome::Biome = crate::kit::read(&p, "biome.rs").unwrap();
            for f in &biome.finishes {
                for m in &f.models {
                    // A material's label (`#Material0/`) after the file's name.
                    let full = assets.join(m.split('#').next().unwrap());
                    let (dir, stem) = (full.parent().unwrap(), full.file_name().unwrap());
                    let found = std::fs::read_dir(dir).is_ok_and(|d| {
                        d.flatten().any(|x| {
                            x.file_name()
                                .to_string_lossy()
                                .starts_with(&*stem.to_string_lossy())
                        })
                    });
                    assert!(
                        found,
                        "{}: finish model {m} matches no file in assets/",
                        p.display()
                    );
                }
            }
        }
    }
}
