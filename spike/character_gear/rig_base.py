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

FIGURE = os.path.join(HERE, "..", "..", "assets", "models", "character", "figure.gltf")
OUT = os.path.join(HERE, "out")
body_path = os.path.join(look.HERE, look.args()[0])
WEIGHT_SOURCE = (look.args() + ["heat"])[1]  # heat, or mannequin
OUT_NAME = (look.args() + ["heat", "base-male-rigged"])[2]


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
# Optionally rebuild the body as an even quad mesh first: generated meshes have no edge
# loops at the joints, so they crush and tear where they bend.
REMESH_FACES = int((look.args() + ["", "", "", "0"])[3] or 0)
if REMESH_FACES:
    bpy.context.view_layer.objects.active = body
    body.select_set(True)
    # QuadriFlow needs one closed surface: make it watertight first.
    vox = body.modifiers.new("Watertight", "REMESH")
    vox.mode = "VOXEL"
    # An even voxel grid, its size chosen for the face count asked for. (QuadriFlow, which
    # gives flowing edge loops, cancels when Blender runs without a window.)
    area = sum(p.area for p in body.data.polygons)
    vox.voxel_size = math.sqrt(area / REMESH_FACES)
    bpy.ops.object.modifier_apply(modifier=vox.name)
    result = "voxel"
    for slot in body.material_slots:
        slot.material = look.material("Skin")
    print("REMESHED", result, len(body.data.polygons), "faces")
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
# The importer leaves the skeleton posed in the first frame of one of its animations (a
# crouch). Clear it, or that pose is baked into the rest pose and every animation plays on
# top of it.
for pb in arm.pose.bones:
    pb.matrix_basis.identity()
bpy.context.view_layer.update()
REST = {b.name: (b.head_local.copy(), b.tail_local.copy()) for b in arm.data.bones}

# 2. Scale the skeleton to the body's height (the head bone's tail is near the crown).
bpy.context.view_layer.update()
head_top = (arm.matrix_world @ arm.data.bones["DEF-head"].tail_local).z + 0.12
s = height / head_top
arm.scale = [c * s for c in arm.scale]
bpy.context.view_layer.update()
to_local = arm.matrix_world.inverted()


def turn_arm(tag, angle):
    """Turns an upper arm about the body's forward axis, in the skeleton's own space (the
    importer turns the skeleton to make Z up, so its axes are not the world's). A positive
    angle lowers the hand; the sign is chosen by measuring, not assumed."""
    pb = arm.pose.bones[f"DEF-upper_arm.{tag}"]
    pb.rotation_mode = "QUATERNION"
    rest = pb.bone.matrix_local.to_3x3()
    forward = (arm.matrix_world.inverted().to_3x3() @ Vector((0, 1, 0))).normalized()
    best = None
    for sign in (1, -1):
        r = Matrix.Rotation(sign * angle, 3, forward)
        pb.rotation_quaternion = (rest.inverted() @ r @ rest).to_quaternion()
        bpy.context.view_layer.update()
        z = (arm.matrix_world @ arm.pose.bones[f"DEF-hand.{tag}"].tail).z
        if best is None or (z < best[0]) == (angle > 0):
            best = (z, sign)
    r = Matrix.Rotation(best[1] * angle, 3, forward)
    pb.rotation_quaternion = (rest.inverted() @ r @ rest).to_quaternion()
    bpy.context.view_layer.update()



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
    # Scale the whole arm chain about the shoulder by the same factor on every axis, so no
    # bone changes direction (the animations depend on the directions).
    for name in [b.name for b in eb if b.name.endswith(f".{tag}") and any(k in b.name for k in ("arm", "hand", "f_", "thumb"))]:
        b = eb[name]
        b.head = shoulder + (b.head - shoulder) * stretch
        b.tail = shoulder + (b.tail - shoulder) * stretch
bpy.ops.object.mode_set(mode="OBJECT")
print("ARM ANGLES", {k: round(math.degrees(v), 1) for k, v in arm_angle.items()})

# Pose the arms down to the body's A-pose.
bpy.ops.object.mode_set(mode="POSE")
for tag in ("L", "R"):
    turn_arm(tag, arm_angle[tag])
bpy.ops.object.mode_set(mode="OBJECT")
bpy.context.view_layer.update()

# Bind with automatic weights (heat diffusion) in that pose: apply it as rest first.
select_only(arm)
bpy.ops.object.mode_set(mode="POSE")
bpy.ops.pose.armature_apply(selected=False)
bpy.ops.object.mode_set(mode="OBJECT")
def weight_from_proxy(voxel=0.012, smooth_passes=0):
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
    # Relax the weights a little and keep at most four bones per vertex, normalised, which
    # softens the creases where joints bend hardest.
    select_only(proxy)
    bpy.ops.object.mode_set(mode="WEIGHT_PAINT")
    bpy.ops.object.vertex_group_smooth(group_select_mode="ALL", factor=0.5, repeat=smooth_passes)
    bpy.ops.object.vertex_group_limit_total(group_select_mode="ALL", limit=4)
    bpy.ops.object.vertex_group_normalize_all(group_select_mode="ALL", lock_active=False)
    bpy.ops.object.mode_set(mode="OBJECT")
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
for tag in ("L", "R"):
    turn_arm(tag, -arm_angle[tag])
bpy.ops.object.mode_set(mode="OBJECT")
bpy.context.view_layer.update()
select_only(body)
bpy.ops.object.modifier_apply(modifier=body.modifiers[0].name)
select_only(arm)
bpy.ops.object.mode_set(mode="POSE")
bpy.ops.pose.armature_apply(selected=False)
bpy.ops.object.mode_set(mode="OBJECT")

# The real weighting, in the T-pose, where the arms are clear of the torso. Either
# Blender's automatic weights, or the artist-made mannequin's weights copied across: the
# mannequin is posed by the same skeleton, so after scaling it matches our body's T-pose.
if WEIGHT_SOURCE == "mannequin":
    for g in list(body.vertex_groups):
        body.vertex_groups.remove(g)
    for m in list(body.modifiers):
        body.modifiers.remove(m)
    before = set(bpy.context.scene.objects)
    bpy.ops.import_scene.gltf(filepath=FIGURE)
    new = [o for o in bpy.context.scene.objects if o not in before]
    m_arm = next(o for o in new if o.type == "ARMATURE")
    m_body = next(o for o in new if o.type == "MESH" and o.name.startswith("Mannequin"))
    m_arm.animation_data_clear()
    for pb in m_arm.pose.bones:
        pb.matrix_basis.identity()
    m_arm.scale = arm.scale
    m_arm.location = arm.location
    bpy.context.view_layer.update()
    # Stretch the mannequin's arms as ours were, so its arms lie along our body's.
    for tag in ("L", "R"):
        pb = m_arm.pose.bones[f"DEF-upper_arm.{tag}"]
        target = arm.data.bones[f"DEF-upper_arm.{tag}"]
        src = m_arm.data.bones[f"DEF-upper_arm.{tag}"]
        k = (arm.data.bones[f"DEF-hand.{tag}"].tail_local - target.head_local).length / max((m_arm.data.bones[f"DEF-hand.{tag}"].tail_local - src.head_local).length, 1e-6)
        pb.scale = (k, k, k)
    bpy.context.view_layer.update()
    for g in m_body.vertex_groups:
        body.vertex_groups.new(name=g.name)
    dt = body.modifiers.new("Weights", "DATA_TRANSFER")
    dt.object = m_body
    dt.use_object_transform = True
    dt.use_vert_data = True
    dt.data_types_verts = {"VGROUP_WEIGHTS"}
    dt.vert_mapping = "POLYINTERP_NEAREST"
    dt.layers_vgroup_select_src = "ALL"
    dt.layers_vgroup_select_dst = "NAME"
    select_only(body)
    bpy.ops.object.modifier_apply(modifier=dt.name)
    for o in new:
        if o.name in bpy.data.objects:
            bpy.data.objects.remove(o)
    select_only(body)
    bpy.ops.object.mode_set(mode="WEIGHT_PAINT")
    bpy.ops.object.vertex_group_smooth(group_select_mode="ALL", factor=0.5, repeat=2)
    bpy.ops.object.vertex_group_limit_total(group_select_mode="ALL", limit=4)
    bpy.ops.object.vertex_group_normalize_all(group_select_mode="ALL", lock_active=False)
    bpy.ops.object.mode_set(mode="OBJECT")
    body.parent = arm
    body.matrix_parent_inverse = arm.matrix_world.inverted()
    am = body.modifiers.new("Armature", "ARMATURE")
    am.object = arm
    weighted = sum(1 for v in body.data.vertices if any(g.weight > 0.01 for g in v.groups))
    print("WEIGHTED", f"{weighted}/{len(body.data.vertices)} vertices (from the mannequin)")
else:
    weight_from_proxy(smooth_passes=int(os.environ.get("SMOOTH_PASSES", "4")))

# The rest pose must still be the skeleton's own, or its animations will not fit.
worst = max(max((b.head_local - REST[b.name][0]).length, (b.tail_local - REST[b.name][1]).length)
            for b in arm.data.bones if not any(k in b.name for k in ("arm", "hand", "f_", "thumb")))
print("REST DRIFT", f"{worst:.4f}")

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
look.stack(rows, os.path.join(OUT, f"{OUT_NAME}.png"))
for r in rows:
    os.remove(r)
select_only(body, arm, active=arm)
bpy.ops.export_scene.gltf(filepath=os.path.join(OUT, f"{OUT_NAME}.glb"), export_format="GLB", use_selection=True)
print("SHEET", os.path.join(OUT, "rigged.png"))
