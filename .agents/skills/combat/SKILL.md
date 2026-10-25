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

## The task list (failing criteria, 2026-10-25, after round 16)

Ordered by what a player notices most (`noticed`), then by how far out. Re-read it from `cargo xtask combat --json` when it moves; the scorecard is the truth, this list its order. The light chain (slash, backhand, finisher, the partner's slash) and the heavy and partner's heavy are keyed to the game's guard, Keyed_Guard (the set's `guard`, D450), and drop the guard to swing: they play `Open_Slash`, `Open_Backhand` and `Open_Heavy`, the keyed take with the shield arm eased out to the left through the blow and back to the guard in the recovery (D454's eased hold, hold.py `in`/`out`; Enshrouded's, [research](../../../research/2026-10-25-shield-arm-in-a-swing.md)), and pass every row; the roll (Fight_Roll, the library's with the arms held out of the tuck, D454) passes every row; the block (Keyed_Block, the shield across the face, its raise measured by moves.ts's `raised` bars) passes every row but one. A landed blow holds 0.03-0.05 s and shows by a flash on what it struck and a punch of the view (D456, the set's `landed`; `hit.*` rows, measured by the tests that hold them).

1. The sword through its own shield in the block, `move.block.sword_in_shield` (D481, bar 0 cm): 19.1 cm (the blade, 0.47 s). The block raises the shield across, so the sword must go out of its way (a hold on the right arm, or a raise keyed with it); not yet a fault in moves.ts (a blow's only).
2. How far a blow drops the guard is the developers' call (round 16 used the research's shipped answer, the shield out left, turned 90° at the shoulder): raised for their review (round 16).
3. `film.feet_sliding` (bar 0; 35 of 36 plants slid after round 9, 3 of 84 after round 10's foot lock, D455, 1 of 77 after round 11, 22.4 cm, the partner's right foot standing): what is left is the walk's own stride (a clip foot gliding in at 3.5 m/s, too fast in the body's frame to lock: stride warping, movement.md phase 2) and the slash's first frames (the clip's left foot sliding as it lands from the guard).

The drawn walk is `Fight_Walk`, the library's walk with the guard's arms held over it (D458, round 13), and passes; a drawn body never shows `Idle_Loop` (it stands in the guard), so the move check measures the guards, the strafes, `Fight_Walk` and `Jump_Loop`.

Not measurable yet (`?`, each with what it needs in `criteria.json`): the blind review score against Enshrouded, Enshrouded's block raise (`timing.block.raise`), the guard's shield face and the blows' specs in the game's set, roll distance, hit reactions, a light blow's recovery, the partner's telegraph seen from the game's camera.

## The pipeline's commands

| Step | Command |
|---|---|
| The move check, every body template | `node tools/characters/moves.ts` (`--json rows.json` for the numbers, `--set`, `--clips`, `--spec`) |
| A clip from its spec (Kimodo, D450) | `node tools/characters/clip.ts <Clip>` into a scratch set; `--measure` rescores; `--into assets/data/moves/sword_and_shield.ron` keeps it, refused if the scorecard is worse |
| The duel film and its checks | `cargo xtask gpu film -- cargo xtask duel`; `cargo xtask duel --check <fight.jsonl>…` on records already made |
| The fight's rules | `cargo test -p terra_motion` (the duel test among them) |
| The Enshrouded reference | `tools/combat/refs/enshrouded.json` (`targets`, `unmeasured`); how it was read: `research/2026-10-24-enshrouded-combat-measured.md`; its pictures under `~/.cache/terra-review/refs/enshrouded-combat/` |
| The scorecard | `cargo xtask combat` (`--film`, `--json`, `--raise "<why>"`) |
