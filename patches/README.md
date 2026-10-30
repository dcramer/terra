# Patched crates

Bevy crates we carry a small change to, as D184 allows when extending Bevy can't do it: each
is the crates.io release we pin, copied whole (its licence files kept), with the change marked
`Terra's patch (Dnnn)` in its source, and taken in by the workspace's `[patch.crates-io]`.

| Crate | Release | Change | Upstream |
|---|---|---|---|
| `bevy_mesh` | 0.20.0 | `Mesh::write_packed_vertex_buffer_data` packs runs of 64 KiB of vertices in ordinary memory, then copies each into the slice in one write; `create_packed_vertex_buffer_data` packs straight into its `Vec` (D435) | [bevyengine/bevy#17000](https://github.com/bevyengine/bevy/issues/17000) |
| `bevy_pbr` | 0.20.0 | `irradiance_volume.wesl`'s `smaller_reflection_probe_holds` and the irradiance volume step of `pbr_functions.wesl`: a reflection probe smaller than every irradiance volume holding a fragment gives its diffuse light, not the volume; and `light_probe/mod.rs`'s `MAX_VIEW_LIGHT_PROBE_SLOTS`: a view holds 64 probes sharing the 8 texture slots (D485); `light_probe/generate.rs`: a generated environment map whose diffuse map is not a storage texture is not convolved, the app filling it (D492); `volumetric_fog/volumetric_fog.wesl`: a fog volume absorbing over 100 a metre, a fire's glow, is marched in 16 steps with no directional light (D496); `material.rs`'s `check_entities_needing_specialization` and `mark_meshes_as_changed_if_their_materials_changed` gather their entities on one thread, not fanned out once a material type (D537); `lib.rs` re-exports the volumetric fog's pass as `volumetric_fog_pass`, so the fires' pass is ordered after it (D553); `atmosphere`'s `AtmosphereSettings::sun_gain` (default 1) scales what `sample_local_inscattering` scatters, so the sky-view and aerial tables and the air cube take the sun above the air while the lit ground keeps the held one (D212); `render/light.rs`'s `per_view_shadow_pass::<true>` builds no late indirect parameters and opens no encoder for a cascade without occlusion culling, which it never draws (D582) | not yet offered |
| `bevy_render` | 0.20.0 | `render_phase/mod.rs`'s `RenderMultidrawableBatchSetGpuBuffers::dirty`, set by every change to a batch set's bin buffers, and `batching/gpu_preprocessing.rs`'s `write_binned_instance_buffers`, which writes them only when it is set, not every frame (D583); `view/visibility/range.rs`'s `extract_visibility_ranges` keeps its table entity by entity, the frame's removed ranges out and its added and changed ones in, never clearing it and reading every ranged entity again (D587) | not yet offered |

## Carrying one to a new Bevy release

1. Look for the fix in the new release's source and changelog first; if it is there, drop the
   crate and its `[patch.crates-io]` line.
2. Otherwise copy the new release from `~/.cargo/registry/src/*/<crate>-<version>/` over the
   old one (leave out `.cargo_vcs_info.json`, `.cargo-ok`, `Cargo.lock` and `Cargo.toml.orig`),
   and make the change again, found by grepping for `Terra's patch`.
3. The tests that hold it (`crates/terra_world/src/mesh_upload_tests.rs` for `bevy_mesh`; the indoor light still, docs/dungeons.md, the sky light's stills, D492, and `fire::fog`'s glow-mark test and the fire sheet, D496, and D537's bench, and `daylight`'s air-sun test, D212, for `bevy_pbr`)
   must pass. `bevy_render`'s are its own `render_phase::tests::render_multidrawable_batch_set`
   and `view::visibility::range::tests` (D587);
   a patched crate is not a workspace member, so run it from a copy whose `Cargo.toml` gains
   an empty `[workspace]` table: `cargo test --manifest-path <copy>/Cargo.toml --lib -- render_phase::tests view::visibility::range`.
