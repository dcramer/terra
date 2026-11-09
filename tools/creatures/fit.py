"""A model on a numbered grid, with its skeleton over it: for placing a creature's bones (D71).

    tools/blender/run.sh tools/creatures/fit.py NAME OUT.png

Draws art/creatures/NAME/model.glb, turned as its creature.json says (`turn`), from the front
(x across, z up), the side (y across, z up; the animal faces -y, to the left) and above (x
across, y down the picture), in the model's own units, the units `bones` are written in:
grid lines every tenth, numbered; the origin's lines heavier; `ground` drawn if given. Each
bone of creature.json is a line from its head (a dot) to its tail, mirrored ones too, so a
bone can be read off the picture and a fitted one checked against the body before the
animal is built (`animal.py`) and judged (`creature_check.py`, `sheet.py`).
"""

import json
import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "characters"))

import bpy
import numpy as np

import look

argv = sys.argv[sys.argv.index("--") + 1:]
NAME, OUT = argv[0], os.path.abspath(argv[1])
HOME = os.path.join(look.ART, "creatures", NAME)
SPEC = {}
if os.path.exists(os.path.join(HOME, "creature.json")):
    with open(os.path.join(HOME, "creature.json")) as f:
        SPEC = json.load(f)

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=os.path.join(HOME, "model.glb"))
points = []
for ob in bpy.data.objects:
    if ob.type == "MESH":
        points += [tuple(ob.matrix_world @ v.co) for v in ob.data.vertices]
P = np.array(points)
turn = math.radians(SPEC.get("turn", 0))
P = np.stack([P[:, 0] * math.cos(turn) - P[:, 1] * math.sin(turn),
              P[:, 0] * math.sin(turn) + P[:, 1] * math.cos(turn), P[:, 2]], 1)

SIDE, PAD = 760, 30
low, high = P.min(0) - 0.05, P.max(0) + 0.05
span = float((high - low).max())
canvas = np.ones((SIDE + 2 * PAD, 3 * (SIDE + 2 * PAD), 3), np.float32)

# A 3 by 5 font for the grid's numbers.
FONT = {"0": "111101101101111", "1": "010110010010111", "2": "111001111100111", "3": "111001111001111",
        "4": "101101111001001", "5": "111100111001111", "6": "111100111101111", "7": "111001001001001",
        "8": "111101111101111", "9": "111101111001111", "-": "000000111000000", ".": "000000000000010"}


def dot(x, y, colour, size=1):
    x, y = int(round(x)), int(round(y))
    canvas[max(y - size, 0):y + size + 1, max(x - size, 0):x + size + 1] = colour


def text(x, y, said, colour=(0.2, 0.2, 0.2)):
    for i, c in enumerate(said):
        for k, on in enumerate(FONT.get(c, "0" * 15)):
            if on == "1":
                px, py = int(x) + i * 8 + (k % 3) * 2, int(y) + (k // 3) * 2
                canvas[py:py + 2, px:px + 2] = colour


def line(a, b, colour, size=1):
    steps = int(max(abs(b[0] - a[0]), abs(b[1] - a[1]))) + 1
    for s in range(steps + 1):
        t = s / steps
        dot(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, colour, size)


# The three views: which axis runs across and which up the picture, and whether it is flipped.
VIEWS = [("front: x across, z up", 0, 2, 1, -1), ("side: y across (ahead is left), z up", 1, 2, 1, -1),
         ("above: x across, y down (ahead is up)", 0, 1, 1, 1)]
bones = dict(SPEC.get("bones", {}))
middle = SPEC.get("middle", 0.0)
for name, (parent, head, tail) in list(bones.items()):
    if name.endswith(".L"):
        mirror = lambda v: [2 * middle - v[0], v[1], v[2]]
        bones[name[:-2] + ".R"] = (parent, mirror(head), mirror(tail))
for panel, (title, ax, ay, sx, sy) in enumerate(VIEWS):
    left = panel * (SIDE + 2 * PAD) + PAD

    def at(v, ax=ax, ay=ay, sy=sy, left=left):
        u = (v[ax] - low[ax]) / span
        w = (v[ay] - low[ay]) / span
        return left + u * SIDE, PAD + (1 - w if sy < 0 else w) * SIDE

    for axis, across in ((ax, True), (ay, False)):
        tick = math.ceil(low[axis] * 10) / 10
        while tick <= low[axis] + span:
            heavy = abs(tick) < 1e-6
            colour = (0.55, 0.55, 0.55) if heavy else (0.86, 0.86, 0.86)
            v0, v1 = list(low), list(low)
            v0[axis] = v1[axis] = tick
            other = ay if across else ax
            v1[other] = low[other] + span
            line(at(v0), at(v1), colour)
            where = at(v0) if across else at(v0)
            label = f"{tick:.1f}".replace("0.", ".").replace("-.", "-.")
            text(where[0] + 2, where[1] + 2 if across else where[1] - 12, label)
            tick = round(tick + 0.1, 6)
    if "ground" in SPEC and ay == 2:
        g0, g1 = list(low), list(low)
        g0[2] = g1[2] = SPEC["ground"]
        g1[ax] = low[ax] + span
        line(at(g0), at(g1), (0.3, 0.6, 0.3), 1)
    for v in P[:: max(1, len(P) // 9000)]:
        dot(*at(v), (0.45, 0.5, 0.6), 0)
    for name, (parent, head, tail) in bones.items():
        colour = (0.85, 0.2, 0.2) if name.endswith(".L") else (0.2, 0.35, 0.9) if name.endswith(".R") else (0.1, 0.1, 0.1)
        line(at(head), at(tail), colour, 1)
        dot(*at(head), colour, 3)
    text(left, 6, "".join(c if c in FONT else "-" for c in f"{panel + 1}"))

image = bpy.data.images.new("fit", canvas.shape[1], canvas.shape[0])
rgba = np.concatenate([canvas[::-1], np.ones(canvas.shape[:2] + (1,), np.float32)], 2)
image.pixels = rgba.ravel()
image.filepath_raw = OUT
image.file_format = "PNG"
image.save()
print("FIT", OUT, "bounds", np.round(P.min(0), 3), np.round(P.max(0), 3), "bones", len(bones))
