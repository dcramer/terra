# Building audit: rules, physics, drawing, cost and looks

2026-10-13. The developers asked to "make sure we're still on track for making the voxels work correctly and efficiently, and look good. and all of our physics rules/etc are working". Does every rule of construction-001, D192 to D194 and the Building decisions since (to D241, roofs as cubes) hold in the game as built, and does building meet [building-plan.md](../docs/building-plan.md)'s bars and D211's? Checked by state, not only by picture, with a tool that does it again (D257).

## How to run it again

`cargo xtask rules` runs all of it and says ok or FAIL for each part; `--pure` leaves out the live run, `--here` runs it on this machine, `--debug` a debug build.

| Part | What | Where |
|---|---|---|
| Rules tests | Each rule below that the operations' own tests did not already walk through; a known failure is `#[ignore]`d with its reason and run here with `--include-ignored` | `crates/terra_build/tests/rules.rs` |
| Random operations | `random_operations_keep_every_rule` over 64 seeds (`TERRA_SEEDS=64`): after every step nothing floats, nothing is in over a cube partly in, every prop in a hole, refunds exact, undo and redo exact, laid twice the same, snapshots and replays the same | `crates/terra_build/src/ops/tests.rs` |
| Drawing audit | One of each sort of village building (D229), in all nine kits with their dress: every section drawn twice at near, middle and far; coplanar overlaps that would z-fight; one cube out of a wall; an operation's cost | `crates/terra_build/examples/drawing.rs` |
| Live | The rules over the remote in an optimised build on the Windows GPU, a roof built and pictured half up and done, timings | `tools/sandbox/rules.ts`, pictures in `shots/rules-*.png` |

Also run for this audit: `cargo xtask check`, `cabin`, `kits`, `sections`, `demo`, `lodcheck`, `pops` and `bench village village-bare` on both GPUs. The machine was shared with other agents' builds (load 30 to 70 on 32 cores), so every frame time below is slower and less even than on a quiet machine; GPU times and counts are not.

## Structure

| Rule | Result | Evidence |
|---|---|---|
| Nothing is built in the air | Pass | Live `in-the-air`; `nothing_is_built_in_the_air…` |
| Nothing off an overhang into nothing; a cantilever joined to the ground is allowed (D142) | Pass | Live `overhang`; the same test |
| Nothing off a post with nothing under it | Pass | Live `unbuilt-post` (its top and a deck beside it refused) |
| Nothing taken from under a cube | Pass | Live `rests-on`; `nothing_is_taken_from_under_a_cube_an_eave_an_item_or_a_prop` |
| … under a roof's eave | Pass | Live `roof` and cabin `refused`; the same test |
| … under an item | Pass in the model; the game maps an item to the cube under its foot | The same test; `build::act::Building::around` |
| … under a prop (a door in its hole) | Pass | The same test; props are out for now (D225) |
| Roofs need every eave cube built | Superseded: since D241 a roof's cubes hold as any cube does, so an eave over a missing wall cube hangs from the eave beside it, as a lintel would; with no wall under it at all it is refused | `a_roof_cube_over_a_missing_wall_cube_hangs…` |
| Carving keeps everything joined | Pass | Live `carve-and-replace`; `carving_and_replacing_in_place_never_leave_anything_unjoined`; `a_cut_is_refused_where_it_would_leave_the_wall_over_it_hanging_or_a_roof_unheld` |
| Brushes and carve boxes obey the same rules | Pass: a brush only names cubes for `LayPlan`, `Carve` and `Empty`, which are checked as any | Live `rests-on` (a take-out), `carve-and-replace` (a carve box) |
| Replacing in place cannot undermine | Pass | Live `carve-and-replace`; the same test |
| Footings go in first and reach the ground | Pass | Cabin `footing` (45 cubes from 2 down, before anything else); D206's tests |
| Undo and redo are exact | Pass | Random operations, 64 seeds; cabin `footing` |
| Refunds are exact | Pass | Random operations; cabin `roof` (36 Wood and half a Wood over, exactly) |
| The plan budget per claim holds | **Fail** | `the_plan_budget_holds…` (ignored): taking apart leaves its cubes planned, as ghosts to build again, and nothing counts them against the budget, so a claim of 1,500 can be built out and taken apart to any number of ghosts |
| Claims don't overlap | **Fail, not enforced** | `play::claim`: one claim a player and nothing checks a stone against another claim or a village's buildings. Single player today, so two claims cannot meet; but a claim can be set in the village, and then walking uses whichever build is within 3 m (`play::movement`), so the player's own walls stop nobody beside a village house |
| A section can be built by hand a cube at a time ([goals review](goals-review-2026-10-13.md) 3) | **Fail** | `a_section_can_be_built_by_hand_a_cube_at_a_time` (ignored): the 256th block laid alone in a section is refused, "Too many parts meet here", as each is its own part and a section's palette holds 255 |
| Rooms are what the cubes close in (review 5) | **Fail** | `a_room_is_what_its_cubes_close_in…` (ignored): a ring of walls from the storey's layer under a roof is indoors; the same ring begun three layers up is not, as rooms are found per storey of six cubes |
| Everything a walker stands on is a cube (review 4) | **Fail** | `everything_a_walker_stands_on_is_a_cube` (ignored): a doorway's steps are `layout::ground::flights`, slabs outside the cubes from the planet's ground at run time |
| A plan outside the claim is refused | Pass | Live `outside-claim`; `a_claim_bounds_what_is_planned…` |
| The ground outside a footprint is never changed | Pass | Cabin `ground` (40 columns round the cabin unchanged); a room's cut stops at its walls' outer faces (`Site::cut_for_room`); `Op::Ground` keeps samples only where something stands |
| The ground is not dug from under a build (D193) | **Was failing, fixed** | The pickaxe dug anywhere (`play::dig`); now refused within a scoop of any column a build stands on or has planned (`the_ground_under_what_is_built_is_not_dug`) |
| Collapse into debris (D193, step 6) | Not built, and nothing can cause lost support | Everything that changes a base goes through `Base::act`'s checks (`build::act`); the shortcuts' unchecked `BuildOp`s run only on the yard (`build::edit`) and in the village's generation; `Op::Ground` and a room's cut refuse to undermine; and the pickaxe, the one thing that changed the ground under a build unchecked, is now refused there. Old saves are converted (tested not to float, `save` tests) but not checked as they load, so a save edited by hand or made under older rules could hold floating cubes, which nothing removes |

## Physics and collision

| Rule | Result | Evidence |
|---|---|---|
| Built cubes collide; plans are walked through | Pass | `walk` tests; cabin `in` |
| Stairs step, slopes ramp, halves are half height | Pass | `a_half_cube_is_stepped_onto…`, `a_stair_holds_a_walker…`, `a_flight_of_stairs_or_a_ramp_is_walked_up_a_storey` |
| Inverted shapes and corner slopes block correctly | **Was failing, fixed** | An inverted slope whose underside is over a head at its edge but comes down to the chest across it was walked straight through: `walk` only tested a column at its edge. It now sweeps the move and stops where the body would meet the underside (`shapes_overhead_stop_a_walker…`, every overhead shape at every height; live `head-height`, stopped 0.27 m in) |
| Posts collide at their width | Pass | Live `post`: stopped 0.48 m from its middle (0.18 + 0.30) |
| Trims that stand out never collide | Pass by construction: a walker meets cubes, never pieces | `trims_standing_out_of_a_wall_never_stop_a_walker_short_of_its_cubes` |
| Nothing is built where the player stands | Pass | Live `where-you-stand` ("Something is in the way"); `building_where_someone_stands…` |
| A door is not shut on someone | **Was failing, fixed** | `Op::SetOpen` did not ask who stood in it (`building_where_someone_stands_or_shutting_a_door_on_them_is_refused`) |
| Nothing is built through an item set down (gap 17) | **Was failing, fixed** | Only bodies were in the way; a workbench or the claim stone is now too (live `item-in-the-way`) |
| Stairs reach a second storey and the player walks onto it | Pass | Cabin `upstairs` (walked up to 3.00 m) |
| Doorways are walkable | Pass | Cabin `in` |
| Roof slopes are stood on and walked under (D241) | Pass | `a_roof_of_cubes_is_stood_on_and_its_underside_is_the_ceiling_under_it` |

## Drawing

| Rule | Result | Evidence |
|---|---|---|
| Every state draws: planned, the next lit, part built, built, carved | Pass | `a_plan_is_drawn_as_ghosts_a_piece_each_and_the_next_lit`; pictures: cabin `plan`, `going-up`, `cut`; `rules-roof-half` (slate on, hip rolls still ghosts) |
| Each piece in one section; sections merged are the base laid whole, bit for bit (no seams, gaps or doubles at section edges) | Pass, now for every kit with its dress | `sections::tests` widened by this audit to all nine kits with their dressed pieces, earth under a hip and wattle and daub under a gable each across a corner of four sections, and an L of stone under hips with its valley across a section's edge |
| Laying the same thing twice gives identical meshes | Pass | Drawing audit: 132 sections, each drawn twice at near, middle and far, the same |
| No z-fighting where kinds meet | **Open: many coplanar overlaps to look at** | Drawing audit, below |

Coplanar overlaps facing the same way, of over a square centimetre, in the near meshes of one of each sort of village building (with flat height maps, so a block surface's real displacement may part some of them). The most and the largest:

| What meets | Pairs | Largest |
|---|---|---|
| stone dressing / stone dressing | 6,196 | 436 cm² |
| stone dressing / stone surface | 9,614 | 13 cm² |
| log daub / log ends | 3,748 | 15 cm² |
| wattle frame / wattle infill; wattle ends / infill | 2,087; 2,132 | 13 cm² |
| hewn daub / log ends | 2,142 | 12 cm² |
| earth's oak dressing / earth | 1,319 | 84 cm² |
| roof boards / roof boards (shakes, slate, thatch) | 460, 597, 337 | 243, 205, 95 cm² |
| log boards / log ends | 1,208 | 58 cm² |

Small overlaps where a member's sawn end lies in the plane of the daub or infill beside it are hidden or flicker only at a grazing angle; the hundreds of square centimetres where two dress profiles or two roof boards lie in one plane are real z-fighting where both show. `cargo run --release -p terra_build --example drawing` names the building, section and place of the largest of each. Not fixed here: the dress and roofs are other agents' work in progress.

## Efficiency

| Bar | Result | Evidence |
|---|---|---|
| One edit lays and merges only what it must | Pass for merging, broad for laying | Drawing audit: one cube out of a wall sets every section of its building waiting (8 to 12), and 1 or 2 are merged again; no section outside those changed. Live: 4 to 14 set waiting, 1 or 2 merged anew |
| A section laid and merged under 2 ms on a worker (median) | Pass on the median, far off on the tail | `cargo xtask sections`: median 1.28 ms near, 0.99 middle, 0.68 far; but the 90th percentile 81 ms near, and a building's middle section 146 to 172 ms near (under load) |
| An operation applied and checked under 0.1 ms | **Fail, narrowly** | Drawing audit: a wall's top out and in, median 0.15 ms, slowest 0.38 ms a building (under load) |
| A single cube's edit, from the operation to its mesh on screen | 22 to 33 ms on a small base, 110 to 150 ms on a base with a roofed room | Live `edit-on-screen` (`Build: change on screen`): the whole building is laid again and its big sections merged on the workers |
| No frame above the yard's for the same view; the worst frame while building | **Fail: 200 to 230 ms stalls while building** | Live `building-frames`: worst 208 and 231 ms, the game's own update. Timed system by system: `terra_world::flora::make_way` takes 114 to 215 ms on the main thread whenever a build's footprint changes, hiding every plant on it and growing again the grass squares beside it, synchronously |
| The worst frame walking through the village | 44 to 50 ms worst, under load | Live `village-frames`: median 22 ms, GPU 4 to 15 ms; the game's update is most of it on a busy machine |
| `village` against `village-bare`, RTX 5090 (high) | GPU 2.5 to 2.7 ms against 2.2 (+0.4 ms for fifty buildings) | `cargo xtask bench village village-bare`; frame medians 22 and 19 ms are the busy machine's, not the GPU's (D185's record before this was 4.7 and 4.6) |
| The same, the integrated Radeon (low, D239: 30 fps) | **Village misses**: 39.0 ms median (GPU 31.8) against bare 28.5 (GPU 23.4) | `WGPU_POWER_PREF=low … bench`: fifty buildings add about 8.5 ms of GPU, mostly the main pass (13.8 against 9.0 ms) and the prepass (3.7 against 1.3) |
| Memory per base | Not measured beyond the store | A section is 16 KB of cubes, and the live run's base, a roofed room and a few test pieces, is 14 sections; the meshes a base keeps on the GPU are not totalled anywhere yet, so this bar needs a counter first |

## Looks

From the cabin's, the roof's and the demo's pictures on the Windows GPU, against [art-style.md](../docs/art-style.md) and D211.

- **Slate, half built and done** (`rules-roof-half`, `rules-roof-half-close`, `rules-roof`, `rules-roof-close`): rows of real slate with offset joints and a few darker slates, a board at the eave, hips cut and capped with a round roll; half built, the slates on and the hip rolls still blue ghosts, which reads as a roof going up. The rows are even to the point of ruled: no course or slate is out of line, and every course's shadow line is the same weight, so far off the roof reads as a pattern.
- **The village** (`cargo xtask demo`, `shots/demo/sheet.jpg`): logs, stone, hewn timber, wattle and daub and the mixed schemes read as buildings of real material with plinths stepping down the slope, framed openings and finished roofs; nothing broken, no holes or seams seen at section edges.
- **Cob is flat** (`15-cob-house`): one even orange-brown over the whole wall, its lifts barely showing, so it reads closer to painted plaster than rammed earth; the rubble plinth and thatch round it are good. The earth kit is another agent's work in progress (D236).
- **Two of the demo's pictures are not of what they say**: `14-hewn-timber-house-under-a-hip` puts the camera inside a neighbour's slate ridge roll (its framing, not the building); and `20-going-up` showed the plan only, as the pack held twenty of the 120 Stone given, so only the footing went in. `demo.ts` now brings stone as it builds.
- **Every picture has the hammer's held part** (the claim stone's red ghost) where the camera looks, as in the cabin's, so pictures should be taken with another tool in hand, as `rules.ts` now does.

## Fixed in this audit

- `Base::walk`: a walker stopped where an underside overhead comes lower within a column (D257).
- `Op::SetOpen`: a door is not shut on someone in it.
- `build::act`: an item set down is in the way of a cube built through it.
- `play::dig`: no digging under a build.
- Tools: `cargo xtask rules`; `tests/rules.rs`; `examples/drawing.rs`; `tools/sandbox/rules.ts`; the `release` start option in `tools/sandbox/client.ts`; `Build: change on screen` in the log; `sandbox.frames`; `sections::tests` over every kit with its dress.

## Left, by how much it matters

1. **High: a 100 to 230 ms stall on every change to a build's footprint** (`flora::make_way` on the main thread). Every cube planned or built on new ground stalls the game for a fifth of a second. Hide the plants by the footprint's change only, and grow the grass squares on the workers as `cover` does.
2. **High on the integrated GPU: the village at 39 ms** against the low preset's 33 ms; the buildings' main pass and prepass.
3. **Medium: z-fighting** where dress profiles and roof boards lie in one plane (above).
4. **Medium: a big building's near section takes 80 to 170 ms to lay and merge**, so an edit to a roofed house shows a tenth of a second late.
5. **Medium: the plan budget** is broken by taking apart. Count a cube taken all the way out as planned only once it is laid again, or let taking apart a whole part take its plan away; a design call on what a taken-apart wall should leave (the developers').
6. **Medium, owned elsewhere: the goals review's defects 3 to 5** (the 255 parts a section, door steps outside the cubes, rooms per storey), each now a rule test that fails until it is closed; another agent is closing them.
7. **Low: claims are not kept apart**, from each other or from the village.
8. **Low: an operation takes 0.15 ms** against 0.1: rooms and columns are worked out again over the whole base after every operation (`Base::settle`).
9. **Low: old saves' floating cubes** stay, as nothing re-checks a save as it loads.
