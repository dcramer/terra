//! The fire lights' places and flickers handed to the render world, for what burns to glow
//! with (`FireGlows`), split from `light` for its size (D396).

use super::GLOWS;
use bevy::{
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::RenderAssets,
        renderer::RenderQueue,
        storage::{GpuShaderBuffer, ShaderBuffer},
    },
};

/// The fire lights' places and flickers (`GLOWS` of them: xyz where, w its glow, 0 for
/// none), for what burns to glow with: the char's cracks on a burning building flare as the
/// fire nearest them does. A buffer of the render world, written each frame.
#[derive(Resource, Clone, Debug)]
pub struct FireGlows {
    pub buffer: Handle<ShaderBuffer>,
    pub glows: [[f32; 4]; GLOWS],
}

impl ExtractResource<RenderApp> for FireGlows {
    type Source = FireGlows;

    fn extract_resource(source: &FireGlows) -> Self {
        source.clone()
    }
}

pub(in crate::fire) fn plugin(app: &mut App) {
    let buffer = app
        .world_mut()
        .resource_mut::<Assets<ShaderBuffer>>()
        .add(ShaderBuffer::from(vec![[0.0f32; 4]; GLOWS]));
    app.insert_resource(FireGlows {
        buffer,
        glows: [[0.0; 4]; GLOWS],
    });
    if app.get_sub_app(RenderApp).is_none() {
        return;
    }
    app.add_plugins(ExtractResourcePlugin::<FireGlows>::default());
    app.sub_app_mut(RenderApp)
        .add_systems(Render, write_glows.in_set(RenderSystems::PrepareResources));
}

fn write_glows(
    glows: Option<Res<FireGlows>>,
    buffers: Res<RenderAssets<GpuShaderBuffer>>,
    queue: Res<RenderQueue>,
) {
    let Some(glows) = glows else {
        return;
    };
    if let Some(target) = buffers.get(&glows.buffer) {
        queue.write_buffer(
            &target.buffer,
            0,
            &glows
                .glows
                .iter()
                .flatten()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<u8>>(),
        );
    }
}
