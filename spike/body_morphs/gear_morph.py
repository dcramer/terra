"""Carries a look made on the base onto a morph of it (the spike's README): each part of the
look that follows the skin takes the morph of the skin under it, vertex by vertex; a part that
stands off the body (a pack's bag) moves as one, by the morph of the skin it rests on.

    tools/blender/run.sh spike/body_morphs/gear_morph.py BASE.glb MORPH.npz LOOK.glb OUT.glb
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "..", "tools", "characters"))

import bpy
import numpy as np
from mathutils import Vector
from mathutils.kdtree import KDTree

import look

base_path, morph_path, look_path, out_path = look.args()[:4]
FOLLOWS_M = 0.035  # a part this close to the skin, over most of it, follows it vertex by vertex
look.reset()


def load(path):
    before = set(bpy.context.scene.objects)
    bpy.ops.import_scene.gltf(filepath=os.path.join(look.ROOT, path))
    return [o for o in bpy.context.scene.objects if o not in before]


base = load(base_path)
body = max((o for o in base if o.type == "MESH"), key=lambda o: len(o.data.vertices))
V = np.array([tuple(body.matrix_world @ v.co) for v in body.data.vertices])
delta = np.load(os.path.join(look.ROOT, morph_path))["delta"]
kd = KDTree(len(V))
for i, p in enumerate(V):
    kd.insert(Vector(p), i)
kd.balance()
for o in base:
    bpy.data.objects.remove(o)

piece = load(look_path)
for o in piece:
    if o.type != "MESH":
        continue
    mw = o.matrix_world
    inv = mw.inverted()
    P = np.array([tuple(mw @ v.co) for v in o.data.vertices])
    near = [kd.find(Vector(p)) for p in P]
    dist = np.array([n[2] for n in near])
    d = np.array([delta[n[1]] for n in near])
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
            # Stands off the body: moves as one, with the skin nearest it.
            # by the skin nearest it, then out along the way it stands off the body far enough
            # that it keeps its clearance over the skin that moved out most beneath it.
            nearest = members & (dist <= np.percentile(dist[members], 20))
            shift = d[nearest].mean(axis=0)
            out = (P[members] - V[[n[1] for n, m in zip(near, members) if m]]).mean(axis=0)
            out /= max(np.linalg.norm(out), 1e-9)
            extra = max(0.0, float(((d[members] - shift) @ out).max()))
            moved[members] = shift + out * extra
    for v, m in zip(o.data.vertices, moved):
        v.co = inv @ (mw @ v.co + Vector(m))
    print("CARRIED", o.name, len(P), "vertices,", len(np.unique(part)), "parts")
bpy.ops.object.select_all(action="SELECT")
bpy.ops.export_scene.gltf(filepath=os.path.join(look.ROOT, out_path), export_format="GLB", use_selection=True, export_animations=False, export_extras=True)
print("EXPORTED", out_path)
