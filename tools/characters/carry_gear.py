"""Carries a worn look made on the base body onto the base's morphs, so gear is made once (D128;
research/body-morphs.md).

    tools/blender/run.sh tools/characters/carry_gear.py BASE.glb LOOK.glb OUT.glb MORPH
    tools/blender/run.sh tools/characters/carry_gear.py BASE.glb LOOK.glb OUT.glb --morphs=heavy,thin,...

With one MORPH (tools/characters/out/MORPH.npz) the look is reshaped to it, for checking it
on that morph's body (make_morph.sh). With --morphs the look keeps its shape and gains one morph
target per morph, in that order, as the base body's file has them (export_character.py
--morphs=...), so in the game one look follows any mix of them: the game's file for the item.

Each part of the look (its connected pieces, joined across texture seams) that lies on the skin
(median distance under 3.5 cm) takes the morph of the nearest skin, vertex by vertex: a jerkin,
a cap, a pack's straps. (Turning each vertex's offset with the skin's normal instead was tried
and did worse: a strap's inner and outer faces turned apart.) A part that stands off the body (a pack's bag) moves as one piece, by
the skin nearest it, and out far enough to keep its clearance over the skin that moved out most
beneath it. A look on a socket (no skin) is left as it is: its socket moves instead. The look
keeps its skin weights, sockets and extras.
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy
import numpy as np
from mathutils import Vector
from mathutils.kdtree import KDTree

import look

base_path, look_path, out_path = look.args()[:3]
rest = [a for a in look.args()[3:]]
MORPHS = next((a.split("=", 1)[1].split(",") for a in rest if a.startswith("--morphs=")), None)
APPLY = None if MORPHS is not None else rest[0]
FOLLOWS_M = 0.035  # a part this close to the skin, over most of it, follows it vertex by vertex
look.reset()


def load(path):
    before = set(bpy.context.scene.objects)
    bpy.ops.import_scene.gltf(filepath=os.path.join(look.ROOT, path))
    return [o for o in bpy.context.scene.objects if o not in before]


base = load(base_path)
body = max((o for o in base if o.type == "MESH"), key=lambda o: len(o.data.vertices))
V = np.array([tuple(body.matrix_world @ v.co) for v in body.data.vertices])
kd = KDTree(len(V))
for i, p in enumerate(V):
    kd.insert(Vector(p), i)
kd.balance()
for o in base:
    bpy.data.objects.remove(o)


def morph_delta(name):
    """The morph per vertex of the base body in BASE, matched by position."""
    data = np.load(os.path.join(look.OUT, f"{name}.npz"))
    mkd = KDTree(len(data["points"]))
    for i, p in enumerate(data["points"]):
        mkd.insert(Vector(p), i)
    mkd.balance()
    return np.array([data["delta"][mkd.find(Vector(p))[1]] for p in V])


def carried(o, delta):
    """How far each vertex of `o` moves under the base's morph `delta` (world space)."""
    mw = o.matrix_world
    P = np.array([tuple(mw @ v.co) for v in o.data.vertices])
    near = [kd.find(Vector(p)) for p in P]
    dist = np.array([n[2] for n in near])
    under = np.array([n[1] for n in near])
    d = delta[under]
    # Parts, by connection (glTF splits vertices at seams, so join by position too).
    root = list(range(len(P)))

    def find(i):
        while root[i] != i:
            root[i] = root[root[i]]
            i = root[i]
        return i

    first = {}
    for i, p in enumerate(np.round(P, 5)):
        j = first.setdefault(tuple(p), i)
        root[find(i)] = find(j)
    for poly in o.data.polygons:
        for j in poly.vertices[1:]:
            root[find(poly.vertices[0])] = find(j)
    part = np.array([find(i) for i in range(len(P))])
    moved = d.copy()
    for k in np.unique(part):
        members = part == k
        if np.median(dist[members]) > FOLLOWS_M:
            # Stands off the body: moves as one, by the skin nearest it, then out along the
            # way it stands off the body far enough to keep its clearance over the skin that
            # moved out most beneath it.
            nearest = members & (dist <= np.percentile(dist[members], 20))
            shift = d[nearest].mean(axis=0)
            out = (P[members] - V[under[members]]).mean(axis=0)
            out /= max(np.linalg.norm(out), 1e-9)
            extra = max(0.0, float(((d[members] - shift) @ out).max()))
            moved[members] = shift + out * extra
    return moved


piece = load(look_path)
skinned = [o for o in piece if o.type == "MESH" and any(m.type == "ARMATURE" for m in o.modifiers)]
deltas = {m: morph_delta(m) for m in (MORPHS if MORPHS is not None else [APPLY])}
for o in skinned:
    inv = o.matrix_world.inverted().to_3x3()
    if APPLY is not None:
        for v, m in zip(o.data.vertices, carried(o, deltas[APPLY])):
            v.co = v.co + inv @ Vector(m)
    else:
        if o.data.shape_keys is None:
            o.shape_key_add(name="Basis", from_mix=False)
        for name in MORPHS:
            key = o.shape_key_add(name=name, from_mix=False)
            for v, m in zip(o.data.vertices, carried(o, deltas[name])):
                key.data[v.index].co = v.co + inv @ Vector(m)
    print("CARRIED", o.name, len(o.data.vertices), "vertices", "onto " + APPLY if APPLY else "with morphs " + ",".join(MORPHS))
if not skinned:
    print("CARRIED nothing: a look on a socket, which moves with its socket")
bpy.ops.object.select_all(action="SELECT")
bpy.ops.export_scene.gltf(filepath=os.path.join(look.ROOT, out_path), export_format="GLB", use_selection=True,
                          export_animations=False, export_extras=True, export_morph=True)
print("EXPORTED", out_path)
