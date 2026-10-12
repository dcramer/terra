"""Converts a body rigged by Tripo's auto-rig onto our skeleton, so our animations play on it.

Tripo's current (V3) auto-rig, with its Unreal skeleton (`tripo.ts rig --skeleton unreal`, the
default), names its bones as our skeleton does: ours is the one Quaternius's animation libraries
are made on (`look.FIGURE`, cut by clips.py), which uses Unreal's names. Tripo adds one twist
bone on each upper arm, forearm, thigh and calf, with skin on it; ours has none, and the clips
never move them. This:
  1. turns Tripo's model to face -Y as ours does, scales it to `height`, and welds the
     vertices glTF split at texture seams,
  2. moves each of our joints to the matching Tripo joint, keeping our bones' directions
     (the animations depend on them), and adds Tripo's twist bones, each facing as the bone
     it twists (twist.py turns them),
  3. keeps Tripo's weights, every bone's under its own name, at most four bones a vertex,
     normalised, lightly smoothed, and exports the mesh and skin only.

    tools/blender/run.sh tools/characters/convert_rig.py art/characters/base-male/rigged.glb base-male 1.80
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy
from mathutils import Matrix, Vector

import look
import twist

FIGURE = look.FIGURE
OUT = os.path.join(HERE, "out")

# Tripo's bones and ours share Unreal's names, but for the head's.
RENAME = {"head": "Head"}

src, name, height = (look.args() + ["", "", "1.8"])[:3]
height = float(height)


def select_only(*objs, active=None):
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = active or objs[0]


look.reset()

# 1. Tripo's rigged model, turned to face -Y and scaled to height, applied to both.
bpy.ops.import_scene.gltf(filepath=look.path(src))
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
# Our joint -> where it goes: the same bone of Tripo's.
joints = {RENAME.get(n, n): p for n, p in joints.items()}
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
hips_now = (arm.matrix_world @ arm.data.bones["pelvis"].head_local).z
k = joints["pelvis"].z / hips_now
arm.scale = [c * k for c in arm.scale]
bpy.context.view_layer.update()
to_local = arm.matrix_world.inverted()
select_only(arm)
bpy.ops.object.mode_set(mode="EDIT")
eb = arm.data.edit_bones
for b in eb:
    b.use_connect = False
order = sorted(eb, key=lambda b: len(b.parent_recursive))
moved = {}
for b in order:
    d = (b.tail - b.head)
    if b.name in joints:
        new_head = to_local @ joints[b.name]
    elif b.parent is not None and b.parent.name in moved:
        # A bone Tripo's rig has not (a finger's or toe's end): it moves with its parent.
        new_head = b.head + moved[b.parent.name]
    else:
        continue
    moved[b.name] = new_head - b.head
    b.head = new_head
    b.tail = new_head + d
bpy.ops.object.mode_set(mode="OBJECT")
twist.add(arm, {n: to_local @ joints[n] for n, _ in twist.names() if n in joints})

# 3. Tripo's weights, by the same names on our bones.
weights = {}
groups = {g.index: g.name for g in body.vertex_groups}
for v in body.data.vertices:
    acc = {}
    for g in v.groups:
        theirs = groups.get(g.group)
        ours = RENAME.get(theirs, theirs)
        if ours not in arm.data.bones:
            ours = None
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
