# Controller

The character controller's scorecard (D457): docs/controller.md's production criteria, measured on the plain-Rust course (`terra_motion::course`) and held by the shared ratchet ([tools/scorecard](../scorecard/README.md), D452's). How to work on the controller with it is the `controller` skill (`.agents/skills/controller/SKILL.md`).

```bash
cargo xtask controller                   # every cheap criterion, a line each, held to baseline.tsv
cargo xtask controller --json out.json   # the results also as JSON
cargo xtask controller --bench           # also the mover's release bench: cost.body and cost.forest (µs a body a tick), cost.crowd (ms a tick)
cargo xtask gpu film -- cargo xtask controller --film   # also the in-game filmed course (yard.ts): step.smooth, feet.sliding, film.yard
node tools/controller/yard.ts --check ~/.cache/terra-review/controller-yard   # its checks again on the records already made
node tools/controller/forest-fixture.ts  # cost.forest's fixture again, from the game (when flora's fitting changes)
cargo xtask controller --raise "<why>"   # write the rows that got worse, or are new, with the why, in the change's commit
cargo xtask controller --lower           # write the rows that got better: cargo xtask nightly's alone (D414)
```

| File | What it is |
|---|---|
| `criteria.json` | A row a criterion in D452's shape (`id`, `notice`, `noticed` 1-5, `bar`, `unit`, `source`, `check`, `cost`, `measurable` or what it `needs`); `course` names the test that drives the mover through the course and prints `scorecard<TAB>id<TAB>number` lines |
| `baseline.tsv` | The ratchet: each measured criterion's number as held, with the why of its last rise |
| `yard.ts` | The in-game filmed course: the sandbox's `controller-yard` experiment (steps 0.1-0.5 m dug down and up, a lump, a mound past the slope limit, a base's stairs, doorway, low passage and 1.5 m drop) walked by its route at a jog and a sprint on the GPU (`--cine` with no keys: the game's camera), each run's walk record (`--fight-log`, the player as a walker) checked by `yard-checks.ts` (tested in `yard-checks.test.ts`; feet sliding is the duel's own check), the films side by side; all in `~/.cache/terra-review/controller-yard/` (`yard.mp4`, `checks.txt`, `checks.json`). A continuous check (`controller film`) |
| `scorecard.ts` | `cargo xtask controller`: runs the course's test, judges each number against its bar and its baseline; its parsing is tested in `scorecard.test.ts` |

A stage of `cargo xtask check --all` and of `cargo xtask nightly`, which lowers the baseline; a lowering is carried onto a moved main only if nothing `CONTROLLER` in `xtask/src/scope.rs` names changed. What cargo printed is in `target/controller/scorecard.log`.
