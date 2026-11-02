"""D600's two ways side by side: for each state of undress `slots.ts` measured, the swap figure
(a bare piece in each empty slot) in the top row and the hidden figure (their whole body under
what is worn, each worn slot's region of skin taken out as the game collapses its patches: the
patches `slots.ts` lists, made within the regions by `cut_slots.py`) in the bottom row, the
same moments from the same camera: Sword_Idle and Sprint_Loop at two moments each, then the
state's worst poke and worst open border of either way.

    tools/blender/run.sh tools/characters/slot_stills.py tools/characters/out/slots-male-Ranger.json [STATE ...]

Writes tools/characters/out/d600/d600-OUTFIT-STATE.png, a line each to stdout (Blender runs on
Windows, so copy them to the review desk's ~/.cache/terra-review/char-clothing/ from WSL).
Render only: nothing is written back to any piece (D599).
"""

import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bmesh
import bpy
from mathutils import Vector

import look
import retarget
import twist

SIZE = 420
STILLS = os.path.join(look.OUT, "d600")
data_path, *only = look.args()
with open(look.path(data_path)) as f:
    data = json.load(f)
mix = [f"{s}={o}" for s, o in data["outfits"].items() if o != data["outfits"]["body"]]
label = "-".join([data["outfits"]["body"], *[m.replace("=", "-") for m in mix]]).lower()


def moment(where):
    """'Clip@1.25' (seconds) -> (clip, seconds); 'rest' -> None."""
    if where == "rest" or "@" not in where:
        return None
    clip, t = where.split(" ")[0].split("@")
    return clip, float(t)


def figure(files, hidden):
    """Imports a figure from its files (the first brings the skeleton), takes out the hidden
    patches' faces of the whole body (every face of the worn slots' regions), and bakes the
    clips it needs onto it."""
    look.reset()
    arm, meshes = look.join("+".join(look.path(f) for f in files))
    if hidden:
        body = max(meshes, key=lambda o: len(o.data.polygons))
        layer = body.data.uv_layers[-1]
        gone = set(hidden)
        bm = bmesh.new()
        bm.from_mesh(body.data)
        uv = bm.loops.layers.uv[layer.name]
        bm.faces.ensure_lookup_table()
        bmesh.ops.delete(bm, geom=[f for f in bm.faces if int(f.loops[0][uv].uv[0]) in gone], context="FACES_ONLY")
        bm.to_mesh(body.data)
        bm.free()
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=look.FIGURE)
    mannequin = retarget.source(next(o for o in bpy.data.objects if o not in before and o.type == "ARMATURE"))
    for o in list(bpy.data.objects):
        if o not in before:
            bpy.data.objects.remove(o)
    wanted = {s[0] for s in shots}
    for a in list(bpy.data.actions):
        if a.name.split(".")[0] not in wanted:
            bpy.data.actions.remove(a)
    retarget.bake(arm, list(bpy.data.actions), mannequin)
    twist.bake(arm, list(bpy.data.actions))
    look.skin_as_game()
    look.stage()
    return arm


def render_row(arm, path):
    scene = bpy.context.scene
    scene.render.resolution_x = scene.render.resolution_y = SIZE
    cam = bpy.data.objects.new("Cam", bpy.data.cameras.new("Cam"))
    cam.data.lens = 50
    scene.collection.objects.link(cam)
    scene.camera = cam
    sun = bpy.data.objects["Sun"]
    fps = scene.render.fps / scene.render.fps_base
    tiles = []
    for k, (clip, at, yaw, height, dist) in enumerate(shots):
        action = bpy.data.actions.get(clip) or next(a for a in bpy.data.actions if a.name.startswith(clip))
        ad = arm.animation_data or arm.animation_data_create()
        ad.action = action
        if getattr(action, "slots", None) and len(action.slots):
            ad.action_slot = action.slots[0]
        start, end = action.frame_range
        # A fraction of the clip below 1, or seconds from the measure (marked by a negative).
        scene.frame_set(int(round(start + (end - start) * at)) if at >= 0 else int(round(start - at * fps)))
        y = math.radians(yaw)
        target = Vector((0, 0, height))
        cam.location = target + Vector((math.sin(y), -math.cos(y), 0.15)) * dist
        cam.rotation_euler = (target - cam.location).to_track_quat("-Z", "Y").to_euler()
        ly, lp = y + math.radians(50), math.radians(40)
        towards = Vector((math.sin(ly) * math.cos(lp), -math.cos(ly) * math.cos(lp), math.sin(lp)))
        sun.rotation_euler = (-towards).to_track_quat("-Z", "Y").to_euler()
        tile = os.path.join(look.OUT, f"ss-{os.getpid()}-{k}.png")
        scene.render.filepath = tile
        bpy.ops.render.render(write_still=True)
        tiles.append(tile)
    look.stitch(tiles, path)
    for t in tiles:
        os.remove(t)


os.makedirs(STILLS, exist_ok=True)
states = [s for s in dict.fromkeys(f["state"] for f in data["figures"]) if not only or s in only]
for state in states:
    swap, hid = (next(f for f in data["figures"] if f["state"] == state and f["way"] == w) for w in ("swap", "hidden"))
    # Four set moments, then the worst poke and the worst open border of either way, framed
    # closer on where they are.
    shots = [("Sword_Idle", 0.25, 30, 0.95, 2.7), ("Sword_Idle", 0.65, 210, 0.95, 2.7), ("Sprint_Loop", 0.25, 60, 0.95, 2.7), ("Sprint_Loop", 0.75, 300, 0.95, 2.7)]
    for key in ("poke", "open"):
        worst = max((swap, hid), key=lambda f: f["worst"][key][0])
        m = moment(worst["worst"][key][1])
        if m:
            # Framed on where the worst poke is, from the measure's "(piece on bone, H m up)".
            up = worst["worst"][key][1].rsplit(", ", 1)[-1].split(" m up")[0] if key == "poke" and " m up" in worst["worst"][key][1] else None
            shots.append((m[0], -m[1], 30 if key == "poke" else 150, max(0.3, float(up)) if up else 0.8, 1.6 if up else 2.4))
    rows = []
    for way, f in (("swap", swap), ("hidden", hid)):
        arm = figure(f["files"], f.get("hidden") if way == "hidden" else None)
        row = os.path.join(look.OUT, f"ss-{os.getpid()}-{way}.png")
        render_row(arm, row)
        rows.append(row)
    out = os.path.join(STILLS, f"d600-{label}-{state.replace(' ', '-')}.png")
    look.stack(rows, out)
    for r in rows:
        os.remove(r)
    print("STILL", out, "|", " ".join(f"{c}@{abs(a):.2f}{'s' if a < 0 else ''}" for c, a, *_ in shots))
