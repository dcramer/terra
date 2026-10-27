//! The camera the debug interface is drawn with (egui: the debug menu, D81, and the F1
//! inspector), and the screen's interface with it: a camera of its own, drawn last, over the
//! final picture at the screen's own resolution, as engines draw debug UI over the finished
//! frame (Dear ImGui over the swapchain, Unreal's Slate over the scene viewport). On the main
//! camera egui was drawn into the world's picture, at its render scale when that is scaled
//! (`upscale`), and stretched with it: soft, and not where its points said.
//!
//! bevy_egui's own primary context goes on the first camera made, the main one; it is turned
//! off and the context put here (`PrimaryEguiContext`), as bevy_egui's multi-camera setup
//! does. An offscreen run points this camera at its picture as it does every camera
//! (`shell::picture`).

use bevy::{
    camera::{CameraOutputMode, visibility::RenderLayers},
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    prelude::*,
    render::render_resource::BlendState,
};
use bevy_inspector_egui::bevy_egui::{EguiGlobalSettings, PrimaryEguiContext};

/// After every other camera drawing to the screen: the world's (0) and its upscaled
/// picture's (1).
pub const ORDER: isize = 100;

/// Added after `EguiPlugin`, before any camera is made.
pub fn plugin(app: &mut App) {
    if let Some(mut settings) = app.world_mut().get_resource_mut::<EguiGlobalSettings>() {
        settings.auto_create_primary_context = false;
    }
    app.add_systems(Startup, spawn);
}

/// The camera drawn last: it starts clear, draws the interface and egui over nothing and is
/// laid over what the cameras before it drew, keeping it (premultiplied, as egui draws).
fn spawn(mut commands: Commands) {
    commands.spawn((
        Name::new("Interface camera"),
        Camera2d,
        Camera {
            order: ORDER,
            clear_color: ClearColorConfig::Custom(Color::NONE),
            output_mode: CameraOutputMode::Write {
                blend_state: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                clear_color: ClearColorConfig::None,
            },
            ..default()
        },
        // Nothing of the world is drawn with it, nor smoothed, nor tone mapped.
        RenderLayers::none(),
        Msaa::Off,
        Tonemapping::None,
        DebandDither::Disabled,
        PrimaryEguiContext,
    ));
}
