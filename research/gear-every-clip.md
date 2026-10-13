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

**The coat seemed to let the thigh through its hem** whenever the hip bent deeply:

| Body | Clip | Counted as through |
|---|---|---|
| Base | Crouch-walk | 2.2% |
| Heavy | Crouch-walk | 2.5% |
| Strong | Getting up from sitting | 2.3% |

The clipping fixer could not clear it: its first push made it worse (3.4%), and it stopped. Three fixes were tried, and the third found the cause:

1. **A corrective shape** weighted by the thigh's turn, as Unreal's Pose Driver does. Taught from a thigh raised in a rest pose, it had nothing to fix. Taught from the clips' own poses, every push made it worse: in a deep bend the hem sits in the crease between belly and thigh, and a push out of one part goes into the other.
2. **The hem at the hip joint**, where the dressed vest ends, 12 cm above the cut. Worse, up to 10.5% in the roll: an edge in the crease is folded over by the belly.
3. **Looking at it.** Rendered, the crouch looked as a crouch does: the hem tucked behind the raised thigh, as on a real body.

The check had been watching any uncovered skin near the garment, including the thigh below the hem. Skin that was never under a garment cannot come through it; it can only pass in front of it. The check now watches only the skin the garment covers at rest (a ray out along the skin's facing meets it). With that, the coat as cut passes every clip, worst 1.1%; the hem at the hip still fails, so the hem stays where it was. The corrective code was removed, nothing needing it; long coats may (phase 5).

**The base's own looks were never pushed clear.** The clipping fixer ran on every morph but not on the base itself. Now the base gets the same rounds, carried onto itself as a morph of nothing.

**A tool fault hid all this in pictures.** `look.close_up` named its renders by process and side, not by body. In a comparison of two bodies the second's pictures overwrote the first's, so every column showed the last body. The two packs looked identical for that reason, and so did one before-and-after of the coat's armhole. Comparisons made with `compare.py --focus` before this fix, on more than one body, should not be trusted. Numbers from the checks were not affected.

## What it changed

- **Checks:** every clip; the floor judged against the mannequin; hiding from the pose sweep; the base fixed as the morphs are.
- **Bodies:** the hips lifted by body size, baked for the base and mixed in the game for the morphs.
- **The plan:** pose-driven correctives stay in phase 5, for long coats and robes, if they need them.
