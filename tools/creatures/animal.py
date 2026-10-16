"""An animal for the game from its source art (art/creatures/<name>/): Tripo's textured model
made ours (welded, cut to its budget, turned to face +Z in glTF, at life size, its feet at the
origin), skinned on a skeleton of our own read from its creature.json, and given its clips,
each worked out from a few numbers by its body plan and keyed bone by bone.

    tools/blender/run.sh tools/creatures/animal.py red_squirrel [OUT.glb]

Two body plans, as Spore and Overgrowth animate many bodies with one set of rules
(research/2026-10-14-animation.md): a four-legged runner (`quadruped`) and a bird (`bird`).
Every clip is a pose a frame: each bone turned from its rest by a rotation given in the
animal's own axes (+X its left, -Y ahead, +Z up; a turn about +X pitches it nose down), and its
legs placed by two-bone inverse kinematics from where each foot is on the ground, worked out
here rather than by Blender's constraints, so a planted foot stays exactly where it was put.
Locomotion is in place: the game moves the animal at the clip's pace, which this prints.

creature.json says, in the model's units once turned (Tripo's are a metre across its longest
side), where each bone's head and tail are (`bones`; a bone ending `.L` is mirrored to `.R`),
which bones make each leg (`legs`: upper, lower, the foot that meets the ground, and toes),
and each clip's kind and numbers (`clips`). Check what it made with `sheet.py`.
"""

import json
import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "characters"))

import bmesh
import bpy
from mathutils import Matrix, Quaternion, Vector

import look

FPS = 24
argv = sys.argv[sys.argv.index("--") + 1:]
NAME = argv[0]
HOME = os.path.join(look.ART, "creatures", NAME)
with open(os.path.join(HOME, "creature.json")) as f:
    SPEC = json.load(f)
# A kind made from another's model textured again (`like`) takes that one's skeleton and clips,
# and its own model, size and anything else it gives.
if "like" in SPEC:
    with open(os.path.join(look.ART, "creatures", SPEC["like"], "creature.json")) as f:
        SPEC = {**json.load(f), **SPEC}
OUT = argv[1] if len(argv) > 1 else os.path.join(look.ASSETS, "models", "creatures", SPEC["file"] + ".glb")
SCALE = SPEC["scale"]
X, Y, Z = Vector((1, 0, 0)), Vector((0, 1, 0)), Vector((0, 0, 1))


def q(axis, deg):
    return Quaternion(Vector(axis).normalized(), math.radians(deg))


def smooth(a, b, x):
    t = min(max((x - a) / (b - a), 0.0), 1.0)
    return t * t * (3 - 2 * t)


# The skeleton as the file gives it, mirrored, in the model's turned units.
BONES = {}
for name, (parent, head, tail) in SPEC["bones"].items():
    BONES[name] = (parent, Vector(head), Vector(tail))
    if name.endswith(".L"):
        mirror = lambda v: Vector((-v.x, v.y, v.z))
        p = parent[:-2] + ".R" if parent and parent.endswith(".L") else parent
        BONES[name[:-2] + ".R"] = (p, mirror(Vector(head)), mirror(Vector(tail)))
LEGS = {}
for leg, chain in SPEC["legs"].items():
    LEGS[leg] = chain
    if leg.endswith(".L"):
        LEGS[leg[:-2] + ".R"] = [b[:-2] + ".R" for b in chain]
# The origin: under the middle of the feet, so the game stands it where it is.
feet = [BONES[chain[2]][2] for chain in LEGS.values()]
ORIGIN = Vector((0.0, sum(v.y for v in feet) / len(feet), 0.0))


def ours(v):
    """A point in the model's turned units, in metres from the origin."""
    return (Vector(v) - ORIGIN) * SCALE


# --- The mesh -------------------------------------------------------------------------------
look.reset()
scene = bpy.context.scene
scene.render.fps = FPS
bpy.ops.import_scene.gltf(filepath=os.path.join(HOME, "model.glb"))
mesh = next(o for o in scene.objects if o.type == "MESH")
mesh.data.transform(mesh.matrix_world)
mesh.parent = None
mesh.matrix_world = Matrix.Identity(4)
for m in list(mesh.modifiers):
    mesh.modifiers.remove(m)
mesh.vertex_groups.clear()
for o in list(scene.objects):
    if o is not mesh:
        bpy.data.objects.remove(o)
for a in list(bpy.data.actions):
    bpy.data.actions.remove(a)
mesh.name = mesh.data.name = SPEC["file"]


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
    bpy.ops.mesh.customdata_custom_splitnormals_clear()
tris = sum(len(p.vertices) - 2 for p in mesh.data.polygons)
cut = mesh.modifiers.new("Reduce", "DECIMATE")
cut.ratio = min(1.0, SPEC["budget"] / tris)
cut.use_collapse_triangulate = True
print("decimate", bpy.ops.object.modifier_apply(modifier=cut.name))
mesh.data.validate(clean_customdata=False)
M = (Matrix.Diagonal((SCALE, SCALE, SCALE, 1.0)) @ Matrix.Translation(-ORIGIN)
     @ Matrix.Rotation(math.radians(SPEC.get("turn", 0)), 4, "Z"))
mesh.data.transform(M)
mesh.data.update()
select_only(mesh)
print("smooth", bpy.ops.object.shade_smooth_by_angle(angle=math.radians(50.0), keep_sharp_edges=True))
sizes = SPEC.get("textures", {})
for image in bpy.data.images:
    w, h = image.size
    most = sizes.get("roughness_metal", 512) if image.name.lower().endswith("_rm") else sizes.get("colour", 1024)
    if max(w, h) > most:
        image.scale(w * most // max(w, h), h * most // max(w, h))
    print("texture", image.name, tuple(image.size))

# --- The skeleton ---------------------------------------------------------------------------
arm_data = bpy.data.armatures.new("Skeleton")
rig = bpy.data.objects.new("Skeleton", arm_data)
scene.collection.objects.link(rig)
select_only(rig)
bpy.ops.object.mode_set(mode="EDIT")
for name, (parent, head, tail) in BONES.items():
    eb = arm_data.edit_bones.new(name)
    eb.head, eb.tail = ours(head), ours(tail)
    d = (eb.tail - eb.head).normalized()
    side = X if abs(d.dot(X)) < 0.9 else Y
    eb.align_roll(side.cross(d).normalized())
for name, (parent, _, _) in BONES.items():
    if parent:
        arm_data.edit_bones[name].parent = arm_data.edit_bones[parent]
bpy.ops.object.mode_set(mode="OBJECT")


# Bone heat finds no solution on a body a few centimetres long (its tolerances are absolute), so
# the weights are worked out on a copy three metres long, of the same vertices in the same
# order, and copied back by index. If heat still leaves skin unweighted, as it does on a
# generated mesh that is not closed (the character skill), a closed voxel copy is weighted and
# its weights carried to the skin by nearest surface.
BIG = 3.0 / max(mesh.dimensions)


def heat(target_mesh, closed):
    """Weights for a mesh's vertices from bone heat on a big copy: {vertex: {bone: weight}}."""
    big = target_mesh.copy()
    big.data = target_mesh.data.copy()
    big.vertex_groups.clear()
    for m in list(big.modifiers):
        big.modifiers.remove(m)
    scene.collection.objects.link(big)
    big.parent = None
    big.matrix_world = Matrix.Identity(4)
    big.data.transform(Matrix.Diagonal((BIG, BIG, BIG, 1.0)))
    if closed:
        vox = big.modifiers.new("Voxel", "REMESH")
        vox.mode = "VOXEL"
        vox.voxel_size = max(big.dimensions) / 140
        select_only(big)
        bpy.ops.object.modifier_apply(modifier=vox.name)
    skel = rig.copy()
    skel.data = rig.data.copy()
    scene.collection.objects.link(skel)
    select_only(skel)
    bpy.ops.object.mode_set(mode="EDIT")
    for eb in skel.data.edit_bones:
        eb.head, eb.tail = eb.head * BIG, eb.tail * BIG
    bpy.ops.object.mode_set(mode="OBJECT")
    for o in scene.objects:
        o.select_set(o in (big, skel))
    bpy.context.view_layer.objects.active = skel
    print("heat", "closed copy" if closed else "skin", bpy.ops.object.parent_set(type="ARMATURE_AUTO"))
    if closed:
        # Carried to the skin, as big as the copy, by nearest surface.
        skin = target_mesh.copy()
        skin.data = target_mesh.data.copy()
        skin.vertex_groups.clear()
        for m in list(skin.modifiers):
            skin.modifiers.remove(m)
        scene.collection.objects.link(skin)
        skin.parent = None
        skin.matrix_world = Matrix.Identity(4)
        skin.data.transform(Matrix.Diagonal((BIG, BIG, BIG, 1.0)))
        for g in big.vertex_groups:
            skin.vertex_groups.new(name=g.name)
        tr = skin.modifiers.new("Carry", "DATA_TRANSFER")
        tr.object = big
        tr.use_vert_data = True
        tr.data_types_verts = {"VGROUP_WEIGHTS"}
        tr.vert_mapping = "POLYINTERP_NEAREST"
        select_only(skin)
        print("carry", bpy.ops.object.modifier_apply(modifier=tr.name))
        source = skin
    else:
        source = big
    names = {g.index: g.name for g in source.vertex_groups}
    weights = {v.index: {names[g.group]: g.weight for g in v.groups if g.weight > 1e-4} for v in source.data.vertices}
    for o in {big, source, skel}:
        bpy.data.objects.remove(o)
    return weights


weights = heat(mesh, closed=False)
bare = sum(1 for w in weights.values() if not w)
print("unweighted vertices", bare, "of", len(mesh.data.vertices))
if bare > 0.002 * len(mesh.data.vertices):
    weights = heat(mesh, closed=True)
    print("unweighted vertices from the closed copy", sum(1 for w in weights.values() if not w))
mesh.vertex_groups.clear()
groups = {b.name: mesh.vertex_groups.new(name=b.name) for b in rig.data.bones}
for v, ws in weights.items():
    for b, w in ws.items():
        groups[b].add([v], w, "REPLACE")
mesh.parent = rig
mesh.modifiers.new("Skeleton", "ARMATURE").object = rig
# Four bones a vertex at most, as Bevy reads.
select_only(mesh)
bpy.ops.object.vertex_group_limit_total(group_select_mode="ALL", limit=4)
bpy.ops.object.vertex_group_normalize_all(group_select_mode="ALL", lock_active=False)

# --- Posing ---------------------------------------------------------------------------------
select_only(rig)
bpy.ops.object.mode_set(mode="POSE")
pose_bones = rig.pose.bones
for pb in pose_bones:
    pb.rotation_mode = "QUATERNION"
REST = {b.name: b.matrix_local.copy() for b in rig.data.bones}
HEAD = {b.name: b.head_local.copy() for b in rig.data.bones}
TAIL = {b.name: b.tail_local.copy() for b in rig.data.bones}
PARENT = {b.name: (b.parent.name if b.parent else None) for b in rig.data.bones}
ORDER = []


def visit(bone):
    ORDER.append(bone.name)
    for child in bone.children:
        visit(child)


for b in rig.data.bones:
    if b.parent is None:
        visit(b)
IK_OF = {}
for leg, chain in LEGS.items():
    for i, b in enumerate(chain):
        IK_OF[b] = (leg, i)
CONTACT = {leg: TAIL[chain[2]].copy() for leg, chain in LEGS.items()}
clamped = [0]
# Each bone's scale in the world as the last pose solved left it, and the bones whose scale is
# keyed: a wing's root, which shrinks as it folds.
SC = {}
SCALED = {b for b in ("Humerus.L", "Humerus.R") if b in BONES}


def solve(pose):
    """Each bone's turn from rest and head, in the armature's space, for a pose: `root`, how
    far the root moves; `turn`, a bone's own turn after its parent's, in the animal's axes;
    `legs`, for each leg placed on the ground, where its foot's tip is and how its foot is
    turned; a leg not placed is turned like any other bone; `scale`, a bone grown or shrunk
    with all it carries, as a folded wing is (its world scale is left in `SC`)."""
    turn = pose.get("turn", {})
    placed = pose.get("legs", {})
    grown = pose.get("scale") or turn.get("__scale__", {})
    R, H = {}, {}
    SC.clear()
    for b in ORDER:
        p = PARENT[b]
        if b in IK_OF and IK_OF[b][0] in placed:
            leg, i = IK_OF[b]
            if i != 0:
                continue
            upper, lower, foot = LEGS[leg][:3]
            target, foot_turn = placed[leg]
            Rp, Hp = R[p], H[p]
            ha = Hp + Rp @ (HEAD[upper] - HEAD[p])
            # The foot is turned as asked, about its tip on the ground; the ankle is where that
            # puts its head.
            ankle = target - foot_turn @ (TAIL[foot] - HEAD[foot])
            a, bvec = HEAD[lower] - HEAD[upper], HEAD[foot] - HEAD[lower]
            l1, l2 = a.length, bvec.length
            reach = ankle - ha
            d = reach.length
            lo_, hi_ = abs(l1 - l2) + 1e-4, l1 + l2 - 1e-4
            if d > hi_ or d < lo_:
                clamped[0] += 1
                d = min(max(d, lo_), hi_)
            e1 = reach.normalized()
            n = Rp @ X
            e2 = n.cross(e1).normalized()
            rest_e1 = (HEAD[foot] - HEAD[upper]).normalized()
            side = 1.0 if a.dot(X.cross(rest_e1)) >= 0 else -1.0
            xk = (l1 * l1 - l2 * l2 + d * d) / (2 * d)
            yk = math.sqrt(max(l1 * l1 - xk * xk, 0.0))
            knee = ha + e1 * xk + e2 * yk * side
            Ra = (Rp @ a).rotation_difference(knee - ha) @ Rp
            ankle = ha + e1 * d
            Rb = (Ra @ bvec).rotation_difference(ankle - knee) @ Ra
            for leg_bone in LEGS[leg]:
                SC[leg_bone] = 1.0
            R[upper], H[upper] = Ra, ha
            R[lower], H[lower] = Rb, knee
            R[foot], H[foot] = foot_turn, ankle
            if len(LEGS[leg]) > 3:
                toe = LEGS[leg][3]
                R[toe] = foot_turn @ turn.get(toe, Quaternion())
                H[toe] = ankle + foot_turn @ (HEAD[toe] - HEAD[foot])
            continue
        if b in R:
            continue
        own = turn.get(b, Quaternion())
        if p is None:
            R[b] = own
            H[b] = HEAD[b] + pose.get("root", Vector())
            SC[b] = grown.get(b, 1.0)
        else:
            R[b] = R[p] @ own
            H[b] = H[p] + R[p] @ ((HEAD[b] - HEAD[p]) * SC[p])
            SC[b] = SC[p] * grown.get(b, 1.0)
    return R, H


def world(R, H, b):
    return (Matrix.Translation(H[b]) @ (R[b] @ REST[b].to_quaternion()).to_matrix().to_4x4()
            @ Matrix.Scale(SC.get(b, 1.0), 4))


def tip(R, H, b):
    return H[b] + R[b] @ ((TAIL[b] - HEAD[b]) * SC.get(b, 1.0))


def key(pose, frame):
    R, H = solve(pose)
    W = {b: world(R, H, b) for b in ORDER}
    for b in ORDER:
        p = PARENT[b]
        basis = REST[b].inverted() @ (REST[p] @ W[p].inverted() @ W[b] if p else W[b])
        loc, rot, size = basis.decompose()
        pb = pose_bones[b]
        pb.location = loc
        pb.rotation_quaternion = rot
        pb.keyframe_insert("location", frame=frame)
        pb.keyframe_insert("rotation_quaternion", frame=frame)
        if b in SCALED:
            pb.scale = size
            pb.keyframe_insert("scale", frame=frame)
    return R, H


def bones(prefix):
    return [b for b in ORDER if b.startswith(prefix)]


def side_of(b):
    return -1.0 if b.endswith(".R") else 1.0


def planted(**moved):
    """Every leg's foot where it stands at rest, unless moved."""
    out = {leg: (CONTACT[leg].copy(), Quaternion()) for leg in LEGS}
    out.update(moved)
    return out


def lowest(R, H, names):
    return min(tip(R, H, b).z for b in names)


TAILS = bones("Tail")
NECK = bones("Neck")
EARS = bones("Ear")
SPINE = [b for b in ("Spine", "Chest") if b in BONES]
PLAN = SPEC["plan"]
WINGS = {s: [b for b in ("Humerus" + s, "Forearm" + s, "Hand" + s) if b in BONES] for s in (".L", ".R")}



def looks(t, seconds, seed=0):
    """A look about: yaw, tilt, nod over time, held then turned quickly, as animals do."""
    marks = [(0.0, 0, 0, 0), (0.12, 0, 0, 0), (0.17, 35, -8, 4), (0.4, 35, -8, 4), (0.45, -28, 6, -4),
             (0.62, -28, 6, -4), (0.67, 10, 0, 8), (0.85, 10, 0, 8), (0.9, 0, 0, 0), (1.0, 0, 0, 0)]
    u = (t / seconds + seed * 0.37) % 1.0
    for (a, *va), (b, *vb) in zip(marks, marks[1:]):
        if a <= u <= b:
            k = smooth(0, 1, (u - a) / (b - a)) if b > a else 0
            return [x + (y - x) * k for x, y in zip(va, vb)]
    return [0, 0, 0]


def folded_wings(turn, amount=1.0):
    """The wings closed against the body: the arm back, the forearm forward over it, the hand
    back along the back, each turned so the feathers hang down the side (the bird plan). The
    ways are given in the body's own axes, `fold.dirs`: out, back along the body, up from it."""
    dirs = SPEC.get("fold", {}).get("dirs", [[0.3, 0.9, -0.3], [0.25, -1.0, 0.15], [0.12, 1.0, 0.1]])
    ahead = (TAIL[ORDER[0]] - HEAD[ORDER[0]]).normalized()
    above = ahead.cross(X).normalized()
    for s in (".L", ".R"):
        parent_rot = Quaternion()
        for b, dv in zip(WINGS[s], dirs):
            u0 = (TAIL[b] - HEAD[b]).normalized()
            c0 = (Y - u0 * u0.dot(Y)).normalized()
            u1 = (X * dv[0] * side_of(b) - ahead * dv[1] + above * dv[2]).normalized()
            c1 = (-above - u1 * u1.dot(-above)).normalized()
            f0 = Matrix((u0, c0, u0.cross(c0))).transposed()
            f1 = Matrix((u1, c1, u1.cross(c1))).transposed()
            want = Quaternion().slerp((f1 @ f0.inverted()).to_quaternion(), amount)
            turn[b] = parent_rot.inverted() @ want
            parent_rot = want
    # A spread wing's fan cannot close by turning alone, so the folded wing is drawn smaller
    # too, tucked against the side (`fold.scale`).
    shrink = 1.0 - (1.0 - SPEC.get("fold", {}).get("scale", 0.55)) * amount
    turn["__scale__"] = {WINGS[s][0]: shrink for s in (".L", ".R") if WINGS[s]}
    return turn


def bird_lean(turn, lean):
    """The body tipped forward by `lean`, level on the water, as the birds were made standing
    upright with their wings raised; the neck turned back as much, so the head stays up."""
    if lean:
        turn["Body"] = q(X, lean) @ turn.get("Body", Quaternion())
        for b in NECK[:1]:
            turn[b] = q(X, -lean) @ turn.get(b, Quaternion())
    return turn


def evaluated_low(frame, act):
    """The lowest point of the skin, posed at a frame of a clip."""
    rig.animation_data.action = act
    if act.slots:
        rig.animation_data.action_slot = act.slots[0]
    scene.frame_set(frame)
    deps = bpy.context.evaluated_depsgraph_get()
    ev = mesh.evaluated_get(deps)
    me = ev.to_mesh()
    low = min((ev.matrix_world @ v.co).z for v in me.vertices)
    ev.to_mesh_clear()
    return low


# --- Clips ----------------------------------------------------------------------------------
clips = {}
paces = {}


def new_clip(name):
    act = bpy.data.actions.new(name)
    rig.animation_data_create()
    rig.animation_data.action = act
    return act


def finish(name, act, frames):
    rig.animation_data.action = None
    track = rig.animation_data.nla_tracks.new()
    track.name = name
    strip = track.strips.new(name, 1, act)
    strip.name = name
    clips[name] = (act, frames)
    print("clip", name, "frames", frames, "seconds", round(frames / FPS, 3))


def idle_turn(t, seconds, c):
    turn = {}
    s = math.sin(2 * math.pi * t / 2.0)
    for b in SPINE[-1:]:
        turn[b] = q(X, 0.8 * s)
    yaw, tilt, nod = looks(t, seconds)
    neck = NECK or ["Head"]
    for b in neck:
        turn[b] = q(Z, yaw * 0.5 / len(neck)) @ q(X, nod * 0.3 / len(neck)) @ turn.get(b, Quaternion())
    turn["Head"] = q(Z, yaw * 0.5) @ q(Y, tilt) @ q(X, nod * 0.7)
    for e in EARS:
        flick = math.exp(-(((t % 2.3) - 1.1) / 0.06) ** 2) * 18 if e.endswith(".L") else math.exp(-(((t % 3.1) - 2.0) / 0.06) ** 2) * 18
        turn[e] = q(X, -flick)
    for i, b in enumerate(TAILS):
        turn[b] = q(Z, c.get("tail", 3) * math.sin(2 * math.pi * (t / 1.6 - i * 0.12)))
    return turn


def clip_idle(name, c):
    frames = round(c.get("seconds", 4.0) * FPS)
    act = new_clip(name)
    for f in range(1, frames + 2):
        t = (f - 1) / FPS
        turn = idle_turn(t, frames / FPS, c)
        if PLAN == "bird":
            folded_wings(turn)
            if "Tail" in BONES:
                turn["Tail"] = q(X, -6 * math.exp(-(((t % 1.7) - 0.8) / 0.05) ** 2))
        key({"turn": turn, "legs": planted(), "root": Vector((0, 0, 0.002 * math.sin(2 * math.pi * t / 2.0)))}, f)
    finish(name, act, frames)


def clip_gait(name, c):
    """Walking, trotting, bounding or hopping in place: each foot on the ground for `duty` of the
    cycle, sliding back at the pace, then lifted and swung ahead; `phases` say when each leg
    lands. `flex` arches and stretches the back twice a stride, as a bound does."""
    frames = round(c["cycle"] * FPS)
    stride, lift, duty = c["stride"] * SCALE, c["lift"] * SCALE, c["duty"]
    fold = c.get("fold", 45)
    act = new_clip(name)
    worst = 0.0
    for f in range(1, frames + 2):
        t = (f - 1) / frames
        legs = {}
        for leg in LEGS:
            p = (t - c["phases"].get(leg, 0.0)) % 1.0
            base = CONTACT[leg].copy()
            if p < duty:
                u = p / duty
                y = -stride / 2 + stride * u
                z = 0.0
                ang = c.get("peel", 25) * smooth(0.55, 1.0, u)
            else:
                v = (p - duty) / (1 - duty)
                e = 0.5 - 0.5 * math.cos(math.pi * v)
                y = stride / 2 - stride * e
                z = lift * math.sin(math.pi * v)
                ang = fold * math.sin(math.pi * min(v * 1.3, 1.0)) + c.get("peel", 25) * (1 - v) ** 3
            legs[leg] = (base + Vector((0, y, z)), q(X, ang))
        turn = {}
        flex = c.get("flex", 0.0) * math.sin(2 * math.pi * t)
        bob = c.get("bob", 0.0) * SCALE
        if c.get("flex"):
            # A bound: the back arches as the hind feet come under, stretches as they push.
            for b in SPINE:
                turn[b] = q(X, -flex / max(len(SPINE), 1))
            root = Vector((0, 0, bob * math.sin(2 * math.pi * (t - 0.15))))
            turn[ORDER[0]] = q(X, flex * 0.4)
        else:
            root = Vector((0, 0, -bob * math.cos(4 * math.pi * t)))
            turn[ORDER[0]] = q(Y, c.get("roll", 2.0) * math.sin(2 * math.pi * t))
        # The head held steadier than the body, nodding with the step.
        nod = c.get("nod", 3.0) * math.sin(4 * math.pi * t)
        for b in NECK:
            turn[b] = q(X, nod / max(len(NECK), 1))
        turn["Head"] = q(X, -nod * 0.6 + (flex * 0.5 if c.get("flex") else 0))
        for i, b in enumerate(TAILS):
            turn[b] = q(X, c.get("tail", 4) * math.sin(2 * math.pi * (t - 0.1 * i)))
        for e in EARS:
            turn[e] = q(X, c.get("ears", 0.0))
        if PLAN == "bird":
            folded_wings(turn)
        R, H = key({"turn": turn, "legs": legs, "root": root}, f)
        # The planted feet's tips go where they were put; any clamp would slide them.
        for leg, (target, _) in legs.items():
            worst = max(worst, (tip(R, H, LEGS[leg][2]) - target).length)
    finish(name, act, frames)
    paces[name] = stride / (duty * frames / FPS)
    print(f"pace {name}: {paces[name]:.3f} m/s, worst foot off its mark {worst * 100:.2f} cm")


def clip_graze(name, c):
    """The head down to the ground and nibbling, the feet where they stand: the root and back
    tip forward over the planted forelegs, which bend to let the chest down: the neck and head
    turn down first, then the body crouches and tips until the nose (or beak) just meets the
    ground."""
    frames = round(c.get("seconds", 3.0) * FPS)
    tip_bone = c.get("tip", "Head")

    def down(k):
        turn = {ORDER[0]: q(X, c.get("root", 0 if PLAN == "bird" else 5) * k)}
        for b in SPINE:
            turn[b] = q(X, c.get("spine", 20) * k / max(len(SPINE), 1))
        for b in NECK:
            turn[b] = q(X, c.get("neck", 55) * min(k, 1.0) / max(len(NECK), 1))
        turn["Head"] = q(X, c.get("head", 30) * min(k, 1.0))
        if PLAN == "bird":
            turn[ORDER[0]] = q(X, c.get("body", 30) * k)
            folded_wings(turn)
        return {"turn": turn, "legs": planted(), "root": Vector((0, 0, -c.get("crouch", 0.0) * SCALE * k))}

    reach = None
    for i in range(1, 101):
        k = i * 0.03
        R, H = solve(down(k))
        if tip(R, H, tip_bone).z <= 0.004 * SCALE / 0.3:
            reach = k
            break
    if reach is None:
        reach = 3.0
        print(name, "never reaches the ground; its nose stops", round(tip(R, H, tip_bone).z * 100, 1), "cm up")
    print(name, "reaches the ground at", round(reach, 2))
    act = new_clip(name)
    plan = c.get("plan", [(0, 0.0), (0.2, 1.0), (0.32, 0.9), (0.4, 1.0), (0.52, 0.92), (0.6, 1.0), (0.85, 1.0), (1.0, 0.0)])
    for f in range(1, frames + 2):
        u = (f - 1) / frames
        k = 0.0
        for (a, x), (b, y) in zip(plan, plan[1:]):
            if a <= u <= b:
                k = x + (y - x) * smooth(0, 1, (u - a) / (b - a))
        key(down(reach * k), f)
    finish(name, act, frames)


def clip_sit(name, c):
    """Sitting back on the haunches: the body tipped up about the hips, the hind legs folded under
    it; with `paws` "up", the forepaws raised to the mouth, nibbling, as a squirrel eats, else
    the forelegs straight on the ground, as a dog sits. `hold` keeps the pose to the end, for a
    clip to loop; otherwise it sits up and back down again."""
    frames = round(c.get("seconds", 3.0) * FPS)
    lean = c.get("lean", 55)
    raised = c.get("paws", "up") == "up"
    act = new_clip(name)
    fore = [leg for leg in LEGS if leg.startswith("Fore")]
    for f in range(1, frames + 2):
        t = (f - 1) / FPS
        u = (f - 1) / frames
        up = 1.0 if c.get("hold") else smooth(0.0, 0.25, u) * (1 - smooth(0.85, 1.0, u))
        turn = {ORDER[0]: q(X, -lean * up)}
        for b in SPINE:
            turn[b] = q(X, c.get("arch", -8) * up / max(len(SPINE), 1))
        nib = (4 * math.sin(2 * math.pi * t * 3.0) if raised else 0) * up
        for b in NECK:
            turn[b] = q(X, (lean * 0.55 * up) / max(len(NECK), 1))
        turn["Head"] = q(X, lean * 0.3 * up + nib)
        if raised:
            for leg in fore:
                upper, lower, paw = LEGS[leg][:3]
                turn[upper] = q(X, -(lean * 0.3 + 25) * up)
                turn[lower] = q(X, -95 * up)
                turn[paw] = q(X, 50 * up + nib)
        if c.get("hold"):
            yaw, tilt, nod = looks(t, frames / FPS)
            turn["Head"] = q(Z, yaw * 0.6) @ q(Y, tilt) @ turn["Head"]
        # The tail keeps its way behind as the body sits back.
        if TAILS:
            turn[TAILS[0]] = q(X, lean * c.get("tail_keep", 0.9) * up)
        root = Vector((0, 0, -c.get("drop", 0.04) * SCALE * up))
        legs = {leg: v for leg, v in planted().items() if not (raised and leg in fore) or up < 1e-3}
        # Seated, the hind feet come forward under the hips.
        for leg in legs:
            if leg.startswith("Hind"):
                target, rot = legs[leg]
                legs[leg] = (target + Vector((0, -c.get("tuck", 0.0) * SCALE * up, 0)), rot)
        key({"turn": turn, "legs": legs, "root": root}, f)
    finish(name, act, frames)


def clip_death(name, c):
    """Falling onto its side and lying still, lowered so its side rests on the ground."""
    frames = round(c.get("seconds", 1.0) * FPS)

    def keys(drops):
        for f in range(1, frames + 2):
            u = smooth(0.0, 1.0, (f - 1) / frames)
            turn = {ORDER[0]: q(Y, 88 * u)}
            for b in NECK:
                turn[b] = q(X, 15 * u / max(len(NECK), 1)) @ q(Y, 10 * u)
            for leg, chain in LEGS.items():
                turn[chain[0]] = q(X, (-15 if leg.startswith("Fore") else 15) * u)
                turn[chain[1]] = q(X, 10 * u)
            if PLAN == "bird":
                folded_wings(turn, 1.0 - 0.4 * u)
            key({"turn": turn, "root": Vector((0, 0, -drops[f - 1]))}, f)

    act = new_clip(name)
    keys([0.0] * (frames + 1))
    # Each frame lowered so its lowest skin is on the ground, as it falls and once down.
    lows = [evaluated_low(f, act) for f in range(1, frames + 2)]
    rig.animation_data.action = None
    bpy.data.actions.remove(act)
    act = new_clip(name)
    keys(lows)
    finish(name, act, frames)


def clip_fly(name, c):
    """Flapping flight, wings beating about the body's length, the forearm and hand following
    a little behind; legs tucked, or trailed behind as a heron's; with `glide`, a stretch with
    the wings held out, or with `bound` closed, as finches fly."""
    frames = round(c["cycle"] * FPS)
    act = new_clip(name)
    up, down = c.get("up", 10), c.get("down", 80)
    beats = c.get("beats", 1)
    for f in range(1, frames + 2):
        t = (f - 1) / frames
        beat_t = min(t / c.get("flapping", 1.0), 1.0)
        flapping = t < c.get("flapping", 1.0)
        turn = {ORDER[0]: q(X, c.get("body", 0))}
        for s in (".L", ".R"):
            sgn = side_of(s)
            chain = WINGS[s]
            for i, b in enumerate(chain):
                ph = 2 * math.pi * (beat_t * beats - 0.08 * i)
                a = (up + down) / 2 + (down - up) / 2 * math.sin(ph) if flapping else c.get("hold", 20)
                own = a if i == 0 else 0.3 * ((down - up) / 2) * math.sin(ph - 0.6) * (1 if flapping else 0)
                # The arm held out level from a body tipped level, then beating.
                turn[b] = (q(X, -c.get("body", 0)) if i == 0 else Quaternion()) @ q(Y, own * sgn)
            if c.get("bound") and not flapping:
                folded_wings(turn)
        for leg, chain in LEGS.items():
            tuck = c.get("tuck", [60, -110, 80])
            for b, ang in zip(chain, tuck):
                turn[b] = q(X, -ang)
        for b in NECK:
            turn[b] = q(X, c.get("neck", 0) / max(len(NECK), 1))
        turn["Head"] = q(X, c.get("head", 0))
        if "Tail" in BONES:
            turn["Tail"] = q(X, c.get("tail", 0))
        key({"turn": turn, "root": Vector((0, 0, c.get("rise", 0.0) * SCALE))}, f)
    finish(name, act, frames)


def clip_swim(name, c):
    """Afloat: the body still, the feet paddling under it in turn; with `dabble`, upending,
    head under and tail up."""
    frames = round(c["cycle"] * FPS)
    act = new_clip(name)
    for f in range(1, frames + 2):
        t = (f - 1) / frames
        legs = {}
        for leg in LEGS:
            p = (t - (0.5 if leg.endswith(".R") else 0.0)) % 1.0
            s = c.get("stroke", 0.08) * SCALE
            base = CONTACT[leg] + Vector((0, 0, c.get("tucked", 0.05) * SCALE))
            y = s * math.sin(2 * math.pi * p)
            z = 0.3 * s * math.cos(2 * math.pi * p)
            legs[leg] = (base + Vector((0, y, z)), q(X, 40 + 30 * math.sin(2 * math.pi * p)))
        turn = {}
        if c.get("dabble"):
            u = smooth(0, 0.25, t) * (1 - smooth(0.8, 1.0, t))
            turn[ORDER[0]] = q(X, c["dabble"] * u)
            for b in NECK:
                turn[b] = q(X, 20 * u / max(len(NECK), 1))
        else:
            yaw, _, nod = looks(t * frames / FPS, frames / FPS, 1)
            turn["Head"] = q(Z, yaw * 0.5) @ q(X, nod * 0.5)
        folded_wings(turn)
        bird_lean(turn, c.get("lean", 0.0))
        if "Tail" in BONES:
            turn["Tail"] = q(Z, 6 * math.sin(4 * math.pi * t))
        key({"turn": turn, "legs": legs}, f)
    finish(name, act, frames)




KINDS = {"idle": clip_idle, "gait": clip_gait, "graze": clip_graze, "sit": clip_sit, "death": clip_death,
         "fly": clip_fly, "swim": clip_swim}
for name, c in SPEC["clips"].items():
    KINDS[c["kind"]](name, c)
if clamped[0]:
    print("legs out of reach (clamped):", clamped[0])

# --- Checks and export ----------------------------------------------------------------------
rig.animation_data.action = None
for pb in pose_bones:
    pb.matrix_basis.identity()
bpy.ops.object.mode_set(mode="OBJECT")
for name, (act, frames) in clips.items():
    lows = [evaluated_low(f, act) for f in range(1, frames + 1, max(1, frames // 8))]
    print(f"check {name}: lowest skin {min(lows) * 100:+.1f} cm, highest lowest {max(lows) * 100:+.1f} cm")
rig.animation_data.action = None
scene.frame_set(1)
pts = [v.co for v in mesh.data.vertices]
lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
print("size m: across", round(hi.x - lo.x, 3), "long", round(hi.y - lo.y, 3), "high", round(hi.z - lo.z, 3))
print("triangles", sum(len(p.vertices) - 2 for p in mesh.data.polygons))
print("PACES", json.dumps({k: round(v, 3) for k, v in paces.items()}))

for o in scene.objects:
    o.select_set(o in (mesh, rig))
bpy.ops.export_scene.gltf(
    filepath=OUT,
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
print("EXPORTED", OUT, os.path.getsize(OUT))
