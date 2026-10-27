"""Clips carried from the skeleton they were made on (the mannequin's) onto a body's, by each
bone's rotation in the world from a retarget pose, as shipped retargeters do (D542): Unreal's
IK Retargeter copies each chain's rotations as deltas from both skeletons' retarget poses, its
target's pose aligned to the source's by "auto align" on the mapped chains; Rokoko's and Auto-Rig
Pro's Blender retargeters do the same from a rest pose matched to the source's.

A body's skeleton is ours with its joints moved into its skin (convert_rig.py), each bone turned
to point at its moved child, its roll kept nearest. So:

  - the retarget pose: the body's rest with each limb segment (upper arm, forearm, hand, thigh,
    shin) turned the least way to point as the mannequin's does at its rest, so a forearm the
    body holds bent at rest runs straight on in a clip as the mannequin's does; a foot only turned
    about the up axis (how far it pitches from ankle to ball is its shape, not the stance); every
    other bone carried by its parent, keeping its rest (the pelvis, spine, neck, head, clavicles,
    fingers and toes: their joints' lines inside an upright body are its shape, not a posture,
    as Unreal keeps the retarget root's and the spine's);
  - every frame, each bone's rotation in the world is the mannequin's in that clip, taken off the
    mannequin's rest and laid on the retarget pose: per bone a fixed turn either side of its key,
    K' = A K B, so the keys stay keys and a move keeps its curve.

On the mannequin itself the retarget pose is its rest and every turn is none.
"""

import math

import bpy
from mathutils import Matrix, Quaternion, Vector

# Each limb segment the retarget pose points as the mannequin's: bone -> the joint it reaches.
LIMBS = {f"{b}_{s}": f"{c}_{s}" for s in "lr" for b, c in (
    ("upperarm", "lowerarm"), ("lowerarm", "hand"), ("hand", "middle_01"),
    ("thigh", "calf"), ("calf", "foot"), ("foot", "ball"),
)}
# Turned about the up axis only: its pitch is the foot's shape.
LEVEL = {"foot_l", "foot_r"}
CHANNELS = (("rotation_quaternion", 4), ("location", 3))


def main_children(arm):
    """Each bone with children -> the child its own +Y points at most nearly (the mannequin's
    children all sit on it): the joint a bone is turned to point at."""
    out = {}
    for b in arm.data.bones:
        best = None
        for c in b.children:
            off = c.head_local - b.head_local
            if off.length < 1e-6:
                continue
            y = b.matrix_local.to_3x3().col[1].normalized()
            score = off.normalized().dot(y)
            if best is None or score > best[0]:
                best = (score, c.name)
        if best is not None:
            out[b.name] = best[1]
    return out


def orient(edit_bones, children, keep=("root",)):
    """In edit mode: each bone with a child in `children` (main_children, read before the joints
    moved) turned the least way to point at that child where it now stands, so its roll stays
    nearest what it was, and reaching it. The root keeps its axes: they are the clips' ground."""
    for name, child in children.items():
        if name in keep or name not in edit_bones or child not in edit_bones:
            continue
        b = edit_bones[name]
        line = edit_bones[child].head - b.head
        if line.length < 1e-6:
            continue
        m = b.matrix.copy()
        y = m.to_3x3().col[1].normalized()
        swing = y.rotation_difference(line.normalized())
        head = b.head.copy()
        b.matrix = Matrix.Translation(head) @ (swing.to_matrix() @ m.to_3x3()).to_4x4()
        b.tail = head + line


def _turn(arm):
    """The armature's turn into the world, without its scale."""
    return arm.matrix_world.to_quaternion()


def source(arm):
    """What the retarget needs of the skeleton the clips were made on: each bone's rest turn in
    the world, and each limb's rest line in the world."""
    w = _turn(arm)
    b = arm.data.bones
    rest = {x.name: w @ x.matrix_local.to_quaternion() for x in b}
    lines = {n: (w @ (b[c].head_local - b[n].head_local)).normalized() for n, c in LIMBS.items() if n in b and c in b}
    return {"rest": rest, "lines": lines}


def pose(arm, src):
    """The retarget pose of `arm` against the source: per bone its turn in armature space."""
    w = _turn(arm)
    to_arm = w.inverted()
    bones = arm.data.bones
    rest = {b.name: b.matrix_local.to_quaternion() for b in bones}
    up = Vector((0.0, 0.0, 1.0))
    out = {}
    for b in sorted(bones, key=lambda b: len(b.parent_recursive)):
        n = b.name
        if n in LIMBS and n in src["lines"] and LIMBS[n] in bones:
            ours = (w @ (bones[LIMBS[n]].head_local - b.head_local)).normalized()
            theirs = src["lines"][n].copy()
            if n in LEVEL:
                ours = (ours - up * ours.dot(up)).normalized()
                theirs = (theirs - up * theirs.dot(up)).normalized()
            out[n] = to_arm @ ours.rotation_difference(theirs) @ w @ rest[n]
        elif b.parent is not None:
            p = b.parent.name
            out[n] = out[p] @ rest[p].inverted() @ rest[n]
        else:
            out[n] = rest[n]
    return out


def turns(arm, src):
    """Per bone the fixed turns (A, B) that carry a key of the source's onto `arm`: with R our
    rest, P our retarget pose and M the source's rest, all in our armature space,
    world = parent's world (R_p^-1 R_b) key, and our world = the source's taken off M, laid on P,
    the key is A K B with A = R_b^-1 R_p P_p^-1 M_b and B = M_b^-1 P_b; a location key turns by
    A alone."""
    to_arm = _turn(arm).inverted()
    m = {n: to_arm @ q for n, q in src["rest"].items()}
    p = pose(arm, src)
    out = {}
    for b in arm.data.bones:
        n = b.name
        if n not in m:
            continue  # a twist bone: twist.py keys it
        r = b.matrix_local.to_quaternion()
        if b.parent is not None:
            rp, pp = b.parent.matrix_local.to_quaternion(), p[b.parent.name]
            a = r.inverted() @ rp @ pp.inverted() @ m[n]
        else:
            a = r.inverted() @ m[n]
        out[n] = (a, m[n].inverted() @ p[n])
    return out


def _bags(action):
    if hasattr(action, "fcurves"):
        return [action.fcurves]
    return [bag.fcurves for layer in action.layers for strip in layer.strips for bag in strip.channelbags]


def _curves(action):
    out = {}
    for fcurves in _bags(action):
        for fc in fcurves:
            out[(fc.data_path, fc.array_index)] = fc
    return out


def bake(arm, actions, src):
    """Every bone's keys of each of `actions` carried onto `arm` (keyed every frame). Returns
    the largest turn added to any bone, in degrees, for the log."""
    t = turns(arm, src)
    pose_bones = arm.pose.bones
    ad = arm.animation_data or arm.animation_data_create()
    keep = (ad.action, getattr(ad, "action_slot", None))
    for action in actions:
        start, end = (int(round(f)) for f in action.frame_range)
        frames = list(range(start, end + 1))
        ad.action = action
        if getattr(action, "slots", None) and len(action.slots):
            ad.action_slot = action.slots[0]
        curves = _curves(action)
        for n, (a, b) in t.items():
            pb = pose_bones[n]
            pb.rotation_mode = "QUATERNION"
            path = f'pose.bones["{n}"].'

            def read(channel, i, f, default):
                fc = curves.get((path + channel, i))
                return fc.evaluate(f) if fc is not None else default

            rot, loc = [], []
            last = None
            for f in frames:
                k = Quaternion([read("rotation_quaternion", i, f, 1.0 if i == 0 else 0.0) for i in range(4)])
                q = a @ k @ b
                if last is not None and q.dot(last) < 0:
                    q.negate()
                last = q
                rot.append(q)
                loc.append(a @ Vector([read("location", i, f, 0.0) for i in range(3)]))
            for channel, size in CHANNELS:
                values = rot if channel == "rotation_quaternion" else loc
                for i in range(size):
                    fc = curves.get((path + channel, i))
                    if fc is None:
                        pb.keyframe_insert(channel, index=i, frame=frames[0], group=n)
                        curves = _curves(action)
                        fc = curves[(path + channel, i)]
                    fc.keyframe_points.clear()
                    fc.keyframe_points.add(len(frames))
                    co = [x for f, v in zip(frames, values) for x in (f, v[i])]
                    fc.keyframe_points.foreach_set("co", co)
                    fc.keyframe_points.foreach_set("interpolation", [bpy.types.Keyframe.bl_rna.properties["interpolation"].enum_items["LINEAR"].value] * len(frames))
                    fc.update()
    ad.action, slot = keep
    if slot is not None:
        ad.action_slot = slot
    least = lambda q: min(q.angle, 2 * math.pi - q.angle)  # noqa: E731
    return math.degrees(max((max(least(a), least(b)) for a, b in t.values()), default=0.0))
