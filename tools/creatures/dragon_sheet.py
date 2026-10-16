"""Contact sheets of the dragon: its rest pose from four sides beside the figure for scale, and
each clip through its loop from the front and the three-quarter, skinned as the game skins it.

    tools/blender/run.sh tools/creatures/dragon_sheet.py assets/models/creatures/Dragon.glb SHEET.png [cycles|workbench] [CLIP,...]

Writes SHEET-rest.png and SHEET-<clip>.png. Cycles renders on the CPU, small and few samples,
so it never takes the GPU from a running game.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "characters"))

import bpy
from mathutils import Vector

import look

args = sys.argv[sys.argv.index("--") + 1:]
src, sheet = args[0], args[1]
engine = args[2] if len(args) > 2 else "cycles"
only = args[3].split(",") if len(args) > 3 else None
base = sheet[:-4] if sheet.endswith(".png") else sheet

look.reset()
scene = bpy.context.scene
scene.render.fps = 30
look.stage()
bpy.data.objects["Ground"].location.z = -4.5
if engine == "cycles":
    scene.render.engine = "CYCLES"
    scene.cycles.device = "CPU"
    scene.cycles.samples = 12
    scene.cycles.use_denoising = True
    scene.cycles.max_bounces = 3
else:
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.display.shading.light = "STUDIO"
    scene.display.shading.color_type = "TEXTURE"
    scene.display.shading.show_shadows = False
scene.render.resolution_x, scene.render.resolution_y = 480, 300
bpy.ops.import_scene.gltf(filepath=src)
rig = next(o for o in scene.objects if o.type == "ARMATURE")
look.skin_as_game()
clips = sorted(a.name for a in bpy.data.actions)

before = set(scene.objects)
bpy.ops.import_scene.gltf(filepath=look.FIGURE)
figure = [o for o in scene.objects if o not in before]
for o in figure:
    if o.type == "ARMATURE" and o.animation_data:
        o.animation_data.action = None
    if o.parent is None:
        o.location = Vector((3.0, -3.0, -4.5))

cam = bpy.data.objects.new("Cam", bpy.data.cameras.new("Cam"))
cam.data.lens = 40
scene.collection.objects.link(cam)
scene.camera = cam
sun = bpy.data.objects["Sun"]


def shot(path, yaw, pitch, centre, radius):
    """From `yaw` degrees round from the dragon's front (toward its left), `pitch` above."""
    dist = radius / math.sin(cam.data.angle / 2) * 0.8
    y, p = math.radians(yaw), math.radians(pitch)
    cam.location = centre + Vector((math.sin(y) * math.cos(p), -math.cos(y) * math.cos(p), math.sin(p))) * dist
    cam.rotation_euler = (centre - cam.location).to_track_quat("-Z", "Y").to_euler()
    ly, lp = y + math.radians(40), math.radians(50)
    towards = Vector((math.sin(ly) * math.cos(lp), -math.cos(ly) * math.cos(lp), math.sin(lp)))
    sun.rotation_euler = (-towards).to_track_quat("-Z", "Y").to_euler()
    scene.render.filepath = path
    bpy.ops.render.render(write_still=True)
    return path


def play(clip, frame):
    rig.animation_data_create()
    if clip is None:
        rig.animation_data.action = None
        for pb in rig.pose.bones:
            pb.matrix_basis.identity()
    else:
        act = bpy.data.actions[clip]
        rig.animation_data.action = act
        if act.slots:
            rig.animation_data.action_slot = act.slots[0]
    scene.frame_set(frame)


centre = Vector((0, 0, 0))
mouth = bpy.data.objects["Mouth"]
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.05)
marker = bpy.context.object
glow = bpy.data.materials.new("Marker")
glow.diffuse_color = (1.0, 0.85, 0.0, 1.0)
glow.use_nodes = True
glow.node_tree.nodes["Principled BSDF"].inputs["Emission Color"].default_value = (1.0, 0.85, 0.0, 1.0)
glow.node_tree.nodes["Principled BSDF"].inputs["Emission Strength"].default_value = 5.0
marker.data.materials.append(glow)
marker.hide_render = True
made = []
if not only:
    tiles = []
    for o in figure:
        o.hide_render = False
    play(None, 1)
    for i, (yaw, pitch) in enumerate([(35, 15), (0, 5), (90, 5), (0, 89)]):
        tiles.append(shot(f"{base}.rest{i}.png", yaw, pitch, Vector((0, 0.5, -0.5)), 9.5))
    look.stitch(tiles, f"{base}-rest.png")
    made.append(f"{base}-rest.png")
    for t in tiles:
        os.remove(t)
for o in figure:
    o.hide_render = True
for name in clips:
    act = bpy.data.actions[name]
    if only and name not in only:
        continue
    start, end = (int(v) for v in act.frame_range)
    n = end - start
    frames = [start + round(n * k / 4) for k in range(4)]
    rows = []
    for r, (yaw, pitch) in enumerate([(0, 6), (90, 4), (40, 18)]):
        tiles = [shot(f"{base}.{name}.{r}.{i}.png", yaw, pitch, centre, 11.0) for i, f in enumerate(frames) if play(name, f) or True]
        look.stitch(tiles, f"{base}.{name}.row{r}.png")
        rows.append(f"{base}.{name}.row{r}.png")
        for t in tiles:
            os.remove(t)
    # The head, close, with the mouth's socket marked by a small yellow ball.
    tiles = []
    for i, f in enumerate(frames):
        play(name, f)
        bpy.context.view_layer.update()
        marker.location = mouth.matrix_world.translation
        marker.hide_render = False
        head = rig.matrix_world @ rig.pose.bones["Head"].matrix @ Vector((0, rig.pose.bones["Head"].length * 0.5, 0))
        tiles.append(shot(f"{base}.{name}.head.{i}.png", (75, 35, -30, 110)[i], (0, -20, -10, 10)[i], head, 2.2))
    marker.hide_render = True
    look.stitch(tiles, f"{base}.{name}.row3.png")
    rows.append(f"{base}.{name}.row3.png")
    for t in tiles:
        os.remove(t)
    look.stack(rows, f"{base}-{name}.png")
    for r in rows:
        os.remove(r)
    made.append(f"{base}-{name}.png")
    print("clip", name, "frames", frames)
print("SHEETS", " ".join(made))
