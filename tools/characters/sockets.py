"""Prints where a body file's sockets and main joints stand at rest, in Blender's axes, as
SOCKETS lines: the reference the game's workbench `--joints` lines are checked against.

    tools/blender/run.sh tools/characters/sockets.py tools/characters/out/heavy-game.glb ... [--pose CLIP FRACTION]

With --pose, where they stand that far through a clip instead, to check the game's animated
skeleton against the tools' (the workbench's `pose` over the sandbox remote, then `find`).
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy

import look

args = look.args()
pose = None
if "--pose" in args:
    k = args.index("--pose")
    pose = (args[k + 1], float(args[k + 2]))
    args = args[:k] + args[k + 3:]
for path in args:
    look.reset()
    bpy.ops.import_scene.gltf(filepath=os.path.join(look.ROOT, path))
    arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
    if arm.animation_data:
        arm.animation_data.action = None
    for pb in arm.pose.bones:
        pb.matrix_basis.identity()
    if pose:
        action = bpy.data.actions[pose[0]]
        arm.animation_data_create()
        arm.animation_data.action = action
        if getattr(action, "slots", None) and len(action.slots):
            arm.animation_data.action_slot = action.slots[0]
        start, end = action.frame_range
        bpy.context.scene.frame_set(round(start + (end - start) * pose[1]))
    bpy.context.view_layer.update()
    name = look.body_name(path)
    for b in ("pelvis", "Head", "hand_r", "foot_r"):
        p = arm.matrix_world @ arm.pose.bones[b].head
        print("SOCKETS", name, b, f"({p.x:.3f}, {p.y:.3f}, {p.z:.3f})")
    for o in bpy.context.scene.objects:
        if o.type == "EMPTY" and o.name.startswith("socket."):
            p = o.matrix_world.translation
            print("SOCKETS", name, o.name, f"({p.x:.3f}, {p.y:.3f}, {p.z:.3f})")
