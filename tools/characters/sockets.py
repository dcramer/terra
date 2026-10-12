"""Prints where a body file's sockets and main joints stand at rest, in Blender's axes, as
SOCKETS lines: the reference the game's workbench `--joints` lines are checked against.

    tools/blender/run.sh tools/characters/sockets.py tools/characters/out/heavy-game.glb ...
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy

import look

for path in look.args():
    look.reset()
    bpy.ops.import_scene.gltf(filepath=os.path.join(look.ROOT, path))
    arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
    if arm.animation_data:
        arm.animation_data.action = None
    for pb in arm.pose.bones:
        pb.matrix_basis.identity()
    bpy.context.view_layer.update()
    name = look.body_name(path)
    for b in ("DEF-hips", "DEF-head", "DEF-hand.R", "DEF-foot.R"):
        p = arm.matrix_world @ arm.data.bones[b].head_local
        print("SOCKETS", name, b, f"({p.x:.3f}, {p.y:.3f}, {p.z:.3f})")
    for o in bpy.context.scene.objects:
        if o.type == "EMPTY" and o.name.startswith("socket."):
            p = o.matrix_world.translation
            print("SOCKETS", name, o.name, f"({p.x:.3f}, {p.y:.3f}, {p.z:.3f})")
