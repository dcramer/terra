"""Measures how a body's skin deforms through our clips, along every limb, to find twisting
and squeezing (candy-wrapper twist, collapse at a bend) where the rig check's rings at the
elbows, wrists and knees do not look.

    tools/blender/run.sh tools/characters/deform_check.py tools/characters/out/base-male-game.glb [CLIP ...]

For each limb segment (upper arm, forearm, thigh, shin, each side) it takes rings of skin across
the segment at a quarter, a half and three quarters of the way along it, and one round the
shoulder and the hip. Through each clip it reports, per ring, the worst:
  - squeeze: how much the ring's mean radius about the bone shrinks (0.3 is a third thinner);
  - twist: how far the ring's skin turns about the bone, against how far the bone itself turns
    there, in degrees (skin left behind by a turning bone, the candy wrapper).
Prints DEFORM lines of JSON, worst first.
"""

import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy
import numpy as np
from mathutils import Vector

import look
import twist

CLIPS = ("Idle_Loop", "Walk_Loop", "Jog_Fwd_Loop", "Sword_Attack", "Punch_Cross", "Crouch_Idle_Loop", "Roll")
SLAB_M = 0.012  # half the thickness of a ring

args = look.args()
path, clips = args[0], [a for a in args[1:]] or list(CLIPS)
look.reset()
bpy.ops.import_scene.gltf(filepath=os.path.join(look.ROOT, path))
arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
body = max((o for o in bpy.context.scene.objects if o.type == "MESH"), key=lambda o: len(o.data.vertices))
twist.drive(arm)
actions = {a.name: a for a in bpy.data.actions}
ad = arm.animation_data or arm.animation_data_create()
ad.action = None
for pb in arm.pose.bones:
    pb.matrix_basis.identity()
bpy.context.view_layer.update()


def evaluated():
    dg = bpy.context.evaluated_depsgraph_get()
    ev = body.evaluated_get(dg)
    mesh = ev.to_mesh()
    pts = np.array([tuple(body.matrix_world @ v.co) for v in mesh.vertices])
    ev.to_mesh_clear()
    return pts


def bone_frame(name):
    pb = arm.pose.bones[name]
    head = np.array(arm.matrix_world @ pb.head)
    tail = np.array(arm.matrix_world @ pb.tail)
    m = (arm.matrix_world.to_3x3() @ pb.matrix.to_3x3()).normalized()
    return head, tail, np.array(m)


groups = {g.index: g.name for g in body.vertex_groups}
weight_of = {}
for v in body.data.vertices:
    for g in v.groups:
        weight_of.setdefault(groups.get(g.group, ""), {})[v.index] = g.weight

rest = evaluated()
rings = {}
for s in ("L", "R"):
    for part, a, b in (("upper_arm", "DEF-upper_arm", "DEF-forearm"), ("forearm", "DEF-forearm", "DEF-hand"),
                       ("thigh", "DEF-thigh", "DEF-shin"), ("shin", "DEF-shin", "DEF-foot")):
        bone = f"{a}.{s}"
        head, _, _ = bone_frame(bone)
        end, _, _ = bone_frame(f"{b}.{s}")
        axis = end - head
        length = np.linalg.norm(axis)
        axis /= length
        for t in (0.25, 0.5, 0.75):
            centre = head + axis * length * t
            along = (rest - centre) @ axis
            radial = rest - centre - np.outer(along, axis)
            dist = np.linalg.norm(radial, axis=1)
            near = np.where((np.abs(along) < SLAB_M) & (dist < length * 0.6))[0]
            # Only skin this limb moves, not the torso beside an upper arm at rest.
            moved = set(weight_of.get(bone, {})) | set(weight_of.get(f"{b}.{s}", {})) | set(weight_of.get(f"DEF-forearm.twist.{s}", {}))
            near = np.array([i for i in near if i in moved])
            if len(near) >= 12:
                rings[f"{part}.{s} {int(t * 100)}%"] = (bone, near, t)
    for part, bone in (("shoulder", f"DEF-upper_arm.{s}"), ("hip", f"DEF-thigh.{s}")):
        head, _, _ = bone_frame(bone)
        end, _, _ = bone_frame(f"DEF-forearm.{s}" if part == "shoulder" else f"DEF-shin.{s}")
        axis = (end - head) / np.linalg.norm(end - head)
        along = (rest - head) @ axis
        radial = rest - head - np.outer(along, axis)
        dist = np.linalg.norm(radial, axis=1)
        near = np.where((np.abs(along - 0.03) < SLAB_M * 1.5) & (dist < 0.14))[0]
        if len(near) >= 12:
            rings[f"{part}.{s}"] = (bone, near, 0.0)


def ring_state(pts, bone, near, t):
    head, tail, m = bone_frame(bone)
    axis = (tail - head) / np.linalg.norm(tail - head)
    centre = head + (tail - head) * t
    rel = pts[near] - centre
    radial = rel - np.outer(rel @ axis, axis)
    radius = float(np.linalg.norm(radial, axis=1).mean())
    # The skin's angles about the axis, in the bone's own frame: a turn of the bone carries
    # skin that follows it; what is left is the skin lagging behind.
    x = m[:, 0]
    z = m[:, 2]
    angles = np.arctan2(radial @ z, radial @ x)
    return radius, angles


rest_state = {name: ring_state(rest, *spec) for name, spec in rings.items()}
worst = {name: {"squeeze": 0.0, "twist": 0.0, "at": ""} for name in rings}
for clip in clips:
    action = actions.get(clip)
    if action is None:
        continue
    ad.action = action
    if getattr(action, "slots", None) and len(action.slots):
        ad.action_slot = action.slots[0]
    start, end = action.frame_range
    for k in range(13):
        bpy.context.scene.frame_set(int(start + (end - start) * k / 12))
        pts = evaluated()
        for name, spec in rings.items():
            r0, a0 = rest_state[name]
            r, a = ring_state(pts, *spec)
            squeeze = 1.0 - r / r0
            lag = np.angle(np.mean(np.exp(1j * (a - a0))))
            turn = abs(math.degrees(lag))
            w = worst[name]
            if squeeze > w["squeeze"]:
                w["squeeze"] = squeeze
                w["at"] = f"{clip} {k}/12"
            if turn > w["twist"]:
                w["twist"] = turn
                w["twist_at"] = f"{clip} {k}/12"
ad.action = None
ranked = sorted(worst.items(), key=lambda t: -(t[1]["squeeze"] * 100 + t[1]["twist"]))
for name, w in ranked:
    print("DEFORM", json.dumps({"ring": name, "squeeze": round(w["squeeze"], 3), "at": w["at"],
                                 "twist_deg": round(w["twist"], 1), "twist_at": w.get("twist_at", "")}))
