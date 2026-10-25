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
- The mover is driven through `course::Mover`. Today's walk is driven by `crates/terra_walker/src/movement_course.rs`; stage 2's mover gets its own adapter and the criteria file's `course` names its test. Never change a fixture or a case to make a mover pass.

## The loop, one round

1. **Pick** the top criterion of the list below (or the one the brief names).
2. **Change** the least that moves it, in the mover.
3. **Score**: `cargo xtask controller`. Nothing `✗`; the criterion's number lower. Run `cargo test -p terra_motion --features course` and the mover's crate's tests.
4. **Bank**: land on main (AGENTS.md); nightly lowers the baseline. Two rounds without a gain: stop, step back, hand off with what was tried.

## The task list (failing criteria, 2026-10-24, today's walker)

Ordered by `noticed`, then by how far out. Re-read it from `cargo xtask controller --json` when it moves.

1. `tunnel.wall` 2 of 8 speeds (50, 60 m/s go through a voxel-thick wall); `step.ground` 2 of 10 steps wrong (the walker's step is about 0.7 m, not 0.55).
2. `slope.limit` 12.5 deg off (walks up to about 62 deg: the wall test needs a 0.7 m rise over a 0.4 m stride); `penetration.max` 0.97 m (the head into a tunnel's roof; a point walker's capsule into walls and slopes); `stick.slope` 54 and `stick.stair` 27 airborne ticks sprinting down; `jump.coyote` 0.1 s and `jump.buffer` 0.15 s off (neither exists); `push.overlap` 0.59 m, `push.fixed` 0.2 m; `fit.gap` 3 of 6 (squeezes through 0.5-0.59 m gaps); `swim.enters` (deep water is a wall, D74); `tunnel.body` 3 of 8.
3. `penetration.median` 0.077 m; `ceiling.depth` 0.97 m (no roof check in dug ground); `jump.apex` 0.047 m (Euler's step); `push.share` 0.8 off (no mass); `determinism.crowd` 2 of 3 reruns differ (bodies moved in the order given, not by ID).
4. `ceiling.speed` 5.5 m/s.

Not measurable yet (`?`, each with what it needs): doors, steps of bases, props and rocks, the slope speed curve, acceleration and turn as data, air control, landing speed, swimming's float, dive and breath, stamina; heavy, the in-game filmed course (stage 1b: step smoothing, feet sliding, the yard) and the mover's bench (cost per body).
