//! The ground telegraph of a mob's blow (iso-forest-demo j7): the area its swing judges
//! (play.rs `in_arc`, `REACH` and `ARC` from the mob's feet) drawn on the ground as the
//! action RPGs warn of a monster's attack: Diablo IV's and Lost Ark's red cone whose fill
//! grows out from the monster to its bright rim as the blow nears, gone when it lands.
//! j6's tell was a ForwardDecal splat whose emission (4 nits under the forest's ev100 7.5,
//! 0.02 on screen) never showed and whose flecks lay under the two bodies and the grass.
//! This one is unlit (its colour reaches the screen as it is, a bright core over bloom).
//! j7 drew it over everything (depth test Always) so the ground cover would not hide it; it
//! then hid the hero wholly in a wind-up (j13, frame 1030). As Diablo IV draws its cones, it
//! now lies under the bodies and over the ground: depth tested, and moved `RAISE` toward the
//! camera (an orthographic camera: the same pixels, nearer), so the grass and the ground
//! cover below his knee stay under it and a body standing on it stays over it (`OnTop`).

use crate::play::{ARC, REACH};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;

pub type TellMaterial = ExtendedMaterial<StandardMaterial, OnTop>;

/// A standard material writing no depth, drawn over whatever stands in front of it when
/// `over_bodies` (j7's tell, which hid the hero), else depth tested as any surface. A field,
/// so the occlusion check can read what the pipeline draws (not yet: occlude.rs, j14).
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
#[bind_group_data(OnTopKey)]
pub struct OnTop {
    pub over_bodies: bool,
}

/// What `OnTop`'s pipeline is specialised by.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OnTopKey {
    over_bodies: bool,
}

impl From<&OnTop> for OnTopKey {
    fn from(m: &OnTop) -> Self {
        OnTopKey {
            over_bodies: m.over_bodies,
        }
    }
}

impl MaterialExtension for OnTop {
    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            if key.bind_group_data.over_bodies {
                depth.depth_compare = Some(bevy::render::render_resource::CompareFunction::Always);
            }
            depth.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}

/// How far toward the camera a tell is drawn from the ground (m): ground cover lower than
/// about `RAISE` times the sine of the camera's pitch stays under it, and the hero's body
/// from the knee up (occlude.rs `body`, 0.5 m) stays over it.
pub const RAISE: f32 = 0.45;

/// Where the fan starts out from the mob's feet (a share of `REACH`), so its own legs stay
/// clear of it; the rim's width (a share).
const INNER: f32 = 0.15;
const RIM: f32 = 0.06;
const SEGMENTS: usize = 24;
/// The fill's and the rim's colours as they reach the screen (unlit, before AgX): an
/// ember red, the rim bright enough to bloom.
pub const FILL: LinearRgba = LinearRgba::rgb(1.2, 0.12, 0.03);
pub const EDGE: LinearRgba = LinearRgba::rgb(5.0, 0.9, 0.2);

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<TellMaterial>::default());
}

/// An annular sector in the ground's plane about the origin, facing +Z, from `inner` to
/// `outer` metres over `ARC` either side, its vertices' alpha `a_in` at `inner` and `a_out`
/// at `outer` (vertex colours, which the standard material multiplies its own by).
pub fn sector(inner: f32, outer: f32, a_in: f32, a_out: f32) -> Mesh {
    let half = ARC.to_radians();
    let mut at = Vec::with_capacity(2 * (SEGMENTS + 1));
    let mut colour = Vec::with_capacity(2 * (SEGMENTS + 1));
    for i in 0..=SEGMENTS {
        let a = -half + 2.0 * half * i as f32 / SEGMENTS as f32;
        let way = Vec3::new(libm::sinf(a), 0.0, libm::cosf(a));
        at.push((way * inner).to_array());
        at.push((way * outer).to_array());
        colour.push([1.0, 1.0, 1.0, a_in]);
        colour.push([1.0, 1.0, 1.0, a_out]);
    }
    let mut index = Vec::with_capacity(6 * SEGMENTS);
    for i in 0..SEGMENTS as u32 {
        let (a, b, c, d) = (2 * i, 2 * i + 1, 2 * i + 2, 2 * i + 3);
        index.extend_from_slice(&[a, c, b, b, c, d]);
    }
    let n = at.len();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, at)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; n])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; n])
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colour)
    .with_inserted_indices(Indices::U32(index))
}

/// The fill, the whole area a blow judges, faint at the mob and strong at the rim; it
/// grows out to its full as the hit nears (its scale).
pub fn fill() -> Mesh {
    sector(INNER * REACH, REACH, 0.25, 1.0)
}

/// The rim, the reach's edge, whole from the first moment of the tell.
pub fn edge() -> Mesh {
    sector((1.0 - RIM) * REACH, REACH, 1.0, 1.0)
}

/// A tell's material: unlit, unfogged, blended (the fill) or added (the rim).
pub fn material(colour: LinearRgba, alpha: AlphaMode) -> TellMaterial {
    TellMaterial {
        base: StandardMaterial {
            base_color: colour.into(),
            unlit: true,
            fog_enabled: false,
            alpha_mode: alpha,
            cull_mode: None,
            ..default()
        },
        extension: OnTop { over_bodies: false },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::in_arc;

    fn points(mesh: &Mesh) -> Vec<Vec2> {
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("a tell has positions");
        };
        p.iter().map(|v| Vec2::new(v[0], v[2])).collect()
    }

    /// What the tell shows is what the blow judges: every corner of the fill and the rim is
    /// within the swing's arc from the mob facing +Z (in_arc, a little inside for its float),
    /// and the rim's outer corners stand at its full reach, out to its arc's either edge.
    #[test]
    fn the_tell_covers_the_blows_arc() {
        let face = Vec2::Y;
        for mesh in [fill(), edge()] {
            for p in points(&mesh) {
                assert!(
                    in_arc(Vec2::ZERO, face, p * 0.999),
                    "{p} lies outside the blow"
                );
            }
        }
        let rim = points(&edge());
        let far = rim.iter().map(|p| p.length()).fold(0.0, f32::max);
        assert!(
            (far - REACH).abs() < 1e-3,
            "the rim stands at {far}, not {REACH}"
        );
        let widest = rim
            .iter()
            .map(|p| libm::atan2f(p.x, p.y).abs().to_degrees())
            .fold(0.0, f32::max);
        assert!(
            (widest - ARC).abs() < 0.1,
            "the rim spans {widest}°, not {ARC}°"
        );
    }
}
