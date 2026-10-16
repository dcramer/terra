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

    tools/blender/run.sh tools/characters/shell_garment.py tools/characters/out/base-male-game.glb art/gear/dressed-male-outfit/model.glb leather_coat Leather chest

        [--shape=MORPH] [--over=ITEM,...] [--under=ITEM,...]

The slot (D64) says where on the body the piece can be: a chest piece never reaches the thighs,
whatever colour Tripo gave the dressed model's shorts.

--shape=MORPH: the dressed model is of a morph of the base (the female, drawn from her own
sheet): the piece is built on the base's skin moved by that morph (tools/characters/out/
MORPH.npz), lined up with and baked from the model there, then taken back off, so the game's file
is the base's like every other and carry_gear.py puts the morph back.
--over=ITEM: worn over pieces made before (a bodice over a shift, boots over trousers): it
stands at least OVER_M outside them wherever it covers them, so the two layers never cross.
--under=ITEM: where those pieces, made from the same dressed model, cover this one (a shirt
under a smith's apron), it keeps to the skin and its colour is taken from the other side of the
body, front for back, so it does not wear the other piece painted on it.
--plain: it is one plain cloth (a linen shirt): whatever the dressed model paints on it much
lighter or darker than most of it (another piece's strap or tie) is filled from round it.

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
import skin_piece

OUT = look.GEAR_MADE
LOOK_M = 0.06          # how far out and in from the skin to look for the dressed surface
STANDS_OFF_M = 0.008   # dressed surface further out than this is the piece, whatever its colour
SMOOTH_ROUNDS = 4      # rounds of smoothing the mark
SPECK = 0.03           # marked or unmarked islands smaller than this share of the mark are noise
WHOLE = 0.3            # marked islands smaller than this share of the largest are not the piece
OFFSET_M = 0.008       # how far out from the skin the piece sits
BULK_M = 0.03          # the most it follows the dressed surface standing off
DRAPE_ROUNDS = 12      # rounds of draping the piece over the hollows of the body under it
DRAPE = 0.5            # how far each round moves a vertex toward its neighbours' middle
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
OVER_M = 0.004         # a piece worn over another stands at least this far outside it
UNDER_M = 0.12         # another piece this far out from the skin covers this one
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
    # A coif or hood: the crown down to the hairline at the front, over the ears at the sides
    # and to the nape at the back (`nape`, the share at the back), the face left open.
    "head_coif": {"bones": ("Head",), "hem": None, "crown": 0.3, "nape": 0.95},
    # A shirt tucked in, its hem at the hips joint.
    "chest_long_sleeves_tucked": {"bones": ("spine_", "clavicle_", "pelvis", "upperarm_", "lowerarm_"), "hem": 0.0},
    # A bodice: the body from the hips to a scoop neckline, with straps over the shoulders and
    # armholes cut round them (`neckline`: how far below the neck joint the scoop falls at the
    # front and at the back; `straps`: how far from the middle a strap starts and ends, in
    # metres on the base).
    "bodice": {"bones": ("spine_", "clavicle_", "pelvis"), "hem": 0.0, "neckline": (0.17, 0.11), "straps": (0.075, 0.125)},
    # Shoes to the ankle, boots up their shaft: `shaft`, the share of the way from the ankle to
    # the knee they reach; `crease`, the two bones of the joint whose crease the piece lies
    # close on (below).
    "feet_ankle": {"bones": ("foot_", "ball_", "calf_"), "hem": None, "shaft": 0.3, "crease": ("foot_", "calf_")},
}

options, given = skin_piece.flags(look.args())
body_path, dressed_path, item, role, coverage = (given + ["Cloth", "chest_sleeveless"])[:5]
COVER = COVERAGE[coverage]
body_name = look.body_name(body_path)
SHAPE = options.get("shape") or None
OVER = [x for x in options.get("over", "").split(",") if x]
UNDER = [x for x in options.get("under", "").split(",") if x]

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
# The base's skin, where what the piece covers is said; and, for a morph's dressed model, the
# skin moved by that morph, where the piece is built and baked.
base_pts = [mw @ v.co for v in body.data.vertices]
base_np = np.array([tuple(p) for p in base_pts])
delta = skin_piece.morph_delta(SHAPE, base_np)
if SHAPE:
    inv_mw = mw.inverted()
    for v in body.data.vertices:
        v.co = inv_mw @ (base_pts[v.index] + Vector(delta[v.index]))
    body.data.update()
skin_pts = [mw @ v.co for v in body.data.vertices]
skin_nrm = [(mw.to_3x3() @ v.normal).normalized() for v in body.data.vertices]
skin = BVHTree.FromPolygons(skin_pts, [tuple(p.vertices) for p in body.data.polygons])
top = max(p.z for p in skin_pts)

# 1. The dressed model, turned to face -Y as ours do (Tripo's face +X), scaled and lined up.
dressed = skin_piece.line_up(look.path(dressed_path), skin, top, MATCH_M)
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
# What it is worn over and under, as they stand on this skin.
over_trees = [skin_piece.made(body_name, o, base_np, delta)[0] for o in OVER]
under_trees = [skin_piece.made(body_name, u, base_np, delta)[0] for u in UNDER]

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
head_top = max(p.z for p in base_pts)
neck = joint("neck_01").z if "neck_01" in arm.data.bones else head_top - 0.3
head_y = joint("Head").y
middle_x = joint("pelvis").x


def crown_z(p):
    """How low a piece on the head reaches at the base's skin point `p`: its crown share at the
    front, its nape share at the back (+Y), blended across the head."""
    if not COVER.get("crown"):
        return -math.inf
    back = COVER.get("nape", COVER["crown"])
    t = min(1.0, max(0.0, (p.y - head_y + 0.09) / 0.18))
    share = COVER["crown"] + (back - COVER["crown"]) * t * t * (3 - 2 * t)
    return head_top - (head_top - neck) * share


ankle = (joint("foot_l").z + joint("foot_r").z) / 2
shaft_z = ankle + (knee - ankle) * COVER["shaft"] if COVER.get("shaft") else math.inf
shoulder_z = (joint("clavicle_l").z + joint("clavicle_r").z) / 2


def cut_away(p):
    """Whether a bodice's neckline or armholes leave the base's skin point `p` bare."""
    if not COVER.get("neckline"):
        return False
    front, back = COVER["neckline"]
    inner, outer = COVER["straps"]
    x = abs(p.x - middle_x)
    if x > outer:
        return p.z > shoulder_z - 0.09
    if x < inner:
        return p.z > neck - (front if p.y < joint("spine_03").y else back)
    return False


least = np.zeros(len(skin_pts))   # how far out it must stand, over what it is worn over
hidden = np.zeros(len(skin_pts))  # 1 where a piece made from the same model covers it
for i, v in enumerate(body.data.vertices):
    bone = main_bone(v)
    b = base_pts[i]
    if b.z < crown_z(b) or cut_away(b):
        continue
    covered = bone.startswith(COVER["bones"]) or (COVER["hem"] is not None and bone.startswith("thigh_") and b.z > hem_z)
    if not covered or (bone.startswith("pelvis") and b.z < hem_z):
        continue
    if bone.startswith("calf_") and b.z > shaft_z:
        continue
    if any(groups.get(g.group, "").startswith(uncovered_limbs) and g.weight > LIMB_SHARE for g in v.groups):
        continue
    mark[i] = 1.0
    p, n = skin_pts[i], skin_nrm[i]
    # Under another piece it keeps to the skin: the dressed surface there is the other piece's.
    hidden[i] = float(any(tree.ray_cast(p + n * 0.001, n, UNDER_M)[0] is not None for tree in under_trees))
    hit = dressed_tree.ray_cast(p - n * LOOK_M, n, 2 * LOOK_M)
    if hit[0] is not None and not hidden[i]:
        stand[i] = max(0.0, (hit[0] - p).dot(n))
    for tree in over_trees:
        found = tree.ray_cast(p - n * 0.002, n, LOOK_M + 0.002)
        if found[0] is not None:
            least[i] = max(least[i], (found[0] - p).dot(n) + OVER_M)
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
least_l = bm.verts.layers.float.new("least")
hidden_l = bm.verts.layers.float.new("hidden")
delta_l = [bm.verts.layers.float.new(f"delta_{k}") for k in "xyz"]
field = np.array([sum(marked[f.index] for f in v.link_faces) / max(1, len(v.link_faces)) for v in bm.verts])
for _ in range(CUT_SMOOTH_ROUNDS):
    total, count = np.zeros(len(field)), np.zeros(len(field))
    np.add.at(total, edges[:, 0], field[edges[:, 1]])
    np.add.at(total, edges[:, 1], field[edges[:, 0]])
    np.add.at(count, edges[:, 0], 1)
    np.add.at(count, edges[:, 1], 1)
    field = (total + field) / (count + 1)
for v in bm.verts:
    v[stand_l] = stand[v.index]
    for k in range(3):
        v[normal_l[k]] = skin_nrm[v.index][k]
    v[body_l] = v.index
    v[field_l] = field[v.index]
    v[least_l] = least[v.index]
    v[hidden_l] = hidden[v.index]
    for k in range(3):
        v[delta_l[k]] = delta[v.index][k]
for v in skin_piece.cut(bm, field_l, 0.5, CUT_CLEAR):
    v[body_l] = -1
# The edge evened out along itself, each round put back on the skin.
skin_piece.relax(bm, mw, skin, EDGE_RELAX_ROUNDS)
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
piece_least = layer("least")
piece_delta = np.stack([layer(f"delta_{k}") for k in "xyz"], axis=1)
under = {int(i) for i in layer("body_vertex") if i >= 0}
for name in ("stand", "normal_x", "normal_y", "normal_z", "body_vertex", "field", "least", "delta_x", "delta_y", "delta_z"):
    attrs.remove(attrs[name])
if not UNDER:
    attrs.remove(attrs["hidden"])
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


def in_crease(v):
    """How far into the crease of COVER's `crease` joint a vertex lies, 0 to 1: 1 where its
    weight is split evenly between the joint's two bones, where the skin folds as it bends.
    There the piece lies close on the skin, as a boot's leather creases close on the instep:
    standing off, it is what the skin folding toward it meets, and comes through (the shin of
    the heavy body, in a crouch, through a boot standing 1.5 cm off the front of its ankle)."""
    if not COVER.get("crease"):
        return 0.0
    a, b = COVER["crease"]
    wa = sum(g.weight for g in v.groups if groups.get(g.group, "").startswith(a))
    wb = sum(g.weight for g in v.groups if groups.get(g.group, "").startswith(b))
    return 2 * min(wa, wb) / (wa + wb) if wa + wb > 0 else 0.0


crease = [in_crease(v) for v in piece.data.vertices]
for v in piece.data.vertices:
    i = v.index
    # Over another piece: never nearer the skin than it, its edge lying on it, not the skin.
    lowest = max(OFFSET_M, float(piece_least[i]))
    above = on_skin[i].z - piece_delta[i][2] > waist
    most = max(CROSSED_M if above else BULK_M, lowest)
    out = min(max(lowest, piece_stand[i] + 0.004), most)
    out = lowest + (out - lowest) * (1.0 - crease[i])
    # Above the waist only: an armhole or a neckline lies on the body, a hem hangs free.
    t = min(1.0, from_edge.get(i, math.inf) / TAPER_M) if above else 1.0
    out = lowest + (out - lowest) * t * t * (3 - 2 * t)
    v.co = inv @ (on_skin[i] + normals[i] * out)
piece.data.update()

# 3b. Drape: cloth bridges the hollows between muscles rather than following the skin into
# them, as a shrink-wrapped shell does (the adversarial review, 2026-10-15: "the jacket is a
# muscle suit"). The surface is smoothed, each vertex moved toward its neighbours' middle, but
# never nearer the skin than it was, so hollows fill and rises stay; the edges, where an
# armhole, neckline or hem lies, are held.
placed = [mw @ v.co for v in piece.data.vertices]
least = [(placed[i] - on_skin[i]).dot(normals[i]) for i in range(len(placed))]
for _ in range(DRAPE_ROUNDS):
    moved = []
    for i, p in enumerate(placed):
        if i in border or not near[i]:
            moved.append(p)
            continue
        middle = sum((placed[j] for j, _ in near[i]), Vector()) / len(near[i])
        # Held at the edges, and in a crease, which it would fill again.
        held = min(1.0, from_edge.get(i, math.inf) / TAPER_M) * (1.0 - crease[i])
        q = p.lerp(middle, DRAPE * held)
        off = (q - on_skin[i]).dot(normals[i])
        if off < least[i]:
            q += normals[i] * (least[i] - off)
        moved.append(q)
    placed = moved
for v in piece.data.vertices:
    v.co = inv @ placed[v.index]
piece.data.update()
print(f"SHELL {item} draped: {sum((placed[i] - on_skin[i]).dot(normals[i]) for i in range(len(placed))) / len(placed) * 100:.2f} cm off the skin on average")
# 4. Weights: the vertex groups came with the copy of the body, so each vertex keeps its own.

# 5. Its colour, surface and roughness, baked from the dressed model onto a new unwrap.
skin_piece.bake(piece, dressed, item, role, TEXTURE, LOOK_M, under="hidden" if UNDER else None, plain="plain" in options)
if UNDER:
    attrs.remove(attrs["hidden"])

# Built on a morph's skin, taken back onto the base's, as every look is kept.
if SHAPE:
    for v in piece.data.vertices:
        v.co = inv @ (placed[v.index] - Vector(piece_delta[v.index]))
    piece.data.update()

# A rim turned in from the edge to just off the skin.
skin_piece.rim(piece, lambda i: inv @ (on_skin[i] - Vector(piece_delta[i]) + normals[i] * RIM_M))

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
