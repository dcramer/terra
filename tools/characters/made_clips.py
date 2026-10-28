"""Puts the clips we made, rather than took from Quaternius's libraries, into the game's
mannequin file, each in place of the library's clip of the same name
(research/2026-10-14-animation.md).

    tools/blender/run.sh tools/characters/made_clips.py [OUT.glb] [--try=CLIP:FILE.bvh] [--list=FILE.json]

Each clip in art/animation/made.json has its take, a BVH on Kimodo's SOMA skeleton in its
standard T-pose, at art/animation/made/<clip>.bvh, or a GLB on our own skeleton made by Uthana
(D532) at art/animation/made/<clip>.glb (both made by motion.ts). For each:

  - the stride: from one left heel strike to the next, in the take's steady middle, whose first
    and last poses are nearest alike, so it loops with the smallest seam; or for a clip marked
    "still", which stays where it stands (an idle), the two frames 2 to 5 seconds apart that are,
    or as far apart as its "loop" says (seconds, least and most), for a longer action, and for
    one marked "busy", which works where it stands (a hammer, an axe), in its busiest stretch;
  - carried onto our skeleton as shipped retargeters do: each bone given the source bone's
    world rotation relative to its T-pose, on our bone's T-pose; the hips' travel scaled by leg
    length and taken off along the ground, so it runs in place as every game clip does;
  - the upper arms' and thighs' roll about their own length handed down to the forearm and
    shin, where the twist step (twist.py) spreads it; our skinning assumes the upper limbs never
    roll (clip_check.py), as Quaternius's clips never do;
  - or, for an entry Uthana closed into a loop itself ("looped"), the whole take;
  - closed into a loop: the last frame the first again, the frames before it eased into it;
    its seam, how far the hands, feet and head are from where they started relative to the
    hips before the easing, printed (cm) beside the most a limb or the trunk turned (deg), and
    the take's mean turn of its bones there, which chose the loop (apart);
  - for an idle with "stand", its hips raised toward the body's rest and its head drawn back
    over them, its feet where they stood (stand.py);
  - for a stride with "arms", its arms set to a runner's swing, its legs and spine untouched
    (swing_arms), and for one with "fist", its fingers closed that share of the way into the
    library's clenched fist (Kimodo leaves them straight).

A stride that travels `way` degrees to the right of where the body faces (a strafe) is turned
to face ahead, not along its travel. A clip marked "once" (a blow, a block, a dodge) is kept
from the take's start to its end, or over its "span" (seconds in the take), at the take's own
pace, in place, and not closed into a loop; an ONCE line says when each hand is raised highest
and moves fastest, when the faster hand's blow lands, and how far the body travels, which the
scene moves it by.

OUT defaults to the game's mannequin file, which it reads and writes; clips.py makes it from
the libraries first where they are here. --try puts one take into OUT instead of the kept one,
to measure a candidate with gait_check.py before keeping it; --try=CLIP#N:FILE puts it in as a
clip of its own, CLIP#N, made by CLIP's entry, so several takes of one clip are measured side by
side (the clip step, clip.ts). --list=FILE makes another set of
clips (a creature's, a demo's: art/animation/sets/fight.json, its takes in sets/fight/) onto
the game's mannequin into OUT, which must be named and then carries the game's clips and the
set's, so a body exported with it (export_character.py --clips=OUT) plays both: the troll walks
and is hit as a person is and smashes as a troll, the fight's man idles and rolls as the game's.
"""

import json
import math
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy
from mathutils import Matrix, Quaternion, Vector

import look
import stand

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
PLANTED_MPS = 0.75  # a foot slower than this over the ground, near its lowest, is planted (a sprint's
                    # planted foot still slides a little in the take)
LOW_M = 0.05
EASE = 4            # frames eased into the loop's seam


def load_take(path):
    """The take on its own skeleton, a key a frame, and its frame rate (Kimodo's 30), which is
    not the scene's: its frames are counted by its own rate and keyed at the scene's. A GLB take
    (Uthana's, on our skeleton, motion.ts) is keyed at the scene's rate by the importer, its
    bones named as SOMA's that have one, so it is carried as a Kimodo take is: rest to rest, the
    same skeleton, its world rotations kept."""
    before = set(bpy.data.objects)
    if path.endswith(".glb"):
        bpy.ops.import_scene.gltf(filepath=path)
        new = [o for o in bpy.data.objects if o not in before]
        src = next(o for o in new if o.type == "ARMATURE")
        for o in new:
            if o is not src:
                bpy.data.objects.remove(o)
        soma = {ours: theirs for theirs, ours in MAP.items()}
        for bone in src.data.bones:
            if bone.name in soma:
                bone.name = soma[bone.name]   # its action's paths follow the name
        scene = bpy.context.scene
        return src, scene.render.fps / scene.render.fps_base
    bpy.ops.import_anim.bvh(filepath=path, global_scale=0.01, rotate_mode="NATIVE", axis_forward="-Z", axis_up="Y", use_fps_scale=False)
    with open(path) as f:
        frame_time = next(float(line.split(":")[1]) for line in f if line.startswith("Frame Time:"))
    return next(o for o in bpy.data.objects if o not in before), 1.0 / frame_time


def still(src, fps, loop=(2.0, 5.0), busy=False):
    """For a clip that stays where it stands (an idle): the frames `loop` seconds apart (2 to 5
    unless its entry says), after the take's first second, whose poses best match, so it loops
    with the smallest seam. For one that works where it stands (`busy`: a hammer, an axe), only
    among stretches that move at least half as much as the busiest: a take that stops working
    and stands is stillest, so has the best seam, where it does nothing."""
    scene = bpy.context.scene
    action = src.animation_data.action
    start, end = (int(round(x)) for x in action.frame_range)
    poses = []
    for f in range(start, end + 1):
        scene.frame_set(f)
        poses.append({s: src.pose.bones[s].matrix_basis.to_quaternion() for s in MAP})

    def apart(p, q):
        return sum(math.degrees(p[s].rotation_difference(q[s]).angle) for s in MAP) / len(MAP)

    moved = [0.0]   # how far the pose has turned by each frame, summed
    for k in range(1, len(poses)):
        moved.append(moved[-1] + apart(poses[k - 1], poses[k]))
    windows = [(a, b) for a in range(int(STEADY_S * fps), len(poses)) for b in range(a + int(loop[0] * fps), min(len(poses), a + int(loop[1] * fps) + 1))]
    if busy and windows:
        busiest = max((moved[b] - moved[a]) / (b - a) for a, b in windows)
        windows = [(a, b) for a, b in windows if (moved[b] - moved[a]) / (b - a) >= busiest / 2]
    best = None
    for a, b in windows:
        seam = apart(poses[a], poses[b])
        if best is None or seam < best[2]:
            best = (start + a, start + b, seam)
    if best is None:
        raise SystemExit(f"{src.name} is too short to loop: it needs {STEADY_S + loop[0]:.0f} seconds")
    return best


def stride(src, fps):
    """The take's frames from one left heel strike to the next whose poses best match."""
    scene = bpy.context.scene
    action = src.animation_data.action
    start, end = (int(round(x)) for x in action.frame_range)
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


def one_shot(src, fps, span=None):
    """For a one-shot (a blow, a block, a dodge): the take from its start to its end, or over
    `span` (seconds in the take), unlooped; its seam is how far its last pose is from its first,
    which is small for one that starts and ends in the same stance."""
    scene = bpy.context.scene
    start, end = (int(round(x)) for x in src.animation_data.action.frame_range)
    f0, f1 = (start, end) if span is None else (start + int(round(span[0] * fps)), min(end, start + int(round(span[1] * fps))))
    poses = []
    for f in (f0, f1):
        scene.frame_set(f)
        poses.append({s: src.pose.bones[s].matrix_basis.to_quaternion() for s in MAP})
    return f0, f1, sum(math.degrees(poses[0][s].rotation_difference(poses[1][s]).angle) for s in MAP) / len(MAP)


def moments(src, fps, f0, f1):
    """What a one-shot does, to time it in a scene by: when each hand moves fastest (a blow's
    moment), when the faster hand's blow lands (its contact: the first moment after its fastest
    that it slows below 30% of that, the cut stopping), how far the body travels ahead and to
    its right (which the clip, played in place, leaves the scene to move it by), and how far it
    has turned by its end."""
    scene = bpy.context.scene
    ahead = facing(src, f0)
    right = ahead.cross(Vector((0.0, 0.0, 1.0)))
    hands, hips = {"RightHand": [], "LeftHand": []}, []
    for f in range(f0, f1 + 1):
        scene.frame_set(f)
        for h in hands:
            hands[h].append(src.matrix_world @ src.pose.bones[h].head)
        hips.append(src.matrix_world @ src.pose.bones["Hips"].head)
    fastest, contact = {}, {}
    for h, at in hands.items():
        speeds = [(at[i + 1] - at[i]).length * fps for i in range(len(at) - 1)]
        i = max(range(len(speeds)), key=speeds.__getitem__)
        # Raised: its highest before then, the top of a wind-up.
        top = max(range(i + 1), key=lambda j: at[j].z)
        fastest[h] = ((i + 0.5) / fps, speeds[i], top / fps, at[top].z - at[0].z)
        # Lands: the first moment after then that it falls below 30% of that speed.
        stop = next((j for j in range(i + 1, len(speeds)) if speeds[j] < 0.3 * speeds[i]), len(speeds) - 1)
        contact[h] = (stop + 0.5) / fps
    hand = max(fastest, key=lambda h: fastest[h][1])
    moved = hips[-1] - hips[0]
    turned = math.degrees(ahead.angle(facing(src, f1))) * (1 if ahead.cross(facing(src, f1)).z > 0 else -1)
    return fastest, (hand, contact[hand]), moved.dot(ahead), moved.dot(right), turned


def world_rest(arm, bone):
    return (arm.matrix_world @ arm.data.bones[bone].matrix_local).to_quaternion()


def without_roll(q):
    """`q`, a bone's own rotation, with its turn about the bone's length (Blender's Y) taken out."""
    twist = Quaternion((q.w, 0.0, q.y, 0.0))
    if twist.magnitude < 1e-6:
        return q
    twist.normalize()
    return q @ twist.inverted()


def facing(src, f):
    """Which way the take's body faces on the ground at frame `f`: up crossed with the hips'
    left-to-right."""
    bpy.context.scene.frame_set(f)
    across = (src.matrix_world @ src.pose.bones["RightLeg"].head) - (src.matrix_world @ src.pose.bones["LeftLeg"].head)
    return Vector((0.0, 0.0, 1.0)).cross(Vector((across.x, across.y, 0.0))).normalized()


def retarget(src, arm, name, f0, f1, k=1.0, way=0.0, once=False, travels=False, planted=False):
    """Frames f0 to f1 of `src` as an action called `name` on `arm`, in place and looped, each
    frame of the take `k` frames of the action. A take that travels `way` degrees to the right
    of where it faces (a strafe) is turned by its travel less that; a one-shot (`once`) by where
    its body faces at the take's start (Kimodo's ahead), and not closed into a loop. One that
    `travels` (a start, a stop) keeps its hips' travel, its root motion, which the game takes off
    by the clip's distance curve as Lyra's distance matching does (markers.ts bakes it)."""
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
    # Turned to face our front, -Y, and its travel taken off. A clip that hardly travels, such as
    # an idle, faces the way its hips do: forward is up crossed with the hips' left-to-right.
    # Facing +Y, the right is +X: travel `way` to the right is ahead turned clockwise by it.
    if once:
        ahead = facing(src, int(round(src.animation_data.action.frame_range[0])))
    elif travel.length > 0.3:
        ahead = Matrix.Rotation(math.radians(way), 3, "Z") @ travel
    else:
        ahead = facing(src, f0)
    turn = Matrix.Rotation(math.atan2(-ahead.x, -ahead.y), 4, "Z")
    per_frame = travel * 0.0 if travels else travel / (f1 - f0)

    def drift(f):
        # How far the body has slid by frame f, taken off its hips: a stride's travel spread
        # evenly, or for a clip whose feet stand `planted` (an idle) how far the forefeet's
        # midpoint has moved over the ground since f0, so they stay where they stood and the
        # hips keep only their sway over them (Uthana's idle slid 7 cm, its feet together).
        if not planted:
            return per_frame * (f - f0)
        scene.frame_set(f)
        mid = sum((src.matrix_world @ src.pose.bones[b].head for b in ("LeftToeBase", "RightToeBase")), Vector()) / 2
        return Vector((mid.x - mid0.x, mid.y - mid0.y, 0.0))

    if planted:
        scene.frame_set(f0)
        mid0 = sum((src.matrix_world @ src.pose.bones[b].head for b in ("LeftToeBase", "RightToeBase")), Vector()) / 2
    action = bpy.data.actions.new(name)
    arm.animation_data_create()
    arm.animation_data.action = action
    order = [bone.name for bone in arm.data.bones if bone.name in dst_rest]   # parents first
    for f in range(f0, f1 + 1):
        slid = drift(f)
        scene.frame_set(f)
        worlds = {d: turn.to_quaternion() @ (src.matrix_world @ src.pose.bones[s].matrix).to_quaternion() @ src_rest[s].inverted() @ dst_rest[d] for s, d in MAP.items()}
        hips = turn @ (src.matrix_world @ src.pose.bones["Hips"].head - slid - Vector((a.x, a.y, 0.0)))
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
            pb.keyframe_insert("rotation_quaternion", frame=(f - f0) * k)
            if d == "pelvis":
                pb.keyframe_insert("location", frame=(f - f0) * k)
    last = f1 - f0
    if once:
        return action, (0.0, 0.0)
    seam = seam_of(arm, last * k)
    for fc in curves(action):
        gap = fc.evaluate(0) - fc.evaluate(last * k)
        for e in range(1, EASE + 1):
            w = e / EASE
            f = (last - EASE + e) * k
            fc.keyframe_points.insert(f, fc.evaluate(f) + gap * w * w * (3 - 2 * w), options={"REPLACE"})
    return action, seam


SEAM_BONES = ("hand_l", "hand_r", "foot_l", "foot_r", "Head")
SEAM_TURNS = ("pelvis", "spine_03", "Head", "upperarm_l", "upperarm_r", "lowerarm_l", "lowerarm_r", "hand_l", "hand_r",
              "thigh_l", "thigh_r", "calf_l", "calf_r", "foot_l", "foot_r")


def seam_of(arm, end):
    """How far the hands, feet and head at frame `end` of `arm`'s action are from where they
    are at frame 0, each relative to the hips, at most (cm), and how far its limbs and trunk
    are turned from how they were, each relative to the hips and the hips in the world, at most
    (deg): the jump a loop's last frames must ease over."""
    scene = bpy.context.scene

    def marks(f):
        scene.frame_set(int(f), subframe=f - int(f))
        hips = arm.pose.bones["pelvis"]
        at = [arm.matrix_world.to_3x3() @ (arm.pose.bones[b].head - hips.head) for b in SEAM_BONES]
        turns = [hips.matrix.to_quaternion() if b == "pelvis" else hips.matrix.to_quaternion().inverted() @ arm.pose.bones[b].matrix.to_quaternion() for b in SEAM_TURNS]
        return at, turns

    (a, p), (b, q) = marks(0), marks(end)
    return max((x - y).length for x, y in zip(a, b)) * 100, max(math.degrees(x.rotation_difference(y).angle) for x, y in zip(p, q))


FINGERS = ("thumb", "index", "middle", "ring", "pinky")
FIST_CLIP = "Punch_Cross"   # the library's clenched fist, held through its punch


def fist_pose(clip):
    """Each finger bone's turn in the middle of the library's `clip`, from its curves."""
    action = bpy.data.actions.get(clip)
    if action is None:
        raise SystemExit(f"no {clip} in the mannequin file to take a fist from")
    mid = sum(action.frame_range) / 2
    by_bone = {}
    for fc in curves(action):
        bone = fc.data_path.split('"')[1] if '"' in fc.data_path else ""
        if fc.data_path.endswith("rotation_quaternion") and bone.split("_")[0] in FINGERS:
            by_bone.setdefault(bone, [0.0] * 4)[fc.array_index] = fc.evaluate(mid)
    return {b: Quaternion(q).normalized() for b, q in by_bone.items()}


def close_hands(action, fist, share):
    """Every finger key of `action` turned `share` of the way to `fist`: Kimodo leaves the
    fingers straight, an open, stiff hand; a runner's are loosely curled, the thumb on them."""
    by_bone = {}
    for fc in curves(action):
        bone = fc.data_path.split('"')[1] if '"' in fc.data_path else ""
        if fc.data_path.endswith("rotation_quaternion") and bone in fist:
            by_bone.setdefault(bone, {})[fc.array_index] = fc
    for bone, fcs in by_bone.items():
        if len(fcs) != 4:
            continue
        for k in range(len(fcs[0].keyframe_points)):
            q = Quaternion([fcs[i].keyframe_points[k].co[1] for i in range(4)])
            if q.dot(fist[bone]) < 0:
                q.negate()
            q = q.slerp(fist[bone], share)
            for i in range(4):
                fcs[i].keyframe_points[k].co[1] = q[i]
                fcs[i].keyframe_points[k].handle_left_type = fcs[i].keyframe_points[k].handle_right_type = "AUTO_CLAMPED"
        for fc in fcs.values():
            fc.update()


def swing_arms(arm, action, spec):
    """The arms of a stride `action` set to a runner's, its legs and spine left exactly as they
    are: an additive pass on the arm chain, as a game layers an arm pose over a locomotion clip.
    Each of `spec`'s entries is optional:

      - swing: [forward, back], degrees from the trunk's line: the upper arms' swing mapped from
        the take's own range onto it, both arms timed by the left's, the right half a stride
        later (Kimodo's right arm may hardly swing at all, where a runner's mirror each other);
      - tuck: degrees: the upper arm kept at most this far out from the trunk's side, the
        elbows close to the ribs;
      - elbow: [back, front], degrees bent: the elbow closing as the hand comes forward and up
        and opening behind, as a runner's does (Kimodo's bends most behind, the hand hovering at
        the belly);
      - across: degrees: the forearm turned out about the upper arm's line until it points
        across the body at most this much, the hands passing in front of the ribs rather than
        meeting at the belly;
      - wrist: degrees: the hand bent at most this far off the forearm's line;
      - palm: degrees: the hand rolled about the forearm to this far from thumb side up toward
        palm down, as a loose fist is carried.

    Returns, per side, how far the take's upper arm swung forward and back of the trunk."""
    scene = bpy.context.scene
    mw = arm.matrix_world
    pose = arm.pose.bones
    front, back = spec.get("swing", (None, None))
    elbow_range = spec.get("elbow")
    wrist = spec.get("wrist", 180.0)
    across = spec.get("across", 90.0)
    palm = spec.get("palm")
    tuck = spec.get("tuck", 90.0)
    frames = range(int(round(action.frame_range[0])), int(round(action.frame_range[1])) + 1)
    ad = arm.animation_data
    ad.action = action
    if getattr(action, "slots", None) and len(action.slots):
        ad.action_slot = action.slots[0]

    def trunk():
        down = (mw @ pose["pelvis"].head - mw @ pose["neck_01"].head).normalized()
        ahead = Vector((0.0, -1.0, 0.0))
        ahead = (ahead - down * ahead.dot(down)).normalized()
        return down, ahead

    def swing_of(s):
        down, ahead = trunk()
        u = mw @ pose[f"lowerarm_{s}"].head - mw @ pose[f"upperarm_{s}"].head
        return math.degrees(math.atan2(u.dot(ahead), u.dot(down)))

    took = {s: [] for s in "lr"}
    for f in frames:
        scene.frame_set(f)
        for s in "lr":
            took[s].append(swing_of(s))
    n = len(frames) - 1   # the last frame is the first again
    lo, hi = min(took["l"]), max(took["l"])
    gain = (front + back) / (hi - lo) if front is not None else 1.0

    def driven(s, k):
        t = (k + (n / 2 if s == "r" else 0)) % n
        i = int(t)
        w = t - i
        return took["l"][i] * (1 - w) + took["l"][(i + 1) % n] * w

    def turn(s, f, turns):
        names = (f"upperarm_{s}", f"lowerarm_{s}", f"hand_{s}")
        was = {b: (mw @ pose[b].matrix).to_quaternion() for b in names}
        for b, t in zip(names, turns):
            pb = pose[b]
            pb.matrix = mw.inverted() @ Matrix.LocRotScale((mw @ pb.matrix).translation, t @ was[b], None)
            if b in NO_ROLL:
                pb.rotation_mode = "QUATERNION"
                pb.rotation_quaternion = without_roll(pb.rotation_quaternion)
            bpy.context.view_layer.update()
            pb.keyframe_insert("rotation_quaternion", frame=f)

    for s in "lr":
        for k, f in enumerate(frames):
            scene.frame_set(f)
            down, ahead = trunk()
            shoulder, elbow, wrist_at = (mw @ pose[b].head for b in (f"upperarm_{s}", f"lowerarm_{s}", f"hand_{s}"))
            knuckle = mw @ pose[f"middle_01_{s}"].head
            if front is not None:
                new = -back + (driven(s, k) - lo) * gain
                share = (new + back) / (front + back)
            else:
                new = took[s][k]
                share = (new - min(took[s])) / (max(took[s]) - min(took[s]))
            swing = Quaternion(down.cross(ahead), math.radians(new - took[s][k]))
            outward = down.cross(ahead) * (1 if s == "r" else -1)   # the body's left is +X
            # The upper arm brought in toward the ribs, out from the trunk's side at most `tuck`
            # degrees: a turn about the forward line, which takes a hanging arm toward +X.
            u = swing @ (elbow - shoulder)
            out = math.degrees(math.asin(max(-1.0, min(1.0, u.normalized().dot(outward)))))
            if out > tuck:
                swing = Quaternion(ahead, math.radians(out - tuck) * (1 if s == "r" else -1)) @ swing
            # The elbow, about its own hinge, from the take's bend to the one this swing asks for.
            u, fa = swing @ (elbow - shoulder), swing @ (wrist_at - elbow)
            hinge = Quaternion()
            if elbow_range:
                bent = elbow_range[0] + (elbow_range[1] - elbow_range[0]) * share * share * (3 - 2 * share)
                hinge = Quaternion(u.cross(fa).normalized(), math.radians(bent - math.degrees(u.angle(fa))))
            fa = hinge @ fa
            # The forearm turned out about the upper arm's line, found by halving.
            twist = Quaternion()
            axis = u.normalized()
            inward = lambda q: math.degrees(math.asin(max(-1.0, min(1.0, -(q @ fa).normalized().dot(outward)))))
            if inward(twist) > across:
                way = 1 if inward(Quaternion(axis, 0.01)) < inward(twist) else -1
                lo_a, hi_a = 0.0, math.radians(90)
                for _ in range(30):
                    mid = (lo_a + hi_a) / 2
                    lo_a, hi_a = (mid, hi_a) if inward(Quaternion(axis, way * mid)) > across else (lo_a, mid)
                twist = Quaternion(axis, way * hi_a)
            fa = twist @ fa
            # The hand brought toward the forearm's line.
            h = twist @ hinge @ swing @ (knuckle - wrist_at)
            bend = math.degrees(h.angle(fa))
            straighten = Quaternion() if bend <= wrist else Quaternion().slerp(h.rotation_difference(fa), 1 - wrist / bend)
            # The hand's roll: from thumb side up (which turns with the forearm as it sweeps, so
            # the hand does not wring), `palm` degrees toward the palm facing down, the thumb
            # turned in, as a loose fist is carried (twist.py spreads the roll down the forearm).
            hand_turn = straighten @ twist @ hinge @ swing
            line = fa.normalized()
            radial = hand_turn @ (mw @ pose[f"index_01_{s}"].head - mw @ pose[f"pinky_01_{s}"].head)
            radial = radial - line * radial.dot(line)
            neutral = line.cross(ahead.cross(down))   # the body's left crossed: thumb up when the forearm points ahead
            middle = -outward - line * (-outward).dot(line)
            if palm is not None and radial.length > 1e-6 and neutral.length > 1e-6 and middle.length > 1e-6:
                aim_at = neutral.normalized() * math.cos(math.radians(palm)) + middle.normalized() * math.sin(math.radians(palm))
                off = radial.angle(aim_at)
                sign = 1 if line.dot(radial.cross(aim_at)) > 0 else -1
                hand_turn = Quaternion(line, sign * off) @ hand_turn
            turn(s, f, (swing, twist @ hinge @ swing, hand_turn))
    ad.action = None
    return {"l": (hi, -lo), "r": (max(took["r"]), -min(took["r"]))}


def curves(action):
    if hasattr(action, "fcurves"):
        return list(action.fcurves)
    return [c for layer in action.layers for strip in layer.strips for bag in strip.channelbags for c in bag.fcurves]


def main():
    args = look.args()
    out = next((look.path(a) for a in args if not a.startswith("--")), None)
    tried = dict(a.split("=", 1)[1].split(":", 1) for a in args if a.startswith("--try="))
    # Another set of clips than the game's (a creature's, a demo's): its own list, its takes in
    # the folder named as the list is, made onto the game's mannequin into OUT, which then
    # carries the game's clips and the set's.
    chosen = next((look.path(a.split("=", 1)[1]) for a in args if a.startswith("--list=")), None)
    if chosen and out is None:
        raise SystemExit("a set (--list) is made into a copy of the mannequin: name OUT.glb")
    listed, kept = (chosen, chosen[: -len(".json")]) if chosen else (LIST, MADE)
    out = out or look.FIGURE
    with open(listed) as f:
        made = json.load(f)
    # An entry with no take kept yet is still being made (motion.ts); the library's clip stays.
    # A take tried is made alone, into a copy that already has the kept ones.
    # A clip's take is Uthana's GLB if it has one, else Kimodo's BVH.
    def take_of(name):
        return next((p for p in (os.path.join(kept, f"{name}{ext}") for ext in (".glb", ".bvh")) if os.path.exists(p)), None)
    takes = {name: look.path(path) for name, path in tried.items()} or {name: take_of(name) for name in made if take_of(name)}
    look.reset()
    bpy.ops.import_scene.gltf(filepath=look.FIGURE if chosen else out)
    arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    body = next(o for o in bpy.data.objects if o.type == "MESH" and any(m.type == "ARMATURE" for m in o.modifiers))
    ad = arm.animation_data or arm.animation_data_create()
    ad.action = None
    names = [t.name for t in ad.nla_tracks]
    fist = fist_pose(FIST_CLIP) if any("fist" in made.get(n.split("#")[0], {}) for n in takes) else {}
    for name, path in takes.items():
        for pb in arm.pose.bones:
            pb.matrix_basis.identity()
        src, fps = load_take(path)
        entry = made.get(name.split("#")[0], {})
        if entry.get("once") or entry.get("looped"):
            # A one-shot, or a take Uthana made a loop of: the whole take.
            f0, f1, seam = one_shot(src, fps, entry.get("span"))
        elif entry.get("still"):
            f0, f1, seam = still(src, fps, tuple(entry.get("loop", (2.0, 5.0))), entry.get("busy", False))
        else:
            f0, f1, seam = stride(src, fps)
        if entry.get("once"):
            fastest, (hand, lands), ahead, right, turned = moments(src, fps, f0, f1)
            print(f"ONCE {name}: " + ", ".join(f"{h[:-4].lower()} hand raised {up:+.2f} m at {top:.2f} s, fastest {v:.1f} m/s at {t:.2f} s"
                                               for h, (t, v, top, up) in fastest.items())
                  + f"; {hand[:-4].lower()} hand's blow lands at {lands:.2f} s; travels {ahead:+.2f} m ahead, {right:+.2f} m right; turned {turned:+.0f} deg (left +) by its end")
        old = bpy.data.actions.get(name)
        if old is not None:
            old.name = name + ".old"
        # A clip that stays where it stands plays at the take's own pace: its frames, Kimodo's 30
        # a second, keyed at the scene's, rounded so the loop is a whole number of frames, which
        # the exporter samples. A stride keeps a key a frame, as made: the game plays it at the
        # pace its feet set (pace.py), and fewer keys a stride would blur its contacts.
        # A one-shot plays at the take's own pace too: it does not travel at a pace the feet set.
        k = 1.0
        if entry.get("still") or entry.get("once"):
            k = round((f1 - f0) * bpy.context.scene.render.fps / bpy.context.scene.render.fps_base / fps) / (f1 - f0)
        action, (seam_at, seam_turn) = retarget(src, arm, name, f0, f1, k, entry.get("way", 0.0), entry.get("once", False), entry.get("travels", False), entry.get("planted", False))
        if entry.get("way"):
            # Which way the take went, measured: its travel from where its hips face.
            scene = bpy.context.scene
            scene.frame_set(f0)
            a = src.matrix_world @ src.pose.bones["Hips"].head
            scene.frame_set(f1)
            went = (src.matrix_world @ src.pose.bones["Hips"].head) - a
            ahead = facing(src, (f0 + f1) // 2)
            print(f"WAY {name}: travelled {math.degrees(math.atan2(went.dot(ahead.cross(Vector((0.0, 0.0, 1.0)))), went.dot(ahead))):+.0f} deg "
                  f"to the right of where its hips face mid-stride, asked {entry['way']}")
        if "arms" in entry:
            took = swing_arms(arm, action, entry["arms"])
            now = entry["arms"].get("swing")
            print(f"ARMS {name}: upper arms swung {', '.join(f'{s} {a:.0f} forward to {b:.0f} back' for s, (a, b) in took.items())} of the trunk in the take"
                  + (f", now {now[0]} to {now[1]}" if now else ", kept"))
        if "fist" in entry:
            close_hands(action, fist, entry["fist"])
        if entry.get("stand"):
            ad.action = action
            if getattr(action, "slots", None) and len(action.slots):
                ad.action_slot = action.slots[0]
            times = sorted({kp.co[0] for fc in curves(action) if fc.data_path == 'pose.bones["pelvis"].location' for kp in fc.keyframe_points})
            rise, c, back, lumbar = stand.stand(arm, action, times, bpy.context.view_layer.update, without_roll)
            print(f"STAND {name}: lower back's held bend of {lumbar:.1f} deg taken out, hips raised {rise:.1f} cm "
                  f"(each knee's bend over {stand.KNEE_MIN:.0f} deg kept {c:.2f} of the take's), head drawn back {back:.1f} cm at the neck")
        bpy.data.objects.remove(src)
        ad.action = None
        if old is not None:
            bpy.data.actions.remove(old)
        if name not in names:
            names.append(name)
        print(f"MADE {name} from {os.path.relpath(path, look.ROOT)}: frames {f0} to {f1}, {(f1 - f0) / fps:.2f} s, seam {seam_turn:.1f} deg, {seam_at:.1f} cm "
              f"(apart {seam:.2f} deg)")
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
    # Into the file it read, only the clips made are written (glb_clips.ts): exporting the whole
    # mannequin again would move its rest a little every time (Blender's glTF round trip), so a
    # run with nothing changed would not give the same bytes (D563). A set (--list) is written
    # whole into its own copy.
    made_into = out if chosen else out + ".made.glb"
    bpy.ops.export_scene.gltf(
        filepath=made_into, export_format="GLB", use_selection=True,
        export_animations=True, export_animation_mode="NLA_TRACKS", export_force_sampling=True,
        export_influence_nb=4,
    )
    if not chosen:
        try:
            if takes:
                put = subprocess.run(["node", os.path.join(HERE, "glb_clips.ts"), out, made_into, *takes], capture_output=True, text=True)
                print(put.stdout.strip() or put.stderr.strip())
                if put.returncode:
                    raise SystemExit(f"glb_clips.ts exited {put.returncode}")
        finally:
            os.remove(made_into)
    print("MADE", len(takes), "clips into", os.path.relpath(out, look.ROOT))


main()
