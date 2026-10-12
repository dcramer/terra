"""Fits MakeHuman's body onto one of our bodies, so that body becomes a shape of the shared
mesh (the spike's README).

    tools/blender/run.sh spike/shared_mesh/fit.py assets/models/character/base-male.glb base-male

1. Poses the shared mesh like the body: each limb of MakeHuman's skeleton turned along the
   matching bone of ours, skinned with MakeHuman's weights.
2. Mixes MakeHuman's own shapes (build, sex, height, proportions) to come as close as they can.
3. Fits what is left with a smooth residual: each vertex drawn to the body's skin, the pull
   smoothed over the mesh so the shared mesh keeps its own clean form and hands.
4. Bakes the body's texture onto the shared mesh's layout.
5. Writes it rigged with MakeHuman's game skeleton and weights, its joints found again from
   the fitted skin, turned to face +X as Tripo's rigs do, so tools/characters/finish_character.sh
   takes it as it takes a Tripo rig. Also writes the fitted vertices, for the gear steps.
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(HERE, "..", "..", "tools", "characters"))

import bpy
import numpy as np
from mathutils import Matrix, Quaternion, Vector
from mathutils.bvhtree import BVHTree

import look
import mh

OUT = os.path.join(HERE, "out")
src, name = look.args()[:2]
# The shared mesh's skin weights, by vertex, once made (from the first body); without them
# they are copied from this body's own rig and written for the others to use.
WEIGHTS = next((a.split("=", 1)[1] for a in look.args() if a.startswith("--weights=")), None)
# For looking at the steps: "pose" stops after posing, "shapes" after the shapes' mix.
STOP = next((a for a in look.args()[2:] if not a.startswith("--")), "")

# MakeHuman's bone -> (our bone it lies along, from its head to the head of this one).
ALONG = {
    "pelvis": ("DEF-hips", "DEF-spine.001"),
    "spine_01": ("DEF-spine.001", "DEF-spine.002"),
    "spine_02": ("DEF-spine.002", "DEF-spine.003"),
    "spine_03": ("DEF-spine.003", "DEF-neck"),
    "neck_01": ("DEF-neck", "DEF-head"),
}
# ... and the MakeHuman bone whose head ends the direction on its side.
CHILD = {"pelvis": "spine_01", "spine_01": "spine_02", "spine_02": "spine_03", "spine_03": "neck_01", "neck_01": "head"}
for mh_s, s in (("l", "L"), ("r", "R")):
    for a, b, ours_a, ours_b in (
        ("clavicle", "upperarm", "shoulder", "upper_arm"),
        ("upperarm", "lowerarm", "upper_arm", "forearm"),
        ("lowerarm", "hand", "forearm", "hand"),
        ("hand", "middle_01", "hand", "f_middle.01"),
        ("thigh", "calf", "thigh", "shin"),
        ("calf", "foot", "shin", "foot"),
        ("foot", "ball", "foot", "toe"),
    ):
        ALONG[f"{a}_{mh_s}"] = (f"DEF-{ours_a}.{s}", f"DEF-{ours_b}.{s}")
        CHILD[f"{a}_{mh_s}"] = f"{b}_{mh_s}"

look.reset()

# The body to fit, at rest.
bpy.ops.import_scene.gltf(filepath=os.path.join(look.ROOT, src))
arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
if arm.animation_data:
    arm.animation_data.action = None
for pb in arm.pose.bones:
    pb.matrix_basis.identity()
bpy.context.view_layer.update()
target = max((o for o in bpy.context.scene.objects if o.type == "MESH"), key=lambda o: len(o.data.vertices))
tmw = target.matrix_world
t_pts = [tmw @ v.co for v in target.data.vertices]
tree = BVHTree.FromPolygons(t_pts, [tuple(p.vertices) for p in target.data.polygons])
ours = {b.name: np.array(arm.matrix_world @ b.head_local) for b in arm.data.bones}
t_z = np.array([p.z for p in t_pts])
t_height = t_z.max() - t_z.min()

# The shared mesh and its data.
H, faces, uvs, groups = mh.base()
rig = mh.rig()
W = mh.weights()
names, B = mh.basis()
drawn = sorted({i for vi, _ in faces for i in vi})
eyes = [i for i in drawn if i >= mh.BODY]
print("FIT", name, "shared mesh", len(drawn), "vertices,", len(faces), "faces,", len(names), "shapes")

# Scaled to the body's height, centred on its hips.
h_body = H[: mh.BODY]
s = t_height / (h_body[:, 2].max() - h_body[:, 2].min())
H = H * s
B = B * s

# Skin weights over every vertex we draw (the eyes go with the head), normalised.
bones = list(rig)
bi = {b: i for i, b in enumerate(bones)}
Wm = np.zeros((len(H), len(bones)))
for b, (idx, wt) in W.items():
    Wm[idx, bi[b]] = wt
Wm[eyes] = 0
Wm[eyes, bi["head"]] = 1
Wm = Wm / np.maximum(Wm.sum(axis=1, keepdims=True), 1e-9)


def depth(b):
    n = 0
    while rig[b]["parent"]:
        b = rig[b]["parent"]
        n += 1
    return n


order = sorted(bones, key=depth)


def joints(V):
    return {b: mh.point(V, groups, rig[b]["head"]) for b in bones}, {b: mh.point(V, groups, rig[b]["tail"]) for b in bones}


def rotation_between(a, b):
    a = Vector(a).normalized()
    b = Vector(b).normalized()
    return np.array(a.rotation_difference(b).to_matrix())


# 1. The pose that lays each MakeHuman bone along ours, as a world rotation per bone.
J0, _ = joints(H)
R = {}
for b in order:
    parent = rig[b]["parent"]
    Rp = R[parent] if parent else np.eye(3)
    if b in ALONG and all(x in ours for x in ALONG[b]):
        d = Rp @ (J0[CHILD[b]] - J0[b])
        want = ours[ALONG[b][1]] - ours[ALONG[b][0]]
        R[b] = rotation_between(d, want) @ Rp
    else:
        R[b] = Rp


def pose(V):
    """V (every vertex, at rest in MakeHuman's A-pose) skinned into the pose above."""
    J, _ = joints(V)
    P = {}
    for b in order:
        parent = rig[b]["parent"]
        P[b] = J[b] if parent is None else P[parent] + R[parent] @ (J[b] - J[parent])
    out = np.zeros_like(V)
    for b in bones:
        w = Wm[:, bi[b]]
        live = w > 0
        if live.any():
            out[live] += w[live, None] * (P[b] + (V[live] - J[b]) @ R[b].T)
    return out, P


P0, posed_joints = pose(H)
D = np.stack([pose(H + B[k])[0] - P0 for k in range(len(B))])
# Hips onto the body's hips.
shift = ours["DEF-hips"] - posed_joints["pelvis"]
P0 = P0 + shift
for b in posed_joints:
    posed_joints[b] = posed_joints[b] + shift

idx = np.array(drawn)
body_idx = idx[idx < mh.BODY]

# Neighbours over the drawn mesh, for smoothing.
edges = set()
for vi, _ in faces:
    for a, b in zip(vi, vi[1:] + vi[:1]):
        edges.add((min(a, b), max(a, b)))
edges = np.array(sorted(edges))
deg = np.zeros(len(H))
np.add.at(deg, edges[:, 0], 1)
np.add.at(deg, edges[:, 1], 1)


def smooth(X, times):
    for _ in range(times):
        acc = np.zeros_like(X)
        np.add.at(acc, edges[:, 0], X[edges[:, 1]])
        np.add.at(acc, edges[:, 1], X[edges[:, 0]])
        X = 0.5 * X + 0.5 * acc / np.maximum(deg, 1)[:, None]
    return X


def normals(V):
    n = np.zeros_like(V)
    for vi, _ in faces:
        a, b, c = V[vi[0]], V[vi[1]], V[vi[2]]
        fn = np.cross(b - a, c - a)
        for i in vi:
            n[i] += fn
    return n / np.maximum(np.linalg.norm(n, axis=1, keepdims=True), 1e-12)


def nearest(V, reach):
    """For each body vertex: the nearest point on the body's skin, and whether to trust it
    (near enough, and facing the same way)."""
    N = normals(V)
    C = V.copy()
    ok = np.zeros(len(V), dtype=bool)
    for i in body_idx:
        hit = tree.find_nearest(Vector(V[i]), reach)
        if hit[0] is None:
            continue
        C[i] = hit[0]
        ok[i] = np.dot(np.array(hit[1]), N[i]) > 0.5
    return C, ok


# 2. MakeHuman's own shapes, mixed by least squares to the body's skin.
a = np.zeros(len(B))
X = P0.copy()
for it in range(0 if STOP == "pose" else 8):
    C, ok = nearest(X, 0.15)
    rows = body_idx[ok[body_idx]]
    A = D[:, rows, :].reshape(len(B), -1).T
    r = (C[rows] - P0[rows]).reshape(-1)
    AtA = A.T @ A
    lam = 1e-3 * np.trace(AtA) / len(B)
    a = np.linalg.solve(AtA + lam * np.eye(len(B)), A.T @ r)
    X = P0 + np.tensordot(a, D, axes=1)
    err = np.linalg.norm(C[rows] - X[rows], axis=1)
    print(f"FIT shapes {it} mean {err.mean() * 100:.2f} cm, 95% {np.percentile(err, 95) * 100:.2f} cm")
print("FIT mix", {n.split("/")[-1].replace(".target.gz", ""): round(float(x), 2) for n, x in zip(names, a) if abs(x) > 0.05})

# The head: MakeHuman's own head and face shapes, mixed the same way on the head alone.
face = smooth(Wm[:, [bi["head"]]], 6)[:, 0]
head_names, HB = mh.head_basis()
posed_H = pose(H)[0]
HD = np.stack([pose(H + s * HB[k])[0] - posed_H for k in range(len(HB))])
head_rows = body_idx[face[body_idx] > 0.5]
base_X = X.copy()
h = np.zeros(len(HB))
for it in range(0 if STOP == "pose" else 6):
    C, ok = nearest(X, 0.05)
    rows = head_rows[ok[head_rows] & (np.linalg.norm(C[head_rows] - X[head_rows], axis=1) < 0.03)]
    A = HD[:, rows, :].reshape(len(HB), -1).T
    r = (C[rows] - base_X[rows]).reshape(-1)
    AtA = A.T @ A
    h = np.clip(np.linalg.solve(AtA + 1e-2 * np.trace(AtA) / len(HB) * np.eye(len(HB)), A.T @ r), -1, 1)
    X = base_X + np.tensordot(h, HD, axes=1)
    err = np.linalg.norm(C[rows] - X[rows], axis=1)
    print(f"FIT head {it} mean {err.mean() * 1000:.1f} mm, 95% {np.percentile(err, 95) * 1000:.1f} mm")
print("FIT head mix", {n.split("/")[-1].replace(".target.gz", ""): round(float(x), 2) for n, x in zip(head_names, h) if abs(x) > 0.1})

# 3. The residual below the head, smooth, sparing the hands (the shared mesh's own hands are
# better than a generated body's) and carrying the eyes with the skin round them. Smoothed by
# distance, not by mesh steps, so dense and sparse parts of the mesh fit alike, from broad to
# a centimetre and a half, never so fine it copies a crease of the generated body's clothes.
hand = np.zeros(len(H))
for b in bones:
    if b.startswith(("hand_", "thumb_", "index_", "middle_", "ring_", "pinky_")):
        hand += Wm[:, bi[b]]
keep = np.clip(1.0 - hand, 0.0, 1.0) * (1.0 - face)
keep = smooth(keep[:, None], 4)[:, 0]
eye_ring = {}
for e in eyes:
    d = np.linalg.norm(H[body_idx] - H[e], axis=1)
    eye_ring[e] = body_idx[np.argsort(d)[:30]]

from mathutils.kdtree import KDTree


def kernel(V, sigma):
    """Gaussian weights between drawn vertices within three sigma: (rows, cols, weights)."""
    kd = KDTree(len(idx))
    for n, v in enumerate(idx):
        kd.insert(Vector(V[v]), n)
    kd.balance()
    rows, cols, ws = [], [], []
    for n, v in enumerate(idx):
        for _, m, dist in kd.find_range(Vector(V[v]), 3 * sigma):
            rows.append(v)
            cols.append(idx[m])
            ws.append(math.exp(-0.5 * (dist / sigma) ** 2))
    return np.array(rows), np.array(cols), np.array(ws)


def spread(K, pull, trust):
    rows, cols, ws = K
    num = np.zeros_like(pull)
    den = np.zeros(len(pull))
    np.add.at(num, rows, (ws * trust[cols])[:, None] * pull[cols])
    np.add.at(den, rows, ws * trust[cols])
    return num / np.maximum(den, 1e-9)[:, None]


for sigma, times in () if STOP else ((0.05, 8), (0.03, 8), (0.02, 8), (0.015, 8)):
    K = kernel(X, sigma)
    for it in range(times):
        C, ok = nearest(X, 0.06)
        pull = spread(K, C - X, ok.astype(float)) * keep[:, None]
        for e, ring in eye_ring.items():
            pull[e] = pull[ring].mean(axis=0)
        X = X + 0.7 * pull
# Left and right alike: a body is made symmetric, as the shared mesh is; a generated body's
# small differences between its sides are noise, not design.
m = mh.mirror()
flip = np.array([-1.0, 1.0, 1.0])
X = 0.5 * (X + X[m] * flip)
C, ok = nearest(X, 0.1)
err = np.linalg.norm(C[body_idx] - X[body_idx], axis=1)
print(f"FIT residual mean {err.mean() * 1000:.1f} mm, 95% {np.percentile(err, 95) * 1000:.1f} mm, max {err.max() * 1000:.1f} mm")

# The joints, found again from the fitted skin: each written once as a mix of the skin round
# it on the posed base, then read off the fitted skin.
fitted_joints = {}
for b in bones:
    ring, w = mh.regressor(P0, posed_joints[b])
    fitted_joints[b] = w @ X[ring]

# The skin weights, on our own skeleton's bones, by vertex of the shared mesh: the same for
# every body. Copied once from a body whose rig passes our checks (tools/characters), each
# vertex taking the weights of the skin it lies on; the forearm's twist is left to the export.
if WEIGHTS:
    w = np.load(os.path.join(look.ROOT, WEIGHTS), allow_pickle=True)
    skin_bones = [str(b) for b in w["bones"]]
    skin = dict(zip(w["drawn"].tolist(), w["weights"]))
    joint_rings = w["joints"].item()
else:
    from mathutils.interpolate import poly_3d_calc

    groups_t = {g.index: g.name.replace(".twist", "") if ".twist" in g.name else g.name for g in target.vertex_groups}
    skin_bones = sorted(set(groups_t.values()))
    col = {b: i for i, b in enumerate(skin_bones)}
    tw = np.zeros((len(target.data.vertices), len(skin_bones)), dtype=np.float32)
    for v in target.data.vertices:
        for g in v.groups:
            tw[v.index, col[groups_t[g.group]]] += g.weight
    skin = {}
    for v in drawn:
        if v in eyes:
            row = np.zeros(len(skin_bones), dtype=np.float32)
            row[col["DEF-head"]] = 1
        else:
            hit = tree.find_nearest(Vector(X[v]))
            poly = target.data.polygons[hit[2]]
            corners = [t_pts[i] for i in poly.vertices]
            share = poly_3d_calc(corners, hit[0])
            row = sum(f * tw[i] for f, i in zip(share, poly.vertices))
            # The hands are the shared mesh's own, not the body's, so they keep MakeHuman's.
            if hand[v] > 0:
                own = np.zeros(len(skin_bones), dtype=np.float32)
                for mb, ob in mh.OURS.items():
                    own[col[ob]] += Wm[v, bi[mb]]
                row = row / max(row.sum(), 1e-9) * (1 - hand[v]) + own * hand[v]
        skin[v] = row / max(row.sum(), 1e-9)
    # The joints of the same rig, each written as a mix of the fitted skin round it, so every
    # body finds them on its own skin: weights and joints are made for each other.
    mh_hand = {o: m for m, o in mh.OURS.items() if m.startswith(("hand_", "thumb_", "index_", "middle_", "ring_", "pinky_"))}
    joint_rings = {b: mh.regressor(X, fitted_joints[mh_hand[b]] if b in mh_hand else p)
                   for b, p in ours.items() if b.startswith("DEF-") and ".twist" not in b}
    np.savez(os.path.join(OUT, f"{name}-weights.npz"), drawn=np.array(drawn), bones=np.array(skin_bones),
             weights=np.stack([skin[v] for v in drawn]), joints=np.array(joint_rings, dtype=object))
    print("FIT weights copied from", src, len(skin_bones), "bones")

# 4. The mesh, with the shared layout.
remap = {v: i for i, v in enumerate(drawn)}
mesh = bpy.data.meshes.new(f"{name}-shared")
mesh.from_pydata([tuple(X[v]) for v in drawn], [], [[remap[i] for i in vi] for vi, _ in faces])
uv = mesh.uv_layers.new(name="UVMap")
k = 0
for vi, ti in faces:
    for t in ti:
        uv.data[k].uv = tuple(uvs[t])
        k += 1
mesh.update()
for p in mesh.polygons:
    p.use_smooth = True
body = bpy.data.objects.new(f"{name}-shared", mesh)
bpy.context.scene.collection.objects.link(body)

TEXTURE = 2048
baked = bpy.data.images.new(f"{name}_skin", TEXTURE, TEXTURE)
mat = bpy.data.materials.new("Skin")
mat.use_nodes = True
tex = mat.node_tree.nodes.new("ShaderNodeTexImage")
tex.image = baked
bsdf = mat.node_tree.nodes["Principled BSDF"]
mat.node_tree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
bsdf.inputs["Roughness"].default_value = 0.8
mesh.materials.append(mat)
mat.node_tree.nodes.active = tex
scene = bpy.context.scene
scene.render.engine = "CYCLES"
scene.cycles.device = "CPU"
scene.cycles.samples = 1
scene.render.bake.use_pass_direct = False
scene.render.bake.use_pass_indirect = False
scene.render.bake.use_selected_to_active = True
scene.render.bake.cage_extrusion = 0.02
scene.render.bake.max_ray_distance = 0.04
scene.render.bake.margin = 16
target.modifiers.clear()
bpy.ops.object.select_all(action="DESELECT")
target.select_set(True)
body.select_set(True)
bpy.context.view_layer.objects.active = body
bpy.ops.object.bake(type="DIFFUSE")
baked.pack()
print("FIT baked", TEXTURE)

# 5. Rigged with MakeHuman's game skeleton and weights, turned to face +X.
for o in list(bpy.context.scene.objects):
    if o is not body:
        bpy.data.objects.remove(o)
turn = Matrix.Rotation(math.pi / 2, 4, "Z")
mesh.transform(turn)
R3 = np.array(turn.to_3x3())
fitted_joints = {b: R3 @ p for b, p in fitted_joints.items()}
skel = bpy.data.armatures.new(f"{name}-mh")
rig_obj = bpy.data.objects.new(f"{name}-mh", skel)
bpy.context.scene.collection.objects.link(rig_obj)
bpy.context.view_layer.objects.active = rig_obj
bpy.ops.object.mode_set(mode="EDIT")
eb = {}
# Our skeleton, its joints found on this body's skin; the export then keeps our bones' own
# directions (tools/characters/convert_rig.py), so a tail only needs to point somewhere.
heads = {b: R3 @ (w @ X[ring]) for b, (ring, w) in joint_rings.items()}
for b, head in heads.items():
    e = skel.edit_bones.new(b)
    e.head = Vector(head)
    e.tail = Vector(head + np.array([0, 0, 0.03]))
bpy.ops.object.mode_set(mode="OBJECT")
for k, b in enumerate(skin_bones):
    g = body.vertex_groups.new(name=b)
    for v in drawn:
        if skin[v][k] > 1e-4:
            g.add([remap[v]], float(skin[v][k]), "REPLACE")
body.parent = rig_obj
mod = body.modifiers.new("Armature", "ARMATURE")
mod.object = rig_obj
bpy.ops.object.select_all(action="DESELECT")
rig_obj.select_set(True)
body.select_set(True)
bpy.context.view_layer.objects.active = rig_obj
os.makedirs(OUT, exist_ok=True)
path = os.path.join(OUT, f"{name}-rigged.glb")
bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_animations=False)
# The bare mesh, for Tripo to paint (texture.sh): facing +X as Tripo's own models do.
plain = bpy.data.objects.new(f"{name}-paint", mesh.copy())
bpy.context.scene.collection.objects.link(plain)
plain.data.materials.clear()
bpy.ops.object.select_all(action="DESELECT")
plain.select_set(True)
bpy.ops.export_scene.gltf(filepath=os.path.join(OUT, f"{name}-paint.glb"), export_format="GLB", use_selection=True)
np.savez(os.path.join(OUT, f"{name}-fitted.npz"), verts=X[drawn], drawn=np.array(drawn), height=t_height)
print("EXPORTED", path)
