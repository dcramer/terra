# Why the base body's torso looks wrong

2026-11-01. The developer, on the idle's stills: "whys our models torso look weird af". Question: which of the body's proportions are really off, where each fault came from, and how to fix it. This note changes no asset; the choice is [D593](../docs/decisions/D593.md), Open.

## In plain words

- **The arms are too short.** On a real 1.80 m man the fingertips hang at mid-thigh, 67 cm off the floor. Ours stop at the crotch, 82 cm. That is the biggest fault, and it makes everything else look worse.
- **The torso's bones are fine; its skin is not.** The bones from hip to shoulder are the same length as the mannequin's. But the crotch sits about 5 cm low and the top of the shoulders about 4 cm high, so the part you see reads about 9 cm long.
- **The build is very heavy on top.** The chest and shoulders are as wide as the widest 1 in 100 men, over an ordinary waist.
- **The notch is the arm's skinning.** The skin under the armpit is tied partly to the arm and collarbone, so lowering the arm pulls it into a step. It is not in the model's shape.
- **The shrug is mostly the animation.** `Idle_Loop` holds the shoulders about 6 cm higher than `Walk_Loop`, on the mannequin too.
- **It came from the drawing.** Every design sheet we have drew the arms short, and nothing in the pipeline measures proportions.
- **A stretch of the arms in the pipeline fixes most of it, free.** A scratch copy with 25% longer arms and the notch's weights moved looks like a person. It needs a rebuild and a re-check of combat, no new art.
- **Legs 10% longer, the whole man scaled back to 1.80 m, helps too, and moves well.** A second copy has a 1.86 m reach, the crotch at 0.85 m and a smaller head, and walks on the game's clips with its feet closer to the ground than today's. Going on to 17% puts the crotch at 0.87 m but looks stilted, and the hips by then sit 7 cm too high: the crotch is low in the model's shape, not because the legs are short. See [the second test](#the-second-test-longer-arms-and-longer-legs).

## Numbers

The reference is ANSUR II (2012), the US Army's survey: 4,082 men, each measure taken as a share of that man's own height, then multiplied by 1.80 m. It is a survey of soldiers, not an ideal, and it measures skin landmarks, which compare directly with a mesh. Joint-centre tables (de Leva, Drillis and Contini through Winter) could not be read at their source, so none is used here.

| Measure at 1.80 m | Real mean | 1st to 99th percentile | Ours | Reads |
|---|---|---|---|---|
| Reach, fingertip to fingertip | 1.86 m | 1.75 to 1.99 | **1.68** | below the 1st |
| Fingertip height, standing | 0.67 m | 0.61 to 0.73 | **0.82** | beyond the 99th |
| Wrist height, standing | 0.87 m | 0.82 to 0.92 | **1.00 to 1.03** | beyond the 99th |
| Crotch height | 0.87 m | 0.81 to 0.93 | **0.81** | at the 1st |
| Crotch to top of shoulder | 0.61 m | 0.55 to 0.67 | **0.70** | beyond the 99th |
| Top of shoulder | 1.48 m | 1.43 to 1.51 | 1.51 | at the 99th (T-pose) |
| Across the shoulders, arms on | 0.52 m | 0.45 to 0.61 | 0.61 | at the 99th |
| Chest width | 0.30 m | 0.26 to 0.34 | **0.36** | beyond the 99th |
| Chest depth | 0.26 m | 0.20 to 0.32 | 0.31 | high |
| Waist width | 0.335 m | 0.26 to 0.42 | 0.334 | normal |
| Hip width | 0.355 m | 0.30 to 0.41 | 0.39 | high |
| Hand | 0.198 m | 0.18 to 0.22 | 0.194 | normal |
| Hip joint / knee height | 0.92 / 0.50 m | | 0.93 / 0.50 | normal |

Widths are standing in `Idle_Loop`'s first frame, where the legs close up; the T-pose's stand 42 cm wide at the knees and inflate its hip. The head is about 7.35 heads to the body, a little large against the drawn 7.5, and not a fault anyone would see.

Shoulder joint to wrist is 0.445 m. The real figure, from acromion height less wrist height less about 5 cm for the joint below the acromion, is about 0.56 m, not the 0.59 first guessed. The mannequin's is 0.54 m, and its idle's fingertips hang at 0.69 to 0.72 m.

## Where each fault came from

| Stage | Reach / height | Crotch / height | What it did |
|---|---|---|---|
| `sheet.png` | 0.927 | 0.457 | **Drew both faults** |
| `front.png` | 0.928 | | Only crops |
| Tripo's model | 0.965 | 0.452 | Lengthened the arms a little |
| Tripo's rig | 0.965 | 0.452 | Joints follow the skin |
| `convert_rig.py` | 0.932 | 0.452 | Hand step takes 3 cm off each side |
| `base.glb` | 0.932 | 0.452 | |

- **The short arm is in the mesh and in the rig, the mesh first.** The rig only follows it. The hand step is right: the hands were too big, and at 0.194 m they are normal now. It only took the arm's last 3 cm with them.
- **Every sheet does it.** Male 0.927, female 0.921, heavy 0.927, thin 0.930, strong 0.927, troll 0.900. The morphs and the female were drawn with the base's sheet as a reference, so theirs may be one fault copied. The layout may be the deeper cause: three T-poses across a 16:9 image leave a real reach no room. Both are guesses, not measurements.
- **Every template shares it.** The morphs keep the base's arms (`morph.py`), so reach is 0.932 on heavy, thin and strong, and 0.919 on female, who was drawn with the same fault.
- **Nobody asked for it.** D186 and `docs/characters.md` say nothing of proportions, and the prompt asks for "natural anatomy and true proportions". `rig_check.py`'s `reach` is how far a bone drives skin, not a limb's length, and no step measures the sheet.

## The notch

Measured on the flank, arms cut away, as the game skins:

| | Rest | `Idle_Loop` | Weights moved |
|---|---|---|---|
| Biggest step inward going up the flank | none, it only widens | 1.5 and 1.75 cm, at 1.24 to 1.31 m | 0.07 and 0.32 cm |

- Linear or dual-quaternion skinning changes nothing, so it is not the blend.
- Between 1.24 and 1.32 m the rib skin is 18 to 38% clavicle and up to 23% upper arm, 13 cm below the armpit.
- Those weights are Tripo's: its raw rig has them, and our smoothing moves them by 1 to 3 points.
- A T-pose asks the skin to travel 90 degrees to a relaxed arm; rigging forums call the armpit its weakest place and bind nearer 45 degrees. We tried generating in an A-pose in 2026-10 and it crushed the shoulders ([research](ai-models-for-art.md)), so this stays a weights fix.

## The shrug

| Neck joint above shoulder joint | Rest | `Idle_Loop` | `Walk_Loop` |
|---|---|---|---|
| Base | 7.7 cm | 6.7 cm | 12.5 cm |
| Mannequin | 4.6 cm | 1.9 cm | 8.0 cm |

The clip holds the shoulders about 6 cm higher than the walk on both bodies, so the shrug is the clip's. `Sheet_Idle` starts from the same first pose and inherits it. The body adds to it: its T-pose shoulder top is flat and about 4 cm high, and nothing lowers it when the arm comes down.

## Ways to fix it

Shipped games carry proportions in the bones (UMA, Final Fantasy XIV, [T1](character-systems-industry.md)), and Unreal's retargeter carries rotations and scales hand goals by chain. Ours already carries rotations (D542).

| | A: stretch in the pipeline | B: bones scaled in the game (T1) | C: remake from a new sheet |
|---|---|---|---|
| What it is | A step in `convert_rig.py`, as the hand and foot steps are, moving arm joints and skin; the same stretch on each design and dressed model | Arm bones scaled as the game runs, skin as it is | A corrected sheet, Tripo model and rig, every design and dressed model again |
| Look | Arms right; muscle and skin stretch 25% along the arm | The same at best; scale on a parent shears its children, so it needs helper bones | Could fix the chest, shoulders and crotch too, but may be a different man |
| Costs | Free; a job or two, and about half an hour's rebuild | Most code; it buys arm sliders, which D111 doesn't want | About 10 Tripo models (roughly 250 credits at 25 each), rigs and paintings |
| Notch | Separate fix in the same step | Not touched | Tripo's rig may well do it again |
| Repeats? | Fixed for any new body, with the check | Every body needs its numbers | Only if the image model draws arms right, which none of our 6 sheets did |

What longer arms touch, whichever way:

| Thing | What happens |
|---|---|
| Clips (D542) | Rotations carry on free, and hands land nearer where the clips' author put them (idle fingertips 0.71 m, the mannequin's 0.69 to 0.72) |
| `fight.glb` | Rebuilds free from stored takes; `hold.py` solves the shield arm for the new bones. No Kimodo or Uthana run |
| Hand keys in `fight.json` | Metres from the hips, stored: must be judged again with `keys.ts` |
| `moves.ts`, scorecard | `move.*.hands`, `sword_in_shield`, `shield_in_arm` rows will move and must be re-measured |
| Reach | The sword reaches about 11 cm further. `stop_at` values in `sword.shielded.ron` are hand-typed and want re-judging |
| Grips, held items | Nothing: a hand socket is the same on every body |
| Worn gear (D546) | Made from the base's faces, so it follows; colour bakes from dressed models along short rays, so sleeves need those models stretched too. Every look re-passes `gear_check.py` |
| Morphs | Arm joints stay the base's. Each design needs the same stretch first; female gets it through `convert_rig.py` |
| Icons | Gear files change bytes: `cargo xtask icons --keep` where a look didn't change |
| Skin | Texture stretches 25% along the arm: judge it on the skin still |

## Recommendation

**A, with a proportion check.** It is the only way that fixes every body now and every body after, for no credits.

1. A proportion check, failing today's base: reach, wrist and crotch against ANSUR bands, run on the sheet before any credit is spent and in `rig_check.py` after.
2. Arm stretch and flank weights in `convert_rig.py`, the same stretch on designs and dressed models.
3. Rebuild, re-check gear, re-measure combat.
4. Lower the shoulders in the idle, in the clip.

The heavy chest and low crotch are a separate design call. The arm stretch does not reach them, and a local shape change or C would. Legs 10% longer and the whole-body scale, tested below, take the chest to the edge of normal and the crotch to 0.85 m; add them to step 2 if the developer likes the look.

## The scratch

A copy in memory, exported nowhere (`target/job/after.py`): both arm bones 25.5% longer, skin stretched with them, arm and clavicle weight ramped off the ribs between 1.24 and 1.38 m. `Idle_Loop` played on it unchanged.

| | Today | Longer arms, flank freed | And shoulders 3 cm down | Real |
|---|---|---|---|---|
| Reach | 1.68 m | 1.90 m | 1.90 m | 1.86 m |
| Fingertips | 0.82 m | 0.71 m | 0.68 m | 0.67 m |
| Wrists | 1.00 to 1.03 m | 0.90 to 0.92 m | 0.87 to 0.89 m | 0.87 m |
| Flank step | 1.5, 1.75 cm | 0.07, 0.32 cm | 0.38, 1.08 cm | |

The reach overshoots the mean by 4 cm, inside the normal range, because our shoulder joints sit wide. Letting the shoulders down brings part of the notch back, so the weights and that pose want working out together.

## The second test: longer arms and longer legs

The developer asked to try both. Scratch copies again, no game file changed: thigh and shin lengthened together along each leg's own line, the feet moved whole, the arms stretched and the flank freed as above, then the whole body scaled back to 1.80 m. Each copy went through `export_character.py` and so plays clips carried onto its own bones, not a pose borrowed from the old body. An arms-only control keeps today's floor and walk figures to the hundredth, so the route adds nothing of its own.

| At 1.80 m | Today | Arms only | Legs +5% | Legs +10% | Legs +17% | Real mean (1st to 99th) |
|---|---|---|---|---|---|---|
| Whole-body scale | 1 | 1 | 0.978 | 0.956 | 0.929 | |
| Height in heads | 7.35 | 7.35 | 7.52 | 7.69 | 7.92 | |
| Reach | 1.68 m | 1.86 | 1.86 | 1.86 | 1.86 | 1.86 (1.75 to 1.99) |
| Shoulder joint to wrist | 0.445 m | 0.538 | 0.539 | 0.547 | 0.558 | about 0.56 |
| Fingertips, standing | 0.82 m | 0.73 | 0.73 | 0.74 | 0.74 | 0.67 (0.61 to 0.73) |
| Wrists, standing | 1.00 to 1.03 m | 0.92 to 0.94 | the same | the same | the same | 0.87 (0.82 to 0.92) |
| Crotch | 0.81 m | 0.81 | 0.83 | 0.85 | 0.87 | 0.87 (0.81 to 0.93) |
| Hip joint | 0.93 m | 0.93 | 0.95 | 0.97 | 0.99 | 0.92 |
| Knee | 0.50 m | 0.50 | 0.51 | 0.52 | 0.53 | 0.50 |
| Most both feet are off the ground in `Walk_Loop` | 3.5 cm | 3.5 | 2.8 | 2.2 | 1.5 | |
| Deepest into the ground in `Walk_Loop` | 0.05 cm | 0.05 | 0.15 | 0.21 | none | |
| `Walk_Loop`'s pace | 1.38 m/s | 1.38 | 1.41 | 1.44 | 1.48 | |

What it shows:

- **The reach went to the mean, not below it.** Trimming to 1.83 m put the fingertips at 0.75 m, past the 99th. At 1.86 m they stop at 0.74 m, a centimetre past it. Shoulder height and the idle's shrug hold them up, so lowering the idle's shoulders (step 4) is what closes the rest.
- **Longer legs do not float the feet.** They do the reverse: the walk's gap closes as the legs lengthen, and sinking stays about 2 mm. `Idle_Loop` does not change.
- **The crotch is low in the model's shape.** It sits 12 cm under the hip joints, a real man's about 5 cm. Legs alone reach 0.87 m only at +17%, with the hip joint 7 cm above the real height and the knee 3 cm above. A local lift of the crotch would close the last 2 cm from +10% without that; it was not tried.
- **The artist's warning holds at +17%.** By eye, +5% and +10% read as the same man, a little taller in the leg, and +17% starts to look stilted under the heavy chest. +10% is the last that looks right to this eye; that is the developer's call.
- **The scale helps the top half,** by plain arithmetic on the first table: chest 0.36 to 0.34 m, the edge of normal; shoulders 0.61 to 0.58 m; crotch to shoulder top 0.70 to 0.68 m (real 0.61, to 0.67). The top of the shoulder rises from 1.51 to about 1.52 m, a centimetre past the 99th, and hand and foot shrink 4%.
- **No new fault found in the skin.** `rig_check.py` passes on the +10% copy, as on today's base. There is no tear or smear at the hips or thighs, front or back. Its skin was stretched without new normals, a scratch shortcut.

With sword and shield, on the +10% copy, the fight set carried by the same export with its holds (`fight-mannequin.glb`, `hold.py`). The steps after that export were not run on the copy (`clear.ts`, `wrist_limit.ts`, `mirror.ts`, `additive.ts`, `markers.ts`), and today's column is the finished `fight.glb`, so a small part of the difference may be theirs:

| | Today | Legs +10% |
|---|---|---|
| `Low_Guard`: left wrist (shield) | 1.03 m high | 1.04 m; `hold.py` sets the board 0.5 cm off its target |
| `Low_Guard`: right wrist (sword) | 1.04 m high | 1.00 m, and 3 cm further forward |
| `Low_Guard`: deepest foot in the ground | 0.96 cm | 0.49 cm |
| `Carry_Tool`: wrists | 1.02 m | 0.93 to 0.95 m |
| `Carry_Tool`: lowest point of a sword held in it | 0.34 m | 0.27 m |

The carries hang about 8 cm lower, as the hands do, and a long item's end comes 7 cm nearer the ground; nothing touches it. Measured in the middle of each clip, from Blender, with the fingers open as this tool leaves them. More of the sword counts as inside the body in `Low_Guard` (164 of 1,734 points against 40), not located; `moves.ts` must rule on that, and on every other row, against a real rebuild. Not checked: gear, morphs, the game's own renderer.

## Sources

- [ANSUR II public data](https://www.openlab.psu.edu/ansur2/), and its report, Gordon et al., [NATICK/TR-15/007](https://apps.dtic.mil/sti/pdfs/ADA611869.pdf).
- [T-pose against A-pose](https://cgcookie.com/community/20686-t-pose-or-not-to-t-pose-or-a-pose) and [Polycount's practitioners](https://polycount.com/discussion/comment/2582820): forum reports, not measurements.
- Epic: [retargeting](https://www.unrealengine.com/blog/intro-to-animation-retargeting-in-ue4?lang=ja) and [goal scaling](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/IKRetargetScaleGoalsChainSettings).
