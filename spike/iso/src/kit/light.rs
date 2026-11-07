//! Light presets as data (D634): `spike/iso/biomes/light/<name>.ron`, one an hour and mood
//! (ruin-night, forest-day, marsh-dusk, city-night-rain), each the key light (the sun or the
//! moon), the sky's fill, the exposure and tone curve, the grade and the distance haze. A
//! biome names its preset and may override a number of it (biome.rs `LightRef`). Nothing in
//! here is a genre's: a street lamp's night and a moonlit ruin are the same fields.

use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use serde::Deserialize;

/// A colour as written: sRGB (as a picker shows it), sRGB with alpha, or linear RGB (as a
/// light's colour is often measured).
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum Colour {
    Srgb(f32, f32, f32),
    Srgba(f32, f32, f32, f32),
    Linear(f32, f32, f32),
}

impl Colour {
    pub fn color(self) -> Color {
        match self {
            Colour::Srgb(r, g, b) => Color::srgb(r, g, b),
            Colour::Srgba(r, g, b, a) => Color::srgba(r, g, b, a),
            Colour::Linear(r, g, b) => Color::linear_rgb(r, g, b),
        }
    }
}

/// Toward the key light: degrees up and round from the south (+z) toward the east (+x), as
/// a sun chart gives it, or a vector (need not be unit length).
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum Toward {
    Angles { elevation: f32, azimuth: f32 },
    Vector(f32, f32, f32),
}

impl Toward {
    pub fn vector(self) -> Vec3 {
        match self {
            Toward::Angles { elevation, azimuth } => {
                let (el, az) = (elevation.to_radians(), azimuth.to_radians());
                Vec3::new(el.cos() * az.sin(), el.sin(), el.cos() * az.cos())
            }
            Toward::Vector(x, y, z) => Vec3::new(x, y, z),
        }
    }

    /// The light's transform, shining from there.
    pub fn transform(self) -> Transform {
        Transform::default().looking_to(-self.vector(), Vec3::Y)
    }
}

/// The hour a preset is for; a layout drawn over the glade's lit hours (the forest) runs the
/// glade's at this one.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hour {
    Day,
    Dusk,
    Night,
}

impl Hour {
    pub fn scene(self) -> crate::scene::Hour {
        match self {
            Hour::Day => crate::scene::Hour::Day,
            Hour::Dusk => crate::scene::Hour::Dusk,
            Hour::Night => crate::scene::Hour::Night,
        }
    }
}

/// Bevy's tone curves by name.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Curve {
    None,
    Reinhard,
    ReinhardLuminance,
    AcesFitted,
    AgX,
    SomewhatBoringDisplayTransform,
    TonyMcMapface,
    BlenderFilmic,
}

impl Curve {
    pub fn tonemapping(self) -> Tonemapping {
        match self {
            Curve::None => Tonemapping::None,
            Curve::Reinhard => Tonemapping::Reinhard,
            Curve::ReinhardLuminance => Tonemapping::ReinhardLuminance,
            Curve::AcesFitted => Tonemapping::AcesFitted,
            Curve::AgX => Tonemapping::AgX,
            Curve::SomewhatBoringDisplayTransform => Tonemapping::SomewhatBoringDisplayTransform,
            Curve::TonyMcMapface => Tonemapping::TonyMcMapface,
            Curve::BlenderFilmic => Tonemapping::BlenderFilmic,
        }
    }
}

/// The key light's cascaded shadows: how many cascades, the first's far bound and the
/// last's, in metres.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Shadows {
    pub cascades: usize,
    pub first: f32,
    pub far: f32,
    /// The penumbra's size (Bevy's `soft_shadow_size`), in metres at the caster.
    pub soft: f32,
}

impl Default for Shadows {
    fn default() -> Self {
        Shadows {
            cascades: 2,
            first: 30.0,
            far: 60.0,
            soft: 1.0,
        }
    }
}

/// The sun or the moon.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Key {
    pub toward: Toward,
    pub colour: Colour,
    /// On the ground, in lux.
    pub lux: f32,
    #[serde(default)]
    pub shadows: Shadows,
}

/// The sky's fill (Bevy's global ambient light).
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Sky {
    pub colour: Colour,
    pub nits: f32,
}

/// The grade over the tone curve: the whole frame's saturation and the midtones'.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Grade {
    pub saturation: f32,
    pub mid_saturation: f32,
}

impl Default for Grade {
    fn default() -> Self {
        Grade {
            saturation: 1.0,
            mid_saturation: 1.0,
        }
    }
}

/// The haze over the land beyond the fight (Bevy's distance fog), linear from `start` to
/// `end` metres off the camera's distance to its focus (so `-3.0` starts 3 m before it).
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Haze {
    pub colour: Colour,
    /// Its colour looking toward the key light, and how tight that glow is (Bevy's default 8).
    pub toward_key: Colour,
    #[serde(default = "eight")]
    pub exponent: f32,
    pub start: f32,
    pub end: f32,
}

fn eight() -> f32 {
    8.0
}

/// A preset: one hour's and mood's light.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Resource)]
#[serde(deny_unknown_fields)]
pub struct Light {
    pub hour: Hour,
    pub key: Key,
    pub sky: Sky,
    pub ev100: f32,
    pub curve: Curve,
    #[serde(default)]
    pub grade: Grade,
    #[serde(default)]
    pub haze: Option<Haze>,
}

/// A biome's change to its preset's numbers: only what it names.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Default)]
#[serde(deny_unknown_fields, default)]
pub struct Overrides {
    pub toward: Option<Toward>,
    pub key_colour: Option<Colour>,
    pub lux: Option<f32>,
    pub sky_colour: Option<Colour>,
    pub nits: Option<f32>,
    pub ev100: Option<f32>,
    pub curve: Option<Curve>,
    pub saturation: Option<f32>,
    pub mid_saturation: Option<f32>,
}

impl Light {
    /// `biomes/light/<name>.ron`.
    pub fn load(name: &str) -> Result<Light, String> {
        let path = super::dir().join("light").join(format!("{name}.ron"));
        super::read(&path, "light.rs")
    }

    pub fn with(mut self, o: &Overrides) -> Light {
        let k = &mut self.key;
        k.toward = o.toward.unwrap_or(k.toward);
        k.colour = o.key_colour.unwrap_or(k.colour);
        k.lux = o.lux.unwrap_or(k.lux);
        self.sky.colour = o.sky_colour.unwrap_or(self.sky.colour);
        self.sky.nits = o.nits.unwrap_or(self.sky.nits);
        self.ev100 = o.ev100.unwrap_or(self.ev100);
        self.curve = o.curve.unwrap_or(self.curve);
        self.grade.saturation = o.saturation.unwrap_or(self.grade.saturation);
        self.grade.mid_saturation = o.mid_saturation.unwrap_or(self.grade.mid_saturation);
        self
    }

    /// The sky's fill and the key light, shadowed, for a layout that spawns its own.
    pub fn spawn(&self, commands: &mut Commands) {
        commands.insert_resource(GlobalAmbientLight {
            color: self.sky.colour.color(),
            brightness: self.sky.nits,
            ..default()
        });
        let s = self.key.shadows;
        commands.spawn((
            DirectionalLight {
                color: self.key.colour.color(),
                illuminance: self.key.lux,
                shadow_maps_enabled: true,
                soft_shadow_size: Some(s.soft),
                ..default()
            },
            CascadeShadowConfigBuilder {
                num_cascades: s.cascades,
                minimum_distance: 1.0,
                first_cascade_far_bound: s.first,
                maximum_distance: s.far,
                overlap_proportion: 0.2,
            }
            .build(),
            self.key.toward.transform(),
        ));
    }

    /// The camera's side: exposure, tone curve, grade and haze (`distance`, the camera's
    /// from its focus).
    pub fn camera(&self, commands: &mut Commands, camera: Entity, distance: f32) {
        let mut c = commands.entity(camera);
        c.insert((
            bevy::camera::Exposure { ev100: self.ev100 },
            self.curve.tonemapping(),
            bevy::render::view::ColorGrading {
                global: bevy::render::view::ColorGradingGlobal {
                    post_saturation: self.grade.saturation,
                    ..default()
                },
                midtones: bevy::render::view::ColorGradingSection {
                    saturation: self.grade.mid_saturation,
                    ..default()
                },
                ..default()
            },
        ));
        if let Some(h) = self.haze {
            c.insert(DistanceFog {
                color: h.colour.color(),
                directional_light_color: h.toward_key.color(),
                directional_light_exponent: h.exponent,
                falloff: FogFalloff::Linear {
                    start: distance + h.start,
                    end: distance + h.end,
                },
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every preset in biomes/light/ parses, and the four the biomes and the city need are there.
    #[test]
    fn every_preset_parses() {
        let mut names = Vec::new();
        for f in std::fs::read_dir(crate::kit::dir().join("light")).unwrap() {
            let p = f.unwrap().path();
            let name = p.file_stem().unwrap().to_str().unwrap().to_string();
            Light::load(&name).unwrap_or_else(|e| panic!("{e}"));
            names.push(name);
        }
        for want in ["ruin-night", "forest-day", "marsh-dusk", "city-night-rain"] {
            assert!(names.iter().any(|n| n == want), "no preset {want}");
        }
    }

    /// The angles' convention: due south at 0 is +z, east at 90 is +x.
    #[test]
    fn angles_point_where_a_sun_chart_says() {
        let at = |elevation, azimuth| Toward::Angles { elevation, azimuth }.vector();
        assert!(at(0.0, 0.0).distance(Vec3::Z) < 1e-6);
        assert!(at(0.0, 90.0).distance(Vec3::X) < 1e-6);
        assert!(at(90.0, 0.0).distance(Vec3::Y) < 1e-6);
    }

    /// An override changes its number and leaves the rest of the preset.
    #[test]
    fn an_override_changes_only_its_number() {
        let base = Light::load("ruin-night").unwrap();
        let o = Overrides {
            lux: Some(7.0),
            ..default()
        };
        let lit = base.with(&o);
        assert_eq!(lit.key.lux, 7.0);
        assert_eq!(
            Light {
                key: base.key,
                ..lit
            },
            base
        );
    }
}
