"""Preview sheet of the hen in the game's light: front, side, three-quarter beside the figure,
then a walk, a peck and a look-about frame, close.

    tools/blender/run.sh tools/creatures/sheet.py assets/models/creatures/Chicken.glb SHEET.png
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "characters"))

import bpy
from mathutils import Vector

import look

src, sheet = sys.argv[sys.argv.index("--") + 1:][:2]
look.reset()
scene = bpy.context.scene
scene.render.fps = 24
look.stage()
scene.render.resolution_x = scene.render.resolution_y = 420
bpy.ops.import_scene.gltf(filepath=src)
rig = next(o for o in scene.objects if o.type == "ARMATURE")
hen = [o for o in scene.objects if o.type == "MESH" and o.parent is rig]

# The figure, for scale, beside the hen in the first view only.
before = set(scene.objects)
bpy.ops.import_scene.gltf(filepath=look.FIGURE)
figure = [o for o in scene.objects if o not in before]
for o in figure:
    if o.type == "ARMATURE" and o.animation_data:
        o.animation_data.action = None
for o in figure:
    if o.parent is None:
        o.location = Vector((0.55, 0.15, 0))

cam = bpy.data.objects.new("Cam", bpy.data.cameras.new("Cam"))
cam.data.lens = 50
scene.collection.objects.link(cam)
scene.camera = cam
sun = bpy.data.objects["Sun"]


def pose(clip, frame):
    rig.animation_data_create()
    if clip is None:
        rig.animation_data.action = None
        for pb in rig.pose.bones:
            pb.matrix_basis.identity()
        scene.frame_set(1)
        return
    act = next(a for a in bpy.data.actions if a.name.startswith(clip))
    rig.animation_data.action = act
    if act.slots:
        rig.animation_data.action_slot = act.slots[0]
    scene.frame_set(frame)


def shot(path, yaw, pitch, lo, hi):
    centre = (lo + hi) / 2
    dist = max((hi - lo).length / 2, 0.2) / math.sin(cam.data.angle / 2) * 0.85
    y, p = math.radians(yaw), math.radians(pitch)
    cam.location = centre + Vector((math.sin(y) * math.cos(p), -math.cos(y) * math.cos(p), math.sin(p))) * dist
    cam.rotation_euler = (centre - cam.location).to_track_quat("-Z", "Y").to_euler()
    ly, lp = y + math.radians(40), math.radians(45)
    towards = Vector((math.sin(ly) * math.cos(lp), -math.cos(ly) * math.cos(lp), math.sin(lp)))
    sun.rotation_euler = (-towards).to_track_quat("-Z", "Y").to_euler()
    scene.render.filepath = path
    bpy.ops.render.render(write_still=True)
    return path


hen_lo, hen_hi = Vector((-0.15, -0.25, 0)), Vector((0.15, 0.27, 0.52))
views = [
    # (label, clip, frame, yaw, pitch, figure shown)
    ("scale", None, 1, 35, 12, True),
    ("front", None, 1, 0, 8, False),
    ("side", None, 1, 90, 5, False),
    ("three-quarter", None, 1, 35, 18, False),
    ("back", None, 1, 200, 15, False),
    ("walk f5", "Walk", 5, 90, 5, False),
    ("walk f9 3/4", "Walk", 9, 40, 15, False),
    ("peck f14", "Peck", 14, 70, 10, False),
    ("idle f30", "Idle", 30, 20, 12, False),
]
tiles = []
for i, (label, clip, frame, yaw, pitch_deg, with_figure) in enumerate(views):
    for o in figure:
        o.hide_render = not with_figure
    pose(clip, frame)
    lo, hi = hen_lo.copy(), hen_hi.copy()
    if with_figure:
        lo = Vector((-0.2, -0.3, 0))
        hi = Vector((0.9, 0.5, 1.85))
    tiles.append(shot(f"{sheet}.{i}.png", yaw, pitch_deg, lo, hi))
    print("view", i, label)
look.stitch(tiles[:5], sheet.replace(".png", "-still.png"))
look.stitch(tiles[5:], sheet.replace(".png", "-anim.png"))
look.stack([sheet.replace(".png", "-still.png"), sheet.replace(".png", "-anim.png")], sheet) if False else None
for t in tiles:
    os.remove(t)
print("SHEETS", sheet.replace(".png", "-still.png"), sheet.replace(".png", "-anim.png"))
