# Repository layout

2026-10-12. Question: where should each kind of thing in this repository live (game code, developer tools, the asset pipeline, source art, runtime assets, docs, research, experiments), what do the folders we have actually hold, and what do other projects and the games industry call these things?

Read from the tree at `ff0c37f` (519 tracked files) and from the top-level layouts of Veloren, Bevy, Fish Folk's Jumpy and rust-analyzer, fetched the same day. The prior art for engines (Unreal, Valve's Source, Unity) is from general knowledge, not checked.

## What we have

| Folder | Meant to be | Actually holds | Problem |
|---|---|---|---|
| `crates/terra_gen`, `_sim`, `_voxel`, `_core`, `_build`, `_items` | Engine-free rules (D28) | That | None. `terra_voxel` has a `main.rs` that is a benchmark. |
| `crates/terra_sandbox` | A test environment that "is not the game" (D46) | The game. The `play` experiment is 48 files and about 18,000 of the sandbox's 26,000 lines: player, inventory, crafting, creatures, people, combat, saving, options, music. Also the developer tools (`workbench`, `icons`), the testbeds (`forest`, `scatter`, `look`, `build`), the shell every one of them uses, and an end-to-end test in shell (`loop.sh`, 117 lines). | The biggest one. The README and D46 say the opposite of what is there. `terra_game` exists only on paper. |
| `crates/terra_sandbox/assets` | The sandbox's files | Every model, sound, shader and icon the game has, 27 MB. `cargo xtask icons` writes into it; the character skill copies into it. | Runtime content owned by a test app. A second binary would have to reach into the sandbox's folder. |
| `spike/character_gear` | A throwaway spike | The working character pipeline: rig conversion, the rig checker, export, gear fitting, held looks. The `character` and `tripo` skills, `tools/tripo`, `tools/sandbox`, `docs/development.md` and comments in game code all point into it. | Breaks our own rule that nothing depends on a spike. |
| `spike/model_bakeoff` | A throwaway spike | The bake-off, plus `run.sh`, which the character pipeline uses to reach Blender, and `out/`, ignored, where the skills write Tripo output. | Same. |
| `spike/.../out/` (ignored) | Scratch output | The **inputs** we cannot remake for free: approved design sheets, Tripo's rigged bodies, the checker's baseline. The character skill says to rerun every character from them. They exist in one place: an ignored folder inside one agent worktree (`llm-art-model-research`), 432 MB of which about 10 MB is real input (seven design sheets, 8.5 MB; two rigged bodies, 1.8 MB) and the rest scratch. | Source art with no home. Removing that worktree would lose it, and no one else can rebuild a committed character. |
| `spike/` root | Index of spikes | A README for the first Godot spike, which describes untracked folders (`bin/`, `solar_system_demo/`), plus `demo.patch` and a 1 MB screenshot | Stale. |
| `spike/planet_test`, `spike/sim_rust`, `demo/`, `prototypes/sim_toy` | Godot builds kept as reference until Rust passes the same checks (D12) | Four Godot or GDExtension projects in three places | Scattered. `prototypes/` holds one thing, and "prototype" in `docs/prototypes.md` means a milestone build, not this. |
| `tools/` | Team tooling outside the game (D121) | Four TypeScript or shell tools, each with a README | Fine. `token-stats` is `.mjs`/`.js`, not TypeScript. |
| `xtask/` | Repository tasks | That | Fine. |
| `docs/` | Decisions and design | That, plus `proposals/` and `images/` (18 MB of screenshots, several cited only from `research/`) | Mostly fine. Images sit with the wrong owner. |
| `research/` | Dated findings | That | Fine. Borrows its pictures from `docs/images`. |
| `.claude/skills` | Agent playbooks | Four skills | Fine in place; two of them route through `spike/`. |

The pattern behind most of it: things started as spikes or experiments, became load-bearing, and were never moved. The sandbox grew the game; the spikes grew the pipeline; the spikes' scratch folder grew the source art.

## Prior art and terms

| Term | Means | Who | For us |
|---|---|---|---|
| **Virtual workspace, flat `crates/`** | A root `Cargo.toml` with no package, every crate in `crates/<name>`, folder named as the crate | rust-analyzer (matklad's "Large Rust workspaces"), Bevy (`crates/bevy_*`) | We do this. Keep it. |
| **xtask** | Repository tasks as a Rust binary run by `cargo xtask` | rust-analyzer, Bevy-adjacent projects | We do this. End-to-end runs such as `loop.sh` belong here. |
| **common / client / server / world** | Split a multiplayer game into shared rules, the frontend, the authority and generation | Veloren: `common`, `client`, `server`, `voxygen` (the frontend), `world`, `network`, `rtsim` | Our engine-free crates are Veloren's `common` and `world`; `terra_game` is its `voxygen`. Validates D28. |
| **Runtime assets, `assets/` at the root** | What the shipped game loads, in one folder every binary reads | Bevy's default asset path; Bevy's repo (one `assets/` for all examples); Veloren (`assets/`, split by who reads it, with `credits.ron`); Jumpy | Move ours to the root. |
| **Source art / source assets, versus cooked or built assets** | The editable inputs (DCC files: `.blend`, concept sheets, raw generator output) kept apart from what the pipeline produces from them | Unreal (`Content/` versus a `SourceArt/` or `Raw/` folder beside it); Valve's Source (`content/` versus `game/`) | We have no source folder. Ours are in an ignored scratch folder. |
| **Asset pipeline; bake, cook, import** | The scripts that turn source art into runtime assets | Industry-wide; D129 already calls icons "baked" | `spike/character_gear` is our pipeline. |
| **Data-driven content** | A new kind of thing is a data file, not code | Veloren's `assets/common/items/*.ron` | D127 asks for it. Our catalogues are still Rust. Not a layout change yet, but it decides where data will go. |
| **Testbed, gym, test map** | A small scene for one mechanic or one measurement, never shipped | Box2D and Rapier "testbeds", Bevy's `examples/`, "gym levels" in game studios, Unreal test maps | `forest`, `scatter`, `look` and `build` are these. "Experiment" works; it just should not hold the game. |
| **Tools** | In games: editors and pipeline utilities, not the game | Bevy's `tools/` (repository Rust binaries); studio "tools teams" | `workbench` and the icon baker are tools that need the engine, so they stay Rust in the sandbox; Blender and Tripo scripts go in `tools/`. |
| **Spike** | Throwaway code that answers one question, then is deleted (XP, Kent Beck) | Agile usage generally | Ours are not deleted, and some stopped being throwaway. |
| **Prototype, vertical slice, first playable** | Milestone builds | Game production | Our milestones in `docs/prototypes.md`. The `prototypes/` folder clashes with this. |
| **GDD, TDD** | Game design document; technical design document | Game production | `docs/` holds both, grouped by the index. |
| **ADR, RFC** | A record of one decision and why; a numbered proposal for review | Nygard's ADRs; Rust's `rfcs/text/NNNN-name.md` | `decisions.md` is an ADR log in one table; `docs/proposals` are RFCs. Both fine. |
| **AGENTS.md** | Instructions for coding agents at the root | agents.md convention; rust-analyzer has `AGENTS.md` and `CLAUDE.md` as we do | Fine. |

## Proposed layout

```
assets/              Runtime assets every binary loads: models, sounds, shaders, icons; later data (D127)
art/                 Source art: approved design sheets, generator output we keep, .blend files
crates/
  terra_gen …        Engine-free rules (D28), unchanged
  terra_game         The game: its Bevy plugins, one module per feature, and a binary to play it
  terra_sandbox      Testbeds and engine-side tools (workbench, icon baker), on top of terra_game
docs/                Decisions, design, tech, proposals; images the docs cite
research/            Dated notes; images the notes cite
spike/<question>/    Only throwaway work, each with a README stating its question; deleted once its note is written
tools/               Everything outside the game: tripo, sandbox remote, token stats, blender runner,
  characters/        and the asset pipeline: rig conversion, checker, export, gear and held fitting
xtask/               Repository tasks, including the end-to-end loop
```

Where a new thing goes:

| It is | It goes in |
|---|---|
| A rule every player must agree on | An engine-free crate |
| Game behaviour in Bevy | A plugin module in `terra_game` |
| A scene for testing or measuring one thing | A sandbox experiment |
| A developer tool that needs the renderer | A sandbox experiment, run by an `xtask` task if it writes files |
| A script that runs outside the game (Node, Blender, shell glue) | `tools/<name>/`, with a README |
| A file the game loads | `assets/` |
| A file the pipeline reads to make one the game loads | `art/` |
| Anything a pipeline writes that we do not keep | An ignored `out/` in the tool, never something a later step relies on |
| A question being tried | `spike/<question>/`; when anything else starts to depend on it, it moves first |

## Moves, in order

Each is its own commit and passes `cargo xtask check`.

1. **Assets to the root.** `crates/terra_sandbox/assets` to `assets/`; point Bevy's asset path, `xtask win` and `xtask icons` at it; fix paths in skills, docs and `THIRD_PARTY.md`.
2. **The pipeline out of the spikes.** The scripts the character skill uses go to `tools/characters/`, calling `tools/blender/run.sh` directly; skills, docs and code comments follow. What is left in the two spikes is the bake-off itself.
3. **Source art in.** `art/characters/<name>/` holds the approved sheet and Tripo's rigged body; the checker's baseline is regenerated from `assets/` or committed beside the tool. Needs the developers (below).
4. **The game out of the sandbox.** `experiments/play` and the parts of the shell every build has (frame readout, logs, remote, recording) become `terra_game`; the sandbox depends on it. D46 is rewritten to match.
5. **Godot in one place.** `demo/`, `prototypes/sim_toy`, `spike/planet_test`, `spike/sim_rust`, the root spike README and `demo.patch` go to one folder, or are deleted (below). `prototypes/` goes away.
6. **Small ones.** `loop.sh` becomes `cargo xtask loop` (it has real logic, D121); `terra_voxel`'s benchmark becomes a Cargo example; research images move to `research/images/`; `token-stats` moves to TypeScript.

## Questions for the developers

- **Source art in a public repository.** Commit approved sheets and Tripo's rigged bodies to `art/` in plain git, use Git LFS, or keep them outside git? Recommendation: plain git, only for inputs that cost credits to remake, and revisit LFS if `art/` passes a few hundred megabytes. LFS on a public GitHub repository charges bandwidth to us for every clone. The inputs are about 10 MB today, so plain git costs little. Whatever we choose, copy them out of the `llm-art-model-research` worktree before it is removed.
- **The Godot builds.** D12 retires them once the Rust build passes the same checks. Keep them, gathered in `legacy/godot/`, until the terrain match in `docs/terrain.md` passes, or delete them now and rely on git history and `research/godot-spike.md`? Recommendation: gather them now, delete when that check passes.

## Limits

- The engine layouts (Unreal, Source, Unity) are from general knowledge, not read for this note.
- Sizes are measured; the cost of LFS bandwidth for a repository like ours is not.
- Splitting `terra_game` from the sandbox is sketched by crate, not by module. Which parts of the shell belong to the game is decided when it is done.
