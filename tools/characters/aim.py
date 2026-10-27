"""A stride's feet landing a step apart, for the clips that ask for it (`"step"` in
art/animation/made.json or a set's list), as a leg IK pass placing the footfalls would.

Every clip is carried onto a body with its limbs pointing as the mannequin's do (retarget.py).
A runner's feet land nearly on one line under the body, 5 to 10 cm apart, where the takes' land
about 19 (research/2026-10-15-run.md). So each leg of such a clip is turned in at the hip,
about the body's forward line, by the one angle that brings the feet down the clip's `step`
apart, ankle to ankle, the foot kept flat, without moving them along or up. One angle a clip,
so nothing pops.
"""

import math

import bpy
from mathutils import Quaternion, Vector

STANCE_M = 0.03     # an ankle this near its lowest in a clip is on the ground


def play(arm, action):
    ad = arm.animation_data or arm.animation_data_create()
    ad.action = action
    if getattr(action, "slots", None) and len(action.slots):
        ad.action_slot = action.slots[0]


def frames_of(action):
    start, end = (int(round(f)) for f in action.frame_range)
    return range(start, end + 1)


def rest_in_parent(bone):
    return (bone.parent.matrix_local.to_3x3().inverted() @ bone.matrix_local.to_3x3()).to_quaternion()


def key(pb, q, f):
    pb.rotation_mode = "QUATERNION"
    pb.rotation_quaternion = q
    pb.keyframe_insert("rotation_quaternion", frame=f, group=pb.name)


def footfalls(arm, action):
    """Where each foot comes down across (world X), its mean through its stance, and how high
    its hip stands over it then; with each frame's turns and places of the legs, to change."""
    pose, mw = arm.pose.bones, arm.matrix_world
    names = [f"{b}_{s}" for s in "lr" for b in ("thigh", "calf", "foot")] + [pose["thigh_l"].parent.name]
    play(arm, action)
    frames = []
    for f in frames_of(action):
        bpy.context.scene.frame_set(f)
        frames.append((f, {n: pose[n].matrix.to_quaternion() for n in names}, {n: mw @ pose[n].head for n in names}))
    out = {}
    for s in "lr":
        low = min(p[f"foot_{s}"].z for _, _, p in frames)
        down = [p for _, _, p in frames if p[f"foot_{s}"].z < low + STANCE_M]
        out[s] = (sum(p[f"foot_{s}"].x for p in down) / len(down), sum(p[f"thigh_{s}"].z - p[f"foot_{s}"].z for p in down) / len(down))
    return out, frames


def narrow(arm, action, step):
    """Each leg turned in at the hip so the feet land `step` apart. Returns the step before and
    after, in metres, and the turn, in degrees."""
    pose, bones, mw = arm.pose.bones, arm.data.bones, arm.matrix_world
    to_arm = mw.to_3x3().inverted()
    ahead = (to_arm @ Vector((0.0, -1.0, 0.0))).normalized()
    feet, frames = footfalls(arm, action)
    before = abs(feet["l"][0] - feet["r"][0])
    inward = max(0.0, (before - step) / 2)
    turn = {}
    _, _, at = frames[0]
    for s, (x, height) in feet.items():
        other = feet["r" if s == "l" else "l"][0]
        angle = math.atan2(inward, height)
        # Whichever way round the forward line moves this ankle toward the other foot.
        leg = at[f"foot_{s}"] - at[f"thigh_{s}"]
        moved = mw.to_3x3() @ (Quaternion(ahead, angle) @ (to_arm @ leg))
        toward = (moved.x - leg.x) * (other - x) > 0
        turn[s] = Quaternion(ahead, angle if toward else -angle)
    hips = pose["thigh_l"].parent.name
    for f, w, _ in frames:
        for s in "lr":
            thigh, calf, foot = f"thigh_{s}", f"calf_{s}", f"foot_{s}"
            r = turn[s]
            # The thigh turned, the calf following it, the foot's own turn in the world kept.
            key(pose[thigh], rest_in_parent(bones[thigh]).inverted() @ w[hips].inverted() @ r @ w[thigh], f)
            key(pose[foot], rest_in_parent(bones[foot]).inverted() @ (r @ w[calf]).inverted() @ w[foot], f)
    after, _ = footfalls(arm, action)
    return before, abs(after["l"][0] - after["r"][0]), math.degrees(turn["l"].angle)


def bake(arm, actions, steps):
    """Narrows each of `actions` named in `steps` (clip: step width in metres) on `arm`. A take
    tried as `X#N` (the clip step, clip.ts) is X's."""
    ad = arm.animation_data or arm.animation_data_create()
    keep = (ad.action, getattr(ad, "action_slot", None))
    for action in actions:
        name = action.name.split(".")[0]
        base = name.split("#")[0]
        if not steps.get(base):
            continue
        before, after, angle = narrow(arm, action, steps[base])
        print(f"STEP {name}: feet down {before * 100:.1f} cm apart, now {after * 100:.1f} ({angle:.1f} deg in at the hip)")
    ad.action, slot = keep
    if slot is not None:
        ad.action_slot = slot
