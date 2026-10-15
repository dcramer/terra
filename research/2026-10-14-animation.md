# Animation: a dressed run that looks right, and creatures with bodies of their own

2026-10-14. **Questions:**
1. Why does the dressed character's run look wrong, and what makes it look right?
2. How do we animate creatures whose bodies are not a person's (the Worman, the Drillhog, the Seawish, D250) without an animator?

## What is wrong with the run

Seen on the workbench (a contact sheet of `Jog_Fwd_Loop` and `Sprint_Loop`, the base body in coat, cap and small pack, eight moments from the side and the front): the body leans about 30 degrees forward, the head juts, the shoulders ride up, the elbows flare and the fists pump at chest height. It reads as an ape charging, not a traveller running. The jog and the sprint are the same clip played at two speeds.

Measured, with `tools/characters/gait_check.py`, a ruler made for this, against the mannequin the clips were made on:

| Clip | Body | Arms in torso | Hands (hips 0, neck 1) | Elbow flare | Lean | Shoulders | Foot slide |
|---|---|---|---|---|---|---|---|
| Walk | mannequin | 0% | 0.06 | 0.14 | 12 deg | 0.15 | 6.7 cm |
| Walk | base | 0% | 0.12 | 0.15 | 12 deg | -0.08 | 8.0 cm |
| Jog | mannequin | 0% | 0.43 | 0.25 | 25 deg | 0.36 | 2.8 cm |
| Jog | base | 0% | 0.43 | 0.27 | 28 deg | -0.06 | 3.6 cm |
| Sprint | mannequin | 0% | 0.23 | 0.24 | 39 deg | 0.43 | 1.8 cm |
| Sprint | base | 0% | 0.27 | 0.26 | 41 deg | -0.06 | 5.4 cm |

1. **The clip reaches our body faithfully.** Each clip is copied onto every body bone by bone (`tools/characters/export_character.py`), which is safe only if the bones rest alike, and they do: every rest frame in `base.glb` matches `mannequin.glb`'s to 0.0 degrees, and through the jog every bone points the same way within 0.0 degrees but the forearms, 3 to 5 degrees on average from the twist step. In the game, read over the remote protocol, the clavicles, upper arms, spine, head and pelvis turn exactly as the mannequin's. Hands, elbows and lean match within a few hundredths and degrees, and no arm goes into the torso (the test finds 33% when the arms are swung across the chest, so it does see it).
2. **The clip is the fault.** Quaternius's jog leans the trunk 25 degrees and its sprint 39, with the hands pumping at the chest; a real runner leans about 5 to 10 degrees at a steady jog and 10 to 20 sprinting. Stylised motion reads as a cartoon on the slim mannequin and as wrong on a realistic, heavy body. A more natural source is the fix, not a different way of copying.
3. **Our shoulders sit differently.** The base's shoulder joints sit below the clavicles' roots (-0.06 clavicle lengths), the mannequin's well above (0.36): its clavicle is about 12 cm long against the mannequin's 22, under a much heavier torso. It adds to the hunched look; it is the body's build, from its rig, and is for the body, not the clips.
4. **Feet slide** 2 to 8 cm a contact on both bodies, the walk worst: the game's playback-rate matching cannot remove what is in the clip. Foot locking is the fix (below).

Already known and still open (`research/skin-deformation.md`, `research/gear-every-clip.md`, D115): bent elbows and knees lose 20 to 35% of their volume, the feet reach lower than the mannequin's toes in walking and running, and the clips' unchanging joint translations are still to be stripped. The game plays one clip at a time with 180 ms crossfades, matched to speed only by its playback rate (`play/figure.rs`, `pace.rs`): no blend from walk to jog to sprint by speed, no foot IK, no lean (`research/bevy-character-capabilities.md`).

## What shipped games and current tools do

**Humanoid locomotion.** The standard set of fixes over clips (Unreal's Pose Warping, Holden's write-ups): stride warping, which scales the legs' stride to the real speed; foot IK and foot locking, which plant each foot while it is on the ground (Holden, "Inverse kinematics foot locking", 2026-07: contacts where the toe is under 10 cm high and slower than 0.1 to 0.5 m/s, kept by a 5-frame majority vote, released with an inertialised blend); orientation warping, which turns the legs to the direction of travel while the spine counters; lean, a spine tilt from acceleration; and blends by speed between walk, jog and sprint. Motion matching needs tens of minutes of dense locomotion capture, which we do not have.

**Sources of better motion**, for a team with no animator (dates and licences checked 2026-10-14):

| Source | What it is | Can we ship its output? |
|---|---|---|
| NVIDIA [Kimodo](https://github.com/nv-tlabs/kimodo), 2026-03 (v1.1 2026-04) | Text and constraints (keyframes, foot and hand targets, a path on the ground) to humanoid motion, trained on about 700 hours of capture. Runs locally, under 3 GB of graphics memory with its text encoder on the processor | Yes: Apache-2.0 code, most checkpoints under NVIDIA's Open Model License, which allows commercial use; the SMPL-X checkpoint is research only. Outputs its own 77-joint skeleton, to be retargeted |
| Mixamo | Captured humanoid clips, free with an Adobe account | Yes, in a game; not as a clip library. Downloaded by hand from its website |
| Uthana, Meshy, Tripo `animate` | Paid services; text or presets to humanoid motion. Tripo's are 90 of 100 biped presets | Yes, by subscription |
| Tencent HY-Motion 1.0, 2025-12 | Text to motion | No: its licence excludes the EU, the UK and South Korea, and needs 24 GB |
| MDM, MoMask, MotionGPT | Text to motion | No: trained on AMASS, research only |
| GVHMR, WHAM, 4DHumans | Motion from an ordinary video | No: GVHMR is research only, the others output SMPL, which is non-commercial without a paid licence. Move One ($50 a month) and Rokoko Vision are paid services that can |
| Cascadeur 2026.2 | Animation tool with physics-assisted posing (AutoPhysics) and inbetweening | Yes, about $96 a year; still needs someone to pose the keys |

**Bodies that are not people.** No released model makes motion for an arbitrary skeleton that we could ship: AnyTop (2025, MIT code) needs example clips of the same skeleton and has no text control, and its training data's licence is unpublished. Games animate such bodies procedurally, over a few hand-made poses:

- **Spore** (Hecker et al., 2008): motion authored once on a generic body with labelled limbs, fitted to any body at runtime by IK. The nearest precedent for many body plans sharing one behaviour.
- **Overgrowth** (Rosen, GDC 2014): a handful of key poses per action, blended by code from the speed and smoothed with springs.
- **Rain World**: a few physics points held at fixed distances, the limbs and tail drawn and animated over them.
- **Snakes and worms**: each spine bone follows the path the head took (follow the leader), with no slip; inching movement is a wave of shortening running down an anchored body.
- **Fish**: a travelling wave along the spine, its swing growing toward the tail.
- **Secondary motion**: spring bones or Verlet chains for hair, weed, cloth ends, as VRM and Godot's SpringBoneSimulator3D do.

Learned physics controllers (DeepMimic to NVIDIA's MaskedMimic and ProtoMotions, 2018 to 2025) are not for us: they are tied to the simulator they were trained in, need a network evaluated per character every tick, for a thousand players under one server, and need example motion our monsters do not have. An active ragdoll, joint motors pulling toward the animated pose, is the cheap physics option, for hit reactions later.

**Bevy 0.20** has an animation graph with blend and additive nodes and masks, and nothing else of the above: no IK, root motion, retargeting or spring bones. `bevy_skeleton_modifiers` (2026-10-09, MIT, Bevy 0.19) ports Godot's two-bone, FABRIK, CCD and spline IK and spring bones, tested against Godot's own output; it is days old. Godot 4.6's IK modifiers (MIT, 2025-12) are the port source D196 points at, with ozz-animation (MIT), whose samples now use Quaternius's library, our clips.

## What we would do

For the run, in order, each judged by a number an agent reads, not by eye alone:

1. **A locomotion ruler.** Made: `tools/characters/gait_check.py`, above. Still to add: the clip's speed against the body's, feet below or above the ground, pops across a blend.
2. **A natural run.** Try a Kimodo jog and run, and Mixamo's captured ones, retargeted onto our skeleton, against Quaternius's on the ruler (lean near a real runner's, hands lower, slide under 1 cm) and on a sheet side by side. A retarget that keeps hands and feet where the source body had them, as Unreal's IK Retargeter does, only if a source's proportions are far from ours.
3. **In the game:** blend walk, jog and sprint by speed (the graph's blend nodes), then foot IK and locking, lean and stride warping, ported from Godot 4.6 and Holden's article (MIT).

For creatures, one shared set of parts, and each creature data that says which of them it uses (D136):

- **A rig made from the model:** a chain of bones along the body's middle (a worm, a tail), and chains for limbs, fins and fronds; Tripo's `rig --type other` where it covers the body (quadrupeds, serpents, fish), our own chain maker in Blender where it does not.
- **Movers:** a spine that follows its own path (the Worman's crawl), a travelling wave (the Seawish's tail in water, a hop on land), legs placed by IK on a gait (the Drillhog's trot), spinning parts (its drills).
- **Springs** for what trails: the Seawish's weed, the Worman's sway, a cap's brim.
- **Keyed poses on top,** three to six an action (wind-up, strike, recover, hurt, death), and the arms of the Worman and the Seawish borrowed from Quaternius's humanoid clips through a mask on the arm chains, so a knife or sword attack costs no new animation.
- Measured by: the base's slip while the body crawls, foot slide for legs, bone stretch from springs (under 1%), settling time, and the body passing through itself.

## First trial: a Kimodo run on our body

Kimodo's text encoder is Meta's Llama 3 8B, gated on Hugging Face, so a developer's account has to accept Meta's licence and give a token before Kimodo makes anything new. Its bundled examples need neither. One, made from "A person runs forward and then leaps over an obstacle", was taken one stride long (frames 47 to 66, left foot to left foot, by its own contact labels, 3.9 m/s), turned into a BVH on its 77-joint skeleton with Kimodo's code, and carried onto our skeleton in Blender: each bone given the source bone's world rotation relative to its T-pose, on our bone's T-pose, the hips' travel scaled by leg length (0.99) and taken off along the ground so it runs in place.

| Clip on the base body | Hands | Elbow flare | Lean | Foot slide |
|---|---|---|---|---|
| Quaternius's jog | 0.43 | 0.27 | 28 deg | 3.6 cm |
| Kimodo's run-up stride | 0.32 | 0.22 | 16 deg | 5.8 cm |

On a sheet side by side, dressed, the Kimodo runner is upright, its arms swing close to the body, its stride is a person's; Quaternius's hunches and pumps. The lean is still above a steady jog's, being a run-up to a leap, and the feet slide more because one cut stride does not loop and its travel was taken off at its average speed. A steady jog and run made for the purpose, cut at matching contacts and looped, with the travel taken off frame by frame, is the next step; then foot locking.

## Second trial: a walk, jog and sprint made by Kimodo for us

Kimodo-SOMA-RP-v1.1 run locally, its text encoder on the processor (about 16 GB of memory, 25 s to load; the gated Llama 3 weights from NousResearch's ungated copy, which Meta's licence allows to be redistributed), the motion model on the graphics card (about 3 GB). Each prompt held to a straight path on the ground at a set speed, six seconds, three samples: "A person walks forward at a steady, relaxed pace, arms swinging naturally" at 1.4 m/s, "A person jogs forward at a steady, comfortable pace" at 3.2 m/s, "A person runs forward fast at full speed" at 6 m/s. From each take, the stride from one left heel strike to the next (by Kimodo's contact labels) in the steady middle whose first and last poses match best: 0.45 to 1.3 degrees apart on average per joint for walks and jogs, 3 to 4.5 for sprints; one sprint had no clean stride. Each was retargeted as in the first trial, closed into a loop, and measured; the ruler's foot contact was corrected on the way (a foot is planted near its lowest only while moving back under the body: Kimodo's walk swings the foot within 6 cm of the ground, which the first rule took for a contact).

| Clip on the base body | Hands | Elbow flare | Lean | Foot slide, worst moment |
|---|---|---|---|---|
| Walk, Quaternius | 0.12 | 0.15 | 12 deg | 8.0 cm |
| Walk, Kimodo | 0.02 | 0.12 | 0.4 deg | 4.5 cm |
| Jog, Quaternius | 0.43 | 0.27 | 28 deg | 3.2 cm |
| Jog, Kimodo | 0.53 | 0.19 | 7.8 deg | 5.7 cm |
| Sprint, Quaternius | 0.27 | 0.26 | 41 deg | 3.5 cm |
| Sprint, Kimodo | 0.51 | 0.22 | 7.1 deg | 13.1 cm |

On sheets side by side, dressed, the Kimodo walk and jog read as a person walking and jogging, upright, arms close; Quaternius's as stiff and hunched. The Kimodo sprint is upright and quick but its arms stay tucked at the chest and it leans less than a real sprint (10 to 20 degrees), a hard run more than a sprint: a prompt asking for driving arms is the next try. Feet slide about as much as Quaternius's, a few centimetres; foot locking in the game removes it from either.

## Third trial: the sprint, and the prompt read properly

The text encoder wraps each prompt in Llama 3's chat format only when the model calls itself `meta-llama/Meta-Llama-3-8B-Instruct`; pointing the LLM2Vec adapters at a local copy had renamed it, so the walk and jog above were made from prompts read without that format, and the straight path did most of the work. With the name restored (only the adapters' `base_model_name_or_path` points at the copy), "A person sprints forward at full speed, leaning forward, arms driving hard back and forth" at 6.5 m/s gave three takes plainly following the words: one leaning 21.5 degrees with the hands at 0.33, one 31 degrees, one upright with the hands high. The first is kept: a real sprinter's lean is 10 to 20 degrees, Quaternius's 41. The walk and jog already measure and look right and are kept as made.

The made clips are part of the build: `tools/characters/made_clips.py` puts them into the mannequin file in place of the library's, the upper arm's and thigh's roll handed to the forearm and shin so they pass `clip_check.py` (0 degrees of upper-limb roll; the jog had 34, over the 30 our skinning takes), the other 23 clips coming through to the frame unchanged; `motion.ts` makes and measures a clip's takes (D316).

## Not settled

Prices for Cascadeur and the commercial terms of Rokoko Vision were not confirmed. The figure of 13 key poses for Overgrowth's whole character comes from a forum post.
