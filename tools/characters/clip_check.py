"""Checks that every clip the game plays is one our skinning can follow, before any body is
built on it (research/skin-deformation.md).

    tools/blender/run.sh tools/characters/clip_check.py [MANNEQUIN.glb] [CLIP ...] [--trace]

By default the game's mannequin file (clips.py) and all its clips. For each clip:
  - shoulder and hip roll: how far the upper arm and thigh turn about their own length. Our
    twist step (twist.py) assumes they never do, as Quaternius's clips never do; a clip that
    does would wring the skin at the shoulder or hip.
  - limb roll: after the twist step moves the forearm's and shin's roll to the hand and foot,
    the roll between each segment and the bone beyond it, which the twist bone halves. The
    largest step between neighbouring bones is what the skin must turn across; dual
    quaternion skinning keeps its volume, but goes the short way round past half a turn.
  - the twist step itself: the hands and feet must end up exactly where the clip puts them.
Prints a CLIPCHECK line of JSON per clip, then CLIPCHECK pass or fail. With --trace, also a
TRACE line per frame of each clip named: each limb's roll and its twist bone's share, in
degrees, to see what the twist step did.
"""

import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy

import look
import twist

LIMITS = {
    "shoulder_hip_roll_deg": 30.0,  # the upper arm's or thigh's own roll, which nothing spreads
    "step_deg": 135.0,              # roll between neighbouring bones of a limb after the twist step
    "moved_deg": 0.1,               # how far the twist step may turn a hand or foot: not at all
}

args = look.args()
path = look.path(args[0]) if args and args[0].endswith((".glb", ".gltf")) else look.FIGURE
wanted = [a for a in args if not a.endswith((".glb", ".gltf")) and not a.startswith("--")]
trace = "--trace" in args
look.reset()
bpy.ops.import_scene.gltf(filepath=path)
arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
bones = arm.data.bones
ROOTS = [(f"upperarm_{s}", f"lowerarm_{s}") for s in twist.SIDES] + [(f"thigh_{s}", f"calf_{s}") for s in twist.SIDES]
shares = {(f"{seg}_{s}", f"{to}_{s}"): share for s in twist.SIDES for _, seg, to, share in twist.TWIST}

ok = True
for action in sorted(bpy.data.actions, key=lambda a: a.name):
    if wanted and action.name not in wanted:
        continue
    frames, _, turns, moved = twist.rolled(arm, action)
    if trace:
        for i, (f, _) in enumerate(frames):
            print("TRACE", action.name, f, "  ".join(f"{seg}>{to} {math.degrees(t[i]):6.1f} twist {shares[(seg, to)] * math.degrees(t[i]):6.1f}"
                                                    for (seg, to), t in turns.items() if shares[(seg, to)] > 0))
    # The upper arm's and thigh's own roll, read back from the frames rolled() sampled.
    root_roll = 0.0
    for f, _ in frames:
        bpy.context.scene.frame_set(f)
        for seg, to in ROOTS:
            q = arm.pose.bones[seg].matrix_basis.to_quaternion()
            root_roll = max(root_roll, abs(math.degrees(twist.split(q, twist.axis(arm, seg, to))[2])))
    steps = {}
    for pair, turn in turns.items():
        share = shares[pair]
        if share <= 0:
            continue
        steps[pair[0]] = max(max(share, 1 - share) * abs(math.degrees(t)) for t in turn)
    worst = max(steps.items(), key=lambda kv: kv[1]) if steps else ("", 0.0)
    passed = (root_roll <= LIMITS["shoulder_hip_roll_deg"] and worst[1] <= LIMITS["step_deg"]
              and moved <= LIMITS["moved_deg"])
    ok &= passed
    print("CLIPCHECK", json.dumps({"clip": action.name, "pass": passed, "shoulder_hip_roll_deg": round(root_roll, 1),
                                   "worst_step_deg": round(worst[1], 1), "at": worst[0], "hand_foot_moved_deg": round(moved, 3)}))
print("CLIPCHECK", "pass" if ok else "fail", json.dumps(LIMITS))
