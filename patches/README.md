# Patched crates

Bevy crates we carry a small change to, as D184 allows when extending Bevy can't do it: each
is the crates.io release we pin, copied whole (its licence files kept), with the change marked
`Terra's patch (Dnnn)` in its source, and taken in by the workspace's `[patch.crates-io]`.

| Crate | Release | Change | Upstream |
|---|---|---|---|
| `bevy_mesh` | 0.20.0 | `Mesh::write_packed_vertex_buffer_data` packs runs of 64 KiB of vertices in ordinary memory, then copies each into the slice in one write; `create_packed_vertex_buffer_data` packs straight into its `Vec` (D435) | [bevyengine/bevy#17000](https://github.com/bevyengine/bevy/issues/17000) |
| `bevy_pbr` | 0.20.0 | `irradiance_volume.wesl`'s `smaller_reflection_probe_holds` and the irradiance volume step of `pbr_functions.wesl`: a reflection probe smaller than every irradiance volume holding a fragment gives its diffuse light, not the volume; and `light_probe/mod.rs`'s `MAX_VIEW_LIGHT_PROBE_SLOTS`: a view holds 64 probes sharing the 8 texture slots (D485) | not yet offered |

## Carrying one to a new Bevy release

1. Look for the fix in the new release's source and changelog first; if it is there, drop the
   crate and its `[patch.crates-io]` line.
2. Otherwise copy the new release from `~/.cargo/registry/src/*/<crate>-<version>/` over the
   old one (leave out `.cargo_vcs_info.json`, `.cargo-ok`, `Cargo.lock` and `Cargo.toml.orig`),
   and make the change again, found by grepping for `Terra's patch`.
3. The tests that hold it (`crates/terra_world/src/mesh_upload_tests.rs` for `bevy_mesh`; the indoor light still, docs/dungeons.md, for `bevy_pbr`)
   must pass.
