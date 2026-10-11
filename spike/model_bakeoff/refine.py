"""Makes a generated model ours: one flat palette colour per face, read from its texture,
at the size it should be, checked and rendered in the game's light.

    run.sh refine.py out/tripo/wolf-p1.glb Wolf 1.0 0,4000,2500 Fur FurDark FurLight Black

After the name: the model's height in metres; the triangle budgets to make, as a list
(0 keeps the generated mesh as it is), each becoming a row of the sheet so the lowest one
that still holds its shape can be chosen by eye; then the palette colours it may use.
Each face gets the one nearest its texture's colour there.
"""

import os
import sys
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import bmesh
import bpy
import numpy as np
from mathutils import Matrix

import kit
import look

look.PALETTE.update(kit.ROLES)

args = look.args()
# --smooth: shade smooth (hard edges kept), as characters in the painted look are.
smooth = "--smooth" in args
src, name, height, budgets, *allowed = [a for a in args if a != "--smooth"]
height = float(height)
budgets = [int(b) for b in budgets.split(",")]
palette = np.array([look.PALETTE[n] for n in allowed])


def texture_of(mat):
    if not mat or not mat.use_nodes:
        return None
    for node in mat.node_tree.nodes:
        if node.type == "TEX_IMAGE" and node.image:
            img = node.image
            w, h = img.size
            return np.array(img.pixels[:], dtype=np.float32).reshape(h, w, 4)
    return None


def load(label):
    before = set(bpy.context.scene.objects)
    bpy.ops.import_scene.gltf(filepath=os.path.join(look.HERE, src))
    new = [o for o in bpy.context.scene.objects if o not in before]
    meshes = [o for o in new if o.type == "MESH"]
    ob = look.join(meshes, label) if len(meshes) > 1 else meshes[0]
    ob.name = label
    ob.parent = None
    ob.data.transform(ob.matrix_world)
    ob.matrix_world = Matrix.Identity(4)
    for o in new:
        if o is not ob:
            bpy.data.objects.remove(o)
    # The file's own smoothed normals would override flat shading, here and in the game.
    for o in bpy.context.scene.objects:
        o.select_set(o is ob)
    bpy.context.view_layer.objects.active = ob
    if ob.data.has_custom_normals:
        bpy.ops.mesh.customdata_custom_splitnormals_clear()
    return ob


def weld(me):
    """Generators split the mesh at every UV seam; weld it back into one surface. UVs live
    on face corners, so they survive."""
    bm = bmesh.new()
    bm.from_mesh(me)
    bmesh.ops.remove_doubles(bm, verts=bm.verts[:], dist=1e-4)
    bm.to_mesh(me)
    bm.free()


def reduce(ob, budget):
    tris = sum(len(p.vertices) - 2 for p in ob.data.polygons)
    if budget <= 0 or tris <= budget:
        return
    dec = ob.modifiers.new("Reduce", "DECIMATE")
    dec.ratio = budget / tris
    dec.use_collapse_triangulate = True
    bpy.context.view_layer.objects.active = ob
    bpy.ops.object.modifier_apply(modifier=dec.name)


def paint(ob):
    """Each face takes the palette colour nearest its texture's colour."""
    me = ob.data
    textures = [texture_of(m) for m in me.materials]
    uv = me.uv_layers.active.data if me.uv_layers else None
    picked = []
    for poly in me.polygons:
        tex = textures[poly.material_index] if poly.material_index < len(textures) else None
        if tex is None or uv is None:
            picked.append(0)
            continue
        h, w = tex.shape[:2]
        corners = np.array([uv[i].uv for i in poly.loop_indices])
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


def flatten_regions(me, angle=12.0):
    """Faces lying nearly flat against each other, such as one plank's top, share one
    colour: whichever covers most of their area. Texture shading otherwise splits them."""
    import math

    bm = bmesh.new()
    bm.from_mesh(me)
    bm.faces.ensure_lookup_table()
    limit = math.radians(angle)
    seen = set()
    for start in bm.faces:
        if start.index in seen:
            continue
        region, stack = [], [start]
        seen.add(start.index)
        while stack:
            f = stack.pop()
            region.append(f)
            for e in f.edges:
                for g in e.link_faces:
                    if g.index not in seen and f.normal.angle(g.normal, 0.0) < limit:
                        seen.add(g.index)
                        stack.append(g)
        if len(region) < 2:
            continue
        area = Counter()
        for f in region:
            area[f.material_index] += f.calc_area()
        colour = area.most_common(1)[0][0]
        for f in region:
            f.material_index = colour
    bm.to_mesh(me)
    bm.free()


def despeckle(me):
    """A face whose neighbours mostly share another colour takes theirs, so colours sit in
    broad areas as they do in the concept."""
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


look.reset()
models = []
for budget in budgets:
    ob = load(f"{name}{budget or ''}")
    weld(ob.data)
    reduce(ob, budget)
    paint(ob)
    flatten_regions(ob.data)
    despeckle(ob.data)
    look.ground_to_origin(ob)
    ob.data.transform(Matrix.Scale(height / ob.dimensions.z, 4))
    look.ground_to_origin(ob)
    if smooth:
        for o in bpy.context.scene.objects:
            o.select_set(o is ob)
        bpy.context.view_layer.objects.active = ob
        bpy.ops.object.shade_smooth_by_angle(angle=0.7)
    models.append(ob)
look.finish(models, f"refined-{name}", budget=max(budgets) or 20000)
