"""Renders a rigged body through our animations: a row per animation, four moments each,
and a close row of the deepest crouch, to look at what the checker's numbers mean.

    tools/blender/run.sh tools/characters/rig_render.py tools/characters/out/base-male-tripo-s2.glb sheet-name
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy
from mathutils import Vector

import look
import twist

FIGURE = look.FIGURE
OUT = os.path.join(HERE, "out")
src, name = look.args()[:2]
ACTIONS = [a for a in (look.args()[2:] or ["Idle_Loop", "Walk_Loop", "Sword_Attack", "Crouch_Idle_Loop", "Roll"])]

look.reset()
bpy.ops.import_scene.gltf(filepath=look.path(src))
arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
body = max((o for o in bpy.context.scene.objects if o.type == "MESH"), key=lambda o: len(o.data.vertices))
for o in list(bpy.context.scene.objects):
    if o not in (arm, body):
        bpy.data.objects.remove(o)
before = set(bpy.data.objects)
bpy.ops.import_scene.gltf(filepath=FIGURE)  # brings the animations
for o in list(bpy.data.objects):
    if o not in before:
        bpy.data.objects.remove(o)
twist.bake(arm, list(bpy.data.actions))
look.skin_as_game()
look.stage()
scene = bpy.context.scene
scene.render.resolution_x = scene.render.resolution_y = 380
cam = bpy.data.objects.new("Cam", bpy.data.cameras.new("Cam"))
cam.data.lens = 50
scene.collection.objects.link(cam)
scene.camera = cam
sun = bpy.data.objects["Sun"]
rows = []
for action_name in ACTIONS:
    action = bpy.data.actions.get(action_name) or next(a for a in bpy.data.actions if a.name.startswith(action_name))
    ad = arm.animation_data or arm.animation_data_create()
    ad.action = action
    if getattr(action, "slots", None) and len(action.slots):
        ad.action_slot = action.slots[0]
    start, end = action.frame_range
    tiles = []
    for k, (yaw, t, dist) in enumerate(((30, 0.0, 3.4), (30, 0.45, 3.4), (300, 0.7, 3.4), (60, 0.45, 1.9))):
        scene.frame_set(int(start + (end - start) * t))
        y = math.radians(yaw)
        target = Vector((0, 0, 0.85 if k < 3 else 0.7))
        cam.location = target + Vector((math.sin(y), -math.cos(y), 0.2)) * dist
        cam.rotation_euler = (target - cam.location).to_track_quat("-Z", "Y").to_euler()
        ly, lp = y + math.radians(50), math.radians(40)
        towards = Vector((math.sin(ly) * math.cos(lp), -math.cos(ly) * math.cos(lp), math.sin(lp)))
        sun.rotation_euler = (-towards).to_track_quat("-Z", "Y").to_euler()
        tile = os.path.join(OUT, f"rr-{name}-{action_name}-{k}.png")
        scene.render.filepath = tile
        bpy.ops.render.render(write_still=True)
        tiles.append(tile)
    row = os.path.join(OUT, f"rr-{name}-{action_name}.png")
    look.stitch(tiles, row)
    for t in tiles:
        os.remove(t)
    rows.append(row)
look.stack(rows, os.path.join(OUT, f"{name}.png"))
for r in rows:
    os.remove(r)
print("SHEET", os.path.join(OUT, f"{name}.png"))
