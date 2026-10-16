"""Carries a worn look made on the base body onto the base's morphs, so gear is made once (D128;
research/body-morphs.md).

    tools/blender/run.sh tools/characters/carry_gear.py BASE.glb LOOK.glb OUT.glb MORPH
    tools/blender/run.sh tools/characters/carry_gear.py BASE.glb LOOK.glb OUT.glb --morphs=heavy,thin,...

With one MORPH (tools/characters/out/MORPH.npz) the look is reshaped to it, for checking it
on that morph's body (make_morph.sh). With --morphs the look keeps its shape and gains one morph
target per morph, in that order, as the base body's file has them (export_character.py
--morphs=...), so in the game one look follows any mix of them: the game's file for the item.

Each part of the look (its connected pieces, joined across texture seams) that lies on the skin
(median distance under 3.5 cm) takes the morph of the nearest skin, vertex by vertex: a jerkin,
a cap, a pack's straps. (Turning each vertex's offset with the skin's normal instead was tried
and did worse: a strap's inner and outer faces turned apart.) A part that stands off the body (a pack's bag) moves as one piece, by
the skin nearest it, and out far enough to keep its clearance over the skin that moved out most
beneath it. A look hung on a socket (no skin) moves with its socket, which each morph moves
already (export_character.py); on top of that it moves by the same rule as a part that stands
off, less the socket's own move, so a bigger seat does not swallow a quiver: with one MORPH,
by that much; with --morphs, recorded per morph in its extras, `{"morphs": {M: [x, y, z]}}` in
glTF's axes in its socket's space, for the game to mix as it mixes the body's morphs. The look
keeps its skin weights, sockets and extras.

Where gear_check.py --fix found skin coming through the look on a morph's body, through our
clips (tools/characters/out/M-ITEM-push.npz), that morph also pushes the look out there, by
as much, spread smoothly over the look round it: the clipping fixer, baked per morph.
"""

import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy
import numpy as np
from mathutils import Vector
from mathutils.kdtree import KDTree

import look
import skin_piece

base_path, look_path, out_path = look.args()[:3]
rest = [a for a in look.args()[3:]]
MORPHS = next((a.split("=", 1)[1].split(",") for a in rest if a.startswith("--morphs=")), None)
APPLY = None if MORPHS is not None else rest[0]
FOLLOWS_M = 0.035  # a part this close to the skin, over most of it, follows it vertex by vertex
PUSH_SPREAD_M = 0.03  # a fixer's push fades to nothing this far round where it was needed
ITEM = os.path.splitext(os.path.basename(look_path))[0]
BASE_NAME = look.body_name(base_path)
look.reset()


def load(path):
    before = set(bpy.context.scene.objects)
    bpy.ops.import_scene.gltf(filepath=os.path.join(look.ROOT, path))
    return [o for o in bpy.context.scene.objects if o not in before]


def rest_sockets(objects):
    """Each socket's place and turn at rest, among a body file's `objects`: the importer leaves
    the skeleton in some clip's first frame, so its pose is cleared first."""
    arm = next(o for o in objects if o.type == "ARMATURE")
    if arm.animation_data:
        arm.animation_data.action = None
    for pb in arm.pose.bones:
        pb.matrix_basis.identity()
    bpy.context.view_layer.update()
    # A second file's sockets import with Blender's ".001" on their names.
    return {re.sub(r"\.\d{3}$", "", o.name): o.matrix_world.copy() for o in objects if o.name.startswith("socket.")}


base = load(base_path)
body = max((o for o in base if o.type == "MESH"), key=lambda o: len(o.data.vertices))
V = np.array([tuple(body.matrix_world @ v.co) for v in body.data.vertices])
# The skin's outward normals at rest: which way is away from the body, wherever a look is.
NV = np.array([tuple((body.matrix_world.to_3x3() @ v.normal).normalized()) for v in body.data.vertices])
kd = KDTree(len(V))
for i, p in enumerate(V):
    kd.insert(Vector(p), i)
kd.balance()
SOCKETS = rest_sockets(base)
for o in base:
    bpy.data.objects.remove(o)


def morph_sockets(name):
    """Where each socket stands at rest on the morph's own body (tools/characters/out/M-game.glb)."""
    objects = load(os.path.join(look.OUT, f"{name}-game.glb"))
    found = rest_sockets(objects)
    for o in objects:
        bpy.data.objects.remove(o)
    return found


def morph_delta(name):
    """The morph per vertex of the base body in BASE, matched by position. The base itself
    is a morph of nothing, so its own looks are fixed by the same rounds as a morph's."""
    if name == BASE_NAME:
        return np.zeros((len(V), 3))
    # Each array read once: an .npz reads an array from disk on every access.
    with np.load(os.path.join(look.OUT, f"{name}.npz")) as data:
        points, delta = data["points"], data["delta"]
    mkd = KDTree(len(points))
    for i, p in enumerate(points):
        mkd.insert(Vector(p), i)
    mkd.balance()
    return delta[[mkd.find(Vector(p))[1] for p in V]]


def standing_off(P, delta):
    """How far a piece standing off the body moves as one, for a morph `delta` of the base's
    skin: by the skin nearest it, then out along the way it stands off far enough to keep its
    clearance over the skin beneath it that moved out most. `P`: its points, world space."""
    near = [kd.find(Vector(p)) for p in P]
    dist = np.array([n[2] for n in near])
    under = np.array([n[1] for n in near])
    d = delta[under]
    nearest = dist <= np.percentile(dist, 20)
    shift = d[nearest].mean(axis=0)
    out = (P - V[under]).mean(axis=0)
    out /= max(np.linalg.norm(out), 1e-9)
    extra = max(0.0, float(((d - shift) @ out).max()))
    return shift + out * extra


def pushes(o, name, moved):
    """How far, world space, each vertex of `o` moves out to clear the skin of morph `name`
    through our clips (gear_check.py --fix), spread over the look round each place. `moved`:
    how far the morph itself moves each vertex, so each is found where the check saw it."""
    path = os.path.join(look.OUT, f"{name}-{ITEM}-push.npz")
    zero = np.zeros((len(o.data.vertices), 3))
    if not os.path.exists(path):
        return zero
    with np.load(path) as npz:
        data = {k: npz[k] for k in npz.files}
    mw = o.matrix_world
    P = np.array([tuple(mw @ v.co) for v in o.data.vertices]) + moved
    # Away from the skin nearest each vertex: never along the look's own normal, which on the
    # side of a bag that faces the back points into the body.
    N = NV[[kd.find(Vector(p - m))[1] for p, m in zip(P, moved)]]
    total = np.zeros((len(P), 3))
    # Each round was measured on the look as the rounds before it left it, so each is found
    # where those rounds put the vertices.
    for n in range(sum(1 for k in data if k.startswith("need"))):
        need, at = data[f"need{n}"], data[f"at{n}"]
        spread = np.zeros(len(P))
        for k in np.nonzero(need)[0]:
            fade = np.clip(1.0 - np.linalg.norm(P + total - at[k], axis=1) / PUSH_SPREAD_M, 0.0, 1.0)
            spread = np.maximum(spread, need[k] * fade)
        total += N * spread[:, None]
    if not total.any():
        return zero
    print("PUSHED", o.name, "for", name, f"{int((np.linalg.norm(total, axis=1) > 0).sum())} vertices, up to {np.linalg.norm(total, axis=1).max() * 100:.1f} cm")
    return total


def carried(o, delta):
    """How far each vertex of `o` moves under the base's morph `delta` (world space)."""
    mw = o.matrix_world
    P = np.array([tuple(mw @ v.co) for v in o.data.vertices])
    if o.get("hung"):
        # A skirt or apron hangs off the skin (hung_garment.py): it moves as the skin round it
        # does, evened out over it, so its hem does not tear between the shins.
        return skin_piece.follow(P, V, delta, [tuple(e.vertices) for e in o.data.edges])
    near = [kd.find(Vector(p)) for p in P]
    dist = np.array([n[2] for n in near])
    under = np.array([n[1] for n in near])
    d = delta[under]
    # Parts, by connection (glTF splits vertices at seams, so join by position too).
    root = list(range(len(P)))

    def find(i):
        while root[i] != i:
            root[i] = root[root[i]]
            i = root[i]
        return i

    first = {}
    for i, p in enumerate(np.round(P, 5)):
        j = first.setdefault(tuple(p), i)
        root[find(i)] = find(j)
    for poly in o.data.polygons:
        for j in poly.vertices[1:]:
            root[find(poly.vertices[0])] = find(j)
    part = np.array([find(i) for i in range(len(P))])
    moved = d.copy()
    for k in np.unique(part):
        members = part == k
        if np.median(dist[members]) > FOLLOWS_M:
            # Stands off the body: moves as one (standing_off).
            moved[members] = standing_off(P[members], delta)
    return moved


piece = load(look_path)
skinned = [o for o in piece if o.type == "MESH" and any(m.type == "ARMATURE" for m in o.modifiers)]
deltas = {m: morph_delta(m) for m in (MORPHS if MORPHS is not None else [APPLY])}
def shift(o, moves, inv):
    """Moves each vertex of `o` by `moves` (world space), every shape key it already has with
    it, so a shape on the look keeps its place on it."""
    keys = o.data.shape_keys.key_blocks if o.data.shape_keys else []
    for v, m in zip(o.data.vertices, moves):
        d = inv @ Vector(m)
        v.co = v.co + d
        for k in keys:
            k.data[v.index].co = k.data[v.index].co + d


for o in skinned:
    inv = o.matrix_world.inverted().to_3x3()
    # The base's own pushes first, where its skin came through the look in our clips: they are
    # part of the look on every body. (Carried onto the base itself, they are its morph's.)
    if APPLY != BASE_NAME:
        shift(o, pushes(o, BASE_NAME, np.zeros((len(o.data.vertices), 3))), inv)
    if APPLY is not None:
        moved = carried(o, deltas[APPLY])
        shift(o, moved + pushes(o, APPLY, moved), inv)
    else:
        if o.data.shape_keys is None:
            o.shape_key_add(name="Basis", from_mix=False)
        basis = o.data.shape_keys.key_blocks[0].data
        for name in MORPHS:
            key = o.shape_key_add(name=name, from_mix=False)
            moved = carried(o, deltas[name])
            for v, m in zip(o.data.vertices, moved + pushes(o, name, moved)):
                key.data[v.index].co = basis[v.index].co + inv @ Vector(m)
    print("CARRIED", o.name, len(o.data.vertices), "vertices", "onto " + APPLY if APPLY else "with morphs " + ",".join(MORPHS))
# A look hung on a socket: its socket moves with each morph already; it moves the rest of
# the way to keep its clearance.
hung = [o for o in piece if o.type == "MESH" and o.get("socket") in SOCKETS and o not in skinned]
for o in hung:
    socket = SOCKETS[o["socket"]]
    # The look's file places it in its socket's space.
    P = np.array([tuple(socket @ (o.matrix_world @ v.co)) for v in o.data.vertices])
    offsets = {}
    for name in (MORPHS if MORPHS is not None else [APPLY]):
        socket_moved = morph_sockets(name)[o["socket"]].translation - socket.translation
        shift = Vector(standing_off(P, deltas[name])) - socket_moved
        offsets[name] = socket.to_3x3().inverted() @ shift  # in the socket's space, Blender's axes
    if APPLY is not None:
        inv = o.matrix_world.inverted().to_3x3()
        for v in o.data.vertices:
            v.co = v.co + inv @ offsets[APPLY]
    else:
        # glTF's axes: Blender's (x, y, z) is glTF's (x, z, -y).
        o["morphs"] = {n: [round(x.x, 5), round(x.z, 5), round(-x.y, 5)] for n, x in offsets.items()}
    print("CARRIED", o.name, "on", o["socket"], ", ".join(f"{n} {tuple(round(c, 3) for c in x)}" for n, x in offsets.items()))
if not skinned and not hung:
    print("CARRIED nothing: a look on a socket the base has not")
# The game's file, for every body: hides no patch that opened in any pose on any of them
# (gear_check.py writes which, beside each look it checks).
if MORPHS is not None:
    shown = set()
    for m in [BASE_NAME] + MORPHS:
        path = os.path.join(look.OUT, f"{m}-{ITEM}-uncover.json")
        if os.path.exists(path):
            with open(path) as f:
                shown |= set(json.load(f))
    for o in bpy.context.scene.objects:
        if "covers" in o:
            kept = [p for p in o["covers"] if p not in shown]
            print("UNCOVERED", ITEM, f"{len(o['covers']) - len(kept)} of {len(o['covers'])} patches, which open in some pose")
            o["covers"] = kept
bpy.ops.object.select_all(action="SELECT")
look.fit_textures(sys.argv)
bpy.ops.export_scene.gltf(filepath=os.path.join(look.ROOT, out_path), export_format="GLB", use_selection=True,
                          export_animations=False, export_extras=True, export_morph=True)
print("EXPORTED", out_path)
