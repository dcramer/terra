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
