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

Ordered by what a player notices most (`noticed`), then by how far out. Re-read it from `cargo xtask combat --json` when it moves; the scorecard is the truth, this list its order. The light chain (slash, backhand, finisher, the partner's slash) and the heavy and partner's heavy are keyed takes (Keyed_*, D450) played as `Open_Slash`, `Open_Backhand` and `Open_Heavy`: the shield arm held low at the left side throughout (upper arm down and out, forearm forward, face out; hold.py `aim`/`roll`, round 20), where the game's guard rests it (`Low_Guard`, the set's `guard`: Keyed_Guard with that arm, D531, round 32), so a blow starts and ends with the shield at the hip and the chain carries nothing (`move.chain.shield_round_trip` 0.00 m; no `chained`/`carry` in the set, D454); the strafes lay the same arm with the forearm a little further out (`Low_Strafe`), clear of the stepping thigh; the roll (Fight_Roll, the library's with the arms held out of the tuck, D454) passes every row; the block (`Clear_Block`: Keyed_Block, the shield raised from the low rest across the face and back down to it, an `out` before an `in`, its raise measured by moves.ts's `raised` bars, with the sword arm eased out to the right behind the board, D454, round 23) passes every row. A landed blow holds 0.03-0.05 s and shows by a burst where the blade met the body (a glow 3 frames and 20 sparks along the blade, `burst`) and a punch of the view, the struck body keeping its own colours (D456, D487, the set's `landed`; `hit.*` rows, measured by the tests that hold them, `hit.flash.frames` the struck body unlit at 0).

The sword through its own shield and the shield through the sword arm are a fault of every move and drawn clip moves.ts checks, not only the blows (D481, round 29): every one clears the heater, so a re-posed hold, a re-hung board or a new take must too, and clip.ts refuses a clashing take.

1. The fist on the heater's hand strap (D504, D459: the prototype carries a strapped heater, `models/held/heater_shield.glb`, its origin the hand strap), `held.shield.grip_hand` (bar 1 cm): 18.1 cm (moves.ts row `grip`). The fist is turned onto the strap in every clip (fight.json `Strap_Grip`, round 25; `grip_across_palm` passes), but the board hangs where the clips were keyed for it, about 11 cm off the forearm with its hand strap 11 cm past the fist. Hung as a real heater is (STRAPS `at` the held fist, about [0, 0.31, 0.07], `face` and `grip` square to the true forearm, elbow to wrist), the board is 11 cm nearer the body and 11 cm nearer the elbow, and 27 rows clash (sword through it in the guard 9 cm, thighs in the blows, the head in the block, the spine in the jump): the left arm must be re-posed in every clip around the new hang (round 22's solver), and `at` follow the body's forearm (the female's fist sits 2.4 cm nearer the elbow). How far a blow drops the guard stays the developers' call (round 20's low side pose, for their review).
2. `film.stagger.shield_jump` (bar 0.15 m): 0.21 m after round 29 (was 0.47), 0.10 m (passing) since round 32's low guard, whose board has less far to go. A reel plays under the guard's hold and a stagger blends in over 0.3 s (D454), so the shield goes to the guard and stays; what is left is the 0.3 s crossfade itself, the reel's torso turning the guard's shoulder. Next: an inertialized transition into the reel (Bollo, GDC 2018) rather than a longer blend, which would soften the reel.
3. `film.feet_sliding` (bar 0; 35 of 36 plants slid after round 9, 3 of 84 after round 10's foot lock, D455, 1 of 77 after round 11, 22.4 cm, the partner's right foot standing): what is left is the walk's own stride (a clip foot gliding in at 3.5 m/s, too fast in the body's frame to lock: stride warping, movement.md phase 2) and the slash's first frames (the clip's left foot sliding as it lands from the guard).

The game's composed pose (D530, round 30: the duel record's `pose`, after the hold layer, foot lock, lift and strap; `tools/sandbox/duel/pose.ts`), the developer's complaints from play, each a `--film` row, first numbers on main:
4. `film.pose.head_yaw_blows` (bar 15°): 41° left of the facing in the backhand, 35° on average through the blows, 26° on the guard; the clips' heads turn with the chest (off the hips 41°). Nothing turns the head back: a look-at on the neck and head toward the aim (Unreal's Look At / Godot's LookAtModifier3D) laid after the clips, or keys.
5. `film.pose.shield_rest_high` (bar 10 cm) passes since round 32: 8.1 cm (was 27.7; 61 in the block), the set's guard `Low_Guard` (D531). Left: the duel check counts 249 resting frames over 10 cm, the guard's own sway (the player's idle peaks at 14 cm) and the partner's walk-in (`Walk_Loop+Low_Guard`, median 13.8).
6. `film.pose.shield_forearm_deg` (bar 10°) 15.6°, `film.pose.shield_forearm_off` (bar 5 cm) 12.5 cm, `film.pose.shield_strap_fist` (bar 1 cm) 17.6 cm: the strap (`heater_shield.glb`'s `strap`) hangs the board off the forearm bone, the same in every stance; task 1's re-hang.
`film.pose.arms_in_body` (bar 2 cm) passes since round 33: -1.0 cm (was 6.2, the sheathed walk's and idle's right fingers in the thigh), by `terra_walker::arms` (D535), which turns an arm out at the shoulder after the clips whenever its hand hangs at the hip, so any idle or walk (a bought one too) gets it with no clip changed.

Each fighter plays the set its own hands give (holding.md step 3, round 27: `Hands`, the off hand's slot, D460); `held.torch.*` measure a torch carried in the off hand beside the sword over the guard, walk and strafes (a moves row's `hold`).

The drawn walk is the library's `Walk_Loop` with the set's guard laid over its arms at run time by a bone mask (D458, `terra_walker::layer`, round 28; `Fight_Walk` is gone), and passes, as do the jog and sprint under it; a drawn body never shows `Idle_Loop` (it stands in the guard), so the move check measures the guards, the strafes, `Walk_Loop+Low_Guard` (a clip under a hold; the `move.Keyed_Guard.*` and `move.Fight_Walk.*` rows read the `Low_Guard` rows since round 32), the jog and sprint so, and `Jump_Loop`. The guard laid over the strafes or the jump puts the sword into the thigh, so the strafes keep their own sword arm (with `Low_Strafe`'s shield arm baked) and the jump its own arms.

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
