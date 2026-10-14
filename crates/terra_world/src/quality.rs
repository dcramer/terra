//! How much the GPU is asked to draw: a quality preset, as every PC game has (D239).
//! Chosen from the GPU when the game starts, an integrated one low and any other high, as
//! shipped games pick their first preset from the adapter; `--quality` or the options menu
//! overrides it. Each system that draws something costly reads what the preset gives it
//! here and follows it when it changes, so a preset is these numbers and nothing else.
//!
//! Low is the integrated GPU's: 30 frames a second at 1080p shown, drawn at a lower
//! resolution and scaled up (`render_scale`), as consoles and low presets do. High is
//! what the game is made to look like, on a mid-range discrete GPU at 60 (D25).

use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::renderer::RenderAdapterInfo;

/// A quality preset.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq, Default, Reflect)]
pub enum Quality {
    Low,
    Medium,
    #[default]
    High,
}

/// The preset asked for, by `--quality` or the options menu; `None` lets the GPU choose.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Asked(pub Option<Quality>);

/// What the GPU chose, before anything asked for another.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Automatic(pub Quality);

impl Quality {
    pub const ALL: [Quality; 3] = [Quality::Low, Quality::Medium, Quality::High];

    pub fn name(self) -> &'static str {
        match self {
            Quality::Low => "low",
            Quality::Medium => "medium",
            Quality::High => "high",
        }
    }

    pub fn from_name(name: &str) -> Option<Quality> {
        Quality::ALL.into_iter().find(|q| q.name() == name)
    }

    /// The preset for a GPU, by its kind as wgpu names it: an integrated GPU shares the
    /// CPU's memory and has a few compute units, so it starts low.
    pub fn for_device(device_type: &str) -> Quality {
        match device_type {
            "IntegratedGpu" => Quality::Low,
            _ => Quality::High,
        }
    }

    /// The share of the screen's width and height the world is drawn at, before it is
    /// scaled up to it: a pixel's shading is most of an integrated GPU's frame.
    pub fn render_scale(self) -> f32 {
        match self {
            Quality::Low => 0.6,
            Quality::Medium | Quality::High => 1.0,
        }
    }

    /// The sun's shadow map's side, in texels, its cascades, how far the first reaches and
    /// how far shadows are drawn at all, in metres.
    pub fn shadows(self) -> Shadows {
        match self {
            Quality::High => Shadows {
                map: 4096,
                cascades: 4,
                first_m: 24.0,
                most_m: 300.0,
            },
            Quality::Medium => Shadows {
                map: 2048,
                cascades: 3,
                first_m: 20.0,
                most_m: 200.0,
            },
            Quality::Low => Shadows {
                map: 2048,
                cascades: 2,
                first_m: 12.0,
                most_m: 60.0,
            },
        }
    }

    /// Shade in corners (screen-space ambient occlusion) and short contact shadows.
    pub fn occlusion(self) -> bool {
        self != Quality::Low
    }

    /// Shadows' edges softened by many samples a pixel, smoothed over frames; or the
    /// hardware's four, at the low preset.
    pub fn soft_shadows(self) -> bool {
        self != Quality::Low
    }

    /// A soft glow round the sun and bright sky.
    pub fn bloom(self) -> bool {
        self != Quality::Low
    }

    /// How many steps the haze's light is marched in, if it is drawn at all.
    pub fn haze_steps(self) -> Option<u32> {
        match self {
            Quality::High => Some(48),
            Quality::Medium => Some(24),
            Quality::Low => None,
        }
    }

    /// How many layers relief mapping takes through a material's height, bark's and
    /// stone's furrows; none is flat, its normal map alone.
    pub fn relief_layers(self) -> f32 {
        match self {
            Quality::High => 16.0,
            Quality::Medium => 8.0,
            Quality::Low => 0.0,
        }
    }

    /// How far each level of detail reaches, as a share of the distances it is made for:
    /// buildings', trees' and plants' finer details give way sooner. It is the
    /// `detail::DetailScale` the preset sets (D230).
    pub fn detail_reach(self) -> f32 {
        match self {
            Quality::High => 1.0,
            Quality::Medium => 0.8,
            Quality::Low => 0.5,
        }
    }

    /// How much of the grass is grown, as a share of the blades and of how far out.
    pub fn grass(self) -> f32 {
        match self {
            Quality::High => 1.0,
            Quality::Medium => 0.75,
            Quality::Low => 0.5,
        }
    }
}

/// The sun's shadows at a preset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadows {
    pub map: usize,
    pub cascades: usize,
    pub first_m: f32,
    pub most_m: f32,
}

pub fn plugin(app: &mut App) {
    let asked = app
        .world()
        .get_resource::<crate::shell::Options>()
        .and_then(|options| options.quality);
    app.init_resource::<Quality>()
        .init_resource::<crate::detail::DetailScale>()
        .insert_resource(Asked(asked))
        .insert_resource(Forced(asked.is_some()))
        .add_systems(First, choose)
        .add_systems(PostUpdate, follow_relief::<StandardMaterial>);
}

/// A material drawn by Bevy's standard shader, which may be relief-mapped.
pub trait Relieved: Asset {
    fn standard(&mut self) -> &mut StandardMaterial;
    fn relieved(&self) -> bool;
}

impl Relieved for StandardMaterial {
    fn standard(&mut self) -> &mut StandardMaterial {
        self
    }
    fn relieved(&self) -> bool {
        self.depth_map.is_some()
    }
}

impl<E: MaterialExtension> Relieved for ExtendedMaterial<StandardMaterial, E> {
    fn standard(&mut self) -> &mut StandardMaterial {
        &mut self.base
    }
    fn relieved(&self) -> bool {
        self.base.depth_map.is_some()
    }
}

/// Relief mapping at the preset's depth (`Quality::relief_layers`), on every material of a
/// kind that has a height map, as each is made and whenever the preset changes; with none,
/// its height map is put aside, so the shader is built without it, and given back when a
/// preset has it again. A system for each kind of material that is relief-mapped.
pub fn follow_relief<M: Relieved>(
    quality: Res<Quality>,
    mut made: MessageReader<AssetEvent<M>>,
    mut materials: ResMut<Assets<M>>,
    mut aside: Local<HashMap<AssetId<M>, Handle<Image>>>,
    mut applied: Local<Option<Quality>>,
) {
    let changed = *applied != Some(*quality);
    *applied = Some(*quality);
    let new: Vec<AssetId<M>> = made
        .read()
        .filter_map(|event| match event {
            AssetEvent::Added { id } => Some(*id),
            _ => None,
        })
        .collect();
    let ids: Vec<AssetId<M>> = if changed {
        materials
            .iter()
            .filter(|(id, material)| material.relieved() || aside.contains_key(id))
            .map(|(id, _)| id)
            .collect()
    } else {
        new.into_iter()
            .filter(|id| materials.get(*id).is_some_and(Relieved::relieved))
            .collect()
    };
    let layers = quality.relief_layers();
    for id in ids {
        let Some(mut material) = materials.get_mut(id) else {
            continue;
        };
        let standard = material.standard();
        if layers <= 0.0 {
            if let Some(map) = standard.depth_map.take() {
                aside.insert(id, map);
            }
        } else {
            if standard.depth_map.is_none() {
                standard.depth_map = aside.remove(&id);
            }
            standard.max_parallax_layer_count = layers;
        }
    }
}

/// Whether `--quality` chose the preset, which the options menu then leaves alone.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Forced(pub bool);

/// The preset in effect: the one asked for, or the GPU's.
fn choose(
    asked: Res<Asked>,
    adapter: Option<Res<RenderAdapterInfo>>,
    automatic: Option<Res<Automatic>>,
    mut commands: Commands,
    mut quality: ResMut<Quality>,
    mut scale: ResMut<crate::detail::DetailScale>,
) {
    let automatic = match (automatic, adapter) {
        (Some(automatic), _) => automatic.0,
        (None, Some(adapter)) => {
            // Bevy does not re-export wgpu's device types; its name is enough.
            let chosen = Quality::for_device(&format!("{:?}", adapter.device_type));
            info!(
                "Graphics: {} for {}, unless asked otherwise.",
                chosen.name(),
                adapter.name
            );
            commands.insert_resource(Automatic(chosen));
            chosen
        }
        (None, None) => Quality::High,
    };
    let wanted = asked.0.unwrap_or(automatic);
    if *quality != wanted {
        *quality = wanted;
    }
    // Every level of detail's distances follow the preset (D230).
    if quality.is_changed() && scale.0 != quality.detail_reach() {
        scale.0 = quality.detail_reach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_integrated_gpu_starts_low_and_any_other_high() {
        assert_eq!(Quality::for_device("IntegratedGpu"), Quality::Low);
        assert_eq!(Quality::for_device("DiscreteGpu"), Quality::High);
        // Software rendering draws pictures to judge the look by, so at the look's best.
        assert_eq!(Quality::for_device("Cpu"), Quality::High);
    }

    #[test]
    fn each_step_down_asks_no_more_of_the_gpu() {
        for pair in Quality::ALL.windows(2) {
            let (lower, higher) = (pair[0], pair[1]);
            let (a, b) = (lower.shadows(), higher.shadows());
            assert!(a.map <= b.map && a.cascades <= b.cascades && a.most_m <= b.most_m);
            assert!(lower.render_scale() <= higher.render_scale());
            assert!(lower.haze_steps().unwrap_or(0) <= higher.haze_steps().unwrap_or(0));
            assert!(lower.relief_layers() <= higher.relief_layers());
            assert!(lower.detail_reach() <= higher.detail_reach());
            assert!(lower.grass() <= higher.grass());
            assert!(!lower.occlusion() || higher.occlusion());
            assert!(!lower.bloom() || higher.bloom());
            assert!(!lower.soft_shadows() || higher.soft_shadows());
        }
    }

    #[test]
    fn a_preset_is_named_as_it_is_asked_for() {
        for quality in Quality::ALL {
            assert_eq!(Quality::from_name(quality.name()), Some(quality));
        }
        assert_eq!(Quality::from_name("ultra"), None);
    }
}
