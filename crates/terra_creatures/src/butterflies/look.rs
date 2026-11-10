//! How a butterfly is drawn: its model's own textures (D14), its wings beaten about its
//! body in the vertex shader (`assets/shaders/butterfly.wesl`), so a group is so many
//! instances of one mesh in one material, with no skeleton and nothing animated here.

use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use terra_survival::props::{textured_material, textured_mesh};
use terra_world::models::read_model;

/// The beating a butterfly's material adds. Field order must match `Wings` in
/// butterfly.wesl.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct Wings {
    /// How many times a second the wings beat, how many bouts of beating a second, and the
    /// share of a bout that is beating, the rest a glide.
    #[uniform(100)]
    pub beat: Vec4,
    /// The body's axis, which the wings turn about ([`Axis`]): across, up, and how far out
    /// a wing is hinged.
    #[uniform(100)]
    pub hinge: Vec4,
}

/// A wing is hinged this far from the body's axis, as a share of the span: a body is about
/// a tenth of the span across.
const HINGE: f32 = 0.05;
/// How rough a wing is drawn: a guess at a matte, powdery surface, not measured.
const ROUGHNESS: f32 = 0.85;

/// The line along a model's body its wings turn about, in the model's own units: where it
/// is across and up, how far out from it a wing is hinged, and the span.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Axis {
    pub across: f32,
    pub up: f32,
    pub hinge: f32,
    pub span: f32,
}

/// A model's axis: down the middle of its span, at the height of its wings' roots (the
/// points just past the hinge; a model stands on its base, so that is not its origin).
pub fn axis(points: &[Vec3]) -> Option<Axis> {
    let (left, right) = points
        .iter()
        .fold((f32::MAX, f32::MIN), |(left, right), at| {
            (left.min(at.x), right.max(at.x))
        });
    let span = right - left;
    if points.is_empty() || span <= 0.0 {
        return None;
    }
    let (across, hinge) = ((left + right) * 0.5, span * HINGE);
    let roots = |within: f32| {
        let (sum, count) = points
            .iter()
            .filter(|at| (hinge..=within).contains(&(at.x - across).abs()))
            .fold((0.0, 0), |(sum, count), at| (sum + at.y, count + 1));
        (count > 0).then(|| sum / count as f32)
    };
    Some(Axis {
        across,
        up: roots(hinge * 3.0).or_else(|| roots(span))?,
        hinge,
        span,
    })
}

impl MaterialExtension for Wings {
    fn vertex_shader() -> ShaderRef {
        "shaders/butterfly.wesl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/butterfly.wesl".into()
    }
}

pub type ButterflyMaterial = ExtendedMaterial<StandardMaterial, Wings>;

/// A kind's look: its mesh, its material, and how wide the model is as made, which a
/// butterfly of another span is scaled from.
pub struct Look {
    pub mesh: Handle<Mesh>,
    pub material: Handle<ButterflyMaterial>,
    pub span_m: f32,
}

/// The look of the model in `assets/models/<file>`, beating so many times a second in
/// bouts so long with a glide so long between: its first textured part, as `tools/props`
/// makes it, wings spread flat in the level, the body along z with the head to +Z (D303)
/// and the span along x.
pub fn look(
    file: &str,
    beat_hz: f32,
    bout_s: (f32, f32),
    assets: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ButterflyMaterial>,
) -> Result<Look, String> {
    let parts = read_model(file)?;
    let (part, textures) = parts
        .iter()
        .find_map(|part| Some((part, part.textures.as_ref()?)))
        .ok_or_else(|| format!("{file} has no textured part: make it through tools/props."))?;
    let axis = axis(&part.positions).ok_or_else(|| format!("{file} has no width along its x."))?;
    let mut base = textured_material(&format!("models/{file}"), textures, None, assets);
    // A wing is a sheet, seen from over and under.
    base.double_sided = true;
    base.cull_mode = None;
    // A wing's scales are matte. The models' baked roughness is not used: it differs by
    // breed (a mean of 0.09 for the peacock, which speckles white lit straight on, to 0.65;
    // butterfly-models j1) where the wings do not.
    base.metallic_roughness_texture = None;
    base.perceptual_roughness = ROUGHNESS;
    base.metallic = 0.0;
    Ok(Look {
        mesh: meshes.add(textured_mesh(part, textures)),
        material: materials.add(ButterflyMaterial {
            base,
            extension: Wings {
                beat: Vec4::new(
                    beat_hz,
                    1.0 / (bout_s.0 + bout_s.1).max(0.01),
                    super::flight::beats(bout_s),
                    0.0,
                ),
                hinge: Vec4::new(axis.across, axis.up, axis.hinge, 0.0),
            },
        }),
        span_m: axis.span,
    })
}
