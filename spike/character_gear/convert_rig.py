"""Converts a body rigged by Tripo's auto-rig onto our skeleton, so our animations play on it.

Tripo rigs onto its own humanoid skeleton (78 bones, with twist and helper bones). Ours is
the Quaternius one every sandbox animation is made for. This:
  1. turns Tripo's model to face -Y as ours does, scales it to `height`, and welds the
     vertices glTF split at texture seams,
  2. moves each of our joints to the matching Tripo joint, keeping our bones' directions
     (the animations depend on them),
  3. renames Tripo's weights to our bones, folding each helper bone into the bone it helps,
  4. adds forearm twist bones (twist.py), keeps at most four bones a vertex, normalised,
     and exports the mesh and skin only.

    spike/model_bakeoff/run.sh ../character_gear/convert_rig.py out/tripo/base-male-tpose-hd-rigged.glb base-male-tripo 1.80
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "model_bakeoff"))
sys.path.insert(0, HERE)

import bpy
from mathutils import Matrix, Vector

import look
import twist

FIGURE = os.path.join(HERE, "..", "..", "assets", "models", "character", "figure.gltf")
OUT = os.path.join(HERE, "out")

# Tripo bone -> ours. Helper bones (twist, share, eyes) fold into the bone they help.
MAP = {
    "Hip": "DEF-hips", "Pelvis": "DEF-hips", "BoneRoot": "DEF-hips",
    "Waist": "DEF-spine.001", "Spine01": "DEF-spine.002", "Spine02": "DEF-spine.003",
    "NeckTwist01": "DEF-neck", "NeckTwist02": "DEF-neck",
    "Head": "DEF-head", "FacialBone": "DEF-head", "L_Eye": "DEF-head", "R_Eye": "DEF-head",
}
for s in ("L", "R"):
    MAP.update({
        f"{s}_Clavicle": f"DEF-shoulder.{s}",
        f"{s}_Upperarm": f"DEF-upper_arm.{s}", f"{s}_UpperarmTwist01": f"DEF-upper_arm.{s}", f"{s}_UpperarmTwist02": f"DEF-upper_arm.{s}",
        f"{s}_Forearm": f"DEF-forearm.{s}", f"{s}_ForearmTwist01": f"DEF-forearm.{s}", f"{s}_ForearmTwist02": f"DEF-forearm.{s}",
        f"{s}_ElbowShareBone": f"DEF-forearm.{s}",
        f"{s}_Hand": f"DEF-hand.{s}",
        f"{s}_Thigh": f"DEF-thigh.{s}", f"{s}_ThighTwist01": f"DEF-thigh.{s}", f"{s}_ThighTwist02": f"DEF-thigh.{s}",
        f"{s}_Calf": f"DEF-shin.{s}", f"{s}_CalfTwist01": f"DEF-shin.{s}", f"{s}_CalfTwist02": f"DEF-shin.{s}",
        f"{s}_KneeShareBone": f"DEF-shin.{s}",
        f"{s}_Foot": f"DEF-foot.{s}", f"{s}_ToeBase": f"DEF-toe.{s}",
    })
    for tripo, ours in (("Thumb", "thumb"), ("Index", "f_index"), ("Mid", "f_middle"), ("Ring", "f_ring"), ("Pinky", "f_pinky")):
        for i in (1, 2, 3):
            MAP[f"{s}_{tripo}{i}"] = f"DEF-{ours}.0{i}.{s}"
# Where each of our joints goes: the head of this Tripo bone.
JOINT = {
    "DEF-hips": "Hip", "DEF-spine.001": "Waist", "DEF-spine.002": "Spine01", "DEF-spine.003": "Spine02",
    "DEF-neck": "NeckTwist01", "DEF-head": "Head",
}
for s in ("L", "R"):
    JOINT.update({
        f"DEF-shoulder.{s}": f"{s}_Clavicle", f"DEF-upper_arm.{s}": f"{s}_Upperarm", f"DEF-forearm.{s}": f"{s}_Forearm",
        f"DEF-hand.{s}": f"{s}_Hand", f"DEF-thigh.{s}": f"{s}_Thigh", f"DEF-shin.{s}": f"{s}_Calf",
        f"DEF-foot.{s}": f"{s}_Foot", f"DEF-toe.{s}": f"{s}_ToeBase",
    })
    for tripo, ours in (("Thumb", "thumb"), ("Index", "f_index"), ("Mid", "f_middle"), ("Ring", "f_ring"), ("Pinky", "f_pinky")):
        for i in (1, 2, 3):
            JOINT[f"DEF-{ours}.0{i}.{s}"] = f"{s}_{tripo}{i}"

src, name, height = (look.args() + ["", "", "1.8"])[:3]
height = float(height)


def select_only(*objs, active=None):
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = active or objs[0]


look.reset()

# 1. Tripo's rigged model, turned to face -Y and scaled to height, applied to both.
bpy.ops.import_scene.gltf(filepath=os.path.join(look.HERE, src))
t_arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
body = max((o for o in bpy.context.scene.objects if o.type == "MESH"), key=lambda o: len(o.data.vertices))
for o in list(bpy.context.scene.objects):
    if o not in (t_arm, body):
        bpy.data.objects.remove(o)
for pb in t_arm.pose.bones:
    pb.matrix_basis.identity()
bpy.context.view_layer.update()
top = max((body.matrix_world @ v.co).z for v in body.data.vertices)
bottom = min((body.matrix_world @ v.co).z for v in body.data.vertices)
s = height / (top - bottom)
turn = Matrix.Rotation(-math.pi / 2, 4, "Z") @ Matrix.Scale(s, 4) @ Matrix.Translation((0, 0, -bottom))
joints = {b.name: turn @ (t_arm.matrix_world @ b.head_local) for b in t_arm.data.bones}
# The body, in world space under the same transform, unparented.
mw = body.matrix_world.copy()
body.parent = None
body.modifiers.clear()
body.data.transform(turn @ mw)
body.matrix_world = Matrix.Identity(4)
bpy.data.objects.remove(t_arm)
# glTF splits a vertex wherever the texture has a seam. Weld them back (Blender keeps the
# texture coordinates on the faces), or smoothing gives each copy different weights and the
# seams open into gaps as the body moves.
select_only(body)
bpy.ops.object.mode_set(mode="EDIT")
bpy.ops.mesh.select_all(action="SELECT")
bpy.ops.mesh.remove_doubles(threshold=1e-5)
bpy.ops.object.mode_set(mode="OBJECT")

# 2. Our skeleton, its pose cleared, joints moved to Tripo's, directions kept.
bpy.ops.import_scene.gltf(filepath=FIGURE)
arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
for o in list(bpy.context.scene.objects):
    if o.type == "MESH" and o is not body:
        bpy.data.objects.remove(o)
arm.animation_data_clear()
for pb in arm.pose.bones:
    pb.matrix_basis.identity()
bpy.context.view_layer.update()
# Scale the skeleton object so its hips sit at Tripo's hip height (kept as object scale, so
# the animations' hip movements stay in the skeleton's own units).
hips_now = (arm.matrix_world @ arm.data.bones["DEF-hips"].head_local).z
k = joints["Hip"].z / hips_now
arm.scale = [c * k for c in arm.scale]
bpy.context.view_layer.update()
to_local = arm.matrix_world.inverted()
select_only(arm)
bpy.ops.object.mode_set(mode="EDIT")
eb = arm.data.edit_bones
for b in eb:
    b.use_connect = False
order = sorted(eb, key=lambda b: len(b.parent_recursive))
for b in order:
    if b.name not in JOINT:
        continue
    d = (b.tail - b.head)
    new_head = to_local @ joints[JOINT[b.name]]
    b.head = new_head
    b.tail = new_head + d
bpy.ops.object.mode_set(mode="OBJECT")

# 3. Weights renamed onto our bones, helpers folded in.
weights = {}
groups = {g.index: g.name for g in body.vertex_groups}
for v in body.data.vertices:
    acc = {}
    for g in v.groups:
        ours = MAP.get(groups.get(g.group))
        if ours and g.weight > 0:
            acc[ours] = acc.get(ours, 0.0) + g.weight
    weights[v.index] = acc
for g in list(body.vertex_groups):
    body.vertex_groups.remove(g)
made = {}
for i, acc in weights.items():
    top4 = sorted(acc.items(), key=lambda t: -t[1])[:4]
    total = sum(w for _, w in top4) or 1.0
    for bone, w in top4:
        if bone not in made:
            made[bone] = body.vertex_groups.new(name=bone)
        made[bone].add([i], w / total, "REPLACE")
unweighted = sum(1 for acc in weights.values() if not acc)
print("WEIGHTS", f"{len(weights) - unweighted}/{len(weights)} vertices weighted, {len(made)} bones")

# A light smoothing of the merged weights eases the creases at the groin and armpits, then
# back to four bones a vertex, normalised.
SMOOTH = int(os.environ.get("SMOOTH", "") or (look.args() + ["", "", "", "2"])[3] or 0)
if SMOOTH:
    select_only(body)
    bpy.ops.object.mode_set(mode="WEIGHT_PAINT")
    bpy.ops.object.vertex_group_smooth(group_select_mode="ALL", factor=0.5, repeat=SMOOTH)
    bpy.ops.object.vertex_group_limit_total(group_select_mode="ALL", limit=4)
    bpy.ops.object.vertex_group_normalize_all(group_select_mode="ALL", lock_active=False)
    bpy.ops.object.mode_set(mode="OBJECT")

# Twist bones at the elbows, so a forearm's turn does not pinch the elbow (twist.py); then
# back to four bones a vertex, normalised.
twist.add(arm, body)
select_only(body)
bpy.ops.object.mode_set(mode="WEIGHT_PAINT")
bpy.ops.object.vertex_group_limit_total(group_select_mode="ALL", limit=4)
bpy.ops.object.vertex_group_normalize_all(group_select_mode="ALL", lock_active=False)
bpy.ops.object.mode_set(mode="OBJECT")

# 4. Bind and export the mesh and skin.
body.parent = arm
body.matrix_parent_inverse = arm.matrix_world.inverted()
am = body.modifiers.new("Armature", "ARMATURE")
am.object = arm
os.makedirs(OUT, exist_ok=True)
select_only(body, arm, active=arm)
path = os.path.join(OUT, f"{name}.glb")
bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_animations=False)
print("EXPORTED", path)
