---
name: combat
description: Work on Terra's melee combat (the sword-and-shield move set, its clips, timings, rules, the duel and the fight camera) so that every round advances one criterion and nothing regresses. Covers the scorecard and its ratchet (cargo xtask combat, D452), the ordered task list of failing criteria, the loop a round follows, and the pipeline's commands (moves.ts, clip.ts, cargo xtask duel, the Enshrouded reference). Use for any task that changes or judges a move, a clip, the move set, the fight's rules or the duel, so a brief needs to say only which criterion.
---

# Combat

Read [docs/movement.md](../../../docs/movement.md) ("The goal", the pipeline) before changing a move, and D450 (clips by Kimodo keyed to the guard), D447 (the move check), D449 (stamina), D451 (the camera), D452 (this ratchet). Design calls (how a move should feel, what a tired fighter can do) are the developers': recommend, put it on the review desk, don't build on it.

## The scorecard and the ratchet (D452)

- `cargo xtask combat`: every cheap criterion of `tools/combat/criteria.json`, a line each, about a minute. `✓` within its bar, `·` below it but held (a task), `✗` worse than `tools/combat/baseline.tsv` (fails), `?` not measurable yet, `-` heavy. `--film` adds the duel film's checks (the GPU: `cargo xtask gpu film -- cargo xtask combat --film`, started in the background, D319).
- A number may never get worse. If a change must make one worse (a trade the round chose), raise it in the same commit: `cargo xtask combat --raise "<why>"`, the why naming the gain it pays for. Never hand-edit the baseline, never lower it: a gain is banked by `cargo xtask nightly` (D414).
- `clip.ts --into` runs the scorecard itself and puts every file back when the set is worse.
- A new criterion is a row of `criteria.json` (what a player notices, `noticed` 1-5, bar, source, check, cost, measurable or what it `needs`); its first number goes in with `--raise "<why>"`.

## The loop, one round

1. **Pick** the top failing criterion of the list below (or the one the brief names). One criterion, or one move's criteria together when one clip fixes them.
2. **Change** the least that moves it: a key in `art/animation/sets/fight.json` and `clip.ts`, a number in `assets/data/moves/sword_and_shield.ron`, a rule in `crates/terra_motion`.
3. **Score**: `cargo xtask combat`. Nothing `✗`; the criterion's number lower.
4. **Bank**: land on main (AGENTS.md); nightly lowers the baseline. Two rounds without a gain on a criterion: stop, step back (AGENTS.md), and hand it off with what was tried.
5. **Film** when a move changed: `cargo xtask gpu film -- cargo xtask combat --film` (the duel's film and its checks; the film in `~/.cache/terra-review/duel/`).
6. **Feel** goes to the review desk: a film for the developer's eye in a `FOR REVIEW` line (`review-desk` skill), never a claim in chat that it feels right.

## The task list (failing criteria, 2026-10-24, main 705a8520)

Ordered by what a player notices most (`noticed`), then by how far out. Re-read it from `cargo xtask combat --json` when it moves; the scorecard is the truth, this list its order.

1. `move.roll.held` 22.8 cm (bar 1): the shield goes through the spine in the roll.
2. `move.backhand.held` 12.5 cm: the shield through a thigh in the backhand.
3. `move.slash.held`, `move.finisher.held`, `move.partner_slash.held` 12.4 / 12.4 / 12.0 cm: the shield through a thigh in Fight_Slash (one clip, three moves).
4. `move.block.held` 6.4 cm: the sword through the head while blocking.
5. `move.heavy.held`, `move.partner_heavy.held` 4.2 cm: the shield in the spine in Sword_Attack.
6. `timing.roll.length` 0.79 s (bar 0.5-0.6): the roll lasts too long.
7. `timing.light.idle_to_hit` 0.41 s (bar 0.22-0.32): a light blow lands late.
8. The duel film's rows (`film.*`, heavy): their first numbers come from the first `--film` run.
9. `move.backhand.hands`, `move.slash.hands`, `move.finisher.hands` 22.0 / 18.1 / 18.1 cm (bar 2): the sword arm in the spine (with the keyed clips of item 10).
10. `move.*.from_guard`, `move.*.to_guard` 0.79-1.24 m (bar 0.1), every blow: the body snaps out of and into the guard; `move.slash.into_next`, `move.backhand.into_next` 0.70 m: the chain jumps. These pass only when the game's blows are keyed to its guard (D450: Keyed_Guard, then Keyed_Slash, Keyed_Backhand into the set, the handoff's order).
11. `timing.heavy.total` 2.02 s (1.4-1.7), `timing.heavy.hit_to_guard` 1.40 s (0.7-1.1): the heavy keeps the player too long.
12. `move.partner_slash.hands` 3.2 cm, `move.roll.hands` 2.6 cm.
13. `timing.hit_stop.*` 0.10-0.20 s (bar 0-0.05): Enshrouded flashes and kicks the camera instead (a design call: raise it on the desk before changing).
14. `move.backhand.contact_off` 61° (bar 35): the backhand lands beside the foe.
15. `timing.finisher.hit_to_guard` 0.78 s (0.8-1.1).
16. `move.Walk_Loop.held` 10.2, `move.Walk_Loop.hands` 6.2, `move.Idle_Loop.held` 21.4 cm: the drawn sword through the leg walking and idle.

Not measurable yet (`?`, each with what it needs in `criteria.json`): the blind review score against Enshrouded, the block's raise (ours and Enshrouded's), the guard's shield face and the blows' specs in the game's set (after `--into`), roll distance, hit reactions, a light blow's recovery, hit flash and camera kick, the partner's telegraph seen from the game's camera.

## The pipeline's commands

| Step | Command |
|---|---|
| The move check, every body template | `node tools/characters/moves.ts` (`--json rows.json` for the numbers, `--set`, `--clips`, `--spec`) |
| A clip from its spec (Kimodo, D450) | `node tools/characters/clip.ts <Clip>` into a scratch set; `--measure` rescores; `--into assets/data/moves/sword_and_shield.ron` keeps it, refused if the scorecard is worse |
| The duel film and its checks | `cargo xtask gpu film -- cargo xtask duel`; `cargo xtask duel --check <fight.jsonl>…` on records already made |
| The fight's rules | `cargo test -p terra_motion` (the duel test among them) |
| The Enshrouded reference | `tools/combat/refs/enshrouded.json` (`targets`, `unmeasured`); how it was read: `research/2026-10-24-enshrouded-combat-measured.md`; its pictures under `~/.cache/terra-review/refs/enshrouded-combat/` |
| The scorecard | `cargo xtask combat` (`--film`, `--json`, `--raise "<why>"`) |
