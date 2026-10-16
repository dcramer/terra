"""The dragon: Tripo's textured wyvern made ours. Its mouth is cut open along the lips and lined,
its tail straightened and its wings lengthened a little to an 18 m span, then it is skinned on a
skeleton of our own (spine, neck, head and jaw, tail, hind legs, and each wing an arm with a
thumb and four fingers spanning the membrane) and its flying clips are keyed here, as no clip
library has wings.

    tools/blender/run.sh tools/creatures/dragon.py art/creatures/dragon/model.glb assets/models/creatures/Dragon.glb

Every position below is in the frame the joints were read in: Tripo's model centred on its box
and scaled to life size (13 m nose to tail), Blender's axes, the head toward -Y (glTF +Z), the
dragon's left toward +X. The file's origin is the body's centre of mass, between the wing roots
and the hips, since it flies. The clips are in place: the game moves, turns and banks the root.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Quaternion, Vector

args = sys.argv[sys.argv.index("--") + 1:]
src, out = args[:2]
FPS = 30
SCALE = 15.24  # Tripo's units to metres: 13 m from nose to tail tip
COM = Vector((0.0, -1.45, 0.45))  # the origin, between the wing roots and the hips
WING_STRETCH = 1.12  # the wing beyond its root, to an 18 m span spread flat
SHOULDER = Vector((0.9, -1.95, 1.25))
TAU3 = 2 * math.pi / 3

bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene
scene.render.fps = FPS
scene.frame_start = 0
bpy.ops.import_scene.gltf(filepath=src)
mesh = next(o for o in scene.objects if o.type == "MESH")
mesh.data.transform(mesh.matrix_world)
mesh.parent = None
mesh.matrix_world = Matrix.Identity(4)
for o in list(scene.objects):
    if o is not mesh:
        bpy.data.objects.remove(o)
mesh.name = mesh.data.name = "Dragon"


def select_only(ob):
    for o in scene.objects:
        o.select_set(o is ob)
    bpy.context.view_layer.objects.active = ob


select_only(mesh)
bm = bmesh.new()
bm.from_mesh(mesh.data)
bmesh.ops.remove_doubles(bm, verts=bm.verts[:], dist=1e-4)
bm.to_mesh(mesh.data)
bm.free()
if mesh.data.has_custom_normals:
    print("clear custom normals", bpy.ops.mesh.customdata_custom_splitnormals_clear())
vs = [v.co for v in mesh.data.vertices]
lo = Vector([min(v[i] for v in vs) for i in range(3)])
hi = Vector([max(v[i] for v in vs) for i in range(3)])
mesh.data.transform(Matrix.Diagonal((SCALE, SCALE, SCALE, 1.0)) @ Matrix.Translation(-(lo + hi) / 2))

# The mouth -------------------------------------------------------------------------------
# Tripo modelled it shut, one skin over both lips. Cut it along the lips with a plane, from
# the mouth's corners round the snout, and line it with a palate and a floor, so the jaw opens
# on a mouth rather than stretching a sheet of skin.
MOUTH_FRONT = (-6.5, 1.06)  # (y, z) of the lip line at the snout
MOUTH_CORNER = (-5.55, 1.12)  # and at the mouth's corners
SLOPE = (MOUTH_CORNER[1] - MOUTH_FRONT[1]) / (MOUTH_CORNER[0] - MOUTH_FRONT[0])
PLANE_NO = Vector((0.0, -SLOPE, 1.0)).normalized()
PLANE_CO = Vector((0.0, MOUTH_FRONT[0], MOUTH_FRONT[1]))


def above_lips(p):
    return (p - PLANE_CO).dot(PLANE_NO)


def in_mouth(p):
    return p.y < MOUTH_CORNER[0] and abs(p.x) < 0.5 and 0.8 < p.z < 1.4


bm = bmesh.new()
bm.from_mesh(mesh.data)
uv = bm.loops.layers.uv.active
region = [f for f in bm.faces if all(in_mouth(v.co) for v in f.verts)]
geom = list({e for f in region for e in f.edges}) + list({v for f in region for v in f.verts}) + region
cut = bmesh.ops.bisect_plane(bm, geom=geom, dist=1e-5, plane_co=PLANE_CO, plane_no=PLANE_NO)
cut_edges = [e for e in cut["geom_cut"] if isinstance(e, bmesh.types.BMEdge) and all(in_mouth(v.co) for v in e.verts)]
print("mouth cut edges", len(cut_edges))
bmesh.ops.split_edges(bm, edges=cut_edges)
bm.verts.ensure_lookup_table()


def face_side(f):
    return 1 if above_lips(f.calc_center_median()) > 0 else -1


# The lips' edges, now open: one loop of the upper lip's and the lower lip's, joined at the
# mouth's corners. Walk each lip from corner to corner.
open_edges = [e for e in bm.edges if e.is_boundary and all(in_mouth(v.co) for v in e.verts)]
lips = {1: [], -1: []}
for e in open_edges:
    lips[face_side(e.link_faces[0])].append(e)
print("lip edges upper", len(lips[1]), "lower", len(lips[-1]))


def chains(edges):
    """The edges as ordered runs of vertices, longest first."""
    left = set(edges)
    at = {}
    for e in edges:
        for v in e.verts:
            at.setdefault(v, []).append(e)
    runs = []
    while left:
        # Start from an end, a vertex with one edge in the set, if there is one.
        start = next((v for e in left for v in e.verts if len([x for x in at[v] if x in left]) == 1), None)
        if start is None:
            start = next(iter(left)).verts[0]
        run = [start]
        v = start
        while True:
            nxt = next((e for e in at[v] if e in left), None)
            if nxt is None:
                break
            left.discard(nxt)
            v = nxt.other_vert(v)
            run.append(v)
        runs.append(run)
    return sorted(runs, key=len, reverse=True)


upper = chains(lips[1])[0]
lower = chains(lips[-1])[0]
print("lips", len(upper), len(lower), "corners shared", {upper[0], upper[-1]} == {lower[0], lower[-1]})


COLOUR = next(i for i in bpy.data.images if "base" in i.name.lower())
PIXELS = COLOUR.pixels[:]


def find_texel(want, what):
    """The texel of the colour map nearest a colour (linear), to paint a part made here."""
    w, h = COLOUR.size
    best, at = 9.0, (0.5, 0.5)
    for j in range(8, h - 8, 8):
        for i in range(8, w - 8, 8):
            k = (j * w + i) * 4
            d = sum((a - b) ** 2 for a, b in zip(PIXELS[k:k + 3], want))
            if d < best:
                best, at = d, ((i + 0.5) / w, (j + 0.5) / h)
    print(what, "texel", at, "distance", round(best, 4))
    return at


flesh = find_texel((0.11, 0.025, 0.02), "flesh")  # the membrane's dark red-brown
ivory = find_texel((0.62, 0.52, 0.36), "ivory")  # the horns' and claws' bone
lining = []
for run, side in ((upper, 1), (lower, -1)):
    try:
        f = bm.faces.new(run)
    except ValueError as err:
        print("lining failed", err)
        continue
    f.normal_update()
    # The palate faces down into the mouth, the floor up.
    if (f.normal.z > 0) == (side > 0):
        f.normal_flip()
    lining.append(f)
for f in lining:
    for loop in f.loops:
        loop[uv].uv = flesh
tri = bmesh.ops.triangulate(bm, faces=lining, quad_method="BEAUTY", ngon_method="BEAUTY")
lining_faces = set(tri["faces"])
print("mouth lining triangles", len(tri["faces"]))

# Teeth along both lips, set in from the lip's edge so a shut mouth hides them in the other
# jaw: three-sided cones, longest at the front, curving back toward the throat.
middle = sum((v.co for v in upper), Vector()) / len(upper)
teeth = []  # (vertex, the lip's vertex it grows from, side)
for run, side in ((upper, 1), (lower, -1)):
    along = [0.0]
    for a, b in zip(run, run[1:]):
        along.append(along[-1] + (b.co - a.co).length)
    total = along[-1]
    at = 0.14
    for v, s in zip(run, along):
        if s < at or s > total - 0.14:
            continue
        at = s + 0.085
        front = 1.0 - abs(s - total / 2) / (total / 2)
        length = (0.07 + 0.09 * front) * (1.15 if side > 0 else 0.9)
        radius = 0.022 + 0.012 * front
        inward = middle - v.co
        inward -= PLANE_NO * inward.dot(PLANE_NO)
        inward.normalize()
        axis = (-side * PLANE_NO + 0.2 * inward + Vector((0.0, 0.25, 0.0))).normalized()
        base = v.co + inward * 0.05 - axis * 0.02
        a = axis.orthogonal().normalized()
        b = axis.cross(a)
        ring = [bm.verts.new(base + radius * (math.cos(k * TAU3) * a + math.sin(k * TAU3) * b)) for k in range(3)]
        tip = bm.verts.new(base + axis * length)
        for k in range(3):
            f = bm.faces.new((ring[k], ring[(k + 1) % 3], tip))
            f.normal_update()
            if f.normal.dot(f.calc_center_median() - base - axis * (length / 3)) < 0:
                f.normal_flip()
            f.smooth = True
            for loop in f.loops:
                loop[uv].uv = ivory
        teeth += [(t, v, side) for t in ring + [tip]]
print("teeth", len(teeth) // 4)

# Which side of the lips each head vertex is on, for the jaw's weights; a vertex on the cut
# goes by its faces, and the corners are shared.
bm.verts.index_update()
lip_side = {}
corners = {upper[0].index, upper[-1].index}
for v in bm.verts:
    if v.co.y > -4.9:
        continue
    d = above_lips(v.co)
    if abs(d) < 1e-4 and v.link_faces:
        sides = {face_side(f) for f in v.link_faces if f not in lining_faces}
        d = 0.0 if len(sides) != 1 else float(next(iter(sides)))
    lip_side[v.index] = 0 if v.index in corners else (1 if d > 0 else -1)
lining_verts = {v.index for f in lining_faces for v in f.verts}
tooth_root = {}
for t, root, side in teeth:
    lip_side[t.index] = side
    tooth_root[t.index] = root.index
bm.to_mesh(mesh.data)
bm.free()

# The tail, straightened (Tripo curled it a metre to the left) and the wings lengthened from
# their roots, as one warp of the mesh and the joints alike.
TAIL_CURVE = [(2.0, 0.0), (2.6, 0.06), (3.1, 0.14), (3.6, 0.26), (4.1, 0.39), (4.6, 0.54), (5.1, 0.69), (5.6, 0.82), (6.1, 0.95), (6.6, 1.03)]


def tail_x(y):
    if y <= TAIL_CURVE[0][0]:
        return 0.0
    for (y0, x0), (y1, x1) in zip(TAIL_CURVE, TAIL_CURVE[1:]):
        if y <= y1:
            return x0 + (x1 - x0) * (y - y0) / (y1 - y0)
    return TAIL_CURVE[-1][1]


def smoothstep(a, b, x):
    t = min(1.0, max(0.0, (x - a) / (b - a)))
    return t * t * (3 - 2 * t)


def warp(p):
    p = Vector(p)
    p.x -= tail_x(p.y)
    side = 1.0 if p.x >= 0 else -1.0
    root = Vector((SHOULDER.x * side, SHOULDER.y, SHOULDER.z))
    s = 1.0 + (WING_STRETCH - 1.0) * smoothstep(1.0, 1.8, abs(p.x))
    p = root + (p - root) * s
    return p - COM


for v in mesh.data.vertices:
    v.co = warp(v.co)
mesh.data.update()
select_only(mesh)
print("smooth", bpy.ops.object.shade_smooth_by_angle(angle=math.radians(50.0), keep_sharp_edges=True))
for image in bpy.data.images:
    print("texture", image.name, tuple(image.size))

# The skeleton ----------------------------------------------------------------------------
# name: (head, tail, parent, deform). The left side's; the right is its mirror.
J = {
    "Root": ((0, -1.45, 0.45), (0, -2.45, 0.45), None, False),
    "Body": ((0, -1.45, 0.45), (0, -2.2, 0.45), "Root", False),
    "Pelvis": ((0, -0.55, -0.05), (0, -1.5, 0.5), "Body", True),
    "Spine": ((0, -1.5, 0.5), (0, -2.3, 0.95), "Pelvis", True),
    "Chest": ((0, -2.3, 0.95), (0, -2.95, 1.12), "Spine", True),
}
NECK = [(-2.95, 1.12), (-3.3, 1.22), (-3.65, 1.30), (-4.0, 1.35), (-4.3, 1.36), (-4.62, 1.34), (-4.95, 1.31)]
for i in range(6):
    (y0, z0), (y1, z1) = NECK[i], NECK[i + 1]
    J[f"Neck{i + 1}"] = ((0, y0, z0), (0, y1, z1), "Chest" if i == 0 else f"Neck{i}", True)
J["Head"] = ((0, -4.95, 1.31), (0, -6.45, 1.15), "Neck6", True)
J["Jaw"] = ((0, -5.1, 1.02), (0, -6.35, 0.93), "Head", True)
TAIL = [(-0.45, -0.3), (0.3, -0.75), (1.0, -0.96), (1.7, -1.08), (2.4, -1.12), (3.1, -1.11), (3.8, -1.06), (4.5, -1.03), (5.2, -0.97), (5.85, -0.97), (6.5, -0.99)]
for i in range(10):
    (y0, z0), (y1, z1) = TAIL[i], TAIL[i + 1]
    J[f"Tail{i + 1}"] = ((0, y0, z0), (0, y1, z1), "Pelvis" if i == 0 else f"Tail{i}", True)
LEG = [("Thigh", (0.5, -0.95, -0.3)), ("Shin", (0.55, -1.55, -1.15)), ("Foot", (0.5, -0.62, -1.75)), ("Toes", (0.55, -0.98, -2.55)), (None, (0.55, -1.25, -3.15))]
WRIST = (3.15, -2.5, 2.92)
WING = {
    "Wing_Upper": ((0.9, -1.95, 1.25), (1.55, -1.5, 1.55), "Chest"),
    "Wing_Fore": ((1.55, -1.5, 1.55), WRIST, "Wing_Upper"),
    "Wing_Hand": (WRIST, (3.45, -2.55, 2.95), "Wing_Fore"),
    "Wing_Thumb": ((3.15, -2.55, 2.95), (3.3, -2.95, 3.2), "Wing_Hand"),
    "Wing_Finger1_1": (WRIST, (5.5, -2.6, 2.95), "Wing_Hand"),
    "Wing_Finger1_2": ((5.5, -2.6, 2.95), (7.5, -2.5, 1.65), "Wing_Finger1_1"),
    "Wing_Finger2_1": (WRIST, (4.9, -2.02, 2.35), "Wing_Hand"),
    "Wing_Finger2_2": ((4.9, -2.02, 2.35), (6.35, -1.63, 1.15), "Wing_Finger2_1"),
    "Wing_Finger3_1": (WRIST, (4.54, -1.49, 1.6), "Wing_Hand"),
    "Wing_Finger3_2": ((4.54, -1.49, 1.6), (5.68, -0.66, 0.53), "Wing_Finger3_1"),
    "Wing_Finger4_1": (WRIST, (3.45, -0.99, 1.2), "Wing_Hand"),
    "Wing_Finger4_2": ((3.45, -0.99, 1.2), (3.54, -0.17, 0.26), "Wing_Finger4_1"),
}
for side, sx in (("L", 1.0), ("R", -1.0)):
    m = lambda p: (p[0] * sx, p[1], p[2])
    for i in range(4):
        name, h = LEG[i]
        t = LEG[i + 1][1]
        J[f"{name}.{side}"] = (m(h), m(t), "Pelvis" if i == 0 else f"{LEG[i - 1][0]}.{side}", True)
    for name, (h, t, parent) in WING.items():
        J[f"{name}.{side}"] = (m(h), m(t), parent if parent == "Chest" else f"{parent}.{side}", True)

arm_data = bpy.data.armatures.new("Skeleton")
rig = bpy.data.objects.new("Skeleton", arm_data)
scene.collection.objects.link(rig)
select_only(rig)
bpy.ops.object.mode_set(mode="EDIT")
for name, (h, t, parent, deform) in J.items():
    eb = arm_data.edit_bones.new(name)
    eb.head, eb.tail = warp(h), warp(t)
    d = (eb.tail - eb.head).normalized()
    # Rolled so each side mirrors the other: the wing's and the leg's local Z toward the back.
    up = Vector((0.0, 0.0, 1.0)) if abs(d.z) < 0.7 else Vector((0.0, 1.0, 0.0))
    eb.align_roll(up)
    eb.use_deform = deform
for name, (h, t, parent, deform) in J.items():
    if parent:
        eb = arm_data.edit_bones[name]
        eb.parent = arm_data.edit_bones[parent]
        eb.use_connect = (eb.head - eb.parent.tail).length < 1e-5
bpy.ops.object.mode_set(mode="OBJECT")
print("bones", len(arm_data.bones), "deform", sum(1 for b in arm_data.bones if b.use_deform))

# Skin by bone heat, as Blender's automatic weights, without the jaw, whose weights come from
# the side of the lips a vertex is on.
arm_data.bones["Jaw"].use_deform = False
for o in scene.objects:
    o.select_set(o in (mesh, rig))
bpy.context.view_layer.objects.active = rig
print("weights", bpy.ops.object.parent_set(type="ARMATURE_AUTO"))
arm_data.bones["Jaw"].use_deform = True
groups = {g.index: g.name for g in mesh.vertex_groups}
unweighted = [v.index for v in mesh.data.vertices if not any(g.weight > 1e-4 for g in v.groups)]
print("unweighted vertices", len(unweighted), "of", len(mesh.data.vertices))

jaw = mesh.vertex_groups.get("Jaw") or mesh.vertex_groups.new(name="Jaw")
head = mesh.vertex_groups["Head"]
moved = 0
for v in mesh.data.vertices:
    side = lip_side.get(v.index)
    if side is None:
        continue
    y = v.co.y + COM.y
    w_head = next((g.weight for g in v.groups if g.group == head.index), 0.0)
    if side > 0:
        continue
    # Below the lips: wholly the jaw's in front of the corners, fading to the head's by the
    # jaw's hinge; the corners half.
    share = 1.0 - smoothstep(MOUTH_CORNER[0] - 0.05, -5.0, y)
    if side == 0:
        share *= 0.5
    total = sum(g.weight for g in v.groups)
    if v.index in lining_verts or (abs(v.co.x) < 0.6 and v.co.z + COM.z > 0.55):
        # The floor of the mouth and the lower jaw: what the head held moves to the jaw.
        take = share * (w_head if total > 0 else 1.0)
        if total <= 0:
            head.add([v.index], 1.0 - take, "REPLACE")
        else:
            head.add([v.index], w_head - take, "REPLACE")
        jaw.add([v.index], take, "ADD")
        moved += 1
print("jaw vertices", moved)
# Each tooth moves as the lip it grows from.
verts = mesh.data.vertices
for t, root in tooth_root.items():
    for g in list(verts[t].groups):
        mesh.vertex_groups[g.group].remove([t])
    for g in verts[root].groups:
        mesh.vertex_groups[g.group].add([t], g.weight, "REPLACE")

select_only(mesh)
print("limit", bpy.ops.object.vertex_group_limit_total(group_select_mode="BONE_DEFORM", limit=4))
print("normalise", bpy.ops.object.vertex_group_normalize_all(group_select_mode="BONE_DEFORM", lock_active=False))
worst = max(len([g for g in v.groups if g.weight > 0]) for v in mesh.data.vertices)
print("most influences on a vertex", worst)

# The mouth's socket, where the game starts the fire: an empty on the head at the lips' front,
# its axes the model's (glTF +Z forward).
mouth = bpy.data.objects.new("Mouth", None)
scene.collection.objects.link(mouth)
mouth.empty_display_size = 0.3
mouth.parent = rig
mouth.parent_type = "BONE"
mouth.parent_bone = "Head"
mouth.matrix_world = Matrix.Translation(warp((0.0, -6.3, 1.03)))

# The clips --------------------------------------------------------------------------------
select_only(rig)
bpy.ops.object.mode_set(mode="POSE")
pose = rig.pose.bones
for pb in pose:
    pb.rotation_mode = "QUATERNION"
X, Y, Z = Vector((1, 0, 0)), Vector((0, 1, 0)), Vector((0, 0, 1))


def turn(name, axis, deg):
    """A turn of deg about an axis of the model's at rest, as the bone's own rotation."""
    rest = rig.data.bones[name].matrix_local.to_3x3()
    return Quaternion((rest.inverted() @ axis).normalized(), math.radians(deg))


def body_turn(name, pitch=0.0, yaw=0.0, roll=0.0):
    """pitch: nose (or that end of the bone) down; yaw: toward the dragon's left; roll: left
    side down, all in degrees about the model's axes."""
    return turn(name, Z, yaw) @ turn(name, X, pitch) @ turn(name, Y, -roll)


def wing_turn(name, side, elev=0.0, sweep=0.0, pitch=0.0):
    """elev: the wing up; sweep: forward, toward the head; pitch: its leading edge down; the
    same on both sides."""
    s = 1.0 if side == "L" else -1.0
    return turn(name, Z, -s * sweep) @ turn(name, Y, -s * elev) @ turn(name, X, pitch)


def leg_turn(name, side, pitch=0.0, spread=0.0):
    """pitch: the lower end back (toward the tail); spread: outward."""
    s = 1.0 if side == "L" else -1.0
    return turn(name, Y, -s * spread) @ turn(name, X, pitch)


def local_move(name, world):
    rest = rig.data.bones[name].matrix_local.to_3x3()
    return rest.inverted() @ Vector(world)


TAU = 2 * math.pi


def wave(x):
    return math.sin(TAU * x)


def stroke(t, down=0.55):
    """The beat's phase, a half for the downstroke, which takes `down` of the beat."""
    t %= 1.0
    return 0.5 * t / down if t < down else 0.5 + 0.5 * (t - down) / (1 - down)


def fold(s):
    """How folded the wing is, 0 to 1, through the upstroke, at phase s."""
    s %= 1.0
    return math.sin(math.pi * (s - 0.5) / 0.5) ** 2 if s > 0.5 else 0.0


SIDES = ("L", "R")
FINGERS = (1, 2, 3, 4)


def tucked_legs(p, amount=1.0):
    """The hind legs drawn up and back under the tail, as in flight."""
    for side in SIDES:
        p[f"Thigh.{side}"] = leg_turn(f"Thigh.{side}", side, 70 * amount, 3)
        p[f"Shin.{side}"] = leg_turn(f"Shin.{side}", side, -25 * amount)
        p[f"Foot.{side}"] = leg_turn(f"Foot.{side}", side, 75 * amount)
        p[f"Toes.{side}"] = leg_turn(f"Toes.{side}", side, 40 * amount)


def hanging_legs(p, sway):
    """The hind legs hanging, a little forward, swinging with the beat."""
    for side in SIDES:
        p[f"Thigh.{side}"] = leg_turn(f"Thigh.{side}", side, 15 + 4 * sway, 4)
        p[f"Shin.{side}"] = leg_turn(f"Shin.{side}", side, -10 - 3 * sway)
        p[f"Foot.{side}"] = leg_turn(f"Foot.{side}", side, 20 + 4 * sway)
        p[f"Toes.{side}"] = leg_turn(f"Toes.{side}", side, 25 + 5 * sway)


def beat(p, s, amp=1.0, lift=0.0, sweep_amp=12.0, fold_amp=1.0, spread=0.0):
    """One wing beat at phase s (0 the top of the stroke, 0.5 the bottom): the wrist leads,
    the fingers lag and the membrane follows; folded through the upstroke. `lift` raises the
    whole stroke, `spread` opens the hand."""
    c = math.cos(TAU * s)
    for side in SIDES:
        f_wrist = fold(s + 0.03) * fold_amp
        f_hand = fold(s - 0.02) * fold_amp
        p[f"Wing_Upper.{side}"] = wing_turn(f"Wing_Upper.{side}", side, elev=lift - 18 + 30 * amp * c, sweep=sweep_amp * wave(s) - 6 * f_wrist, pitch=8 * amp * wave(s))
        p[f"Wing_Fore.{side}"] = wing_turn(f"Wing_Fore.{side}", side, elev=-6 + 12 * amp * math.cos(TAU * (s - 0.05)), sweep=-22 * f_wrist)
        p[f"Wing_Hand.{side}"] = wing_turn(f"Wing_Hand.{side}", side, elev=18 - 22 * f_hand + 8 * amp * math.cos(TAU * (s - 0.1)), sweep=-30 * f_hand + spread)
        p[f"Wing_Thumb.{side}"] = wing_turn(f"Wing_Thumb.{side}", side, elev=10 * f_hand)
        for i in FINGERS:
            # The fingers bend up against the air on the downstroke and trail down on the way
            # up; they close together as the hand folds.
            lag = 0.12 + 0.03 * i
            bend = 6 * amp * wave(s - lag)
            close = (22 - 6 * i) * f_hand
            p[f"Wing_Finger{i}_1.{side}"] = wing_turn(f"Wing_Finger{i}_1.{side}", side, elev=bend, sweep=-close + spread * (1 - 0.3 * i))
            p[f"Wing_Finger{i}_2.{side}"] = wing_turn(f"Wing_Finger{i}_2.{side}", side, elev=14 + 8 * amp * wave(s - lag - 0.06) - 10 * f_hand)


def glide_wings(p, t, flutter=1.0):
    """Wings spread wide and held, with small corrections and the membrane fluttering."""
    for side in SIDES:
        k = 1.0 if side == "L" else -1.0
        p[f"Wing_Upper.{side}"] = wing_turn(f"Wing_Upper.{side}", side, elev=-24 + 1.5 * wave(t + 0.1 * k), sweep=2 + 1.0 * wave(2 * t), pitch=1.0 * wave(t))
        p[f"Wing_Fore.{side}"] = wing_turn(f"Wing_Fore.{side}", side, elev=-8 + 0.8 * wave(t + 0.2), sweep=3)
        p[f"Wing_Hand.{side}"] = wing_turn(f"Wing_Hand.{side}", side, elev=20, sweep=4)
        for i in FINGERS:
            p[f"Wing_Finger{i}_1.{side}"] = wing_turn(f"Wing_Finger{i}_1.{side}", side, elev=2 + 0.6 * flutter * wave(6 * t + 0.25 * i), sweep=4 - 2 * i)
            p[f"Wing_Finger{i}_2.{side}"] = wing_turn(f"Wing_Finger{i}_2.{side}", side, elev=16 + 1.2 * flutter * wave(6 * t + 0.25 * i + 0.15))


EVEN = (1 / 6,) * 6
REACH = (0.6, 0.25, 0.06, 0.04, 0.03, 0.02)  # bent at its root, straight beyond: reaching


def neck_wave(p, s, amp, base=0.0, yaw=0.0, head_pitch=None, sway=0.0, sway_t=0.0, shape=EVEN):
    """The neck: `base` pitch spread along it by `shape`, a travelling wave of `amp` riding the
    beat, and a side sweep; the head holds itself steady against the wave unless given its pitch."""
    total = 0.0
    for i in range(1, 7):
        pitch = base * shape[i - 1] + amp * wave(s - 0.08 - 0.05 * i)
        total += pitch
        p[f"Neck{i}"] = body_turn(f"Neck{i}", pitch=pitch, yaw=yaw / 6 + sway * wave(sway_t - 0.04 * i))
    p["Head"] = body_turn("Head", pitch=(-0.8 * (total - base) if head_pitch is None else head_pitch), yaw=sway * 0.8 * wave(sway_t - 0.3))


def tail_wave(p, s, amp, base=0.0, sway=0.0, sway_t=0.0):
    """The tail: `base` lift spread along it, a wave from the root to the tip, growing."""
    for i in range(1, 11):
        grow = 0.5 + 0.12 * i
        p[f"Tail{i}"] = body_turn(f"Tail{i}", pitch=-(base / 10 + amp * grow * wave(s - 0.15 - 0.06 * i)), yaw=sway * grow * wave(sway_t - 0.07 * i))


def body(p, pitch=0.0, bob=0.0, roll=0.0):
    p["Body"] = (body_turn("Body", pitch=pitch, roll=roll), local_move("Body", (0, 0, bob)))


def jaw(p, deg):
    p["Jaw"] = body_turn("Jaw", pitch=deg)


def fly(t):
    p = {}
    s = stroke(t)
    beat(p, s)
    body(p, pitch=-2 + 1.5 * wave(s - 0.1), bob=0.12 * wave(s - 0.05))
    neck_wave(p, s, 1.6, base=4)
    tail_wave(p, s, 1.2, base=-6)
    tucked_legs(p)
    return p


def glide(t):
    p = {}
    glide_wings(p, t)
    body(p, pitch=1.5 * wave(t), roll=1.5 * wave(t + 0.25), bob=0.04 * wave(t))
    neck_wave(p, t, 0.0, base=4, sway=1.2, sway_t=t)
    tail_wave(p, t, 0.6, base=-5, sway=0.8, sway_t=t + 0.2)
    tucked_legs(p)
    return p


HOVER_TILT = -20


def hover_base(p, s, tilt=HOVER_TILT, amp=1.0):
    """Hovering: the body tilted up, deep beats swept forward and back, legs hanging, tail low."""
    beat(p, s, amp=1.15 * amp, lift=6, sweep_amp=28 * amp, fold_amp=1.25)
    body(p, pitch=tilt + 2.5 * wave(s - 0.1), bob=0.18 * amp * wave(s - 0.05))
    hanging_legs(p, wave(s - 0.25))


def hover(t):
    p = {}
    s = stroke(t, 0.5)
    hover_base(p, s)
    neck_wave(p, s, 1.5, base=-HOVER_TILT * 0.7, head_pitch=-HOVER_TILT * 0.3)
    tail_wave(p, s, 1.6, base=HOVER_TILT * 0.5)
    return p


def breathe(t):
    """Two beats a loop; the neck stretched forward and down at the target, the jaw wide, the
    head sweeping once from side to side."""
    p = {}
    s = stroke(2 * t, 0.5)
    hover_base(p, s, tilt=0, amp=0.85)
    neck_wave(p, s, 0.5, base=50, head_pitch=-12, sway=2.2, sway_t=t, shape=REACH)
    tail_wave(p, s, 1.4, base=-6)
    jaw(p, 30 + 2 * wave(4 * t))
    return p


def fly_breathe(t):
    """A strafing pass, two shallow beats a loop, the neck straight forward and down, jaw open."""
    p = {}
    s = stroke(2 * t)
    beat(p, s, amp=0.6, sweep_amp=7, fold_amp=0.6)
    body(p, pitch=4, bob=0.07 * wave(s - 0.05))
    neck_wave(p, s, 0.4, base=32, head_pitch=-6, sway=1.2, sway_t=t, shape=REACH)
    tail_wave(p, s, 0.8, base=-4)
    tucked_legs(p)
    jaw(p, 28 + 2 * wave(4 * t))
    return p


def roar(t):
    """Three beats hovering; the head thrown up, the jaw opening wide and shaking, and closing."""
    p = {}
    s = stroke(3 * t, 0.5)
    hover_base(p, s)
    up = smoothstep(0.0, 0.2, t) * (1.0 - smoothstep(0.75, 1.0, t))
    neck_wave(p, s, 1.0, base=-HOVER_TILT * 0.7 - 24 * up, head_pitch=-HOVER_TILT * 0.3 - 16 * up)
    tail_wave(p, s, 1.6, base=HOVER_TILT * 0.5)
    jaw(p, 36 * up + 2.5 * up * wave(9 * t))
    return p


CLIPS = [
    # name, seconds, pose at t in [0, 1)
    ("Fly", 1.1, fly),
    ("Glide", 3.0, glide),
    ("Hover", 0.9, hover),
    ("Breathe", 2.0, breathe),
    ("FlyBreathe", 2.2, fly_breathe),
    ("Roar", 2.7, roar),
]
rig.animation_data_create()
for name, seconds, fn in CLIPS:
    frames = round(seconds * FPS)
    act = bpy.data.actions.new(name)
    rig.animation_data.action = act
    for f in range(frames + 1):
        p = fn(f / frames)
        for pb in pose:
            q, loc = p.get(pb.name, Quaternion()), Vector()
            if isinstance(q, tuple):
                q, loc = q
            pb.rotation_quaternion = q
            pb.location = loc
            pb.keyframe_insert("rotation_quaternion", frame=f)
            pb.keyframe_insert("location", frame=f)
    rig.animation_data.action = None
    track = rig.animation_data.nla_tracks.new()
    track.name = name
    strip = track.strips.new(name, 0, act)
    strip.name = name
    print("clip", name, "frames", frames, "seconds", round(frames / FPS, 3))


def evaluated_points():
    deps = bpy.context.evaluated_depsgraph_get()
    ev = mesh.evaluated_get(deps)
    me = ev.to_mesh()
    pts = [ev.matrix_world @ v.co for v in me.vertices]
    ev.to_mesh_clear()
    return pts


def extent(pts):
    lo = Vector([min(p[i] for p in pts) for i in range(3)])
    hi = Vector([max(p[i] for p in pts) for i in range(3)])
    return lo, hi


# Checks: the size at rest and the span through each clip.
for pb in pose:
    pb.matrix_basis.identity()
scene.frame_set(0)
lo, hi = extent([v.co for v in mesh.data.vertices])
print(f"rest size: span {hi.x - lo.x:.2f} m, length {hi.y - lo.y:.2f} m, height {hi.z - lo.z:.2f} m; lo {tuple(round(v, 2) for v in lo)} hi {tuple(round(v, 2) for v in hi)}")
for name, seconds, fn in CLIPS:
    act = bpy.data.actions[name]
    rig.animation_data.action = act
    if act.slots:
        rig.animation_data.action_slot = act.slots[0]
    frames = round(seconds * FPS)
    spans = []
    for f in range(0, frames, max(1, frames // 6)):
        scene.frame_set(f)
        lo, hi = extent(evaluated_points())
        spans.append(f"f{f}: span {hi.x - lo.x:.1f} z {lo.z:+.1f}..{hi.z:+.1f}")
    print("check", name, "; ".join(spans))
rig.animation_data.action = None
for pb in pose:
    pb.matrix_basis.identity()
scene.frame_set(0)
bpy.ops.object.mode_set(mode="OBJECT")
print("triangles", sum(len(p.vertices) - 2 for p in mesh.data.polygons))

for o in scene.objects:
    o.select_set(o in (mesh, rig, mouth))
bpy.ops.export_scene.gltf(
    filepath=out,
    export_format="GLB",
    use_selection=True,
    export_tangents=True,
    export_image_format="JPEG",
    export_jpeg_quality=90,
    export_animations=True,
    export_animation_mode="NLA_TRACKS",
    export_force_sampling=True,
    export_influence_nb=4,
)
print("EXPORTED", out, os.path.getsize(out))
