"""Tests of hold.py's aim and roll, which read a bone's joint line (to its main child), not its
+Y, in Blender's Python:

    tools/blender/run.sh tools/characters/test_hold.py

A bone whose +Y is off its joint line, as Quaternius's clavicles are by 12 degrees (D599), is
aimed so its line takes the way asked, and rolled about that line so its child stays put. On
our converted base, whose bones point at their children, every hold of the fight's list comes
out as it did when aim and roll read +Y.
"""

import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy  # noqa: E402
from mathutils import Vector  # noqa: E402

import hold  # noqa: E402
import look  # noqa: E402

ROOT = os.path.join(HERE, "..", "..")
FIGHT = os.path.join(ROOT, "assets", "models", "character", "fight.glb")
LIST = os.path.join(ROOT, "art", "animation", "sets", "fight.json")


def plus_y(arm, name, children=None):
    """What aim and roll read before: the bone's own +Y."""
    pb = arm.pose.bones[name]
    return (pb.tail - pb.head).normalized()


def skewed():
    """A clavicle whose +Y is 12 degrees off the line to its upper arm, the arm hanging down
    and out from it with the forearm bent forward, with a one-frame clip of its rest."""
    look.reset()
    bpy.ops.object.armature_add(enter_editmode=True)
    arm = bpy.context.object
    eb = arm.data.edit_bones
    eb.remove(eb[0])
    off = math.radians(12)
    cl = eb.new("clavicle_l")
    cl.head, cl.tail = Vector((0.02, 0, 1.45)), Vector((0.02 + 0.15 * math.cos(off), 0, 1.45 + 0.15 * math.sin(off)))
    up = eb.new("upperarm_l")
    up.head, up.tail, up.parent = Vector((0.17, 0, 1.45)), Vector((0.40, 0, 1.20)), cl
    lo = eb.new("lowerarm_l")
    lo.head, lo.tail, lo.parent = Vector((0.40, 0, 1.20)), Vector((0.40, -0.25, 1.20)), up
    bpy.ops.object.mode_set(mode="POSE")
    for pb in arm.pose.bones:
        pb.rotation_mode = "QUATERNION"
        pb.keyframe_insert("rotation_quaternion", frame=1)
    arm.animation_data.action.name = "Rest"
    bpy.context.view_layer.update()
    return arm


def pose_out(arm, at):
    for n, (loc, rot, scale) in at.items():
        pb = arm.pose.bones[n]
        pb.location, pb.rotation_quaternion, pb.scale = loc, rot, scale
    bpy.context.view_layer.update()


def test_a_skewed_bone_is_aimed_along_its_joint_line():
    arm = skewed()
    want_gl = (0.3, -0.8, 0.5)
    held = {"clip": "Rest", "bone": "clavicle_l", "aim": {"clavicle_l": list(want_gl)}}
    at = hold._held_pose(arm, {"Rest": arm.animation_data.action}, "t", held)
    pose_out(arm, at)
    want = Vector((want_gl[0], -want_gl[2], want_gl[1])).normalized()
    p = arm.pose.bones
    got = (p["upperarm_l"].head - p["clavicle_l"].head).normalized()
    assert math.degrees(got.angle(want)) < 1e-3, math.degrees(got.angle(want))


def test_a_skewed_bone_rolls_about_its_joint_line():
    arm = skewed()
    p = arm.pose.bones
    child = p["upperarm_l"].head.copy()
    hand = p["lowerarm_l"].tail.copy()
    held = {"clip": "Rest", "bone": "clavicle_l", "roll": {"clavicle_l": 47}}
    at = hold._held_pose(arm, {"Rest": arm.animation_data.action}, "t", held)
    pose_out(arm, at)
    # The child's joint lies on the axis, so it stays; the arm beyond swings round it.
    assert (p["upperarm_l"].head - child).length < 1e-6, (p["upperarm_l"].head - child).length
    assert (p["lowerarm_l"].tail - hand).length > 0.05


def test_the_fight_holds_on_our_base_are_unchanged():
    look.reset()
    bpy.ops.import_scene.gltf(filepath=look.path(FIGHT))
    arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
    by_name = {a.name: a for a in bpy.data.actions}
    with open(LIST) as f:
        entries = json.load(f)
    joint_line = hold.line
    worst, n = 0.0, 0
    for name, entry in entries.items():
        holds = entry.get("hold", [])
        for held in holds if isinstance(holds, list) else [holds]:
            if not (held.get("aim") or held.get("roll")) or held.get("clip") not in by_name:
                continue
            outs = []
            for fn in (joint_line, plus_y):
                hold.line = fn
                for pb in arm.pose.bones:
                    pb.matrix_basis.identity()
                outs.append(hold._held_pose(arm, by_name, name, held, entries))
            hold.line = joint_line
            for b, (loc, rot, _) in outs[0].items():
                loc0, rot0, _ = outs[1][b]
                d = max((loc - loc0).length, math.degrees(rot.rotation_difference(rot0).angle) / 100.0)
                if d > 0.0:
                    print("DIFF", name, held.get("bone"), sorted(held), b, d)
                worst = max(worst, d)
            n += 1
    assert n >= 10, f"only {n} holds with aim or roll read"
    # Exactly: the shield's solve ends elsewhere on float noise in its start (hold.line).
    assert worst == 0.0, worst


if __name__ == "__main__":
    tests = [(k, v) for k, v in sorted(globals().items()) if k.startswith("test_")]
    failed = 0
    for name, fn in tests:
        try:
            fn()
            print("ok", name)
        except AssertionError as e:
            failed += 1
            print("FAIL", name, e)
    print(f"TESTS {len(tests) - failed}/{len(tests)} pass")
    sys.exit(1 if failed else 0)
