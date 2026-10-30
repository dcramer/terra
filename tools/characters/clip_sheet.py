"""Clips as made, on a body, for the eye: each entry played once at its own speed, the body
moved by the entry's root motion over a ground marked every half metre, a sword and a shield
in its hands, filmed from two sides, with a sheet of six moments of each. clip_sheet.ts runs it
and makes the film; run that, not this.

    tools/blender/run.sh tools/characters/clip_sheet.py OUT BODY.glb ROOTS.json HELD ENTRY...

ENTRY is a clip, or clips joined by + played one after another (a blow and its recovery).
ROOTS.json is {clip: [[x, y, z] per 1/30 s]}, the root's travel in glTF's axes from its first
frame, an empty object for none. HELD is right.glb,left.glb (or "none"), each hung on its hand's
socket by its origin. Writes OUT/a_NNNNN.png and OUT/b_NNNNN.png (the two sides), OUT/frames.json
(the entry each frame shows) and OUT/sheet.png.
"""

import json
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import bpy
from mathutils import Vector

import look

FPS = 30
# The sides filmed: degrees round from the front (the body faces -Y), + toward its right, and
# the camera's distance; the sheet uses the first.
SIDES = ((35, 4.2), (90, 4.2))
MOMENTS = 6

out, body, roots, held, *entries = look.args()
out = look.path(out)
os.makedirs(out, exist_ok=True)
with open(look.path(roots)) as fh:
    roots = json.load(fh)

look.reset()
scene = bpy.context.scene
scene.render.fps = FPS
look.stage()
scene.eevee.taa_render_samples = 8
scene.eevee.use_raytracing = False
scene.render.resolution_x, scene.render.resolution_y = 560, 600

bpy.ops.import_scene.gltf(filepath=look.path(body))
arm = next(o for o in scene.objects if o.type == "ARMATURE")
look.skin_as_game()
if held != "none":
    for side, path in zip("RL", held.split(",")):
        socket = next(o for o in scene.objects if o.name.startswith(f"socket.hand.{side}"))
        before = set(scene.objects)
        bpy.ops.import_scene.gltf(filepath=look.path(path))
        for o in set(scene.objects) - before:
            if o.parent is None:
                o.parent = socket
                o.matrix_parent_inverse.identity()
                o.location = (0, 0, 0)
                o.rotation_mode = "QUATERNION"
                # A held model's +Y runs along its handle, as the socket's does (export_character.py).
                o.rotation_quaternion = (1, 0, 0, 0)

# The ground marked every half metre across and along, so a step and a slide show.
mark = bpy.data.materials.new("Mark")
mark.use_nodes = True
mark.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.05, 0.05, 0.05, 1)
for k in range(-8, 9):
    for along in (True, False):
        bpy.ops.mesh.primitive_cube_add(size=1, location=(0, k * 0.5, 0.001) if along else (k * 0.5, 0, 0.001))
        line = bpy.context.object
        line.scale = (8, 0.012, 0.002) if along else (0.012, 8, 0.002)
        line.data.materials.append(mark)

# Each picture names its entry and moment in a band along its foot (Blender's stamp), nothing else.
r = scene.render
for flag in dir(r):
    if flag.startswith("use_stamp_") and isinstance(getattr(r, flag), bool):
        setattr(r, flag, False)
r.use_stamp = True
r.use_stamp_note = True
r.stamp_font_size = 20


def action(name):
    found = bpy.data.actions.get(name) or next((a for a in bpy.data.actions if a.name.split(".")[0] == name), None)
    if found is None:
        raise SystemExit(f"clip_sheet: no clip {name} in {body}; export the body with it (export_character.py --clips=)")
    return found


def play(a):
    ad = arm.animation_data or arm.animation_data_create()
    ad.action = a
    if getattr(a, "slots", None) and len(a.slots):
        ad.action_slot = a.slots[0]


def travel(name, t):
    """The root's travel `t` seconds into clip `name`, in Blender's axes."""
    curve = roots.get(name)
    if not curve:
        return Vector()
    at = min(t * FPS, len(curve) - 1)
    i = int(at)
    j = min(i + 1, len(curve) - 1)
    p = [a + (b - a) * (at - i) for a, b in zip(curve[i], curve[j])]
    return Vector((p[0], -p[2], p[1]))


# Each entry's frames: (clip, its time, where the body stands).
timeline = []
for entry in entries:
    at = Vector()
    for name in entry.split("+"):
        a = action(name)
        start, end = a.frame_range
        n = int(round((end - start)))
        for f in range(n + (1 if name == entry.split("+")[-1] else 0)):
            t = f / FPS
            timeline.append((entry, name, start + f, at + travel(name, t)))
        at = at + travel(name, n / FPS)

cam = bpy.data.objects.new("Cam", bpy.data.cameras.new("Cam"))
cam.data.lens = 50
scene.collection.objects.link(cam)
scene.camera = cam
base = arm.location.copy()


def shoot(side, dist, span, path, text):
    """From `side`, framing the entry's whole travel (`span`, its first and last place)."""
    a = math.radians(side)
    start, end = span
    look_at = (start + end) / 2 + Vector((0, -0.3, 0.95))
    d = Vector((-math.sin(a), -math.cos(a), 0))
    cam.location = look_at + d * (dist + 0.9 * (end - start).length) + Vector((0, 0, 0.9))
    cam.rotation_euler = (look_at - cam.location).to_track_quat("-Z", "Y").to_euler()
    r.stamp_note_text = text
    scene.render.filepath = path
    bpy.ops.render.render(write_still=True)


shown = []
sheet_rows = {}
for k, (entry, name, frame, at) in enumerate(timeline):
    play(action(name))
    scene.frame_set(int(frame))
    arm.location = base + at
    bpy.context.view_layer.update()
    # The cameras hold still over the entry, not following the body, so its travel shows against the marks.
    mine = [i for i, x in enumerate(timeline) if x[0] == entry]
    span = (timeline[mine[0]][3], timeline[mine[-1]][3])
    text = f"{entry.replace('+', ' + ')}   {(k - mine[0]) / FPS:.2f} s"
    for (side, dist), view in zip(SIDES, "ab"):
        shoot(side, dist, span, os.path.join(out, f"{view}_{k:05d}.png"), text)
    shown.append(entry)
for entry in entries:
    ks = [k for k, x in enumerate(timeline) if x[0] == entry]
    picks = [ks[round(i * (len(ks) - 1) / (MOMENTS - 1))] for i in range(MOMENTS)]
    sheet_rows[entry] = [os.path.join(out, f"a_{k:05d}.png") for k in picks]
rows = []
for n, (entry, tiles) in enumerate(sheet_rows.items()):
    row = os.path.join(out, f"row_{n:03d}.png")
    look.stitch(tiles, row)
    rows.append(row)
look.stack(rows, os.path.join(out, "sheet.png"))
with open(os.path.join(out, "frames.json"), "w") as fh:
    json.dump(shown, fh)
print("SHEET", out, len(timeline), "frames", len(entries), "entries")
