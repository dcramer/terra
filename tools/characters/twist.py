"""Twist bones: a limb's roll about its own length spread along its skin, as engines do it for
clips made without twist bones (Unreal's post-process twist bones, Unity's humanoid arm-twist
setting, HumanIK's roll extraction; research/skin-deformation.md).

Quaternius's clips never roll the upper arm or thigh. The whole roll of an arm is in its
forearm (up to 164 degrees against the upper arm, half an arm's turn) and of a leg in its
shin (up to 107), measured on both libraries (2026-10-12). Skinned as it comes, the skin
round the elbow or knee turns all of that at once and wrings shut. So, for every clip:
  1. the roll of each segment in ROLL moves to the bone beyond it: the forearm only bends at
     the elbow and the hand turns, as Unreal's mannequin keeps its pronation in the hand. Where
     the hand ends up is unchanged: only which bone carries the turn.
  2. each twist bone in TWIST turns by its share of the roll between its segment and the bone
     beyond it, so the skin along the forearm turns from none at the elbow to all of it at the
     wrist, in steps.
Tripo's rig (convert_rig.py) brings one twist bone per segment, weighted, halfway along it.

The game plays clips, so the turns are baked into every clip as it is sampled for a body
(`bake`, by export_character.py and anything else that plays the libraries' clips on a body).
"""

import math

import bpy
from mathutils import Quaternion, Vector

SIDES = ("l", "r")
# Segment -> the bone beyond it, which takes over the segment's roll.
ROLL = (("lowerarm", "hand"), ("calf", "foot"))
# Twist bone -> (its segment, the bone beyond, its share of the roll between them). Halfway
# along, half the roll. The upper arm's and thigh's follow their bone: the clips never roll
# those, and their roll would belong at the shoulder and hip, which ROLL does not move.
TWIST = (
    ("lowerarm_twist_01", "lowerarm", "hand", 0.5),
    ("calf_twist_01", "calf", "foot", 0.5),
    ("upperarm_twist_01", "upperarm", "lowerarm", 0.0),
    ("thigh_twist_01", "thigh", "calf", 0.0),
)


def names():
    """Every twist bone's name, with the segment it is on."""
    return [(f"{t}_{s}", f"{seg}_{s}") for s in SIDES for t, seg, _, _ in TWIST]


def add(arm, heads):
    """Adds each twist bone at its head in `heads` (armature space), a child of its segment and
    facing exactly as it does, so its own roll is the segment's."""
    bpy.ops.object.select_all(action="DESELECT")
    arm.select_set(True)
    bpy.context.view_layer.objects.active = arm
    bpy.ops.object.mode_set(mode="EDIT")
    eb = arm.edit_bones if hasattr(arm, "edit_bones") else arm.data.edit_bones
    for name, seg in names():
        if name not in heads or seg not in eb:
            continue
        main = eb[seg]
        tb = eb.get(name) or eb.new(name)
        tb.head = heads[name]
        tb.tail = tb.head + (main.tail - main.head).normalized() * 0.05
        tb.roll = main.roll
        tb.parent = main
        tb.use_connect = False
        tb.use_deform = True
    bpy.ops.object.mode_set(mode="OBJECT")


def axis(arm, seg, beyond, frame=None):
    """The segment's line, from its joint to the next, in the rest frame of bone `frame` (by
    default the segment's own). Blender's importer guesses each bone's axes, so a twist bone
    exported facing as its segment may come back facing otherwise: each bone's own frame."""
    b = arm.data.bones
    return (b[frame or seg].matrix_local.to_3x3().inverted() @ (b[beyond].head_local - b[seg].head_local)).normalized()


def split(q, a):
    """`q` as swing times twist about `a` (q = swing @ twist); the twist's angle, in (-pi, pi]."""
    v = Vector((q.x, q.y, q.z))
    p = a * v.dot(a)
    t = Quaternion((q.w, p.x, p.y, p.z))
    if t.magnitude < 1e-9:
        return q.copy(), Quaternion(), 0.0
    t.normalize()
    if t.w < 0:
        t.negate()
    angle = 2.0 * math.atan2(Vector((t.x, t.y, t.z)).dot(a), t.w)
    return q @ t.inverted(), t, angle


def unwrapped(angles):
    """A clip's roll, frame by frame, as one continuous turn: each frame's angle (given the
    short way round, in (-pi, pi]) moved by whole turns to follow the frame before, then the
    whole clip by whole turns to keep its largest roll as small as can be. A roll past half a
    turn is ambiguous in any one frame (188 degrees one way is 172 the other); the frames
    around it are not, so no one frame decides which way the skin turns."""
    out = []
    for a in angles:
        if out:
            a += 2.0 * math.pi * round((out[-1] - a) / (2.0 * math.pi))
        out.append(a)
    if not out:
        return out
    # A clip that ends more than half a turn from where it began winds: a loop whose limb
    # circles, as an arm does in a crawl stroke, gains a whole turn of roll on the rest pose's
    # axis each time round (Codman's paradox). No twist bone can follow that without turning
    # back half a turn once a loop; taken frame by frame, as engines take a twist bone's roll
    # (Unreal's twist corrective, Unity's arm twist), that turn falls where the roll passes
    # half a turn either way, and no frame wrings the skin more than a quarter turn a step.
    if abs(out[-1] - out[0]) > math.pi:
        return list(angles)
    shift = min((2.0 * math.pi * k for k in range(-3, 4)), key=lambda d: max(abs(a + d) for a in out))
    return [a + shift for a in out]


def jumps(angles, share, loop):
    """How a twist bone keyed `share` of `angles` (as unwrapped gives them) turns between
    frames, across the seam too for a loop: its largest turn the short way, in degrees; how
    often it flips (turns back more than half a turn of roll at once); and how many flips are
    the loop's own, one for each whole turn the roll winds round it. A clip that is not a loop
    owns none: unwrapped follows it, so a flip in it is a flip in its data."""
    steps = [b - a for a, b in zip(angles, angles[1:])]
    if loop and angles:
        steps.append(angles[0] - angles[-1])
    short = [math.atan2(math.sin(d), math.cos(d)) for d in steps]
    largest = max((share * abs(math.degrees(d)) for d in short), default=0.0)
    flips = sum(1 for d in steps if abs(d) > math.pi)
    wound = round(abs(sum(short)) / (2.0 * math.pi)) if loop else 0
    return largest, flips, wound


def rolled(arm, action):
    """One clip on `arm` with each ROLL segment's roll moved to the bone beyond it: per frame,
    the rotations of the segments and the bones beyond, and per TWIST pair (segment, bone
    beyond) the roll between them, unwrapped over the clip; and the largest change, in
    degrees, the move made to where any bone beyond ends up (none, but for rounding). The
    twist bones need not exist."""
    bones = arm.data.bones
    pose = arm.pose.bones
    rolls = [(f"{seg}_{s}", f"{to}_{s}") for s in SIDES for seg, to in ROLL if f"{seg}_{s}" in bones]
    pairs = [(f"{seg}_{s}", f"{to}_{s}") for s in SIDES for _, seg, to, _ in TWIST if f"{seg}_{s}" in bones]
    ad = arm.animation_data or arm.animation_data_create()
    ad.action = action
    if getattr(action, "slots", None) and len(action.slots):
        ad.action_slot = action.slots[0]
    start, end = (int(round(f)) for f in action.frame_range)
    wanted = {n for pair in rolls + pairs for n in pair}
    frames = []
    for f in range(start, end + 1):
        bpy.context.scene.frame_set(f)
        frames.append((f, {n: pose[n].matrix_basis.to_quaternion() for n in wanted}))
    angles = {pair: [] for pair in pairs}
    moved = 0.0
    for _, q in frames:
        for seg, to in rolls:
            swing, twist, _ = split(q[seg], axis(arm, seg, to))
            # The beyond bone's rest, relative to the segment's: the twist expressed in it.
            h = (bones[seg].matrix_local.to_3x3().inverted() @ bones[to].matrix_local.to_3x3()).to_quaternion()
            before = q[seg] @ h @ q[to]
            q[seg] = swing
            q[to] = h.inverted() @ twist @ h @ q[to]
            moved = max(moved, math.degrees(before.rotation_difference(q[seg] @ h @ q[to]).angle))
        for seg, to in pairs:
            h = (bones[seg].matrix_local.to_3x3().inverted() @ bones[to].matrix_local.to_3x3()).to_quaternion()
            angles[(seg, to)].append(split(h @ q[to] @ h.inverted(), axis(arm, seg, to))[2])
    return frames, rolls, {pair: unwrapped(a) for pair, a in angles.items()}, moved


def bake(arm, actions):
    """Moves each ROLL segment's roll to the bone beyond it and keys each twist bone's share,
    every frame of each of `actions`, on `arm`."""
    pose = arm.pose.bones
    twists = [(f"{t}_{s}", f"{seg}_{s}", f"{to}_{s}", share) for s in SIDES for t, seg, to, share in TWIST
              if f"{t}_{s}" in arm.data.bones]
    if not twists:
        return
    ad = arm.animation_data or arm.animation_data_create()
    keep = (ad.action, getattr(ad, "action_slot", None))
    for action in actions:
        # Everything is read before anything is keyed, so a key never changes what is read.
        frames, rolls, turns, _ = rolled(arm, action)
        for f, q in frames:
            for pair in rolls:
                for n in pair:
                    pose[n].rotation_mode = "QUATERNION"
                    pose[n].rotation_quaternion = q[n]
                    pose[n].keyframe_insert("rotation_quaternion", frame=f, group=n)
        for name, seg, to, share in twists:
            pb = pose[name]
            pb.rotation_mode = "QUATERNION"
            a = axis(arm, seg, to, name)
            for (f, _), angle in zip(frames, turns[(seg, to)]):
                pb.rotation_quaternion = Quaternion(a, share * angle)
                pb.keyframe_insert("rotation_quaternion", frame=f, group=name)
    ad.action, slot = keep
    if slot is not None:
        ad.action_slot = slot
