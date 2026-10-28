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

A hold may also lay a new pose on the held clip's, after its turns: `aim` points bones, in
order, each along a way in the body's frame (`[left, up, ahead]`, as the game's model space
is: +X left, +Y up, +Z ahead), turned the least way at its joint with what is below it, as an
aim constraint does (Unreal's Look At, Blender's Damped Track); `roll` then turns a bone about
its own length by degrees (right-handed about the way it points), with what is below it. The
fight's light blows carry the shield low at the left side so (the upper arm down and out, the
forearm forward, the board's face forward and out; research/2026-10-25-shield-prior-art.md):

    {"clip": "Keyed_Guard", "bone": "clavicle_l",
     "aim": {"upperarm_l": [0.55, -0.75, -0.38], "lowerarm_l": [-0.1, -0.55, 0.83]},
     "roll": {"lowerarm_l": 47}, "in": [0.1, 0.3], "out": [1.35, 1.75]}

A hold may also take `in` and `out` (clip seconds, `[from, to]` each): the part plays the `of`
clip until `in`'s first second, eases to the held pose by its last, stays held, and eases back
to the `of` clip over `out` (either may be left out, the part then held from the start or to
the end). An `out` before the `in` holds the part at both ends and plays the `of` clip
between: the block (`Clear_Block`) eases its shield arm from the low rest into its raise and
back to the rest as it lowers (D531). It is Unreal's Layered Blend per Bone with its weight driven by a curve, a Unity
layer's weight: each held bone slerped in its own parent frame from the `of` clip's pose to the
held one by a smoothstep weight, keyed every frame. The fight's light blows drop the guard so
(the shield out to the left through the blow and back to the guard in its recovery, as
Enshrouded's does, research/2026-10-25-shield-arm-in-a-swing.md).

An entry with `over` (a list of clips) in place of `of` lays its holds over each of those clips
in place, after the clips made from others so they take it too, held throughout (no `in` or
`out`). The strapped heater's straps fix the left hand to the forearm, so its grip (the fist
turned onto the hand strap, the back of the hand to the board) is held so over every clip a
fighter plays (D459):

    "Strap_Grip": {"over": ["Keyed_Guard", ...], "hold": [
        {"clip": "Keyed_Guard", "bone": "hand_l", "aim": {...}, "roll": {"hand_l": -96}}, ...]}
"""

import json
import math
import os

import bpy
import numpy as np
from mathutils import Matrix, Quaternion, Vector

CHANNELS = ("location", "rotation_quaternion", "scale")
TARGETS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "art", "animation", "shield_targets.json")
# The shield arm the `shield` op solves, the chest's bone the target is in, and the fist's socket.
ARM = ("clavicle_l", "upperarm_l", "lowerarm_l", "hand_l")
CHEST = "spine_03"
FIST = "socket.hand.L"
# How much a turn of each bone costs against a centimetre off the target, per radian: the
# shoulder blade moves least, so the solve shrugs only when the arm cannot reach.
COST = {"clavicle_l": 4.0, "upperarm_l": 0.5, "lowerarm_l": 0.5}
# How much the board's face or up off its target weighs, per radian, against a centimetre off
# its centre: 3 degrees as 1 cm, the move check's bars (1 cm, 3 deg) weighing alike.
PER_RADIAN = 1.0 / math.radians(3.0)


def _gl(v):
    """A way in the body's glTF axes (+X left, +Y up, +Z ahead) in Blender's (Z up, ahead -Y)."""
    return Vector((v[0], -v[2], v[1]))


def _fk(arm, basis):
    """The shield arm's bones (`ARM`) in the world, posed by their own location, rotation and
    scale (`basis`, by name; a bone not in it as it is posed now), below its parent as posed."""
    pose = arm.pose.bones
    bones = arm.data.bones
    M = pose[bones[ARM[0]].parent.name].matrix.copy()
    above = bones[ARM[0]].parent.matrix_local
    out = {}
    for n in ARM:
        b = bones[n]
        M = M @ (above.inverted() @ b.matrix_local) @ basis.get(n, pose[n].matrix_basis)
        above = b.matrix_local
        out[n] = arm.matrix_world @ M
    return out


def _board(arm, world, fist, board):
    """The strapped board in the world at the arm's pose `world`: its centre, face and up. As
    terra_walker::strap hangs it: its origin at the fist (`fist`, the socket in the hand's
    frame), turned in the true forearm's frame, the forearm bone's turned to run from the elbow
    to the wrist; `board` its centre, face and up in that frame (shield_targets.json)."""
    hand, fore = world["hand_l"], world["lowerarm_l"]
    at = (hand @ fist).translation
    R = fore.to_3x3().normalized()
    R = R.col[1].rotation_difference((hand.translation - fore.translation).normalized()).to_matrix() @ R
    return at + R @ Vector(board["centre"]), R @ Vector(board["face"]), R @ Vector(board["up"])


def _solve(arm, name, target, board, hand):
    """Turns the shield arm (shoulder blade, upper arm, forearm with its roll) from its pose now
    the least way that puts the strapped board on `target` (its centre, face and up in the
    chest's frame, shield_place.ts), the hand held at `hand` (its pose in its own frame, the
    clips' strap grip): a damped Gauss-Newton solve over each bone's turn in its own frame, as
    an IK control on the prop is baked to the arm in a DCC (D554). Returns how far off it ends."""
    pose = arm.pose.bones
    W = arm.matrix_world
    chest = W @ pose[CHEST].matrix
    rest = W @ arm.data.bones[CHEST].matrix_local
    Cr = chest.to_3x3().normalized() @ rest.to_3x3().normalized().inverted()
    want = (chest.translation + Cr @ _gl(target["centre"]), Cr @ _gl(target["face"]), Cr @ _gl(target["up"]))
    socket = bpy.data.objects[FIST]
    fist = (W @ pose["hand_l"].matrix).inverted() @ socket.matrix_world
    start = {n: pose[n].matrix_basis.decompose() for n in COST}
    turned = list(COST)

    def basis(x):
        out = {"hand_l": hand}
        for k, n in enumerate(turned):
            loc, rot, scale = start[n]
            d = Vector(x[3 * k : 3 * k + 3])
            q = Quaternion(d.normalized(), d.length) if d.length > 1e-12 else Quaternion()
            out[n] = Matrix.LocRotScale(loc, rot @ q, scale)
        return out

    def residual(x):
        c, f, u = _board(arm, _fk(arm, basis(x)), fist, board)
        off = [*(100.0 * (c - want[0])), *(PER_RADIAN * (f - want[1])), *(PER_RADIAN * (u - want[2]))]
        return np.array(off + [COST[n] * v for k, n in enumerate(turned) for v in x[3 * k : 3 * k + 3]])

    x = np.zeros(3 * len(turned))
    for _ in range(60):
        r = residual(x)
        J = np.empty((len(r), len(x)))
        for j in range(len(x)):
            e = np.zeros(len(x))
            e[j] = 1e-5
            J[:, j] = (residual(x + e) - r) / 1e-5
        step = np.linalg.solve(J.T @ J + 1e-6 * np.eye(len(x)), -J.T @ r)
        x = x + step
        if np.linalg.norm(step) < 1e-7:
            break
    for n, m in basis(x).items():
        if n in COST:
            pose[n].matrix_basis = m
    bpy.context.view_layer.update()
    c, f, u = _board(arm, _fk(arm, {"hand_l": hand}), fist, board)
    cm = (c - want[0]).length * 100.0
    deg = max(math.degrees(f.angle(want[1])), math.degrees(u.angle(want[2])))
    turns = ", ".join(f"{n} {math.degrees(np.linalg.norm(x[3 * k : 3 * k + 3])):.1f} deg" for k, n in enumerate(turned))
    print(f"SHIELD {name}: the board on {target.get('pose', '')} target {cm:.2f} cm, {deg:.2f} deg off; turned {turns}")
    return cm, deg


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


def _held_pose(arm, by_name, name, held, entries=None):
    """A hold's part as it is to be held: each bone's location, rotation and scale in its own
    frame, at the held clip's first frame with the hold's turns laid on."""
    hand = None
    if "shield" in held:
        # The hand as the clips hold it on the strap (`hand`, an entry laid over the clips).
        grip = (entries or {}).get(held.get("hand", "Strap_Grip"))
        if grip is None:
            raise SystemExit(f"{name}: no entry {held.get('hand', 'Strap_Grip')} for the hand on the strap")
        for g in grip["hold"]:
            at = _held_pose(arm, by_name, name, g)
            if "hand_l" in at:
                loc, rot, scale = at["hand_l"]
                hand = Matrix.LocRotScale(loc, rot, scale)
        if hand is None:
            raise SystemExit(f"{name}: {held.get('hand', 'Strap_Grip')} holds no hand_l")
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
    to_arm = arm.matrix_world.to_3x3().inverted()
    for bone, way in held.get("aim", {}).items():
        if bone not in bones:
            raise SystemExit(f"{name}: {bone} is not in the part below {held['bone']}")
        pb = pose[bone]
        # The body's frame as Blender imports glTF: +X left, glTF +Y up is Blender +Z, glTF
        # +Z ahead is Blender -Y.
        want = (to_arm @ Vector((way[0], -way[2], way[1]))).normalized()
        now = (pb.tail - pb.head).normalized()
        head = pb.head.copy()
        swing = now.rotation_difference(want).to_matrix().to_4x4()
        pb.matrix = Matrix.Translation(head) @ swing @ Matrix.Translation(-head) @ pb.matrix
        bpy.context.view_layer.update()
    for bone, degrees in held.get("roll", {}).items():
        if bone not in bones:
            raise SystemExit(f"{name}: {bone} is not in the part below {held['bone']}")
        pb = pose[bone]
        head = pb.head.copy()
        along = (pb.tail - pb.head).normalized()
        pb.matrix = Matrix.Translation(head) @ Matrix.Rotation(math.radians(degrees), 4, along) @ Matrix.Translation(-head) @ pb.matrix
        bpy.context.view_layer.update()
    if "shield" in held:
        with open(TARGETS) as f:
            kept = json.load(f)
        target = kept["targets"].get(held["shield"])
        if target is None or "board" not in kept:
            raise SystemExit(f"{name}: no target {held['shield']} or no strapped board in {TARGETS}")
        _solve(arm, name, {**target, "pose": held["shield"]}, kept["board"], hand)
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
    no `in` or `out`; held at both ends and played between when `out` comes before `in`."""
    if "in" in held and "out" in held and held["out"][1] <= held["in"][0]:
        return max(_ease(t, held["out"], False), _ease(t, held["in"], True))
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
            at = _held_pose(arm, by_name, name, held, entries)
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
        _key_still(arm, still, start, end)
        actions.append(action)
        made.append(action)
        bones = len(still) + sum(len(at) for _, at in eased)
        print(f"HOLD {name}: {entry['of']} with {_said(holds)} ({bones} bones), frames {start} to {end} at {fps:g} fps")
    # Holds laid over clips in place (`over`), after the clips made above, so those take them too.
    by_name = {a.name: a for a in actions}
    for name, entry in entries.items():
        if "over" not in entry or "hold" not in entry:
            continue
        holds = entry["hold"] if isinstance(entry["hold"], list) else [entry["hold"]]
        if any("in" in h or "out" in h for h in holds):
            raise SystemExit(f"{name}: a hold laid over clips is held throughout (no in or out)")
        # Every held pose is read before any clip changes: a clip held over may be the one held.
        still = {}
        for held in holds:
            still.update(_held_pose(arm, by_name, name, held, entries))
        for clip in entry["over"]:
            action = by_name.get(clip)
            if action is None:
                raise SystemExit(f"{name}: no clip {clip} to lay its hold over")
            for bag in _bags(action):
                for c in [c for c in bag if any(c.data_path.startswith(f'pose.bones["{n}"]') for n in still)]:
                    bag.remove(c)
            _use(arm, action)
            start, end = (int(round(f)) for f in action.frame_range)
            _key_still(arm, still, start, end)
        print(f"HOLD {name}: {_said(holds)} ({len(still)} bones) laid over {', '.join(entry['over'])}")
    ad.action, slot = keep
    if slot is not None:
        ad.action_slot = slot
    return made


def _key_still(arm, still, start, end):
    """Keys each of `still`'s bones at its held location, rotation and scale, at the first and
    last frames of the clip in use."""
    for n, (loc, rot, scale) in still.items():
        pb = arm.pose.bones[n]
        pb.rotation_mode = "QUATERNION"
        pb.location, pb.rotation_quaternion, pb.scale = loc, rot, scale
        for f in (start, end):
            for channel in CHANNELS:
                pb.keyframe_insert(channel, frame=f, group=n)


def _said(holds):
    """Holds in words, for a HOLD line."""
    return "; ".join(f"{h['bone']} and below at {h['clip']}'s first pose"
                     + "".join(f", {b} turned {d:+.0f} deg" for b, d in h.get("turn", {}).items())
                     + "".join(f", {b} aimed along {tuple(round(x, 2) for x in w)}" for b, w in h.get("aim", {}).items())
                     + "".join(f", {b} rolled {d:+.0f} deg" for b, d in h.get("roll", {}).items())
                     + (f", the arm solved to {h['shield']}'s board" if "shield" in h else "")
                     + "".join(f", {k} {h[k][0]:.2f}-{h[k][1]:.2f} s" for k in ("in", "out") if k in h) for h in holds)
