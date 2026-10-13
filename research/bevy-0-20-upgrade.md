# Moving to Bevy 0.20

2026-10-12. Question: what does moving from Bevy 0.19.1 to 0.20.0 take, and does the game look, skin and run the same afterwards?

## Answer

Little code, mostly shaders. Bevy 0.20 writes its shaders in WESL and drops the naga_oil dialect, so our seven shaders were ported and renamed `.wesl`. Our weight-blended skinning still replaces Bevy's by the same means, under the new path. Everything rendered under WSL matches 0.19 pixel for pixel, but for blended materials in shade, which 0.20 no longer darkens with ambient occlusion twice, and frame times are within run-to-run noise of 0.19 on both GPUs, though `world` and `forest` came out about 9% slower on the fast one. The one forced code change is the game's buttons, since `Interaction` is deprecated. The inspector has no release for 0.20 and runs on the commit of its open port (D31).

## What changed

- **Shaders** (read in `bevy_pbr-0.20.0`, `bevy_shader-0.20.0` and the shipped migration guide, `_release-content/migration-guides/shaders_use_wesl.md`): `#import a::b` became `import a::b;`, before any declaration; `#ifdef X` became `@if(X)` on a declaration, struct member, import or statement; `#{MATERIAL_BIND_GROUP}` became `constants::MATERIAL_BIND_GROUP`; module paths follow the file, so `bevy_pbr::forward_io` is `bevy_pbr::render::forward_io` and `bevy_pbr::prepass_io` is `bevy_pbr::prepass::io`. A `.wgsl` file is now taken as plain WGSL, without imports, so ours had to become `.wesl`. `alpha_discard` takes the flags and cutoff instead of the material. Vertex shaders now read their input through `decompress_vertex`, for meshes whose attributes are compressed; ours (`body_mask`, `wind`) follow Bevy's `render/mesh.wesl` and `prepass/prepass.wesl` as they are in 0.20.
- **Skinning.** Bevy's skinning shader is `embedded://bevy_pbr/render/skinning.wesl`, and `Shader::from_wesl` gives a shader loaded from an embedded path the module name of its crate and file, `bevy_pbr::render::skinning`. The shader cache still keys modules by asset id and recompiles dependents when one is replaced (`ShaderCache::set_shader`), so putting our source in `Assets<Shader>` at the id of Bevy's, as on 0.19, makes every mesh shader that imports it skin our way. Our file declares the same bindings and functions behind `@if(SKINNED)`, as Bevy's does. No patch of `bevy_pbr` was needed.
- **Buttons.** `Button` and `Interaction` are deprecated, and the lints refuse deprecated items. The game's buttons are now Bevy's headless `ui_widgets::Button` with `Hovered`: a click is `Added<Pressed>` on a frame the left mouse button went down, since Bevy's button is pressed by any mouse button and right-clicks mean other things here (`play/button.rs`). Hovering comes from picking, which, unlike the old focus system, lets a visible node with no `Pickable` block what is under it.
- **Dependencies.** `glam` 0.33 with Bevy; `bevy_egui` 0.43; `bevy-inspector-egui` from jakobhellermann/bevy-inspector-egui#324 at `9a5b086`, open and marked ready for review on 2026-10-11. Avian, a Leaning pick (D33) and not a dependency, has no 0.20 release yet.

## What was checked

Renders under WSL's CPU renderer, taken on 0.19 before the upgrade and on 0.20 after, compared with ImageMagick (`compare -metric AE -fuzz 2%`, pixels that differ):

| Scene | Pixels that differ |
|---|---|
| Default (ground shader) | 0 |
| `look` (sky, ground, rocks) | 0 |
| `forest` | 0 |
| Workbench, base body, Sword_Attack, forearm close-up, our skinning | 0 |
| The same, `--linear-skinning` | 0 |
| Workbench in `leather_coat`, three sides (body mask, water behind) | 0 |
| `play` (sky, ground, wind on grass, characters) | 10,072 of 1.44 million, scattered along blade and branch edges (RMSE 0.5%) |
| `play` with `lush` on the yard, 13:00 (the foliage shader) | 102,634, mostly the shaded part of the path (RMSE 1.2%) |

The lush forest's path is lighter in shade on 0.20, by about 5 in 255. The path is a blended material, and 0.20 only applies screen-space ambient occlusion to materials that write depth (`bevy_pbr-0.20.0/src/render/mesh.rs`, `SCREEN_SPACE_AMBIENT_OCCLUSION` pushed only with `depth_write_enabled`); 0.19 applied it to blended ones too, reading the occlusion of the ground beneath, so they were darkened twice. Bevy's fix, not ours to undo; the sea, also blended, will be a little lighter where the occlusion was dark.

Our skinning and the linear blend differ in 70,442 pixels of the forearm sheet on both versions, so the swap is in effect on 0.20; the run logs "skinning: weight-blended, in place of Bevy's linear blend" and "skinning: 25721 of 27511 vertices of a mesh mostly dual quaternion". Rebased onto the new base bodies in plain underwear (`ad0f26e`), the forearm, linear-blend and coat sheets again match 0.19 pixel for pixel; the two skinnings now differ in 65,854 pixels, and the body logs 24363 of 26919 vertices mostly dual quaternion. The buttons were checked live over the remote (`point`, `click`, added for this): hovering outlines a backpack item and describes it; a click takes off and wears a coat and eats berries; a right-click on an item does nothing and on a control clears it; a build-menu tab and tile pick. The station screen uses the same pieces and was not clicked.

Frame times with `cargo xtask bench` (D185: release, offscreen 1920x1080, the Windows build), 0.19 (`3b26412`) and 0.20 run in turn, three rounds, so a GPU busy with something else shows in both. Each cell is the scene's median of three runs, for each round:

| Scene, RTX 5090 | 0.19 | 0.20 | Mean of rounds, 0.19 → 0.20 |
|---|---|---|---|
| `yard` | 4.80, 5.09, 4.58 ms | 4.88, 3.80, 5.38 ms | 4.82 → 4.69 ms |
| `world` | 5.62, 5.95, 5.78 ms | 6.55, 5.94, 6.47 ms | 5.78 → 6.32 ms (+9%) |
| `forest` | 5.66, 5.18, 4.67 ms | 5.82, 5.20, 5.92 ms | 5.17 → 5.65 ms (+9%) |
| `forest-walk` | 6.05, 4.99, 6.00 ms | 5.19, 5.20, 5.68 ms | 5.68 → 5.36 ms (−6%) |

| Scene, integrated Radeon (`WGPU_POWER_PREF=low`), one run each | 0.19 | 0.20 |
|---|---|---|
| `world` | 141.6, 154.5, 148.0 ms | 150.1, 148.2 ms |

Earlier single runs of the same scenes ranged from 4.95 to 7.55 ms for `world` on 0.20 alone, so the spread between runs is as wide as the differences. In the debug build, `--experiment play --bench` was 7.59 and 6.60 ms on 0.19 and 6.98 and 6.83 ms on 0.20 on the RTX 5090, 168 and 172 ms on the Radeon; `--experiment crowd --crowd 1000` was 13.69 ms on 0.20 against 13.3 ms recorded on 0.19 ([characters.md](../docs/characters.md), phase 1).

## Limits

- The 10,072 differing pixels in `play` were not traced; they look like grass and branches a hair apart, not a change in any shader's look.
- `world` and `forest` came out about 9% slower on 0.20 on the RTX 5090, `world` in all three rounds, but inside the spread between runs; nothing was traced. Worth rerunning once a baseline for this GPU is recorded (`--record`). The Radeon shows no difference.
- The inspector (F1, F2) was built but not opened.
- Only Linux under WSL and Windows were run; macOS was not.

## Sources

- [Bevy 0.20 release notes](https://bevy.org/news/bevy-0-20/), [migration guide](https://bevy.org/learn/migration-guides/0-19-to-0-20/)
- [bevy-inspector-egui#324](https://github.com/jakobhellermann/bevy-inspector-egui/pull/324)
- Bevy's 0.20 example shaders, such as [extended_material.wesl](https://github.com/bevyengine/bevy/blob/v0.20.0/assets/shaders/extended_material.wesl), and the button example, [button.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/examples/ui/widgets/button.rs)
