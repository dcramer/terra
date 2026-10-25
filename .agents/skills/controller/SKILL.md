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

## The task list (failing criteria, 2026-10-24, the mover)

Ordered by `noticed`, then by how far out. Re-read it from `cargo xtask controller --json` when it moves.

1. None below its bar (round 4: 34 pass). Next: the walker driving the mover (handoff `controller-stage2.md`, round 4's Next), then `cost.body` (the mover's bench).

The kind-data rows (`accel.data`, `turn.data`, `air.control`, `slope.speed`, `landing.speed`, `data.kinds`) are `course/data.rs`, run for a man and a test-only second kind; a mover reports facing and landings through `Mover::facing` and `Mover::landed` (`None` by default, so today's walker's adapter is unchanged and fails them).

What stops a body besides the ground (`step.building`, `step.prop`, `step.rock`, `tunnel.door`) is `course/sources.rs`: a fixture's `obstacles` (`mover::shapes`: turned boxes for a base's cubes, a door's leaf and a prop; `terra_core::Solid` pieces for a rock), answered apart from its ground field. A step is as high as named at its face, where it is climbed: a box laid level there, a rock's crest lowered by the world's curve at its middle.

Not measurable yet (`?`, each with what it needs): swimming's float, dive and breath, stamina (D61 decides the attributes, not their costs); heavy, the in-game filmed course (stage 1b: step smoothing, feet sliding, the yard) and the mover's bench (cost per body).
