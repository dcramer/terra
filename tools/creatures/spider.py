"""The giant spider: Tripo's textured model fitted onto the skeleton and clips the game's spider
already has (Quaternius's SpiderArmature, kept as `art/creatures/spider/rig.glb`), so every clip,
its timing, its foot contacts and the hit zones' joints stay as they were.

    tools/blender/run.sh tools/creatures/spider.py art/creatures/spider assets/models/creatures/Spider.glb [REPORT.json]

The body is found from above: the model's top view is rasterised and eroded until only the thick
parts are left (cephalothorax and abdomen; legs are thin), the abdomen is the larger part past
the waist, and that gives its front. The legs are what the mesh holds apart once the body is
taken away, eight pieces, each matched to the rig's leg it lies nearest in angle. Each leg is then
bent onto its bone chain as a spline mesh is bent along a spline (Unreal's SplineMeshComponent,
from its documentation): every vertex keeps its offset from the leg's centre line, in a frame
carried along that line, while the line is laid along the rig's bones, the leg's highest point
(its knee) on the rig's knee. The leg's thickness is kept; its length becomes the rig's. A leg is
weighted along its chain by where it lies on it; the body is weighted by part as the old one is
(abdomen, thorax, head, parted at the waist), each leg's root handed over to it along the leg;
where the parts part, how wide they blend and how far the roots hand over are chosen by posing
the skin through every clip and measuring its trunk and seams as creature_check does. Clips are the rig's own, untouched; the check at the
end imports the export again and measures how far each joint moved from the rig's own clips.
"""

import heapq
import itertools
import json
import math
import os
import re
import struct
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "characters"))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import bmesh
import bpy
import numpy as np
from mathutils import Matrix, Quaternion, Vector

import look
import measures as ms

argv = sys.argv[sys.argv.index("--") + 1:]
folder, out = argv[:2]
report_path = argv[2] if len(argv) > 2 else None
spec = json.load(open(os.path.join(folder, "creature.json")))
BUDGET = int(spec["budget"])
TEX = spec["textures"]
RIG = os.path.join(folder, "rig.glb")
SOURCE = os.path.join(folder, "model.glb")
with open(os.path.join(look.ASSETS, "data", "creatures.ron")) as f:
    GAME_SCALE = float(re.search(r'"models/creatures/%s\.glb",\s*([0-9.]+)' % spec["file"], f.read()).group(1))
LEGS = [f"{leg}{side}" for side in (".L", ".R") for leg in ("Front", "MidFront", "MidBack", "Back")]
report = {"budget": BUDGET, "game_scale": GAME_SCALE}


def chain(leg):
    name, side = leg.split(".")
    return [f"{name}Leg.{side}", f"{name}Leg2.{side}", f"{name}Leg3.{side}", f"{name}Leg3.{side}_end"]


def smoothstep(a, b, x):
    t = min(1.0, max(0.0, (x - a) / (b - a))) if b != a else float(x >= a)
    return t * t * (3 - 2 * t)


# The rig's rest joints, read from its own glTF so a tip (a `_end` node, not a joint) is there too.
def glb_json(path):
    with open(path, "rb") as fh:
        data = fh.read()
    return json.loads(data[20:20 + struct.unpack_from("<I", data, 12)[0]])


def node_worlds(j):
    world = {}

    def local(n):
        t = Vector(n.get("translation", (0, 0, 0)))
        r = n.get("rotation", (0, 0, 0, 1))
        s = n.get("scale", (1, 1, 1))
        return Matrix.Translation(t) @ Quaternion((r[3], r[0], r[1], r[2])).to_matrix().to_4x4() @ Matrix.Diagonal((*s, 1))

    def walk(i, parent):
        m = parent @ local(j["nodes"][i])
        world[j["nodes"][i]["name"]] = m
        for c in j["nodes"][i].get("children", []):
            walk(c, m)

    for i in j["scenes"][j.get("scene", 0)]["nodes"]:
        walk(i, Matrix.Identity(4))
    return world


def bind_joints(path, j):
    """Each joint where the skin is bound to it (its inverse bind matrix inverted, under the
    skinned mesh's node), and each `_end` tip at its offset from its joint there: the bind
    pose, which in this rig is not the nodes' rest (up to a unit apart)."""
    with open(path, "rb") as fh:
        data = fh.read()
    n = struct.unpack_from("<I", data, 12)[0]
    binary = data[20 + n + 8:]
    skin = j["skins"][0]
    acc = j["accessors"][skin["inverseBindMatrices"]]
    view = j["bufferViews"][acc["bufferView"]]
    start = view.get("byteOffset", 0) + acc.get("byteOffset", 0)
    worlds = node_worlds(j)
    mesh_node = next(nd["name"] for nd in j["nodes"] if "skin" in nd)
    got = {}
    for k, ji in enumerate(skin["joints"]):
        f = struct.unpack_from("<16f", binary, start + 64 * k)
        ibm = Matrix([f[r::4] for r in range(4)])
        bind = worlds[mesh_node] @ ibm.inverted()
        got[j["nodes"][ji]["name"]] = bind
        for c in j["nodes"][ji].get("children", []):
            child = j["nodes"][c]
            if c not in skin["joints"]:
                got[child["name"]] = bind @ Matrix.Translation(Vector(child.get("translation", (0, 0, 0))))
    return {k: (Y_UP @ m).translation.copy() for k, m in got.items()}


Y_UP = Matrix(((1, 0, 0, 0), (0, 0, -1, 0), (0, 1, 0, 0), (0, 0, 0, 1)))  # glTF to Blender
rig_json = glb_json(RIG)
JOINT = bind_joints(RIG, rig_json)
CLIPS = [a["name"] for a in rig_json["animations"]]

look.reset()
scene = bpy.context.scene
scene.render.fps = 24
bpy.ops.import_scene.gltf(filepath=RIG)
rig = next(o for o in scene.objects if o.type == "ARMATURE")
old_meshes = [o for o in scene.objects if o.type == "MESH"]
# Only the skin counts: the file holds other meshes below it, and the ground is the origin.
old_skins = [o for o in old_meshes if any(m.type == "ARMATURE" for m in o.modifiers)]
drift = max((rig.matrix_world @ b.head_local - JOINT[b.name]).length for b in rig.data.bones if b.name in JOINT)
print("rig joints, Blender's rest against the glTF's: max", round(drift, 6))
rig.data.pose_position = "REST"
bpy.context.view_layer.update()

# The old skin at rest, with its weights, for the body's weights.
dg = bpy.context.evaluated_depsgraph_get()
old_pts, old_w = [], []
for o in old_skins:
    ev = o.evaluated_get(dg)
    me = ev.to_mesh()
    names = {g.index: g.name for g in o.vertex_groups}
    for v, ov in zip(me.vertices, o.data.vertices):
        old_pts.append(ev.matrix_world @ v.co)
        old_w.append({names[g.group]: g.weight for g in ov.groups if g.weight > 0})
    ev.to_mesh_clear()
leg_bones = {b for leg in LEGS for b in chain(leg)}
BODY_BONES = sorted({n for w in old_w for n in w} & {"Body", "Thorax", "Head", "Abdomen"})
# The old body's lowest point: its skin whose strongest bone is a body bone.
old_body_low = min(p.z for p, w in zip(old_pts, old_w) if w and max(w, key=w.get) in BODY_BONES)
report["old_rest_lowest_m"] = round(min(p.z for p in old_pts) * GAME_SCALE, 3)
report["old_body_lowest_m"] = round(old_body_low * GAME_SCALE, 3)
print("body bones", BODY_BONES, "old skin lowest", report["old_rest_lowest_m"], "m, old body lowest", report["old_body_lowest_m"], "m")

# Tripo's model: welded, cut to its budget.
before = set(scene.objects)
bpy.ops.import_scene.gltf(filepath=SOURCE)
body = next(o for o in scene.objects if o not in before and o.type == "MESH")
for o in [o for o in scene.objects if o not in before and o is not body]:
    bpy.data.objects.remove(o)
body.data.transform(body.matrix_world)
body.parent = None
body.matrix_world = Matrix.Identity(4)
body.name = body.data.name = spec["file"]


def select_only(ob):
    for o in scene.objects:
        o.select_set(o is ob)
    bpy.context.view_layer.objects.active = ob


select_only(body)
bm = bmesh.new()
bm.from_mesh(body.data)
bmesh.ops.remove_doubles(bm, verts=bm.verts[:], dist=1e-5)
bm.to_mesh(body.data)
bm.free()
if body.data.has_custom_normals:
    bpy.ops.mesh.customdata_custom_splitnormals_clear()
tris = sum(len(p.vertices) - 2 for p in body.data.polygons)
cut = body.modifiers.new("Reduce", "DECIMATE")
cut.ratio = min(1.0, BUDGET / tris)
cut.use_collapse_triangulate = True
bpy.ops.object.modifier_apply(modifier=cut.name)
body.data.validate(clean_customdata=False)
me = body.data
report["source_triangles"] = tris
print("triangles", tris, "to", sum(len(p.vertices) - 2 for p in me.polygons))
V = np.array([v.co[:] for v in me.vertices])
EDGES = np.array([e.vertices[:] for e in me.edges])
NEIGH = [[] for _ in range(len(V))]
for a, b in EDGES:
    NEIGH[a].append(b)
    NEIGH[b].append(a)


# The body from above: the top view rasterised, eroded past the legs' thickness, grown back.
def disk_shift(mask, r, grow):
    outm = np.zeros_like(mask) if grow else np.ones_like(mask)
    pad = np.pad(mask, r)
    for dy in range(-r, r + 1):
        for dx in range(-r, r + 1):
            if dx * dx + dy * dy <= r * r:
                win = pad[r + dy:r + dy + mask.shape[0], r + dx:r + dx + mask.shape[1]]
                outm = (outm | win) if grow else (outm & win)
    return outm


def components(mask):
    label = np.zeros(mask.shape, int)
    n = 0
    for start in zip(*np.nonzero(mask)):
        if label[start]:
            continue
        n += 1
        label[start] = n
        todo = [start]
        while todo:
            y, x = todo.pop()
            for q in ((y + 1, x), (y - 1, x), (y, x + 1), (y, x - 1)):
                if 0 <= q[0] < mask.shape[0] and 0 <= q[1] < mask.shape[1] and mask[q] and not label[q]:
                    label[q] = n
                    todo.append(q)
    return label, n


G = 192
lo2 = V[:, :2].min(0)
span = float((V[:, :2].max(0) - lo2).max())
cell = span / (G - 8)
lo2 = lo2 - 4 * cell
tri_idx = np.array([t.vertices[:] for t in me.loop_triangles])
m = 10
bary = np.array([(i / m, j / m) for i in range(m + 1) for j in range(m + 1 - i)])
A, B, C = V[tri_idx[:, 0], :2], V[tri_idx[:, 1], :2], V[tri_idx[:, 2], :2]
samples = A[:, None] + bary[None, :, 0:1] * (B - A)[:, None] + bary[None, :, 1:2] * (C - A)[:, None]
cells = np.clip(((samples.reshape(-1, 2) - lo2) / cell).astype(int), 0, G - 1)
top = np.zeros((G, G), bool)
top[cells[:, 1], cells[:, 0]] = True
top = disk_shift(disk_shift(top, 2, True), 2, False)
r_e = max(2, round(0.045 * span / cell))
core = disk_shift(top, r_e, False)
label, n = components(core)
sizes = [(label == i).sum() for i in range(1, n + 1)]
keep = [i + 1 for i, s in enumerate(sizes) if s >= 0.15 * max(sizes)]
core = np.isin(label, keep)
blob = disk_shift(core, r_e + 2, True) & top
print("body from above: erode", r_e, "cells; parts", sorted(sizes, reverse=True)[:4], "kept", len(keep))

ys, xs = np.nonzero(core)
P2 = np.stack([xs, ys], 1) * cell + lo2
centre2 = P2.mean(0)
w_, vecs = np.linalg.eigh(np.cov((P2 - centre2).T))
axis2 = vecs[:, -1]
t = (P2 - centre2) @ axis2
bins = np.linspace(t.min(), t.max(), 25)
width = np.histogram(t, bins)[0]
mid = slice(5, 20)
waist = bins[5 + int(np.argmin(width[mid]))]
front, back = P2[t > waist], P2[t <= waist]
ahead = 1.0
if len(front) > len(back):
    front, back = back, front
    ahead = -1.0
fwd_new = Vector((*(front.mean(0) - back.mean(0)), 0)).normalized()
print("waist at", round(float(waist / (t.max() - t.min())), 2), "of the length; cephalothorax", len(front), "cells, abdomen", len(back))
# Each vertex's place along the body from the waist, in the body's lengths, the cephalothorax
# ahead (positive). Turning and scaling keep it, so the body's weights are read from it.
U = ahead * ((V[:, :2] - centre2) @ axis2 - waist) / (t.max() - t.min())

in_blob = blob[np.clip(((V[:, 1] - lo2[1]) / cell).astype(int), 0, G - 1), np.clip(((V[:, 0] - lo2[0]) / cell).astype(int), 0, G - 1)]

# The legs: the mesh's pieces once the body is taken away, the eight reaching farthest.
piece = -np.ones(len(V), int)
pieces = []
for s in range(len(V)):
    if in_blob[s] or piece[s] >= 0:
        continue
    piece[s] = len(pieces)
    todo, members = [s], []
    while todo:
        a = todo.pop()
        members.append(a)
        for b in NEIGH[a]:
            if not in_blob[b] and piece[b] < 0:
                piece[b] = len(pieces)
                todo.append(b)
    pieces.append(np.array(members))
reach = [float(np.linalg.norm(V[p, :2] - centre2, axis=1).max()) for p in pieces]
order = np.argsort(reach)[::-1]
print("pieces off the body", len(pieces), "reaching", [round(reach[i] / span, 2) for i in order[:10]])
if len(pieces) < 8 or reach[order[7]] < 0.25 * span:
    sys.exit("fewer than eight legs stand apart from the body")
leg_pieces = [pieces[i] for i in order[:8]]
rest = np.ones(len(V), bool)
for p in leg_pieces:
    rest[p] = False


# Each leg's centre line: distance from the body over its own skin, binned, the bins' middles.
def geodesic(members):
    inside = set(members.tolist())
    dist = {}
    heap = [(0.0, int(a)) for a in members if any(in_blob[b] for b in NEIGH[a])]
    if not heap:
        heap = [(0.0, int(members[np.argmin(np.linalg.norm(V[members, :2] - centre2, axis=1))]))]
    while heap:
        d, a = heapq.heappop(heap)
        if a in dist:
            continue
        dist[a] = d
        for b in NEIGH[a]:
            if b in inside and b not in dist:
                heapq.heappush(heap, (d + float(np.linalg.norm(V[a] - V[b])), b))
    return np.array([dist.get(int(a), 0.0) for a in members])


N = 24


def centre_line(members, u):
    k = np.minimum((u * N).astype(int), N - 1)
    pts = np.array([V[members[k == i]].mean(0) if (k == i).any() else np.full(3, np.nan) for i in range(N)])
    good = ~np.isnan(pts[:, 0])
    pts = np.array([np.interp(np.arange(N), np.nonzero(good)[0], pts[good, c]) for c in range(3)]).T
    for _ in range(2):
        pts[1:-1] = 0.25 * pts[:-2] + 0.5 * pts[1:-1] + 0.25 * pts[2:]
    return pts


legs = []
for members in leg_pieces:
    s = geodesic(members)
    legs.append({"members": members, "s": s / s.max(), "line": centre_line(members, s / s.max())})

# Ours: turned to the rig's front, scaled so the feet reach as far as the rig's, the legs' roots
# on the rig's hips.
fwd_old = JOINT["Head_end"] - JOINT["Abdomen_end"]
fwd_old = Vector((fwd_old.x, fwd_old.y, 0)).normalized()
turn = math.atan2(fwd_old.y, fwd_old.x) - math.atan2(fwd_new.y, fwd_new.x)
hips = np.array([JOINT[chain(leg)[0]][:] for leg in LEGS])
tips_old = np.array([JOINT[chain(leg)[3]][:] for leg in LEGS])
roots = np.array([lg["line"][0] for lg in legs])
tips_new = np.array([lg["line"][-1] for lg in legs])
reach_old = np.linalg.norm(tips_old[:, :2] - hips[:, :2].mean(0), axis=1).mean()
scale = reach_old / np.linalg.norm(tips_new[:, :2] - roots[:, :2].mean(0), axis=1).mean()
Rz = np.array(Matrix.Rotation(turn, 3, "Z"))
pivot = np.append(roots[:, :2].mean(0), roots[:, 2].mean())


def ours(p):
    return (np.asarray(p) - pivot) @ Rz.T * scale + hips.mean(0)


V = ours(V)
for lg in legs:
    lg["line"] = ours(lg["line"])
report["turn_degrees"] = round(math.degrees(turn), 1)
report["scale"] = round(float(scale), 4)
print("turned", report["turn_degrees"], "deg, scaled", report["scale"])

# Placed by its leg roots alone, Tripo's body hangs deeper below them than the old one did, into
# the floor, and a bigger body than the old swings lower in a clip that tilts it (Death). So the
# body is posed through every clip with its parts' weights, as a rigger poses a fitted mesh
# before handing it on: the largest scale about the hips at which, lifted until its rest lowest
# is the old body's, it goes no more than FIT_TOLERANCE below the old body in any frame. The legs
# keep the rig's lengths, so the scale sets only the body's size and the legs' thickness. Each
# leg's root follows the body; `lay` fades that offset out by the knee.
#
# The body is weighted as the old one is, rigidly by part: the abdomen to Abdomen, the
# cephalothorax to Thorax and, ahead of the old body's neck (as far along the cephalothorax), to
# Head, with a short blend at the waist and the neck. Taking each vertex's weights from the old
# skin nearest it instead fails once the body is resized and lifted: the nearest old skin is then
# the wrong part (the head reaching back over the thorax), and Death, which turns the parts about
# each other, tears the body and swings it into the floor.
OLD_DOM = np.array([max(w, key=w.get) if w else "" for w in old_w])
old_u = np.array([p.dot(fwd_old) for p in old_pts])
old_body = np.isin(OLD_DOM, BODY_BONES)
if not {"Abdomen", "Thorax", "Head"} <= set(BODY_BONES):
    sys.exit(f"the old skin's body bones are {BODY_BONES}, not Abdomen, Thorax and Head")


def split_at(behind, ahead_):
    """The place along the body that best parts the old skin's vertices `behind` from `ahead_`."""
    cand = np.quantile(old_u[behind | ahead_], np.linspace(0.0, 1.0, 401))
    miss = [int((behind & (old_u > c)).sum() + (ahead_ & (old_u < c)).sum()) for c in cand]
    return float(cand[int(np.argmin(miss))]), min(miss)


waist_old, miss_w = split_at(OLD_DOM == "Abdomen", np.isin(OLD_DOM, ["Thorax", "Head"]))
neck_old, miss_n = split_at(OLD_DOM == "Thorax", OLD_DOM == "Head")
front_old = float(np.quantile(old_u[old_body], 0.995))
back_old = float(np.quantile(old_u[old_body], 0.005))
NECK = (neck_old - waist_old) / (front_old - waist_old)
front_new = float(np.quantile(U[rest], 0.995))
BLEND = 0.04
report["body_parts"] = {
    "old_waist_of_length": round((waist_old - back_old) / (front_old - back_old), 3),
    "new_waist_of_length": round(-float(np.quantile(U[rest], 0.005)) / (front_new - float(np.quantile(U[rest], 0.005))), 3),
    "neck_of_cephalothorax": round(NECK, 3),
    "old_vertices_off_their_part": [miss_w, miss_n],
    "old_body_blended": round(float(np.mean([max(old_w[k].values()) < 0.9 for k in np.nonzero(old_body)[0]])), 3),
}
print("body parts", json.dumps(report["body_parts"]))


def part_weights(u, blend=BLEND, waist_at=0.0, neck_at=None):
    """The body's bones at `u` along it (U's measure), as the old body's parts lie: Abdomen behind
    `waist_at`, Head ahead of `neck_at` (the old neck's place along the cephalothorax), Thorax
    between, each blended over `blend` either side."""
    neck_at = NECK * front_new if neck_at is None else neck_at
    a = smoothstep(waist_at - blend, waist_at + blend, u)
    h = smoothstep(max(neck_at, waist_at) - blend, max(neck_at, waist_at) + blend, u)
    w = {"Abdomen": 1 - a, "Thorax": a * (1 - h), "Head": a * h}
    return {n: x for n, x in w.items() if x > 0}


REST_W = [part_weights(u) for u in U[rest]]


def clip_poses():
    """Every rig clip, every other frame from the rest: each bone's skinning matrix in the world,
    and the old body's lowest point there."""
    world, inv = np.array(rig.matrix_world), np.array(rig.matrix_world.inverted())
    rig.data.pose_position = "POSE"
    rig.animation_data_create()
    poses = []
    for act in bpy.data.actions:
        name = next((c for c in CLIPS if c.split("|")[-1] in act.name), None)
        if not name:
            continue
        rig.animation_data.action = act
        if act.slots:
            rig.animation_data.action_slot = act.slots[0]
        for pb in rig.pose.bones:
            pb.matrix_basis = Matrix()
        f0, f1 = (int(x) for x in act.frame_range)
        for f in range(f0, f1 + 1, 2):
            scene.frame_set(f)
            dgf = bpy.context.evaluated_depsgraph_get()
            zs = []
            for o in old_skins:
                ev = o.evaluated_get(dgf)
                mm = ev.to_mesh()
                zs.extend((ev.matrix_world @ v.co).z for v in mm.vertices)
                ev.to_mesh_clear()
            M = {pb.name: world @ np.array(pb.matrix) @ np.array(pb.bone.matrix_local.inverted()) @ inv for pb in rig.pose.bones}
            poses.append((name.split("|")[-1], f, M, float(np.array(zs)[old_body].min())))
    rig.animation_data.action = None
    for pb in rig.pose.bones:
        pb.matrix_basis = Matrix()
    rig.data.pose_position = "REST"
    bpy.context.view_layer.update()
    return poses


def worst_below_old(P, poses):
    """How far the body P, skinned by its parts' weights, goes below the old body at worst, and where."""
    W_ = REST_W
    names = sorted({n for w in W_ for n in w})
    Wm = np.array([[w.get(n, 0.0) for n in names] for w in W_])
    Wm /= Wm.sum(1)[:, None]
    Ph = np.c_[P, np.ones(len(P))]
    worst = (1e9, "", 0)
    for clip, f, M, old_low in poses:
        z = sum(Wm[:, j] * (Ph @ M[n][2]) for j, n in enumerate(names))
        worst = min(worst, (float(z.min() - old_low), clip, f))
    return worst


# Past some scale a part that is not the body's bulk (the fangs, as the head dips in Death) is what
# goes lowest, and shrinking no longer helps; there the body is lifted the rest of the way instead,
# by at most EXTRA_LIFT over the old body's rest height.
FIT_TOLERANCE, EXTRA_LIFT = 0.02 / GAME_SCALE, 0.03 / GAME_SCALE
poses = clip_poses()
centre, V0, fit = hips.mean(0), V.copy(), {}
for k in range(10):
    body_scale = round(1.0 - 0.05 * k, 2)
    P = centre + (V0[rest] - centre) * body_scale
    P[:, 2] += old_body_low - P[:, 2].min()
    fit[body_scale] = worst_below_old(P, poses)
    if -fit[body_scale][0] - FIT_TOLERANCE <= EXTRA_LIFT:
        break
else:
    body_scale = max(fit, key=lambda k: (fit[k][0], k))
extra = max(0.0, -fit[body_scale][0] - FIT_TOLERANCE)
report["body_fit"] = {str(k): [round(v[0] * GAME_SCALE, 3), v[1], v[2]] for k, v in fit.items()}
print("body fit: scale, worst below the old body (m), clip, frame", json.dumps(report["body_fit"]))
V = centre + (V0 - centre) * body_scale
lift = old_body_low - V[rest, 2].min() + extra
V[:, 2] += lift
for lg in legs:
    lg["line"] = centre + (lg["line"] - centre) * body_scale
    lg["line"][:, 2] += lift
report["body_scale"] = body_scale
report["body_lift_m"] = round(float(lift) * GAME_SCALE, 3)
print("body scaled", body_scale, "lifted", report["body_lift_m"], "m")


def wrap(a):
    return (a + math.pi) % (2 * math.pi) - math.pi


hc = hips.mean(0)
a_old = [math.atan2(t[1] - hc[1], t[0] - hc[0]) for t in tips_old]
a_new = [math.atan2(lg["line"][-1][1] - hc[1], lg["line"][-1][0] - hc[0]) for lg in legs]
best = min(itertools.permutations(range(8)), key=lambda p: sum(wrap(a_new[p[i]] - a_old[i]) ** 2 for i in range(8)))
report["leg_angle_off_degrees"] = {LEGS[i]: round(math.degrees(wrap(a_new[best[i]] - a_old[i])), 1) for i in range(8)}
print("legs matched, off by", report["leg_angle_off_degrees"])


# Laid on the rig's bones: a frame carried along each line (parallel transport), each vertex kept
# at its offset in it.
def frames(pts):
    T = np.gradient(pts, axis=0)
    T /= np.linalg.norm(T, axis=1)[:, None]
    side = np.cross(T[0], (0.0, 0.0, 1.0))
    side = side / np.linalg.norm(side) if np.linalg.norm(side) > 1e-6 else np.array((1.0, 0.0, 0.0))
    got = []
    for t_ in T:
        side = side - side.dot(t_) * t_
        side /= np.linalg.norm(side)
        got.append(np.stack([t_, side, np.cross(t_, side)]))
    return np.array(got)


def along(pts):
    return np.concatenate([[0.0], np.cumsum(np.linalg.norm(np.diff(pts, axis=0), axis=1))])


def at(table, xs, x):
    i = int(np.clip(np.searchsorted(xs, x) - 1, 0, len(xs) - 2))
    f = (x - xs[i]) / max(1e-9, xs[i + 1] - xs[i])
    return table[i] * (1 - f) + table[i + 1] * f


def orthonormal(F):
    U, _, Wt = np.linalg.svd(F)
    return U @ Wt


def nearest_on_line(p, line, u_line, u_guess):
    best_u, best_d = u_guess, 1e9
    k_mid = int(u_guess * (len(line) - 1))
    for k in range(max(0, k_mid - 4), min(len(line) - 1, k_mid + 4)):
        seg = line[k + 1] - line[k]
        f = float(np.clip((p - line[k]).dot(seg) / max(1e-12, seg.dot(seg)), 0, 1))
        d = float(np.linalg.norm(p - (line[k] + f * seg)))
        if d < best_d:
            best_d, best_u = d, u_line[k] + f * (u_line[k + 1] - u_line[k])
    return best_u


weights = [dict() for _ in range(len(V))]
ROOT_SHARE = {}
report["legs"] = {}


def chain_line(leg):
    """A leg's bound chain, root to its `_end` tip, sampled along its length."""
    J = np.array([JOINT[b][:] for b in chain(leg)])
    lens = np.linalg.norm(np.diff(J, axis=0), axis=1)
    cum = np.concatenate([[0.0], np.cumsum(lens)])
    a_s = np.linspace(0, cum[-1], 96)
    return J, lens, cum, a_s, np.array([at(J, cum, a) for a in a_s])


# The old skin's feet: each leg's tip is weighted to a foot bone parented to the root (the rig's IK
# target, Quaternius's setup), so a planted foot stays where the clip puts it whatever the leg's
# last bone does (in Idle that bone's tip lies 9 cm under the floor). A new leg hands its tip to
# that bone where the old leg's skin does along the chain: each old vertex goes to the chain
# nearest it; the hand-over runs from the farthest of its own the last leg bone holds most to the
# nearest the foot bone does.
P_old = np.array([p[:] for p in old_pts])
near = np.stack([np.linalg.norm(P_old[:, None, :] - chain_line(leg)[4][None], axis=2).min(1) for leg in LEGS])
strongest = [max(w, key=w.get) if w else "" for w in old_w]
FOOT = {}
for i, leg in enumerate(LEGS):
    J, lens, cum, a_s, Q = chain_line(leg)
    own = np.nonzero((near.argmin(0) == i) & (near[i] < 0.25 * cum[-1]))[0]
    on_foot = [k for k in own if "Foot" in strongest[k]]
    on_last = [k for k in own if strongest[k] == chain(leg)[2]]
    if on_foot:
        bone = max({strongest[k] for k in on_foot}, key=[strongest[k] for k in on_foot].count)
        arc = lambda ks: a_s[np.linalg.norm(P_old[ks][:, None, :] - Q[None], axis=2).argmin(1)]
        a_foot = float(arc(on_foot).min())
        a_ring = min(float(arc(on_last).max()), a_foot - 0.02 * cum[-1]) if on_last else a_foot - 0.1 * lens[2]
        FOOT[leg] = (bone, a_ring, a_foot)
report["feet"] = {leg: [bone, round(r * GAME_SCALE, 3), round(a * GAME_SCALE, 3)] for leg, (bone, r, a) in FOOT.items()}
print("feet: bone, from, to (m along the chain)", json.dumps(report["feet"]))


def lay(leg, lg, fade, foot_span=1.0):
    J, lens, cum, a_s, Q = chain_line(leg)
    a_end = cum[-1]
    soft = Q.copy()
    for _ in range(6):
        soft[1:-1] = 0.25 * soft[:-2] + 0.5 * soft[1:-1] + 0.25 * soft[2:]
    F_old, line = frames(soft), lg["line"]
    F_new = frames(line)
    u_line = along(line) / along(line)[-1]
    # The knee: the point highest over the line from root to tip.
    above = line[:, 2] - (line[0, 2] + u_line * (line[-1, 2] - line[0, 2]))
    within = (u_line > 0.12) & (u_line < 0.65)
    knee_new = u_line[int(np.argmax(np.where(within, above, -1e9)))]
    above_old = J[1:3, 2] - (J[0, 2] + cum[1:3] / cum[-1] * (J[3, 2] - J[0, 2]))
    knee_old = cum[1 + int(np.argmax(above_old))]
    report.setdefault("bones_m", {})[leg] = [round(float(x) * GAME_SCALE, 3) for x in lens]
    a0 = min(0.8 * cum[1], a_s[int(np.argmin(np.linalg.norm(Q - line[0], axis=1)))])

    def a_of(u):
        if u < knee_new:
            return a0 + u / knee_new * (knee_old - a0)
        return knee_old + (u - knee_new) / (1 - knee_new) * (a_end - knee_old)

    h1, h2 = 0.25 * min(lens[0], lens[1]), 0.25 * min(lens[1], lens[2])
    b = chain(leg)
    foot, a_ring, a_foot = FOOT.get(leg, (None, a_end, a_end))
    # Where the leg's root lies off the rig's hip (the body lifted above it): carried by the laid
    # leg from its root and faded out by the knee, so the knee and all below it are the rig's.
    root_off = line[0] - at(Q, a_s, a_of(0.0))
    for v, s_v in zip(lg["members"], lg["s"]):
        u = nearest_on_line(V[v], line, u_line, s_v)
        local = orthonormal(at(F_new, u_line, u)) @ (V[v] - at(line, u_line, u))
        a = a_of(u)
        laid = at(Q, a_s, a) + orthonormal(at(F_old, a_s, a)).T @ local + (1 - smoothstep(0.0, knee_new, u)) * root_off
        V[v] = V[v] + smoothstep(0.0, fade, u) * (laid - V[v])
        w2, w3 = smoothstep(cum[1] - h1, cum[1] + h1, a), smoothstep(cum[2] - h2, cum[2] + h2, a)
        # The root's share goes to the body's part there (never `Body`, which the old skin
        # leaves unweighted and which moves apart from the parts in Death).
        share = 1 - smoothstep(0.0, fade, u)
        wf = smoothstep(a_foot - foot_span * (a_foot - a_ring), a_foot, a) if foot else 0.0
        weights[v] = {b[0]: (1 - w2) * (1 - share), b[1]: (w2 - w3) * (1 - share), b[2]: w3 * (1 - wf) * (1 - share)}
        if foot:
            weights[v][foot] = wf * (1 - share)
        if share > 0:
            ROOT_SHARE[v] = share
    report["legs"][leg] = {
        "vertices": int(len(lg["members"])),
        "length_old_m": round(float(cum[-1]) * GAME_SCALE, 3),
        "length_new_m": round(float(along(line)[-1]) * GAME_SCALE, 3),
        "knee_at": round(float(knee_new), 2),
    }


# How far along the leg (its share of the leg's length) the root hands over from the body to the
# leg, in place and in weight. The hip joints lie under the body (it is lifted above them), so a
# leg turning about its hip swings its root off the body: a short hand-over tears the seam there.
# Chosen by the pose search below with the body's parts.
ROOT_FADES = (0.12, 0.3, 0.4)
V0 = V.copy()


def lay_all(fade, foot_span=1.0):
    V[:] = V0
    ROOT_SHARE.clear()
    for i, leg in enumerate(LEGS):
        lay(leg, legs[best[i]], fade, foot_span)


lay_all(ROOT_FADES[0])
print("legs laid", json.dumps(report["legs"]))

# The body's weights: its parts', and each leg root's share of them. Where the waist and the neck
# lie and how wide they blend is chosen as the body's scale was, by posing the skin through every
# clip: Death turns the abdomen and the head about the thorax, and a blend too narrow, or a waist
# away from the joint it turns about, tears the trunk there. Each choice is measured as
# creature_check measures the trunk (the skin on its bones between the legs' roots, and the
# belly): how far its edges stretch, and how low it goes in Death. The waist is tried at the
# body's narrowest and at the Abdomen and Thorax joints, the neck at the old one's place and the
# Head joint.
bones = sorted({n for w in weights for n in w} | {"Abdomen", "Thorax", "Head"})


def assemble(blend, waist_at, neck_at, root_to="parts"):
    """Every vertex's weights for one choice: smoothed over the skin twice, at most four bones a
    vertex, normalised. A leg root's share of the body goes to the body's parts where it lies
    (`parts`) or all to one bone (`root_to`)."""
    rows = []
    for i in range(len(V)):
        part = part_weights(U[i], blend, waist_at, neck_at) if rest[i] or i in ROOT_SHARE else {}
        w = part if rest[i] else dict(weights[i])
        on_root = part if root_to == "parts" else {root_to: 1.0}
        for n, x in (on_root.items() if i in ROOT_SHARE else ()):
            w[n] = w.get(n, 0.0) + ROOT_SHARE[i] * x
        rows.append(w)
    Wc = np.array([[w.get(n, 0.0) for n in bones] for w in rows])
    Wc /= Wc.sum(1)[:, None]
    for _ in range(2):
        Wc = 0.5 * Wc + 0.5 * np.array([Wc[nb].mean(0) if nb else Wc[i] for i, nb in enumerate(NEIGH)])
    for i in range(len(Wc)):
        Wc[i, np.argsort(Wc[i])[:-4]] = 0.0
    return Wc / Wc.sum(1)[:, None]


# creature_check's trunk, as it finds it: its legs are the sided bones ending below the spine, its
# roots those whose parent is no leg (the hips, or the feet, which hang from Root), and the trunk
# lies between the roots' front and back halves (y forward, x across).
TRUNK = ["Abdomen", "Thorax"]
spine_z = float(np.mean([JOINT[n][2] for n in TRUNK]))
RW = rig.matrix_world
cc_legs = [b for b in bones if b.endswith((".L", ".R")) and (RW @ rig.data.bones[b].tail_local).z < spine_z - 0.05 * float(np.ptp(V[:, 2]))]
cc_roots = [b for b in cc_legs if not (rig.data.bones[b].parent and rig.data.bones[b].parent.name in cc_legs)]
root_at = np.array([(RW @ rig.data.bones[b].head_local)[:] for b in cc_roots])
mid_y = (root_at[:, 1].min() + root_at[:, 1].max()) / 2
ya, yb = sorted((root_at[root_at[:, 1] < mid_y, 1].mean(), root_at[root_at[:, 1] >= mid_y, 1].mean()))
fw = np.array(fwd_old[:])
me.calc_loop_triangles()
TRIS = np.array([t.vertices[:] for t in me.loop_triangles], int).reshape(-1, 3)
HINGE_PAIRS, HINGE_EDGES = ms.hinges(TRIS)
LIMB = [bones.index(b) for b in cc_legs]
FOOTB = [bones.index(f[0]) for f in FOOT.values()]
BODYB = [bones.index(b) for b in ("Abdomen", "Thorax", "Head")]
HIP_PARENTS = sorted({(rig.data.bones[chain(leg)[0]].parent or rig.data.bones[chain(leg)[0]]).name for leg in LEGS})
print("creature_check's legs", cc_legs, "roots", cc_roots, "hips' parents", HIP_PARENTS)


def measure_setup():
    """What the measures read of the skin at rest, again after the legs are laid anew."""
    global inner, belly, el0, Ph
    inner = (V[:, 1] > ya + 0.15 * (yb - ya)) & (V[:, 1] < yb - 0.15 * (yb - ya))
    sel = inner & (np.abs(V[:, 0]) < 0.5 * float(np.abs(root_at[:, 0]).min()))
    low_z = float(V[sel, 2].min())
    belly = sel & (V[:, 2] < low_z + 0.35 * (spine_z - low_z))
    el0 = np.linalg.norm(V[EDGES[:, 0]] - V[EDGES[:, 1]], axis=1)
    Ph = np.c_[V, np.ones(len(V))]


measure_setup()


def seam_measures(Wc):
    """Per clip, as creature_check measures the seam where the legs' skin meets the body's (skin
    10-90% on the legs' bones): its edges' worst p95 stretch, and the 99th percentile of how much
    further its hinges fold than at rest (degrees; slivers left out)."""
    on_limb = Wc[:, LIMB].sum(1)
    band = (on_limb > 0.1) & (on_limb < 0.9)
    area = np.linalg.norm(np.cross(V[TRIS[:, 1]] - V[TRIS[:, 0]], V[TRIS[:, 2]] - V[TRIS[:, 0]]), axis=1)
    inb = band[HINGE_EDGES[:, 0]] | band[HINGE_EDGES[:, 1]]
    big = area > 0.2 * np.median(area[HINGE_PAIRS[inb].ravel()]) if inb.any() else area > 0
    pairs = HINGE_PAIRS[inb & big[HINGE_PAIRS[:, 0]] & big[HINGE_PAIRS[:, 1]]]
    fold0 = ms.folds(V, TRIS, pairs)
    se = (el0 > 1e-7) & (band[EDGES[:, 0]] | band[EDGES[:, 1]])
    se &= el0 > 0.4 * np.median(el0[se]) if se.any() else se
    # Where along the leg each seam edge lies: at the foot's hand-over, at the root (on the body's
    # bones), or at a joint of the leg (creature_check counts the hip bone and the feet as the
    # legs, so the hip's hand-over to the next bone is a seam to it too).
    on_foot = Wc[:, FOOTB].sum(1)
    on_body = Wc[:, BODYB].sum(1)
    ea, eb = EDGES[se, 0], EDGES[se, 1]
    region = np.where(np.maximum(on_foot[ea], on_foot[eb]) > 0.05, 0, np.where(np.maximum(on_body[ea], on_body[eb]) > 0.05, 1, 2))
    used = [k for k in range(len(bones)) if Wc[:, k].any()]
    out = {}
    for clip, _f, M, _old in poses:
        P = np.einsum("vk,kij,vj->vi", Wc[:, used], np.array([M[bones[k]] for k in used]), Ph)[:, :3]
        el = np.linalg.norm(P[ea] - P[eb], axis=1)
        o = out.setdefault(clip, [0.0, 0.0, 1e9, [0.0, 0.0, 0.0]])
        o[0] = max(o[0], ms.stretch(el0[se], el))
        o[1] = max(o[1], ms.crease(fold0, ms.folds(P, TRIS, pairs)) if len(pairs) else 0.0)
        o[2] = min(o[2], float(P[:, 2].min()) * GAME_SCALE * 100)
        for r in range(3):
            if (region == r).any():
                o[3][r] = max(o[3][r], ms.stretch(el0[se][region == r], el[region == r]))
    for o in out.values():
        o[3] = {"foot": round(o[3][0], 3), "root": round(o[3][1], 3), "joint": round(o[3][2], 3), "edges": [int((region == r).sum()) for r in range(3)]}
    return out


def trunk_measures(Wc):
    """Per clip: the trunk's edges' worst p95 stretch, and its lowest point (cm)."""
    skin = belly | (inner & np.isin(np.array(bones)[np.argmax(Wc, 1)], TRUNK))
    te = (el0 > 1e-7) & skin[EDGES[:, 0]] & skin[EDGES[:, 1]]
    idx = np.nonzero(skin)[0]
    used = [k for k in range(len(bones)) if Wc[idx, k].any()]
    out = {}
    for clip, _f, M, _old in poses:
        P = np.zeros((len(V), 3))
        P[idx] = sum(Wc[idx, k, None] * (Ph[idx] @ M[bones[k]].T)[:, :3] for k in used)
        el = np.linalg.norm(P[EDGES[te, 0]] - P[EDGES[te, 1]], axis=1)
        o = out.setdefault(clip, [0.0, 1e9])
        o[0] = max(o[0], float(np.percentile(np.abs(el / el0[te] - 1), 95)) if te.any() else 0.0)
        o[1] = min(o[1], float(P[idx, 2].min()) * GAME_SCALE * 100 if len(idx) else 0.0)
    return out


def on_trunk(Wc):
    """The share of the body's vertices whose strongest bone is each of the trunk's."""
    dom_new = np.array(bones)[np.argmax(Wc[rest], 1)]
    return {n: float(np.mean(dom_new == n)) for n in TRUNK}


# The joints' places in U's measure: where the parts turn about.
lin = np.polyfit(V[rest] @ fw, U[rest], 1)
joint_u = {n: float(np.polyval(lin, np.array(JOINT[n][:]) @ fw)) for n in ("Abdomen", "Thorax", "Head")}
old_share = {n: float(np.mean(OLD_DOM[old_body] == n)) for n in TRUNK}
choices = {}
WAISTS = sorted({0.0} | {round(joint_u[n], 3) for n in ("Abdomen", "Thorax")})
NECKS = sorted({round(NECK * front_new, 3), round(joint_u["Head"], 3)})


def trunk_score(m, sh):
    """Against the bars (45% stretch, 25% in the walk; Death's trunk no lower than 2.5 cm, inside
    its 3.2 cm bar); a choice that leaves the trunk bones' skin (a vacuous pass) is out."""
    score = max(v[0] / (0.25 if c == "Spider_Walk" else 0.45) for c, v in m.items()) + max(0.0, -2.5 - m["Spider_Death"][1])
    return score + (10.0 if any(sh[n] < 0.5 * old_share[n] for n in TRUNK) else 0.0)


for blend, waist_at, neck_at in itertools.product((0.08, 0.12, 0.16, 0.2, 0.25, 0.3), WAISTS, NECKS):
    Wc = assemble(blend, waist_at, neck_at)
    m, sh = trunk_measures(Wc), on_trunk(Wc)
    choices[(blend, waist_at, neck_at)] = (trunk_score(m, sh), m, sh)
pick = min(choices, key=lambda k: choices[k][0])
report["body_joints_u"] = {n: round(u, 3) for n, u in joint_u.items()}
report["body_choices"] = [
    {"blend": k[0], "waist": k[1], "neck": k[2], "score": round(s, 3), "trunk": {c: [round(v[0], 3), round(v[1], 1)] for c, v in m.items()}, "on": {n: round(x, 3) for n, x in sh.items()}}
    for k, (s, m, sh) in sorted(choices.items(), key=lambda kv: kv[1][0])
]
report["body_pick"] = {"blend": pick[0], "waist": pick[1], "neck": pick[2]}
print("body joints along it", json.dumps(report["body_joints_u"]), "front", round(front_new, 3))
for row in report["body_choices"]:
    print("body choice", json.dumps(row))
print("body pick", json.dumps(report["body_pick"]))

# The leg roots, chosen on the best bodies: how far along the leg they hand over, and to which of
# the body's bones (its parts where they lie, the cephalothorax the legs grow from, or the hips'
# parent). Scored on the trunk as above and on the seam: its worst and mean stretch over the clips
# and its worst crease, so a tear in one clip and a fold in all count.
ROOT_TO = ["parts", "Head"] + [n for n in HIP_PARENTS if n in ("Abdomen", "Thorax")]
# The foot's hand-over is tried shorter too (the share of the old's span it takes, ending where the
# old's foot begins), with the skin's lowest point held to creature_check's bars: 1 cm under the
# floor standing, 3.2 cm moving (Death lies down and is not held).
FOOT_SPANS = (1.0, 0.4, 0.15)
root_choices = {}
for fade, foot_span in itertools.product(ROOT_FADES, FOOT_SPANS):
    lay_all(fade, foot_span)
    measure_setup()
    for body_k in sorted(choices, key=lambda k: choices[k][0])[:2]:
        for to in ROOT_TO:
            Wc = assemble(*body_k, root_to=to)
            m, sh, sm = trunk_measures(Wc), on_trunk(Wc), seam_measures(Wc)
            seam_s = max(v[0] for v in sm.values()) + float(np.mean([v[0] for v in sm.values()])) + max(v[1] for v in sm.values()) / 180
            below = sum(max(0.0, -(1.0 if c == "Spider_Idle" else 3.2) - v[2]) for c, v in sm.items() if c != "Spider_Death")
            root_choices[(fade, foot_span, to) + body_k] = (trunk_score(m, sh) + seam_s + below, m, sm)
root_pick = min(root_choices, key=lambda k: root_choices[k][0])
report["root_choices"] = [
    {"fade": k[0], "foot_span": k[1], "to": k[2], "body": k[3:], "score": round(s, 3), "trunk": {c: round(v[0], 3) for c, v in m.items()},
     "seam": {c: [round(v[0], 3), round(v[1], 1), round(v[2], 1), v[3]] for c, v in sm.items()}}
    for k, (s, m, sm) in sorted(root_choices.items(), key=lambda kv: kv[1][0])
]
report["root_pick"] = {"fade": root_pick[0], "foot_span": root_pick[1], "to": root_pick[2], "body": root_pick[3:]}
for row in report["root_choices"]:
    print("root choice", json.dumps(row))
print("root pick", json.dumps(report["root_pick"]))
lay_all(*root_pick[:2])
W = assemble(*root_pick[3:], root_to=root_pick[2])
for v, co in zip(me.vertices, V):
    v.co = co
me.update()
groups = {n: body.vertex_groups.new(name=n) for n in bones}
for i in range(len(W)):
    for j in np.nonzero(W[i] > 1e-4)[0]:
        groups[bones[j]].add([i], float(W[i, j]), "REPLACE")
own = sum(1 for i, leg in enumerate(LEGS) for v in legs[best[i]]["members"] if bones[int(np.argmax(W[v]))] in chain(leg) + ["Body", FOOT.get(leg, ("",))[0]])
report["leg_vertices_on_own_chain"] = round(own / sum(len(lg["members"]) for lg in legs), 4)
# The trunk guard: the body's vertices on each body bone, against the old body's. A body whose
# weights left Abdomen and Thorax (all on Head, say) passes creature_check's trunk measures
# vacuously, as they count only the trunk bones' skin.
new_dom = np.array([bones[int(np.argmax(W[i]))] for i in np.nonzero(rest)[0]])
report["body_shares"] = {n: [round(float(np.mean(new_dom == n)), 3), round(float(np.mean(OLD_DOM[old_body] == n)), 3)] for n in BODY_BONES}
print("body vertices by bone, new against old", json.dumps(report["body_shares"]))
for n in ("Abdomen", "Thorax"):
    new_share, old_share = report["body_shares"][n]
    if new_share < 0.5 * old_share:
        sys.exit(f"only {new_share:.0%} of the body's vertices are on {n}, against the old body's {old_share:.0%}")
report["body_lowest_m"] = round(float(V[rest, 2].min() * GAME_SCALE), 3)
report["rest_lowest_m"] = round(float(V[:, 2].min() * GAME_SCALE), 3)
print("leg vertices weighted to their own chain", report["leg_vertices_on_own_chain"], "body lowest", report["body_lowest_m"], "m, lowest", report["rest_lowest_m"], "m (from the origin)")

for o in old_meshes:
    bpy.data.objects.remove(o)
# The skin in the armature's own space, its object on the armature's, so the exporter's inverse
# bind matrices and the vertices it writes are in one frame.
print("rig matrix_world", [round(x, 4) for row in rig.matrix_world for x in row])
body.data.transform(rig.matrix_world.inverted())
body.parent = rig
body.matrix_parent_inverse = Matrix.Identity(4)
body.matrix_basis = Matrix.Identity(4)
body.modifiers.new("Armature", "ARMATURE").object = rig
select_only(body)
bpy.ops.object.shade_smooth_by_angle(angle=math.radians(50.0), keep_sharp_edges=True)

# Textures at its budget (art-style, Budgets; D304): colour and normal, roughness-metal at half.
report["textures"] = {}
for image in bpy.data.images:
    kind = "normal" if "normal" in image.name.lower() else "roughness_metal" if image.name.upper().startswith("ORM") else "colour"
    w, h = image.size
    most = int(TEX[kind])
    if max(w, h) > most:
        image.scale(w * most // max(w, h), h * most // max(w, h))
    report["textures"][kind] = list(image.size)
print("textures", report["textures"])

for act in bpy.data.actions:
    name = next((c for c in CLIPS if c.split("|")[-1] in act.name), None)
    if name:
        act.name = name
print("clips", [a.name for a in bpy.data.actions])
rig.data.pose_position = "POSE"
if rig.animation_data:
    rig.animation_data.action = None
for pb in rig.pose.bones:
    pb.matrix_basis.identity()
for o in scene.objects:
    o.select_set(o in (body, rig))
report["triangles"] = sum(len(p.vertices) - 2 for p in me.polygons)
bpy.ops.export_scene.gltf(
    filepath=out,
    export_format="GLB",
    use_selection=True,
    export_tangents=True,
    export_image_format="JPEG",
    export_jpeg_quality=90,
    export_animations=True,
    export_animation_mode="ACTIONS",
    export_force_sampling=True,
    export_influence_nb=4,
)
print("EXPORTED", out, os.path.getsize(out))


# The check: both files imported again and every clip sampled: how far the joints moved from the
# rig's own, and how low the skin reaches (its feet on the ground) through each clip.
def sampled(path):
    look.reset()
    bpy.ops.import_scene.gltf(filepath=path)
    r = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
    skins = [o for o in bpy.context.scene.objects if o.type == "MESH" and any(m.type == "ARMATURE" for m in o.modifiers)]
    rest = r.data.pose_position
    r.data.pose_position = "REST"
    bpy.context.view_layer.update()
    dgs = bpy.context.evaluated_depsgraph_get()
    rest_z = {}
    for o in skins:
        ev = o.evaluated_get(dgs)
        mm = ev.to_mesh()
        rest_z[o.name] = [(ev.matrix_world @ v.co).z for v in mm.vertices]
        print("REST", os.path.basename(path), o.name, "lowest", round(min(rest_z[o.name]) * GAME_SCALE, 3), "m")
        ev.to_mesh_clear()
    r.data.pose_position = rest
    r.animation_data_create()
    got = {}
    for act in bpy.data.actions:
        r.animation_data.action = act
        if act.slots:
            r.animation_data.action_slot = act.slots[0]
        # A bone a clip does not key keeps whatever pose the last clip left it in, so each clip
        # starts from the rest, as on a model that has just been placed.
        for pb in r.pose.bones:
            pb.matrix_basis = Matrix()
        f0, f1 = (int(x) for x in act.frame_range)
        rows = []
        for f in range(f0, f1 + 1, 2):
            bpy.context.scene.frame_set(f)
            dgs = bpy.context.evaluated_depsgraph_get()
            low = (1e9, "")
            for o in skins:
                ev = o.evaluated_get(dgs)
                mm = ev.to_mesh()
                z, i = min(((ev.matrix_world @ v.co).z, i) for i, v in enumerate(mm.vertices))
                # Which bone holds the lowest point (a leg's or the body's) and how high it is at rest.
                gs = o.data.vertices[i].groups
                bone = o.vertex_groups[max(gs, key=lambda g: g.weight).group].name if len(gs) else ""
                low = min(low, (z, f"{bone} (rest {rest_z[o.name][i] * GAME_SCALE:.3f} m)"))
                ev.to_mesh_clear()
            rows.append(({pb.name: (r.matrix_world @ pb.head).copy() for pb in r.pose.bones}, low[0], low[1]))
        got[act.name] = rows
    return got


def lows(rows):
    """A clip's lowest skin over its frames, lowest and highest, and the bone under the lowest."""
    return [round(min(x[1] for x in rows) * GAME_SCALE, 3), round(max(x[1] for x in rows) * GAME_SCALE, 3), min(rows, key=lambda x: x[1])[2]]


old_s, new_s = sampled(RIG), sampled(out)
report["clips"] = {}
for name in CLIPS:
    if name not in new_s:
        report["clips"][name] = "missing"
        continue
    moved, worst = max(((a[0][b] - c[0][b]).length, b) for a, c in zip(old_s[name], new_s[name]) for b in a[0] if b in c[0])
    report["clips"][name] = {"joint_drift_mm": round(moved * GAME_SCALE * 1000, 2), "worst_joint": worst, "frames": [len(old_s[name]), len(new_s[name])],"lowest_skin_old_m": lows(old_s[name]), "lowest_skin_new_m": lows(new_s[name])}
print("CLIPS", json.dumps(report["clips"]))
print("TRIANGLES", report["triangles"])
if report_path:
    with open(report_path, "w") as fh:
        json.dump(report, fh, indent=1)
