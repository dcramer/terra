"""Renders models from straight in front, one picture each, so which way each faces and
whether it stands level can be seen at a glance (props.ts fronts lays them out).

    tools/blender/run.sh tools/props/fronts.py OUTDIR MODEL.glb...

Every model the game loads should face the standard way, glTF's +Z (Blender's -Y), stand
level, and have its foot at the origin. Each is seen from that front, a little above, in the
game's light, over a ground line at the origin's height, so a model facing sideways, leaning
or floating shows. Writes OUTDIR/<model>.png.
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "characters"))

import bpy
from mathutils import Vector

import look

out, *models = look.args()
os.makedirs(look.path(out), exist_ok=True)
for model in models:
    look.reset()
    look.stage()
    bpy.ops.import_scene.gltf(filepath=look.path(model))
    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH" and o.name != "Ground"]
    bpy.context.view_layer.update()
    pts = [o.matrix_world @ Vector(c) for o in meshes for c in o.bound_box]
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    scene = bpy.context.scene
    scene.render.resolution_x = scene.render.resolution_y = 360
    cam = bpy.data.objects.new("Cam", bpy.data.cameras.new("Cam"))
    scene.collection.objects.link(cam)
    scene.camera = cam
    centre = Vector(((lo.x + hi.x) / 2, (lo.y + hi.y) / 2, max(hi.z, 0.05) / 2))
    radius = max((hi - Vector((lo.x, lo.y, min(lo.z, 0)))).length / 2, 0.1)
    pitch = math.radians(20)
    dist = radius / math.sin(cam.data.angle / 2) * 1.05
    cam.location = centre + Vector((0, -math.cos(pitch), math.sin(pitch))) * dist
    cam.rotation_euler = (centre - cam.location).to_track_quat("-Z", "Y").to_euler()
    scene.render.filepath = os.path.join(look.path(out), os.path.splitext(os.path.basename(model))[0] + ".png")
    bpy.ops.render.render(write_still=True)
