# Anim

The figure's animation scorecard (D631): how the man's figure goes through every transition it makes (starts, stops, gait changes, turns, jumps, into and out of every sword-set move), measured by numbers alone on the animation course (`terra_walker` `figure::course`: male.glb posed headless with no GPU by the game's own figure, foot lock, pose layers and lift, moved as the mover moves a man) and held by the shared ratchet ([tools/scorecard](../scorecard/README.md), D452's). How to work with it is the `anim` skill (`.agents/skills/anim/SKILL.md`).

```bash
cargo xtask anim                   # every criterion, a line each, held to baseline.tsv, and the failing row to take next
cargo xtask anim --json out.json   # the results also as JSON
cargo xtask anim --raise "<why>"   # write the rows that got worse with the why, in the change's commit; new rows as their first number
cargo xtask anim --lower           # write the rows that got better (nightly's, D414)
node --test tools/anim/scorecard.test.ts
```

| File | What it is |
|---|---|
| `criteria.json` | A row a criterion in D452's shape; `course` names the ignored test that prints `scorecard<TAB>id<TAB>number` lines and an `anim` line a transition (name, family, clips, worst pop with its joint and time, slide, glide) |
| `baseline.tsv` | The ratchet: each criterion's number as held, with the why of its last rise |
| `scorecard.ts` | `cargo xtask anim`: runs the course in release (seconds once built), judges each number, ranks the failing rows (`noticed`, then bars out) and prints the top one's worst transition |

The measures (`crates/terra_walker/src/figure/course/measure.rs`, unit-tested): a pop is a joint's change of angular velocity in a frame over the worst its clips do alone at full weight (floored at 60 deg/s); a slide a ball moving while its clips have it planted (their baked lock curve at 1) within 2 cm of its standing height (the course stands on the world, its up a place from the planet's centre as the game's); a glide the body's speed against the weighted pace x rate x warp of its leg clips. What cargo printed is in `target/anim/scorecard.log`. A stage of `cargo xtask check --all`.
