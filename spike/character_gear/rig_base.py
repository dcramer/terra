"""Rig a generated base body to our skeleton (the Quaternius one every sandbox animation is
made for), by script, at no cost, then play animations on it to test how it bends.

  1. Load the body (refined, standing in an A-pose) and our skeleton (T-pose).
  2. Scale the skeleton to the body's height, and move its arm and leg joints onto the
     body's limbs, measured from the mesh.
  3. Pose the skeleton's arms down into the body's A-pose and bind the body to it with
     Blender's automatic weights.
  4. Pose it back to the T-pose, bake the body into that pose, and make it the rest pose,
     so animations made for the T-pose play unchanged.

    spike/model_bakeoff/run.sh ../character_gear/rig_base.py out/refined-BaseMaleHD-BaseMaleHD16000.glb
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "model_bakeoff"))

import bpy
import numpy as np
from mathutils import Matrix, Vector

import look

FIGURE = os.path.join(HERE, "..", "..", "crates", "terra_sandbox", "assets", "models", "character", "figure.gltf")
OUT = os.path.join(HERE, "out")
body_path = os.path.join(look.HERE, look.args()[0])


def select_only(*objs, active=None):
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = active or objs[0]


look.reset()
bpy.ops.import_scene.gltf(filepath=body_path)
body = next(o for o in bpy.context.scene.objects if o.type == "MESH")
body.parent = None
body.data.transform(body.matrix_world)
body.matrix_world = Matrix.Identity(4)
# The pipeline's models face +X; the skeleton faces -Y.
body.data.transform(Matrix.Rotation(-math.pi / 2, 4, "Z"))
for o in list(bpy.context.scene.objects):
    if o is not body:
        bpy.data.objects.remove(o)
for slot in body.material_slots:
    slot.material = look.material("Skin")
pts = np.array([tuple(v.co) for v in body.data.vertices])
height = pts[:, 2].max()
body.data.transform(Matrix.Translation((-pts[:, 0].mean() * 0, -np.median(pts[:, 1]), 0)))
pts = np.array([tuple(v.co) for v in body.data.vertices])

bpy.ops.import_scene.gltf(filepath=FIGURE)
arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
for o in list(bpy.context.scene.objects):
    if o.type == "MESH" and o is not body:
        bpy.data.objects.remove(o)
arm.animation_data_clear()

# 2. Scale the skeleton to the body's height (the head bone's tail is near the crown).
bpy.context.view_layer.update()
head_top = (arm.matrix_world @ arm.data.bones["DEF-head"].tail_local).z + 0.12
s = height / head_top
arm.scale = [c * s for c in arm.scale]
bpy.context.view_layer.update()
to_local = arm.matrix_world.inverted()


def limb_line(side):
    """The body's arm, from shoulder to fingertip: the points far out to one side, above the hips."""
    sel = pts[(np.sign(pts[:, 0]) == side) & (pts[:, 2] > height * 0.45)]
    tip = sel[np.argmax(np.abs(sel[:, 0]) + (height - sel[:, 2]) * 0.2)]
    return tip


# 3. In edit mode, put each arm's joints along the body's arm, at the skeleton's own
# proportions, which keeps the skeleton's T-pose geometry for the animations.
select_only(arm)
bpy.ops.object.mode_set(mode="EDIT")
eb = arm.data.edit_bones
arm_angle = {}
for side, tag in ((1, "L"), (-1, "R")):
    shoulder = Vector(eb[f"DEF-upper_arm.{tag}"].head)
    tip = to_local @ Vector(limb_line(side))
    # The body's arm hangs down at this angle from horizontal.
    d = tip - shoulder
    arm_angle[tag] = math.atan2(-d.z, abs(d.x))
    # Stretch the arm chain along X so its length matches the body's arm in the T-pose.
    chain = [f"DEF-upper_arm.{tag}", f"DEF-forearm.{tag}", f"DEF-hand.{tag}"]
    rig_len = abs(eb[chain[-1]].tail.x - shoulder.x)
    stretch = d.length / max(rig_len, 1e-6)
    for name in [b.name for b in eb if b.name.endswith(f".{tag}") and any(k in b.name for k in ("arm", "hand", "f_", "thumb"))]:
        b = eb[name]
        for attr in ("head", "tail"):
            p = getattr(b, attr)
            p.x = shoulder.x + (p.x - shoulder.x) * stretch
            setattr(b, attr, p)
bpy.ops.object.mode_set(mode="OBJECT")
print("ARM ANGLES", {k: round(math.degrees(v), 1) for k, v in arm_angle.items()})

# Pose the arms down to the body's A-pose.
bpy.ops.object.mode_set(mode="POSE")
for tag, sign in (("L", 1), ("R", -1)):
    pb = arm.pose.bones[f"DEF-upper_arm.{tag}"]
    pb.rotation_mode = "QUATERNION"
    rest = pb.bone.matrix_local.to_3x3()
    pb.rotation_quaternion = (rest.inverted() @ Matrix.Rotation(-sign * arm_angle[tag], 3, "Y") @ rest).to_quaternion()
bpy.ops.object.mode_set(mode="OBJECT")
bpy.context.view_layer.update()

# Bind with automatic weights (heat diffusion) in that pose: apply it as rest first.
select_only(arm)
bpy.ops.object.mode_set(mode="POSE")
bpy.ops.pose.armature_apply(selected=False)
bpy.ops.object.mode_set(mode="OBJECT")
def weight_from_proxy(voxel=0.012):
    """Heat weighting needs one closed surface, which generated meshes rarely are: weight a
    watertight remeshed copy, then copy its weights onto the real body."""
    for g in list(body.vertex_groups):
        body.vertex_groups.remove(g)
    for m in list(body.modifiers):
        body.modifiers.remove(m)
    proxy = body.copy()
    proxy.data = body.data.copy()
    proxy.parent = None
    bpy.context.scene.collection.objects.link(proxy)
    rm = proxy.modifiers.new("Remesh", "REMESH")
    rm.mode = "VOXEL"
    rm.voxel_size = voxel
    select_only(proxy)
    bpy.ops.object.modifier_apply(modifier=rm.name)
    select_only(proxy, arm, active=arm)
    bpy.ops.object.parent_set(type="ARMATURE_AUTO")
    for g in proxy.vertex_groups:
        body.vertex_groups.new(name=g.name)
    dt = body.modifiers.new("Weights", "DATA_TRANSFER")
    dt.object = proxy
    dt.use_vert_data = True
    dt.data_types_verts = {"VGROUP_WEIGHTS"}
    dt.vert_mapping = "POLYINTERP_NEAREST"
    dt.layers_vgroup_select_src = "ALL"
    dt.layers_vgroup_select_dst = "NAME"
    select_only(body)
    bpy.ops.object.modifier_apply(modifier=dt.name)
    bpy.data.objects.remove(proxy)
    body.parent = arm
    body.matrix_parent_inverse = arm.matrix_world.inverted()
    am = body.modifiers.new("Armature", "ARMATURE")
    am.object = arm
    weighted = sum(1 for v in body.data.vertices if any(g.weight > 0.01 for g in v.groups))
    print("WEIGHTED", f"{weighted}/{len(body.data.vertices)} vertices")


# A rough first weighting in the A-pose, only to carry the body into the T-pose.
weight_from_proxy()

# 4. Back to the T-pose: pose the arms up, bake the body in that pose, make it the rest pose.
select_only(arm)
bpy.ops.object.mode_set(mode="POSE")
for tag, sign in (("L", 1), ("R", -1)):
    pb = arm.pose.bones[f"DEF-upper_arm.{tag}"]
    pb.rotation_mode = "QUATERNION"
    rest = pb.bone.matrix_local.to_3x3()
    pb.rotation_quaternion = (rest.inverted() @ Matrix.Rotation(sign * arm_angle[tag], 3, "Y") @ rest).to_quaternion()
bpy.ops.object.mode_set(mode="OBJECT")
bpy.context.view_layer.update()
select_only(body)
bpy.ops.object.modifier_apply(modifier=body.modifiers[0].name)
select_only(arm)
bpy.ops.object.mode_set(mode="POSE")
bpy.ops.pose.armature_apply(selected=False)
bpy.ops.object.mode_set(mode="OBJECT")

# The real weighting, in the T-pose, where the arms are clear of the torso.
weight_from_proxy()

# Play the sandbox's animations and render a sheet.
look.stage()
scene = bpy.context.scene
scene.render.resolution_x = scene.render.resolution_y = 360
cam = bpy.data.objects.new("Cam", bpy.data.cameras.new("Cam"))
cam.data.lens = 50
scene.collection.objects.link(cam)
scene.camera = cam
sun = bpy.data.objects["Sun"]
bpy.ops.import_scene.gltf(filepath=FIGURE)  # brings the animation actions back
for o in list(bpy.context.scene.objects):
    if o.name.startswith(("Mannequin", "Icosphere")) or (o.type == "ARMATURE" and o is not arm):
        bpy.data.objects.remove(o)
os.makedirs(OUT, exist_ok=True)
rows = []
for action_name in ("Idle_Loop", "Walk_Loop", "Sword_Attack", "Crouch_Idle_Loop"):
    action = bpy.data.actions.get(action_name) or next(a for a in bpy.data.actions if a.name.startswith(action_name))
    ad = arm.animation_data or arm.animation_data_create()
    ad.action = action
    if getattr(action, "slots", None) and len(action.slots):
        ad.action_slot = action.slots[0]
    start, end = action.frame_range
    tiles = []
    for k, (yaw, t) in enumerate(((30, 0.0), (30, 0.4), (300, 0.7), (130, 0.9))):
        scene.frame_set(int(start + (end - start) * t))
        y = math.radians(yaw)
        target = Vector((0, 0, height * 0.52))
        cam.location = target + Vector((math.sin(y), -math.cos(y), 0.18)) * 3.6
        cam.rotation_euler = (target - cam.location).to_track_quat("-Z", "Y").to_euler()
        ly, lp = y + math.radians(50), math.radians(40)
        towards = Vector((math.sin(ly) * math.cos(lp), -math.cos(ly) * math.cos(lp), math.sin(lp)))
        sun.rotation_euler = (-towards).to_track_quat("-Z", "Y").to_euler()
        tile = os.path.join(OUT, f"rig-{action_name}-{k}.png")
        scene.render.filepath = tile
        bpy.ops.render.render(write_still=True)
        tiles.append(tile)
    row = os.path.join(OUT, f"rigrow-{action_name}.png")
    look.stitch(tiles, row)
    for t in tiles:
        os.remove(t)
    rows.append(row)
look.stack(rows, os.path.join(OUT, "rigged.png"))
for r in rows:
    os.remove(r)
select_only(body, arm, active=arm)
bpy.ops.export_scene.gltf(filepath=os.path.join(OUT, "base-male-rigged.glb"), export_format="GLB", use_selection=True)
print("SHEET", os.path.join(OUT, "rigged.png"))
