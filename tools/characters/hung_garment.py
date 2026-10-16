"""Makes a piece that hangs from the body rather than lying on it, a long skirt or an apron, for
the base body from a model of a body wearing it (D128, docs/characters.md). shell_garment.py
only follows the skin, so a skirt made by it would be a pair of leggings.

    tools/blender/run.sh tools/characters/hung_garment.py tools/characters/out/base-male-game.glb art/gear/dressed-female-kirtle/model.glb skirt Wool skirt --shape=female --over=shift,bodice --under=apron

The last word is its kind, one of KINDS: a bell all round the body (`around`, a skirt) or a
panel at its front (`front`, an apron), from what height to what height, as shares of the way
between two of the skeleton's joints. The piece is made as games without cloth simulation
make a long skirt (the usual hand-skinning practice: a pelvis core, a smooth blend across both
legs, and room left in the mesh, as rigging forums for Unity and Reallusion's characters give
it), with no new bones:

  1. Rings at heights from its top to its hem. Each ring is the convex outline of the body (its
     trunk and legs, never the arms) at that height, and of anything it is worn over, pushed out
     by a clearance that grows toward the hem, and out to the dressed model's own surface where
     that stands further off, up to BULK_M more, so it keeps the dressed model's bell. Below the
     hips no ring is narrower than the one above it: cloth falls, it is not drawn in between the
     legs.
  2. Weights: above the hips, the skin nearest it, as a shirt has; below, the pelvis's share
     falling away toward the hem, the rest to the thighs, each side to its own thigh, blended
     across the middle front and back over SIDE_M, so the cloth between the legs goes half with
     each; from the knee down a share of the calves (up to CALF), so the hem follows the shins
     out of the way in a stride and falls with them when sitting. At most four bones a vertex.
  3. Its colour, surface and roughness baked from the dressed model, as a shell's are; where
     another piece from the same model covers it (--under, an apron over a skirt), from the
     other side; with --plain, another piece's paint on it filled from round it.
  4. A rim turned in at its top to the skin, so nothing shows into it, and a thickness at its
     hem and sides.

Its object is marked `hung`, so carry_gear.py moves it with a morph as the skin round it moves,
smoothed over it, rather than vertex by nearest vertex. Writes tools/characters/out/gear/
<body>/<item>.glb (look.GEAR_MADE) and prints HUNG lines.
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bmesh
import bpy
import numpy as np
from mathutils import Vector
from mathutils.bvhtree import BVHTree

import look
import skin_piece

TEXTURE = 2048
LOOK_M = 0.08        # how far out and in from the piece its colour is looked for
BAND_M = 0.025       # skin this far above and below a ring's height shapes it
CLEAR_M = 0.012      # how far out from the body a ring stands at its top,
CLEAR_HEM_M = 0.04   # and at its hem
BULK_M = 0.12        # the most it follows the dressed model out past that
FLARE = 0.06         # below the hips each ring at least this much wider a metre lower
SIDE_M = 0.07        # the cloth's middle, front and back, blends between the legs over this
CALF = 0.45          # the most of a hem's weight on the calves
LEG_FULL = 0.45      # all of the weight on the legs by this share of the way from hips to knee
THICK_M = 0.004      # the cloth's thickness at its hem and sides
SMOOTH_ROUNDS = 6    # rounds of evening the rings out, round and down
TOP_RIM_M = 0.003    # the rim at its top stops this far off the skin
TRUNK = ("pelvis", "spine_", "thigh_", "calf_", "foot_")
# Heights: (joint, joint, share) is that share of the way from the first joint to the second.
KINDS = {
    # Ankle length, from the waist.
    "skirt": {"shape": "around", "top": ("spine_01", "spine_02", 0.0), "hem": ("calf", "foot", 0.8), "columns": 64, "rings": 30},
    # A waist apron to the shins, over the front of a skirt.
    "apron": {"shape": "front", "top": ("spine_01", "spine_02", -0.05), "hem": ("calf", "foot", 0.55), "columns": 22, "rings": 24,
              "width": ((0.0, 0.17), (1.0, 0.21))},
    # A smith's apron, bib and all, from the chest to the knee.
    "smith_apron": {"shape": "front", "top": ("spine_03", "neck_01", 0.3), "hem": ("calf", "foot", -0.15), "columns": 22, "rings": 40,
                    "width": ((0.0, 0.12), (0.3, 0.13), (0.42, 0.2), (1.0, 0.22))},
}

options, given = skin_piece.flags(look.args())
body_path, dressed_path, item, role, kind = given[:5]
KIND = KINDS[kind]
SHAPE = options.get("shape") or None
OVER = [x for x in options.get("over", "").split(",") if x]
UNDER = [x for x in options.get("under", "").split(",") if x]
body_name = look.body_name(body_path)

look.reset()
bpy.ops.import_scene.gltf(filepath=look.path(body_path))
arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
body = max((o for o in bpy.context.scene.objects if o.type == "MESH"), key=lambda o: len(o.data.vertices))
arm.animation_data_clear()
for pb in arm.pose.bones:
    pb.matrix_basis.identity()
bpy.context.view_layer.update()
bpy.ops.object.select_all(action="DESELECT")
body.select_set(True)
bpy.context.view_layer.objects.active = body
bpy.ops.object.mode_set(mode="EDIT")
bpy.ops.mesh.select_all(action="SELECT")
bpy.ops.mesh.remove_doubles(threshold=1e-5)
bpy.ops.object.mode_set(mode="OBJECT")
mw = body.matrix_world
groups = {g.index: g.name for g in body.vertex_groups}
base = np.array([tuple(mw @ v.co) for v in body.data.vertices])
delta = skin_piece.morph_delta(SHAPE, base)
skin = base + delta
weights = [{groups[g.group]: g.weight for g in v.groups if g.weight > 0} for v in body.data.vertices]
main = [max(w, key=w.get) if w else "" for w in weights]
trunk = np.array([m.startswith(TRUNK) for m in main])
skin_tree = BVHTree.FromPolygons([Vector(p) for p in skin], [tuple(p.vertices) for p in body.data.polygons])
base_kd = skin_piece.kd_of(base)


def joint(name):
    return np.array(tuple(arm.matrix_world @ arm.data.bones[name].head_local))


def side_joint(name):
    return (joint(f"{name}_l") + joint(f"{name}_r")) / 2 if f"{name}_l" in arm.data.bones else joint(name)


def height(spec):
    """A height on the base, `share` of the way from one joint to the next."""
    a, b, share = spec
    za, zb = side_joint(a)[2], side_joint(b)[2]
    return za + (zb - za) * share


def morphed_z(z):
    """The base's height `z` on the morph's skin: as far as the trunk's skin there moves up."""
    near = trunk & (np.abs(base[:, 2] - z) < BAND_M)
    return z + (float(np.median(delta[near, 2])) if near.any() else 0.0)


top_b, hem_b = height(KIND["top"]), height(KIND["hem"])
hips_b = joint("pelvis")[2]
knee_b = side_joint("calf")[2]
middle = joint("pelvis")[:2] + np.median(delta[trunk & (np.abs(base[:, 2] - hips_b) < 0.05), :2], axis=0)
rings = KIND["rings"]
heights_b = np.linspace(top_b, hem_b, rings)
heights = np.array([morphed_z(z) for z in heights_b])
print("HUNG", item, kind, f"from {top_b:.2f} to {hem_b:.2f} m on the base, {heights[0]:.2f} to {heights[-1]:.2f} on {SHAPE or 'it'}")

# The dressed model, lined up with this skin; and what it is worn over and under.
dressed = skin_piece.line_up(look.path(dressed_path), skin_tree, float(skin[:, 2].max()), 0.03)
dressed_tree = BVHTree.FromPolygons([v.co for v in dressed.data.vertices], [tuple(p.vertices) for p in dressed.data.polygons])
over_pts = np.concatenate([skin_piece.made(body_name, o, base, delta)[1] for o in OVER]) if OVER else np.zeros((0, 3))
under_trees = [skin_piece.made(body_name, u, base, delta)[0] for u in UNDER]


def hull(points):
    """The convex outline of 2D `points`, counter-clockwise (Andrew's monotone chain)."""
    pts = sorted(set(map(tuple, np.round(points, 4))))
    if len(pts) < 3:
        return np.array(pts)

    def cross(o, a, b):
        return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])

    lower, upper = [], []
    for p in pts:
        while len(lower) >= 2 and cross(lower[-2], lower[-1], p) <= 0:
            lower.pop()
        lower.append(p)
    for p in reversed(pts):
        while len(upper) >= 2 and cross(upper[-2], upper[-1], p) <= 0:
            upper.pop()
        upper.append(p)
    return np.array(lower[:-1] + upper[:-1])


def outline(z):
    """The body's trunk and legs, and what is worn over them, at height `z` (on this skin)."""
    near = trunk & (np.abs(skin[:, 2] - z) < BAND_M)
    pts = skin[near, :2]
    if len(over_pts):
        pts = np.concatenate([pts, over_pts[np.abs(over_pts[:, 2] - z) < BAND_M, :2]])
    return hull(pts)


def reach(poly, origin, direction):
    """How far from `origin` along `direction` the outline `poly` ends (0 if it misses)."""
    best = 0.0
    for a, b in zip(poly, np.roll(poly, -1, axis=0)):
        e = b - a
        m = np.array([[direction[0], -e[0]], [direction[1], -e[1]]])
        if abs(np.linalg.det(m)) < 1e-12:
            continue
        t, s = np.linalg.solve(m, a - origin)
        if t > 0 and 0 <= s <= 1:
            best = max(best, t)
    return best


def outermost(origin3, direction3, far=1.0):
    """How far out from `origin3` along `direction3` the dressed model's outermost surface is."""
    o, d = Vector(origin3), Vector(direction3)
    hit = dressed_tree.ray_cast(o + d * far, -d, far)
    return (hit[0] - o).dot(d) if hit[0] is not None else 0.0


def clearance(k):
    t = k / (rings - 1)
    return CLEAR_M + (CLEAR_HEM_M - CLEAR_M) * t


cols = KIND["columns"]
grid = np.zeros((rings, cols, 3))
if KIND["shape"] == "around":
    angles = np.linspace(0, 2 * math.pi, cols, endpoint=False)
    # Angle 0 is the front (-Y), turning toward the left (+X).
    dirs = np.stack([np.sin(angles), -np.cos(angles)], axis=1)
    radius = np.zeros((rings, cols))
    least = np.zeros((rings, cols))
    # The front's mirror at the back: a skirt is the same before and behind, and the less of
    # the two is the skirt's own, not an apron or its ties over it.
    behind = (cols // 2 - np.arange(cols)) % cols
    for k, z in enumerate(heights):
        poly = outline(z)
        least[k] = [reach(poly, middle, d) + clearance(k) if len(poly) >= 3 else 0.0 for d in dirs]
        dressed_r = np.array([outermost((middle[0], middle[1], z), (d[0], d[1], 0.0)) for d in dirs])
        dressed_r = np.minimum(dressed_r, dressed_r[behind])
        radius[k] = np.maximum(least[k], np.minimum(dressed_r, least[k] + BULK_M))
    # Evened round each ring and down the rings, never inside the body's room; below the hips
    # no ring narrower than the one above.
    for _ in range(SMOOTH_ROUNDS):
        radius = np.maximum(least, (np.roll(radius, 1, axis=1) + 2 * radius + np.roll(radius, -1, axis=1)) / 4)
        radius[1:-1] = np.maximum(least[1:-1], (radius[:-2] + 2 * radius[1:-1] + radius[2:]) / 4)
        for k in range(1, rings):
            if heights_b[k] < hips_b:
                radius[k] = np.maximum(radius[k], radius[k - 1] + FLARE * (heights[k - 1] - heights[k]))
    for k, z in enumerate(heights):
        print("DEBUGR", k, round(float(z), 3), [round(float(radius[k, j]), 3) for j in (0, cols // 8, cols // 4, 3 * cols // 8, cols // 2)], [round(float(least[k, j]), 3) for j in (0, cols // 4, cols // 2)])
        grid[k, :, 0] = middle[0] + dirs[:, 0] * radius[k]
        grid[k, :, 1] = middle[1] + dirs[:, 1] * radius[k]
        grid[k, :, 2] = z
else:
    widths = np.array(KIND["width"])
    front = np.zeros((rings, cols))
    most = np.zeros((rings, cols))
    xs = np.zeros((rings, cols))
    for k, z in enumerate(heights):
        t = k / (rings - 1)
        half = float(np.interp(t, widths[:, 0], widths[:, 1]))
        poly = outline(z)
        xs[k] = middle[0] + np.linspace(-half, half, cols)
        lo, hi = (poly[:, 0].min(), poly[:, 0].max()) if len(poly) >= 3 else (middle[0], middle[0])
        for j, x in enumerate(xs[k]):
            # The outline's front at this column, or past its side, its front edge carried on
            # and turned back round the body.
            cx = min(max(x, lo + 0.005), hi - 0.005)
            body_y = middle[1] - reach(poly, np.array([cx, middle[1]]), np.array([0.0, -1.0])) if len(poly) >= 3 else middle[1]
            body_y += 0.5 * abs(x - cx)
            body_y -= clearance(k)
            most[k, j] = body_y
            dressed_y = middle[1] - outermost((x, middle[1], z), (0.0, -1.0, 0.0))
            front[k, j] = min(body_y, max(dressed_y, body_y - BULK_M))
    # Front is -Y: the room the body needs is a most, not a least.
    for _ in range(SMOOTH_ROUNDS):
        front[:, 1:-1] = np.minimum(most[:, 1:-1], (front[:, :-2] + 2 * front[:, 1:-1] + front[:, 2:]) / 4)
        front[1:-1] = np.minimum(most[1:-1], (front[:-2] + 2 * front[1:-1] + front[2:]) / 4)
        for k in range(1, rings):
            if heights_b[k] < hips_b:
                front[k] = np.minimum(front[k], front[k - 1] - FLARE * 0.5 * (heights[k - 1] - heights[k]))
    grid[:, :, 0] = xs
    grid[:, :, 1] = front
    grid[:, :, 2] = heights[:, None]
closed = KIND["shape"] == "around"

# The mesh, on a copy of the body's own (which carries its vertex groups' names).
piece = body.copy()
piece.data = body.data.copy()
piece.name = item
bpy.context.scene.collection.objects.link(piece)
piece.modifiers.clear()
bm = bmesh.new()
bm.from_mesh(piece.data)
bmesh.ops.delete(bm, geom=bm.verts[:], context="VERTS")
verts = [[bm.verts.new(Vector(grid[k, j])) for j in range(cols)] for k in range(rings)]
# One unwrap of its own, none of the body's (its texture's and its patches').
for layer in list(bm.loops.layers.uv.values()):
    bm.loops.layers.uv.remove(layer)
uv = bm.loops.layers.uv.new("grid")
for k in range(rings - 1):
    for j in range(cols if closed else cols - 1):
        jn = (j + 1) % cols
        # Wound so the face looks out, away from the body.
        face = bm.faces.new((verts[k][j], verts[k + 1][j], verts[k + 1][jn], verts[k][jn]))
        # Unwrapped as the grid it is: round the body across, down it down, so its cloth runs
        # on unbroken where an automatic unwrap would cut it into strips.
        for loop, (kk, jj) in zip(face.loops, ((k, j), (k + 1, j), (k + 1, j + 1), (k, j + 1))):
            loop[uv].uv = (0.005 + 0.99 * jj / (cols if closed else cols - 1), 0.995 - 0.99 * kk / (rings - 1))
bmesh.ops.recalc_face_normals(bm, faces=bm.faces[:])
bm.verts.ensure_lookup_table()
# Faces out: the first ring's middle is inside, so its faces look away from it.
f = bm.faces[0]
if (f.calc_center_median() - Vector((middle[0], middle[1], f.calc_center_median().z))).dot(f.normal) < 0:
    bmesh.ops.reverse_faces(bm, faces=bm.faces[:])
bm.to_mesh(piece.data)
for layer in [l for l in piece.data.uv_layers if l.name != "grid"]:
    piece.data.uv_layers.remove(layer)
piece.data.uv_layers["grid"].name = "UVMap"
piece.data.uv_layers.active = piece.data.uv_layers["UVMap"]
# Smooth across the cloth: never a flat-shaded face (docs/art-style.md).
piece.data.shade_smooth()
bm.free()
P = np.array([tuple(mw @ v.co) for v in piece.data.vertices])
edges = np.array([tuple(e.vertices) for e in piece.data.edges])
on_base = P - skin_piece.follow(P, skin, delta, edges) if SHAPE else P.copy()
ring_of = np.repeat(np.arange(rings), cols)


def smooth(e0, e1, x):
    t = min(1.0, max(0.0, (x - e0) / (e1 - e0)))
    return t * t * (3 - 2 * t)


# 2. Weights.
mid_x = joint("pelvis")[0]
leg_full_b = hips_b - (hips_b - knee_b) * LEG_FULL
below_knee = knee_b + 0.05
named = {g.name: g for g in piece.vertex_groups}
for i, p in enumerate(on_base):
    z = p[2]
    _, nearest, _ = base_kd.find(Vector(p))
    torso = {b: w for b, w in weights[nearest].items() if b.startswith(("pelvis", "spine_"))}
    s = sum(torso.values())
    torso = {b: w / s for b, w in torso.items()} if s > 0 else {"pelvis": 1.0}
    leg = smooth(0.0, 1.0, (hips_b - z) / (hips_b - leg_full_b))
    left = smooth(-SIDE_M, SIDE_M, p[0] - mid_x)
    calf = CALF * smooth(0.0, 1.0, (below_knee - z) / max(1e-3, below_knee - hem_b))
    w = {b: v * (1.0 - leg) for b, v in torso.items()}
    for b, v in (("thigh_l", leg * left * (1 - calf)), ("thigh_r", leg * (1 - left) * (1 - calf)),
                 ("calf_l", leg * left * calf), ("calf_r", leg * (1 - left) * calf)):
        if v > 0:
            w[b] = w.get(b, 0.0) + v
    kept = dict(sorted(w.items(), key=lambda bw: -bw[1])[:4])
    total = sum(kept.values())
    for b, v in kept.items():
        if v / total > 1e-4:
            named[b].add([i], v / total, "REPLACE")

# 3. Colour, surface and roughness from the dressed model.


def column_behind(u):
    """A column of the unwrap (0 to 1) and the one on the other side of the body, the angle
    from the front turned to the back: where a skirt under an apron finds its own cloth."""
    j = (u - 0.005) / 0.99 * cols
    return 0.005 + 0.99 * ((cols / 2 - j) % cols) / cols


inv = piece.matrix_world.inverted()
if UNDER:
    hidden = piece.data.attributes.new("hidden", "FLOAT", "POINT")
    normals = [(piece.matrix_world.to_3x3() @ v.normal).normalized() for v in piece.data.vertices]
    for i, p in enumerate(P):
        o, n = Vector(p), normals[i]
        hidden.data[i].value = float(any(t.ray_cast(o + n * 0.001, n, 0.08)[0] is not None for t in under_trees))
skin_piece.bake(piece, dressed, item, role, TEXTURE, LOOK_M, under="hidden" if UNDER else None, plain="plain" in options, unwrap=False,
                behind=column_behind if closed else None)
if UNDER:
    piece.data.attributes.remove(piece.data.attributes["hidden"])

# Back onto the base's skin, as every look is kept.
for v in piece.data.vertices:
    v.co = inv @ Vector(on_base[v.index])
piece.data.update()

# 4. Its edges: the top turned in to the skin, the hem and sides given a thickness.
base_tree = BVHTree.FromPolygons([Vector(p) for p in base], [tuple(p.vertices) for p in body.data.polygons])
normals = [(piece.matrix_world.to_3x3() @ v.normal).normalized() for v in piece.data.vertices]


def inward(i):
    p = Vector(on_base[i])
    if ring_of[i] == 0:
        hit = base_tree.find_nearest(p)
        if hit[0] is not None:
            return inv @ (hit[0] + hit[1] * TOP_RIM_M)
    return inv @ (p - normals[i] * THICK_M)


skin_piece.rim(piece, inward)
piece["hung"] = 1
piece["covers"] = []
print("HUNG", item, f"{len(piece.data.vertices)} vertices, {len(piece.data.polygons)} faces")

placed = piece.matrix_world.copy()
piece.parent = None
piece.matrix_world = placed
piece.parent = arm
piece.matrix_parent_inverse = arm.matrix_world.inverted()
piece.modifiers.new("Armature", "ARMATURE").object = arm
bpy.data.objects.remove(dressed)
bpy.data.objects.remove(body)
os.makedirs(os.path.join(look.GEAR_MADE, body_name), exist_ok=True)
path = os.path.join(look.GEAR_MADE, body_name, f"{item}.glb")
bpy.ops.object.select_all(action="DESELECT")
piece.select_set(True)
arm.select_set(True)
bpy.context.view_layer.objects.active = arm
bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_animations=False,
                          export_influence_nb=4, export_extras=True)
print("EXPORTED", path)
