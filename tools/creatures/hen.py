"""The farmyard hen: Tripo's textured model made ours, on a skeleton of our own (Tripo's avian
auto-rig weighted its feet to a tail bone), with Idle, Walk and Peck keyed through leg IK and
baked to plain bone keys.

    tools/blender/run.sh tools/creatures/hen.py art/creatures/chicken/model.glb assets/models/creatures/Chicken.glb
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "characters"))

import bmesh
import bpy
from mathutils import Matrix, Quaternion, Vector

import look

src, out = sys.argv[sys.argv.index("--") + 1:][:2]
SCALE = 0.52
BUDGET = 2500
TEX = 1024
FPS = 24

look.reset()
scene = bpy.context.scene
scene.render.fps = FPS
bpy.ops.import_scene.gltf(filepath=src)
mesh = next(o for o in scene.objects if o.type == "MESH" and o.name.startswith("tripo"))
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
for arm in list(bpy.data.armatures):
    bpy.data.armatures.remove(arm)
mesh.name = "Chicken"
mesh.data.name = "Chicken"


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
cut.ratio = BUDGET / tris
cut.use_collapse_triangulate = True
print("decimate", bpy.ops.object.modifier_apply(modifier=cut.name))
print("validate fixed", mesh.data.validate(clean_customdata=False))

# Feet at the origin: the middle of the two feet's soles, in Tripo's frame (head toward -X).
vs = [v.co.copy() for v in mesh.data.vertices]
sole = [c for c in vs if c.z < 0.03]
foot_x = sum(c.x for c in sole) / len(sole)
ground = min(c.z for c in vs)
# Tripo's frame to ours: head from -X to -Y (glTF +Z), at life size, standing on the origin.
M = Matrix.Diagonal((SCALE, SCALE, SCALE, 1.0)) @ Matrix.Rotation(math.radians(90), 4, "Z") @ Matrix.Translation((-foot_x, 0.0, -ground))
mesh.data.transform(M)
mesh.data.update()
select_only(mesh)
print("smooth", bpy.ops.object.shade_smooth_by_angle(angle=math.radians(50.0), keep_sharp_edges=True))

# Textures: colour and normal at 1K, roughness-metal at half (art-style, Budgets; small prop).
for image in bpy.data.images:
    w, h = image.size
    most = TEX // 2 if image.name.lower().endswith("_rm") else TEX
    if max(w, h) > most:
        image.scale(w * most // max(w, h), h * most // max(w, h))
    print("texture", image.name, tuple(image.size))


def P(x, y, z):
    """A point given in Tripo's frame (before M), in ours."""
    return M @ Vector((x, y, z))


# Joints, in Tripo's frame, read off the model's profile. Tripo's +Y is the hen's right.
JOINTS = {
    # name: (head, tail, parent, deform)
    "Body": ((0.0, 0.0, 0.40), (0.0, 0.0, 0.58), None, True),
    "Neck1": ((-0.20, 0.0, 0.58), (-0.26, 0.0, 0.72), "Body", True),
    "Neck2": ((-0.26, 0.0, 0.72), (-0.27, 0.0, 0.83), "Neck1", True),
    "Head": ((-0.27, 0.0, 0.83), (-0.446, 0.0, 0.835), "Neck2", True),
    "Tail": ((0.24, 0.0, 0.56), (0.42, 0.0, 0.84), "Body", True),
    "Wing.L": ((-0.15, -0.19, 0.50), (0.17, -0.21, 0.36), "Body", True),
    "Wing.R": ((-0.15, 0.19, 0.50), (0.17, 0.21, 0.36), "Body", True),
}
for side, y in (("L", -1.0), ("R", 1.0)):
    JOINTS[f"Thigh.{side}"] = ((0.0, 0.08 * y, 0.40), (0.07, 0.11 * y, 0.19), "Body", True)
    JOINTS[f"Shank.{side}"] = ((0.07, 0.11 * y, 0.19), (0.02, 0.115 * y, 0.03), f"Thigh.{side}", True)
    JOINTS[f"Foot.{side}"] = ((0.02, 0.115 * y, 0.03), (-0.15, 0.12 * y, 0.01), f"Shank.{side}", True)
    # Controls, removed once the clips are baked.
    JOINTS[f"IK.{side}"] = ((0.02, 0.115 * y, 0.03), (-0.15, 0.12 * y, 0.01), None, False)

arm_data = bpy.data.armatures.new("Skeleton")
rig = bpy.data.objects.new("Skeleton", arm_data)
scene.collection.objects.link(rig)
select_only(rig)
bpy.ops.object.mode_set(mode="EDIT")
LATERAL = Vector((1.0, 0.0, 0.0))
for name, (h, t, parent, deform) in JOINTS.items():
    eb = arm_data.edit_bones.new(name)
    eb.head, eb.tail = P(*h), P(*t)
    d = (eb.tail - eb.head).normalized()
    # Local X along the hen's left (+X), so a pitch is a turn about local X.
    z = LATERAL.cross(d)
    if z.length > 1e-4:
        eb.align_roll(z.normalized())
    eb.use_deform = deform
for name, (h, t, parent, deform) in JOINTS.items():
    if parent:
        eb = arm_data.edit_bones[name]
        eb.parent = arm_data.edit_bones[parent]
        eb.use_connect = (eb.head - eb.parent.tail).length < 1e-5
bpy.ops.object.mode_set(mode="OBJECT")

# Skin by bone heat, as Blender's automatic weights; deform bones only.
for o in scene.objects:
    o.select_set(o in (mesh, rig))
bpy.context.view_layer.objects.active = rig
print("weights", bpy.ops.object.parent_set(type="ARMATURE_AUTO"))
unweighted = sum(1 for v in mesh.data.vertices if not any(g.weight > 1e-4 for g in v.groups))
print("unweighted vertices", unweighted, "of", len(mesh.data.vertices))
for g in list(mesh.vertex_groups):
    if g.name.startswith("IK."):
        mesh.vertex_groups.remove(g)

# Leg IK: each shank reaches its IK control, flexing only about local X; each foot turns
# as its control does, so it stays flat on the ground.
select_only(rig)
bpy.ops.object.mode_set(mode="POSE")
pose = rig.pose.bones
for side in ("L", "R"):
    ik = pose[f"Shank.{side}"].constraints.new("IK")
    ik.target, ik.subtarget, ik.chain_count = rig, f"IK.{side}", 2
    for b in (f"Thigh.{side}", f"Shank.{side}"):
        pose[b].lock_ik_y = pose[b].lock_ik_z = True
    cr = pose[f"Foot.{side}"].constraints.new("COPY_ROTATION")
    cr.target, cr.subtarget = rig, f"IK.{side}"
for pb in pose:
    pb.rotation_mode = "QUATERNION"


def pitch(deg):
    """Nose down by deg, about the hen's left (local X on every bone here)."""
    return Quaternion((1.0, 0.0, 0.0), math.radians(deg))


def axis_local(name, axis_world):
    rest = rig.data.bones[name].matrix_local.to_3x3()
    return rest.inverted() @ Vector(axis_world)


def turn(name, axis_world, deg):
    return Quaternion(axis_local(name, axis_world), math.radians(deg))


def key(name, frame, rot=None, loc=None):
    pb = pose[name]
    if rot is not None:
        pb.rotation_quaternion = rot
        pb.keyframe_insert("rotation_quaternion", frame=frame)
    if loc is not None:
        pb.location = loc
        pb.keyframe_insert("location", frame=frame)


def loc_local(name, world):
    """A move given in the armature's axes, as the bone's own location."""
    rest = rig.data.bones[name].matrix_local.to_3x3()
    return rest.inverted() @ Vector(world)


def rest_all(frame):
    for pb in pose:
        key(pb.name, frame, rot=Quaternion(), loc=Vector())


def new_clip(name, length):
    act = bpy.data.actions.new(name + ".keys")
    rig.animation_data_create()
    rig.animation_data.action = act
    for pb in pose:
        pb.rotation_quaternion = Quaternion()
        pb.location = Vector()
    return act


clips = {}

# Idle, 4 s: breathing, and the hen looking about with quick turns and holds.
IDLE = 96
new_clip("Idle", IDLE)
rest_all(1)
rest_all(IDLE + 1)
for f in range(1, IDLE + 2, 4):
    s = math.sin(2 * math.pi * (f - 1) / 48.0)
    key("Body", f, rot=pitch(1.2 * s), loc=loc_local("Body", (0, 0, 0.002 * s)))
    key("Tail", f, rot=pitch(-2.0 * s))
    for side, sgn in (("L", 1), ("R", -1)):
        key(f"Wing.{side}", f, rot=turn(f"Wing.{side}", (0, 1, 0), sgn * 1.5 * s))
looks = [(1, 0, 0, 0), (14, 0, 0, 0), (17, 38, -12, 4), (40, 38, -12, 4), (43, -25, 6, -6), (62, -25, 6, -6), (65, 8, 0, 10), (84, 8, 0, 10), (88, 0, 0, 0), (97, 0, 0, 0)]
for f, yaw, tilt, nod in looks:
    head = turn("Head", (0, 0, 1), yaw * 0.6) @ turn("Head", (0, -1, 0), tilt) @ pitch(nod * 0.5)
    key("Head", f, rot=head)
    key("Neck2", f, rot=turn("Neck2", (0, 0, 1), yaw * 0.4) @ pitch(nod * 0.5))
clips["Idle"] = (rig.animation_data.action, IDLE)

# Walk, one stride of both feet: each foot on the ground 60% of the cycle, sliding back at the
# walking speed, then lifted with its toes curled and swung forward. In place: the game moves
# the hen at STRIDE / cycle.
CYCLE = 16
STRIDE = 0.15
LIFT = 0.045
DROP = 0.018
ANKLE = P(0.02, 0.0, 0.03).z
TOE = (P(-0.15, 0.0, 0.01) - P(0.02, 0.0, 0.03)).length
new_clip("Walk", CYCLE)
for f in range(1, CYCLE + 2):
    t = (f - 1) / CYCLE
    for side, offset in (("L", 0.0), ("R", 0.5)):
        p = (t + offset) % 1.0
        if p < 0.6:
            u = p / 0.6
            y = -STRIDE / 2 + STRIDE * u
            z = 0.0
            curl = 0.0
        else:
            u = (p - 0.6) / 0.4
            e = 0.5 - 0.5 * math.cos(math.pi * u)
            y = STRIDE / 2 - STRIDE * e
            z = LIFT * math.sin(math.pi * u)
            # Toes curl once off the ground, never reaching below it.
            curl = 0.0
            for c in range(0, 46):
                th = math.radians(c)
                if ANKLE + z - ANKLE * math.cos(th) - TOE * math.sin(th) >= 0.003:
                    curl = float(c)
        key(f"IK.{side}", f, loc=loc_local(f"IK.{side}", (0, y, z)), rot=pitch(curl))
    # Body: highest at mid-stance, twice a cycle; rolling over the foot on the ground.
    bob = -DROP + 0.006 * math.cos(4 * math.pi * t)
    roll = 3.0 * math.sin(2 * math.pi * t)
    key("Body", f, rot=pitch(4.0) @ turn("Body", (0, -1, 0), roll), loc=loc_local("Body", (0, 0, bob)))
    # The head held still in the air, then thrust forward: twice a cycle.
    h = (2 * t) % 1.0
    thrust = (h / 0.75) if h < 0.75 else 1.0 - (h - 0.75) / 0.25
    neck = 8.0 - 16.0 * thrust
    key("Neck1", f, rot=pitch(neck))
    key("Neck2", f, rot=pitch(-neck * 0.3))
    key("Head", f, rot=pitch(-neck * 0.7 - 4.0))
    key("Tail", f, rot=pitch(-3.0 * math.cos(4 * math.pi * t)))
    for side, sgn in (("L", 1), ("R", -1)):
        key(f"Wing.{side}", f, rot=turn(f"Wing.{side}", (0, 1, 0), sgn * 2.0 * math.sin(4 * math.pi * t)))
clips["Walk"] = (rig.animation_data.action, CYCLE)


def evaluated_min(names, frame):
    """The lowest point of the vertices weighted mostly to the bones named, posed at frame."""
    scene.frame_set(frame)
    deps = bpy.context.evaluated_depsgraph_get()
    ev = mesh.evaluated_get(deps)
    me = ev.to_mesh()
    idx = {mesh.vertex_groups[n].index for n in names}
    low = 9.0
    for v_src, v in zip(mesh.data.vertices, me.vertices):
        if v_src.groups and max(v_src.groups, key=lambda g: g.weight).group in idx:
            low = min(low, (ev.matrix_world @ v.co).z)
    ev.to_mesh_clear()
    return low


# Peck, 2 s: down, two quick pecks at the ground, up. The reach is found so the beak's tip
# just meets the ground.
PECK = 48


def peck_pose(k):
    # The body tips, the neck points down, and the head turns back up against it so the beak
    # meets the ground point first rather than curling under.
    return {"Body": pitch(35 * k), "Neck1": pitch(55 * k), "Neck2": pitch(55 * k), "Head": pitch(-70 * k), "Tail": pitch(-15 * k)}


new_clip("Peck", PECK)
reach, low = None, None
for i in range(4, 31):
    k = i * 0.05
    for b, q in peck_pose(k).items():
        key(b, 1, rot=q)
    low = evaluated_min(["Head"], 1)
    print(f"peck k {k:.2f}: beak low {low:+.4f}")
    if low <= 0.004:
        reach = k
        break
print("peck reach", reach, "beak low", round(low, 4))
act = rig.animation_data.action
rig.animation_data.action = None
bpy.data.actions.remove(act)
new_clip("Peck", PECK)
hover = 0.82
plan = [(1, 0.0), (11, hover), (14, 1.0), (17, hover), (20, 1.0), (23, hover), (33, 0.15), (40, 0.0), (49, 0.0)]
for f, amount in plan:
    for b, q in peck_pose(reach * amount).items():
        key(b, f, rot=q)
    for side in ("L", "R"):
        key(f"IK.{side}", f, rot=Quaternion(), loc=Vector())
clips["Peck"] = (rig.animation_data.action, PECK)

# Bake each clip's posed deform bones to plain keys, then drop the controls.
deform = [b.name for b in rig.data.bones if b.use_deform]
baked = {}
for name, (act, length) in clips.items():
    rig.animation_data.action = act
    frames = {}
    for f in range(1, length + 2):
        scene.frame_set(f)
        frames[f] = {b: rig.convert_space(pose_bone=pose[b], matrix=pose[b].matrix, from_space="POSE", to_space="LOCAL") for b in deform}
    baked[name] = frames
rig.animation_data.action = None
for pb in pose:
    for c in list(pb.constraints):
        pb.constraints.remove(c)
for act, _ in clips.values():
    bpy.data.actions.remove(act)
bpy.ops.object.mode_set(mode="EDIT")
for side in ("L", "R"):
    arm_data.edit_bones.remove(arm_data.edit_bones[f"IK.{side}"])
bpy.ops.object.mode_set(mode="POSE")
pose = rig.pose.bones
for pb in pose:
    pb.rotation_mode = "QUATERNION"
track_of = {}
for name, frames in baked.items():
    act = bpy.data.actions.new(name)
    rig.animation_data.action = act
    for f, mats in frames.items():
        for b, m in mats.items():
            loc, rot, _ = m.decompose()
            pose[b].location = loc
            pose[b].rotation_quaternion = rot
            pose[b].keyframe_insert("location", frame=f)
            pose[b].keyframe_insert("rotation_quaternion", frame=f)
    rig.animation_data.action = None
    track = rig.animation_data.nla_tracks.new()
    track.name = name
    strip = track.strips.new(name, 1, act)
    strip.name = name
    track_of[name] = (act, len(frames) - 1)
    print("clip", name, "frames", len(frames) - 1, "seconds", round((len(frames) - 1) / FPS, 3))

# Checks on the baked clips: the lowest foot never through the ground, a planted foot still
# while the body moves, the beak reaching the ground in the peck.
for name, (act, length) in track_of.items():
    rig.animation_data.action = act
    lows = [evaluated_min(["Foot.L", "Foot.R"], f) for f in range(1, length + 1)]
    feet_low = min(lows)
    if name == "Walk":
        print("walk feet low by frame", [round(v, 3) for v in lows])
    head_low = min(evaluated_min(["Head"], f) for f in range(1, length + 1, 2))
    print(f"check {name}: feet lowest {feet_low:+.4f} m, head lowest {head_low:+.4f} m")
rig.animation_data.action = None
scene.frame_set(1)
for pb in pose:
    pb.matrix_basis.identity()
bpy.ops.object.mode_set(mode="OBJECT")

pts = [v.co for v in mesh.data.vertices]
lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
print("size (blender) x", round(hi.x - lo.x, 3), "y (length)", round(hi.y - lo.y, 3), "z (height)", round(hi.z - lo.z, 3), "lo", tuple(round(v, 3) for v in lo), "hi", tuple(round(v, 3) for v in hi))
band = [p.z for p in pts if abs(p.y - (lo.y + hi.y) / 2) < 0.03]
print("back height at mid-length", round(max(band), 3))
print("triangles", sum(len(p.vertices) - 2 for p in mesh.data.polygons))
print("head toward", "-Y (glTF +Z)" if sum(p.y for p in pts if p.z > hi.z - 0.05) < 0 else "+Y")

for o in scene.objects:
    o.select_set(o in (mesh, rig))
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
