//! A biome's file, `spike/iso/biomes/<name>.ron` (D634): its layout, light, ground, scatter,
//! decals, water, weather, effects and life, and what the judging loop compares it with.
//! Unknown fields are refused, so a typo fails the run with its line rather than being
//! ignored (kit/mod.rs's `read` says the fix).
//!
//! Fields the kit does not draw yet are parsed and carried (empty or `None` in the three
//! biomes, whose layouts draw their own until the kit's job for it lands); each says which
//! job of the iso-kit line draws it. Nothing assumes a genre: a neon street's wet asphalt
//! and sodium lamps are the same layers, water and lights as a ruin's moss and braziers.

use super::light::{Colour, Overrides};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, bevy::prelude::Resource)]
#[serde(deny_unknown_fields)]
pub struct Biome {
    /// What its stills are judged as, the kinds of reference they stand beside.
    pub kind: Vec<Kind>,
    /// Its references' folder, `~/.cache/terra-review/iso-kit/refs/<refs>/` (iso-kit j2).
    pub refs: String,
    pub layout: Layout,
    pub light: LightRef,
    /// The ground's layers and how they blend (kit/ground.rs draws it, iso-kit j6).
    #[serde(default)]
    pub ground: Ground,
    /// Props and plants strewn by rule (iso-kit j4).
    #[serde(default)]
    pub scatter: Vec<Scatter>,
    /// Flat marks laid on the ground (iso-kit j4).
    #[serde(default)]
    pub decals: Vec<Decal>,
    /// Standing and wet water (iso-kit j5).
    #[serde(default)]
    pub water: Option<Water>,
    /// Wind, rain, snow and mist (iso-kit j7).
    #[serde(default)]
    pub weather: Option<Weather>,
    /// Effect presets the scene plays (iso-kit j6).
    #[serde(default)]
    pub effects: Vec<Effect>,
    /// What lives and moves in it (iso-kit j7).
    #[serde(default)]
    pub life: Vec<Life>,
}

/// The kind of a reference and a still.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Day,
    Dusk,
    Night,
    Rain,
    Snow,
    Water,
    Interior,
}

/// How the scene is built: the land's shape, where the fight stands and the camera looks.
/// For now each runs a biome's own modules (ruin/, wood/, marsh/); the kit's layouts replace
/// them as its jobs land (D634).
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// A ruined court on flat paving between fallen walls (ruin/).
    Ruin,
    /// The glade's wood about a clearing by a pond (wood/ over the glade's modules).
    Forest,
    /// Mud banks between open channels (marsh/).
    Marsh,
}

/// The light: a preset in `biomes/light/`, and the numbers this biome changes of it.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LightRef {
    pub preset: String,
    #[serde(default)]
    pub overrides: Overrides,
}

/// The ground (kit/ground.rs): its layers, the light and hue its floor drifts between over
/// tens of metres, and the scorched points (a camp fire's ash and char).
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Ground {
    /// Bottom first; up to eight, height-blended where their masks overlap.
    pub layers: Vec<Layer>,
    /// The broad variation's darkest and brightest, as factors on the blended colour.
    pub broad: (f32, f32),
    /// Up to four scorched points, x and z.
    pub scorch: Vec<(f32, f32)>,
    /// A laid floor on the layout's paving (kit/laid.rs), if it has one.
    pub laid: Option<Laid>,
}

impl Default for Ground {
    fn default() -> Self {
        Ground {
            layers: Vec::new(),
            broad: (0.78, 1.15),
            scorch: Vec::new(),
            laid: None,
        }
    }
}

/// A laid floor (kit/laid.rs): slabs, pavers, cobbles, bricks or kerbs, a scan of laid
/// units (a folder of `assets/materials/` with its colour, normal and ARM maps) at its true
/// size, drawn straight so its joints run true, weathered in them: grime packed in every
/// joint (the scan's occlusion marks them), hairline cracks, the detail grain, moss out of
/// the joints and over the units, and standing water, each where its mask lays it.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Laid {
    pub material: String,
    /// Metres the scan covers.
    pub metres: f32,
    /// The factor grime puts on the joints' colour (1 clean, 0 black).
    #[serde(default = "joints")]
    pub joints: f32,
    /// The factor on the hairline cracks' colour (1: none).
    #[serde(default = "one")]
    pub cracks: f32,
    /// The detail grain (D362): metres it repeats, how strongly it shows (0 none).
    #[serde(default)]
    pub grain: (f32, f32),
    /// Where moss grows out of the joints and over the units (none when `None`).
    #[serde(default)]
    pub moss: Option<Mask>,
    /// Where water stands on the floor (none when `None`).
    #[serde(default)]
    pub wet: Option<Mask>,
}

fn joints() -> f32 {
    0.3
}

/// A ground layer: a scanned material (a layer of the `materials/ground` stack, by name) at
/// its true size, where its mask lays it, `amount` of the way (0 to 1). One without a
/// material only wets what lies under it (`wets`).
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    #[serde(default)]
    pub material: Option<String>,
    pub mask: Mask,
    #[serde(default = "one")]
    pub amount: f32,
    /// Metres the scan covers, if not its true size (kit/ground.rs's `STACK`).
    #[serde(default)]
    pub metres: Option<f32>,
    /// A factor on the scan's colour: the scans are dark, photographed under cloud.
    #[serde(default = "white")]
    pub tint: (f32, f32, f32),
    /// Broken into drifts: (broad scale, fine scale, band from, band to); a band running
    /// down (0.65, 0.35) lays the drifts' inverse.
    #[serde(default)]
    pub breakup: Option<(f32, f32, f32, f32)>,
    /// How wet the ground reads where it lies, 0 to 1: darker and glossier.
    #[serde(default)]
    pub wets: f32,
    /// Projected from three sides (rock on the steep), not from above.
    #[serde(default)]
    pub triplanar: bool,
}

/// Where a layer lies: everywhere; by the layout's own map of that name (`Site("grass")`,
/// the place's authored weights); or by the kit's rules (kit/ground.rs): worn on paths,
/// mud low by water, wet nearer and lower, moss in shade and creases, snow on what faces
/// up, grime in hollows, steep where the land is.
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Mask {
    Base,
    Site(String),
    Wear,
    Mud,
    Wet,
    Moss,
    Snow,
    Grime,
    Steep,
}

/// A scatter set: models (paths under `assets/`) strewn at `density` a square metre, scaled
/// between `scale`, only where `on` lays (everywhere when `None`).
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Scatter {
    pub models: Vec<String>,
    pub density: f32,
    #[serde(default = "unit")]
    pub scale: (f32, f32),
    #[serde(default)]
    pub on: Option<Mask>,
}

/// Decals: a texture (a path under `assets/`) laid `count` times, `size` metres across
/// between its two numbers, where `on` lays.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Decal {
    pub texture: String,
    pub count: u32,
    pub size: (f32, f32),
    #[serde(default)]
    pub on: Option<Mask>,
}

/// Water: what shape it takes, its face's height, and its colour deep down.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Water {
    pub kind: WaterKind,
    #[serde(default)]
    pub level: f32,
    #[serde(default)]
    pub deep: Option<Colour>,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterKind {
    Pond,
    River,
    Puddles,
    /// A film over a hard floor (wet stone or asphalt), no depth.
    Wet,
}

/// Weather: the wind (x, z, metres a second), and rain, snow and mist from 0 to 1.
#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(deny_unknown_fields, default)]
pub struct Weather {
    pub wind: (f32, f32),
    pub rain: f32,
    pub snow: f32,
    pub mist: f32,
}

/// An effect preset by name (iso-kit j6 names them), and where it plays if not the fight's.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub preset: String,
    #[serde(default)]
    pub at: Option<(f32, f32)>,
}

/// A kind of life (iso-kit j7 names them: insects, birds, beasts, drones) and how many.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Life {
    pub kind: String,
    pub count: u32,
}

fn one() -> f32 {
    1.0
}

fn white() -> (f32, f32, f32) {
    (1.0, 1.0, 1.0)
}

fn unit() -> (f32, f32) {
    (1.0, 1.0)
}

impl Biome {
    /// `biomes/<name>.ron`.
    pub fn load(name: &str) -> Result<Biome, String> {
        let path = super::dir().join(format!("{name}.ron"));
        super::read(&path, "biome.rs")
    }

    /// Its light: the preset with its overrides.
    pub fn light(&self) -> Result<super::light::Light, String> {
        Ok(super::light::Light::load(&self.light.preset)?.with(&self.light.overrides))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every biome in biomes/ parses and its light preset loads, and the three the spike's
    /// scenes alias are there.
    #[test]
    fn every_biome_parses() {
        let mut names = Vec::new();
        for f in std::fs::read_dir(crate::kit::dir()).unwrap() {
            let p = f.unwrap().path();
            if p.extension().is_some_and(|e| e == "ron") {
                let name = p.file_stem().unwrap().to_str().unwrap().to_string();
                let b = Biome::load(&name).unwrap_or_else(|e| panic!("{e}"));
                b.light().unwrap_or_else(|e| panic!("{name}'s light: {e}"));
                names.push(name);
            }
        }
        for want in ["ruin", "forest", "marsh"] {
            assert!(names.iter().any(|n| n == want), "no biome {want}");
        }
    }

    const FULL: &str = r#"(
        kind: [Night, Rain, Water],
        refs: "neon-street",
        layout: Ruin,
        light: (preset: "city-night-rain", overrides: (lux: 30.0, curve: AgX)),
        ground: (layers: [
            (material: "asphalt", mask: Base),
            (material: "grime", mask: Grime, amount: 0.6), (mask: Wet, wets: 0.8),
        ], laid: (material: "concrete_pavement", metres: 1.8, wet: Site("gutter"))),
        scatter: [(models: ["models/bin.glb"], density: 0.05, scale: (0.9, 1.1), on: Some(Wear))],
        decals: [(texture: "decals/oil.png", count: 12, size: (0.5, 1.5))],
        water: Some((kind: Wet, deep: Some(Srgb(0.02, 0.02, 0.03)))),
        weather: (wind: (1.0, 0.5), rain: 0.8, mist: 0.3),
        effects: [(preset: "steam", at: Some((2.0, -1.0)))],
        life: [(kind: "drone", count: 2)],
    )"#;

    /// A biome may name every field the schema has (a non-fantasy one here), and its
    /// overrides change its preset's numbers.
    #[test]
    fn a_full_biome_parses() {
        let b: Biome = crate::kit::parse(FULL, "full.ron", "biome.rs").unwrap();
        assert_eq!(b.ground.layers[1].mask, Mask::Grime);
        assert_eq!(b.weather.as_ref().unwrap().snow, 0.0);
        let lit = b.light().unwrap();
        assert_eq!(lit.key.lux, 30.0);
        assert_eq!(lit.curve, super::super::light::Curve::AgX);
    }

    /// A misspelt field fails with the file, its line, the field and the fix.
    #[test]
    fn a_bad_field_says_where_and_how_to_fix_it() {
        let bad = FULL.replace("weather:", "wether:");
        let e = crate::kit::parse::<Biome>(&bad, "biomes/bad.ron", "biome.rs").unwrap_err();
        for want in [
            "biomes/bad.ron:13:",
            "wether",
            "weather",
            "Fix:",
            "kit/biome.rs",
        ] {
            assert!(e.contains(want), "the error lacks {want:?}: {e}");
        }
        let missing = FULL.replace("refs: \"neon-street\",", "");
        let e = crate::kit::parse::<Biome>(&missing, "m.ron", "biome.rs").unwrap_err();
        assert!(e.contains("refs") && e.contains("add the field"), "{e}");
        let variant = FULL.replace("mask: Grime", "mask: Grim");
        let e = crate::kit::parse::<Biome>(&variant, "v.ron", "biome.rs").unwrap_err();
        assert!(
            e.contains("Grim") && e.contains("one of those names"),
            "{e}"
        );
    }
}
