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
2. **Change** the least that moves it: a key in `art/animation/sets/fight.json` and `clip.ts`, a number in `assets/data/moves/sword.shielded.ron`, a rule in `crates/terra_motion`.
3. **Score**: `cargo xtask combat`. Nothing `✗`; the criterion's number lower.
4. **Bank**: land on main (AGENTS.md); nightly lowers the baseline. Two rounds without a gain on a criterion: stop, step back (AGENTS.md), and hand it off with what was tried.
5. **Film** when a move changed: `cargo xtask gpu film -- cargo xtask combat --film` (the duel's film and its checks; the film in `~/.cache/terra-review/duel/`).
6. **Feel** goes to the review desk: a film for the developer's eye in a `FOR REVIEW` line (`review-desk` skill), never a claim in chat that it feels right.

## The task list (failing criteria, 2026-10-25, after round 25)

Ordered by what a player notices most (`noticed`), then by how far out. Re-read it from `cargo xtask combat --json` when it moves; the scorecard is the truth, this list its order. The light chain (slash, backhand, finisher, the partner's slash) and the heavy and partner's heavy are keyed to the game's guard, Keyed_Guard (the set's `guard`, D450), and drop the guard to swing: they play `Open_Slash`, `Open_Backhand` and `Open_Heavy`, the keyed take with the shield arm eased down to the left side through the blow (upper arm down and out, forearm forward, face out; hold.py `aim`/`roll`, round 20) and back to the guard in the recovery (D454's eased hold, hold.py `in`/`out`; Enshrouded's, [research](../../../research/2026-10-25-shield-arm-in-a-swing.md)), and pass every row; a chained blow mixes in its held take (`chained`, no `in`) by how far out the blow it cut short still carries the shield (`carry`; the runner's `takes`, the figure's `mix`, D454), so the shield stays out between blows (`move.chain.shield_round_trip` 0.08 m, the worst press of the window); the roll (Fight_Roll, the library's with the arms held out of the tuck, D454) passes every row; the block (`Clear_Block`: Keyed_Block, the shield across the face, its raise measured by moves.ts's `raised` bars, with the sword arm eased out to the right behind the board, D454, round 23) passes every row. A landed blow holds 0.03-0.05 s and shows by a burst where the blade met the body (a glow 3 frames and 20 sparks along the blade, `burst`) and a punch of the view, the struck body keeping its own colours (D456, D487, the set's `landed`; `hit.*` rows, measured by the tests that hold them, `hit.flash.frames` the struck body unlit at 0).

1. The fist on the heater's hand strap (D504, D459: the prototype carries a strapped heater, `models/held/heater_shield.glb`, its origin the hand strap), `held.shield.grip_hand` (bar 1 cm): 18.1 cm (moves.ts row `grip`). The fist is turned onto the strap in every clip (fight.json `Strap_Grip`, round 25; `grip_across_palm` passes), but the board hangs where the clips were keyed for it, about 11 cm off the forearm with its hand strap 11 cm past the fist. Hung as a real heater is (STRAPS `at` the held fist, about [0, 0.31, 0.07], `face` and `grip` square to the true forearm, elbow to wrist), the board is 11 cm nearer the body and 11 cm nearer the elbow, and 27 rows clash (sword through it in the guard 9 cm, thighs in the blows, the head in the block, the spine in the jump): the left arm must be re-posed in every clip around the new hang (round 22's solver), and `at` follow the body's forearm (the female's fist sits 2.4 cm nearer the elbow). How far a blow drops the guard stays the developers' call (round 20's low side pose, for their review).
2. `film.feet_sliding` (bar 0; 35 of 36 plants slid after round 9, 3 of 84 after round 10's foot lock, D455, 1 of 77 after round 11, 22.4 cm, the partner's right foot standing): what is left is the walk's own stride (a clip foot gliding in at 3.5 m/s, too fast in the body's frame to lock: stride warping, movement.md phase 2) and the slash's first frames (the clip's left foot sliding as it lands from the guard).

The drawn walk is `Fight_Walk`, the library's walk with the guard's arms held over it (D458, round 13), and passes; a drawn body never shows `Idle_Loop` (it stands in the guard), so the move check measures the guards, the strafes, `Fight_Walk` and `Jump_Loop`.

Not measurable yet (`?`, each with what it needs in `criteria.json`): the blind review score against Enshrouded, Enshrouded's block raise (`timing.block.raise`), the guard's shield face and the blows' specs in the game's set, roll distance, hit reactions, a light blow's recovery, the partner's telegraph seen from the game's camera.

## The pipeline's commands

| Step | Command |
|---|---|
| The move check, every body template | `node tools/characters/moves.ts` (`--json rows.json` for the numbers, `--set`, `--clips`, `--spec`) |
| A clip from its spec (Kimodo, D450) | `node tools/characters/clip.ts <Clip>` into a scratch set; `--measure` rescores; `--into assets/data/moves/sword.shielded.ron` keeps it, refused if the scorecard is worse |
| The duel film and its checks | `cargo xtask gpu film -- cargo xtask duel`; `cargo xtask duel --check <fight.jsonl>…` on records already made |
| The fight's rules | `cargo test -p terra_motion` (the duel test among them) |
| The Enshrouded reference | `tools/combat/refs/enshrouded.json` (`targets`, `unmeasured`); how it was read: `research/2026-10-24-enshrouded-combat-measured.md`; its pictures under `~/.cache/terra-review/refs/enshrouded-combat/` |
| The scorecard | `cargo xtask combat` (`--film`, `--json`, `--raise "<why>"`) |
