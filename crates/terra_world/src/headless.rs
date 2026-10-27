//! An unattended run drawing into a picture opens no window (D417).
//!
//! The window entity stays, for what reads it (the cursor, the screen's size), but without
//! winit no window is made of it, so nothing is drawn to it and Bevy's own loop drives the
//! frames, as in its `headless_renderer` example. Wayland cannot hide a window, and a hidden
//! one still has to answer the desktop: a scripted frame waits seconds for building
//! (`layout::collect`), and Hyprland showed every such run as not responding.

use bevy::{
    app::{PluginGroupBuilder, ScheduleRunnerPlugin},
    prelude::*,
    winit::WinitPlugin,
};
use bevy_inspector_egui::bevy_egui::EguiGlobalSettings;

/// Bevy's plugins for a run, without winit when it opens no window.
pub fn engine(app: &mut App, windowless: bool) -> PluginGroupBuilder {
    let engine = DefaultPlugins.build();
    // Traced, the render world runs after the game on the main thread, not beside it, so a
    // worker's span is plainly one world's (tools/sandbox/spans.ts).
    #[cfg(feature = "trace")]
    let engine = engine.disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>();
    if !windowless {
        return engine;
    }
    app.add_plugins(ScheduleRunnerPlugin::default())
        // Egui's on-screen keyboard is the window's, and asks for it every frame.
        .insert_resource(EguiGlobalSettings {
            enable_ime: false,
            ..default()
        });
    engine.disable::<WinitPlugin>()
}
