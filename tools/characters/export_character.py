"""Exports a rigged character as the game loads one: body, skeleton and our animation clips in
one .glb, with every clip sampled on this character's own skeleton.

Bevy plays a clip by setting each joint's whole transform, position included, so clips taken
from the mannequin's file would put our moved joints back where the mannequin's were.
Re-sampling the clips on this skeleton writes its own joint positions into them.

    tools/blender/run.sh tools/characters/export_character.py tools/characters/out/NAME.glb tools/characters/out/NAME-game.glb

make_character.sh runs it last. With --morphs=heavy,thin,... it also gives the body those morphs
(made by make_morph.sh), as the game's base body is exported: see the MORPHS step below.
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import re

import bpy
import numpy as np
from mathutils import Matrix, Vector

import look
import patches
import twist

# --clips=FILE: the clips from another mannequin file cut by clips.py, such as a set of the
# libraries' hardest clips to check a body through; by default the game's.
FIGURE = next((look.path(a.split("=", 1)[1]) for a in look.args() if a.startswith("--clips=")), look.FIGURE)
src, dst = look.args()[:2]

look.reset()
bpy.ops.import_scene.gltf(filepath=look.path(src))
arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
body = max((o for o in bpy.context.scene.objects if o.type == "MESH"), key=lambda o: len(o.data.vertices))
for o in list(bpy.context.scene.objects):
    if o not in (arm, body):
        bpy.data.objects.remove(o)
arm.name = "Rig"
for pb in arm.pose.bones:
    pb.matrix_basis.identity()
bpy.context.view_layer.update()

# Sockets: named points on the skeleton that what is held and worn hangs from, placed from
# this body's own bones and skin so every body gets its own. The game finds them by name.
#   socket.hand.R/L  the middle of a closed fist. +Y runs along a held handle toward its
#                    working end (the thumb's way), +Z toward the wrist, and +X out of the
#                    back of the right hand or the palm of the left (they mirror).
#   socket.head      the top of the head; socket.back the middle of the upper back;
#   socket.belt.R/L  the side of the waist. These face as the body does: +Y up, +Z forward.
# Blender's Z is up and the body faces -Y; the exporter turns Y up, so in glTF a socket
# with Blender's identity rotation faces as the body does.
names = {g.index: g.name for g in body.vertex_groups}
mw = body.matrix_world


def region(bones):
    """Rest positions of the vertices whose strongest bone is one of `bones`."""
    pts = []
    for v in body.data.vertices:
        if v.groups:
            g = max(v.groups, key=lambda g: g.weight)
            if names.get(g.group) in bones:
                pts.append(mw @ v.co)
    return pts


def joint(name):
    return arm.matrix_world @ arm.data.bones[name].head_local


def socket(name, bone, at, axes=None):
    empty = bpy.data.objects.new(name, None)
    bpy.context.scene.collection.objects.link(empty)
    empty.empty_display_size = 0.05
    empty.parent = arm
    empty.parent_type = "BONE"
    empty.parent_bone = bone
    bpy.context.view_layer.update()
    m = Matrix.Translation(at)
    if axes:
        m = m @ Matrix((axes[0], axes[1], axes[2])).transposed().to_4x4()
    empty.matrix_world = m
    return empty


sockets = []
for s, side in (("R", -1), ("L", 1)):
    hand = region({f"hand_{s.lower()}"})
    knuckles = sum((joint(f"{f}_01_{s.lower()}") for f in ("index", "middle", "ring", "pinky")), Vector()) / 4
    wrist = joint(f"hand_{s.lower()}")
    palm = min(p.z for p in hand)
    at = knuckles.lerp(wrist, 0.2)
    at.z = palm - 0.015
    # Columns: where the socket's X, Y and Z point, in Blender's axes, chosen so that in
    # glTF +X is up out of the back of the T-posed hand, +Y forward and +Z toward the wrist.
    # The left hand mirrors the right, so its +X comes out of the palm instead.
    axes = ((0, 0, 1), (-1, 0, 0), (0, -1, 0)) if s == "R" else ((0, 0, -1), (1, 0, 0), (0, -1, 0))
    sockets.append(socket(f"socket.hand.{s}", f"hand_{s.lower()}", at, axes))
head = region({"Head"})
top = max(head, key=lambda p: p.z)
centre = sum(head, Vector()) / len(head)
sockets.append(socket("socket.head", "Head", Vector((centre.x, centre.y, top.z))))
chest = joint("spine_03")
back = [p for p in region({"spine_02", "spine_03"}) if abs(p.x) < 0.08 and abs(p.z - chest.z) < 0.08]
sockets.append(socket("socket.back", "spine_03", Vector((0, max(p.y for p in back), chest.z))))
hips = joint("pelvis")
for s, side in (("R", -1), ("L", 1)):
    waist = [p for p in region({"pelvis", "spine_01"}) if abs(p.z - (hips.z + 0.05)) < 0.03]
    edge = max(waist, key=lambda p: side * p.x)
    sockets.append(socket(f"socket.belt.{s}", "pelvis", Vector((edge.x, sum(p.y for p in waist) / len(waist), edge.z))))
print("SOCKETS", ", ".join(f"{e.name} {tuple(round(c, 3) for c in e.matrix_world.translation)}" for e in sockets))

# The body's patches, which what is worn can hide (patches.py).
# A body that is a shape of the base keeps the base's patches, so what a garment hides
# is the same skin on every body; only a new base cuts them.
if len(body.data.uv_layers) >= 2:
    print("PATCHES kept from the base")
else:
    patches.mark(body)
    print("PATCHES", patches.PATCHES)

# The base's morphs (D112, make_morph.sh), each a morph target of the body, in the order
# given. The game also needs to know where each morph moves the skeleton's joints and the
# sockets: those are read off the morph's own exported body (tools/characters/out/M-game.glb),
# as offsets in the model's space, glTF's axes (x, z, -y of Blender's), and kept on the
# skeleton's node as `{"morphs": {M: {node: [x, y, z]}}}` in its glTF extras.
MORPHS = next((a.split("=", 1)[1].split(",") for a in look.args() if a.startswith("--morphs=")), [])
if MORPHS:
    import json
    from mathutils.kdtree import KDTree

    world = [mw @ v.co for v in body.data.vertices]
    if body.data.shape_keys is None:
        body.shape_key_add(name="Basis", from_mix=False)
    node_world = {b.name: arm.matrix_world @ b.head_local for b in arm.data.bones}
    node_world.update({e.name: e.matrix_world.translation.copy() for e in sockets})
    offsets = {}
    for m in MORPHS:
        # Each array read once: an .npz reads an array from disk on every access.
        with np.load(os.path.join(look.OUT, f"{m}.npz")) as npz:
            data = {"points": npz["points"], "delta": npz["delta"]}
        kd = KDTree(len(data["points"]))
        for i, p in enumerate(data["points"]):
            kd.insert(Vector(p), i)
        kd.balance()
        key = body.shape_key_add(name=m, from_mix=False)
        inv = mw.inverted().to_3x3()
        for v, p in zip(body.data.vertices, world):
            _, i, _ = kd.find(p)
            key.data[v.index].co = v.co + inv @ Vector(data["delta"][i])
        before = set(bpy.data.objects)
        had = set(bpy.data.actions)
        bpy.ops.import_scene.gltf(filepath=os.path.join(look.OUT, f"{m}-game.glb"))
        new = [o for o in bpy.data.objects if o not in before]
        other = next(o for o in new if o.type == "ARMATURE")
        if other.animation_data:
            other.animation_data.action = None
        for pb in other.pose.bones:
            pb.matrix_basis.identity()
        bpy.context.view_layer.update()
        theirs = {b.name: other.matrix_world @ b.head_local for b in other.data.bones}
        # Its sockets import with Blender's ".001" on their names, ours being taken.
        theirs.update({re.sub(r"\.\d{3}$", "", o.name): o.matrix_world.translation.copy() for o in new if o.type == "EMPTY" and o.name.startswith("socket.")})
        moved = {}
        for n, p in node_world.items():
            if n in theirs:
                d = theirs[n] - p
                if d.length > 1e-5:
                    moved[n] = [round(d.x, 5), round(d.z, 5), round(-d.y, 5)]
        offsets[m] = moved
        for o in new:
            bpy.data.objects.remove(o)
        for a in [a for a in bpy.data.actions if a not in had]:
            bpy.data.actions.remove(a)
        print("MORPH", m, "target added;", len(moved), "joints and sockets move")
    arm["morphs"] = json.dumps(offsets)

# The clips, from the mannequin's file; its objects are thrown away.
before = set(bpy.data.objects)
bpy.ops.import_scene.gltf(filepath=FIGURE)
for o in list(bpy.data.objects):
    if o not in before:
        bpy.data.objects.remove(o)
actions = [a for a in bpy.data.actions]
# The limbs' roll moved to the hands and feet and spread over the twist bones (twist.py).
twist.bake(arm, actions)
# Each clip as a track on our skeleton, so the exporter samples all of them on it.
ad = arm.animation_data or arm.animation_data_create()
for a in actions:
    track = ad.nla_tracks.new()
    track.name = a.name
    strip = track.strips.new(a.name, int(a.frame_range[0]), a)
    if getattr(a, "slots", None) and len(a.slots):
        strip.action_slot = a.slots[0]
    track.mute = True
ad.action = None
for o in bpy.context.scene.objects:
    o.select_set(o in (arm, body) or o in sockets)
bpy.context.view_layer.objects.active = arm
bpy.ops.export_scene.gltf(
    filepath=look.path(dst), export_format="GLB", use_selection=True,
    export_animations=True, export_animation_mode="NLA_TRACKS", export_force_sampling=True,
    export_influence_nb=4, export_extras=True, export_morph=True,
)
print("EXPORTED", dst, len(actions), "clips")
