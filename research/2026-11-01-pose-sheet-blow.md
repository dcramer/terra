# A blow keyed from a pose sheet

2026-11-01. Question: can an agent make a fight clip that meshes with our guard by reading a generated pose sheet and writing key poses on our own skeleton, with no motion generator in between? The developer's idea: "I have found using chatgpt to create a pose sheet of the animation sequence, and then I get opus to animate against the pose sheet reasonably effective." One blow, the slash, on the player's body. The tools are in [spike/pose-sheet](../spike/pose-sheet/README.md); the process is in [docs/keyed-moves.md](../docs/keyed-moves.md) (D588).

## In plain words

- **It worked at the first attempt.** One sheet from `gpt-image-2.5`, given a render of our guard, showed clear poses with planted feet and the weight moving through the blow. Ten keys written by reading it matched it panel by panel after three rounds of correction.
- **The faults that sank the bought and generated blows are gone by construction.** The clip starts and ends on the guard exactly. A foot not told to move does not move. The moment of the hit is written down, not hoped for. The same keys always give the same clip.
- **It was quick.** About 16 minutes from first render to the duel, including writing the tool. The image was free.
- **It is not finished.** The blow stands its ground, so it lands less often. Stepping in has to be keyed as a real step.
- **The developer's eye has not judged it yet.** Numbers passed clips before that the eye rejected, so this note does not call the look good.

## How it was made

1. Render our body in `Low_Guard` from two sides.
2. Ask for eight numbered panels of the blow from one fixed camera, naming the feet as planted, with that render as the reference (`tools/tripo/tripo.ts image --model gpt-image-2.5`).
3. Write the keys in `slash.json`: hips, chest, the sword hand's place and blade, the leading edge, the elbow's way, the heel. Arms and legs are solved to reach them; a monotone curve joins the keys, so a pose keyed is the pose reached, with no overshoot.
4. Render at the panels' moments from the sheet's camera, lay it under the sheet, and correct by eye.

Panels 2 and 3 of the sheet have the blade pointing forward during the wind-up, which a real cut does not do. The keys keep the sheet's arm and body but set the blade upright and back. A sheet is a reference for an animator to read, not a thing to trace.

## Numbers

`tools/characters/mesh.ts`, on our body, clip only:

| | Open_Slash (ours today) | Sheet_Slash |
|---|---|---|
| Start and end, feet off the guard | 0.1 / 0.2 cm | 0.0 / 0.0 cm |
| Slip of a standing foot | 3.6 / 2.8 cm | 0.3 / 0.6 cm |
| Sword hand's fastest | 3.1 m/s | 7.7 m/s |
| Hips / shoulders turned | 2 / 22 deg | 50 / 99 deg |
| Strike | 0.27 s | 0.18 s |
| Whole clip | 1.62 s | 1.15 s |

Against the Quaternius and Uthana blows ([why they slide](2026-10-31-why-the-bought-blows-slide.md), [Uthana's](2026-10-31-uthana-sword-blows.md)): no drag or snap of a foot, and the hit needs a play rate of 1.46 where Uthana's takes needed 1.5 to 4. The 8 m/s hand bar is still unproven for a sword.

In the real duel, with the clip in place of `Open_Slash` and its rows retimed on a working copy (`slash`, `finisher` and `partner_slash`: their `drive` and `warp` removed, nothing landed):

| | Old film, 10-30 | This run |
|---|---|---|
| Plants sliding past 3 cm | 20 of 111, worst 73.0 cm | 2 of 84, worst 23.7 cm in a strafe |
| Blows in reach in their active frames | 7 | 2 |
| Contacts | 7 | 5 |

The old film is from an older main, and taking out the drive changes the fight's length, so the foot row is an indication and not a clean before and after. The drop in reach and contacts is the cost of the blow standing still. `glb_clips.ts` does not carry a clip's foot locks, so this run probably played the new clip on `Open_Slash`'s old locks, which makes the foot row weaker still; that is read from the code, not checked against the run.

## What the test does not show

- **A step.** The bought blows' slide came largely from moving the body over feet the clip kept still, and this clip was tested with that drive removed. Whether a keyed step plus `drive` holds up is untested.
- **Other blows and chains.** Only the slash changed; a chain into `Open_Backhand` was played but not judged.
- **The gates a blow must pass.** `moves.ts` was not run, so the wrist's bar and the sword through its own shield are unchecked. The film's shield clears by eye only.
- **The look.** Waiting on the developer, on the review desk (`pose-sheet-slash`).

## The idle, one flow

The developer: "the idle animation might be our good test". `Idle_Loop`, standing relaxed with empty hands, keyed as a 6 s loop from its own first pose. The bar is today's idle (8.08 s), which the developer picked on the desk.

- **The sheet helped little.** One draw, of a possible two. It ignored the layout (two rows, twelve panels, repeated numbers), and its panels barely differ. It did show a hip shift onto one straight leg with the other knee soft, and a head turn. The keys' numbers are my own reading, so this is close to keying without a sheet.
- **The tool needed two small additions.** `loop` runs the curve's speed through the seam, and `hang` lets empty arms ride the chest with a little sway.
- **Fine motion is the open risk.** Eight keys hold the fingers at the first pose. Nobody has judged the film, so this note does not say whether it reads alive or stiff.

| | Today's `Idle_Loop` | `Sheet_Idle` |
|---|---|---|
| Seam, pose off / speed off | not measured | 0.000 cm / 0.003 m/s |
| Ball of the foot, left / right, range | 2.5 / 2.1 cm | 0.6 / 0.0 cm |
| Hips, range | 2.5 cm | 6.7 cm |
| Chest, range | 4.2 cm | 5.6 cm |
| Head, range | 6.0 cm | 5.9 cm |
| Hands, left / right, range | 11.8 / 6.3 cm | 6.6 / 6.1 cm |
| `moves.ts` hands, deepest into the body | -8.0 cm, clear | -9.1 cm, clear |

Ours swings its hips over twice as far as today's and its left hand about half as far. The left leg reached 100% of its length at one point, so that key is at the limit.

Gates: the seam and the feet were measured directly, and the arms by `moves.ts --also`. `mesh.ts` does not apply, since it times a blow. The `held` rows don't apply either: they hang a shield on an idle that holds nothing. The film of the clip on our body exists. **The film in the real game was not made**, because the job ran out of context.

## Why this fits many clips

This is how game animators work: block the key poses, then time them. The sheet stands in for the thumbnail sketches. Two pieces make it repeat: a control set the agent can write in plain places and angles, and a gate (`mesh.ts`, `moves.ts`) before any film. Nothing generates the motion, so nothing has to be cleaned up afterwards, and a wrong blow is a changed number in a short text file. It leans on D565's reference item per class: keyed once per class, then played by every item in the class's envelope.

## Limits

- One sheet, one blow, one agent reading it. A second sheet of the same blow would differ, and the keys are one reading of it.
- The body's short arms (0.44 m shoulder to wrist) keep the wind-up lower than the sheet's.
- The forearm's roll and the fingers are simple: half the wrist's turn goes up the forearm, and the fingers hold the guard's grip.
- `key.ts` knows the player's body and one guard; other bodies and templates were not tried.

## The idle's palms

The developer, on today's `Idle_Loop`: "hes standing there with his palms out". Measured at its first frame, a resting palm should face the thigh or a little behind it, as `Walk_Loop`'s do (17-27% backward on the right). Today's left palm is 36 degrees off the thigh and faces 40% forward; the right faces 10% forward. The keyed idle had copied those hands from its start pose, so it had the same fault. `palms` in the keys rolls each forearm (right 15, left 35 degrees), which brought the right to 16% backward. No test failed on this, and only the eye caught it: the idle was picked and passed the checks with its palms out. The arms still stand well away from the body in both idles.

## The idle again, through the pipeline

The developer asked for the arms in, then: "remember this is a test of our new pipeline". So the change went through the steps, not a tweak.

- **The brief made the sheet.** The same model, asked for one row of five panels with each one's difference spelled out and the hands by name, followed the layout and drew clearly different poses. The first sheet was vague, not unusable: step 3 works for small moves when the brief is exact.
- **The layover now works** for empty hands (`stills.py ... empty`), so step 5 ran.
- **A new gate catches the fault.** `rest.ts` fails today's `Idle_Loop` on both hands (left palm 46% forward, wrists 26 and 27 cm out) with bars from `Walk_Loop`.
- **Two gates pull against each other.** Arms 12 degrees in leave the right wrist 19 cm out against the 18 cm bar, and at 15 degrees the hand goes 2.9 cm into the thigh on the female template. The clip keeps 12: hands clear of every body (0.2 cm), `rest.ts` still ✗ on the right by a centimetre.
- **Still inherited.** The head stays turned as in `Idle_Loop`'s first pose, where the sheet looks ahead, and the feet stand closer than the sheet's. The left leg reaches full stretch at one point. Fingers don't move. The look and the game are unjudged.

## The idle on the longer body, from a neutral start

2026-11-02. The developer, on the longer body (D593): "He looks fine but his pose is atupid there". The stupid part was the start pose, `Idle_Loop`'s first frame: head turned 26 degrees to his right, shoulders 4 to 7 cm higher over the chest than the walk carries them, the right foot tucked 6 cm inside the hip. A key can't fix a start pose, since every key is measured from it, so `key.ts` gained `start`: the chest's roll levelled, the head set facing ahead (2 degrees down from the bind pose), the shoulders at `Walk_Loop`'s mean, and the balls of the feet 22 cm apart under the hips. The arms keep the way they hung, then `arms_in` and `palms` set them.

- **The sheet was drawn from that start**, one of two allowed, with the brief that worked before. It kept the layout and the poses (a hip shift onto one straight leg, the other knee soft and its heel up, a breath, the other side), but drew in a painted style with outlines, which the prompt's "clear outlines" invited. For reading poses that doesn't matter. It put the first shift on his left where the brief said right; the keys follow the sheet.
- **Two rounds of layover.** The first had the free knee too straight; a heel lift of 11 degrees in place of 7 matched the sheet. The sheet's free knee still drops further in than ours, which reads as posing in an idle.
- **The arm settings retuned for the longer arms**: `arms_in` right 11, left 15 (were 12 and 12). The two gates still pull: at 12 the right hand touches the female thigh (0.1 cm in), at 11 it is clear by 0.8 cm with the wrist at 18 cm against the 18 cm bar.

| | Today's `Idle_Loop` | `Sheet_Idle` |
|---|---|---|
| Head turned at the start | 26 deg right | 0 |
| Shoulders over the chest at the start, right / left | 16.5 / 18.5 cm | 12.5 / 12.4 cm (walk 13.5 / 11.4) |
| `rest.ts`, palm forward at most, right / left | 13% / 46% ✗ | -14% / -16% ✓ |
| `rest.ts`, wrist out from the hip, right / left | 28 / 30 cm ✗ | 18 / 17 cm ✓ |
| `moves.ts` hands, deepest into any template | -7.5 cm | -0.8 cm |
| Seam, worst joint | 0.030 cm | 0.000 cm |
| Ball of the foot, left / right, range | 2.0 / 2.0 cm | 0.4 / 0.0 cm |
| Hips / chest / head, range | 2.5 / 3.1 / 5.2 cm | 6.0 / 3.6 / 4.6 cm |
| Hands, left / right, range | 11.7 / 6.1 cm | 5.1 / 5.1 cm |

**Does it still blend?** The game crossfades from the idle into the walk, jog, guard and the rest, from whatever moment it is in. `blend.ts` takes, for each of the idle's moments, the nearest pose of the other clip by the head, shoulders, elbows, hands and knees about the hips. From the start pose: walk 8.5 → 4.5 cm, jog 14.9 → 14.9, `Sword_Idle` 21.1 → 23.8, `Low_Guard` 21.6 → 22.0, `Interact` 8.7 → 10.9. Nearer the walk by half, the others within 3 cm of today's; the arms nearer the sides are what move it from the guard and the reach.

Not carried by a clip swap: the carry poses made of the idle (`Carry_Torch` and the rest, `hold.py`) and its arm swing (`Idle_Loop.Arm_*_Add`, `additive.ts`) are built from `Idle_Loop` by the `fight` step, so landing the clip means rebuilding them from it and checking every carried look again (`moves.ts --carry`). The in-game film below swapped the clip and rebuilt the arm swing, not the carry poses.

**In the game.** The first time the flow reached the game: the clip put into `base.glb` and `fight.glb` on a working copy (`glb_clips.ts`, `additive.ts`, `markers.ts`), the player filmed standing on the yard with empty hands (an empty slot pressed, since he starts holding the hammer, whose carry pose overrides the arm), from one fixed camera (`--cine`), then today's the same way, and the files put back. The game's pose layers (feet, gaze, the fit) run over the clip and change nothing the eye catches: the arms stay near the sides, the shoulders low, the pack's bedroll now showing over the lowered shoulder. A camera's `lens` in a `--cine` shot is not a 35 mm lens: 35 gives 22.6 degrees high, so a man at 3.4 m wants about 20. The look is the developer's to judge.

## The idle's arms, from a person at ease

2026-11-02. The developer, on the in-game film: "the arms still dont look natural?", then that the sheet's arms look wonky too. The sheet had been drawn with a render of our start pose as its reference, so it copied our arms. So the arms' bars came from people, not clips or sheets, and a new sheet was drawn from the brief alone (`idle-sheet-2.jpg`: no reference image, the arms written out: upper arm against the side, elbow a little bent and pointing back, hand resting against the outer thigh, palm in, thumb forward, fingers loosely curled).

**The bars** (`rest.ts`): an upright CT of 79 adults standing at ease with "the palms touching the lateral sides of the thighs" (Yoshida et al. 2022, J Orthop Surg Res 17:239, PMC9013055): the arm 4.5 degrees out at the shoulder (IQR 1 to 8), the elbow bent 15.5 (13 to 18), the forearm mid-turn, the arm turned in 9. The resting hand's finger cascade (Lee and Jung 2014, Int J Occup Saf Ergon 20:91). Gaps are skin to skin on every body template, from the move check's skin.

**The bought idles are no bar.** Measured with the same tool, they all hold the arms off the body: Quaternius UAL1's `Idle_Loop` 17 to 20 degrees out, its fingers near a fist (155); the Uthana takes 7 to 25 out, palms turned back 40 to 60 (their prompt asked for arms "a little away from the body"). Arms held out clear every body, which is why bought clips do it.

| | `Idle_Loop` | `Sheet_Idle` before | `Sheet_Idle` now |
|---|---|---|---|
| Upper arm out | 12..20° ✗ | 3..8° | 6..9° |
| Elbow bent | 3..16° ✗ | 4..11° ✗ | 17..20° |
| Palm from facing the thigh | -8..30° ✗ | -14..-8° | -18..-11° |
| Fingers (middle, two joints) | 6..74° ✗ | 52..71° | 72..77° |
| Hand gap, male / female | 15..19 / 10..13 cm ✗ | 4..8 / 0..5 cm ✗ | 1..4 / -2..2 cm (game: -1..3) |
| `moves.ts` hands | -7.5 cm | -0.8 cm | 0.8 cm (bar 2) |
| Blend to the walk, first pose | 8.5 cm | 4.5 cm | 3.8 cm |

**What changed.** `key.ts` `rest_arm` poses both arms whole from the bind pose (out, forward, turned in, elbow, palm; the same both sides), `fingers` curls each finger in the resting cascade; `arms_in` and `palms` are gone. The new sheet's panel 1 measured by eye: upper arm 5 degrees out, the forearm foreshortened for a bend of 15 to 20, hands touching the thighs, palms in, fingers curled; it passed the bars, so it was keyed (out 7.5 on our broader body, turned in 20, elbow 18, fingers 90). Its other panels barely move, so the weight shifts stay the keys' (as the idle note above says), the glance taken from its panel 4 (15 degrees). The start levelled the hips and set the feet level front to back: the guard's tilted hips and staggered feet had put one hand 2 cm nearer its thigh than the other.

**Every body.** The hand touches the thigh on the widest hips and hangs 2 to 4 cm off it on the thin body; the hips' sway moves it 3 cm more. One clip can't touch every thigh, so the clip is posed for the middle and the game's per-body correction (D535's hip keeping, `rest.ts --arms`) turns the arm out where a wide body would take the hand in: as the game composes it, every template passes. The clip alone has the right hand 2 cm into the female thigh at the deepest of the sway (bar 1.5).

**The clothes.** The shirt's swollen shoulders were not changed and are a later job; with the arms against the sides they will show more, since the sleeve now sits on the ribs.
