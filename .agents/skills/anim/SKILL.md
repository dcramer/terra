---
name: anim
description: Work on how Terra's figure goes from one clip to the next (starts, stops, jog and sprint changes, turns, jumps, into and out of the sword set's moves: blends, inertialization, distance matching, sync markers, stride and orientation warping) so that every job advances one criterion and nothing regresses. Covers the scorecard and its ratchet (cargo xtask anim, D631), the headless animation course in terra_walker figure::course, the ordered task list, the methods followed and the loop a job follows. Use for any task that changes or judges the figure's transitions or blends, so a brief needs to say only "anim-blend jN".
---

# Anim

Read D631 (the methods, in order), D455 (the foot lock), D452 (the ratchet's rules) before changing a blend. The clips themselves are the `character` skill's; the mover is the `controller` skill's; this is what the figure does between clips.

## The scorecard (D631)

- `cargo xtask anim`: the animation course (`crates/terra_walker/src/figure/course.rs`, an ignored test run in release, seconds once built), a line a criterion of `tools/anim/criteria.json`, held to `tools/anim/baseline.tsv`; its last lines name the failing row to take next (most `noticed`, then most bars out) and that row's worst transition: its clips, the joint and when.
- Every transition's line is in `target/anim/scorecard.log` (`anim<TAB>name<TAB>family<TAB>from -> to<TAB>pop ...`). Read the row's lines there, never the whole log.
- A number may never get worse; a trade is raised in the same commit, `cargo xtask anim --raise "<why>"`. Never hand-edit or lower the baseline (nightly's, D414; not wired into `cargo xtask nightly` yet: next steps).
- The course: steps in `course()` (a transition is a `tagged` step and its family), the measures in `course/measure.rs` (unit-tested). A new criterion is a family or measure the test prints and a row of `criteria.json`. Never change a step or a measure to make a blend pass; a measure found wrong is fixed with a unit test that shows it.

## The loop, one job

1. **Pick**: the `next:` row (or the one the brief names). Read its worst transition's line.
2. **Find the cause** by measurement first (change the one thing, rerun), before building the method D631 names for it.
3. **Change** the least that moves it, in the figure (`figure.rs` and its modules; `figure.rs` is near the 1,000-line limit: a new method is its own module).
4. **Score**: `cargo xtask anim`: nothing `✗`, the row lower. Run `cargo test -p terra_walker --lib` and `cargo xtask controller` (the feet and stops share the figure).
5. **Confirm in play**: one still or film of the game's own frame for the `FOR REVIEW` line when the figure's look changed; the course scores the pose, not the camera.
6. **Bank**: land on main. Two jobs without a gain on a row: stop, step back, hand off what was tried.

## The task list (anim-blend j2, 2026-11-06)

1. `pop.fight` 13.6 (into.roll, upperarm_r at its first frame; out.roll 8.3): inertialize the act's blend (D631).
2. `glide.stop` 2.73 m/s, `pop.stop` 3.74: play `Jog_Stop`/`Jog_Stop_R` (fight.glb) by distance on the planted foot (`terra_motion::distance`, `Markers::distance`); the glide measure then takes a distance-matched clip's root speed as its stride.
3. `start.root` 1.61 m/s, `pop.start` 2.79: `Jog_Start` by distance.
4. `pop.jump` 7.49 (Jump_Loop cut in at airborne 0.12 s, hand_l): inertialize, or a take-off clip.
5. `into.guard`/`out.guard` pop 7.49 on hand_l at +0.00: a hold laid at once (`layer::lay`), not a clip.
6. `seam.loops` 2.79 (Sprint_Loop's own wrap, upperarm_l): the clip's seam (the `character` skill).
7. `slide.fight` 0.21 m (the roll's feet), `glide.gait` 0.79 m/s.
