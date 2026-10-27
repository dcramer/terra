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

/// The size of each block of GPU memory wgpu's allocator takes from the driver (D543); host
/// memory, what uploads are staged in, comes in blocks of half that.
pub const MEMORY_BLOCK: u64 = 64 * 1024 * 1024;

/// Bevy's plugins for a run, without winit when it opens no window.
pub fn engine(app: &mut App, windowless: bool) -> PluginGroupBuilder {
    // wgpu's allocator keeps GPU memory in blocks, each a `vkAllocateMemory` the driver clears
    // before handing over: 40 µs a MiB on RADV, so its default 256 MiB block held the render
    // world 10 ms in whichever frame overflowed the last one (a slab's growth, castle's worst
    // frame). Blocks of 64 MiB, a full mesh slab's page (D420), cost 2.5 ms each instead; VMA,
    // the allocator Godot's Vulkan driver uses, likewise begins with blocks an eighth of its
    // 256 MiB default (D543).
    let engine = DefaultPlugins.build().set(bevy::render::RenderPlugin {
        render_creation: bevy::render::settings::WgpuSettings {
            memory_hints: bevy::render::settings::MemoryHints::Manual {
                suballocated_device_memory_block_size: MEMORY_BLOCK..MEMORY_BLOCK,
            },
            ..default()
        }
        .into(),
        ..default()
    });
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
