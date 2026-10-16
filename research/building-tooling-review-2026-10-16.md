# Building's new tools, reviewed against the goals

2026-10-16. Question: do the tools the [workflow retro](building-workflow-retro-2026-10-16.md) asked for (debug views D385, the material bench D386, the scene registry and fuzz pass D287, the contract as data D348, the building skill, load-aware bench and QA D185), as landed on origin/main from 5ef04358 to a26f5d83, work when an agent drives them, read at a glance, cost nothing in play, and hold D333's baseline without baggage?

## Run once each

| Tool | Result |
|---|---|
| `cargo xtask fuzz --seeds 20` | Works: 1.9 s for 20 seeds, 7 with faults, each shrunk (4 elements to 1, 6 to 1) and printed as a test to paste. Faults name kinds by number ("Logs 3"), not by file |
| `cargo xtask artifacts --quick --ratchet` | Holds: 189 scenes in 40 s (machine under load) |
| `cargo xtask artifacts --ratchet` | Holds: 388 scenes in 452 s; run by no gate |
| Contract as data | 61 rows, 52 closed, 9 gaps (10 to 18); gap 19 is prose only |
| Skill | Every path and command in it exists, but one (below) |

## Findings, worst first

1. **Half the registry is gated by nothing.** The push ratchets 189 scenes; the other 199 (every roof over every plan but the rectangle, every two kinds mixed, the states, previews, castles, the example, the sheet's acted rows) are held only by `artifacts --ratchet`, which no hook runs and the skill asks for only "for the scenes it touches". The drawing audit leaves out the sheet's rows too (gap 19). *Fix:* hold the Plans and Mixed families before a push (most of the baseline's look), and the rest in a scheduled or pre-release full ratchet; measure the cost quiet.
2. **Look targets are bands over two materials, so the better one can drift far.** Each pattern's targets are set a little under the *worse* of its materials: slate's lit band 0.13 to 0.45 against its measured 0.17 to 0.18 (2.5 times brighter passes); slate's sharpness at 2 m 26.9% held to 17; hewn timber's 37% held to 23. They catch the big faults (density 1,000 against 1,024; a crushed range) but not a material's own regression. *Fix:* record each material's numbers as its own baseline (a ratchet, as artifacts keeps), with the pattern's bands as the floor every new material must meet.
3. **Three tools named look.** `cargo xtask look` (materials, tools/kits/look.ts), `cargo xtask looks` (places, D356) and tools/sandbox/look.ts (a picture's numbers, D286); the new one re-implements luma, percentiles, judging and tables beside the old. *Fix:* rename the bench (`cargo xtask material`) and share the measuring code.
4. **The load check is written twice.** xtask/src/load.rs (330 lines) and tools/sandbox/load.ts (168) sample the same load with the same thresholds kept equal by hand. *Fix:* one implementation, the other calling it (`cargo xtask load --wait`), or a test that the constants match.
5. **The registry grows as the square of the materials.** Every two wall kinds mixed, and every wall under every roof, are registered; with three walls and two roofs that is 388 scenes and 7.5 minutes. Hundreds of materials (D182) cannot run so. *Fix:* the families by pattern, one material each, and a material's own preview for each material.
6. **Small, fixed here:** the skill's material checks named `cargo xtask contract`, which checks placed models' sizes, not the voxel contract; development.md's artifacts row still described the old list of scenes and had no fuzz row, and its check rows left out the fuzz ratchet; fuzz.rs said faulty seeds are left out by name when they are held in the baseline.

Not checked: the debug views and the bench on the GPU (no run was cheap enough while the machine was loaded); the views' cost when off was read from the code, which compiles their shader only under `BUILD_VIEW` and runs their systems only while one is on, leaving one key check a frame and a `Level` component on every building mesh.
