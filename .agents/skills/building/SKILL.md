---
name: building
description: Work on Terra's construction (terra_build and the game's building): walls, floors, roofs, openings, decks and stairs of cubes, their materials, kits and dressed pieces, how they are drawn, damaged and taken down, and the checks that hold them. Covers the scope (D333), the agents' routine, which checks a change of geometry, look, performance or material must pass, where each concern lives in the code and data, how to measure, and what to report. Use for any task that changes or judges building, its kits or its checks, so a brief needs to say only the task.
---

# Building

Read [docs/building-voxels.md](../../../docs/building-voxels.md) (the voxel contract, D348) before changing how cubes are laid or drawn, and [docs/building.md](../../../docs/building.md) for the rest. This page is the routine around the work; the docs are the rules.

## Scope (D333)

The baseline is **wood and stone, built by hand and destroyed, with infinite pretend resources**: logs, hewn timber and shakes; stone and slate. It must be flawless and fast before anything else. Earth, limewash, wattle and thatch are frozen (kept building and passing checks, not polished). No costs, storage, carrying or recruits, and no new materials, props, forests or logistics unless asked. A design question (how something should look or play) goes to the developers as a recommendation; a technical one is yours, recorded in `docs/decisions/`.

## Routine (D375)

AGENTS.md's routine, plus for building:

- At the start, check the latest commits of anyone working in the same files.
- The hooks only format and lint (D37); the scheduled full check on main runs the rest. For a change to geometry, drawing or kits, run `cargo test -p terra_build -p terra_build_mesh -p terra_build_scenes`, `cargo xtask kits` and `cargo xtask artifacts --quick --ratchet` yourself before landing.

## Where each concern lives

The rules are `crates/terra_build/src`; the layout (with walking and how props hang) `crates/terra_build_layout/src`; the drawing and its checks `crates/terra_build_mesh/src`; the fixtures `crates/terra_build_scenes/src` (D407).

| Concern | Where |
|---|---|
| Cubes, shapes, kinds, fill | `crates/terra_build/src/cubes.rs`; who owns each face and edge: the contract's §2 |
| Patterns (block, coursed, frame, roof) | `assets/data/kits/patterns/*.ron`; their code is the layout below |
| Materials and packs | `assets/data/kits/*.ron` over `assets/data/packs/*.ron`, textures in `assets/materials/`; read by `kit.rs`, checked by the drawing's `packs.rs`; made by `cargo xtask pack` |
| Roles (arris, crest, foot, quoin, jamb…) | `layout/roles.rs` |
| Layout of pieces | `layout/`: `runs.rs` (coursed walls, under roofs), `blocks.rs` (stone openings, lintels), `trims.rs` (frames, end boards), `floors.rs` (floors, decks), `stairs.rs`, `shapes.rs`, `frames.rs`, `roofs/` (squares, lines, flashing, standing on roofs) |
| Block surface (drawing) | `surface.rs`, `surface/slots.rs` |
| Timber members | `timber/` (cuts, planks, sweeps, harm) |
| Roof covering | `roofing.rs` |
| Dressed pieces | `dress/`, `modules.rs` |
| Sections, detail levels, ghosts | `sections.rs`, `sections/` (far, ghosts, laying, blocks) |
| Player's tools as operations | `plans.rs` → `ops.rs`; loss and collapse in `loss.rs` |
| Scenes | `terra_build_scenes`: the registry (`registry.rs`) holds every scene the game ships or a check looks at; sheet, QA cabin, castle, burg, fuzz |
| The contract | `docs/building-voxels.md`, its matrix data `docs/building-voxels.json` |
| The mesh checker | the drawing's `inspect.rs`; its families the rules' `junctions.rs` and the fixtures' `roof_walls.rs` |

## Which checks, for which change

Every change: the tests of what it touches, run yourself; the hooks format and lint, and there is no CI while we are pre-release (D37).

- **Geometry** (layout, roles, a shape, a junction): the module's tests; `cargo xtask artifacts <name>` for the scenes it touches, then `--quick --ratchet`; `cargo xtask fuzz` (seeds 0 to 200; `--seeds N` for more). A new scene goes in `scenes::registry` and nowhere else: the checks find it. A junction closed or opened changes `docs/building-voxels.json` (its rule, code, tests, registered cases) and `node tools/docs/docs.ts --render`; a fault found and not fixed is a numbered gap with its rule. Fixed faults: leave the baseline (`crates/terra_build_scenes/examples/artifacts-baseline/`), a fall passes and the full run on main (`cargo xtask nightly`) lowers it; the check refuses a lowering (D414). A rise that is meant: `--raise "<why>"` on the same command (`cargo xtask artifacts --quick`, `cargo xtask artifacts`, `cargo xtask fuzz`), which writes only what rose, with the why.
- **Look** (a texture, relief, a piece's model): `cargo xtask material <material>` first (D386: fixed vantages beside its reference and a grey card, its numbers against the pattern's targets, failing on a regression), `cargo xtask kits`, `cargo xtask texels <name>`, the kit sheet (`cargo xtask yard --kit-sheet --sheet-rows "<row>"`, then `--vantage` as it logs and `--shot`), a material's preview (`node tools/kits/preview.ts <name>`), and `cargo xtask artifacts --frames` on the GPU. Judge against [docs/art-style.md](../../../docs/art-style.md) and the material's reference.
- **Performance**: `cargo xtask sections` (laying and merging against the 2 ms bar), `cargo xtask bench village going-up rising town300` (D185), `cargo xtask qa clips` traces; both load-aware (`--runs 3`).
- **Material** (a new or changed file): `cargo xtask pack`, `cargo xtask kits`, `cargo xtask material <material>`, and `cargo xtask artifacts <name>` for it, as a material outside the settlement is in no ratchet.
- **Fire** (the building fire, char, burning, `terra_world::fire`, its shaders, textures or presets, or a tonemapping patch): the fire sheet's gate runs on main continuously (`cargo xtask continuous`, D415), not before landing; read its status (`.git/terra-continuous/status.txt`) after landing, and a change that brings a fire's numbers in rewrites the held baseline with `cargo xtask fire --hold` and commits it.
- **Rules** (support, refusals, collapse): `cargo xtask rules` (tests with known failures, random operations, the drawing audit, live rules).

## Measuring

- Benchmarks on the real GPU (under WSL, the Windows build), release, offscreen 1920x1080; report the integrated GPU too. Software rendering numbers mean nothing.
- A quiet machine: `bench` and `qa` note the load (leaving out the run itself) and the GPU lock (D319, D185); run them with `--runs 3` or more and report the median with its spread, never one run, and say when a run was under load.
- Measure before tuning, change one thing, measure again; keep or revert.

## Debug views and the material bench

- **Debug views** (D385, [development.md](../../../docs/development.md#buildings-debug-views)): F4 (Shift+F4 back) or `cargo xtask remote debug.view <name>` on a run started with `--remote` colours every building by texels, roles, detail, damage, shade, normals or plan, each with its legend. Look before measuring: most "is it the mesh, the shader or the load?" questions are one view.
- **The material bench** (`cargo xtask material <material>`, D386): for any change to how a material looks, before and after; its shots go to `shots/material/`. Judge by its numbers and the reference beside them, not by ad hoc stills.

## Report

Short, and only what the reader needs: what changed and why; the checks run and their numbers (artifacts and fuzz counts against the baseline, bench medians with spread and load); any baseline count raised, with why; gaps opened or closed (by number); decisions added or changed; the commits on origin/main.
