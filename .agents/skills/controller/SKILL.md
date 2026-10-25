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
- The mover is `terra_motion::mover` (D53, ported from Jolt's `CharacterVirtual`; its kinds' numbers in `assets/data/movers.ron`), driven through `course::Mover` by `mover/course.rs`, the criteria file's `course`. The game's walk drives it (`terra_walker::movement`, unit C): the point walk it replaced and its course adapter are deleted, so the course scores the mover alone. Never change a fixture or a case to make a mover pass.

## The loop, one round

1. **Pick** the top criterion of the list below (or the one the brief names).
2. **Change** the least that moves it, in the mover.
3. **Score**: `cargo xtask controller`. Nothing `✗`; the criterion's number lower. Run `cargo test -p terra_motion --features course` and the mover's crate's tests.
4. **Bank**: land on main (AGENTS.md); nightly lowers the baseline. Two rounds without a gain: stop, step back, hand off with what was tried.

## The task list (failing criteria, 2026-10-25 unit U4d, the mover)

Every measurable row passes (45 with --bench at unit U4; 38 at round 7: stairs fixed, handoff `controller-stage2.md`, "Round 7"). What is left, ordered by `noticed`, then by how far out. Re-read it from `cargo xtask controller --json` when it moves.

1. `cost.body` (passes: 22.2 µs on the course, 266 field calls a tick, bar 25; handoff "Unit A", "Unit B2", "Unit B4"): `cargo xtask controller --bench` runs the mover's release bench (`terra_walker` `movement_bench.rs`: the course and `Planet::new` at 2 and 30 km, µs and field calls a tick); and `cost.forest`, bar 50, in `movement_bench_forest.rs`: the walk scene's route on `Planet::rolling` among a fixture of the solids flora fitted there, written again from the game by `tools/controller/forest-fixture.ts` when the fitting changes, its calls a tick by source). Bodies are f64 and queries f32 offsets from a capsule's f64 origin (D482): sources take `DVec3`; the planet is asked through one `HeightMemo` a body (`Planet::distance_in(DVec3)`, `keep_within`); the march asks distances alone. 30 km now 13.2 µs / 162 calls, near 2 km's 11.0 / 153. Left for headroom: walk_stairs on a riser (500-1000 calls a tick for 4 ticks), the sweep's golden-section refine (most of a march step's calls), a shared gradient, per-tick allocations.
2. `cost.crowd` (at its bar on a quiet box, held on a busy one; handoff "Unit E", "Unit E2", "Unit E3"): a thousand men walking through each other on the planet at 30 km (`movement_bench_crowd.rs`), ms a tick on 8 threads (`THREADS`, the server's cores assumed), bar 4 ms, the median of 7 on rayon's pool (the server's) beside Bevy's (the game's), each printed with its CPU (lanes busy), serial and lane-start ms and the load. Bodies step against the tick's start (Jacobi, D53) through the caller's `mover::Executor` (`Bodies::tick_on`), a lane a thread with its own height memo and its own block of bodies, stealing after; `determinism.crowd` reruns on 5 and 16 threads, and terra_walker's test on Bevy's pool. Measure pinned (`taskset -c 0,2,..,14`) with the load under ~4: 3.8-3.9 ms, serial 0.37; the lanes' CPU is ~20% over one thread's and the floor probe is 66 of 192 ground calls a body.

The game's walk drives the mover (landed in unit U4d on a fixed-eye A/B, D53; handoff `controller-stage2.md` "Unit U4" to "Unit U4d"): `terra_walker::movement` and its sources in `terra_walker::mover_world` (props, flora, a base by `terra_build_layout::solids`, others, water); the shown figure's step smoothing in `stepping.rs`. A change to the mover that the game feels is checked in the game too: `cargo xtask cabin` (door, stone steps, deck stairs), `cargo xtask collide` (its step read from movers.ron, 0 failures), and `build.contacts` when the player stops where they should not.

Far from the origin (`far.grounded`, `far.float`, `far.queries`, criterion 22, D482) is `course/far.rs`: level ground, 20 deg up and down and 0.3 m steps walked at 2 km and at 30 km (`Course::on`), tick by tick against each other; reverting D482's frames fails all three.

Flights and a doorway's sill are `course/stairs.rs`: the ground's steps 0.1-0.5 m and a base's whole, half and stair cubes, up and down at the kind's jog (`stair.up`, `stair.down`, `stair.hop`), and a floor 0-0.1 m over the ground through a doorway (`door.hop`).

The kind-data rows (`accel.data`, `turn.data`, `air.control`, `slope.speed`, `landing.speed`, `data.kinds`) are `course/data.rs`, run for a man and a test-only second kind; a mover reports facing and landings through `Mover::facing` and `Mover::landed`.

What stops a body besides the ground (`step.building`, `step.prop`, `step.rock`, `tunnel.door`) is `course/sources.rs`: a fixture's `obstacles` (`mover::shapes`: turned boxes for a base's cubes, a door's leaf and a prop; `terra_core::Solid` pieces for a rock), answered apart from its ground field. A step is as high as named at its face, where it is climbed: a box laid level there, a rock's crest lowered by the world's curve at its middle.

Water (`swim.shore`, `swim.wade`, `swim.float`, criterion 17) is `course/water.rs`: a lake with smooth shores to 12 and 25 deg walked into, waded, swum across and out, straight and 25 and 45 deg off, and a body floated to rest; water is a medium the mover reads at the feet (`World.water`), never a contact.

The in-game filmed course (`step.smooth`, `feet.sliding`, `film.yard`; stage 1b) is one command, `cargo xtask gpu film -- cargo xtask controller --film` (started in the background, a few minutes; a continuous check, `controller film`): the sandbox's `controller-yard` experiment digs and builds the course's features east of the start (a trench of digs side by side stepping 0.1-0.5 m down to 1.5 m and up, a lump, a base's stairs, doorway, 2 m passage and 1.5 m drop, then a mound past the slope limit), its route jogged and sprinted, filmed through the game's camera, and `tools/controller/yard.ts` measures the rows from each run's walk record (`--fight-log`, the player as a walker: body, `below`, balls, camera), not by eye; `node tools/controller/yard.ts --check ~/.cache/terra-review/controller-yard` re-judges the records. Film and checks in `~/.cache/terra-review/controller-yard/`. A change to the walker's figure, camera or stepping runs it.

3. `feet.sliding` (0.90 m, bar 0.03; 80 of 87 plants slid, median 3.8 cm at a jog, 10.5 at a sprint): locomotion has no foot lock or stride warping (D455's lock is the fighters'): stage 3, pose for locomotion.

Not measurable yet (`?`, each with what it needs): swimming's dive and breath, stamina (D61 decides the attributes, not their costs). No walk pace to press (the player jogs or sprints), and no water at the yard: the filmed course has neither. A change to the mover's queries runs `--bench` too: no ✗ on `cost.body`.
