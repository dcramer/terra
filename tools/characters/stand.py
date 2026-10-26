"""The stand pass: an idle's hips raised and its head drawn back over them, its feet left where
they stood, for a take that stands with soft knees and its head forward (Uthana's 3.0 idles:
knees 27 degrees, hips 3 cm low, as stance.ts measures, 2026-10-26). An opt-in pass a
clip's entry in made.json asks for with "stand": true, laid over the take as "arms" lays a
runner's arms (D360), driven by the body's own rest alone, nothing set per clip:

  - the hips raised toward the body's rest less DROP, on average through the clip, by
    straightening the knees rather than lifting the hips one height: each frame's limiting knee
    bent KNEE_MIN plus a share `c` of the take's own bend over it, `c` found so the hips come up
    as far as asked (`softness`). A knee near straight turns about a degree for a millimetre
    the hip moves, so a lift of one height amplifies the take's own small steps and a hard
    limit adds corners (2.1 deg a frame on Uthana's take 0, against 0.9 raw); scaling the bend
    keeps each step a share `c` of the take's, and a knee never locks straight;
  - each leg laid back onto its foot by two-bone IK, as Unreal's Two Bone IK node and leg IK
    do (the knee in the plane of the hip, the ankle and the knee it had), each foot turned as it
    was, so a planted foot stays planted;
  - the head's mean forward offset from the hips, against rest, taken out at the neck, the
    head turned back as it was, so the face looks where it looked.

The geometry is plain functions of mathutils vectors (tested in test_stand.py, run in
Blender's Python); `stand` applies them to an action.
"""

import math

from mathutils import Matrix, Quaternion, Vector

DROP = 0.01        # m: the hips stand this far below the body's rest, the knees a little soft
KNEE_MIN = 8.0     # degrees: the least a knee bends, so it never locks straight
UP = Vector((0.0, 0.0, 1.0))


def reach(l1, l2, bend):
    """How far a hip is from its ankle when a thigh `l1` and a shin `l2` meet bent `bend` degrees
    (the thigh's line to the shin's; 0 is straight)."""
    return math.sqrt(l1 * l1 + l2 * l2 + 2.0 * l1 * l2 * math.cos(math.radians(bend)))


def highest(hip, ankle, far, up=UP):
    """How far `hip` may move along `up` and stay within `far` of `ankle` (m): negative when it
    must come down, or, if no height reaches, to its nearest."""
    d = hip - ankle
    b = d.dot(up)
    disc = b * b - (d.length_squared - far * far)
    return -b if disc < 0.0 else -b + math.sqrt(disc)


def two_bone(hip, ankle, knee, l1, l2, least=KNEE_MIN):
    """Where the knee goes for a thigh `l1` and shin `l2` from `hip` reaching for `ankle`, bent
    in the plane of the hip, the ankle and `knee` (where the knee was), and the ankle reached:
    `ankle`, or short of it on the line to it when it lies beyond the reach of a knee bent
    `least` degrees."""
    to = ankle - hip
    axis = to.normalized()
    d = max(abs(l1 - l2) + 1e-6, min(to.length, reach(l1, l2, least)))
    cos_a = max(-1.0, min(1.0, (l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d)))
    side = knee - hip
    side = side - axis * side.dot(axis)
    side.normalize()
    return hip + (axis * cos_a + side * math.sqrt(1.0 - cos_a * cos_a)) * l1, hip + axis * d


def bend(hip, knee, ankle):
    """How far a knee is bent (deg): the thigh's line to the shin's, 0 straight."""
    return math.degrees((knee - hip).angle(ankle - knee, 0.0))


def softened(was, c, least=KNEE_MIN):
    """A knee's bend `was` straightened to `least` plus a share `c` of its bend over it; one
    already straighter is left as it was."""
    return was if was <= least else least + c * (was - least)


def lift_at(frame, legs, c, up=UP):
    """How far the hips of a `frame` ({side: (hip, knee, ankle)}) rise so that the knee that
    limits them bends `softened` by `c`, its feet where they were: the least over the legs."""
    return min(highest(hip, ankle, reach(*legs[s], softened(bend(hip, knee, ankle), c)), up) for s, (hip, knee, ankle) in frame.items())


def softness(frames, legs, rise, up=UP):
    """The share `c` (0 to 1) of each knee's bend over KNEE_MIN kept so the hips rise `rise` on
    average through `frames`, found by halving: none (0) when even knees at KNEE_MIN raise
    them less, the whole (1, no change) when `rise` is none."""
    def mean(c):
        return sum(lift_at(f, legs, c, up) for f in frames) / len(frames)

    if rise <= 0.0:
        return 1.0
    if mean(0.0) <= rise:
        return 0.0
    lo, hi = 0.0, 1.0
    for _ in range(30):
        mid = (lo + hi) / 2.0
        if mean(mid) > rise:
            lo = mid
        else:
            hi = mid
    return (lo + hi) / 2.0


def ahead_of(left, up=UP):
    """Where a body faces on the ground, from its hips' right-to-left `left`: our front is -Y,
    the left +X."""
    fwd = left.cross(up)
    fwd.z = 0.0
    return fwd.normalized()


def drawn_back(v, left, back, up=UP):
    """The turn about the body's `left` line that takes a neck's `v` (its base to the head) `back`
    metres back along where the body faces, found by halving between none and 60 degrees."""
    fwd = ahead_of(left, up)
    axis = left.normalized()
    goal = v.dot(fwd) - back
    lo, hi = 0.0, math.radians(60.0)
    # A turn about the left line by a positive angle takes up toward the front (-Y): back is negative.
    for _ in range(40):
        mid = (lo + hi) / 2.0
        if (Quaternion(axis, -mid) @ v).dot(fwd) > goal:
            lo = mid
        else:
            hi = mid
    return Quaternion(axis, -hi)


def stand(arm, action, times, update, without_roll):
    """The stand pass on `action`, played on `arm`, at its key `times`; `update` refreshes the
    pose and `without_roll` takes a bone's roll out (the thighs never roll: clip_check.py).
    Returns what it did: the hips raised (cm, on average), the share of each knee's bend over
    KNEE_MIN kept, and the head drawn back (cm)."""
    import bpy

    scene = bpy.context.scene
    mw = arm.matrix_world
    pose = arm.pose.bones
    rest = {b.name: mw @ b.head_local for b in arm.data.bones}
    legs = {s: ((rest[f"calf_{s}"] - rest[f"thigh_{s}"]).length, (rest[f"foot_{s}"] - rest[f"calf_{s}"]).length) for s in "lr"}

    def at(t):
        scene.frame_set(int(math.floor(t)), subframe=t - math.floor(t))

    def head_ahead(p):
        return (p["Head"] - p["pelvis"]).dot(ahead_of(p["thigh_l"] - p["thigh_r"]))

    rest_ahead = head_ahead(rest)
    was = []
    for t in times:
        at(t)
        p = {b: mw @ pose[b].head for b in ("pelvis", "Head", "thigh_l", "thigh_r", "calf_l", "calf_r", "foot_l", "foot_r")}
        turns = {f"foot_{s}": (mw @ pose[f"foot_{s}"].matrix).to_quaternion() for s in "lr"}
        was.append((p, turns))
    rise = max(0.0, rest["pelvis"].z - DROP - sum(p["pelvis"].z for p, _ in was) / len(was))
    back = max(0.0, sum(head_ahead(p) for p, _ in was) / len(was) - rest_ahead)
    frames = [{s: (p[f"thigh_{s}"], p[f"calf_{s}"], p[f"foot_{s}"]) for s in "lr"} for p, _ in was]
    c = softness(frames, legs, rise)

    def turn_to(name, q):
        pb = pose[name]
        pb.matrix = mw.inverted() @ Matrix.LocRotScale((mw @ pb.matrix).translation, q, None)
        update()

    lifts = []
    for t, (p, turns), frame in zip(times, was, frames):
        at(t)
        lift = lift_at(frame, legs, c)
        lifts.append(lift)
        pelvis = pose["pelvis"]
        m = mw @ pelvis.matrix
        m.translation += UP * lift
        pelvis.matrix = mw.inverted() @ m
        update()
        pelvis.keyframe_insert("location", frame=t)
        for s in "lr":
            hip = mw @ pose[f"thigh_{s}"].head
            knee, ankle = two_bone(hip, p[f"foot_{s}"], p[f"calf_{s}"] + UP * lift, *legs[s])
            for bone, child, goal in ((f"thigh_{s}", f"calf_{s}", knee), (f"calf_{s}", f"foot_{s}", ankle)):
                pb = pose[bone]
                now = mw @ pose[child].head - mw @ pb.head
                turn_to(bone, (now.rotation_difference(goal - mw @ pb.head)) @ (mw @ pb.matrix).to_quaternion())
                if bone.startswith("thigh"):
                    pb.rotation_mode = "QUATERNION"
                    pb.rotation_quaternion = without_roll(pb.rotation_quaternion)
                    update()
                pb.keyframe_insert("rotation_quaternion", frame=t)
            turn_to(f"foot_{s}", turns[f"foot_{s}"])
            pose[f"foot_{s}"].keyframe_insert("rotation_quaternion", frame=t)
        if back > 0.0:
            neck, head = pose["neck_01"], pose["Head"]
            face = (mw @ head.matrix).to_quaternion()
            left = mw @ pose["thigh_l"].head - mw @ pose["thigh_r"].head
            q = drawn_back(mw @ head.head - mw @ neck.head, left, back)
            turn_to("neck_01", q @ (mw @ neck.matrix).to_quaternion())
            turn_to("Head", face)
            neck.keyframe_insert("rotation_quaternion", frame=t)
            head.keyframe_insert("rotation_quaternion", frame=t)
    return sum(lifts) / len(lifts) * 100.0, c, back * 100.0
