"""Stills of a clip at given moments, from the pose sheet's camera, tiled in a strip to lay beside
the sheet. Run through tools/blender/run.sh:

    tools/blender/run.sh spike/pose-sheet/stills.py OUT.png BODY.glb CLIP SIDE t1,t2,... [empty]

"empty" hangs no sword or shield and comes in close on the arms.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../tools/characters"))

import bpy
from mathutils import Vector

import look

FPS = 30
out, body, clip, side, moments, *rest = look.args()
empty = bool(rest)
out = look.path(out)
look.reset()
scene = bpy.context.scene
scene.render.fps = FPS
look.stage()
scene.eevee.taa_render_samples = 8
scene.eevee.use_raytracing = False
scene.render.resolution_x, scene.render.resolution_y = 360, 600

bpy.ops.import_scene.gltf(filepath=look.path(body))
arm = next(o for o in scene.objects if o.type == "ARMATURE")
look.skin_as_game()
for s, path in () if empty else zip("RL", ("assets/models/held/arming_sword.glb", "assets/models/held/heater_shield.glb")):
    socket = next(o for o in scene.objects if o.name.startswith(f"socket.hand.{s}"))
    before = set(scene.objects)
    bpy.ops.import_scene.gltf(filepath=look.path(path))
    for o in set(scene.objects) - before:
        if o.parent is None:
            o.parent = socket
            o.matrix_parent_inverse.identity()
            o.location = (0, 0, 0)
            o.rotation_mode = "QUATERNION"
            o.rotation_quaternion = (1, 0, 0, 0)

action = bpy.data.actions.get(clip) or next(a for a in bpy.data.actions if a.name.split(".")[0] == clip)
ad = arm.animation_data or arm.animation_data_create()
ad.action = action
if getattr(action, "slots", None) and len(action.slots):
    ad.action_slot = action.slots[0]
start = action.frame_range[0]

cam = bpy.data.objects.new("Cam", bpy.data.cameras.new("Cam"))
cam.data.lens = 50
scene.collection.objects.link(cam)
scene.camera = cam
a = math.radians(float(side))
look_at = Vector((0, 0, 0.95)) if empty else Vector((0, -0.2, 1.05))
cam.location = look_at + Vector((-math.sin(a), -math.cos(a), 0)) * (2.6 if empty else 5.2) + Vector((0, 0, 0.25))
cam.rotation_euler = (look_at - cam.location).to_track_quat("-Z", "Y").to_euler()

tiles = []
for k, t in enumerate(float(x) for x in moments.split(",")):
    # Blender's import keeps the file's times; a key frame is seconds at the scene's rate.
    f = start + t * scene.render.fps
    scene.frame_set(int(f), subframe=f - int(f))
    path = out + f".{k}.png"
    scene.render.filepath = path
    bpy.ops.render.render(write_still=True)
    tiles.append(path)
look.stitch(tiles, out)
print("STILLS", out, len(tiles))
