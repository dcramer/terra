"""Each moving clip's own pace: how fast the ground must pass under the body for its feet
not to slide (docs/characters.md). A planted foot moves backward under the body at the speed
the body moves over the ground; measured here as the middle of the speeds at which either
foot, near its lowest, moves backward (the body faces -Y), or across for a clip that steps
sideways (a strafe). The game plays each clip at the
speed the character moves divided by its pace, so feet stay where they are put, as every
engine's locomotion does. Clips that do not travel (an idle, a swing) have no pace.
"""

import math

import bpy
from mathutils import Vector

FEET = ("ball_l", "ball_r")
LOW_M = 0.05        # a foot this near its lowest in the clip may be planted
SLOWEST = 0.3       # a clip whose feet move back slower than this, in m/s, does not travel


def pace(arm, action):
    """`action`'s pace on `arm`, in metres a second, or None if it does not travel."""
    ad = arm.animation_data or arm.animation_data_create()
    ad.action = action
    if getattr(action, "slots", None) and len(action.slots):
        ad.action_slot = action.slots[0]
    scene = bpy.context.scene
    fps = scene.render.fps / scene.render.fps_base
    start, end = (int(round(x)) for x in action.frame_range)
    feet = []
    for f in range(start, end + 1):
        scene.frame_set(f)
        feet.append([arm.matrix_world @ arm.pose.bones[b].head for b in FEET])
    # The way a planted foot passes under the body: backward (+Y) for a clip that travels
    # ahead or near it, else the way the feet near their lowest mostly move, for one that
    # travels sideways (a strafe), its planted feet passing under it across.
    moving = []
    for side in range(len(FEET)):
        low = min(at[side].z for at in feet)
        moving += [((feet[k][side] - feet[k - 1][side]) * fps).xy for k in range(1, len(feet)) if feet[k][side].z < low + LOW_M]
    if not moving:
        return None
    typical = Vector((sorted(v.x for v in moving)[len(moving) // 2], sorted(v.y for v in moving)[len(moving) // 2]))
    way = Vector((0.0, 1.0))
    if typical.length > 1e-6 and abs(typical.normalized().dot(way)) < math.cos(math.radians(45)):
        way = typical.normalized()
    speeds = []
    for side in range(len(FEET)):
        low = min(at[side].z for at in feet)
        for k in range(1, len(feet)):
            back = ((feet[k][side] - feet[k - 1][side]) * fps).xy.dot(way)
            if back > 0 and feet[k][side].z < low + LOW_M:
                speeds.append(back)
    if not speeds:
        return None
    speeds.sort()
    middle = speeds[len(speeds) // 2]
    return round(middle, 3) if middle >= SLOWEST else None


def paces(arm, actions):
    """Each travelling clip of `actions` and its pace."""
    out = {}
    for action in actions:
        if all(b in arm.pose.bones for b in FEET):
            p = pace(arm, action)
            if p is not None:
                out[action.name.split(".")[0]] = p
    return out
