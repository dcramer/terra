"""Bodies side by side at rest, from the front, the side, the back and close on the face.

    tools/blender/run.sh spike/body_morphs/compare.py OUT.png a.glb b.glb@90 ...

`@degrees` turns a model about Z first (a rig written facing +X takes @-90); a second
`@metres` scales it to that height, feet on the ground (Tripo returns models a unit tall).
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "..", "tools", "characters"))

import bpy
from mathutils import Matrix, Vector

import look

out, *models = look.args()
flags = {m for m in models if m.startswith("--")}
models = [m for m in models if not m.startswith("--")]
clay = "--clay" in flags
only_face = "--face" in flags
look.reset()
look.stage()
scene = bpy.context.scene
GAP = 0.9
for n, spec in enumerate(models):
    path, *rest = spec.split("@")
    turn = rest[0] if rest else 0
    tall = float(rest[1]) if len(rest) > 1 else None
    before = set(bpy.context.scene.objects)
    # "a.glb+b.glb" stands several files together: a body and what it wears.
    for part in path.split("+"):
        bpy.ops.import_scene.gltf(filepath=os.path.join(look.ROOT, part))
    new = [o for o in bpy.context.scene.objects if o not in before]
    for o in new:
        if o.type == "ARMATURE" and o.animation_data:
            o.animation_data.action = None
            for pb in o.pose.bones:
                pb.matrix_basis.identity()
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

cam = bpy.data.objects.new("Cam", bpy.data.cameras.new("Cam"))
scene.collection.objects.link(cam)
scene.camera = cam
cam.data.type = "ORTHO"
scene.render.resolution_x = 320 * len(models)
scene.render.resolution_y = 640
tiles = []
width = GAP * len(models)
views = [(0, 0.95, max(2.1, width * 1.05)), (90, 0.95, max(2.1, width * 1.05)), (180, 0.95, max(2.1, width * 1.05)), (0, 1.62, 0.35 * len(models) * 1.6)]
if only_face:
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
    path = os.path.join(HERE, "out", f"cmp-{k}.png")
    scene.render.filepath = path
    bpy.ops.render.render(write_still=True)
    tiles.append(path)
look.stack(tiles, os.path.join(look.ROOT, out))
print("COMPARED", out)
