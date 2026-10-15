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

## Not settled

Kimodo's output on our skeleton, and how close a Kimodo or Mixamo run comes to the look we want, is unknown until tried. Prices for Cascadeur and the commercial terms of Rokoko Vision were not confirmed. The figure of 13 key poses for Overgrowth's whole character comes from a forum post.
