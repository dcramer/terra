"""Bodies side by side at rest, from the front, the side, the back and close on the face, in
the game's light, for a person to look at.

    tools/blender/run.sh tools/characters/compare.py OUT.png [--clay] [--face] [--linear|--dqs] [--pose=CLIP,F] [--focus=BONE,M] a.glb b.glb@90 c.glb+d.glb ...

`@degrees` turns a model about Z first (a model facing +X, as Tripo's do, takes @-90); a second
`@metres` scales it to that height, feet on the ground (Tripo returns models a unit tall).
`a.glb+b.glb` stands several files together: a body and what it wears. --clay draws them all
in one plain material, to see shape alone; --face only the faces. --pose holds every body F of
the way through one of its clips, --focus closes on one bone from four sides, and --linear or
--dqs skins with one method throughout instead of the game's weight-blended skinning.
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy
from mathutils import Matrix, Vector

import look

out, *models = look.args()
flags = {m for m in models if m.startswith("--")}
models = [m for m in models if not m.startswith("--")]
clay = "--clay" in flags
# Skinned as the game skins (look.skin_as_game); --linear or --dqs: all one way, to compare.
method = "linear" if "--linear" in flags else "dqs" if "--dqs" in flags else None
only_face = "--face" in flags
# --pose=CLIP,FRACTION: every body held that far through one of its own clips, and
# --focus=BONE,METRES: the camera on that bone of the first body, that wide.
pose = next((f.split("=", 1)[1].split(",") for f in flags if f.startswith("--pose=")), None)
focus = next((f.split("=", 1)[1].split(",") for f in flags if f.startswith("--focus=")), None)
look.reset()
look.stage()
scene = bpy.context.scene
GAP = float(next((f.split("=", 1)[1] for f in flags if f.startswith("--gap=")), 0.9))
for n, spec in enumerate(models):
    path, *rest = spec.split("@")
    turn = rest[0] if rest else 0
    tall = float(rest[1]) if len(rest) > 1 else None
    before = set(bpy.context.scene.objects)
    had = set(bpy.data.actions)
    # "a.glb+b.glb" stands several files together: a body and what it wears, put on as the game
    # puts it on: a skinned look bound to the body's skeleton (its own thrown away), a rigid
    # one hung on the socket its extras name.
    body_parts = None
    for k, part in enumerate(path.split("+")):
        had_objects = set(bpy.context.scene.objects)
        bpy.ops.import_scene.gltf(filepath=os.path.join(look.ROOT, part))
        added = [o for o in bpy.context.scene.objects if o not in had_objects]
        if k == 0:
            body_parts = added
            continue
        rig = next((o for o in body_parts if o.type == "ARMATURE"), None)
        sockets = {o.name.split(".0")[0] if o.name.count(".") > 2 else o.name: o for o in body_parts if o.name.startswith("socket.")}
        for o in added:
            if o.type != "MESH":
                continue
            mods = [m for m in o.modifiers if m.type == "ARMATURE"]
            if mods and rig is not None:
                mw = o.matrix_world.copy()
                o.parent = rig
                o.matrix_world = mw
                for m in mods:
                    m.object = rig
            elif o.get("socket") in sockets:
                local = o.matrix_world.copy()
                o.parent = sockets[o["socket"]]
                o.matrix_parent_inverse.identity()
                o.matrix_basis = local
        for o in added:
            if o.type == "ARMATURE" or (o.type == "EMPTY" and not o.children):
                bpy.data.objects.remove(o)
    new = [o for o in bpy.context.scene.objects if o not in before]
    for o in new:
        if o.type == "ARMATURE":
            if o.animation_data:
                o.animation_data.action = None
            for pb in o.pose.bones:
                pb.matrix_basis.identity()
            if pose:
                # This file's own clip: a second file's copy of a name comes in as "Name.001",
                # and every body must play its own (each body's clips are sampled on it).
                mine = [a for a in bpy.data.actions if a not in had]
                action = next((a for a in mine if a.name.split(".")[0] == pose[0]), None)
                if action is None:
                    raise SystemExit(f"{path} has no clip {pose[0]}")
                o.animation_data_create()
                o.animation_data.action = action
                if getattr(action, "slots", None) and len(action.slots):
                    o.animation_data.action_slot = action.slots[0]
                o["pose_fraction"] = float(pose[1])
    roots = [o for o in new if o.parent is None]
    if clay:
        for o in new:
            if o.type == "MESH":
                o.data.materials.clear()
                o.data.materials.append(look.material("Skin"))
    m = Matrix.Translation((n * GAP - (len(models) - 1) * GAP / 2, 0, 0)) @ Matrix.Rotation(math.radians(float(turn or 0)), 4, "Z")
    if tall:
        bpy.context.view_layer.update()
        zs = [(o.matrix_world @ v.co).z for o in new if o.type == "MESH" for v in o.data.vertices]
        k = tall / (max(zs) - min(zs))
        m = m @ Matrix.Translation((0, 0, -min(zs) * k)) @ Matrix.Scale(k, 4)
    for o in roots:
        o.matrix_world = m @ o.matrix_world

look.skin_as_game(method=method)
if pose:
    # Every body's copy of the clip has the same frames; hold them all at the fraction.
    rigs = [o for o in scene.objects if o.type == "ARMATURE" and o.animation_data and o.animation_data.action]
    if rigs:
        start, end = rigs[0].animation_data.action.frame_range
        scene.frame_set(round(start + (end - start) * float(pose[1])))
        print("POSED", ", ".join(f"{o.name}: {o.animation_data.action.name}" for o in rigs))
cam = bpy.data.objects.new("Cam", bpy.data.cameras.new("Cam"))
scene.collection.objects.link(cam)
scene.camera = cam
cam.data.type = "ORTHO"
scene.render.resolution_x = 320 * len(models)
scene.render.resolution_y = 640
tiles = []
width = GAP * len(models)
views = [(0, 0.95, max(2.1, width * 1.05)), (90, 0.95, max(2.1, width * 1.05)), (180, 0.95, max(2.1, width * 1.05)), (0, 1.62, 0.35 * len(models) * 1.6)]
if focus:
    # Each body on its own, close on the same bone: rows are the sides, columns the bodies.
    # Ortho cameras look along a horizontal line, so another body never stands in front.
    rigs = [o for o in scene.objects if o.type == "ARMATURE" and focus[0] in o.pose.bones]
    span = float(focus[1]) if len(focus) > 1 else 0.5
    columns = [look.close_up(rig, focus[0], span) for rig in rigs]
    rows = []
    for k, cells in enumerate(zip(*columns)):
        row = os.path.join(look.OUT, f"compare-{os.getpid()}-row-{k}.png")
        look.stitch(list(cells), row)
        rows.append(row)
    look.stack(rows, os.path.join(look.ROOT, out))
    print("COMPARED", out)
    raise SystemExit(0)
elif only_face:
    views = [(0, 1.62, GAP * len(models)), (60, 1.62, GAP * len(models))]
    scene.render.resolution_x = 512 * len(models)
    GAP = 0.9
for k, (side, z, scale) in enumerate(views):
    a = math.radians(side)
    d = Vector((math.sin(a), -math.cos(a), 0)) * 6
    cam.location = Vector((0, 0, z)) + d
    cam.rotation_euler = (math.radians(90), 0, a)
    cam.data.ortho_scale = scale
    if k == 3 or only_face:
        scene.render.resolution_y = 512 if only_face else 320
    path = os.path.join(look.OUT, f"compare-{k}.png")
    scene.render.filepath = path
    bpy.ops.render.render(write_still=True)
    tiles.append(path)
look.stack(tiles, os.path.join(look.ROOT, out))
print("COMPARED", out)
