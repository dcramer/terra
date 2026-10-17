# Building's code health

2026-10-15, with a second pass on 2026-10-16 (below). Question: after many changes to the building system, does it carry baggage (code, data, options and tools nothing uses, leftovers of what was removed or replaced), the same logic in two places, modules grown into monoliths, or rules on the wrong side of a layer? The developers: "make sure we've not overloaded a bunch of baggage too. weve had lots of changes in the system so we want to keep the code well organized".

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

**R1. Split `surface::draw_slots` (1,302 lines in one function).** Done (7e7dbdc1): `surface/slots.rs`, its stages `Neighbourhood`, `Spacing`, `draw_face`, `smooth_normals` and `put`; `draw_face`, one face, split along its stages too (b253fd27): `as_before`, `Lines`, `gather_round` and a `Drafting` whose methods place each point, hole it and cut the face into triangles. It is seven stages sharing closures: the neighbourhood grid and what is cut or full (577 to 627), the shrunk boxes (628), inside edges and their fade (650 to 720), each cube's signature for the cache (721), where points fall along each axis (777 to 965), the face loop (966 to 1704, 740 lines), and smoothing normals (1705 on). Turn the first three into a `Neighbourhood` struct built once, the placement closures into methods on it, the face loop's body into `draw_face`, and smoothing into its own function. Hold it with `cargo xtask sections` (same triangles and timings) and the artifacts ratchet.

**R2. Split `sections.rs` (2,876 lines; `draw_blocks` 592, `draw_details` 278, `cluster_keys` 157, `ghost` 154).** Done (f5218d95): nine modules under `sections/`, `sections.rs` 588 lines; `draw_blocks` split along its stages (b253fd27): `Sorted`, `Wanted`, `Cutting`, `Beds`, `Holes`, then `Blocks::draw_set` and `draw_cores`. Its loose ends done (b2fc4340: `plain()`'s ignored `laid`, `BaseMesh` a `Surface`). Along its concerns: keys and caches (`cluster_keys`, the signatures), laying a section, drawing near (`draw_blocks`, `draw_details`), far and baked colours (`fallback`, the sRGB table), ghosts and the plain kinds. In passing: `plain()` takes `laid` and ignores it (`let _ = laid`), and `BaseMesh::textured` is never filled since c08a50cc showed it, so `Base::mesh` can return a `Surface`.

**R3. Plans built three times in the game.** Done (18e04678): `plans::Footings::of`, `plans::rectangle`, `plans::ridge_along_x`, `plans::flight_top`, `Kits::roof_over`. High value, low risk for the pure moves; waits on nobody but touches `play/tool/aim.rs`, `build/remote.rs`, `build/going_up.rs`. `plans::Footings` is assembled five times by hand; the roof's footprint and "ridge along the longer side" three times; `kits.roof_for(k).unwrap_or(Kind::FIRST_ROOF)` eight times (game, `inspect`, `qa_cabin`, examples); "a ramp's top is a slope, else a stair" twice. Make `Kits::roof_over(kind) -> Kind`, `plans::rectangle_roof(min, max, along_x)`, and have `BuildRules` hand out the footings' sampler so `Footings` is built in one place.

**R4. Doors and shutters fitted three ways, and the hammer's differs.** Done (d450568e): one `plans::fit`, tested. High value, behaviour change. `aim.rs`, `remote.rs` and `build::props::fit_all` each fit a leaf into a hole; the remote and `fit_all` ask `plans::hang_for` whether the hole is a door's or a window's, the hammer does not, so it can hang shutters in a doorway. One `plans::fit`, with a test that the hammer refuses shutters in a doorway first. A fix, not a tidy: tell the developers.

**R5. Two answers for the ground under an unsampled column.** Done (D367): every column with a cube keeps its own sample, and `Ground::height_cm` takes the layout's answer. First tried without the first step, and not done then. With `Ground::height_cm` taking the layout's answer (its neighbours' mean), the property test `random_operations_keep_every_rule` fails at seed 3: a plan laid beside a column of cubes with no sample of its own changes that column's ground, and cubes on it float. What holds would then depend on samples beside it, which no operation's check looks at. The ground under a cube must be its own column's; the fix is to make sure every column with a cube keeps a sample (each plan and fill samples its own columns), after which the two answers differ only beside a building, where only dressing asks. Medium value, behaviour change. `Ground::height_cm` gives level ground (0) where no sample is, which `support`, `loss` and `ops` use; `layout::roles::ground_ticks` gives the mean of the neighbours' samples, which roles and dress use. A base keeps samples under its cubes, so the two agree wherever support asks; they part only for a cube whose column lost or never had its sample, which would be held up by level ground but dressed for its neighbours'. Put one fallback in `Ground` and measure faults and the rules before and after. The game samples the ground two ways too (`act::ground_under`, the column's middle; `build::ground_cm`, the lowest of its corners less the footing's depth): intended, but worth a comment where each is.

**R6. The Indexed-to-Surface flatten, copied.** Done (4b915bed): `Indexed::loose` and `Surface::add` keep wear, tested. Medium value, low risk; waits on the quoins agent (`dress.rs`). `modules::put` and `dress.rs` (about line 298) flatten shared corners to loose triangles word for word, and both drop `wear`. One `Indexed::loose()` in `mesh.rs`, beside `Surface::indexed`, carrying wear; check whether dropping it was meant before keeping that.

**R7. Two ear-clippers.** Done (4b915bed): `mesh::ear_clip` for both, tested on either winding and points in a line. Medium value, small behaviour change; waits on the quoins agent. `modules::ear_clip` copes with either winding and collinear points and falls back to a fan; `dress::ears` stops when it finds no ear and leaves a hole. Use the first in both; run the artifacts ratchet.

**R8. `KitColours.dark` and `roof`: dead data.** Done 2026-10-15 (D349): the fields gone, the roof kits' timber and the block and frame kinds' daub colour taken out, and the kit checker refusing any field a pattern never reads. Medium value, low risk; waits on the material-checks agent (`checker.rs`, `pack.ts`). After c08a50cc nothing reads them but the checker's range check. Remove them from the struct and its default, `patterns/block.ron`, `coursed.ron`, `frame.ron`, `packs/packed_earth.ron`, `limewash.ron`, `pack.ts`'s writer (line 111) and the checker; fix `block.ron`'s header ("its plain colours far off"). Likewise dead in data: `timber` on the roof pattern and the shakes, slate and thatch packs (roof kits never lay timber; rafters are modules), and `chinking` on block and frame kits (only log runs read it).

**R9. Cut old saves.** Done (D353): the developers said "we dont care about old saves"; every older format is refused and its readers deleted.

**R10. One hash for a place.** Done (1febbc0d): `layout::mix3` and `unit3`, used by loss's shading, broken timber, the cell maps, the damage mask and the packs' test, each with its inputs as before, so bit for bit the same. Fire's `luck` and the game's dust mix with other multipliers, and `fragments::unit` takes the low 16 bits of `layout::hash` where `layout::unit` takes the high 24: making them one would change what they give, so they stay, each said where it is. Since D396 every copy of a mixer's finaliser in the workspace, these two included, calls `terra_gen::hash` (`prospector32_round`, `prospector32`, `lowbias32`, `lowbias32_round`, `splitmix64`) with its inputs as before; the dust's puffs and an animal's `luck`, which were variants of a mixer rather than copies, land differently.

**R11. Big game-side functions.** Done but for the performance agent's: `act` (486 lines) to a loop over its steps and `aim` (482) to a `Sight` worked out once and a function a tool (167bdeb7); `highlight` (338) to its look, its film's quads, the cubes touched and the snapping grid (ec709d7a); fragments' `step` (249) to warming up, the fall, keeping within the most and drawing (9774579d). `build/layout.rs::launch` (385) and `build/draw.rs::show` and `upload` wait on the performance agent, still in them.

**R12. Tools baggage.** Done (3269e65a, 9187dfb0): `xtask/src/main.rs` (2,469 lines) split by task family: the task table and how a task runs stay; `bench`, its `scenes`, `check`, `build` (kits, contract, rules, sections, artifacts, texels), `windows` and `doctor` are modules. The QA scenarios take the cabin's places and the clips' sandbox options from `qa/lib/cabin.ts`; `harm.ts` no longer points at `build.ts`; the pack README's examples name art that exists; `build.ground` is in the sandbox README. `qa.ts`'s `--first` and `--answers` were documented since. `--walls logs` is kept: it names the default.

**R13. Demo scenes in the shared crate, and the kit sheet in the game.** Done (09c31835, 2e79aa86): `terra_build::scenes` holds the castle, the burg, the QA cabin, the example bases and the kit sheet's rows (about 1,000 lines out of the game, which keeps their placing); `plans::with_ground` is the hammer's and the QA cabin's one ground sampling; `town::svg` is `town/svg.rs`, with the town split by what it lays. `junctions` and `roof_walls` belong in `scenes` too; they move once the faults agent is out of `inspect.rs` and `layout/runs.rs`, which name them.

## The second pass (2026-10-16)

After the fifth goals review's finding 5 (the code had grown back into large files where D351 did not reach) and finding 7 (shared pieces a new material could look wrong in). Every split moves code as it was; each was checked by a throwaway hash of everything drawn for all 672 inspected bases (identical before and after), the town's by forty seeds' plans, the kit sheet's by its log of rows and vantages.

| Commit | What |
|---|---|
| 9c9d7bef | `layout/roofs.rs` (2,940 lines) split by concern: the surface and `lay` in `roofs/mod.rs`, and `lines`, `frame`, `covering`, `flashing`, `standing` (what stands on a roof) and its tests; `roofs::lay` from 364 lines to its stages |
| 2ce4e296 | The roof pattern lays its own frame (D290): the rafter and batten move from shakes' own pieces to the pattern's (`art/kits/roof`, the same meshes), its bark is the pattern's `Log`, and a ridge of two lapped boards caps a covering with none of its own. `cargo xtask pack <name> --pattern roof --under none` now passes the kit checker (it failed on rafters, battens, ridge, Log, Piece and relief). Slate's hewn oak and thatch's poles stay their own overrides |
| da60ce1d | A block material's copings, sills, lintels and plinths wear its own wall's texture set, as its quoins do, unless its pack gives a `Dressed`; the pattern's `rock_boulder_dry` is the fallback (D215 amended). The one change of look |
| 5580374f | `dress.rs` (1,937) to `dress/`: profiles, corner stones, masks and tests; the profile sweep's 236-line `draw` cut into its ends' `Cut`, its faces and its closed ends |
| de37ad9e | `timber.rs` (2,273) to `timber/`: cuts, harm, the two shapes, planks, the sweep and tests; a round log's knots, rings, shade, tone and wear each a function |
| 587cdc9e | `frames::face` (436 lines) to a `Face` worked out once and its posts, plates, rails and gable braces |
| 2e79aa86 | `town.rs` (2,034) to `town/`: geometry, the square and streets, plots and yards, the path graph and the SVG plan |
| 07f99dee, 6804198c | The kit sheet's `lay` (322 lines) and its `rows` (206) split by what they lay |

**Waiting, as the other agents are in these files:** `inspect.rs` (2,545 lines; the faults agent); `layout/runs.rs` (`under_roofs` 398, `eave_blocking` 271, `courses` 187), `floors::lay` (236) and `dressing::shaped` (269), the faults agent's uncommitted work today; `build/draw.rs` (1,772: `show` 338, `upload` 228), `build/layout.rs::launch` (385), `sections/details::draw_details` (281) and `surface/slots.rs::face` (265), the performance agent's. Older long functions not grown since the first pass (`plan_work` 263, `prompt` 228, `town::paths` 183) are left.

**Found on the way:** a roof made with `--under none` at 1.5 m and phase 0 has 18 inward-wound covering triangles on hips and valleys and a verge against a wall, the same over shakes, so it is the covering's numbers, not the new frame; the baseline's coverings have none. The kit sheet laying many rows segfaults under WSL's CPU renderer before and after the split alike. `cargo xtask rules` at 6804198c: the rules tests, 64 seeds of random operations and the live rules hold; the drawing audit fails on coplanar pairs (stone's `Dressed` 62 pairs, the largest 1,029 cm² in the farmstead; hewn's floors and sawn ends), the same with the dressed-stone change turned off, so it is what 9e0970a4 now counts (the fifth review's finding 6), not this pass.

## Gates (2026-10-16, D396)

What this note and the reviews found by hand is now found at every commit, by stages of `cargo xtask check --quick`:

- **Sizes.** No Rust file over 1,000 lines, no function over 150, but the 129 that were over when it began (`xtask/sizes-baseline.tsv`, `cargo xtask sizes`), each of which may shrink and never grow. The "Waiting" list above is in it at its sizes today.
- **One definition.** The hash mixers written out in 15 files are `terra_gen::hash`, the lowest crate every user reaches; their constants anywhere else fail the check (`PRIMITIVES` in `xtask/src/primitives.rs`, where a new shared primitive is a row).
- **Drift.** The docs check fails on a `cargo xtask` task the guides name that is not one, and on a path a skill names that is not there. It found none of the first, and in the models skill a village piece's folder (`assets/models/village`, now props) and two paths not made yet, reworded.

## Tests

No test was found pinning a tuned number: the large literals asserted are outcomes (a part limit, a cube count, a texture size), and the fixed versions held by the layout's test are its determinism, which is the point. Two test-only helpers sit in live code (`save::snapshot_of`, `Base::same_cubes`); they are small and may stay. Not audited: whether the property tests in `ops/tests.rs` and the rules in `tests/rules.rs` overlap.

## Limits

Unused dependencies were not checked (no nightly `udeps`). Public methods whose names are shared with others elsewhere (`new`, `fill`, `kind`) could hide an unused one from the name count; a pass with every item made crate-private and the compiler's dead-code lint would find them, and is worth doing when no one else is in `terra_build`.
