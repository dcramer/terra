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

    tools/blender/run.sh tools/characters/shell_garment.py tools/characters/out/base-male-game.glb art/gear/dressed-male-coat/model.glb leather_coat Leather chest

The slot (D64) says where on the body the piece can be: a chest piece never reaches the thighs,
whatever colour Tripo gave the dressed model's shorts.

Writes tools/characters/out/gear/<body>/<item>.glb (look.GEAR_MADE) and prints SHELL lines.
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bmesh
import bpy
import numpy as np
from mathutils import Matrix, Vector
from mathutils.bvhtree import BVHTree

import look
import patches

OUT = look.GEAR_MADE
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
TEXTURE = 2048         # the bake's size; the game's files take gear at its budget (D188)
CUT_SMOOTH_ROUNDS = 3  # rounds of smoothing the field the piece's edge is cut along
CUT_CLEAR = 0.1        # the field kept this far from the cut at the body's vertices, so the cut
                       # never passes a hair from one and leaves a sliver of a triangle
EDGE_RELAX_ROUNDS = 4  # rounds of evening out the edge along itself, kept on the skin
TAPER_M = 0.02         # over this far in from its edge above the waist, the piece comes down
                       # to OFFSET_M, so an armhole or neckline lies on the body as a real one does
RIM_M = 0.0015         # the rim turned in from the edge stops this far off the skin
COVERED = 1.0          # a patch wholly under the piece is hidden; part-covered ones stay, or a band of skin vanishes below a hem or rim
# What each kind of piece covers: the bones whose skin it covers, and the heights of its
# hems, as shares of the way from one joint to the next (0 at the first, 1 at the second):
# a chest piece's hem from the hips joint toward the knee, a sleeve from the shoulder toward
# the elbow or wrist.
COVERAGE = {
    "chest_sleeveless": {"bones": ("spine_", "clavicle_", "pelvis"), "hem": 0.25},
    "chest_short_sleeves": {"bones": ("spine_", "clavicle_", "pelvis", "upperarm_"), "hem": 0.25},
    "chest_long_sleeves": {"bones": ("spine_", "clavicle_", "pelvis", "upperarm_", "lowerarm_"), "hem": 0.25},
    "legs_short": {"bones": ("pelvis", "thigh_"), "hem": None},
    "legs_long": {"bones": ("pelvis", "thigh_", "calf_"), "hem": None},
    "gloves": {"bones": ("hand_", "index_", "middle_", "ring_", "pinky_", "thumb_"), "hem": None},
    "feet": {"bones": ("foot_", "ball_"), "hem": None},
    # A cap: the crown, down this share of the way from the top of the head to the neck.
    "head_cap": {"bones": ("Head",), "hem": None, "crown": 0.45},
}

body_path, dressed_path, item, role, coverage = (look.args() + ["Cloth", "chest_sleeveless"])[:5]
COVER = COVERAGE[coverage]
body_name = look.body_name(body_path)

look.reset()
bpy.ops.import_scene.gltf(filepath=look.path(body_path))
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
bpy.ops.import_scene.gltf(filepath=look.path(dressed_path))
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


hips = joint("pelvis").z
knee = (joint("calf_l").z + joint("calf_r").z) / 2
hem_z = hips - (hips - knee) * COVER["hem"] if COVER["hem"] is not None else -math.inf
mark = np.zeros(len(skin_pts))
stand = np.zeros(len(skin_pts))
# Skin a limb the piece does not cover moves even this much is left out of it, so an armhole
# or a hem stays clear of the arm or thigh as it moves.
LIMB_SHARE = 0.15
LIMBS = ("upperarm_", "lowerarm_", "thigh_", "calf_", "hand_", "foot_", "Head", "neck_01")
# A piece with a hem goes down as far as its hem, whatever moves the skin there.
uncovered_limbs = tuple(b for b in LIMBS if not b.startswith(COVER["bones"])
                        and not (COVER["hem"] is not None and b == "thigh_"))
head_top = max(p.z for p in skin_pts)
neck = joint("neck_01").z if "neck_01" in arm.data.bones else head_top - 0.3
crown_z = head_top - (head_top - neck) * COVER["crown"] if COVER.get("crown") else -math.inf
for i, v in enumerate(body.data.vertices):
    bone = main_bone(v)
    if skin_pts[i].z < crown_z:
        continue
    covered = bone.startswith(COVER["bones"]) or (COVER["hem"] is not None and bone.startswith("thigh_") and skin_pts[i].z > hem_z)
    if not covered or (bone.startswith("pelvis") and skin_pts[i].z < hem_z):
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

# 3. The piece: those faces of the body, cut out along a smooth line and pushed out. The faces
# kept have a stepped border along the body's triangles; instead, a smooth field over the
# vertices (the share of each one's faces kept, averaged with its neighbours') is cut at one
# half, through the triangles, as sculpting tools extract a garment from a body. What each
# vertex was on the body (its stand-off, its normal, which vertex) goes with it in layers, and
# the cut's new vertices take theirs, and their weights, between the two ends of their edge.
bm.verts.ensure_lookup_table()
stand_l = bm.verts.layers.float.new("stand")
normal_l = [bm.verts.layers.float.new(f"normal_{k}") for k in "xyz"]
body_l = bm.verts.layers.int.new("body_vertex")
field_l = bm.verts.layers.float.new("field")
field = np.array([sum(marked[f.index] for f in v.link_faces) / max(1, len(v.link_faces)) for v in bm.verts])
for _ in range(CUT_SMOOTH_ROUNDS):
    total, count = np.zeros(len(field)), np.zeros(len(field))
    np.add.at(total, edges[:, 0], field[edges[:, 1]])
    np.add.at(total, edges[:, 1], field[edges[:, 0]])
    np.add.at(count, edges[:, 0], 1)
    np.add.at(count, edges[:, 1], 1)
    field = (total + field) / (count + 1)
field = np.where(np.abs(field - 0.5) < CUT_CLEAR, np.where(field >= 0.5, 0.5 + CUT_CLEAR, 0.5 - CUT_CLEAR), field)
for v in bm.verts:
    v[stand_l] = stand[v.index]
    for k in range(3):
        v[normal_l[k]] = skin_nrm[v.index][k]
    v[body_l] = v.index
    v[field_l] = field[v.index]
made = set()
for e in [e for e in bm.edges if (e.verts[0][field_l] - 0.5) * (e.verts[1][field_l] - 0.5) < 0]:
    a, b = e.verts
    _, v = bmesh.utils.edge_split(e, a, (0.5 - a[field_l]) / (b[field_l] - a[field_l]))
    v[field_l] = 0.5
    v[body_l] = -1
    made.add(v)
for f in list(bm.faces):
    on = [v for v in f.verts if v in made]
    if len(on) == 2:
        bmesh.ops.connect_verts(bm, verts=on)
bmesh.ops.delete(bm, geom=[f for f in bm.faces if sum(v[field_l] for v in f.verts) / len(f.verts) < 0.5], context="FACES")
bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
bmesh.ops.dissolve_degenerate(bm, dist=1e-4, edges=bm.edges[:])
# The edge evened out along itself, each round put back on the skin.
for _ in range(EDGE_RELAX_ROUNDS):
    rim = [v for v in bm.verts if v.is_boundary]
    moved = {}
    for v in rim:
        along = [e.other_vert(v) for e in v.link_edges if e.is_boundary]
        if len(along) == 2:
            p = mw @ (v.co * 0.5 + (along[0].co + along[1].co) * 0.25)
            hit = skin.find_nearest(p)
            moved[v] = mw.inverted() @ (hit[0] if hit[0] is not None else p)
    for v, co in moved.items():
        v.co = co
# Onto a copy of the body's own mesh, which carries the names of its vertex groups.
piece = body.copy()
piece.data = body.data.copy()
bm.to_mesh(piece.data)
bm.free()
piece.name = item
bpy.context.scene.collection.objects.link(piece)
piece.modifiers.clear()
# What each vertex was on the body, from the layers the cut carried.
attrs = piece.data.attributes


def layer(name):
    values = np.zeros(len(piece.data.vertices), dtype=np.int32 if name == "body_vertex" else np.float32)
    attrs[name].data.foreach_get("value", values)
    return values


piece_stand = layer("stand")
piece_normal = np.stack([layer(f"normal_{k}") for k in "xyz"], axis=1)
under = {int(i) for i in layer("body_vertex") if i >= 0}
for name in ("stand", "normal_x", "normal_y", "normal_z", "body_vertex", "field"):
    attrs.remove(attrs[name])
on_skin = [mw @ v.co for v in piece.data.vertices]
normals = [Vector(n).normalized() for n in piece_normal]
# How far each vertex is from the piece's edge, along the piece.
import heapq

near = {i: [] for i in range(len(piece.data.vertices))}
for e in piece.data.edges:
    a, b = e.vertices
    d = (on_skin[a] - on_skin[b]).length
    near[a].append((b, d))
    near[b].append((a, d))
counts = {}
for poly in piece.data.polygons:
    for key in poly.edge_keys:
        counts[key] = counts.get(key, 0) + 1
border = {i for key, n in counts.items() if n == 1 for i in key}
from_edge = {i: 0.0 for i in border}
queue = [(0.0, i) for i in border]
while queue:
    d, i = heapq.heappop(queue)
    if d > from_edge.get(i, math.inf) or d > TAPER_M:
        continue
    for j, step in near[i]:
        if d + step < from_edge.get(j, math.inf):
            from_edge[j] = d + step
            heapq.heappush(queue, (d + step, j))
inv = mw.inverted()
waist = joint("spine_01").z
for v in piece.data.vertices:
    i = v.index
    most = CROSSED_M if on_skin[i].z > waist else BULK_M
    out = min(max(OFFSET_M, piece_stand[i] + 0.004), most)
    # Above the waist only: an armhole or a neckline lies on the body, a hem hangs free.
    t = min(1.0, from_edge.get(i, math.inf) / TAPER_M) if on_skin[i].z > waist else 1.0
    out = OFFSET_M + (out - OFFSET_M) * t * t * (3 - 2 * t)
    v.co = inv @ (on_skin[i] + normals[i] * out)
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
# Its surface as well as its colour: the dressed model's normals (its normal map included)
# and roughness, baked the same way, so leather reads as leather and not as a printed sheet.
bsdf = nodes["Principled BSDF"]
for kind in ("NORMAL", "ROUGHNESS"):
    image = bpy.data.images.new(f"{item}_{kind.lower()}", TEXTURE, TEXTURE, is_data=True)
    image.colorspace_settings.name = "Non-Color"
    node = nodes.new("ShaderNodeTexImage")
    node.image = image
    nodes.active = node
    bpy.ops.object.bake(type=kind, normal_space="TANGENT")
    image.pack()
    if kind == "NORMAL":
        mapping = nodes.new("ShaderNodeNormalMap")
        mat.node_tree.links.new(node.outputs["Color"], mapping.inputs["Color"])
        mat.node_tree.links.new(mapping.outputs["Normal"], bsdf.inputs["Normal"])
    else:
        mat.node_tree.links.new(node.outputs["Color"], bsdf.inputs["Roughness"])
nodes.active = tex

# A rim turned in from the edge to just off the skin, so the edge reads as the material's
# thickness and nothing shows into the gap between piece and skin. Its faces take the colour
# along the edge (their texture coordinates are the edge's), and the edge is kept sharp.
bm = bmesh.new()
bm.from_mesh(piece.data)
bm.verts.ensure_lookup_table()
uv = bm.loops.layers.uv.active
edge = [e for e in bm.edges if e.is_boundary]
# Keyed by the elements themselves: the extrusion leaves new elements' indices unset.
edge_uv = {}
for e in edge:
    face = e.link_faces[0]
    edge_uv[e] = {l.vert: l[uv].uv.copy() for l in face.loops if l.vert in e.verts}
    e.smooth = False
old = set(bm.verts)
index = {v: v.index for v in bm.verts}
grown = bmesh.ops.extrude_edge_only(bm, edges=edge)
source = {}
for v in [g for g in grown["geom"] if isinstance(g, bmesh.types.BMVert)]:
    source[v] = next(e.other_vert(v) for e in v.link_edges if e.other_vert(v) in old)
    i = index[source[v]]
    v.co = inv @ (on_skin[i] + normals[i] * RIM_M)
for f in [g for g in grown["geom"] if isinstance(g, bmesh.types.BMFace)]:
    e = next(e for e in f.edges if e in edge_uv)
    for l in f.loops:
        l[uv].uv = edge_uv[e][l.vert if l.vert in old else source[l.vert]]
bm.normal_update()
bm.to_mesh(piece.data)
bm.free()
piece.data.update()

# The patches it hides.
covered_by = {}
total = {}
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
