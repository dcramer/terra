"""Exports a rigged character as the game loads one: body, skeleton and our animation clips in
one .glb, with every clip sampled on this character's own skeleton.

Bevy plays a clip by setting each joint's whole transform, position included, so clips taken
from the mannequin's file would put our moved joints back where the mannequin's were.
Re-sampling the clips on this skeleton writes its own joint positions into them.

    spike/model_bakeoff/run.sh ../character_gear/export_character.py ../character_gear/out/NAME.glb ../character_gear/out/NAME-game.glb

make_character.sh runs it last.
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "model_bakeoff"))

import bpy

import look

FIGURE = os.path.join(HERE, "..", "..", "crates", "terra_sandbox", "assets", "models", "character", "figure.gltf")
src, dst = look.args()[:2]

look.reset()
bpy.ops.import_scene.gltf(filepath=os.path.join(look.HERE, src))
arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
body = max((o for o in bpy.context.scene.objects if o.type == "MESH"), key=lambda o: len(o.data.vertices))
for o in list(bpy.context.scene.objects):
    if o not in (arm, body):
        bpy.data.objects.remove(o)
arm.name = "Rig"
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
    o.select_set(o in (arm, body))
bpy.context.view_layer.objects.active = arm
bpy.ops.export_scene.gltf(
    filepath=os.path.join(look.HERE, dst), export_format="GLB", use_selection=True,
    export_animations=True, export_animation_mode="NLA_TRACKS", export_force_sampling=True,
    export_influence_nb=4,
)
print("EXPORTED", dst, len(actions), "clips")
