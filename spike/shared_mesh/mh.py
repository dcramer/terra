"""MakeHuman's CC0 data, read from its file formats (no MakeHuman or MPFB code, which is GPL):
the base mesh (hm08), its targets, the game_engine rig and its weights.

Everything is turned into Blender's frame as we use it: metres, Z up, facing -Y. MakeHuman's
files are in decimetres, Y up, facing +Z.
"""

import gzip
import json
import os

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HERE, "mh")
# The body's vertices are the first BODY of the mesh; after them come the helpers (tights,
# skirt, hair, eyes, teeth, joint cubes), which the game does not draw.
BODY = 13380


def to_blender(p):
    p = np.asarray(p, dtype=np.float64)
    return np.stack([p[..., 0], -p[..., 2], p[..., 1]], axis=-1) * 0.1


# The parts of the mesh we draw: the body, and its eyes, which sit in the body's eye holes.
DRAWN = ("body", "helper-l-eye", "helper-r-eye")


def base():
    """The base mesh: every vertex (Blender frame), the drawn parts' faces as vertex and
    texture coordinate indices, the texture coordinates, and each named group's vertices."""
    verts, uvs, faces, groups = [], [], [], {}
    group = None
    with open(os.path.join(DATA, "3dobjs", "base.obj")) as f:
        for line in f:
            if line.startswith("v "):
                verts.append([float(x) for x in line.split()[1:4]])
            elif line.startswith("vt "):
                uvs.append([float(x) for x in line.split()[1:3]])
            elif line.startswith("g "):
                group = line.split()[1]
            elif line.startswith("f "):
                corners = [c.split("/") for c in line.split()[1:]]
                vi = [int(c[0]) - 1 for c in corners]
                ti = [int(c[1]) - 1 for c in corners]
                groups.setdefault(group, set()).update(vi)
                if group in DRAWN:
                    faces.append((vi, ti))
    return to_blender(verts), faces, np.array(uvs), {g: sorted(v) for g, v in groups.items()}


def target(path):
    """A target's offsets for every vertex (zero where it does not move one)."""
    d = np.zeros((19158, 3))
    with gzip.open(os.path.join(DATA, "targets", path), "rt") as f:
        for line in f:
            if line.startswith("#") or not line.strip():
                continue
            i, x, y, z = line.split()
            d[int(i)] = [float(x), float(y), float(z)]
    return to_blender(d)


def basis():
    """The macro targets of a young adult, which the fit mixes: build, sex, height,
    proportions and the three ethnic shapes."""
    names = []
    for g in ("male", "female"):
        for m in ("minmuscle", "averagemuscle", "maxmuscle"):
            for w in ("minweight", "averageweight", "maxweight"):
                names.append(f"macrodetails/universal-{g}-young-{m}-{w}.target.gz")
        for h in ("minheight", "maxheight"):
            names.append(f"macrodetails/height/{g}-young-averagemuscle-averageweight-{h}.target.gz")
        for p in ("idealproportions", "uncommonproportions"):
            names.append(f"macrodetails/proportions/{g}-young-averagemuscle-averageweight-{p}.target.gz")
        for e in ("african", "asian", "caucasian"):
            names.append(f"macrodetails/{e}-{g}-young.target.gz")
    names = [n for n in names if os.path.exists(os.path.join(DATA, "targets", n))]
    return names, np.stack([target(n) for n in names])


# The head's own shapes the fit may use: its form, not its features (eyes, lids, mouth), which
# stay MakeHuman's so they open, close and move as made.
HEAD = ("head", "neck", "nose", "chin", "cheek", "forehead", "ears")


def head_basis():
    names = sorted(f"{d}/{f}" for d in HEAD for f in os.listdir(os.path.join(DATA, "targets", d)) if f.endswith(".target.gz"))
    return names, np.stack([target(n) for n in names])


def mirror():
    """For each vertex, its mirror image across the body's middle."""
    m = np.arange(19158)
    with open(os.path.join(DATA, "mesh_metadata", "hm08.mirror")) as f:
        for line in f:
            a, b, _ = line.split()
            m[int(a)] = int(b)
    return m


# MakeHuman's game_engine bones -> ours (tools/characters, the Quaternius skeleton).
OURS = {"pelvis": "DEF-hips", "spine_01": "DEF-spine.001", "spine_02": "DEF-spine.002",
        "spine_03": "DEF-spine.003", "neck_01": "DEF-neck", "head": "DEF-head"}
for _mh, _s in (("l", "L"), ("r", "R")):
    for _a, _b in (("clavicle", "shoulder"), ("upperarm", "upper_arm"), ("lowerarm", "forearm"), ("hand", "hand"),
                   ("thigh", "thigh"), ("calf", "shin"), ("foot", "foot"), ("ball", "toe")):
        OURS[f"{_a}_{_mh}"] = f"DEF-{_b}.{_s}"
    for _a, _b in (("thumb", "thumb"), ("index", "f_index"), ("middle", "f_middle"), ("ring", "f_ring"), ("pinky", "f_pinky")):
        for _i in (1, 2, 3):
            OURS[f"{_a}_0{_i}_{_mh}"] = f"DEF-{_b}.0{_i}.{_s}"


def rig():
    """The game_engine rig: each bone's parent and the points its head and tail sit at, as
    (kind, names): a joint cube's centre, or the mean of listed vertices."""
    with open(os.path.join(DATA, "rigs", "standard", "rig.game_engine.json")) as f:
        r = json.load(f)
    out = {}
    for name, b in r.items():
        ends = []
        for end in (b["head"], b["tail"]):
            if end["strategy"] == "CUBE":
                ends.append(("cube", end["cube_name"]))
            else:
                ends.append(("mean", end["vertex_indices"]))
        out[name] = {"parent": b["parent"] or None, "head": ends[0], "tail": ends[1]}
    return out


def point(verts, groups, end):
    kind, what = end
    if kind == "cube":
        return verts[groups[what]].mean(axis=0)
    return verts[what].mean(axis=0)


def weights():
    """The game_engine rig's skin weights over the body's vertices: {bone: (indices, weights)}."""
    with open(os.path.join(DATA, "rigs", "standard", "weights.game_engine.json")) as f:
        w = json.load(f)["weights"]
    out = {}
    for bone, pairs in w.items():
        pairs = [(i, x) for i, x in pairs if i < BODY]
        if pairs:
            idx, wt = zip(*pairs)
            out[bone] = (np.array(idx), np.array(wt))
    return out


def regressor(verts, p):
    """Writes point `p` as an affine mix of the body vertices round it, so it can be found
    again on any shape of the body: the ring of skin round a joint carries the joint."""
    body = verts[:BODY]
    d = np.linalg.norm(body - p, axis=1)
    near = np.argsort(d)
    ring = near[d[near] <= d[near[0]] * 1.6][:200]
    if len(ring) < 12:
        ring = near[:12]
    a = body[ring].T  # 3 x k
    k = len(ring)
    mu = 1e-4
    # Minimise |A w - p|^2 + mu |w - 1/k|^2 subject to sum(w) = 1.
    m = np.zeros((k + 1, k + 1))
    m[:k, :k] = a.T @ a + mu * np.eye(k)
    m[:k, k] = 1
    m[k, :k] = 1
    rhs = np.concatenate([a.T @ p + mu / k, [1.0]])
    w = np.linalg.solve(m, rhs)[:k]
    return ring, w
