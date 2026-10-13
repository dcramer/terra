"""Measures how a body's skin deforms through our clips, along every limb, to find twisting
and squeezing (candy-wrapper twist, collapse at a bend) where the rig check's rings at the
elbows, wrists and knees do not look.

    tools/blender/run.sh tools/characters/deform_check.py tools/characters/out/base-male-game.glb [CLIP ...] [--linear]

The body is skinned as the game skins it, weight-blended (look.skin_as_game); --linear uses
the plain linear blend throughout and --dqs dual quaternions throughout, to compare. --sheet=OUT.png also renders the worst moments, close on
each ring that squeezes more than SHEET_SQUEEZE, from the front and the side, in plain clay:
one row a ring, listed by the SHEET lines it prints in the same order.

For each limb segment (upper arm, forearm, thigh, shin, each side) it takes rings of skin across
the segment at a quarter, a half and three quarters of the way along it, one round the shoulder
and the hip, and one across the middle of each spine bone and the neck. Through each clip it reports, per ring, the worst:
  - squeeze: how much the ring's mean radius about the bone shrinks (0.3 is a third thinner);
  - flatten: how much flatter the ring is than at rest, its narrowest width against its widest
    (0.5 is half as round): a ring can keep its size and still collapse into a ribbon;
  - twist: how far the ring's skin turns about the bone, against how far the bone itself turns
    there, in degrees. With twist bones the skin is meant to turn along a segment, so read this
    with the others, not alone.
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

CLIPS = look.game_clips()
SLAB_M = 0.012  # half the thickness of a ring
SHEET_SQUEEZE = 0.12  # a ring this much thinner is drawn on the sheet
SHEET_ROWS = 8

args = look.args()
path, clips = args[0], [a for a in args[1:] if not a.startswith("--")] or list(CLIPS)
sheet = next((a.split("=", 1)[1] for a in args if a.startswith("--sheet=")), None)
look.reset()
bpy.ops.import_scene.gltf(filepath=os.path.join(look.ROOT, path))
arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
body = max((o for o in bpy.context.scene.objects if o.type == "MESH"), key=lambda o: len(o.data.vertices))
look.skin_as_game(method="linear" if "--linear" in args else "dqs" if "--dqs" in args else None)
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
for s in ("l", "r"):
    for part, b in (("upperarm", "lowerarm"), ("lowerarm", "hand"), ("thigh", "calf"), ("calf", "foot")):
        bone = f"{part}_{s}"
        head, _, _ = bone_frame(bone)
        end, _, _ = bone_frame(f"{b}_{s}")
        axis = end - head
        length = np.linalg.norm(axis)
        axis /= length
        for t in ((0.05, 0.25, 0.5, 0.75) if part in ("lowerarm", "calf") else (0.25, 0.5, 0.75, 0.95)):
            centre = head + axis * length * t
            along = (rest - centre) @ axis
            radial = rest - centre - np.outer(along, axis)
            dist = np.linalg.norm(radial, axis=1)
            near = np.where((np.abs(along) < SLAB_M) & (dist < length * 0.6))[0]
            # Only skin this limb moves, not the torso beside an upper arm at rest.
            moved = set(weight_of.get(bone, {})) | set(weight_of.get(f"{b}_{s}", {})) | set(weight_of.get(f"{part}_twist_01_{s}", {}))
            near = np.array([i for i in near if i in moved])
            if len(near) >= 12:
                rings[f"{part}_{s} {int(t * 100)}%"] = (bone, f"{b}_{s}", near, t)
    for part, bone, b in (("shoulder", f"upperarm_{s}", f"lowerarm_{s}"), ("hip", f"thigh_{s}", f"calf_{s}")):
        head, _, _ = bone_frame(bone)
        end, _, _ = bone_frame(b)
        axis = (end - head) / np.linalg.norm(end - head)
        along = (rest - head) @ axis
        radial = rest - head - np.outer(along, axis)
        dist = np.linalg.norm(radial, axis=1)
        near = np.where((np.abs(along - 0.03) < SLAB_M * 1.5) & (dist < 0.14))[0]
        if len(near) >= 12:
            rings[f"{part}_{s}"] = (bone, b, near, 0.0)


# Round the torso and the neck: rings across each spine bone and the neck, of the skin they
# and their neighbours move, the arms left out.
TORSO = ("pelvis", "spine_01", "spine_02", "spine_03", "neck_01", "Head")
torso_moved = set()
for group in TORSO:
    torso_moved |= set(weight_of.get(group, {}))
for a_name, b_name in zip(TORSO[1:-1], TORSO[2:]):
    head, _, _ = bone_frame(a_name)
    end, _, _ = bone_frame(b_name)
    axis = end - head
    length = float(np.linalg.norm(axis))
    axis /= length
    centre = head + axis * length * 0.5
    along = (rest - centre) @ axis
    radial = rest - centre - np.outer(along, axis)
    dist = np.linalg.norm(radial, axis=1)
    near = np.array([i for i in np.where((np.abs(along) < SLAB_M) & (dist < 0.25))[0] if i in torso_moved])
    if len(near) >= 12:
        rings[f"{a_name} 50%"] = (a_name, b_name, near, 0.5)


def ring_state(pts, bone, end_bone, near, t):
    # The segment from this joint to the next, posed: never the bone's tail, which a glTF file
    # does not keep (the importer guesses one).
    head, _, m = bone_frame(bone)
    end, _, _ = bone_frame(end_bone)
    axis = (end - head) / np.linalg.norm(end - head)
    centre = head + (end - head) * t
    rel = pts[near] - centre
    radial = rel - np.outer(rel @ axis, axis)
    radius = float(np.linalg.norm(radial, axis=1).mean())
    # How round the ring is: its narrowest spread against its widest (1 is round).
    flat = radial - radial.mean(axis=0)
    spread = np.sort(np.linalg.eigvalsh(flat.T @ flat / len(flat)))[1:]
    roundness = float(math.sqrt(max(spread[0], 0.0) / max(spread[1], 1e-12)))
    # The skin's angles about the axis, in the bone's own frame: a turn of the bone carries
    # skin that follows it; what is left is the skin lagging behind.
    x = m[:, 0]
    z = m[:, 2]
    angles = np.arctan2(radial @ z, radial @ x)
    return radius, angles, roundness


rest_state = {name: ring_state(rest, *spec) for name, spec in rings.items()}
worst = {name: {"squeeze": 0.0, "twist": 0.0, "flatten": 0.0, "at": ""} for name in rings}
for clip in clips:
    action = actions.get(clip)
    if action is None:
        continue
    ad.action = action
    if getattr(action, "slots", None) and len(action.slots):
        ad.action_slot = action.slots[0]
    start, end = action.frame_range
    for k in range(13):
        frame = int(start + (end - start) * k / 12)
        bpy.context.scene.frame_set(frame)
        pts = evaluated()
        for name, spec in rings.items():
            r0, a0, round0 = rest_state[name]
            r, a, rnd = ring_state(pts, *spec)
            squeeze = 1.0 - r / r0
            flatten = 1.0 - rnd / max(round0, 1e-9)
            lag = np.angle(np.mean(np.exp(1j * (a - a0))))
            turn = abs(math.degrees(lag))
            w = worst[name]
            if squeeze > w["squeeze"]:
                w["squeeze"] = squeeze
                w["at"] = f"{clip} {k}/12"
                w["moment"] = (clip, frame)
            if flatten > w["flatten"]:
                w["flatten"] = flatten
                w["flatten_at"] = f"{clip} {k}/12"
            if turn > w["twist"]:
                w["twist"] = turn
                w["twist_at"] = f"{clip} {k}/12"
ad.action = None
ranked = sorted(worst.items(), key=lambda t: -max(t[1]["squeeze"], t[1]["flatten"]))
for name, w in ranked:
    print("DEFORM", json.dumps({"ring": name, "squeeze": round(w["squeeze"], 3), "at": w["at"],
                                 "flatten": round(w["flatten"], 3), "flatten_at": w.get("flatten_at", ""),
                                 "twist_deg": round(w["twist"], 1), "twist_at": w.get("twist_at", "")}))

if sheet:
    look.stage()
    for o in bpy.context.scene.objects:
        if o.type == "MESH" and o.name != "Ground":
            o.data.materials.clear()
            o.data.materials.append(look.material("Skin"))
    rows = []
    shown = [(n, w) for n, w in sorted(worst.items(), key=lambda t: -t[1]["squeeze"]) if w["squeeze"] > SHEET_SQUEEZE]
    for name, w in shown[:SHEET_ROWS]:
        clip, frame = w["moment"]
        ad.action = actions[clip]
        if getattr(ad.action, "slots", None) and len(ad.action.slots):
            ad.action_slot = ad.action.slots[0]
        bpy.context.scene.frame_set(frame)
        bone, end_bone, _, t = rings[name]
        row = os.path.join(look.OUT, f"deform-{os.getpid()}-{len(rows)}.png")
        look.stitch(look.close_up(arm, bone if t < 0.5 else end_bone, 0.5, sides=(0, 90)), row)
        rows.append(row)
        print("SHEET", len(rows), name, f"squeeze {w['squeeze']:.2f}", clip, "frame", frame)
    if rows:
        look.stack(rows, look.path(sheet))
        print("SHEET saved", sheet)
    else:
        print("SHEET nothing over", SHEET_SQUEEZE)

