# Why do the fight clips look bad: the source takes, the offline bends, or the game's layers?

2026-10-30. The developer, 2026-10-28: "the animations all look like garbage". This note puts the heavy blow, a light blow and the block through each stage, beside Quaternius's hand-keyed sword clips (UAL2 Source v2.2, bought; `Sword_Heavy_A`, `Sword_Light_A`, `Sword_Block`, and UAL1's `Sword_Attack`). It uses the same ladder as research/2026-10-29-why-the-skeleton-moves-wrong.md. It changes no clip or code.

## Short answer

It is **the source takes** (c). The carry onto our body is faithful. The offline bends are small for the sword arm and large only for the shield arm. The game's layers add nothing a player would see in a blow.

- **Our blows are all arm.** The body stands still while the forearm chops from a bent elbow.
  - In the heavy, our hips turn 2°, the feet do not move and the pelvis does not drop.
  - In Quaternius's blows the hips turn 78-135°, the shoulders 180-214°, he steps 40-137 cm and drops 27-30 cm. A person's cut is driven from the legs and hips, though less than Quaternius's. His heavy is a full spin.
- **Our blows have no snap.** As the game plays them, our sword hand's fastest 100 ms is 3.1-4.5 m/s, and it travels 0.7 m in the strike. Quaternius's hand travels 2.3-3.8 m at 17-24 m/s.
  - Ours is a push, about a third of a person's speed. A good batter's hands reach roughly 9-11 m/s (Blast Motion's hand-speed readings, a two-handed proxy).
  - Quaternius's is about twice a person's: a cartoon snap with a smear.
- **The slowness is written into the keys.** The heavy's keys move the grip 0.72 m in 0.23 s (3.1 m/s), and the slash's keys 0.53 m in 0.2 s. Kimodo fills them in as given.
  - The game then plays each clip faster to hit Enshrouded's timings: the heavy 1.52x, the slash 1.21x, the backhand 2.18x, the block 1.81x. The totals match Enshrouded, but the shape inside does not.
  - Ours: a long, even wind-up (heavy 492 ms), a slow strike (183-267 ms) and a long recovery (heavy 892 ms).
  - Quaternius: a short wind-up (133-367 ms), a strike of 100-175 ms, then recovery.
- **The block's raise is not in its take.** Kimodo's `Keyed_Block` barely moves the shield arm (39° of swing, the elbow 4°). The raise you see in play is made by hold.py's eased blend (112° of swing). Nothing else in the body moves: the knees change 2° and the hips 1°. Quaternius's block crouches 75° at the knee and turns the shoulders 53°.

## Films and stills

They are in `~/.cache/terra-review/fight-ladder/`. Each film has six columns: (1) Quaternius's clip on his mannequin, (2) the same clip carried onto our body bare, (3) our Kimodo take on the mannequin, (4) our played clip on our body bare, (5) in the game with every layer, (6) in the game with only D584's three layers (`--pose-layers only=feet,gaze,grip`).

- Columns 1 to 4 were rendered in Blender, skinned as the game skins. Each shows the side view above the front-left view. Columns 3 and 4 play at the move set's speed.
- Columns 5 and 6 are the Windows build on the GPU, a camera following the player from the front-right, with sword and shield. They come from a scratch fight script: heavy, then a light blow, then a block, each alone.

| File | Shows |
|---|---|
| `heavy-six-columns.mp4`, `heavy-windup-contact-follow.jpg` | `Sword_Heavy_A`+`_Rec` against `Keyed_Heavy` and `Open_Heavy` at 1.52x |
| `light-six-columns.mp4`, `light-windup-contact-follow.jpg` | `Sword_Light_A`+`_Rec` against `Keyed_Slash` and `Open_Slash` at 1.21x |
| `block-six-columns.mp4`, `block-raise-hold-lower.jpg` | `Sword_Block` (a sword block; the packs have no shield raise) against `Keyed_Block` and `Clear_Block` at 1.81x |

The stills are three moments of our clip (wind-up, contact, follow-through), one row each. Columns 1 and 2 show the reference at the same time from its start.

## Stage by stage

### Offline: what bends the clip before the game

| Stage | What it does | Degrees off the stage before, mean / worst |
|---|---|---|
| `kimodo_keys.py` reach solver | Solves each key pose (grip, blade, edge) by two-bone IK. Kimodo fills in between the keys. | It does not bend a finished clip; it sets the motion. Its keys turn the sword forearm 121-168° toward palm-up (+129° in the heavy's first 0.45 s, +136° in the slash's first 0.25 s). The elbow swings up to 108° in 0.23 s. The forearm's roll over a blow is 136-141°, Quaternius's 109-137°: the same. |
| carry onto our body (`export_character.py`, by aim) | Kimodo's take on the mannequin, carried to our bones | 0.0 on every limb. The fixed rest offsets are the same in every frame: collarbone 41, pelvis 14.5, spine 7, foot 6. |
| `hold.py` (`Open_*`, `Clear_Block`) and `Strap_Grip` | The shield arm is replaced by a held pose solved to the board's place, and the fist is turned onto the strap | Shield arm: upper arm 53 / 78, forearm 61 / 67, hand 98 / 109 (the strap's turn) in the heavy and slash. Block: forearm 93 / 110, upper arm 68 / 107, and the sword arm eased out (forearm 34 / 47). |
| `clear.ts` | Lifts a gliding swing foot | Touches no blow (strafes and jog stops only). |
| `wrist_limit.ts` | Clamps the sword wrist into a person's range | hand_r only. Heavy 0.8 / 8.9 over 10 frames; slash 0.7 / 7.0 over 9 frames. Block 6.3 / 7.0 and the guard 8.1 / 9.4, on every frame. |

The committed `fight.glb` is byte-for-byte what these stages make: the rebuild matches it at 0.0°.

### Joint ranges over each blow (degrees; cm)

| Measure | Quaternius light | Quaternius `Sword_Attack` | Quaternius heavy (spin) | our slash | our heavy |
|---|---|---|---|---|---|
| hips' turn, span | 92 | 78 | 232 | 3 | 2 |
| shoulders' turn, span | 205 | 181 | 359 | 22 | 33 |
| shoulders against hips, span | 112 | 103 | 360 | 20 | 32 |
| sword elbow | 2-63 | 7-46 | 2-44 | 59-129 | 59-142 |
| sword arm's swing, span | 186 | 199 | 123 | 150 | 118 |
| knee, span | 56 | 49 | 91 | 6 | 8 |
| pelvis drop | 28 | 27 | 38 | 1 | 1 |
| feet apart, span | 41 | 137 | 185 | 1 | 1 |

### Timing and hand speed

Real milliseconds, the sword hand in the body's frame.

| Clip, as played | Total | Wind-up | Strike | Recovery | Strike path | Fastest 100 ms |
|---|---|---|---|---|---|---|
| Quaternius `Sword_Light_A`+`_Rec` | 867 | 133 | 175 | 558 | 3.18 m | 19.7 m/s |
| Quaternius `Sword_Regular_A`+`_Rec` | 1400 | 183 | 100 | 1117 | 2.34 m | 24.0 m/s |
| Quaternius `Sword_Attack` | 1500 | 333 | 142 | 1025 | 2.80 m | 22.7 m/s |
| Quaternius `Sword_Heavy_A`+`_Rec` | 1733 | 367 | 167 | 1200 | 3.81 m | 22.9 m/s |
| our slash 1.21x | 1617 | 233 | 267 | 1117 | 0.67 m | 3.1 m/s |
| our finisher 1.66x | 1183 | 167 | 192 | 825 | 0.67 m | 4.1 m/s |
| our backhand 2.18x | 1183 | 200 | 233 | 750 | 0.76 m | 3.9 m/s |
| our heavy 1.52x | 1567 | 492 | 183 | 892 | 0.69 m | 4.5 m/s |
| our partner's slash 0.37x | 5290 | | | | 0.67 m | under 1 |

Notes on the table:
- The strike is the run around the hand's peak speed at 40% of the peak or more.
- Our clips as made (1x) are slower still: the heavy's hand peaks at 3.2 m/s.
- On our body Quaternius's hand is 10-25% slower than on his mannequin. Part of this is the export resampling a 3-frame strike.
- The shield hand in our block rises at 4.5 m/s for 0.1 s. That speed comes from hold.py's ease; Kimodo's own take moves it 1.1 m/s.

### Runtime layers during blows

These are measured by the bone audit (`--bone-audit`) in the yard's duel, all layers against D584's three. "Mean" counts every frame; the share in brackets is the frames on which the layer moved the bone at all.

| Layer | Guard | Block | Slash | Chain | Heavy to contact |
|---|---|---|---|---|---|
| `feet` | foot 9 (100%), worst calf 21 | foot 9, worst calf 41 | foot 9, worst calf 44 | calf 12, worst 44 | foot 16, worst 26 |
| `gaze` | head 12, neck 8 | head 15 | head 15, worst 19 | head 15, worst 21 | head 13 |
| `grip` | the right fingers 93-115, constant | same | same | same | same |
| `shield_arm` (not D584's) | upper arm 4.7 | 2.0, worst 9 | 4.5, worst 12 | 4.6 | 4.7 |
| `arms` (not D584's) | upper arm 3.6 | 0.3 | 1.0, worst 5 | 0.7, worst 6 | 0.9, worst 5 |

Columns 5 and 6 look the same: the two layers D584 retires move a fighter by at most 12°. The foot lock bends the legs 8-16° on average and up to 44° at the calf. This is because the game drives the body forward during a blow (`drive`, up to 3.2 m/s in the heavy, a stand-in for root motion) while the take's feet stay planted, so the lock holds the feet against the slide. The gaze holds the head 12-15° off the clip throughout. The heavy is also turned 15° by its aim (D573), which moves the whole body, not a bone.

## Read, in plain words

Put a sword in Quaternius's man's hand and he winds up for a third of a second, then whips through in a sixth of a second with his whole body: he steps, drops, turns at the hips and throws the arm out straight. It is too much, a cartoon, but it reads as a blow.

Ours stands like a post. The feet stay put, the hips stay square and the knees stay locked in one bend. The fist moves 70 cm from the shoulder to the belly at walking pace, from a bent elbow, like someone wiping a window. Speeding the clip up makes the wipe faster, not a blow.

The block is a take of a man standing still with his left arm up, made to move only by the blend that lifts the shield.

The game's layers are not the cause: with all of them on or only three, the fight looks the same. The pipe from take to body is not the cause either. The wrist clamp moves the hand under 9°. The shield-arm hold swaps a whole arm, but that arm was never part of the swing.

## Which of (a), (b) and (c)

- **(c) The source takes: yes, the main cause.** The keys ask for a short, slow, arm-only cut, and Kimodo makes exactly that. No key moves the hips, the feet or the trunk, and Kimodo adds none of its own.
- **(b) The offline bends: small, except the shield arm.** `wrist_limit.ts` moves the sword hand up to 9°. `hold.py` and `Strap_Grip` replace the shield arm wholesale (50-110°), so the shield arm never moves with the blow. That is a choice for the board's place (D531, D554), not a fault in the swing. Twisting the forearm to the edge keys is in Quaternius's range, so it is not the cause.
- **(a) The layers: not what looks bad.** The foot lock bends a leg more than it should (the drive with no root motion, as in the jog), and the gaze holds the head 12-15°. Neither makes the blow look weak.

## What I recommend (docs/animation.md step 5)

Replace each blow with a clip made striking. Don't key, retime or patch the takes we have.

1. **The light chain and the heavy from the bought packs.**
   - UAL2 has light, regular and heavy sets (`Sword_Light_A-D`, `Sword_Regular_A-C`, `Sword_Heavy_A-D`, each with a `_Rec` recovery, and combos). They come with root motion (`UAL2_RM.glb`), so the drive can come from the clip rather than the move set.
   - They are cartoon-fast and turn hard (`Sword_Heavy_A` spins all the way round). Pick on the desk from a film of each, played as made with only the three layers. Calm them only by choosing a gentler clip, not by a layer.
   - They have no shield. The shield arm wants a guard overlay from a clip (step 4 of the method), laid from the spine up only on that arm.
2. **What the packs lack, from Uthana, made striking with a sword and shield in hand.** Candidates are a shield raise for the block, a diagonal heavy that is not a spin, and a backhand. Judge each on the same measures: hips and shoulders turning, a step, a strike under 200 ms, and the hand at about 9-11 m/s.
3. **Retire, as the clips are replaced:**
   - the keyed takes and their keys (`kimodo_keys.py`'s reach solver, `keys.ts`);
   - `wrist_limit.ts` on blows;
   - the play-rate fit to Enshrouded's timings. A clip should play within a tenth of as made, as D585 says for gaits. Pick clips whose own timing is near Enshrouded's: Quaternius's light chain hits at 133-183 ms, Enshrouded's at 220-320.
4. **The foot lock under a driven blow:** give the blow its root motion. Do not tune the lock.

This changes D532's line that the fight's moves come from Kimodo's keyed takes (D450). That line is the developers' (D532 is theirs), so it goes on the desk as a recommendation, not a decision.
