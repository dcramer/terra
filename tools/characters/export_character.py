"""Exports a rigged character as the game loads one: body, skeleton and our animation clips in
one .glb, with every clip sampled on this character's own skeleton.

Bevy plays a clip by setting each joint's whole transform, position included, so clips taken
from the mannequin's file would put our moved joints back where the mannequin's were.
Re-sampling the clips on this skeleton writes its own joint positions into them.

    tools/blender/run.sh tools/characters/export_character.py tools/characters/out/NAME.glb tools/characters/out/NAME-game.glb

make_character.sh runs it last.
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy
from mathutils import Matrix, Vector

import look
import patches
import twist

FIGURE = os.path.join(HERE, "..", "..", "assets", "models", "character", "figure.gltf")
src, dst = look.args()[:2]

look.reset()
bpy.ops.import_scene.gltf(filepath=look.path(src))
arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
# The twist bones' constraints, which glTF does not keep; the export bakes them into every clip.
twist.drive(arm)
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
    hand = region({f"DEF-hand.{s}"})
    knuckles = sum((joint(f"DEF-f_{f}.01.{s}") for f in ("index", "middle", "ring", "pinky")), Vector()) / 4
    wrist = joint(f"DEF-hand.{s}")
    palm = min(p.z for p in hand)
    at = knuckles.lerp(wrist, 0.2)
    at.z = palm - 0.015
    # Columns: where the socket's X, Y and Z point, in Blender's axes, chosen so that in
    # glTF +X is up out of the back of the T-posed hand, +Y forward and +Z toward the wrist.
    # The left hand mirrors the right, so its +X comes out of the palm instead.
    axes = ((0, 0, 1), (-1, 0, 0), (0, -1, 0)) if s == "R" else ((0, 0, -1), (1, 0, 0), (0, -1, 0))
    sockets.append(socket(f"socket.hand.{s}", f"DEF-hand.{s}", at, axes))
head = region({"DEF-head"})
top = max(head, key=lambda p: p.z)
centre = sum(head, Vector()) / len(head)
sockets.append(socket("socket.head", "DEF-head", Vector((centre.x, centre.y, top.z))))
chest = joint("DEF-spine.003")
back = [p for p in region({"DEF-spine.002", "DEF-spine.003"}) if abs(p.x) < 0.08 and abs(p.z - chest.z) < 0.08]
sockets.append(socket("socket.back", "DEF-spine.003", Vector((0, max(p.y for p in back), chest.z))))
hips = joint("DEF-hips")
for s, side in (("R", -1), ("L", 1)):
    waist = [p for p in region({"DEF-hips", "DEF-spine.001"}) if abs(p.z - (hips.z + 0.05)) < 0.03]
    edge = max(waist, key=lambda p: side * p.x)
    sockets.append(socket(f"socket.belt.{s}", "DEF-hips", Vector((edge.x, sum(p.y for p in waist) / len(waist), edge.z))))
print("SOCKETS", ", ".join(f"{e.name} {tuple(round(c, 3) for c in e.matrix_world.translation)}" for e in sockets))

# The body's patches, which what is worn can hide (patches.py).
patches.mark(body)
print("PATCHES", patches.PATCHES)

# The clips, from the mannequin's file; its objects are thrown away.
before = set(bpy.data.objects)
bpy.ops.import_scene.gltf(filepath=FIGURE)
for o in list(bpy.data.objects):
    if o not in before:
        bpy.data.objects.remove(o)
actions = [a for a in bpy.data.actions]
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
    export_influence_nb=4,
)
print("EXPORTED", dst, len(actions), "clips")
