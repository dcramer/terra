"""Spike: can generated gear be fitted and skinned to one base body automatically, and move
without tearing or poking through? (research/ai-models-for-art.md)

Takes the sandbox's rigged figure and refined gear models, and for each piece:
  1. turns it to face the body's front and fits it to its body region (from the bones' weights),
  2. pushes out any part that sinks into the body,
  3. copies the body's skin weights onto it and binds it to the skeleton,
  4. hides the body's faces it covers.
Then plays three animations and renders frames to one sheet.

    spike/model_bakeoff/run.sh ../character_gear/fit.py
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "model_bakeoff"))

import bmesh
import bpy
import numpy as np
from mathutils import Matrix, Vector
from mathutils.bvhtree import BVHTree

import look

FIGURE = os.path.join(HERE, "..", "..", "assets", "models", "character", "figure.gltf")
GEAR = os.path.join(HERE, "..", "model_bakeoff", "out")
OUT = os.path.join(HERE, "out")

# Which bones' vertices make up each slot's region of the body.
REGIONS = {
    "chest": ["DEF-hips", "DEF-spine.001", "DEF-spine.002", "DEF-spine.003", "DEF-shoulder.L", "DEF-shoulder.R"],
    "boot.L": ["DEF-shin.L", "DEF-foot.L", "DEF-toe.L"],
    "boot.R": ["DEF-shin.R", "DEF-foot.R", "DEF-toe.R"],
    "head": ["DEF-head"],
}


def region_points(body, bones):
    """Rest-pose positions of the body's vertices whose strongest bone is one of `bones`."""
    names = {g.index: g.name for g in body.vertex_groups}
    pts = []
    for v in body.data.vertices:
        if not v.groups:
            continue
        g = max(v.groups, key=lambda g: g.weight)
        if names[g.group] in bones:
            pts.append(body.matrix_world @ v.co)
    return np.array([tuple(p) for p in pts])


def bounds(points):
    return points.min(axis=0), points.max(axis=0)


def mesh_points(ob):
    return np.array([tuple(ob.matrix_world @ v.co) for v in ob.data.vertices])


def import_gear(name):
    before = set(bpy.context.scene.objects)
    bpy.ops.import_scene.gltf(filepath=os.path.join(GEAR, f"refined-{name}-{name}.glb"))
    new = [o for o in bpy.context.scene.objects if o not in before]
    ob = next(o for o in new if o.type == "MESH")
    ob.parent = None
    # Tripo's models face +X; the body faces -Y.
    ob.data.transform(Matrix.Rotation(-math.pi / 2, 4, "Z") @ ob.matrix_world)
    ob.matrix_world = Matrix.Identity(4)
    for o in new:
        if o is not ob:
            bpy.data.objects.remove(o)
    return ob


def fit_box(ob, lo, hi, pad=(1.06, 1.06, 1.0), lift=0.0):
    """Scales and moves a piece so its bounds match a region's, padded a little."""
    g_lo, g_hi = bounds(mesh_points(ob))
    size, gsize = hi - lo, g_hi - g_lo
    scale = [size[i] * pad[i] / max(gsize[i], 1e-6) for i in range(3)]
    centre = (lo + hi) / 2
    gcentre = (g_lo + g_hi) / 2
    m = (Matrix.Translation(Vector(centre) + Vector((0, 0, lift))) @ Matrix.Diagonal((*scale, 1))
         @ Matrix.Translation(-Vector(gcentre)))
    ob.data.transform(m)
    ob.data.update()


def split_pair(ob):
    """A pair of boots as two objects, cut down the middle: left (+X) and right. Anything
    joining the two (a sole, a strap, a base) is cut away rather than left to stretch."""
    halves = {}
    for side, keep in (("L", 1), ("R", -1)):
        half = ob.copy()
        half.data = ob.data.copy()
        bpy.context.scene.collection.objects.link(half)
        bm = bmesh.new()
        bm.from_mesh(half.data)
        bmesh.ops.bisect_plane(bm, geom=bm.verts[:] + bm.edges[:] + bm.faces[:], plane_co=(0, 0, 0),
                               plane_no=(-keep, 0, 0), clear_outer=True)
        # Drop the thin sliver nearest the cut, which belonged to whatever joined the pair.
        xs = [abs(v.co.x) for v in bm.verts]
        near = sorted(xs)[max(0, len(xs) // 25)]
        cut = [f for f in bm.faces if all(abs(v.co.x) <= near + 0.002 for v in f.verts)]
        bmesh.ops.delete(bm, geom=cut, context="FACES")
        bm.to_mesh(half.data)
        bm.free()
        halves[side] = half
    bpy.data.objects.remove(ob)
    return halves


def push_out(ob, body, offset=0.012):
    mod = ob.modifiers.new("PushOut", "SHRINKWRAP")
    mod.target = body
    mod.wrap_method = "NEAREST_SURFACEPOINT"
    mod.wrap_mode = "OUTSIDE"
    mod.offset = offset
    apply(ob, mod)


def skin(ob, body, arm):
    for g in body.vertex_groups:
        ob.vertex_groups.new(name=g.name)
    dt = ob.modifiers.new("Weights", "DATA_TRANSFER")
    dt.object = body
    dt.use_vert_data = True
    dt.data_types_verts = {"VGROUP_WEIGHTS"}
    dt.vert_mapping = "POLYINTERP_NEAREST"
    dt.layers_vgroup_select_src = "ALL"
    dt.layers_vgroup_select_dst = "NAME"
    apply(ob, dt)
    ob.parent = arm
    ob.matrix_parent_inverse = arm.matrix_world.inverted()
    mod = ob.modifiers.new("Armature", "ARMATURE")
    mod.object = arm


def apply(ob, mod):
    bpy.ops.object.select_all(action="DESELECT")
    ob.select_set(True)
    bpy.context.view_layer.objects.active = ob
    bpy.ops.object.modifier_apply(modifier=mod.name)


def hide_covered(body, pieces, margin=0.025):
    """Deletes the body's faces that lie inside a piece or just under its surface (within
    `margin`), so skin cannot poke through as the body moves."""
    trees = []
    for p in pieces:
        bm = bmesh.new()
        bm.from_mesh(p.data)
        bm.transform(p.matrix_world)
        trees.append(BVHTree.FromBMesh(bm))
        bm.free()
    inv = body.matrix_world
    inside = []
    for v in body.data.vertices:
        w = inv @ v.co
        hidden = False
        for t in trees:
            hit = t.find_nearest(w)
            if hit[0] is not None and (w - hit[0]).dot(hit[1]) < margin and (w - hit[0]).length < 0.12:
                hidden = True
                break
        inside.append(hidden)
    bm = bmesh.new()
    bm.from_mesh(body.data)
    gone = [f for f in bm.faces if all(inside[v.index] for v in f.verts)]
    bmesh.ops.delete(bm, geom=gone, context="FACES")
    bm.to_mesh(body.data)
    bm.free()
    return len(gone)


# Build --------------------------------------------------------------------------------------

look.reset()
look.PALETTE["Brass"] = (0.80, 0.62, 0.30)
bpy.ops.import_scene.gltf(filepath=FIGURE)
for o in list(bpy.context.scene.objects):
    if o.type == "MESH" and o.name.startswith("Icosphere"):
        bpy.data.objects.remove(o)
arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
body = next(o for o in bpy.context.scene.objects if o.type == "MESH")
for slot in body.material_slots:
    slot.material = look.material("Skin")
arm.animation_data_clear()
for b in arm.pose.bones:
    b.matrix_basis.identity()
bpy.context.view_layer.update()

report = {}
pieces = []

jerkin = import_gear("Jerkin")
lo, hi = bounds(region_points(body, REGIONS["chest"]))
lo[2] = lo[2] + (hi[2] - lo[2]) * 0.12   # from the hips' top, not the thighs
fit_box(jerkin, lo, hi, pad=(1.08, 1.32, 1.02))
pieces.append(jerkin)

boots = import_gear("Boots")
pair = split_pair(boots)
for side, ob in pair.items():
    lo, hi = bounds(region_points(body, REGIONS[f"boot.{side}"]))
    hi[2] = lo[2] + (hi[2] - lo[2]) * 0.82  # to just below the knee
    fit_box(ob, lo, hi, pad=(1.25, 1.12, 1.0), lift=0.004)
    pieces.append(ob)

helmet = import_gear("Helmet")
lo, hi = bounds(region_points(body, REGIONS["head"]))
lo[2] = lo[2] + (hi[2] - lo[2]) * 0.38  # crown and brow, not the jaw
fit_box(helmet, lo, hi, pad=(1.18, 1.18, 1.0), lift=0.025)
pieces.append(helmet)

for p in pieces:
    push_out(p, body)
covered = hide_covered(body, pieces)
for p in pieces:
    skin(p, body, arm)
report["hidden_body_faces"] = covered
report["pieces"] = {p.name: len(p.data.polygons) for p in pieces}
print("REPORT", report)

# Render: three animations, four moments each, from the front three-quarters.

look.stage()
look.SUN_YAW, look.SUN_PITCH = 50, 40
scene = bpy.context.scene
scene.render.resolution_x = scene.render.resolution_y = 360
cam = bpy.data.objects.new("Cam", bpy.data.cameras.new("Cam"))
cam.data.lens = 50
scene.collection.objects.link(cam)
scene.camera = cam
sun = bpy.data.objects["Sun"]
os.makedirs(OUT, exist_ok=True)
rows = []
for action_name in ("Idle_Loop", "Walk_Loop", "Sword_Attack"):
    action = bpy.data.actions[action_name]
    ad = arm.animation_data or arm.animation_data_create()
    ad.action = action
    if getattr(action, "slots", None) and len(action.slots):
        ad.action_slot = action.slots[0]
    start, end = action.frame_range
    tiles = []
    for k, (yaw, t) in enumerate(((30, 0.0), (30, 0.33), (300, 0.66), (120, 0.85))):
        scene.frame_set(int(start + (end - start) * t))
        y = math.radians(yaw)
        target = Vector((0, 0, 0.95))
        cam.location = target + Vector((math.sin(y), -math.cos(y), 0.18)) * 3.4
        cam.rotation_euler = (target - cam.location).to_track_quat("-Z", "Y").to_euler()
        ly, lp = y + math.radians(50), math.radians(40)
        towards = Vector((math.sin(ly) * math.cos(lp), -math.cos(ly) * math.cos(lp), math.sin(lp)))
        sun.rotation_euler = (-towards).to_track_quat("-Z", "Y").to_euler()
        tile = os.path.join(OUT, f"tile-{action_name}-{k}.png")
        scene.render.filepath = tile
        bpy.ops.render.render(write_still=True)
        tiles.append(tile)
    row = os.path.join(OUT, f"row-{action_name}.png")
    look.stitch(tiles, row)
    for t in tiles:
        os.remove(t)
    rows.append(row)
look.stack(rows, os.path.join(OUT, "fitted.png"))
for r in rows:
    os.remove(r)
bpy.ops.export_scene.gltf(filepath=os.path.join(OUT, "fitted.glb"), export_format="GLB")
print("SHEET", os.path.join(OUT, "fitted.png"))
