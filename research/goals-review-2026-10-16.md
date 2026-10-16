# Building against the developers' goals: the fifth review

2026-10-16. Question: does what landed in building since the fourth review ([goals-review-2026-10-15b.md](goals-review-2026-10-15b.md), at 907f729) match what the developers asked for? The bar is D333: wood and stone, built and destroyed, with infinite resources, flawless and performant. D348 adds the voxel contract ([building-voxels.md](../docs/building-voxels.md)). Since the last review the developers have also asked for these:

- walls through and on roofs, done right ("we had gaps, because we didnt think this through");
- "a very clear design doc on how the voxels need to work so we can make sure there's no gaps";
- a building drawn from what is built, not from the blueprint;
- doors and shutters in frames, centred on the wall's real face;
- simple QA clips for each set;
- no old saves;
- materials simple and data only (D290, D349), with corner stones generated;
- textures with great detail;
- logs varied, realistic and tight, with no gaps;
- a neutral grade, close to real life;
- the code kept well organised;
- the work done faster (D375).

How it was done: I read the 170 commits from 907f729 to 9fe066d8 that touch `terra_build`, `terra_game/src/build`, `tools`, `xtask`, `docs`, `research` and the kits and packs. I read the code where a question needed it: the no-per-kit test, `ops`, `fitting`, `Base::scorch`, the packs and patterns, and every test the matrix names. All runs were fresh at 9fe066d8 on the RTX 5090, with the machine at 2 to 7% load:

- `cargo xtask artifacts` (the whole set);
- `cargo xtask rules`, `kits` and `contract`;
- `cargo xtask qa clips --set logs` and `--set stone`, 16 clips, each filmed and traced;
- `cargo xtask bench village town300`, three runs each.

I bisected the artifact faults by checking out older commits and running the checker on the failing bases. I looked at two contact sheets (build-openings in logs, build-storey in stone) and watched no clips. Ranks: **drift** (away from a goal, or a developers' call made by an agent), **defect** (meant to work, and does not), **polish**.

## The short answer

Most of what the developers asked for has landed, and it landed properly:

- walls on, through and beside roofs (D343, R1 to R7);
- the voxel contract, with all nine of its gaps closed;
- a building going up drawn from what is built (D358);
- doors and shutters with their own frames, on the wall's real face (D225);
- QA clips for each set;
- old saves refused (D353);
- corner stones cut from each material's own wall (D215);
- textures at 2,048 texels (D362);
- tight, weathered logs;
- a neutral grade (D370);
- the routine and the shared seed build (D375, D183).

Of the fourth review's findings, all were fixed or settled by the developers except its looks (finding 10, not judged here) and their showing (finding 13).

It is still not flawless, and the check that should say so does not run:

- **Wood and stone are no longer at zero faults.** The whole mesh checker finds 42 faults: sky through the footings of the hewn cabin, a 15 cm gap under the roof where a roof falls to a hewn wall, and z-fighting. Two commits in this window caused them, ff3ecf06 and b5d08fc2.
- **Nothing gates the whole checker.** Commits and pushes check only the rectangles. So a junction the contract marks "Closed", a roof falling to a wall, has an open gap in hewn timber, which is exactly what the developers asked us to remove.
- **Performance is close but not there.** On the 5090, all 8 logs clips hold D25's 13.3 ms at the 99th percentile. Stone's storey clip (14.3 ms) and town300 (14.19 ms) miss it, and fills still make 20 to 24 ms frames on the main thread. Nothing has been measured on the 3080, which is the GPU D25 names, since 4e5dd18d.

Since the baseline was set, fire (D377) has been added to building's crate as an agent's Tech decision. The code has grown back into large files in the places D351 did not reach.

## Findings, ranked

| # | Rank | Finding | Smallest correction |
|---|---|---|---|
| 1 | Defect | **Wood and stone have faults again, and no gate sees them.** `cargo xtask artifacts` fails with 42 faults: 15 coplanar, 18 see-through and 9 under the roof, every one in logs or hewn. Bisected: **ff3ecf06** (D367, one ground under a column) lets sky through the hewn QA cabin's footing (cubes [0,−1,14] and [0,−1,15], at every detail, 12 faults). **b5d08fc2** (tight logs) opens a gap 15 cm over a hewn wall's top under the roof wherever a roof falls to the wall, under shakes and slate (9 faults); it also adds coplanar faults to the hewn hip roof (1 fault before, 3 after). The logs QA cabin has a coplanar fault of 31 cm² at 6.5 m. The baseline file, `artifacts-baseline.tsv`, still records zero. `cargo xtask check` ratchets only `--quick`, each kit's rectangle under a gable (D287, D37), so the QA cabin, the castle, the junctions family and the roofs-and-walls family (which "prove" the D343 rows of the matrix) are in no gate. The contract's "A roof falling to a wall" row says Closed, and in hewn it is not. | Fix the two regressions; the bases named above are the test cases. Put every baseline kit's QA cabin, junctions and roofs-and-walls bases into `--quick`, a few tens of seconds, so the pre-push check holds what the matrix cites. |
| 2 | Defect | **Not yet within D25 everywhere.** On the 5090, quiet, as p99 / worst in ms with the frames over budget: **logs** walls 10.4 / 14.0 (2 over); openings 10.8 / 21.4 (3); roof 10.6 / 12.7 (0); swap 13.0 / 13.5 (1); carve 10.7 / 14.0 (1); storey 13.1 / 20.3 (11); deck 12.8 / 24.4 (6); destroy 12.5 / 16.9 (4). **Stone** walls 10.8 / 13.8; openings 10.6 / 16.5; roof 10.7 / 13.8; swap 13.0 / 20.7 (5); carve 10.9 / 13.5; **storey 14.3 / 23.5 (23 over)**; deck 11.3 / 14.2; destroy 13.1 / 16.6 (6). No hitches and no torn changes in any clip. Each worst frame is a `build.fill` with the game's main thread at 13 to 19 ms and the GPU at 3.3. Edits reach the screen in a median of 23 to 37 ms, worst 76 ms; the first change of each clip takes 40 to 79 ms, and a swap 52 to 58 ms. Bench: village 4.69 / 6.70 / 9.31 (median, p99, worst) holds. **town300 6.70 / 14.19 / 25.26 MISSED**, with 1.5% of frames over, a main thread of 5.2 ms and a render world busy for 5.7 ms; each base is "textured in a median 172 ms" on a worker. In the live rules run, village-frames has p99 14.4 and worst 21.4, and the run still says "ok". The latest town300 on the 3080 is 11.31 / 40.18 (4e5dd18d), and no Radeon run has been made since 2d00544. | Profile a stone storey fill system by system, as a70d51f6 did for uploads, and take what the main thread does per fill off it. Record village and town300 on the 3080 and on the integrated GPU. Make the live rules run judge its frames against 13.3 ms. |
| 3 | Defect | **The voxel contract has fallen behind the code.** It names two tests that no longer exist under those names: `a_doors_frame_fills_its_hole_and_its_leaf_is_centred_in_the_frame` is now `a_frame_fills_its_hole_and_its_leaves_are_centred_in_it`, and `shutters_fill_their_opening_with_no_gap_at_any_edge` is now `shutters_are_centred_in_their_frame_and_seated_on_the_walls_face` (both in `fitting.rs`). Its §2 gives a door its own frame, but shutters now have one too (ee46ead7). Fire (ee8b4469) changed what a section lays (`Laid::scorched`) and draws (char on the texture arrays) without touching the page, though D348 asks for that in the same commit. Invariant 1 lists the cubes, the ground, the kits and the seed as all that is drawn from, but `Base::scorch` is a fifth input, and it is not saved, so a reloaded base draws differently. | Rename the two tests on the page and add shutters to §2. Add scorch to §1 and invariant 1, and either save it or say it is transient. Have the docs check fail on any test the page names that does not exist (D71). |
| 4 | Drift | **Fire in the building crate during the baseline hold, filed as an agent's Tech decision.** D377 adds a cellular fire, char and soot, and `terra_build::burg`, "a castle built to burn": 1,742 lines, among them `burns` overrides in the shakes and thatch packs. What burns, how fast ("a shake roof in seconds, a log wall in half a minute") and a castle made to burn are design under D122, and D333 says "No new materials or props unless asked". It was made in the other developer's session (Greg Pstrucha's commits, with the dragon), so it was probably asked for, but the row says Tech, By Agent, and quotes no one. The dragon (e1f87eea) has no decision row and cites D377, which is the fire. | Ask the developers. If it was asked for, mark D377's design half Design, By Developers, with their words, and give the dragon its own row. Otherwise mark it Open and hold it out of the baseline. |
| 5 | Drift | **"Keep the code well organised" holds only where D351 reached.** R1 to R9 of the code health note are done. R10 to R13 are open, and some have grown. The same 32-bit hash mixer is written out in 9 files (fire.rs adds another 6 multiplies). `act` (486 lines), `aim` (482), `launch` (385), `show` (338) and `highlight` (338) are unchanged. `xtask/src/main.rs` grew from 2,169 to 2,469 lines, and the building tasks are still not in a module of their own. A new scene, `burg.rs` (520 lines), sits at the crate's root beside `castle`, `qa_cabin`, `village` and `town` (2,034 lines). New monoliths since 907f729: `layout/roofs.rs` 1,873 to 2,940 lines (`roofs::lay` 364), `runs::under_roofs` 398, `frames::face` 436, `inspect.rs` 1,869 to 2,545, `dress.rs` 1,429 to 1,937, `timber.rs` 1,810 to 2,273, `build/draw.rs` 1,475 to 1,772. The code health note was last updated at 480748d8. | One pass: split `roofs.rs` by concern (frame, covering, what stands on a roof), make one `unit3` hash (R10), move the scenes into a `scenes` module (R13) and the building tasks into `xtask/src/build.rs` (R12). Bring the note up to date. |
| 6 | Defect | **Two junk measures disagree.** The drawing audit in `cargo xtask rules` reports coplanar pairs where the mesh checker finds none in the same kits: logs Daub/LogEnd 760 pairs, hewn Floor/stone Floor 698 (161 cm²), stone Dressed/Dressed 70 (334 cm², the farmstead). It passes whatever the count. Either it counts faces back to back that the checker rightly ignores, or the checker misses them. | Check one case by hand, the farmstead's 334 cm² Dressed pair, then retire the audit's count in favour of `inspect`, or fix the checker. |
| 7 | Polish | **Shared pieces a new material could look wrong in.** The block pattern's `Dressed` texture (`rock_boulder_dry`) is worn by every stone's copings, sills, lintels and plinth, so a dark stone gets pale trims. That is the same risk the developers named for quoins, and quoins are now cut from the material's own wall. A new wood made `--under pine_logs` wears pine's daub, sawn ends and modelled log. A new roof wears the shakes' rafters, battens and ridge, because those still sit in shakes' and slate's own pieces, not the roof pattern's (construction-002, point 4). Slate carries its own rafter and batten numbers, and hewn's pack dresses four roles itself, which D215 calls the exception, not the norm. No game code is customised per kit, and the test in `kit.rs` passes. | Make `Dressed` default to the material's own wall set, as quoins do. Move the rafter and batten into the roof pattern's shared pieces. |
| 8 | Polish | **Stale rows and leftovers.** D321 is still Leaning, although D350 (by the developers) settled its town and five decisions build on that town (D324, D330, D344, D346, D381). The `materials` QA scenario still steps through limewash, wattle and earth, which D333 freezes out of the QA sets. building-plan.md's step 2 has said "under way" since 2026-10-13. The fourth review's finding 13, the looks of D305 and D320 shown to the developers, has no answer recorded. | Mark D321 Decided for its town. Limit `materials` to the baseline's sets. Close step 2. Put the destroy clips per set in front of the developers and record what they say. |

## What the developers asked for, one by one

| Asked | Landed | Verdict |
|---|---|---|
| Walls through and on roofs, done right | 6ae59f68 (designed), 96a434a1, db2045f8 (D343, R1 to R7) | Built and tested. The hewn gap under a roof falling to a wall is finding 1. |
| A clear voxel design doc | 00cfb954, 52933c4b (D348) | Done: every matrix row names its proof and all nine gaps are closed. It has drifted from the code since (findings 1 and 3). |
| Drawn from what is built | 1e162b8c, 0e97f7a5 (D358) | Done, with tests and the frames check's *going up*. |
| Door and shutter frames, centred on the real face | 4015762d, ee46ead7, 866aa25d, 79c5852d (D225) | Done. `cargo xtask contract` passes for every door and shutter, and the openings sheet shows the frames. |
| Simple QA clips per set | f9b5186d (D308) | Done: 8 clips a set, each in 1 to 2 minutes, with a trace and an index. |
| No saves | 78a3f55f (D353) | Done; the old readers are deleted. |
| Materials simple and data only; corner stones generated | c3b756f5 (D349), cb1f986a (D215 amended), b02ced99 | Done. `pack` cleans up after itself when it fails, and the kit checker refuses fields a pattern never reads. The shared pieces left are finding 7. |
| Textures with great detail | f1ff58b0, 71ecc05c (D362) | Done: 2,048 texels and BC7, with texture arrays at 312 MiB. |
| Logs varied, realistic, tight | b5d08fc2 | Done; a test holds the joints to at most 2 cm. The same commit opened the hewn gap in finding 1. |
| A neutral grade | fbbd7074 (D370, by the developers), D373 | Done. |
| Code well organised | D351, 7e7dbdc1, f5218d95, 18e04678, 4b915bed, d450568e, ff3ecf06 | Half done (finding 5). |
| Faster work | 943f5a35, 5ba34928, 38c2d331 (D183, D37, D375) | Done. The quick check is also why the whole mesh checker went ungated (finding 1). |

## The fourth review's findings

| # | Then | Now |
|---|---|---|
| 1 | Baseline not written | **Fixed**: D333, by the developers; the settlement builds in logs, stone and hewn alone. |
| 2 | Not performant; 16.7 ms used everywhere | **Mostly fixed**: one 13.3 ms budget (D332), and logs within it. What is left is finding 2. |
| 3 | A breach could not be rebuilt | **Fixed** (50ba8771), with a test. |
| 4 | Junk the checker could not see | **Fixed** (24d86795, 52933c4b), then regressed out of the gate's sight (finding 1). |
| 5 | Plans left floating | **Fixed** (50ba8771), with a test. |
| 6 | Adding a material is one command only for stone | **Mostly fixed**: `--under` is the default and documented, and a failed run cleans up. The roof's shared pieces are finding 7. |
| 7 | Contract misses corner stones and stretches | **Fixed**: D303 amended, and corner stones generated (D215, by the developers). |
| 8 | Unwritten layers drawn | **Fixed**: a section waits for its texture sets to be resident. |
| 9 | Town on a Leaning design | **Settled in practice** by D350 (the developers); the row is stale (finding 8). |
| 10 | Looks | Slate and the logs much changed (D362, D370, D373, b5d08fc2); not judged here. |
| 11 | QA runs collide | **Fixed**: the copy is retried, and GPU slots are shared (D319). |
| 12 | Stale docs | **Fixed**. New ones are finding 3. |
| 13 | Looks chosen by an agent, not shown | **Open** (finding 8). |

## Answers to the questions asked

1. **Is the baseline flawless and performant?** No.
   - Defects: the 42 mesh faults (finding 1), the frames over budget (finding 2) and the two measures that disagree (finding 6).
   - What holds: every rule (18 rules tests, 64 seeds of random operations and the live rules all pass), every QA clip's stage with no refusal and no torn change, `kits` and `contract` clean, and the breach and floating-plan fixes.
   - Over budget against 13.3 ms: stone storey (p99 14.3), town300 (p99 14.19, worst 25.3), live village-frames (p99 14.4), and in every clip a few fill frames of 14 to 24 ms. Swap and destroy in stone sit at 13.0 to 13.1. The cost is the main thread; the GPU stays at 2.4 to 3.4 ms. All of this is on a 5090, so the 3080 that D25 names is likely worse.
2. **Does the voxel contract hold?** In design, yes: every row names a test or an inspected case, and no gap is open. In fact, not quite:
   - Two of the names are stale (finding 3).
   - The rows proved only by the whole mesh checker are in no gate, and one of them, a roof falling to a wall, is open in hewn. That breaks invariants 3 and 10 there.
   - Invariant 1 has an unlisted input, the scorch.
   - Invariants 2, 5, 7, 8 and 9 hold by their tests, the rules and the kit test. Invariant 4 holds by the kit checker. Invariant 6 holds except where finding 1's faults appear at far detail too.
3. **Is anything customised per kit?** Not in code: the test in `kit.rs` passes, and the scenes name materials only in tests. In data, a few materials carry what their pattern should (finding 7): shakes and slate carry their own rafters and battens, slate its own framing numbers, hewn four role overrides, and shakes and thatch their own `burns`. The asset contract (D303) holds: `cargo xtask contract` passes for every door, shutter and prop, and corner stones state their size. The shared pieces that could look wrong are the block pattern's `Dressed` trims, pine's daub and sawn ends under any new wood, and the shakes' rafters under any new roof.
4. **Drift and scope creep?**
   - In building's own work, little: almost every commit answers a developers' ask.
   - In building's crate: fire and the burg (finding 4), the time-lapse (D381, which sensibly reuses the hammer's own steps) and the town's growth. Those came from the other developer's track, on decisions recorded as the agent's.
   - The code's organisation has slipped where D351 did not reach (finding 5).
   - There is no drift from Enshrouded's approach: what is built, fire's harm (sent as `Op::Damage`) and collapse all read the cubes.
5. **Stale docs and decisions:** finding 3, and finding 8's D321, step 2, the `materials` scenario and the open finding 13. Calls that belong to the developers: D377's design half and the missing dragon row (finding 4), and the D305 and D320 looks (finding 8).

## What is on track, and should stay

- The cube as the truth: plans, what is built, damage, collapse, fire's harm, doors and frames are all worked out from the cubes.
- The rules: 18 rules tests, 64 seeds of random operations, the live rules, refusals that say why, and breaches rebuilt.
- QA clips per set, the same every run, each traced against 13.3 ms. In both sets every edit reaches the screen untorn, in a median of 23 to 37 ms.
- Building's own thread pool (9fe066d8) and near-only laying (a70d51f6), which brought the edits that queued behind the world back down.
- Materials as data checked when they are added: the pack checks, the kit checker, the no-per-kit test, and corner stones from each material's own wall.
- The routine, the shared seed and the quick check; this review ran on a seeded worktree with no dependency compiled.

## Limits

- Every number is from the RTX 5090. Nothing was run on the 3080 or the Radeon.
- The bisects were of the named bases only, not the whole set. The hewn hip roof's first coplanar fault, already there before b5d08fc2, was not traced further.
- The looks were judged on two contact sheets alone. The frames check (`--frames`) was not run.
