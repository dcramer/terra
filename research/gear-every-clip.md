# Gear through every clip

2026-10-12. **Question:** with the new realistic base bodies (D186), do the bodies and every worn item still pass when the checks play every clip the game plays, not the six or seven they used to? Where not, why, and what fixes each in the way shipped games do?

## What was checked

The rig, gear and deformation checks played six or seven clips each, and no two lists agreed. None had the sprint, which the player uses all the time, or the crouch-walk, the jumps, sitting or kneeling. They now play all 25 clips in `art/animation/clips.txt` (`look.game_clips`). The bodies are base, heavy, thin, strong and female; the items are the coat, cap, quiver and both packs.

## Findings

**Deformation holds through every clip.** The new base passes every clip's edge and pinch checks. The twist step and the weight-blended skinning hold up on clips they were never tuned on.

**The floor test was wrong for clips that touch the floor.** The mannequin itself goes into the floor in many clips:

| Clip | Mannequin's lowest point |
|---|---|
| Walk_Loop | −2.7 cm |
| Sprint_Loop | −2.9 cm |
| Roll | −4.9 cm |
| Fixing_Kneeling | −5.4 cm |
| Death01 | −7.6 cm |
| Jump_Start | −14.8 cm |

So the floor test now asks each body to go no deeper than the mannequin plus 2 cm, per clip. The new base went 4.9 cm past that in the roll, being thicker. With the hips lifted by body size (`tools/characters/lift.py`), it passes every clip:

| Clip | Lift |
|---|---|
| Roll | up to 6.7 cm |
| Death01 | up to 6.6 cm |
| Jog_Fwd_Loop | up to 4.3 cm |
| Fixing_Kneeling | up to 3.7 cm |
| Walk_Loop | up to 1.7 cm |

Locomotion lifts for a different reason: the longer feet reach lower than the mannequin's at the toes. The full answer there is foot IK, the rest of phase 4.

**The packs opened holes.** They hid skin that showed when the pack lifted off it: up to 6.3% of the hidden skin, falling dead and in the punch and jump landing.

- Hiding now comes from a sweep of poses (T6, conservative): a patch is hidden only while it stays covered in every frame of every clip on every body.
- Each pack gave up 3 patches, and the holes went to none.

**The coat lets the thigh through its hem**, whenever the hip bends deeply:

| Body | Clip | Skin through |
|---|---|---|
| Base | Crouch-walk | 2.2% |
| Heavy | Crouch-walk | 2.5% |
| Strong | Getting up from sitting | 2.3% |

The skin coming through lies at the hem line itself (0.82–0.89 m up, the hem at about 0.84 m). That tells us three things:

- **Finer hiding cannot fix it:** any hiding safe from holes must leave that skin drawn.
- **The clipping fixer cannot fix it either:** its first push made it worse (3.4%), and it stopped. A push baked into the shape moves the hem in every pose, not only when the thigh is up.
- **The fix shipped games use is a pose-driven corrective:** a corrective shape on the garment, its weight set each frame from the thigh's angle, as Unreal's Pose Driver and MetaHuman's correctives do.

**The base's own looks were never pushed clear.** The clipping fixer ran on every morph but not on the base itself. Now the base gets the same rounds, carried onto itself as a morph of nothing.

**A tool fault hid all this in pictures.** `look.close_up` named its renders by process and side, not by body. In a comparison of two bodies the second's pictures overwrote the first's, so every column showed the last body. The two packs looked identical for that reason, and so did one before-and-after of the coat's armhole. Comparisons made with `compare.py --focus` before this fix, on more than one body, should not be trusted. Numbers from the checks were not affected.

## What it changed

- **Checks:** every clip; the floor judged against the mannequin; hiding from the pose sweep; the base fixed as the morphs are.
- **Bodies:** the hips lifted by body size, baked for the base and mixed in the game for the morphs.
- **The plan:** pose-driven correctives (docs/characters.md, phase 5) are the next fix, first for the coat's hem.
