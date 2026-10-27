//! The smoke over a fire, as it is lit (docs/fire.md, "The smoke"; D415): each card a baked puff
//! whose six-way lightmaps (`tools/fire/bake/smoke.ts`) say how much of the light from each side
//! it gives back, so `fire_smoke.wesl` lights it as a volume is lit, from the sun, the fires'
//! own lights and the open sky round it, instead of as a surface facing the eye. The sky's
//! light is the one the shading reads: skylight.rs's [`OpenSky`] times the environment's
//! brightness, in its turned frame.

use crate::skylight::OpenSky;
use bevy::{
    light::EnvironmentMapLight,
    pbr::MaterialExtension,
    prelude::*,
    render::{render_resource::AsBindGroup, storage::ShaderBuffer},
    shader::ShaderRef,
};

/// What the smoke over a fire is told. Field order must match `fire_smoke.wesl`'s `Smoke`.
#[derive(Clone, Copy, Debug, Default, Reflect, bevy::render::render_resource::ShaderType)]
pub struct FireSmokeUniform {
    /// The wind, and how many slots are filled.
    pub wind: Vec4,
    /// x which smoke (0 over the blazes, 1 a breath's soot), y the time the puffs were let
    /// go by, w how many of the first slots are kept fires.
    pub kind: Vec4,
    /// The open sky's light from +X, -X, +Y, -Y, +Z and -Z of the sky's frame, as the
    /// environment map's diffuse light is (irradiance over pi), times its brightness.
    pub sky: [Vec4; 6],
    /// The sky frame's axes in the world.
    pub sky_frame: [Vec4; 3],
    /// Each smoke set's light scale, in books.rs's `SMOKE_SETS` order.
    pub sets: Vec4,
}

/// The smoke over a fire, or a breath's soot.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct FireSmokeExtension {
    #[uniform(100)]
    pub uniform: FireSmokeUniform,
    #[storage(101, read_only)]
    pub slots: Handle<ShaderBuffer>,
    #[storage(102, read_only)]
    pub puffs: Handle<ShaderBuffer>,
    /// The puffs' six-way map 0 (right, top, back, alpha), a layer a set.
    #[texture(103, dimension = "2d_array")]
    #[sampler(104)]
    pub near: Handle<Image>,
    /// Their map 1 (left, bottom, front, glow).
    #[texture(105, dimension = "2d_array")]
    pub far: Handle<Image>,
}

impl MaterialExtension for FireSmokeExtension {
    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        if super::matte::on() {
            super::matte::keep_smoke_out(descriptor);
        }
        Ok(())
    }

    fn vertex_shader() -> ShaderRef {
        "shaders/fire_smoke.wesl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/fire_smoke.wesl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/fire_smoke.wesl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/fire_smoke.wesl".into()
    }
}

/// The open sky's six sides as the smoke reads them: what comes from above and below the
/// horizon toward each, times the environment's brightness.
pub fn sky_sides(open: &OpenSky, brightness: f32) -> [Vec4; 6] {
    std::array::from_fn(|i| ((open.sky[i] + open.ground[i]) * brightness).extend(0.0))
}

/// The sky frame's axes in the world, for an environment turned by `rotation`.
pub fn sky_axes(rotation: Quat) -> [Vec4; 3] {
    [Vec3::X, Vec3::Y, Vec3::Z].map(|axis| (rotation * axis).extend(0.0))
}

/// The open sky as the smoke materials are to read it this frame: from the camera's
/// environment light, or none before there is one, dimmed to the share of it that reaches
/// the air round the camera (`air_lit`, `IndoorSky`, D485). Undimmed, a dungeon's fire bowl
/// at noon sent up two grey searchlight cones, its wisp lit by the whole sky (round 8).
pub fn sky(
    open: Option<&OpenSky>,
    environment: Option<&EnvironmentMapLight>,
    air_lit: f32,
) -> ([Vec4; 6], [Vec4; 3]) {
    match (open, environment) {
        (Some(open), Some(environment)) => (
            sky_sides(open, environment.intensity * air_lit.clamp(0.0, 1.0)),
            sky_axes(environment.rotation),
        ),
        _ => ([Vec4::ZERO; 6], sky_axes(Quat::IDENTITY)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shaders_struct_follows_the_uniforms_fields_in_order() {
        let shader =
            std::fs::read_to_string(crate::data::assets().join("shaders/fire_smoke.wesl")).unwrap();
        let start = shader.find("struct Smoke {").unwrap();
        let body = &shader[start..start + shader[start..].find('}').unwrap()];
        let fields: Vec<&str> = body
            .lines()
            .skip(1)
            .filter_map(|line| line.trim().split(':').next())
            .filter(|name| !name.is_empty())
            .collect();
        assert_eq!(fields, ["wind", "kind", "sky", "sky_frame", "sets"]);
        assert!(shader.contains("sky: array<vec4<f32>, 6>,"));
        assert!(shader.contains("sky_frame: array<vec4<f32>, 3>,"));
        for binding in ["@binding(103)", "@binding(104)", "@binding(105)"] {
            assert!(shader.contains(binding), "{binding}");
        }
    }

    #[test]
    fn a_turned_sky_frame_carries_its_up_onto_the_worlds() {
        let up = Vec3::new(0.3, 0.9, -0.2).normalize();
        let axes = sky_axes(Quat::from_rotation_arc(Vec3::Y, up));
        assert!(axes[1].truncate().distance(up) < 1e-5);
        assert!(axes[0].truncate().dot(axes[2].truncate()).abs() < 1e-5);
    }

    #[test]
    fn the_sky_from_each_side_is_its_sky_and_ground_together() {
        let mut open = OpenSky::default();
        open.sky[2] = Vec3::new(0.2, 0.3, 0.5);
        open.ground[2] = Vec3::new(0.1, 0.1, 0.1);
        let sides = sky_sides(&open, 2.0);
        assert!(sides[2].truncate().distance(Vec3::new(0.6, 0.8, 1.2)) < 1e-6);
        assert_eq!(sides[3], Vec4::ZERO);
    }

    #[test]
    fn smoke_in_a_room_shut_off_from_the_sky_takes_none_of_its_light() {
        let mut open = OpenSky::default();
        open.sky[2] = Vec3::splat(0.4);
        let environment = EnvironmentMapLight {
            intensity: 1800.0,
            ..default()
        };
        let lit = |air_lit| sky(Some(&open), Some(&environment), air_lit).0[2].x;
        assert!(lit(1.0) > 0.0);
        // A room letting in a tenth of the sky lights its smoke a tenth as much, a closed one not.
        assert!((lit(0.1) - 0.1 * lit(1.0)).abs() < 1e-3);
        assert_eq!(lit(0.0), 0.0);
    }
}
