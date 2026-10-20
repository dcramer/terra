# How the crates should be cut

2026-10-18. Question: how should Terra's Rust crates be segmented the way a large Rust/Bevy game is, so a change rebuilds and tests less, a dedicated server (D52) compiles without the art, and the layer rule (D198) is held by Cargo rather than by convention?

**Agreed by the developers on 2026-10-18 (D407)**, who added that the point is as much smaller, better-scoped modules as build time. The migration below is under way, with these names and `terra_life` merged into `terra_town` (2026-10-19).

How it was done: line counts with tests at d85a5fbe; each module's `crate::`/`super::` names outside tests, gathered by a script and grouped into the crates proposed below, every name that would point from a lower crate to a higher one listed (the "seams"); rebuild times from the investigation the same morning; Veloren's, Bevy's and the community's layouts read at their sources ([below](#how-shipped-projects-do-it)).

## The short answer

Two crates hold 131k of the crates' 192k lines and every rebuild runs through both: `terra_build` (71k) and `terra_game` (60k). Cut them along lines already in their code, into nine domain crates of 6 to 17k lines, plus one of 27k that is cut again only if measuring says so:

- **Building's plain Rust in three**, as Veloren keeps `common` apart from `voxygen`: the rules (cubes, operations, plans, kits as rules, rooms, support, loss, saves; 17k), the layout (where each piece of a base goes and what a walker stands on, already written to need no art, "so a server needs no art"; 15k), and the drawing (pieces into surfaces, sections, materials, the drawing's checks; 27k). Settlements (town, village and `terra_life`) become one crate of 9k on the rules alone.
- **The game in seven Bevy crates**, one per layer or domain above the world: the base on screen (10k), the walker (8k), survival: what a player carries, makes and needs (12k), and above it, side by side, creatures and fighting (11k), townsfolk (6k) and building play (10k), with `terra_game` the app that bundles them (4k).
- **The world stops depending on building**: it names five things of it, two of which belong in `terra_core` and the rest only fit its start to the example base. Then no building change rebuilds or tests the world.

The layer order becomes Cargo's: a crate cannot name one above it, so "a layer never needs one above it" (D198) is a compile error, not a review comment. Some 30 names cross a proposed line the wrong way today, each one type, each fixed the way tech.md already prescribes (the asker owns the type). The migration is 15 to 20 pushes, each a pure move or one seam, none blocking another agent for longer than a rebase.

What the shipped projects say, and this proposal follows: the shape of the graph matters more than the number of crates (a chain rebuilds everything above a change, a wide graph does not), the split that pays first is the server's half from the drawing, and nothing is decided before `cargo build --timings` has measured it.

## What is there now

| Crate | Lines | On | Notes |
|---|---|---|---|
| `terra_build` | 71,240 | gen | dev opt-level 3 (D406); rules, layout, drawing, settlements and fixtures together |
| `terra_game` | 60,437 | world, build, life, items, … | `play/` 41k in ~80 flat modules; `build/` 14k, the base on screen and building play |
| `terra_world` | 28,533 | build, land, voxel, … | names `terra_build` only in `start.rs` and `flora.rs` |
| `terra_sandbox` | 12,320 | game, world | |
| `terra_land`, `terra_life`, `terra_voxel`, `terra_climate`, `terra_items`, `terra_core`, `terra_gen`, `terra_sim` | 0.8k to 5.5k | | `terra_life` names only `town` and `village` of building |

Rebuild of `terra_sandbox` in dev after touching one file, under the usual load (investigation of 2026-10-18): `terra_build` 29 s, `terra_world` 23 s, `terra_game` 16 s, the sandbox 7 s; a building change pays all four, 75 s, in series. Release builds, which every shot, film and bench runs (`tools/sandbox/client.ts` `buildOffscreen`), are not incremental: a touched crate is compiled whole, then everything above it. A push testing a building change tests `terra_build`, `terra_life`, `terra_world`, `terra_game` and `terra_sandbox`: everything on Bevy (D398's inverted tree).

## The proposed crates

### Plain Rust (D28: no Bevy)

| Crate | Lines (with tests) | What it holds | From |
|---|---|---|---|
| `terra_gen` | 0.8k + | as now, plus building's hashers (`mesh::QuickMap`, `mesh::Corners`, `layout::{hash, unit, mix3, unit3}`) into `terra_gen::hash`, their home by AGENTS.md | |
| `terra_core` | 1.3k + | as now, plus `Walker` (a body's radius, height and step: what the world's flora and the walker both ask) and the start's plot (below) | `terra_build::walk` |
| `terra_build` | 16.8k | **The rules**: cubes, ops, plans, kits as rules (what a kit costs, burns, how it is framed), rooms, support, loss, fire, saves, cost, order, construct, put_up, parts, shortcuts, roofer, roof_walls, junctions, ground, layers, the hammer's ray | `terra_build` |
| `terra_build_layout` | 14.7k | **Where pieces go**: `layout` (written so "a server needs no art"), `walk` (what holds a walker up, read from the laid pieces), `fitting`'s hanging of doors and shutters | `terra_build` |
| `terra_build_mesh` | 26.7k | **The drawing**: `sections`, `surface`, `mesh`, `draw`, `hull`, `timber`, `roofing`, `modules`, `dress`, `fragments`, `cells`, `relief`, `look`, `packs`, and the drawing's checks (`inspect`, `checker`) | `terra_build` |
| `terra_build_scenes` | 3.8k | The example bases, fuzz and registry fixtures the tests, the sandbox's testbeds and the bench build: a dev dependency below, not a domain | `terra_build::scenes` |
| `terra_town` | 9.2k | **Settlements, laid out and lived in**: `town`, `village`, and `terra_life` merged in (people's days on the town's paths) | `terra_build`, `terra_life` |
| `terra_voxel`, `terra_climate`, `terra_land`, `terra_items`, `terra_sim` | as now | Unchanged: small leaves, cheap to rebuild, already one domain each | |

The drawing is over the range. It splits once more along its own order, the base types and a kit's pieces (`mesh`, `dress`, `modules`, `timber`, `roofing`, `fragments`, `cells`, `relief`, `packs`, 12k) under the assembly that merges them by section (`sections`, `surface`, `draw`, `hull`, `look` and the checks, 14k). Not proposed yet: `draw` and `timber` name each other today, and whether the second cut pays is the measurement's to say (step 9 below).

### On Bevy

| Crate | Lines (with tests) | What it holds | From |
|---|---|---|---|
| `terra_world` | 28.5k | As now, without `terra_build` | |
| `terra_bases` | 10.2k | **The base on screen**: `Site` and the build plugin's core (`build.rs`), drawing (`draw`, `arrays`, `views`, `light`, `atlases`, `hlod`, `layout`, `maps`), memory, store | `terra_game::build` |
| `terra_walker` | 7.6k | **The walker layer** (D198): camera, controls, options, movement, walker, intent, bodies, the figure and what it holds, and the character's look (`appearance`, `character`, `skinning`, `lift`, `swing`, `pace`, `model_files`) | `terra_game::play`, `terra_game` |
| `terra_survival` | 11.5k | **What a player carries, makes and needs**: inventory, bag, storage and its screen, catalogue, registries, kinds, crafting and stations, props, interact, claim, contract, harvest, dig, work, vitals, warmth, death, skills, wear, gains, tiles, torchlight, pile, told, the item icons | `terra_game::play`, `icons.rs` |
| `terra_creatures` | 11.0k | **Creatures and fighting**: creatures, wildlife, fliers, species, coats, sighted, staged, zones, and melee, archery, hits, combat (both sides of a fight together) | `terra_game::play` |
| `terra_folk` | 6.0k | **Townsfolk**: people, townsfolk, the town and settlement on screen, town animals, fences, stalls, the town's sound, houses rising and their scaffolds, pooling | `terra_game::play` |
| `terra_building` | 9.9k | **Building play**: the hammer (`tool`), build menu and dial, acting on a base, loss and fire as they play, props built, the remote's build methods, going up, the sheet, villages placed | `terra_game::play`, `terra_game::build` |
| `terra_game` | 4.2k | **The app**: `GamePlugins` bundling the rest, saving, the probe, the map, the game's sounds and music | `terra_game::play` |
| `terra_sandbox` | 12.3k | As now | |

Names are working names; the developers may prefer others.

## The dependency graph

```
plain:  terra_gen ─ terra_core ─┬─ terra_voxel ─ terra_land (+ climate)
                                ├─ terra_items
                                └─ terra_build ─┬─ terra_build_layout ─ terra_build_mesh
                                                ├─ terra_town
                                                └─ terra_build_scenes (dev: layout, mesh, sandbox)

Bevy:   terra_world ─ terra_bases ─ terra_walker ─ terra_survival ─┬─ terra_creatures ─ terra_folk ─┬─ terra_game ─ terra_sandbox
        (land, voxel)  (build_mesh)                (items)          └─ terra_building ──────────────┘
                                                                          (town under folk; layout, mesh under building)
```

Each Bevy crate depends on those before it on its line and on the plain crates under it; nothing points right. `terra_world` depends on no building crate, `terra_walker` on none above the bases. The gameplay crates fork above survival: building play names nothing of creatures or townsfolk once `Village` moves down (the hammer's `aim` names it to stop building in a town), so a change to either side does not rebuild the other, and they compile in parallel. `terra_folk` stands on `terra_creatures` because the town's animals are wildlife (`town` names `wildlife`). The base layers stay a chain, as layers are: a change to the walker rebuilds every gameplay crate, as it does now.

matklad's measure applies here: "The most important property of a crate is which crates it doesn't (transitively) depend on." The world does not depend on building or the game; the rules, layout and settlements do not depend on the drawing; building play does not depend on creatures or townsfolk.

**The layer rule, by Cargo.** Cargo refuses a cycle between crates, so a lower crate naming a higher one does not build. D198's "a layer never needs a system or resource above it" and tech.md's "the walker still names a few gameplay types" become one rule the compiler holds. What stays convention is only within a crate.

**D28 changes in wording, not in rule.** Its list of crates on Bevy grows from three to the nine above (`BEVY_CRATES` in `xtask/src/check.rs`); the rules every player must agree on stay plain Rust, now with the art out of them.

### Where the shared types go

Every name that crosses a proposed line the wrong way, and its home by tech.md's rule (the side that asks owns it; it moves down only when a lower crate needs it):

| Asker → named today | Type | Fix |
|---|---|---|
| world `start` → building | `Base`, `CELL_M`, `Kind`, `floor_layer`, `example` | The start is fitted to a plot (its size and floor height) the world owns as data in `terra_core`; a test in `terra_build_scenes` holds that the example base fits it |
| world `flora` → building | `Walker` | Into `terra_core` |
| rules → layout/drawing | `layout::roofs::stage_of`, `layout::roles::Role`, `RoofPart`, `timber::section`, `mesh::Material`, `fitting::Hang` | Into the rules (they are fill stages, piece roles, a kit's numbers and a door's side); `ops`' and `loss`' uses of `layout` are tests or a hash |
| kit (rules) → drawing | `Kit::made: dress::Pieces`, `Kit::breaks: fragments::Breaks` | A kit's look keyed by its name in the drawing crate; the kit file still says both, the rules crate reads the rest |
| `town`, `village` → layout | `hash`, `unit` | `terra_gen::hash` |
| walker `movement` → survival, building | `Prop`, `push_out`, `Vitals`, `tool::Builder` | The walker owns a `Solid` (what stops a body; props add it to every item too tall to step over, by an observer) and a `Stamina` it asks of (`Vitals` requires it and fills it, and waking resets both); `Builder` stays building play's, since its fields are building play's own types (the work, the price, the target), and requires a bases-owned `build::hammer::Hammer` holding only what the layers below read: the material picked (`draw` keeps its textures resident) and whether a part is in hand (Ctrl is then the material's key, not the roll); the hammer's systems update it where they change those (changed from moving `Builder` itself, 2026-10-19) |
| walker `figure` → survival | `held::Grip` | The hand's socket and whose hand it is, `Grip { player }`, into the walker's `figure`, which puts it on the hand; what it shows and whether a sword is drawn stay survival's, in `held::InHand`, which held's plugin makes `Grip` require, and the sword fight (`melee`) sets `drawn` on (changed from moving `Grip` whole, 2026-10-19: what it shows is an item kind, survival's, and the walker draws no item) |
| survival `bag` → building | `BuildMenu`, `Dial` | The bag asks only whether the number keys are a screen's, which the walker's `Hands::choosing` already says (the game fills it from the dial and the build menu, for the walker's intent); the bag reads that (changed from a one-open-screen state, 2026-10-19: the question had an owner already, and menus are not being redesigned) |
| survival `claim`, `storage`; building `tool` → folk | `settlement::Village` | The marker into survival, which asks whose land this is; this is the move that lets building play stand beside the townsfolk |
| survival `crafting`, `warmth` → folk | `people::{Doing, Person}` | A station's or fire's user, owned by survival: `crafting::Worker` (name, trade, the spot they are at work at, how hard they work), which `Person` requires; people set it when put in the world (an insert hook) and with what they do (`Person::set_doing`, in the same systems, so no reader sees one a tick old) |
| survival `harvest` → creatures | `combat::Swing` | Into survival: a tool's swing, which combat uses too (`harvest::Swing`, which harvest's plugin makes every player have) |
| survival `registries` → creatures | `species::Bestiary` | Each domain reads its own data files (D136): creatures their bestiary, as a `registries::Registry` (its file, how it is read, its kinds) that their plugin has read with the rest (`read_with_the_rest`), so it is still watched, read again and refused with the others as one whole |
| survival `station_screen` → creatures | `archery::bow` | The bow as an item into survival: how fast and hard a bow shoots at full draw, `kinds::BOW_SPEED` and `BOW_DAMAGE`, beside the `ToolDef::shoots` that makes an item a bow; archery shoots by them, a station's screen shows them |
| creatures `sighted` → folk | `town_animals::Beast` | A tame marker the creatures own, the town sets: `creatures::Tame`, which `Beast` requires; what a film sees of the animals counts the wild `Creature` and the `Tame` |
| building `loss` → creatures | `hits::Shake` | Into the walker, whose view it is: `shake::Shake`, its running down and the camera's move off where it was put; blows (`hits`) and a collapse nearby (`loss`) kick it, the blows before it runs down, as before |
| folk `rising`, `town`, `settlement` → building | `build::props::fit_all` | Into folk's `settlement`: only the townsfolk's houses are fitted whole (the player hangs each door), and it names only the bases' `Build` and the rules |

## What a dedicated server compiles

Every plain crate above but `terra_build_mesh`: the rules, the layout (pieces and walking, for collision, D51's server deciding everything), settlements and their people's days, the land, terrain, climate, items. No height maps, pieces, surfaces or materials: the server builds without the drawing's 27k lines and the art they read. This is the split Veloren makes between `common` and `server` on one side and `voxygen` on the other.

The Bevy domain crates still hold their own drawing beside their logic, as `terra_game` does now. A headless server of them (D51: the same code) would build them without Bevy's rendering, or cut each into its logic and its view; that is networking's track (D) to choose when it starts, not now. What this proposal keeps possible is the rule that a domain's drawing is its own plugin inside its crate.

## What it should change

These are expectations, to be measured, not results.

| Change in | Rebuilt in dev now | Rebuilt after | Push tests now | Push tests after |
|---|---|---|---|---|
| building's drawing (27k) | build (71k), world, game, sandbox | mesh, then the Bevy crates from the bases up | build, life, world, game, sandbox | the same as it rebuilds: not the rules', layout's or towns' sweeps, not the world |
| building's rules | build, world, game, sandbox | every building crate, the town, the Bevy crates from the bases up | build, life, world, game, sandbox | the same, not the world |
| a town or a person's day | build (71k) or life, world, game, sandbox | town, folk, building, game, sandbox | build or life and all above | town, folk, building, game, sandbox |
| a creature or a fight | game (60k), sandbox | creatures (11k), folk, game, sandbox | game, sandbox | the same four, not building play |
| building play | game (60k), sandbox | building (10k), game, sandbox | game, sandbox | the same three |
| the walker or survival | game (60k), sandbox | it and every gameplay crate above | game, sandbox | the same |
| the world | world, game, sandbox | world and every Bevy crate | world, game, sandbox | the same |

A change low in a chain still rebuilds all above it: the gain there is only that each crate rebuilt is smaller and rustc's incremental cache is per crate. The gains are at the top and at the sides: gameplay domains, settlements, and the world kept clear of building.

- **Incremental dev rebuilds.** The world leaves every building rebuild, 23 s of the 75 s. Work in the drawing recompiles 27k lines rather than 71k, and in a gameplay domain 6 to 12k rather than 60k. Cargo marks every crate above a changed one dirty (its fingerprints, `fingerprint/mod.rs`), and rustc's incremental cache then reuses what it can inside each; Bevy's generics are instantiated again in each crate that uses them, so the total compile work can rise while the rebuilt part shrinks. Only measuring says by how much.
- **Release rebuilds.** Release is not incremental (Cargo's default, `incremental = false`), so a touched crate is compiled whole, and today that is 60 or 71k lines. Smaller crates help more here than in dev. Crates side by side also compile in parallel (`terra_town` beside the layout and the drawing; creatures beside building play), which one 71k-line crate cannot.
- **Dev opt-levels.** `[profile.dev.package."*"]` covers dependencies, not the workspace's own crates, so each building crate needs its own `opt-level = 3` line as `terra_build` has now (D406), and Veloren has for its world. Generic code is optimised at the level of the crate that instantiates it (the Cargo book), so the hashers moving to `terra_gen` keep their speed only where they are not generic over the caller's types; the sweeps' times before and after say so.
- **The scoped gates (D398).** A push tests the changed crates and their dependents; with smaller crates the dependents are fewer, and building's slow sweeps (D406) run only when their own crate or one under it changed. `xtask/src/scope.rs` places a file by its crate folder already; its `READ_BY` lists, the building ratchets' trigger (the kit checker, artifacts, fuzz) and `BEVY_CRATES` need the new names.
- **Not this proposal.** Bevy's `dynamic_linking` feature, which its setup guide calls "the most impactful compilation time decrease", and a faster linker are cheaper wins in the same direction; they need no move and are measured on their own.
- **For agents.** A crate of 5 to 15k lines is one an agent reads by its README and module list without reading the rest, and one whose tests run alone in seconds.

### How to measure

Before the first move, and after each step below, on the same machine with the load recorded:

1. `cargo build -p terra_sandbox --timings` in dev and release from clean: per-crate compile time and how much ran in parallel (`target/cargo-timings/`).
2. Touch-only rebuilds of the sandbox in dev and release after touching one file in each future crate (a fixed list, `layout/runs.rs`, `sections.rs`, `town/mod.rs`, `play/creatures.rs`, `play/movement.rs`, …).
3. `cargo xtask check` (the push gate) on a one-line change in each of the same files.

That is the same check done some fifteen times, so it is a task (D71): `cargo xtask rebuilds` ([tools/rebuilds](../tools/rebuilds/README.md)) does 1 and 2; the push gate's time is read from each push.

### Measured: the baseline

At 9a523313, before any cut, on the 10-core Mac with other agents building (load average 32 to 56). Seconds to rebuild the sandbox after the probe's file is touched, and the workspace crates compiled for it:

| Probe | Dev | Of which | Release | Of which |
|---|---|---|---|---|
| core (`terra_core/src/lib.rs`) | 54 | game 25, world 19, sandbox 13 | 823 | game 645, world 367, sandbox 111 |
| building's rules (`cubes.rs`) | 58 | world 22, game 21, build 11, sandbox 9 | 1,136 | game 768, world 487, build 232, sandbox 113, life 48 |
| building's layout (`layout/runs.rs`) | 48 | game 19, world 18, build 9, sandbox 7 | | |
| building's drawing (`sections.rs`) | 46 | world 16, game 15, sandbox 9, build 8 | | |
| a town (`town/streets.rs`) | 38 | game 14, world 11, build 8, sandbox 7 | | |
| the world (`flora.rs`) | 43 | game 21, world 16, sandbox 10 | | |
| the walker (`play/movement.rs`) | 44 | game 29, sandbox 13 | | |
| a creature (`play/creatures.rs`) | 38 | game 26, sandbox 10 | | |

Per-crate seconds overlap, since cargo compiles crates side by side. The world is 11 to 22 s of every building rebuild in dev and 487 s of one in release, which step 3 removes; release rebuilds, which every shot and film pays, run to 14 to 19 minutes under this load, `terra_game` alone 11 to 13. The release pass was stopped after two probes so as not to hold the machine for two hours. These release figures predate D408 (c5a762b3), which builds our crates incrementally in release too, so later release runs are not comparable with them; the dev figures are. A building push's gate took 16 to 59 minutes, and a push that slow often loses the race to main to another agent's.

### Measured: after step 3

At f0062671, the world off building (steps 2 and 3 on main), on the same Mac under load 34 to 70, dev only (`cargo xtask rebuilds --dev`). Four probes waited on a lock behind other builds, so their totals (1,462 to 2,399 s) are left out and only their crates' seconds count:

| Probe | Dev | Of which | Baseline |
|---|---|---|---|
| core (`terra_core/src/lib.rs`) | 50 | game 23, world 21, sandbox 10, build 8 | 54: game 25, world 19, sandbox 13 |
| building's rules (`cubes.rs`) | 38 | game 21, sandbox 10, build 7 | 58: world 22, game 21, build 11, sandbox 9 |
| building's layout (`layout/runs.rs`) | 50 | game 28, sandbox 13, build 8 | 48: game 19, world 18, build 9, sandbox 7 |
| building's drawing (`sections.rs`) | (waited) | game 35, sandbox 14, build 11 | 46: world 16, game 15, sandbox 9, build 8 |
| a town (`town/streets.rs`) | (waited) | game 28, build 9, sandbox 7 | 38: game 14, world 11, build 8, sandbox 7 |
| the world (`flora.rs`) | (waited) | world 22, game 21, sandbox 9 | 43: game 21, world 16, sandbox 10 |
| the walker (`play/movement.rs`) | (waited) | game 28, sandbox 13 | 44: game 29, sandbox 13 |
| a creature (`play/creatures.rs`) | 73 | game 32, sandbox 19 | 38: game 26, sandbox 10 |

No building probe compiles the world any more (0 s, from 11 to 22 s), and a building change no longer tests it. What is left of a building rebuild is the game, which names building throughout until it is cut (step 4 onward), so the wall time falls only where the world was on the critical path (the rules, 58 to 38 s). Building now stands on `terra_core`, so a core change compiles building too (8 s), which it did not before. The game's own seconds wander with the load (19 to 35 s for a building probe), more than the cut moves them.

**After step 4** (f3c257ef, `terra_town`; load 11 to 13, so lower than above for the load alone): a town (`terra_town/src/town/streets.rs`) 9 s, game 5, sandbox 2, and no longer building (0 s, from 9); building's rules (`cubes.rs`) 9 s, game 4, sandbox 2, build 2, the town under 1 s.

### Measured: after step 5

At f8536e3c, building cut into the rules, layout, drawing and scenes and the village in the town (step 5 on main), on the same Mac under load 6 to 9, dev only (`cargo xtask rebuilds --dev`). The load is a tenth of the earlier runs', so the seconds fall with it; what to compare is which crates compile. A crate not named was not compiled:

| Probe | Dev | Of which | After step 3 | Baseline |
|---|---|---|---|---|
| core (`terra_core/src/lib.rs`) | 13 | game 6, world 5, sandbox 3, build 0.9, build_mesh 0.7, build_layout 0.5, town 0.5; land, build_scenes, voxel and core under 0.4 | 50: game 23, world 21, sandbox 10, build 8 | 54: game 25, world 19, sandbox 13 |
| building's rules (`cubes.rs`) | 9 | game 4, sandbox 2, build 1, build_mesh 1, town 0.5, build_layout 0.5, build_scenes 0.3 | 38: game 21, sandbox 10, build 7 | 58: world 22, game 21, build 11, sandbox 9 |
| building's layout (`layout/runs.rs`) | 8 | game 4, sandbox 2, build_mesh 0.7, build_layout 0.4, build_scenes 0.3 | 50: game 28, sandbox 13, build 8 | 48: game 19, world 18, build 9, sandbox 7 |
| building's drawing (`sections.rs`) | 8 | game 4, sandbox 2, build_mesh 0.6, build_scenes 0.3 | (waited): game 35, sandbox 14, build 11 | 46: world 16, game 15, sandbox 9, build 8 |
| a town (`town/streets.rs`) | 8 | game 4, sandbox 2, town 0.5, build_scenes 0.3 | (waited): game 28, build 9, sandbox 7 | 38: game 14, world 11, build 8, sandbox 7 |
| the world (`flora.rs`) | 10 | game 4, world 4, sandbox 2 | (waited): world 22, game 21, sandbox 9 | 43: game 21, world 16, sandbox 10 |
| the walker (`play/movement.rs`) | 7 | game 4, sandbox 2 | (waited): game 28, sandbox 13 | 44: game 29, sandbox 13 |
| a creature (`play/creatures.rs`) | 7 | game 4, sandbox 2 | 73: game 32, sandbox 19 | 38: game 26, sandbox 10 |

Against the baseline, no building or town change compiles the world, and against step 3, a change to building's layout, drawing or a town no longer compiles the rules (`terra_build`): the layout compiles only what is above it (the drawing, the scenes), the drawing only itself and the scenes, a town only itself and the scenes, and a building push's gate tests those crates rather than one crate of everything. Every building crate together is under 2.5 s of a rebuild; the game and the sandbox are 6 of every probe's 7 to 10 s, since `terra_game` is still one crate that names building throughout, which step 8 cuts.

## How shipped projects do it

Read at their sources on 2026-10-18: Veloren master at c1e20929 (its manifests and `cargo metadata`), Bevy main at 85503941 (0.20.0-dev; 0.20.0 the latest release), Cargo master at 3022244f; line counts ours, with tests.

| Project | Layout | What it shows |
|---|---|---|
| **Veloren**, the open-source voxel RPG on a dedicated server ([manifest](https://gitlab.com/veloren/veloren/-/raw/master/Cargo.toml), [book](https://book.veloren.net/contributors/developers/codebase-structure.html)) | 25 crates: `common` (68k) with small sub-crates (`base` 0.7k, `ecs` 0.5k, `net` 3k, `state` 2k, `systems` 9k), `world` (75k), `rtsim` (12k), `server` (39k) and `server/agent` (13k), `network`, `client` (5k), and `voxygen`, the graphical client (121k) with `voxygen/anim` (43k) | The headless server, `server-cli`, compiles `common`, `world`, `rtsim`, `server`, `network`, and neither `voxygen` nor `client`; voxygen sits on top of everything, `server` included, for single player. The book gives two reasons for crates: parallel compiling and depending only on what one needs. Dev opt-level is set per crate (world, anim and the network protocol at 3), which only works because they are crates. Its big crates stay big: it is evidence for the server/client cut, not for even sizes |
| **Bevy** ([crates](https://github.com/bevyengine/bevy/tree/main/crates)) | 65 crates in a flat `crates/*`, 569k lines: `bevy_ecs` 128k, `bevy_reflect` 45k, `bevy_pbr` 41k, `bevy_render` 38k, `bevy_app` 6k, many of 1 to 8k | Layered by dependency: ecs and app at the bottom, then vocabulary crates (math, asset, transform, mesh, camera, light, material), then render, then pbr on render. Recent releases pulled types out of `bevy_render` into `bevy_camera`, `bevy_mesh`, `bevy_light` and others so crates above need less: moving a type down to the crate that must know it, as tech.md's rule does. Its README's goals are "modular" and changes that "compile quickly"; no document gives a measured case for the split. `bevy_dylib` exists to link Bevy dynamically for faster iterative builds |
| **Digital Extinction**, an open-source Bevy RTS ([repo](https://github.com/DigitalExtinction/Game)) | 28 crates in `crates/*`, 34k lines, the largest 3.7k (audio, camera, combat, construction, movement, pathing, terrain, …) | A crate per feature: the "microservices" end D198 rules out |
| **Emergence** (Leafwing Studios) | one 25k library crate, a 27-line game crate and a macros crate | The other end: one crate |

Community guidance:

- **matklad, [Large Rust Workspaces](https://matklad.github.io/2021/08/22/large-rust-workspaces.html)** (rust-analyzer's author): a flat `crates/*` suits projects of ten thousand to a million lines (rust-analyzer: 200k lines in 32 crates), each folder named as its crate. Terra already does both.
- **matklad, [Fast Rust Builds](https://matklad.github.io/2021/09/04/fast-rust-builds.html)**: the graph's shape drives build time; a chain compiles in series and rebuilds all above a change, a wide graph does neither; a good shape is a small vocabulary crate at the bottom, independent domain crates in the middle and a leaf tying them together; generics instantiated across many crates can make compile time grow out of proportion; serialization belongs at the edges. Measure with `cargo build --timings`.
- **Bevy's [setup guide](https://bevy.org/learn/quick-start/getting-started/setup/)**: dev at opt-level 1 with dependencies at 3, as Terra has; dynamic linking and a fast linker as the biggest compile-time wins. The Bevy Cheatbook is out of date (0.14) and says nothing of crates.
- **Cargo**: a change in a crate marks every crate depending on it dirty (`src/cargo/core/compiler/fingerprint/mod.rs`); dev builds are incremental with 256 codegen units, release not, with 16 ([profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)); `--timings` writes each crate's time and what ran in parallel ([timings](https://doc.rust-lang.org/cargo/reference/timings.html)).

What Terra takes from them: the server's half apart from the drawing first (Veloren); types moved down to the crate that has to know them (Bevy's render split, tech.md's rule already); domains side by side where they can be, not a chain (matklad); domain-sized crates between Digital Extinction's feature crates and one crate, as the developers asked; and every step measured.

## Migration order

Each step is one push that builds, passes the gate and changes no behaviour. A move is a pure rename (`git mv`) plus the manifest and `use` lines, so git's rename detection carries other agents' in-flight edits to the moved files through their rebase. Out of `terra_game`, which ends on top, the old path stays as a re-export (`pub use terra_creatures as creatures`) until nothing names it, so a branch written against it still builds. Out of `terra_build`, which ends at the bottom, a re-export would be a cycle, so that push changes the callers' paths too (the game's and the sandbox's `terra_build::sections`, `::town`, `::scenes` and the like, about a hundred names). Each move gets a line in `docs/decisions-log.md` and lands when no other agent is mid-way through a large change of the same files (the lead checks the linked sessions).

1. **Measure** the baseline (above).
2. **Cut building's seams inside the crate**: the hashers to `terra_gen::hash`; `stage_of`, `Role`, `RoofPart`, `timber::section`, `Material`, `Hang` into the rules' modules; a kit's look out of `Kit`. A test in `terra_build` fails when a rules module names a layout or drawing module, so the line holds before any move. Two or three pushes. Done: a5135e5b (the hashers), c7b331ac (the rest into a rules module, `pieces`, the kit's modelled pieces and `Breaks` moved down as plain data rather than taken out of `Kit`, and the test `seams`).
3. **Take the world off building**: `Walker` to `terra_core`, the start's plot as data, the example's fit as a test. `terra_world`'s manifest loses `terra_build`. One push; the first measured gain. Done: f0062671, measured above.
4. **`terra_town`**: move `town` and `village`, merge `terra_life` in, as the developers chose. One push. Done for `town` and `terra_life` (2026-10-19); `village` followed in step 5. It waited because the scene registry (`scenes::registry`, whose village scenes the mesh checker, the drawing audit and the voxel contract check), `construct` and `put_up`'s tests and a sections test lay its buildings, so it moved once the scenes were a crate of their own that could stand on `terra_town`, its plans, schemes, roofs and the settlement's palette with it, since nothing below the town names them.
5. **`terra_build_scenes`**, then **`terra_build_mesh`**, then `village` into `terra_town`, then **`terra_build_layout`**: four pushes of moves, `terra_build` left as the rules. From the top, since the drawing names the layout and the scenes name both, and a crate's tests can lay only what is below it: each push moves the tests that lay what leaves into the crate that can see both. The example bases built from the rules alone (the example cabin, the bench village, the castle, the burg, the junctions) stay in `terra_build`, whose tests and every crate's above lay them. `terra_build_scenes` done (2026-10-19); `terra_build_mesh` done (2026-10-19), with the layout's tests that draw what they lay moved up into its tests and the village's drawing audit into the scenes'. `village` into `terra_town` done (2026-10-19), with `construct` and `put_up`'s tests that put its buildings up moved into its tests. `terra_build_layout` done (2026-10-19): the walking and a prop's frame and leaves are traits on the rules' types (`Walking`, `Hanging`), a prop's hole and leaf count went to the rules (`pieces`), the operations' cases that walk what they build and the rules and decks integration tests moved up, and the `seams` test went, Cargo holding its line. Step 5 done.
6. **Measure** again; write the result here. Done: f8536e3c, measured above (after step 5).
7. **The game's seams**, one or two a push, in the order of the table above, each with the asker owning the type; `Village` first, since it is what lets building play fork from the townsfolk. A test in `terra_game` (`seams`) places every module in the crate it is to move to and fails when one names a crate above or beside its own, but for the crossings not cut yet, each listed with its seam; it found six more than the table (numbered 10 to 15 there: the base's plugin adding building play's, a fire and creatures, a fall's shake, a rising house's doors, storage acting on a base, wildlife and a rising town), which step 8 needs cut too. `held` and `worn` are survival's (they name the inventory). Done: `Village` (seam 1) into survival's `claim`; seam 2: `Solid` and `Stamina` into the walker (`bodies`, `walker`), `Hammer` into the bases; seam 3: `Grip` into the walker's `figure`, `held::InHand` survival's; seam 4: the bag reads the walker's `Hands::choosing`; seam 5: crafting and warmth read survival's `crafting::Worker`, which people set; seam 6: `Swing` into survival's `harvest`; seam 7: the creatures have their bestiary read with the registries (`Registry`); seam 8: a bow's numbers into survival's `kinds`; seam 9: a town animal is `creatures::Tame`; seam 12: the view's `Shake` into the walker; seam 13: `fit_all` into folk's `settlement` (the seams test now follows a module a `use` brings in by name, which it missed in `town` and `settlement`).
8. **`terra_bases`**, **`terra_walker`**, **`terra_survival`**, **`terra_creatures`**, **`terra_folk`**, **`terra_building`**: one push each, bottom up, what is left in `terra_game` being the app. D28's list, `BEVY_CRATES`, `READ_BY`, `crates/README.md`, docs/tech.md's layers and each crate's README move with the push that makes the crate.
9. **Measure** again, and decide the drawing's second cut and whether `terra_world`'s shell (window, launch options, scripted runs, remote, sound, films; some 9k) is worth its own crate.

Steps 2 to 5 stand alone: if the developers want only the server-facing half, stop after 6.

## Open questions for the developers

- Is this granularity what "large enough chunks i.e. not 'microservices'" (D198) means: nine domain crates of 5 to 17k lines, the small existing leaves kept?
- `terra_life` merged into `terra_town`, or kept apart on it?
- The names.
