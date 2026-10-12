"""Makes a body morph: the base body reshaped into a design of the base person, kept as a
change of shape of the base's own mesh (D112; how and why in research/body-morphs.md).

    tools/blender/run.sh tools/characters/morph.py BASE.glb DESIGN.glb NAME [--joints]

BASE is the base body as exported (tools/characters/out/base-male-game.glb, by build_bodies.sh). DESIGN is either
  - a Tripo model of a design of the base person (art/characters/base-male/morphs/NAME/model.glb):
    it is turned to face -Y, scaled to the base's height, feet on the ground, centred, and only
    its largest connected part is kept (a generated model can carry a slab of its backdrop); or
  - a body already rigged on our skeleton (another base, such as base-female.glb): it stands as
    the base does, and its own joints give the morph its proportions.

Steps:
  1. Pose. A generated design: each limb of the base's skeleton (shoulders, upper and lower
     arms, thighs, shins, feet) is turned about its joint to lie in the design's limb, the
     base's skin following by its own weights, so pose and shape are not confused. A rigged
     design: the base's joints are moved onto the design's.
  2. Shape. The base's skin is drawn onto the design's skin. Each pull is smoothed over
     distance (a Gaussian of 6, 4, 2.5 and then 1.5 cm), never over mesh steps, so dense and
     sparse parts fit alike and no crease of the design's clothes comes through. Skin with no
     trustworthy match (too far, or facing away) is carried by its neighbours. The hands stay
     the base's.
  3. Back to the T-pose, by undoing the pose: the change is a pure change of shape.
  4. Joints. A build morph keeps the base's skeleton, as Starfield's Heavy does: placing joints
     from the skin round them let belly fat drag the hips and spine 9-12 cm. A rigged design
     carries its own joints; --joints makes a generated one carry joints found from its skin.

Prints the fit (MORPH lines: mean and 95th percentile distance from the design's skin, and the
share of skin with no match). Writes tools/characters/out/NAME.npz, the morph per vertex of the
base (with the base's vertex positions, to find them again) and its joint offsets, and tools/characters/out/NAME.glb, the base at the morph's full
weight, rigged and without clips or sockets, for export_character.py.
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy
import numpy as np
from mathutils import Matrix, Vector
from mathutils.bvhtree import BVHTree
from mathutils.kdtree import KDTree

import look

OUT = look.OUT
base_path, design_path, name = look.args()[:3]
look.reset()


def load(path):
    before = set(bpy.context.scene.objects)
    bpy.ops.import_scene.gltf(filepath=os.path.join(look.ROOT, path))
    return [o for o in bpy.context.scene.objects if o not in before]


# The base, at rest.
base_objs = load(base_path)
arm = next(o for o in base_objs if o.type == "ARMATURE")
if arm.animation_data:
    arm.animation_data.action = None
for pb in arm.pose.bones:
    pb.matrix_basis.identity()
bpy.context.view_layer.update()
body = max((o for o in base_objs if o.type == "MESH"), key=lambda o: len(o.data.vertices))
mw = body.matrix_world
V_all = np.array([tuple(mw @ v.co) for v in body.data.vertices])

# glTF splits vertices at texture seams; work on the welded surface, one point per position.
keys = {}
canon = np.empty(len(V_all), dtype=int)
for i, p in enumerate(V_all):
    k = tuple(np.round(p, 5))
    canon[i] = keys.setdefault(k, len(keys))
V = np.zeros((len(keys), 3))
V[canon] = V_all
faces = [[canon[i] for i in p.vertices] for p in body.data.polygons]
n = len(V)

# Skin weights on the welded surface.
bones = [b.name for b in arm.data.bones if b.use_deform]
bi = {b: i for i, b in enumerate(bones)}
gname = {g.index: g.name for g in body.vertex_groups}
W = np.zeros((n, len(bones)))
for v in body.data.vertices:
    for g in v.groups:
        b = gname.get(g.group)
        if b in bi:
            W[canon[v.index], bi[b]] = g.weight
W /= np.maximum(W.sum(axis=1, keepdims=True), 1e-9)
amw = arm.matrix_world
J = {b.name: np.array(amw @ b.head_local) for b in arm.data.bones}
parent = {b.name: (b.parent.name if b.parent else None) for b in arm.data.bones}
order = sorted(J, key=lambda b: len(arm.data.bones[b].parent_recursive))
children = {b: [c for c in J if parent[c] == b] for b in J}

# The design. A rigged body already on our skeleton (a converted design, such as another base)
# stands as ours does, and its own joints give its proportions; a generated model is turned to
# face -Y, scaled to the base's height, feet on the ground, centred.
design_objs = load(design_path)
design_arm = next((o for o in design_objs if o.type == "ARMATURE"), None)
DJ = None
if design_arm is not None:
    if design_arm.animation_data:
        design_arm.animation_data.action = None
    for pb in design_arm.pose.bones:
        pb.matrix_basis.identity()
    bpy.context.view_layer.update()
    DJ = {b.name: np.array(design_arm.matrix_world @ b.head_local) for b in design_arm.data.bones if b.name in J}
    design = [max((o for o in design_objs if o.type == "MESH"), key=lambda o: len(o.data.vertices))]
else:
    design = [o for o in design_objs if o.type == "MESH"]
pts, polys = [], []
for o in design:
    off = len(pts)
    pts += [o.matrix_world @ v.co for v in o.data.vertices]
    polys += [tuple(off + i for i in p.vertices) for p in o.data.polygons]
P = np.array([tuple(p) for p in pts])
# Only the body: a generated model can carry other pieces (a slab of the backdrop), so keep
# the largest connected part.
root = list(range(len(P)))
# Points split at texture seams are one point.
first = {}
for i, p in enumerate(np.round(P, 5)):
    root[i] = first.setdefault(tuple(p), i)


def find(i):
    while root[i] != i:
        root[i] = root[root[i]]
        i = root[i]
    return i


for poly in polys:
    for j in poly[1:]:
        a_, b_ = find(poly[0]), find(j)
        if a_ != b_:
            root[a_] = b_
comp = np.array([find(i) for i in range(len(P))])
sizes = np.bincount(comp)
polys = [poly for poly in polys if comp[poly[0]] == sizes.argmax()]
print(f"MORPH {name} design: kept the body, {sizes.max()} of {len(P)} points")
used = np.unique([i for poly in polys for i in poly])
if DJ is None:
    P = P @ np.array(Matrix.Rotation(-math.pi / 2, 3, "Z")).T
    P *= (V[:, 2].max() - V[:, 2].min()) / (P[used, 2].max() - P[used, 2].min())
    P[:, 2] -= P[used, 2].min() - V[:, 2].min()
    for a in (0, 1):
        P[:, a] += (V[:, a].max() + V[:, a].min()) / 2 - (P[used, a].max() + P[used, a].min()) / 2
tree = BVHTree.FromPolygons([Vector(p) for p in P], polys)

# Neighbours on the welded surface.
edges = set()
for f in faces:
    for a, b in zip(f, f[1:] + f[:1]):
        if a != b:
            edges.add((min(a, b), max(a, b)))
edges = np.array(sorted(edges))
deg = np.bincount(edges.ravel(), minlength=n)


def smooth(X, times):
    for _ in range(times):
        acc = np.zeros_like(X)
        np.add.at(acc, edges[:, 0], X[edges[:, 1]])
        np.add.at(acc, edges[:, 1], X[edges[:, 0]])
        X = 0.5 * X + 0.5 * acc / np.maximum(deg, 1)[:, None]
    return X


def normals(X):
    N = np.zeros_like(X)
    for f in faces:
        fn = np.cross(X[f[1]] - X[f[0]], X[f[2]] - X[f[0]])
        N[f] += fn
    return N / np.maximum(np.linalg.norm(N, axis=1, keepdims=True), 1e-12)


def nearest(X, reach, facing=0.3):
    N = normals(X)
    C = X.copy()
    ok = np.zeros(n, dtype=bool)
    for i in range(n):
        hit = tree.find_nearest(Vector(X[i]), reach)
        if hit[0] is not None:
            C[i] = hit[0]
            ok[i] = np.dot(np.array(hit[1]), N[i]) > facing
    return C, ok


# 2. The pose: world rotations per bone, skinning the base.
R = {b: np.eye(3) for b in J}


def posed_joints():
    # A rigged design: the base's joints moved onto the design's, so its skin takes the
    # design's proportions before its shape is fitted.
    if DJ is not None:
        return {b: DJ.get(b, J[b]) for b in J}
    p = {}
    for b in order:
        q = parent[b]
        p[b] = J[b] if q is None else p[q] + R[q] @ (J[b] - J[q])
    return p


def pose(X):
    p = posed_joints()
    out = np.zeros_like(X)
    for b in bones:
        w = W[:, bi[b]]
        live = w > 0
        if live.any():
            out[live] += w[live, None] * (p[b] + (X[live] - J[b]) @ R[b].T)
    return out


def unpose(Y):
    """The rest positions that `pose` takes to Y, vertex by vertex."""
    p = posed_joints()
    A = np.zeros((n, 3, 3))
    t = np.zeros((n, 3))
    for b in bones:
        w = W[:, bi[b]][:, None]
        A += w[:, :, None] * R[b]
        t += w * (p[b] - R[b] @ J[b])
    return np.einsum("nij,nj->ni", np.linalg.inv(A), Y - t)


def descendants(b):
    out = [b]
    for c in children[b]:
        out += descendants(c)
    return out


LIMBS = [f"{part}_{s}" for s in ("l", "r") for part in ("clavicle", "upperarm", "lowerarm", "thigh", "calf", "foot")]
for sweep in range(0 if DJ is not None else 3):
    for b in sorted(LIMBS, key=order.index):
        X = pose(V)
        own = W[:, bi[b]] > 0.6
        C, ok = nearest(X, 0.1)
        use = own & ok
        if use.sum() < 20:
            continue
        pivot = posed_joints()[b]
        a = X[use] - pivot
        c = C[use] - pivot
        u, _, vt = np.linalg.svd(a.T @ c)
        d = np.sign(np.linalg.det(vt.T @ u.T))
        Q = vt.T @ np.diag([1, 1, d]) @ u.T
        angle = math.degrees(math.acos(max(-1.0, min(1.0, (np.trace(Q) - 1) / 2))))
        if angle > 25:
            continue
        for x in descendants(b):
            R[x] = Q @ R[x]
X = pose(V)
C, ok = nearest(X, 0.1)
err = np.linalg.norm(C[ok] - X[ok], axis=1)
print(f"MORPH {name} posed: mean {err.mean() * 100:.2f} cm, 95% {np.percentile(err, 95) * 100:.2f} cm")

# 3. The shape, smoothed by distance, the hands kept.
hand = sum(W[:, bi[b]] for b in bones if b.startswith(("hand_", "thumb_", "index_", "middle_", "ring_", "pinky_")))
keep = smooth(np.clip(1.0 - hand, 0, 1)[:, None], 4)[:, 0]


def kernel(X, sigma):
    kd = KDTree(n)
    for i in range(n):
        kd.insert(Vector(X[i]), i)
    kd.balance()
    rows, cols, ws = [], [], []
    for i in range(n):
        for _, j, dist in kd.find_range(Vector(X[i]), 3 * sigma):
            rows.append(i)
            cols.append(j)
            ws.append(math.exp(-0.5 * (dist / sigma) ** 2))
    return np.array(rows), np.array(cols), np.array(ws)


def spread(K, pull, trust):
    rows, cols, ws = K
    num = np.zeros_like(pull)
    den = np.zeros(n)
    np.add.at(num, rows, (ws * trust[cols])[:, None] * pull[cols])
    np.add.at(den, rows, ws * trust[cols])
    return num / np.maximum(den, 1e-9)[:, None]


for sigma in (0.06, 0.04, 0.025, 0.015):
    K = kernel(X, sigma)
    for _ in range(6):
        C, ok = nearest(X, 0.08)
        X = X + 0.7 * spread(K, C - X, ok.astype(float)) * keep[:, None]
C, ok = nearest(X, 0.1)
err = np.linalg.norm(C[ok] - X[ok], axis=1)
print(f"MORPH {name} fitted: mean {err.mean() * 1000:.1f} mm, 95% {np.percentile(err, 95) * 1000:.1f} mm, unmatched {1 - ok.mean():.1%}")

# 4. Back to the T-pose: the morph, and the joints the skin round them carries.
# A rigged design stands in the rest pose already, so what is fitted is the rest shape.
rest = X if DJ is not None else unpose(X)
delta = rest - V


def ring(p):
    d = np.linalg.norm(V - p, axis=1)
    near = np.argsort(d)
    r = near[d[near] <= d[near[0]] * 1.6][:200]
    if len(r) < 12:
        r = near[:12]
    a = V[r].T
    k = len(r)
    m = np.zeros((k + 1, k + 1))
    m[:k, :k] = a.T @ a + 1e-4 * np.eye(k)
    m[:k, k] = 1
    m[k, :k] = 1
    w = np.linalg.solve(m, np.concatenate([a.T @ p + 1e-4 / k, [1.0]]))[:k]
    return r, w


heads = {b: (lambda r: r[1] @ rest[r[0]])(ring(J[b])) for b in J}
moved = {b: float(np.linalg.norm(heads[b] - J[b])) for b in J}
top = sorted(moved.items(), key=lambda t: -t[1])[:3]
print(f"MORPH {name} largest change {np.abs(delta).max() * 100:.1f} cm; the skin round the joints moved most at", [(b, round(m * 100, 1)) for b, m in top])
# A build morph changes flesh, not the skeleton (as Starfield's Heavy does): its joints stay
# the base's. Proportions (height, limb length) are bone scales of their own; --joints lets a
# morph that does change them (a woman's) carry its joints.
if DJ is not None:
    heads = {b: DJ.get(b, J[b]) for b in J}
elif "--joints" not in look.args():
    heads = dict(J)
os.makedirs(OUT, exist_ok=True)
np.savez(os.path.join(OUT, f"{name}.npz"), points=V_all, delta=delta[canon], joints=np.array([heads[b] - J[b] for b in sorted(J)]), bones=np.array(sorted(J)))

# The base reshaped and rigged at the morph's full weight, its joints moved, its clips dropped.
for v in body.data.vertices:
    v.co = mw.inverted() @ Vector(rest[canon[v.index]])
bpy.context.view_layer.objects.active = arm
bpy.ops.object.select_all(action="DESELECT")
arm.select_set(True)
bpy.ops.object.mode_set(mode="EDIT")
inv = amw.inverted()
for e in arm.data.edit_bones:
    shift = inv.to_3x3() @ Vector(heads[e.name] - J[e.name])
    e.head = e.head + shift
    e.tail = e.tail + shift
# Sockets are placed again by the export, from this body's own joints and skin.
for e in [e for e in arm.data.edit_bones if e.name.startswith("socket.")]:
    arm.data.edit_bones.remove(e)
bpy.ops.object.mode_set(mode="OBJECT")
for o in list(bpy.context.scene.objects):
    if o not in (arm, body):
        bpy.data.objects.remove(o)
arm.animation_data_clear()
for a in list(bpy.data.actions):
    bpy.data.actions.remove(a)
bpy.ops.object.select_all(action="DESELECT")
arm.select_set(True)
body.select_set(True)
path = os.path.join(OUT, f"{name}.glb")
bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_animations=False)
print("EXPORTED", path)
