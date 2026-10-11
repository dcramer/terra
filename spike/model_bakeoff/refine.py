"""Makes a generated model ours: one flat palette colour per face, read from its texture,
at the size it should be, checked and rendered in the game's light.

    run.sh refine.py out/tripo/wolf-p1.glb wolf 1.0 Fur FurDark FurLight Black

The arguments after the name are the model's height in metres and the palette colours it
may use; each face gets the one nearest its texture's colour there.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import bpy
import numpy as np
from mathutils import Matrix

import look

src, name, height, *allowed = look.args()
height = float(height)
look.reset()
bpy.ops.import_scene.gltf(filepath=os.path.join(look.HERE, src))
meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
ob = look.join(meshes, name) if len(meshes) > 1 else meshes[0]
ob.name = name
ob.parent = None
ob.data.transform(ob.matrix_world)
ob.matrix_world = Matrix.Identity(4)


def texture_of(mat):
    if not mat or not mat.use_nodes:
        return None
    for node in mat.node_tree.nodes:
        if node.type == "TEX_IMAGE" and node.image:
            img = node.image
            w, h = img.size
            return np.array(img.pixels[:], dtype=np.float32).reshape(h, w, 4)
    return None


me = ob.data
textures = [texture_of(m) for m in me.materials]
uv = me.uv_layers.active.data if me.uv_layers else None
palette = np.array([look.PALETTE[n] for n in allowed])
picked = []
for poly in me.polygons:
    tex = textures[poly.material_index] if poly.material_index < len(textures) else None
    if tex is None or uv is None:
        picked.append(0)
        continue
    h, w = tex.shape[:2]
    loops = list(poly.loop_indices)
    corners = np.array([uv[i].uv for i in loops])
    centre = corners.mean(axis=0)
    # Sample the centre and points two thirds of the way to each corner.
    samples = np.vstack([centre, centre + (corners - centre) * 0.66])
    xs = np.clip((samples[:, 0] % 1.0) * (w - 1), 0, w - 1).astype(int)
    ys = np.clip((samples[:, 1] % 1.0) * (h - 1), 0, h - 1).astype(int)
    colour = tex[ys, xs, :3].mean(axis=0)
    # An 8-bit sRGB texture's pixels come back as stored, already sRGB, as the palette is.
    picked.append(int(np.argmin(((palette - colour) ** 2).sum(axis=1))))

me.materials.clear()
for n in allowed:
    me.materials.append(look.material(n))
for poly, i in zip(me.polygons, picked):
    poly.material_index = i
look.flat(ob)
while me.uv_layers:
    me.uv_layers.remove(me.uv_layers[0])

# Generators split the mesh at every UV seam; weld it back into one surface.
import bmesh

bm = bmesh.new()
bm.from_mesh(me)
bmesh.ops.remove_doubles(bm, verts=bm.verts[:], dist=1e-4)
bm.to_mesh(me)
bm.free()

# Clean up speckle: a face whose neighbours mostly share another colour takes theirs, so
# colours sit in broad areas as they do in the concept.
from collections import Counter

bm = bmesh.new()
bm.from_mesh(me)
bm.faces.ensure_lookup_table()
for _ in range(2):
    current = [f.material_index for f in bm.faces]
    for f in bm.faces:
        around = Counter(current[g.index] for e in f.edges for g in e.link_faces if g is not f)
        if around:
            colour, count = around.most_common(1)[0]
            if colour != current[f.index] and count >= 2:
                f.material_index = colour
bm.to_mesh(me)
bm.free()

look.ground_to_origin(ob)
s = height / ob.dimensions.z
ob.data.transform(Matrix.Scale(s, 4))
look.ground_to_origin(ob)
look.finish([ob], f"refined-{name}", budget=3000)
