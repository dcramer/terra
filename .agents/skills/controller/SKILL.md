---
name: controller
description: Work on Terra's character controller (the mover every body walks, jumps, steps, slides and is pushed by, docs/controller.md, D53) so that every job advances one criterion and nothing regresses. Covers the scorecard and its ratchet (cargo xtask controller, D457), the plain-Rust course in terra_motion::course and its Mover trait, the ordered task list of failing criteria, and the loop a job follows. Use for any task that changes or judges how a body moves through the world, so a brief needs to say only which criterion.
---

# Controller

Read [docs/controller.md](../../../docs/controller.md) (the design, its criteria) and D53 (the mover), D457 (this scorecard), D452 (the ratchet's rules) before changing the mover. Design calls (fall damage, climbing, swimming, who pushes whom: the doc's open questions; climbing's are decided, D505-D511) are the developers': recommend, put it on the review desk, don't build on it. The figure between clips (blends, stops, starts, turns) is the [anim skill](../anim/SKILL.md)'s; the clips are the [character skill](../character/SKILL.md)'s.

## The scorecard and the ratchet (D457)

- `cargo xtask controller`: every cheap criterion of `tools/controller/criteria.json`, a line each, in seconds once built. `✓` within its bar, `·` below it but held (a task), `✗` worse than `tools/controller/baseline.tsv` (fails), `?` not measurable yet (it says what it needs), `-` heavy.
- A number may never get worse. A trade the job chose is raised in the same commit, `cargo xtask controller --raise "<why>"`, the why naming the gain it pays for. Never hand-edit the baseline or lower it: nightly banks a gain (D414).
- The course is `terra_motion::course` (feature `course`): fixtures in `field.rs`, cases and what each measures in `runs.rs`. A new criterion is a row of `criteria.json` and a `Measure` of `runs::score`; its first number goes in with `--raise "<why>"`.
- The mover is `terra_motion::mover` (D53, ported from Jolt's `CharacterVirtual`; its kinds' numbers in `assets/data/movers.ron`), driven through `course::Mover` by `mover/course.rs`, the criteria file's `course`. The game's walk drives it (`terra_walker::movement`, unit C): the point walk it replaced and its course adapter are deleted, so the course scores the mover alone. Never change a fixture or a case to make a mover pass.
- Where each group of rows is measured (the course's files, the game's side, the in-game filmed course `cargo xtask gpu film -- cargo xtask controller --film`): [references/course.md](references/course.md). Read only the group the task touches.

## The loop, one job

1. **Pick** the top criterion of the list below (or the one the brief names).
2. **Change** the least that moves it, in the mover.
3. **Score**: `cargo xtask controller`. Nothing `✗`; the criterion's number lower. Run `cargo test -p terra_motion --features course` and the mover's crate's tests.
4. **Bank**: land on main (AGENTS.md); nightly lowers the baseline. Two jobs without a gain: stop, step back, hand off with what was tried.

**What the developer saw in play comes first.** The course scores the mover alone; what a player sees is the figure after the clips, the lock and placement. So each complaint from play (feet, stride, stops, a pose) becomes a row of the in-game filmed course, read from the walk record, that fails on main before anything is fixed, and goes to the top of the list. A course row never proves a play complaint fixed (combat's clip rows did not, 2026-10-26). A job that changes the figure puts a still from the game's own camera in its `FOR REVIEW` line.

## The task list (2026-10-25 unit U4d, the mover; feet at stage 3 unit 2c-6)

Every measurable row passes. What is left, ordered by `noticed`, then by how far out; re-read it from `cargo xtask controller --json` when it moves. Each item's state, code, handoffs and what was tried: [references/tasks.md](references/tasks.md), its numbered item.

1. `cost.body` passes (22.2 µs, bar 25); `cost.edits` below its bar of 30 and held: next, fewer asks or a body's own cell cache in its `HeightMemo`.
2. `cost.crowd` at its bar of 4 ms on a quiet box, held on a busy one: measure pinned with the load under ~4.
3. `feet.sliding` and `feet.glide` pass (0 m). Left: `pace.match` 0.6 (bar 0.1), Sprint_Loop's pace under a sprint; a real sprint clip is with the developers.
4. `feet.ground`, `feet.sinking`, `feet.aloft` pass. Left: the worst plant (1.4 cm), a heel strike onto the lump's 51 deg foot.
5. `slope.stopped` passes (0 m/s).
6. Starts, stops and turns (`stop.settle`, `stop.steps`, `stop.wall`, `start.steps`, `turn.steps`): the figure's side is the anim skill's (D631).
7. Climbing passes on the course (`climb.*`, `cost.climb`). Left: the game's walk and figure, once climbing clips exist.

Not measurable yet (`?`): stamina (D61 decides the attributes, not their costs); a walk pace (the player jogs or sprints). A change to the mover's queries runs `--bench` too: no ✗ on `cost.body`.
