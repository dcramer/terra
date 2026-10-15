"""A stride carried onto a body by where its limbs point and where its feet land, not by its
bones' rotations, for the clips that ask for it (`"step"` in art/animation/made.json), as
Unreal's IK Retargeter and Unity's Humanoid retarget onto a reference pose and then place
the feet.

Each body's clips are the mannequin's, copied bone by bone (export_character.py), which is
exact only where the two skeletons' joints stand alike in the T-pose. Ours do not: Tripo
places a body's joints in its skin, and the base's rest legs splay 6 degrees out at the hip
and 5 at the knee, its feet turn out 15 and its forearms bend 16 forward, where the
mannequin's limbs run straight along their bones. Copied by rotation, a clip keeps all of that,
and the jog on the base came down on feet 36 cm apart, toes out: a waddle, the "needs to poop"
run (research/2026-10-15-run.md). So, for each such clip, on each body:

  1. aimed: each limb segment points, every frame, exactly as the mannequin's does: the turn
     that takes the body's rest segment onto the mannequin's, in the bone's own frame, is added
     to the bone and taken off its child again (a foot is only turned about the up axis: how
     far it pitches from ankle to ball is its shape, not the stance; a hand, wrist to knuckles,
     so one the body holds bent at rest still carries the fist in line with the forearm);
  2. narrowed: a runner's feet land nearly on one line under the body, 5 to 10 cm apart, where
     the takes' land about 19. Each leg is turned in at the hip, about the body's forward line,
     by the one angle that brings the feet down the clip's `step` apart, ankle to ankle, the
     foot kept flat, as a leg IK pass placing the footfalls would, without moving them along
     or up. One angle a clip, so nothing pops.
"""

import math

import bpy
from mathutils import Quaternion, Vector

# Each segment, from its bone's joint to the next.
SEGMENTS = [(f"{b}_{s}", f"{c}_{s}") for s in "lr" for b, c in (
    ("thigh", "calf"), ("calf", "foot"), ("foot", "ball"),
    ("upperarm", "lowerarm"), ("lowerarm", "hand"), ("hand", "middle_01"),
)]
STANCE_M = 0.03     # an ankle this near its lowest in a clip is on the ground


def segments(arm):
    """Each segment's line in the world at `arm`'s rest (the importer turns the armature so
    the world's Z is up, which its own axes may not be)."""
    b, mw = arm.data.bones, arm.matrix_world
    return {n: (mw @ b[c].head_local - mw @ b[n].head_local).normalized() for n, c in SEGMENTS if n in b and c in b}


def turns(arm, mannequin):
    """Per bone, the turn in its own frame that points `arm`'s rest segment as `mannequin`'s
    (each the segments of `segments`)."""
    ours = segments(arm)
    out = {}
    for n, line in ours.items():
        if n not in mannequin:
            continue
        a, b = line.copy(), mannequin[n].copy()
        if n.startswith("foot_"):
            a.z = b.z = 0.0
            a.normalize()
            b.normalize()
        rest = (arm.matrix_world.to_3x3() @ arm.data.bones[n].matrix_local.to_3x3()).to_quaternion()
        out[n] = rest.inverted() @ a.rotation_difference(b) @ rest
    return out


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


def aim(arm, action, q):
    """Step 1 on one clip: each bone of `q` turned by it, its children kept where they were."""
    bones, pose = arm.data.bones, arm.pose.bones
    # A bone's turn is its parent's, its rest relative to its parent, then its own key:
    # adding Q to a bone and keeping its child where it was is, on the child's key, the
    # parent's Q taken off in the child's rest frame.
    touched = {n for n in q} | {c.name for n in q for c in bones[n].children}
    play(arm, action)
    frames = []
    for f in frames_of(action):
        bpy.context.scene.frame_set(f)
        frames.append((f, {n: pose[n].matrix_basis.to_quaternion() for n in touched}))
    for f, keys in frames:
        for n in touched:
            k = keys[n]
            parent = bones[n].parent
            if parent is not None and parent.name in q:
                rest = rest_in_parent(bones[n])
                k = rest.inverted() @ q[parent.name].inverted() @ rest @ k
            if n in q:
                k = k @ q[n]
            key(pose[n], k, f)


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
    """Step 2 on one clip: each leg turned in at the hip so the feet land `step` apart. Returns
    the step before and after, in metres, and the turn, in degrees."""
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


def bake(arm, actions, mannequin, steps):
    """Aims and narrows each of `actions` named in `steps` (clip: step width in metres) on
    `arm`, against `mannequin`'s rest segments."""
    q = turns(arm, mannequin)
    ad = arm.animation_data or arm.animation_data_create()
    keep = (ad.action, getattr(ad, "action_slot", None))
    for action in actions:
        name = action.name.split(".")[0]
        if name not in steps:
            continue
        aim(arm, action, q)
        before, after, angle = narrow(arm, action, steps[name])
        print(f"AIM {name}: limbs aimed ({', '.join(f'{n} {math.degrees(q[n].angle):.1f}' for n in sorted(q) if n.endswith('_l'))} deg, left);"
              f" feet down {before * 100:.1f} cm apart, now {after * 100:.1f} ({angle:.1f} deg in at the hip)")
    ad.action, slot = keep
    if slot is not None:
        ad.action_slot = slot
