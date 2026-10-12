"""Makes a piece of clothing or armour for a base body from a model of that body wearing it
(D128, docs/characters.md), built from the base body itself so it cannot clip.

The dressed model is drawn from the base body's own sheet ("the same person, wearing ...") and
made by Tripo, as one outer surface. This:

  1. lines it up with the base body (same turn and height, nudged so their bare skin meets);
  2. marks the base body's skin the piece covers, by its coverage, one of COVERAGE: the body
     parts it covers, by the bones that move them, and how far its hem reaches. Said once per
     item, never per character, and nothing guessed from the dressed model's paint;
  3. builds the piece from those faces of the base body, pushed out by OFFSET_M, or as far as
     the dressed surface stands off up to BULK_M, so it has the dressed piece's bulk;
  4. keeps each vertex's skin weights from the base vertex it came from: the piece moves as the
     skin under it does, so skin cannot come through it in any pose;
  5. bakes the dressed model's colour onto it, and lists the patches of skin it hides.

    spike/model_bakeoff/run.sh ../character_gear/shell_garment.py ../../assets/models/character/base-male.glb out/tripo/dressed-male-coat.glb leather_coat Leather chest

The slot (D64) says where on the body the piece can be: a chest piece never reaches the thighs,
whatever colour Tripo gave the dressed model's shorts.

Writes assets/models/gear/<body>/<item>.glb and prints SHELL lines.
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "model_bakeoff"))
sys.path.insert(0, HERE)

import bmesh
import bpy
import numpy as np
from mathutils import Matrix, Vector
from mathutils.bvhtree import BVHTree

import look
import patches

OUT = os.path.join(HERE, "..", "..", "assets", "models", "gear")
LOOK_M = 0.06          # how far out and in from the skin to look for the dressed surface
STANDS_OFF_M = 0.008   # dressed surface further out than this is the piece, whatever its colour
SMOOTH_ROUNDS = 4      # rounds of smoothing the mark
SPECK = 0.03           # marked or unmarked islands smaller than this share of the mark are noise
WHOLE = 0.3            # marked islands smaller than this share of the largest are not the piece
OFFSET_M = 0.008       # how far out from the skin the piece sits
BULK_M = 0.03          # the most it follows the dressed surface standing off
CROSSED_M = 0.018      # and above the waist, where straps cross the chest, a chest piece's height
                       # off the skin, which they clear (fit_gear.py, docs/characters.md)
MATCH_M = 0.03         # skin this close lines the dressed model up
TEXTURE = 1024
COVERED = 1.0          # a patch wholly under the piece is hidden; part-covered ones stay, or a band of skin vanishes below a hem or rim
# What each kind of piece covers: the bones whose skin it covers, and the heights of its
# hems, as shares of the way from one joint to the next (0 at the first, 1 at the second):
# a chest piece's hem from the hips joint toward the knee, a sleeve from the shoulder toward
# the elbow or wrist.
COVERAGE = {
    "chest_sleeveless": {"bones": ("DEF-spine", "DEF-shoulder", "DEF-hips"), "hem": 0.25},
    "chest_short_sleeves": {"bones": ("DEF-spine", "DEF-shoulder", "DEF-hips", "DEF-upper_arm"), "hem": 0.25},
    "chest_long_sleeves": {"bones": ("DEF-spine", "DEF-shoulder", "DEF-hips", "DEF-upper_arm", "DEF-forearm"), "hem": 0.25},
    "legs_short": {"bones": ("DEF-hips", "DEF-thigh"), "hem": None},
    "legs_long": {"bones": ("DEF-hips", "DEF-thigh", "DEF-shin"), "hem": None},
    "gloves": {"bones": ("DEF-hand", "DEF-f_", "DEF-thumb"), "hem": None},
    "feet": {"bones": ("DEF-foot", "DEF-toe"), "hem": None},
    # A cap: the crown, down this share of the way from the top of the head to the neck.
    "head_cap": {"bones": ("DEF-head",), "hem": None, "crown": 0.45},
}

body_path, dressed_path, item, role, coverage = (look.args() + ["Cloth", "chest_sleeveless"])[:5]
COVER = COVERAGE[coverage]
body_name = os.path.splitext(os.path.basename(body_path))[0]

look.reset()
bpy.ops.import_scene.gltf(filepath=os.path.join(look.HERE, body_path))
arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
body = max((o for o in bpy.context.scene.objects if o.type == "MESH"), key=lambda o: len(o.data.vertices))
arm.animation_data_clear()
for pb in arm.pose.bones:
    pb.matrix_basis.identity()
bpy.context.view_layer.update()
# The file splits vertices at texture seams and patch borders; weld them, so the skin is one
# surface to mark and clean (faces keep their texture and patch coordinates).
bpy.ops.object.select_all(action="DESELECT")
body.select_set(True)
bpy.context.view_layer.objects.active = body
bpy.ops.object.mode_set(mode="EDIT")
bpy.ops.mesh.select_all(action="SELECT")
bpy.ops.mesh.remove_doubles(threshold=1e-5)
bpy.ops.object.mode_set(mode="OBJECT")
patch_of = patches.of(body) or {}
mw = body.matrix_world
skin_pts = [mw @ v.co for v in body.data.vertices]
skin_nrm = [(mw.to_3x3() @ v.normal).normalized() for v in body.data.vertices]
skin = BVHTree.FromPolygons(skin_pts, [tuple(p.vertices) for p in body.data.polygons])
top = max(p.z for p in skin_pts)

# 1. The dressed model, turned to face -Y as ours do (Tripo's face +X), scaled and lined up.
before = set(bpy.context.scene.objects)
bpy.ops.import_scene.gltf(filepath=os.path.join(look.HERE, dressed_path))
new = [o for o in bpy.context.scene.objects if o not in before]
dressed = max((o for o in new if o.type == "MESH"), key=lambda o: len(o.data.vertices))
dmw = dressed.matrix_world.copy()
dressed.parent = None
dressed.modifiers.clear()
dressed.data.transform(Matrix.Rotation(-math.pi / 2, 4, "Z") @ dmw)
dressed.matrix_world = Matrix.Identity(4)
for o in new:
    if o is not dressed:
        bpy.data.objects.remove(o)
zs = [v.co.z for v in dressed.data.vertices]
dressed.data.transform(Matrix.Scale(top / (max(zs) - min(zs)), 4) @ Matrix.Translation((0, 0, -min(zs))))
for _ in range(6):
    offsets = [skin.find_nearest(v.co, MATCH_M) for v in list(dressed.data.vertices)[::5]]
    offsets = [hit[0] - v.co for hit, v in zip(offsets, list(dressed.data.vertices)[::5]) if hit[0] is not None]
    if offsets:
        dressed.data.transform(Matrix.Translation(sum(offsets, Vector()) / len(offsets)))
dressed_tree = BVHTree.FromPolygons([v.co for v in dressed.data.vertices], [tuple(p.vertices) for p in dressed.data.polygons])


def image_of(ob):
    for slot in ob.material_slots:
        if slot.material and slot.material.use_nodes:
            for node in slot.material.node_tree.nodes:
                if node.type == "TEX_IMAGE" and node.image:
                    w, h = node.image.size
                    return node.image, np.array(node.image.pixels[:], dtype=np.float32).reshape(h, w, 4)[:, :, :3]
    return None, None


def colour(pixels, uv):
    h, w = pixels.shape[:2]
    return pixels[min(h - 1, max(0, int((uv[1] % 1.0) * h))), min(w - 1, max(0, int((uv[0] % 1.0) * w)))]


def colour_at(ob, pixels, face_index, point):
    """The colour of a face of `ob` at its corner nearest `point`."""
    face = ob.data.polygons[face_index]
    layer = ob.data.uv_layers[0].data
    li = min(face.loop_indices, key=lambda li: (ob.data.vertices[ob.data.loops[li].vertex_index].co - point).length)
    return colour(pixels, layer[li].uv)


dressed_image, dressed_pixels = image_of(dressed)

# 2. Mark the skin the piece covers, by its coverage; and how far the dressed surface stands
# off it there, for the piece's bulk.
groups = {g.index: g.name for g in body.vertex_groups}


def main_bone(v):
    return groups.get(max(v.groups, key=lambda g: g.weight).group, "") if v.groups else ""


def joint(name):
    return arm.matrix_world @ arm.data.bones[name].head_local


hips = joint("DEF-hips").z
knee = (joint("DEF-shin.L").z + joint("DEF-shin.R").z) / 2
hem_z = hips - (hips - knee) * COVER["hem"] if COVER["hem"] is not None else -math.inf
mark = np.zeros(len(skin_pts))
stand = np.zeros(len(skin_pts))
# Skin a limb the piece does not cover moves even this much is left out of it, so an armhole
# or a hem stays clear of the arm or thigh as it moves.
LIMB_SHARE = 0.15
LIMBS = ("DEF-upper_arm", "DEF-forearm", "DEF-thigh", "DEF-shin", "DEF-hand", "DEF-foot", "DEF-head", "DEF-neck")
# A piece with a hem goes down as far as its hem, whatever moves the skin there.
uncovered_limbs = tuple(b for b in LIMBS if not b.startswith(COVER["bones"])
                        and not (COVER["hem"] is not None and b == "DEF-thigh"))
head_top = max(p.z for p in skin_pts)
neck = joint("DEF-neck").z if "DEF-neck" in arm.data.bones else head_top - 0.3
crown_z = head_top - (head_top - neck) * COVER["crown"] if COVER.get("crown") else -math.inf
for i, v in enumerate(body.data.vertices):
    bone = main_bone(v)
    if skin_pts[i].z < crown_z:
        continue
    covered = bone.startswith(COVER["bones"]) or (COVER["hem"] is not None and bone.startswith("DEF-thigh") and skin_pts[i].z > hem_z)
    if not covered or (bone.startswith("DEF-hips") and skin_pts[i].z < hem_z):
        continue
    if any(groups.get(g.group, "").startswith(uncovered_limbs) and g.weight > LIMB_SHARE for g in v.groups):
        continue
    mark[i] = 1.0
    p, n = skin_pts[i], skin_nrm[i]
    hit = dressed_tree.ray_cast(p - n * LOOK_M, n, 2 * LOOK_M)
    if hit[0] is not None:
        stand[i] = max(0.0, (hit[0] - p).dot(n))
print("SHELL", item, coverage, f"marks {int(mark.sum())} of {len(mark)} vertices")
edges = np.array([tuple(e.vertices) for e in body.data.edges])
for _ in range(SMOOTH_ROUNDS):
    total, count = np.zeros(len(mark)), np.zeros(len(mark))
    np.add.at(total, edges[:, 0], mark[edges[:, 1]])
    np.add.at(total, edges[:, 1], mark[edges[:, 0]])
    np.add.at(count, edges[:, 0], 1)
    np.add.at(count, edges[:, 1], 1)
    mark = ((total + mark) / (count + 1) > 0.5).astype(float)

print("SHELL", item, f"marks {int(mark.sum())} vertices after smoothing")
# Islands of marked and unmarked faces: small ones are noise, flipped to their surroundings.
bm = bmesh.new()
bm.from_mesh(body.data)
bm.faces.ensure_lookup_table()
marked = np.array([all(mark[v.index] > 0.5 for v in f.verts) for f in bm.faces])


def islands(selected):
    seen, out = set(), []
    for f in bm.faces:
        if f.index in seen or marked[f.index] != selected:
            continue
        isl, stack = [], [f]
        seen.add(f.index)
        while stack:
            g = stack.pop()
            isl.append(g.index)
            for e in g.edges:
                for h in e.link_faces:
                    if h.index not in seen and marked[h.index] == selected:
                        seen.add(h.index)
                        stack.append(h)
        out.append(isl)
    return out


for selected in (False, True):
    found = islands(selected)
    size = max(1, int(marked.sum()))
    for isl in found:
        if len(isl) < SPECK * size:
            marked[isl] = not selected
# A piece is one or two whole parts (a coat; a pair of boots): keep the marked islands at
# least WHOLE of the largest, dropping strips where the dressed model's skin is only a
# different shade from ours.
found = islands(True)
largest = max((len(isl) for isl in found), default=0)
for isl in found:
    if len(isl) < WHOLE * largest:
        marked[isl] = False
print("SHELL", item, f"marks {int(marked.sum())} of {len(marked)} faces of the body")

# 3. The piece: those faces of the body, pushed out.
bmesh.ops.delete(bm, geom=[f for f in bm.faces if not marked[f.index]], context="FACES")
bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
# Onto a copy of the body's own mesh, which carries the names of its vertex groups.
piece = body.copy()
piece.data = body.data.copy()
bm.to_mesh(piece.data)
bm.free()
piece.name = item
bpy.context.scene.collection.objects.link(piece)
piece.modifiers.clear()
# Which body vertex each piece vertex came from, by place, to take its weights and stand-off.
piece_pts = [mw @ v.co for v in piece.data.vertices]
from mathutils.kdtree import KDTree

kd = KDTree(len(skin_pts))
for i, p in enumerate(skin_pts):
    kd.insert(p, i)
kd.balance()
origin = [kd.find(p)[1] for p in piece_pts]
inv = mw.inverted()
waist = joint("DEF-spine.001").z
for v, i in zip(piece.data.vertices, origin):
    most = CROSSED_M if skin_pts[i].z > waist else BULK_M
    out = min(max(OFFSET_M, stand[i] + 0.004), most)
    v.co = inv @ (skin_pts[i] + skin_nrm[i] * out)
piece.data.update()
# 4. Weights: the vertex groups came with the copy of the body, so each vertex keeps its own.

# 5. Its colour, baked from the dressed model onto a new unwrap.
for layer in list(piece.data.uv_layers):
    piece.data.uv_layers.remove(layer)
piece.data.uv_layers.new(name="UVMap")
bpy.ops.object.select_all(action="DESELECT")
piece.select_set(True)
bpy.context.view_layer.objects.active = piece
bpy.ops.object.mode_set(mode="EDIT")
bpy.ops.mesh.select_all(action="SELECT")
with bpy.context.temp_override(active_object=piece, object=piece, edit_object=piece):
    bpy.ops.uv.smart_project(island_margin=0.01)
bpy.ops.object.mode_set(mode="OBJECT")
baked = bpy.data.images.new(f"{item}_colour", TEXTURE, TEXTURE)
mat = bpy.data.materials.new(role)
mat.use_nodes = True
nodes = mat.node_tree.nodes
tex = nodes.new("ShaderNodeTexImage")
tex.image = baked
mat.node_tree.links.new(tex.outputs["Color"], nodes["Principled BSDF"].inputs["Base Color"])
nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.8
nodes.active = tex
piece.data.materials.clear()
piece.data.materials.append(mat)
scene = bpy.context.scene
scene.render.engine = "CYCLES"
scene.cycles.device = "CPU"
scene.cycles.samples = 1
scene.render.bake.use_pass_direct = False
scene.render.bake.use_pass_indirect = False
scene.render.bake.use_selected_to_active = True
scene.render.bake.cage_extrusion = 2 * LOOK_M
scene.render.bake.max_ray_distance = 4 * LOOK_M
scene.render.bake.margin = 16
bpy.ops.object.select_all(action="DESELECT")
dressed.select_set(True)
piece.select_set(True)
bpy.context.view_layer.objects.active = piece
bpy.ops.object.bake(type="DIFFUSE")
baked.pack()

# The patches it hides.
covered_by = {}
total = {}
under = set(origin)
for i in range(len(skin_pts)):
    p = patch_of.get(i)
    if p is None:
        continue
    total[p] = total.get(p, 0) + 1
    covered_by[p] = covered_by.get(p, 0) + (i in under)
covers = sorted(p for p in total if covered_by[p] / total[p] >= COVERED)
piece["covers"] = covers
print("SHELL", item, f"{len(piece.data.vertices)} vertices, hides {len(covers)} of {len(total)} patches")

# Parented to the skeleton afresh, keeping where it is, and bent by it, so the exporter writes
# it with a skin (a copy keeps the importer's parenting, which it does not).
placed = piece.matrix_world.copy()
piece.parent = None
piece.matrix_world = placed
piece.parent = arm
piece.matrix_parent_inverse = arm.matrix_world.inverted()
piece.modifiers.new("Armature", "ARMATURE").object = arm
bpy.data.objects.remove(dressed)
bpy.data.objects.remove(body)
print("SHELL", item, "parent", piece.parent and piece.parent.name, "armature", piece.find_armature() and piece.find_armature().name,
      "modifiers", [(m.type, m.object and m.object.name) for m in piece.modifiers], "groups", len(piece.vertex_groups),
      "deform bones", sum(1 for b in arm.data.bones if b.use_deform))
os.makedirs(os.path.join(OUT, body_name), exist_ok=True)
path = os.path.join(OUT, body_name, f"{item}.glb")
bpy.ops.object.select_all(action="DESELECT")
piece.select_set(True)
arm.select_set(True)
bpy.context.view_layer.objects.active = arm
bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_animations=False,
                          export_influence_nb=4, export_extras=True)
print("EXPORTED", path)
