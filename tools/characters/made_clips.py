"""Puts the clips we made, rather than took from Quaternius's libraries, into the game's
mannequin file, each in place of the library's clip of the same name
(research/2026-10-14-animation.md).

    tools/blender/run.sh tools/characters/made_clips.py [OUT.glb] [--try=CLIP:FILE.bvh]

Each clip in art/animation/made.json has its take, a BVH on Kimodo's SOMA skeleton in its
standard T-pose, at art/animation/made/<clip>.bvh (made by motion.ts). For each:

  - the stride: from one left heel strike to the next, in the take's steady middle, whose first
    and last poses are nearest alike, so it loops with the smallest seam;
  - carried onto our skeleton as shipped retargeters do: each bone given the source bone's
    world rotation relative to its T-pose, on our bone's T-pose; the hips' travel scaled by leg
    length and taken off along the ground, so it runs in place as every game clip does;
  - the upper arms' and thighs' roll about their own length handed down to the forearm and
    shin, where the twist step (twist.py) spreads it; our skinning assumes the upper limbs never
    roll (clip_check.py), as Quaternius's clips never do;
  - closed into a loop: the last frame the first again, the frames before it eased into it.

OUT defaults to the game's mannequin file, which it reads and writes; clips.py makes it from
the libraries first where they are here. --try puts one take into OUT instead of the kept one,
to measure a candidate with gait_check.py before keeping it.
"""

import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy
from mathutils import Matrix, Quaternion, Vector

import look

MADE = os.path.join(look.ART, "animation", "made")
LIST = os.path.join(look.ART, "animation", "made.json")

# Kimodo's SOMA skeleton to ours. Its second neck joint, eyes, jaw and finger ends have no
# bone of ours; world rotations carry their effect to the bones that follow.
MAP = {"Hips": "pelvis", "Spine1": "spine_01", "Spine2": "spine_02", "Chest": "spine_03", "Neck1": "neck_01", "Head": "Head"}
for _side, _s in (("Left", "l"), ("Right", "r")):
    MAP.update({
        f"{_side}Shoulder": f"clavicle_{_s}", f"{_side}Arm": f"upperarm_{_s}", f"{_side}ForeArm": f"lowerarm_{_s}",
        f"{_side}Hand": f"hand_{_s}", f"{_side}Leg": f"thigh_{_s}", f"{_side}Shin": f"calf_{_s}",
        f"{_side}Foot": f"foot_{_s}", f"{_side}ToeBase": f"ball_{_s}",
    })
    for _finger, _ours in (("Thumb", "thumb"), ("Index", "index"), ("Middle", "middle"), ("Ring", "ring"), ("Pinky", "pinky")):
        # SOMA's fingers start a joint nearer the wrist than ours, all but the thumb.
        _first = 1 if _finger == "Thumb" else 2
        for _k in range(3):
            MAP[f"{_side}Hand{_finger}{_first + _k}"] = f"{_ours}_{_k + 1:02d}_{_s}"
NO_ROLL = ("upperarm_l", "upperarm_r", "thigh_l", "thigh_r")

STEADY_S = 1.0      # the take's first second is its start, not its stride
PLANTED_MPS = 0.6   # a foot slower than this over the ground, near its lowest, is planted
LOW_M = 0.05
EASE = 4            # frames eased into the loop's seam


def load_take(path):
    before = set(bpy.data.objects)
    bpy.ops.import_anim.bvh(filepath=path, global_scale=0.01, rotate_mode="NATIVE", axis_forward="-Z", axis_up="Y", use_fps_scale=False)
    return next(o for o in bpy.data.objects if o not in before)


def stride(src):
    """The take's frames from one left heel strike to the next whose poses best match."""
    scene = bpy.context.scene
    action = src.animation_data.action
    start, end = (int(round(x)) for x in action.frame_range)
    fps = scene.render.fps / scene.render.fps_base
    foot, poses = [], []
    for f in range(start, end + 1):
        scene.frame_set(f)
        foot.append(src.matrix_world @ src.pose.bones["LeftFoot"].head)
        poses.append({s: src.pose.bones[s].matrix_basis.to_quaternion() for s in MAP})
    low = min(p.z for p in foot)
    planted = [k > 0 and foot[k].z < low + LOW_M and ((foot[k] - foot[k - 1]).xy.length * fps) < PLANTED_MPS for k in range(len(foot))]
    strikes = [k for k in range(1, len(foot)) if planted[k] and not planted[k - 1] and k >= STEADY_S * fps and k < len(foot) - 5]
    best = None
    for a, b in zip(strikes, strikes[1:]):
        if not 0.4 * fps <= b - a <= 1.5 * fps:
            continue
        seam = sum(math.degrees(poses[a][s].rotation_difference(poses[b][s]).angle) for s in MAP) / len(MAP)
        if best is None or seam < best[2]:
            best = (start + a, start + b, seam)
    if best is None:
        raise SystemExit(f"no clean stride in {src.name}: left heel strikes at {strikes}")
    return best


def world_rest(arm, bone):
    return (arm.matrix_world @ arm.data.bones[bone].matrix_local).to_quaternion()


def without_roll(q):
    """`q`, a bone's own rotation, with its turn about the bone's length (Blender's Y) taken out."""
    twist = Quaternion((q.w, 0.0, q.y, 0.0))
    if twist.magnitude < 1e-6:
        return q
    twist.normalize()
    return q @ twist.inverted()


def retarget(src, arm, name, f0, f1):
    """Frames f0 to f1 of `src` as an action called `name` on `arm`, in place and looped."""
    scene = bpy.context.scene
    src_rest = {s: world_rest(src, s) for s in MAP}
    dst_rest = {d: world_rest(arm, d) for d in MAP.values()}
    src_hips = src.matrix_world @ src.data.bones["Hips"].head_local
    dst_hips = arm.matrix_world @ arm.data.bones["pelvis"].head_local
    scale = dst_hips.z / src_hips.z
    scene.frame_set(f0)
    a = src.matrix_world @ src.pose.bones["Hips"].head
    scene.frame_set(f1)
    b = src.matrix_world @ src.pose.bones["Hips"].head
    travel = Vector((b.x - a.x, b.y - a.y, 0.0))
    # Turned to face our front, -Y, and its travel taken off.
    turn = Matrix.Rotation(math.atan2(travel.x, -travel.y), 4, "Z")
    per_frame = travel / (f1 - f0)
    action = bpy.data.actions.new(name)
    arm.animation_data_create()
    arm.animation_data.action = action
    order = [bone.name for bone in arm.data.bones if bone.name in dst_rest]   # parents first
    for f in range(f0, f1 + 1):
        scene.frame_set(f)
        worlds = {d: turn.to_quaternion() @ (src.matrix_world @ src.pose.bones[s].matrix).to_quaternion() @ src_rest[s].inverted() @ dst_rest[d] for s, d in MAP.items()}
        hips = turn @ (src.matrix_world @ src.pose.bones["Hips"].head - per_frame * (f - f0) - Vector((a.x, a.y, 0.0)))
        for d in order:
            pb = arm.pose.bones[d]
            m = Matrix.LocRotScale(hips * scale if d == "pelvis" else (arm.matrix_world @ pb.matrix).translation, worlds[d], None)
            pb.matrix = arm.matrix_world.inverted() @ m
            if d in NO_ROLL:
                # Its children are set after it, to their own world rotations, so the roll
                # taken out here is what they take on.
                pb.rotation_mode = "QUATERNION"
                pb.rotation_quaternion = without_roll(pb.rotation_quaternion)
            bpy.context.view_layer.update()
            pb.keyframe_insert("rotation_quaternion", frame=f - f0)
            if d == "pelvis":
                pb.keyframe_insert("location", frame=f - f0)
    last = f1 - f0
    for fc in curves(action):
        gap = fc.evaluate(0) - fc.evaluate(last)
        for k in range(1, EASE + 1):
            w = k / EASE
            f = last - EASE + k
            fc.keyframe_points.insert(f, fc.evaluate(f) + gap * w * w * (3 - 2 * w), options={"REPLACE"})
    return action


def curves(action):
    if hasattr(action, "fcurves"):
        return list(action.fcurves)
    return [c for layer in action.layers for strip in layer.strips for bag in strip.channelbags for c in bag.fcurves]


def main():
    args = look.args()
    out = next((look.path(a) for a in args if not a.startswith("--")), look.FIGURE)
    tried = dict(a.split("=", 1)[1].split(":", 1) for a in args if a.startswith("--try="))
    with open(LIST) as f:
        made = json.load(f)
    # An entry with no take kept yet is still being made (motion.ts); the library's clip stays.
    takes = {name: os.path.join(MADE, f"{name}.bvh") for name in made if os.path.exists(os.path.join(MADE, f"{name}.bvh"))}
    takes.update({name: look.path(path) for name, path in tried.items()})
    look.reset()
    bpy.ops.import_scene.gltf(filepath=out)
    arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    body = next(o for o in bpy.data.objects if o.type == "MESH" and any(m.type == "ARMATURE" for m in o.modifiers))
    ad = arm.animation_data or arm.animation_data_create()
    ad.action = None
    names = [t.name for t in ad.nla_tracks]
    for name, path in takes.items():
        for pb in arm.pose.bones:
            pb.matrix_basis.identity()
        src = load_take(path)
        f0, f1, seam = stride(src)
        old = bpy.data.actions.get(name)
        if old is not None:
            old.name = name + ".old"
        action = retarget(src, arm, name, f0, f1)
        bpy.data.objects.remove(src)
        ad.action = None
        if old is not None:
            bpy.data.actions.remove(old)
        if name not in names:
            names.append(name)
        print(f"MADE {name} from {os.path.relpath(path, look.ROOT)}: frames {f0} to {f1}, seam {seam:.2f} deg")
    # Every clip a track of its own again, each strip where its action starts, as clips.py
    # lays them: the importer's own tracks start a frame late, which the export would keep.
    for t in list(ad.nla_tracks):
        ad.nla_tracks.remove(t)
    for name in names:
        action = bpy.data.actions[name]
        track = ad.nla_tracks.new()
        track.name = name
        strip = track.strips.new(name, int(action.frame_range[0]), action)
        if getattr(action, "slots", None) and len(action.slots):
            strip.action_slot = action.slots[0]
        track.mute = True
    for pb in arm.pose.bones:
        pb.matrix_basis.identity()
    for o in bpy.context.scene.objects:
        o.select_set(o in (arm, body))
    bpy.context.view_layer.objects.active = arm
    bpy.ops.export_scene.gltf(
        filepath=out, export_format="GLB", use_selection=True,
        export_animations=True, export_animation_mode="NLA_TRACKS", export_force_sampling=True,
        export_influence_nb=4,
    )
    print("MADE", len(takes), "clips into", os.path.relpath(out, look.ROOT))


main()
