---
name: controller
description: Work on Terra's character controller (the mover every body walks, jumps, steps, slides and is pushed by, docs/controller.md, D53) so that every round advances one criterion and nothing regresses. Covers the scorecard and its ratchet (cargo xtask controller, D457), the plain-Rust course in terra_motion::course and its Mover trait, the ordered task list of failing criteria, and the loop a round follows. Use for any task that changes or judges how a body moves through the world, so a brief needs to say only which criterion.
---

# Controller

Read [docs/controller.md](../../../docs/controller.md) (the design, its criteria) and D53 (the mover), D457 (this scorecard), D452 (the ratchet's rules) before changing the mover. Design calls (fall damage, climbing, swimming, who pushes whom: the doc's open questions) are the developers': recommend, put it on the review desk, don't build on it.

## The scorecard and the ratchet (D457)

- `cargo xtask controller`: every cheap criterion of `tools/controller/criteria.json`, a line each, in seconds once built. `✓` within its bar, `·` below it but held (a task), `✗` worse than `tools/controller/baseline.tsv` (fails), `?` not measurable yet (it says what it needs), `-` heavy.
- A number may never get worse. A trade the round chose is raised in the same commit, `cargo xtask controller --raise "<why>"`, the why naming the gain it pays for. Never hand-edit the baseline or lower it: nightly banks a gain (D414).
- The course is `terra_motion::course` (feature `course`): fixtures in `field.rs`, cases and what each measures in `runs.rs`. A new criterion is a row of `criteria.json` and a `Measure` of `runs::score`; its first number goes in with `--raise "<why>"`.
- The mover is `terra_motion::mover` (D53, ported from Jolt's `CharacterVirtual`; its kinds' numbers in `assets/data/movers.ron`), driven through `course::Mover` by `mover/course.rs`, the criteria file's `course`. Today's walk is still driven by `crates/terra_walker/src/movement_course.rs` for comparison until the walker drives the mover. Never change a fixture or a case to make a mover pass.

## The loop, one round

1. **Pick** the top criterion of the list below (or the one the brief names).
2. **Change** the least that moves it, in the mover.
3. **Score**: `cargo xtask controller`. Nothing `✗`; the criterion's number lower. Run `cargo test -p terra_motion --features course` and the mover's crate's tests.
4. **Bank**: land on main (AGENTS.md); nightly lowers the baseline. Two rounds without a gain: stop, step back, hand off with what was tried.

## The task list (failing criteria, 2026-10-25 unit U4, the mover)

Every measurable row passes (45 with --bench at unit U3; 38 at round 7: stairs fixed, handoff `controller-stage2.md`, "Round 7"). What is left, ordered by `noticed`, then by how far out. Re-read it from `cargo xtask controller --json` when it moves.

1. The walker driving the mover (unit C, handoff `controller-stage2.md` "Unit U4", "Unit U4b"): the switch is local branch `wip/controller-u4b` (main at 54b0e8c2 + the switch, `build.contacts`, collide.ts's bar from movers.ron, cleanup). On it `cargo xtask cabin` passes every step (door, stone steps, deck stairs), `cargo xtask collide` 0 failures of 26, the scripted play run stands the feet on the ground at start and 300 m out, controller --bench and combat nothing worse. Not landed: on the GPU walk scene (`node tools/sandbox/frametimes.ts` over 5 alternating pairs) the game's p99 is 10.90 ms against main's 9.42, all of it the sky's repaints (`terra_world::skylight::paint`, 7 ms each, the same in both builds), which the switch's eye path sets off 27-29 times a run against main's 24; the mover's own tick is 0.22 ms at p99 against the old walk's 0.20, and with the repaints left out the game's p99 is 7.73 against 7.55. Next: a bar that leaves the repaints out, or why the eye repaints more; then land and delete the unit's old branches and worktrees. Its pieces: `mover::Bodies::{place, remove}`, `Intent::scale`, `Body::jumped`, the sources in `terra_walker::mover_world` (props, flora, a base by `terra_build_layout::solids`, others, water).
2. `cost.body` (passes: 22.2 µs on the course, 266 field calls a tick, bar 25; handoff "Unit A", "Unit B2", "Unit B4"): `cargo xtask controller --bench` runs the mover's release bench (`terra_walker` `movement_bench.rs`: the course and `Planet::new` at 2 and 30 km, µs and field calls a tick, against today's walk); and `cost.forest`, bar 50, in `movement_bench_forest.rs`: the walk scene's route on `Planet::rolling` among a fixture of the solids flora fitted there, written again from the game by `tools/controller/forest-fixture.ts` when the fitting changes, its calls a tick by source). Bodies are f64 and queries f32 offsets from a capsule's f64 origin (D482): sources take `DVec3`; the planet is asked through one `HeightMemo` a body (`Planet::distance_in(DVec3)`, `keep_within`); the march asks distances alone. 30 km now 13.2 µs / 162 calls, near 2 km's 11.0 / 153. Left for headroom: walk_stairs on a riser (500-1000 calls a tick for 4 ticks), the sweep's golden-section refine (most of a march step's calls), a shared gradient, per-tick allocations; `cost.crowd` needs a broadphase.

Far from the origin (`far.grounded`, `far.float`, `far.queries`, criterion 22, D482) is `course/far.rs`: level ground, 20 deg up and down and 0.3 m steps walked at 2 km and at 30 km (`Course::on`), tick by tick against each other; reverting D482's frames fails all three.

Flights and a doorway's sill are `course/stairs.rs`: the ground's steps 0.1-0.5 m and a base's whole, half and stair cubes, up and down at the kind's jog (`stair.up`, `stair.down`, `stair.hop`), and a floor 0-0.1 m over the ground through a doorway (`door.hop`).

The kind-data rows (`accel.data`, `turn.data`, `air.control`, `slope.speed`, `landing.speed`, `data.kinds`) are `course/data.rs`, run for a man and a test-only second kind; a mover reports facing and landings through `Mover::facing` and `Mover::landed` (`None` by default, so today's walker's adapter is unchanged and fails them).

What stops a body besides the ground (`step.building`, `step.prop`, `step.rock`, `tunnel.door`) is `course/sources.rs`: a fixture's `obstacles` (`mover::shapes`: turned boxes for a base's cubes, a door's leaf and a prop; `terra_core::Solid` pieces for a rock), answered apart from its ground field. A step is as high as named at its face, where it is climbed: a box laid level there, a rock's crest lowered by the world's curve at its middle.

Not measurable yet (`?`, each with what it needs): swimming's float, dive and breath, stamina (D61 decides the attributes, not their costs); the crowd's cost (a broadphase); heavy, the in-game filmed course (stage 1b: step smoothing, feet sliding, the yard). A change to the mover's queries runs `--bench` too: no ✗ on `cost.body`.
