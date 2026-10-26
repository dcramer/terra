# The head on the aim while the body swings: how shipped engines lay a look-at after the clips

2026-10-26, combat round 33. Question: the developer played and saw "my head turns like 45 degree to the left when aiming forward and swinging"; the composed pose (D530) measured the head 40.5° left of the aim at worst in the backhand, 35° on average through the blows, 26° on guard, because the clips turn the head with the chest. How do shipped games keep the head on the aim while the chest swings, and what is the least of it that works here?

## Answer

Every engine does it after the clips, in the pose, not in them: a look-at modifier turns the neck and head toward a target after the animation has posed the body, limited to an angle and blended by a weight, so any clip (a blow, an idle, a bought one) keeps the head on the target with no clip re-keyed. Ours (D535, `terra_walker::gaze`): the turn about the body's up alone (yaw), shared 40/60 between `neck_01` and `Head`, the clip's own glance kept up to 8°, at most 70° turned back, let go as the head tilts past 35-60° off upright (a roll). The aim is the body's facing, which is the foe it is locked on, else the camera's ahead (`Motion::facing`).

## Prior art

| Where | What it does | What we take |
|---|---|---|
| Godot 4.4 `LookAtModifier3D` (MIT, `scene/3d/look_at_modifier_3d.cpp`) | A `SkeletonModifier3D`, run after the animation: one bone's forward axis turned toward a target node, a primary rotation axis (yaw) and a secondary (pitch) each with a limit angle and a damping threshold, the turn eased in over a duration when the target changes. | Laid after the clips; the turn about a primary axis; a limit. One bone only: the neck is ours too. |
| Unreal AnimGraph "Look At" node (docs) | A Skeletal Control after the pose: bone to modify, look-at axis, clamp angle, interpolation. Chains are built by several nodes or Control Rig's "Aim" with a weight per bone. | The clamp; several bones. |
| Unity `Animator.SetLookAtWeight(weight, bodyWeight, headWeight, eyesWeight, clampWeight)` | The humanoid IK pass spreads the look over the spine and head by weights and clamps how far it turns. | The turn shared between bones by fixed weights. |
| Lyra (Unreal, docs and sample) | Aim offsets: additive poses keyed by the control rotation's pitch and yaw off the body, laid on the spine and head over locomotion and montages. | The same goal from authored poses; needs keyed aim poses we have not got. |
| Enshrouded (`research/2026-10-24-enshrouded-combat-measured.md`) | The fighter's head stays on the target through a chain while the chest and shoulders swing. | The target behaviour: the head within a glance of the aim. |

## Why yaw alone, the neck and head, and not the spine

The measured fault is the turn about the up (40° left); the clips' nod is their own and reads as effort. Turning the spine would move the shoulders, so the sword's path, the shield and every move-check row; the neck and head carry nothing. A full correction each frame from the clip's own pose (not a smoothed follow) keeps the head steady however fast the chest turns, so a blow cannot yank it; a smoothed follow would lag behind a swing, which is what the developer saw.

## How it is checked

`film.pose.head_yaw_blows` (D530, bar 15°) on the duel's composed pose; `gaze::tests` (a head within the glance of the aim while the chest turns -60° to 60°, turned once though no clip poses it again; a tumbling head let go).
