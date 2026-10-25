"""A clip made from another with parts of the body held as a third clip holds them: a layered
blend per bone, as Unreal's Layered Blend per Bone and Unity's avatar masks lay an arm's pose
over a whole-body clip, baked into the clip here so the game and the move check (moves.ts)
play it as any other. The fight's roll is the library's forward roll with the arms held as
the guard holds them, swung out to the sides, so the strapped shield and the sword stay
beside the body through the tuck rather than through it (the move check, D447).

An entry of a set's list (art/animation/sets/fight.json) asks for one:

    "Fight_Roll": {"of": "Roll", "hold": [
        {"clip": "Keyed_Guard", "bone": "clavicle_l", "turn": {"upperarm_l": 60}}, ...]}

`of` is the clip it is made from. Each hold: `clip` the clip whose first pose the part takes,
`bone` the part (that bone and every bone below it), held at that pose for the whole clip in
their own frames, so the part goes where its parent takes it (an arm stays as the chest holds
it); `turn` lays a turn on that pose, a bone turned that many degrees about the body's up axis
at its joint (+ to the left, seen from above) with what is below it. export_character.py bakes
it on each body after its clips are aimed and twisted, so the part is exactly the held clip's
on that body.

A hold may also take `in` and `out` (clip seconds, `[from, to]` each): the part plays the `of`
clip until `in`'s first second, eases to the held pose by its last, stays held, and eases back
to the `of` clip over `out` (either may be left out, the part then held from the start or to
the end). It is Unreal's Layered Blend per Bone with its weight driven by a curve, a Unity
layer's weight: each held bone slerped in its own parent frame from the `of` clip's pose to the
held one by a smoothstep weight, keyed every frame. The fight's light blows drop the guard so
(the shield out to the left through the blow and back to the guard in its recovery, as
Enshrouded's does, research/2026-10-25-shield-arm-in-a-swing.md).
"""

import math

import bpy
from mathutils import Matrix, Vector

CHANNELS = ("location", "rotation_quaternion", "scale")


def part(arm, bone):
    """`bone` and every bone below it, by name."""
    top = arm.data.bones[bone]
    return {top.name} | {b.name for b in top.children_recursive}


def _bags(action):
    """The fcurve collections of an action: its own (an older Blender) or each slot's."""
    if hasattr(action, "fcurves"):
        return [action.fcurves]
    return [bag.fcurves for layer in action.layers for strip in layer.strips for bag in strip.channelbags]


def _use(arm, action):
    ad = arm.animation_data or arm.animation_data_create()
    ad.action = action
    if getattr(action, "slots", None) and len(action.slots):
        ad.action_slot = action.slots[0]


def _held_pose(arm, by_name, name, held):
    """A hold's part as it is to be held: each bone's location, rotation and scale in its own
    frame, at the held clip's first frame with the hold's turns laid on."""
    pose = arm.pose.bones
    from_clip = by_name.get(held["clip"])
    if from_clip is None:
        raise SystemExit(f"{name}: no clip {held['clip']} to hold a part at")
    bones = part(arm, held["bone"])
    _use(arm, from_clip)
    bpy.context.scene.frame_set(int(round(from_clip.frame_range[0])))
    up = (arm.matrix_world.to_3x3().inverted() @ Vector((0.0, 0.0, 1.0))).normalized()
    for bone, degrees in held.get("turn", {}).items():
        if bone not in bones:
            raise SystemExit(f"{name}: {bone} is not in the part below {held['bone']}")
        pb = pose[bone]
        head = pb.matrix.translation.copy()
        pb.matrix = Matrix.Translation(head) @ Matrix.Rotation(math.radians(degrees), 4, up) @ Matrix.Translation(-head) @ pb.matrix
        bpy.context.view_layer.update()
    out = {}
    for n in bones:
        loc, rot, scale = pose[n].matrix_basis.decompose()
        out[n] = (loc.copy(), rot.copy(), scale.copy())
    return out


def _ease(t, window, rising):
    """A smoothstep from 0 at a window's first second to 1 at its last (1 - that if falling)."""
    lo, hi = window
    s = 1.0 if t >= hi else 0.0 if hi <= lo or t <= lo else (t - lo) / (hi - lo)
    s = s * s * (3.0 - 2.0 * s)
    return s if rising else 1.0 - s


def weight(t, held):
    """How far a hold's part is held at `t` seconds into its clip: 1 throughout for a hold with
    no `in` or `out`."""
    w = 1.0
    if "in" in held:
        w *= _ease(t, held["in"], True)
    if "out" in held:
        w *= _ease(t, held["out"], False)
    return w


def _sampled(arm, action, bones, start, end):
    """Each of `bones`' location, rotation and scale in its own frame at every frame of
    `action` from `start` to `end`; a bone the action does not key at rest."""
    pose = arm.pose.bones
    _use(arm, action)
    for n in bones:
        pose[n].matrix_basis = Matrix.Identity(4)
    out = {}
    for f in range(start, end + 1):
        bpy.context.scene.frame_set(f)
        out[f] = {n: tuple(v.copy() for v in pose[n].matrix_basis.decompose()) for n in bones}
    return out


def bake(arm, actions, entries):
    """For each entry of a list with `of` and `hold`, a new clip by its name: `of` copied with
    each hold's part held. Returns the new actions (each also appended to `actions`), and
    prints a HOLD line each."""
    pose = arm.pose.bones
    by_name = {a.name: a for a in actions}
    ad = arm.animation_data or arm.animation_data_create()
    keep = (ad.action, getattr(ad, "action_slot", None))
    made = []
    for name, entry in entries.items():
        if "of" not in entry or "hold" not in entry:
            continue
        holds = entry["hold"] if isinstance(entry["hold"], list) else [entry["hold"]]
        src = by_name.get(entry["of"])
        if src is None:
            raise SystemExit(f"{name}: no clip {entry['of']} to make it from")
        still, eased = {}, []
        for held in holds:
            at = _held_pose(arm, by_name, name, held)
            if "in" in held or "out" in held:
                eased.append((held, at))
            else:
                still.update(at)
        fps = bpy.context.scene.render.fps / bpy.context.scene.render.fps_base
        first, last = (int(round(f)) for f in src.frame_range)
        played = _sampled(arm, src, {n for _, at in eased for n in at}, first, last) if eased else {}
        action = src.copy()
        old = by_name.get(name)
        if old is not None:
            actions.remove(old)
            bpy.data.actions.remove(old)
        action.name = name
        for bag in _bags(action):
            for c in [c for c in bag if any(c.data_path.startswith(f'pose.bones["{n}"]') for n in still)]:
                bag.remove(c)
            for c in [c for c in bag if any(c.data_path.startswith(f'pose.bones["{n}"]') for _, at in eased for n in at)]:
                bag.remove(c)
        _use(arm, action)
        start, end = (int(round(f)) for f in action.frame_range)
        for held, at in eased:
            for f in range(first, last + 1):
                w = weight((f - first) / fps, held)
                for n, (loc, rot, scale) in at.items():
                    ploc, prot, pscale = played[f][n]
                    pb = pose[n]
                    pb.rotation_mode = "QUATERNION"
                    pb.location = ploc.lerp(loc, w)
                    pb.rotation_quaternion = prot.slerp(rot, w)
                    pb.scale = pscale.lerp(scale, w)
                    for channel in CHANNELS:
                        pb.keyframe_insert(channel, frame=f, group=n)
        for n, (loc, rot, scale) in still.items():
            pb = pose[n]
            pb.rotation_mode = "QUATERNION"
            pb.location, pb.rotation_quaternion, pb.scale = loc, rot, scale
            for f in (start, end):
                for channel in CHANNELS:
                    pb.keyframe_insert(channel, frame=f, group=n)
        actions.append(action)
        made.append(action)
        parts = "; ".join(f"{h['bone']} and below at {h['clip']}'s first pose"
                          + "".join(f", {b} turned {d:+.0f} deg" for b, d in h.get("turn", {}).items())
                          + "".join(f", {k} {h[k][0]:.2f}-{h[k][1]:.2f} s" for k in ("in", "out") if k in h) for h in holds)
        bones = len(still) + sum(len(at) for _, at in eased)
        print(f"HOLD {name}: {entry['of']} with {parts} ({bones} bones), frames {start} to {end} at {fps:g} fps")
    ad.action, slot = keep
    if slot is not None:
        ad.action_slot = slot
    return made
