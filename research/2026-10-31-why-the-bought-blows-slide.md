# Why the bought blows slide

2026-10-31. Question: Quaternius's blows (UAL2) slide in our fight. At which stage, from his own clip with its own root motion up to in-game, does a foot start to slide? Diagnosis only; nothing was tuned. Measured on the parked set, origin/wip/fight-swap-j9 3f184083, `swap.ron`'s fight, the player's blows.

## In plain words

- **Stage 1 is not clean, in one way.** Between steps his feet stand dead still: 0.1 to 1.5 cm. But his steps mostly do not lift. 14 of the 18 steps in these four blows keep the ball of the foot within 3 cm of the floor. The finisher's right foot goes 183 cm in 0.10 s (31 m/s) with the ball rising 0.3 cm. The film's "leg snap" (fight-swap j10, strip 3) is this step, in his file, on the same frames.
- **Stage 3, our ease, breaks the stands.** The whole-body pelvis hold carries both legs with the hips, so a foot he keeps still creeps: 2 to 5 cm carried, up to 32 cm eased. This is the slow flat creep.
- **The drive itself does no harm.** Keyed drive against his real root path changes almost nothing.
- **The warp does some.** In the chain the fighter already stands inside `stop_at`, so the warp cuts the drive to nothing until the blade crosses. The slash, backhand and finisher each move 0.00 m there, against 0.38, 0.37 and 0.57 m keyed, while the feet in the clip go on as if the body moved.
- **The foot lock works where it runs.** It takes the ease's creep out of the slash and backhand. On the finisher and heavy it does nothing: `let_go` frees those two moves for their whole length (their rows are identical with the lock off).

## The table

Two measures, cm, left / right foot, worst of each blow.

**Slip** is the one to read. In each span where the foot stands in the raw clip (any point of its sole under 0.25 m/s, each end trimmed 0.05 s), it is how far the sole's least-moving point travels at that stage, so a pivot on heel or toe does not count. Raw is near 0 by construction; each row is what that stage added. The blows run to their game ends on S4 and later rows.

| Stage | Slash | Backhand | Finisher | Heavy |
|---|---|---|---|---|
| 1 Raw, his RM clip on his skeleton | 0.2 / 0.1 | 0.5 / 1.5 | 0.2 / 0.3 | 0.8 / 1.2 |
| 2 Carried, our body, root scaled 1.078 | 2.0 / 0.6 | 3.1 / 3.1 | 4.0 / 4.4 | 4.9 / 4.7 |
| 3 Eased, the Swap_ take, 0.08 s lead | 7.0 / 0.0 | 9.7 / **31.6** | **15.1 / 19.1** | **13.0 / 24.6** |
| 4a Driven, keyed, no warp | 7.1 / 0.0 | 9.0 / 31.2 | 15.1 / 21.4 | 13.0 / 30.3 |
| 4b In game, lock off (warped, faded, ended at done) | 6.2 / 0.0 | 10.2 / 28.8 | 7.1 / 34.6 | 8.5 / 13.7 |
| 5 In game, lock on, `let_go` as it is | 0.1 / 0.0 | 8.3 / 6.7 | 7.1 / 34.6 | 8.5 / 13.7 |
| 5b Lock let go only while the drive is over 4 m/s (j9's patch) | 0.2 / 0.0 | 8.3 / 5.4 | 0.0 / 34.6 | 17.5 / 8.6 |

Without the ease, to tell it from the warp:

| Stage | Slash | Backhand | Finisher | Heavy |
|---|---|---|---|---|
| Not eased, keyed drive | 2.0 / 0.0 | 2.4 / 2.6 | 5.9 / 6.4 | 3.2 / 6.4 |
| Not eased, the game's warped travel | 2.1 / 0.0 | 14.9 / 2.2 | 9.0 / 12.3 | 4.2 / 3.3 |

**The film's rule** (`checks.ts plants`: a ball within 2 cm of its lowest for 4 frames, worst distance from where it stood) is what the card reads. A foot that never lifts is one plant to it, so it counts his steps as slides, and it reads high on raw.

| Stage | Slash | Backhand | Finisher | Heavy |
|---|---|---|---|---|
| 1 Raw | 39.7 / 26.7 | 26.5 / 76.9 | 20.8 / 15.3 | 9.1 / 9.6 |
| 2 Carried | 45.0 / 24.4 | 32.5 / 71.1 | 33.4 / 186.0 | 14.1 / 3.9 |
| 3 Eased | 5.6 / 18.1 | 15.9 / 25.7 | 40.9 / 132.6 | 100.4 / 25.7 |
| 4a Driven, keyed | 4.4 / 18.3 | 15.9 / 22.7 | 32.8 / 144.6 | 54.6 / 21.2 |
| 4b In game, lock off | 4.4 / 18.6 | 13.0 / 21.1 | 24.9 / 102.5 | 25.8 / 20.9 |
| 5 In game, lock on | 0.1 / 18.6 | 13.2 / 3.5 | 24.9 / 102.5 | 25.8 / 20.9 |
| 5b Rush only | 0.1 / 18.6 | 13.2 / 3.5 | 0.1 / 76.5 | 0.0 / 2.2 |

Whole fight: lock on 15 of 74 plants slid, worst 73.2 cm; rush only 12 of 64, worst 143.2 cm (one plant of the right foot over frames 443-1453, with 161 cm of its path out of any blow; it may be a longer run and not a worse slide, not diagnosed); lock off 25 of 25.

Stage rows can fall as well as rise: the ease's drift and the warp's add as directions, so they partly cancel.

## Where the in-game slides fall

Each of the 14 plants the film flags on the player, frame by frame against his clip: 393 cm of their path falls while he is stepping, 67 cm in his stands, 14 cm out of a blow. So about 83% of what the card counts happens during his steps, and about 14% is in his stands, where all of it is ours. The split cannot say how much our stages changed the motion during his steps. These are path lengths, which run longer than the card's distances from the start.

What the strips of j10 were, by frames (the causes after the colon are inferred from the tables):

| Strip | Path in his steps / stands, cm | Reading |
|---|---|---|
| 3 late, the leg snap | 32 / 0 (frames 396-400) | His step, finisher t 0.28-0.38 |
| 3 early, the finisher's creep back | 73 / 21 (frames 376-394, with the snap's start) | The creep is in his stand, t 0.08-0.28: the body not going, warp and ease |
| 2, finisher left foot | 71 / 8 | During his steps |
| 5, heavy left foot | 19 / 2 | During his step |
| 8, heavy first step | 13 / 0 | During his step |
| 4, the backhand's creep | 57 / 0 | During his steps, slash and backhand |
| 1 and 7, the finisher's recovery | 47 / 27 and 18 / 0 | Mostly during his steps; the right foot's 27 is the ease going back to guard |

## Why the heavy misses

The chop falls 35 to 45 cm off the line to the partner (the fighter 10 degrees off, no `aim` on this move) with the gap at 1.65 m, and the step's end (1.41 m) arrives 5 frames after the blade has passed: it comes within 8 cm of the body at frame 195 and is under the ground by frame 197.

## What shipped games do at these stages

From Epic's Unreal documents; I read no game's own internals, so what follows beyond them is inference.

- **The ease.** Unreal's [Layered blend per bone](https://dev.epicgames.com/documentation/en-us/unreal-engine/animation-blueprint-blend-nodes-in-unreal-engine) blends a pose over a set of bones from a named bone down, the usual tool for an upper body set apart from the legs. A guard laid on from the spine up would leave his legs alone. Ours goes in at the pelvis, which carries both legs.
- **The lock.** Its [Foot Placement node](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/AnimNode_FootPlacement) plants by speed thresholds with [replant settings](https://docs.unrealengine.com/PythonAPI/class/FootPlacementPlantSettings.html), per foot and moment to moment, not per move: j9's patch is the same idea. A foot that moves at 31 m/s is released whichever rule is used.
- **The warp.** [Motion Warping](https://dev.epicgames.com/documentation/en-us/unreal-engine/motion-warping-in-unreal-engine) reshapes root motion so the character lines up with the target by the end of a window, which we follow. Scaling the travel to nothing leaves feet that were keyed for that travel with nowhere to go. Keeping a blow's own travel, or a minimum of it, is the likely answer, but I did not find a document saying so.
- **His steps.** No runtime stage restores a lift that is not in the clip. It has to be made: authored, or lifted by a tool (`clear.ts` lifts a gliding swing foot; not tried on these).

## Verdict

Removing a stage fixes only part of it. Taking the ease off the legs should bring the creep in his stands down to about the unwarped no-ease rows (0 to 6 cm), with the warp adding up to 15 cm on top; that is read off these tables, not run in the game. Freeing the lock only in the rush helps but is not enough alone: in 5b the finisher's left foot goes to 0, its right stays at 34.6, and the heavy's left foot rises from 8.5 to 17.5 cm, which is not diagnosed. The ease exists for a reason, too: his blows start from his stance and not our guard (the hips turned up to 105 degrees), so a blow eased only above the waist starts from a leg stance our guard does not hold, and that join has not been measured.

No stage removes the rest, which is most of what the card counts and what the eye saw as a snap: steps that drag on the floor, the worst of them the finisher's right foot going 1.8 m in a tenth of a second. So the clips are sound in one way, since his feet hold still between steps, and wrong for this fight in two: they begin from another stance than a held guard, and they do not lift. Keeping them means editing their steps as well as changing the ease. Whether that is worth it against other blows is the developers' call.

## Limits

- Stands are trimmed 0.05 s at each end, so the short ones are nearly lost: the finisher's first stand is measured over only 0.10 s (t 0.13-0.23) and the slash's right foot has no measured stand before its game end. The slip table is a floor, not a ceiling.
- Not measured: the fade at a move's end on its own (it is inside 4b and 5), the partner's blows, the ease above the waist only, and his clips on his own body as he previews them. The strips' reading in the table above is by plant, not by eye.
- A stage's stands are the raw clip's, on the raw clip's clock; a take resampled a frame or two off (the Q_ takes are) can leak a little at an edge, which the trim is for.
- Stages 1 to 4a are computed from the clips; 4b, 5 and 5b are the game's records. Offline, the eased clip on the game's own travel came within 0 to 7 cm of 4b on every foot, which is the check that the two agree.
- One fight, one script, one distance between the fighters. The warp's cut to nothing depends on that distance.
- The scripts were uncommitted (D249).
