# Combat

Combat's pipeline (docs/movement.md, "The goal"): a move goes from its spec to a judged film by commands an agent runs. This folder holds what the steps compare against, and the scorecard that holds every gain (D452); the clip and move-set checks live with the bodies in [tools/characters](../characters/README.md) (`phases.ts`, `moves.ts`, `clip.ts`). How to work on combat with it is the `combat` skill (`.agents/skills/combat/SKILL.md`).

```bash
cargo xtask combat                   # every cheap criterion, a line each, held to baseline.tsv (about a minute)
cargo xtask combat --film            # and the duel film's checks (the GPU: cargo xtask gpu film -- cargo xtask combat --film)
cargo xtask combat --film --out DIR  # the film into DIR, not ~/.cache/terra-review/duel/ (two films at once share that)
cargo xtask combat --json out.json   # the results also as JSON
cargo xtask combat --raise "<why>"   # write the rows that got worse, or are new, with the why, in the change's commit
cargo xtask combat --filmed          # the film rows from main's own duel film, not filming (nightly's, so it banks them)
cargo xtask combat --lower           # write the rows that got better: cargo xtask nightly's alone (D414)
```

| File | What it is |
|---|---|
| `criteria.json` | What combat must get right, a row a criterion: `id`, what a player would `notice` and how much (`noticed`, 5 most), its `bar` (`at_most`, `at_least`, `within`, or `pass` for a test) and `unit`, its `source` (an Enshrouded target or a decision), the `check` that measures it (`timing` from the move set, `moves` from moves.ts's JSON, `test` a named cargo test, `measured` the number a named cargo test prints as it passes, `measured <id> <value>`, `film` a duel check), `cost` (cheap: seconds, CPU; heavy: the GPU film), and `measurable` (if not, what it `needs`). A new criterion is a row here |
| `baseline.tsv` | The ratchet: each measured criterion's number as held, with the why of its last rise. Raised by the change that makes a number worse, lowered only by nightly |
| `scorecard.ts` | `cargo xtask combat`: measures the criteria and judges each against its bar and its baseline; its pure parts are tested in `scorecard.test.ts` |
| `refs/enshrouded.json` | Enshrouded's melee measured from first-party video (D180): timings in seconds, distances in body heights, the camera's framing, each with its source, timestamp, method and confidence; the ranges our set should land in (`targets`); and what these videos cannot show. How it was read and how our move set compares: [research/2026-10-24-enshrouded-combat-measured.md](../../research/2026-10-24-enshrouded-combat-measured.md) |

**How it reads.** A line a criterion: `✓` within its bar; `·` below its bar but no worse than held (the task list); `✗` worse than its baseline, or its check gave no number; `?` not measurable yet; `-` heavy, not run without `--film`. A criterion's number is its miss, how far outside its bar it is, so worse is always more whatever the bar; it exits 1 on a `✗` alone. The last line counts them; on a failure the `✗` lines follow it. What the steps printed is in `target/combat/scorecard.log`, moves.ts's rows in `target/combat/moves.json`.

**Where it runs.** A stage of `cargo xtask check --all`; a stage of `cargo xtask nightly`, which lowers the baseline; `clip.ts --into` after writing a set, putting every file back if the set is worse; and the film on main as the `combat film` continuous check (`tools/continuous/checks.json`), whenever what `COMBAT` in `xtask/src/scope.rs` names changes. The duel film writes its checks' numbers to `checks.json` beside `checks.txt` (`tools/sandbox/duel.ts`); a film of a clean checkout keeps them by commit in `~/.cache/terra-review/combat-films/`, and nightly's `--filmed` banks the film's rows from the newest of main's history that nothing `COMBAT` names changed since (`tools/scorecard/films.ts`).

The reference's pictures stay out of the repository, under `~/.cache/terra-review/refs/enshrouded-combat/`.
