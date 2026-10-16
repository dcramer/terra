# Building's workflow: what cost the most, and what to change

2026-10-16. Question: across the construction work of 2026-10-13 to 16 (materials, logs, stone, roofs, damage, faults, performance), where did time, tokens and rework go, and what in our abstractions and tools would make the same work easier, less error prone and faster?

## Where the cost went

| Cost | Seen as | Root |
|---|---|---|
| Looks took 3–6 rounds each (logs, damage, slate, earth) | "still looks bad" after each pass | Judged by eye on ad hoc stills; no numbers or reference beside them until late; causes (1K arrays, BC1, sRGB mips, TAA blur, a stretched bake, inverted heights, a pink grade) found only when an agent finally measured |
| Faults came back after being fixed (42 after zero) | regressions found by a review, not a gate | Each check covered the cases someone thought to add; new scenes (QA cabin, village, junctions) were not checked until listed by hand |
| The same diagnosis repeated | "is it load?" reruns; "is it the shader or the mesh?" | No in-game debug views (texel density, roles, detail level, damage mask, occlusion); no load-aware measuring |
| Long briefs and reports | 1–3k tokens per brief, restating scope, routine, checks, paths | No shared playbook for building work; each brief re-explained D333, the Routine, the checks and where things live |
| Overhead per agent | 10–20 min builds, full checks per commit, log conflicts, disk filling | Fixed by D183/D259/D375/D376 (seed, quick/full, decisions log) |
| One fact in several places | face depth, ground height, roof/wall junctions, piece sizes | Rules spread over roles, layout, dress, roofing and the checker; the voxel contract's matrix is prose, linked to code by hand |

## What to change, best value first

1. **Debug views in the game** (industry standard: every engine ships them). One key cycles the building's views: texel density, roles (each edge coloured by its role), detail level and section/part, damage mask, occlusion and shade factors, normals. Most look and fault questions this week would have been answered by one look instead of a round of measuring.
2. **A material bench** (`cargo xtask look <material>`): one command that shoots the fixed vantages (1, 2, 5, 12 m; noon, evening, shade) beside the material's reference photo and a grey card, and prints the numbers the agents measured by hand: texel density, fine-detail sharpness, grey-card balance, value range, stretch. Targets per pattern in data, so a regression fails, not a review. Builds on the kit sheet, `preview.ts`, `texels` and the colour cubes.
3. **Every scene checked by construction**: the mesh checker and the rules audit take their cases from one registry (`terra_build::scenes`: kit sheet rows, QA cabin, village, junctions, roofs and walls, castle) instead of lists, so a scene added anywhere is gated; plus a fuzz pass that builds random small bases from the shapes and kinds and runs the checker, catching junction gaps before anyone builds them.
4. **The voxel contract as data**: each junction-matrix row names its rule and its test in a table the docs check and the test harness both read, so a row without a passing test fails the check.
5. **A building playbook skill** (`.agents/skills/building/`): scope (D333), the Routine, the checks to run for each kind of change, where each concern lives, how to measure (quiet machine, repeats), what to report. Briefs shrink to the task itself.
6. **Load-aware measuring**: `bench` and `qa` traces wait for a quiet machine (or note it), repeat, and report the median with its spread, so no agent reruns by hand or argues with noise.

Items 1–6 are tooling and structure (technical, D375's spirit); none changes how the game looks or plays.
