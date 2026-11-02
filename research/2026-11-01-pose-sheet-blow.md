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

The old film is from an older main, and taking out the drive changes the fight's length, so the foot row is an indication and not a clean before and after. The drop in reach and contacts is the cost of the blow standing still.

## What the test does not show

- **A step.** The bought blows' slide came largely from moving the body over feet the clip kept still, and this clip was tested with that drive removed. Whether a keyed step plus `drive` holds up is untested.
- **Other blows and chains.** Only the slash changed; a chain into `Open_Backhand` was played but not judged.
- **The gates a blow must pass.** `moves.ts` was not run, so the wrist's bar and the sword through its own shield are unchecked. The film's shield clears by eye only.
- **The look.** Waiting on the developer, on the review desk (`pose-sheet-slash`).

## Why this fits many clips

This is how game animators work: block the key poses, then time them. The sheet stands in for the thumbnail sketches. Two pieces make it repeat: a control set the agent can write in plain places and angles, and a gate (`mesh.ts`, `moves.ts`) before any film. Nothing generates the motion, so nothing has to be cleaned up afterwards, and a wrong blow is a changed number in a short text file. It leans on D565's reference item per class: keyed once per class, then played by every item in the class's envelope.

## Limits

- One sheet, one blow, one agent reading it. A second sheet of the same blow would differ, and the keys are one reading of it.
- The body's short arms (0.44 m shoulder to wrist) keep the wind-up lower than the sheet's.
- The forearm's roll and the fingers are simple: half the wrist's turn goes up the forearm, and the fingers hold the guard's grip.
- `key.ts` knows the player's body and one guard; other bodies and templates were not tried.
