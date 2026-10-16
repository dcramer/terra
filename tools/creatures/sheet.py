"""A preview sheet of an animal in the game's light, to check against docs/art-style.md: a row
of it standing (beside the figure for scale, then from the front, the side, three-quarter and
behind), then a row for each clip, its moments from the side.

    tools/blender/run.sh tools/creatures/sheet.py MODEL.glb SHEET.png [--cpu] [--clips Walk,Run] [--moments 5] [--side 90]

`--cpu` renders with Cycles on the processor, for when the GPU is another's; without it, EEVEE,
under the GPU's turn: `flock ~/.cache/terra/gpu.lock tools/blender/run.sh ...`.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "characters"))

import bpy
from mathutils import Vector

import look

argv = sys.argv[sys.argv.index("--") + 1:]
src, sheet = argv[:2]


def option(name, default):
    return argv[argv.index(name) + 1] if name in argv else default


cpu = "--cpu" in argv
moments = int(option("--moments", "5"))
side = float(option("--side", "90"))
look.reset()
scene = bpy.context.scene
scene.render.fps = 24
look.stage()
if cpu:
    scene.render.engine = "CYCLES"
    scene.cycles.device = "CPU"
    scene.cycles.samples = 16
    scene.cycles.use_denoising = True
scene.render.resolution_x = scene.render.resolution_y = 360
bpy.ops.import_scene.gltf(filepath=src)
rig = next(o for o in scene.objects if o.type == "ARMATURE")
body = [o for o in scene.objects if o.type == "MESH" and o.parent is rig]
names = [a.name for a in bpy.data.actions]
wanted = option("--clips", ",".join(names)).split(",")

before = set(scene.objects)
bpy.ops.import_scene.gltf(filepath=look.FIGURE)
figure = [o for o in scene.objects if o not in before]
for o in figure:
    if o.type == "ARMATURE" and o.animation_data:
        o.animation_data.action = None

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
    act = bpy.data.actions[clip]
    rig.animation_data.action = act
    if act.slots:
        rig.animation_data.action_slot = act.slots[0]
    scene.frame_set(frame)


def bounds():
    deps = bpy.context.evaluated_depsgraph_get()
    pts = []
    for o in body:
        ev = o.evaluated_get(deps)
        me = ev.to_mesh()
        pts += [ev.matrix_world @ v.co for v in me.vertices]
        ev.to_mesh_clear()
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    return lo, hi


def shot(path, yaw, pitch, lo, hi):
    centre = (lo + hi) / 2
    dist = max((hi - lo).length / 2, 0.1) / math.sin(cam.data.angle / 2) * 0.9
    y, p = math.radians(yaw), math.radians(pitch)
    cam.location = centre + Vector((math.sin(y) * math.cos(p), -math.cos(y) * math.cos(p), math.sin(p))) * dist
    cam.rotation_euler = (centre - cam.location).to_track_quat("-Z", "Y").to_euler()
    ly, lp = y + math.radians(40), math.radians(45)
    towards = Vector((math.sin(ly) * math.cos(lp), -math.cos(ly) * math.cos(lp), math.sin(lp)))
    sun.rotation_euler = (-towards).to_track_quat("-Z", "Y").to_euler()
    cam.data.clip_start = dist / 100
    scene.render.filepath = path
    bpy.ops.render.render(write_still=True)
    return path


pose(None, 1)
rest_lo, rest_hi = bounds()
height = rest_hi.z - rest_lo.z
for o in figure:
    if o.parent is None:
        o.location = Vector((max(0.35, (rest_hi.x - rest_lo.x) * 0.5 + 0.35), 0.0, 0))
print("size", tuple(round(v, 3) for v in rest_hi - rest_lo))
rows = []
tiles = []
for i, (yaw, pitch_deg, with_figure) in enumerate([(35, 10, True), (0, 8, False), (90, 5, False), (35, 18, False), (200, 15, False)]):
    for o in figure:
        o.hide_render = not with_figure
    lo, hi = rest_lo.copy(), rest_hi.copy()
    if with_figure:
        hi = Vector((hi.x + 1.0, hi.y, max(hi.z, 1.85)))
    tiles.append(shot(f"{sheet}.still{i}.png", yaw, pitch_deg, lo, hi))
for o in figure:
    o.hide_render = True
rows.append(f"{sheet}.row0.png")
look.stitch(tiles, rows[-1])
for t in tiles:
    os.remove(t)
for clip in wanted:
    act = bpy.data.actions[clip]
    first, last = act.frame_range
    frames = [round(first + (last - first) * k / moments) for k in range(moments)]
    # One frame of reference for the whole clip, so motion shows against it.
    lo, hi = rest_lo.copy(), rest_hi.copy()
    for f in frames:
        pose(clip, f)
        a, b = bounds()
        lo = Vector((min(lo.x, a.x), min(lo.y, a.y), min(lo.z, a.z)))
        hi = Vector((max(hi.x, b.x), max(hi.y, b.y), max(hi.z, b.z)))
    tiles = []
    for k, f in enumerate(frames):
        pose(clip, f)
        tiles.append(shot(f"{sheet}.{clip}{k}.png", side, 8, lo, hi))
    rows.append(f"{sheet}.row-{clip}.png")
    look.stitch(tiles, rows[-1])
    for t in tiles:
        os.remove(t)
    print("row", clip, "frames", frames)
look.stack(rows, sheet)
for r in rows:
    os.remove(r)
print("SHEET", sheet)
