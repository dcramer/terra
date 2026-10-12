"""Forearm twist bones: without them, a forearm's turn in a clip all happens at the elbow, and
the skin blended across the elbow pinches to a thin neck (plain skinning's "candy wrapper").

A twist bone sits at the elbow, a child of the upper arm, and keeps aiming along the forearm
(a damped track, which follows the bend but not the turn). The upper part of the forearm's
skin moves to it, fading toward the wrist, so the turn spreads along the forearm as a real
one does. Our animations never move it; Blender's constraint does, and the export bakes that
into every clip, so the game plays it like any other bone.

glTF keeps the bone and its weights but not the constraint, so every script that loads a
character and plays it (the checker, the renders, the export) calls `drive` after loading.
"""

import bpy
from mathutils.geometry import intersect_point_line

SIDES = ("L", "R")
# How far down the forearm, from the elbow, the twist bone's share of the skin fades to none.
FADE = 0.75


def name(side):
    return f"DEF-forearm.twist.{side}"


def add(arm, body):
    """Adds the twist bones to `arm` and moves the upper forearm's weights onto them."""
    bpy.ops.object.select_all(action="DESELECT")
    arm.select_set(True)
    bpy.context.view_layer.objects.active = arm
    bpy.ops.object.mode_set(mode="EDIT")
    eb = arm.data.edit_bones
    for s in SIDES:
        forearm = eb[f"DEF-forearm.{s}"]
        bone = eb.new(name(s))
        bone.head = forearm.head
        bone.tail = forearm.head + (forearm.tail - forearm.head) * 0.5
        bone.roll = forearm.roll
        bone.parent = eb[f"DEF-upper_arm.{s}"]
        bone.use_deform = True
    bpy.ops.object.mode_set(mode="OBJECT")
    drive(arm)
    to_arm = arm.matrix_world.inverted() @ body.matrix_world
    for s in SIDES:
        forearm = arm.data.bones[f"DEF-forearm.{s}"]
        group = body.vertex_groups.get(f"DEF-forearm.{s}")
        if group is None:
            continue
        twist = body.vertex_groups.get(name(s)) or body.vertex_groups.new(name=name(s))
        for v in body.data.vertices:
            w = next((g.weight for g in v.groups if g.group == group.index), 0.0)
            if w <= 0:
                continue
            _, t = intersect_point_line(to_arm @ v.co, forearm.head_local, forearm.tail_local)
            share = max(0.0, min(1.0, 1.0 - t / FADE))
            if share > 0:
                group.add([v.index], w * (1 - share), "REPLACE")
                twist.add([v.index], w * share, "REPLACE")


def drive(arm):
    """Gives each twist bone its constraint: aim along the forearm, turning with the bend only."""
    for s in SIDES:
        bone = arm.pose.bones.get(name(s))
        if bone is None or any(c.type == "DAMPED_TRACK" for c in bone.constraints):
            continue
        track = bone.constraints.new("DAMPED_TRACK")
        track.target = arm
        track.subtarget = f"DEF-forearm.{s}"
        track.head_tail = 1.0
