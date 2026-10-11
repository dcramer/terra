"""Renders any finished .glb from a concept's camera, in the same plain light as the
image-to-Blender attempts, so the routes compare like for like.

    run.sh image/render_match.py out/refined-Wolf-Wolf3000.glb wolf out/match-tripo-wolf.png
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, ".."))

import bpy
from mathutils import Matrix, Vector

import look

src, view, out = look.args()
look.reset()
bpy.ops.import_scene.gltf(filepath=os.path.join(look.HERE, src))
ob = next(o for o in bpy.context.scene.objects if o.type == "MESH")
ob.parent = None
ob.data.transform(ob.matrix_world)
ob.matrix_world = Matrix.Identity(4)
look.ground_to_origin(ob)
for o in list(bpy.context.scene.objects):
    if o is not ob:
        bpy.data.objects.remove(o)
look.stage()
bpy.data.objects["Ground"].hide_render = True
scene = bpy.context.scene
scene.render.film_transparent = True
scene.render.resolution_x = scene.render.resolution_y = 1024
scene.view_settings.view_transform = "Standard"
cam = bpy.data.objects.new("Cam", bpy.data.cameras.new("Cam"))
cam.data.lens = 50
scene.collection.objects.link(cam)
scene.camera = cam
sun = bpy.data.objects["Sun"]
scene.world.node_tree.nodes["Background"].inputs["Color"].default_value = (0.75, 0.75, 0.78, 1)
size = max(ob.dimensions)
if view == "workbench":
    target = Vector((0, 0, ob.dimensions.z * 0.5))
    yaw, pitch, dist = math.radians(-22), math.radians(33), 3.7 * size / 1.6
    cam.location = target + Vector((math.sin(yaw) * math.cos(pitch), -math.cos(yaw) * math.cos(pitch), math.sin(pitch))) * dist
    sun.rotation_euler = Vector((0.5, 0.8, -0.7)).to_track_quat("-Z", "Y").to_euler()
    sun.data.energy = 2.2
    scene.world.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.7
else:
    target = Vector((0, 0, ob.dimensions.z * 0.45))
    # Tripo's models face +X like ours; the concept looks at the wolf from ahead of its shoulder.
    cam.location = target + Vector((2.6, -1.9, 0.8)) * size / 1.7
    sun.rotation_euler = Vector((-0.4, 0.8, -0.6)).to_track_quat("-Z", "Y").to_euler()
    sun.data.energy = 2.4
    scene.world.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.8
sun.data.color = (1.0, 0.97, 0.92)
cam.rotation_euler = (target - cam.location).to_track_quat("-Z", "Y").to_euler()
scene.render.filepath = os.path.join(look.HERE, out)
bpy.ops.render.render(write_still=True)
