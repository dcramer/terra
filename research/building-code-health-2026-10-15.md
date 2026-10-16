# Building's code health

2026-10-15. Question: after many changes to the building system, does it carry baggage (code, data, options and tools nothing uses, leftovers of what was removed or replaced), the same logic in two places, modules grown into monoliths, or rules on the wrong side of a layer? The developers: "make sure we've not overloaded a bunch of baggage too. weve had lots of changes in the system so we want to keep the code well organized".

How it was done: `terra_build`, `terra_game/src/build`, the parts of `terra_game/src/play` that touch building, the building tasks in `xtask`, `tools/kits`, `tools/sandbox` and the kits and packs in `assets/data`, at 037b75f1. Every public item's name was counted across the workspace and its tools, split into live code and tests; every serde field of the kit data was traced from its file to a reader; every remote method, flag and script was traced to a caller; functions over 150 lines and files over 1,000 were measured. No `cargo udeps` or nightly toolchain is installed, so unused dependencies were not checked. Agents were working at the same time on sections and uploads (performance), roofs and roles, quoins and pieces, the material checks and `tools/sandbox/qa.ts`; their files were left alone, and what needs them is planned below.

## The short answer

Less baggage than feared. No per-kit code (the test in `kit.rs` holds that), no leftovers of gatherable debris, door hinge sides, the 16.7 ms budget or the pre-D290 kit format in code, and every xtask task, sandbox script and remote method has a caller. What there was: a dead drawing path and its data (kits' plain colours), a second weld and a second tangent path, a few functions and one remote option nothing used, and the compatibility layer for older saves spread through `save.rs` and `lib.rs`. Those are fixed. The real debt is size and duplication: one function of 1,302 lines, five more over 400, `sections.rs` at 2,876 lines, and plan-building logic copied between the hammer, the remote and the going-up scene. Most of it sits in files other agents are in today, so it is planned, not done.

## Fixed

| Commit | What |
|---|---|
| c08a50cc | Dead code out: `mesh::build`'s kit colours (`KitLook`; every kitted cube is skipped before its colour is read), `Surface::weld` (a copy of `Indexed::welded` nothing called), `Surface::tangents` (a second, unweighted tangent path only a test used; the test now checks `Indexed::tangents`, the one the game draws with), `Side::is_wall`, `Base::mesh_with_kits`, `Base::attachments`, `village::town_streets` |
| 53a7a5a0 | `build.damage`'s `build` option and `damage_built`: a one-off for D305's castle measurement, passed by no script (D249) |
| 33537f37 | `save.rs` split: the snapshot of now stays; every older format (shortcut lists, walls on edges, rooms, D209's roof sections) moves to `save/older.rs`, so a cut of old saves is one file |
| e24f345a | `lib.rs` split, 1,709 lines to 1,055: the quick tools' shortcuts and wall tiles to `shortcuts.rs`, the hammer's ray to `ray.rs` |

Lines: about 1,220 added and 1,350 removed, nearly all of the added being the two splits' moves; about 200 lines of code deleted outright. Wood and stone's faults are untouched (nothing drawn changed), `cargo xtask check` passed on every commit.

## Leftovers of what was removed, confirmed

| Removed | What is left | Verdict |
|---|---|---|
| Glass (D225) | `Fill`'s `Glass` alias, `OLD_GLASS` and `from_v1` in `cubes.rs`, a save test | Reading old saves only; goes with the save cut (R9) |
| Gatherable debris, `build.gather` (D193 amended) | Nothing; `debris` in `qa.ts` names the fragment log | Gone |
| Brick and the three removed materials (log of D290) | Comments that frame infill may later be brick nogging; `tools/kits/README.md` examples `river_cobble`, `cut_sandstone`, `plastered_timber`, and `art/textures/plaster.png`, `art/kits/shingles`, `art/kits/oak`, none of which exist; `pack.test.ts` fixtures by those names | README examples stale (R12) |
| Storey wall tiles (D277) | The quick tools' view only, now `shortcuts.rs` (`Base::walls`, `filling`, `run`, `Panel`, `BuildOp`), read by the hammer's wall tool, the yard, the examples and older saves | Live, now in one place |
| Pre-D290 kit format | `layers::resolve` gives back a file naming no pattern or pack as it is; patterns end the layering that way too, so it is not only a leftover | Live |
| Generated coverings replaced by modelled pieces (D258) | Script-made pieces still exist by design until modelled ones replace them role by role; `Timber.module` and `pieces/logs.ron` (177 KB) are kept off by D262's comparison | Live by decision |
| D209 roof sections | `save/older.rs` only | Goes with the save cut (R9) |
| Per-kit special cases | None in code; the test in `kit.rs` fails on any name or kind number | Gone |
| The 16.7 ms budget | One xtask test checks a run is judged by the budget it reports, using 60 Hz's figure | Fine |
| Door hinge-side settings | None | Gone |
| Carrying and storage (step 7, paused by D333) | `play/storage.rs` (526 lines), `play/storage_screen.rs` (263), `build::act`'s paying (`pay`, `has`, `costs_nothing`, `Stores`), `terra_build::cost` (398), the remote's `storage.*`, the cabin script's storage step | Paused, not dead: left alone |

## What remains, ranked by value against risk

Each says what to change, why, and what it waits on. Do them in this order once the agents in those files are done.

**R1. Split `surface::draw_slots` (1,302 lines in one function).** High value, medium risk; waits on the performance agent. It is seven stages sharing closures: the neighbourhood grid and what is cut or full (577 to 627), the shrunk boxes (628), inside edges and their fade (650 to 720), each cube's signature for the cache (721), where points fall along each axis (777 to 965), the face loop (966 to 1704, 740 lines), and smoothing normals (1705 on). Turn the first three into a `Neighbourhood` struct built once, the placement closures into methods on it, the face loop's body into `draw_face`, and smoothing into its own function. Hold it with `cargo xtask sections` (same triangles and timings) and the artifacts ratchet.

**R2. Split `sections.rs` (2,876 lines; `draw_blocks` 592, `draw_details` 278, `cluster_keys` 157, `ghost` 154).** Its loose ends done (b2fc4340: `plain()`'s ignored `laid`, `BaseMesh` a `Surface`); the split still waits on the performance agent, who was in `sections.rs` an hour after this note. High value, medium risk; waits on the performance agent. Along its concerns: keys and caches (`cluster_keys`, the signatures), laying a section, drawing near (`draw_blocks`, `draw_details`), far and baked colours (`fallback`, the sRGB table), ghosts and the plain kinds. In passing: `plain()` takes `laid` and ignores it (`let _ = laid`), and `BaseMesh::textured` is never filled since c08a50cc showed it, so `Base::mesh` can return a `Surface`.

**R3. Plans built three times in the game.** Done (18e04678): `plans::Footings::of`, `plans::rectangle`, `plans::ridge_along_x`, `plans::flight_top`, `Kits::roof_over`. High value, low risk for the pure moves; waits on nobody but touches `play/tool/aim.rs`, `build/remote.rs`, `build/going_up.rs`. `plans::Footings` is assembled five times by hand; the roof's footprint and "ridge along the longer side" three times; `kits.roof_for(k).unwrap_or(Kind::FIRST_ROOF)` eight times (game, `inspect`, `qa_cabin`, examples); "a ramp's top is a slope, else a stair" twice. Make `Kits::roof_over(kind) -> Kind`, `plans::rectangle_roof(min, max, along_x)`, and have `BuildRules` hand out the footings' sampler so `Footings` is built in one place.

**R4. Doors and shutters fitted three ways, and the hammer's differs.** Done (d450568e): one `plans::fit`, tested. High value, behaviour change. `aim.rs`, `remote.rs` and `build::props::fit_all` each fit a leaf into a hole; the remote and `fit_all` ask `plans::hang_for` whether the hole is a door's or a window's, the hammer does not, so it can hang shutters in a doorway. One `plans::fit`, with a test that the hammer refuses shutters in a doorway first. A fix, not a tidy: tell the developers.

**R5. Two answers for the ground under an unsampled column.** Done (D367): every column with a cube keeps its own sample, and `Ground::height_cm` takes the layout's answer. First tried without the first step, and not done then. With `Ground::height_cm` taking the layout's answer (its neighbours' mean), the property test `random_operations_keep_every_rule` fails at seed 3: a plan laid beside a column of cubes with no sample of its own changes that column's ground, and cubes on it float. What holds would then depend on samples beside it, which no operation's check looks at. The ground under a cube must be its own column's; the fix is to make sure every column with a cube keeps a sample (each plan and fill samples its own columns), after which the two answers differ only beside a building, where only dressing asks. Medium value, behaviour change. `Ground::height_cm` gives level ground (0) where no sample is, which `support`, `loss` and `ops` use; `layout::roles::ground_ticks` gives the mean of the neighbours' samples, which roles and dress use. A base keeps samples under its cubes, so the two agree wherever support asks; they part only for a cube whose column lost or never had its sample, which would be held up by level ground but dressed for its neighbours'. Put one fallback in `Ground` and measure faults and the rules before and after. The game samples the ground two ways too (`act::ground_under`, the column's middle; `build::ground_cm`, the lowest of its corners less the footing's depth): intended, but worth a comment where each is.

**R6. The Indexed-to-Surface flatten, copied.** Done (4b915bed): `Indexed::loose` and `Surface::add` keep wear, tested. Medium value, low risk; waits on the quoins agent (`dress.rs`). `modules::put` and `dress.rs` (about line 298) flatten shared corners to loose triangles word for word, and both drop `wear`. One `Indexed::loose()` in `mesh.rs`, beside `Surface::indexed`, carrying wear; check whether dropping it was meant before keeping that.

**R7. Two ear-clippers.** Done (4b915bed): `mesh::ear_clip` for both, tested on either winding and points in a line. Medium value, small behaviour change; waits on the quoins agent. `modules::ear_clip` copes with either winding and collinear points and falls back to a fan; `dress::ears` stops when it finds no ear and leaves a hole. Use the first in both; run the artifacts ratchet.

**R8. `KitColours.dark` and `roof`: dead data.** Done 2026-10-15 (D349): the fields gone, the roof kits' timber and the block and frame kinds' daub colour taken out, and the kit checker refusing any field a pattern never reads. Medium value, low risk; waits on the material-checks agent (`checker.rs`, `pack.ts`). After c08a50cc nothing reads them but the checker's range check. Remove them from the struct and its default, `patterns/block.ron`, `coursed.ron`, `frame.ron`, `packs/packed_earth.ron`, `limewash.ron`, `pack.ts`'s writer (line 111) and the checker; fix `block.ron`'s header ("its plain colours far off"). Likewise dead in data: `timber` on the roof pattern and the shakes, slate and thatch packs (roof kits never lay timber; rafters are modules), and `chinking` on block and frame kits (only log runs read it).

**R9. Cut old saves.** Done (D353): the developers said "we dont care about old saves"; every older format is refused and its readers deleted.

**R10. One hash for a place.** Low value, low risk. The same 32-bit mixer is written out in `loss.rs` (`unit`), `timber.rs` (`break_up`), `cells.rs` and `terra_game::build::arrays::damage`, a variant in `build::loss`'s dust; and there are two "hash to 0..1" conventions (`layout::unit`, `fragments::unit`). One `layout::unit3(seed, a, b, c)`; check the outputs match bit for bit before swapping each.

**R11. Big game-side functions.** Medium value, medium risk. `play/tool/aim.rs::aim` (481), `act.rs::act` (474), `tool/show.rs::highlight` (338), `build/layout.rs::launch` (379), `build/draw.rs::show` (336) and `upload` (225), `build/loss.rs::step` (249). Split `aim` and `act` by tool (wall, room, roof, flight, block, fit, replace, carve, take out), each its own function in `tool/`; `launch` by stage. `draw.rs` waits on the performance agent.

**R12. Tools baggage.** Low value, low risk; waits on the QA and material-checks agents. `tools/kits/README.md`'s examples name the removed materials and art that does not exist; `qa/harm.ts` says "as in build.ts", which is gone; `qa/film.ts` repeats `qa/lib/cabin.ts`'s places and overlaps the build clips; the sandbox's argument list is copied in four QA scripts; `qa.ts`'s `--first` and `--answers` are undocumented; `build.ground` is missing from the sandbox README's list; `--walls logs` does nothing (only `plain` is read). `cargo xtask` lives in one 2,180-line `main.rs`: move the building tasks (kits, rules, sections, artifacts, pack, qa, contract) into `xtask/src/build.rs`.

**R13. Demo scenes in the shared crate, and the kit sheet in the game.** Low value, low risk. `terra_build` holds `qa_cabin`, `castle`, `example::bench_village` and `town::svg`, scenes and a debug picture; they are Bevy-free so tests and tools run them headless, which is right, but they belong together (`terra_build::scenes`). The other way round, `build/sheet.rs` (1,223 lines) mentions Bevy twice: its rows are shortcut scenes like `example.rs` and could move beside them, the game keeping only their placing. `qa_cabin`'s ground filling copies `build::act::with_ground` and should be one `plans::with_ground`.

## Tests

No test was found pinning a tuned number: the large literals asserted are outcomes (a part limit, a cube count, a texture size), and the fixed versions held by the layout's test are its determinism, which is the point. Two test-only helpers sit in live code (`save::snapshot_of`, `Base::same_cubes`); they are small and may stay. Not audited: whether the property tests in `ops/tests.rs` and the rules in `tests/rules.rs` overlap.

## Limits

Unused dependencies were not checked (no nightly `udeps`). Public methods whose names are shared with others elsewhere (`new`, `fill`, `kind`) could hide an unused one from the name count; a pass with every item made crate-private and the compiler's dead-code lint would find them, and is worth doing when no one else is in `terra_build`.
