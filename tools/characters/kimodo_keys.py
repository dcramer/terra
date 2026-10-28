"""The keys a combat clip's Kimodo takes are held to (D450): the set's guard pose as a full-body
keyframe at the clip's first and last frames, and the blade's path as right-hand (or, for a
shield move, left-hand) keys at the times the spec gives, each a pose solved by two-bone IK on
Kimodo's skeleton from the guard. Run in Kimodo's own Python (Kimodo speaks nothing else), by
tools/characters/clip.ts:

    $KIMODO_HOME/venv/bin/python -I tools/characters/kimodo_keys.py GUARD.npz KEYS.json OUT.json

GUARD.npz is the guard take as a Kimodo NPZ (kimodo_convert of its BVH). KEYS.json:
{"frame": the guard take's frame that is the pose, "seconds": the clip's length, "hand": [key],
"left": [key]}. A key: {"at": seconds, "grip": [right, ahead, up], "blade": [right, ahead, up],
"edge" (right) or "back" (left): [right, ahead, up], "yaw": degrees (+ to the left)}, metres
and directions in the body's frame from the hips at the guard pose. The grip is the middle of
the fist (the hand's socket); for the right hand the blade is the sword's line out of the fist
and the edge the way its working edge faces (the socket's -Z, the knuckles' side, which leads a
strike: D565 step 3); for the left the blade is the forearm's line toward the fist and the back
the shield's face. Unset, the hand keeps the guard's turn, as little changed as can be; `yaw`
turns the chest about the spine before the arm reaches.

The arm is solved as a person's reaches (D565 step 3): of every way the elbow can swing about
the line from the shoulder to the wrist, the one that keeps the wrist nearest straight and the
forearm's twist in range, the upper arm rolled so the elbow bends as a hinge and the forearm
carrying the hand's twist, so the hand bends off it only by flexion and deviation. A key no arm
can turn the hand to within a person's wrist (wrist.ts's limits, a margin inside them for
Kimodo's slack) is refused: its edge or blade wants changing.

The guard pose is turned to face +Z and set on the origin, as Kimodo's canonical start is.
Prints a KEY line a key: where its fist and blade ended up, how far short of the grip it fell
(a grip beyond the arm's reach is met at full stretch), the wrist's flexion and deviation and
the forearm's twist; exits 1 after them if a key is refused.

    $KIMODO_HOME/venv/bin/python -I tools/characters/kimodo_keys.py --reach GUARD.npz KEYS.json

writes no constraints: KEYS.json is {"frame", "keys": [key with "side": "Right" | "Left"]} and
one REACH line of JSON says each key's reach (the wrist, the forearm's twist, the elbow's swing
from the guard's, the upper arm's line), the keys' bar and the guard's own hands, for
tools/characters/keys.ts (check, search, between), which owns everything but the skeleton.
"""

import json
import math
import sys

import numpy as np
import torch
from scipy.spatial.transform import Rotation as R

from kimodo.skeleton import SOMASkeleton77

FPS = 30
REACH = sys.argv[1] == "--reach"
guard_path, keys_path, out_path = sys.argv[2:4] + [None] if REACH else sys.argv[1:4]
spec = json.load(open(keys_path))
sk = SOMASkeleton77()
names = sk.bone_order_names
idx = {n: i for i, n in enumerate(names)}
parents = sk.joint_parents.tolist()

d = np.load(guard_path)
frame = int(spec["frame"])
local = d["local_rot_mats"][frame].astype(np.float64).copy()
root = d["root_positions"][frame].astype(np.float64).copy()


def fk(local, root):
    g, p, _ = sk.fk(torch.from_numpy(local[None]).float(), torch.from_numpy(root[None]).float())
    return g[0].numpy().astype(np.float64), p[0].numpy().astype(np.float64)


def heading(p):
    r_hip, l_hip = sk.hip_joint_idx
    diff = p[r_hip] - p[l_hip]
    return math.atan2(diff[2], -diff[0])


# Faced +Z (Kimodo's heading 0) and on the origin.
_, p = fk(local, root)
best = None
for sign in (1, -1):
    turn = R.from_euler("y", sign * heading(p))
    h = heading(turn.apply(p - root) + root)
    if best is None or abs(h) < abs(best[1]):
        best = (turn, h)
turn = best[0]
local[0] = turn.as_matrix() @ local[0]
root = turn.apply(root)
root[0] = root[2] = 0.0
guard = local.copy()
G0, P0 = fk(guard, root)
hips = P0[0]


def body(v):
    """The body's frame (right, ahead, up) in Kimodo's (x left, y up, z ahead)."""
    return np.array([-v[0], v[2], v[1]], dtype=np.float64)


def unit(v):
    return v / (np.linalg.norm(v) or 1.0)


def align(a, b):
    """The least rotation taking direction a onto b."""
    a, b = unit(a), unit(b)
    axis = np.cross(a, b)
    s, c = np.linalg.norm(axis), float(np.dot(a, b))
    if s < 1e-9:
        if c > 0:
            return np.eye(3)
        perp = unit(np.cross(a, [1, 0, 0] if abs(a[0]) < 0.9 else [0, 1, 0]))
        return R.from_rotvec(perp * math.pi).as_matrix()
    return R.from_rotvec(axis / s * math.atan2(s, c)).as_matrix()


def to_local(G, j):
    p = parents[j]
    return G[j] if p < 0 else G[p].T @ G[j]


# Kimodo's rotations (its NPZ's, its constraints') are from its standard T-pose, the pose of
# every rotation the identity, the BVHs' rest (--bvh_standard_tpose) and what made_clips.py
# carries onto ours from. The grip's place in the hand there: the knuckles' middle drawn a
# fifth of the way back to the wrist, a little in from the palm, as export_character.py
# places the hand socket.
_, PT = fk(np.tile(np.eye(3), (len(names), 1, 1)), np.zeros(3))


# The T-pose's own axes, measured, since Kimodo's rest frame is not its world's: up from the
# hips to the head, ahead from the heel to the toes. The right fist's blade points ahead at the
# T-pose and the back of either hand up, the palms down.
T_UP = unit(PT[idx["Head"]] - PT[idx["Hips"]])
T_AHEAD = unit(PT[idx["LeftToeBase"]] - PT[idx["LeftFoot"]])
T_AHEAD = unit(T_AHEAD - np.dot(T_AHEAD, T_UP) * T_UP)


def grip_offset(side):
    wrist = PT[idx[f"{side}Hand"]]
    knuckles = sum(PT[idx[f"{side}Hand{f}1"]] for f in ("Index", "Middle", "Ring", "Pinky")) / 4
    return knuckles + 0.2 * (wrist - knuckles) - wrist - 0.015 * T_UP


# A person's wrist and forearm (tools/characters/wrist.ts, AAOS adult averages), degrees, less a
# margin Kimodo's takes stray by, so a take that meets its keys keeps the wrist in range: the
# forearm's twist from the T-pose's palm-down, + toward palm-up (supination).
WRIST = {"flex": 80, "ext": 70, "radial": 20, "ulnar": 35}
MARGIN = 10
TWIST = (-15, 180)


def arm_axes(side):
    """The arm's own axes at the T-pose: the upper arm's and forearm's lines, the elbow's hinge
    (the forearm swings toward ahead about it, + bending), the hand's line to the middle
    knuckle, out of its palm and toward its thumb, and the sign that makes twist + supination."""
    up, fore, hand = (PT[idx[f"{side}{b}"]] for b in ("Arm", "ForeArm", "Hand"))
    upper, forearm = unit(fore - up), unit(hand - fore)
    hinge = unit(np.cross(forearm, T_AHEAD))
    hinge = unit(hinge - np.dot(hinge, upper) * upper)
    along = unit(PT[idx[f"{side}HandMiddle1"]] - hand)
    palm = unit(-T_UP - np.dot(-T_UP, along) * along)
    thumb = PT[idx[f"{side}HandThumb1"]] - hand
    thumb = unit(thumb - np.dot(thumb, along) * along - np.dot(thumb, palm) * palm)
    sup = 1.0 if np.dot(R.from_rotvec(forearm * math.pi / 2).apply(T_AHEAD), T_UP) > 0 else -1.0
    return {"upper": upper, "forearm": forearm, "hinge": hinge, "along": along, "palm": palm, "thumb": thumb, "sup": sup}


AXES = {s: arm_axes(s) for s in ("Right", "Left")}
# The two lines of each hand a key sets, at the T-pose (see reach).
REF = {"Right": (T_AHEAD, T_UP),
       "Left": (AXES["Left"]["forearm"], unit(T_UP - np.dot(T_UP, AXES["Left"]["forearm"]) * AXES["Left"]["forearm"]))}


def bend(hand_g, f, ax):
    """The hand's flexion (+ toward the palm) and deviation (+ toward the thumb) off a forearm
    along f, degrees, as wrist.ts's bendOf."""
    h, n, a = hand_g @ ax["along"], hand_g @ ax["palm"], hand_g @ ax["thumb"]
    fh, fn, fa = float(f @ h), float(f @ n), float(f @ a)
    return math.degrees(math.atan2(-fn, fh)), math.degrees(math.atan2(-fa, math.hypot(fh, fn)))


REST_BEND = {s: bend(np.eye(3), AXES[s]["forearm"], AXES[s]) for s in AXES}


def twist_of(local, axis):
    """The twist of a rotation about an axis, degrees (swing-twist)."""
    x, y, z, w = R.from_matrix(local).as_quat()
    along = float(np.dot([x, y, z], axis))
    # The quaternion's sign is free: w >= 0 keeps the twist in -180..180, not 334 for -26.
    return math.degrees(2 * math.atan2(along, w)) if w >= 0 else math.degrees(2 * math.atan2(-along, -w))


def frame_of(a, b):
    """The rotation taking the orthonormal pair (a0, b0) of the T-pose onto (a, b), as columns."""
    return np.stack([a, b, np.cross(a, b)], axis=1)


def over(flex, dev, twist):
    """How far past a person's range a wrist and forearm are, degrees, the margin counted."""
    return max(0.0, flex - (WRIST["flex"] - MARGIN), -flex - (WRIST["ext"] - MARGIN), dev - (WRIST["radial"] - MARGIN),
               -dev - (WRIST["ulnar"] - MARGIN), TWIST[0] - twist, twist - TWIST[1])


refused = []


def reach(key, side):
    """The guard pose with one arm reaching for a key, as a person reaches, and how it reached."""
    loc = guard.copy()
    if key.get("yaw"):
        G, _ = fk(loc, root)
        spin = R.from_euler("y", math.radians(key["yaw"])).as_matrix()
        j = idx["Spine1"]
        loc[j] = G[parents[j]].T @ spin @ G[j]
    G, P = fk(loc, root)
    up, fore, hand = idx[f"{side}Arm"], idx[f"{side}ForeArm"], idx[f"{side}Hand"]
    ax = AXES[side]
    # The hand's turn from the T-pose, by two of its lines there: the right's blade (the sword
    # pointing ahead) and back of the hand (up), its back from its edge (the socket's -Z, out
    # along the arm): back = edge x blade; the left's blade the forearm's line out to the fist
    # and its back the back of the hand, the shield's face.
    have = G[hand]
    rest = frame_of(*REF[side])
    blade = unit(body(key["blade"])) if "blade" in key else have @ REF[side][0]
    if "edge" in key:
        back = np.cross(unit(body(key["edge"])), blade)
    else:
        back = body(key["back"]) if "back" in key else have @ REF[side][1]
    back = unit(back - np.dot(back, blade) * blade)
    want_std = frame_of(blade, back) @ rest.T
    target = hips + body(key["grip"]) - want_std @ grip_offset(side)
    # Two-bone IK, every swing of the elbow about the shoulder-to-wrist line tried.
    S, E, W = P[up], P[fore], P[hand]
    a, b = np.linalg.norm(E - S), np.linalg.norm(W - E)
    to = target - S
    dist = min(max(np.linalg.norm(to), abs(a - b) + 1e-3), a + b - 1e-3)
    dirn = unit(to)
    pole0 = (E - S) - np.dot(E - S, dirn) * dirn
    pole0 = unit(pole0) if np.linalg.norm(pole0) > 1e-6 else unit(np.cross(dirn, [1.0, 0, 0]))
    side_way = np.cross(dirn, pole0)
    cos_a = (a * a + dist * dist - b * b) / (2 * a * dist)
    sin_a = math.sqrt(max(0.0, 1 - cos_a * cos_a))
    wrist = S + dirn * dist
    upper_rest = frame_of(ax["upper"], ax["hinge"])
    best = None
    for swing in range(-180, 180, 3):
        pole = math.cos(math.radians(swing)) * pole0 + math.sin(math.radians(swing)) * side_way
        elbow = S + a * (cos_a * dirn + sin_a * pole)
        u, f = unit(elbow - S), unit(wrist - elbow)
        # The upper arm rolled so its hinge is square to the arm's bend; a straight arm keeps the guard's.
        k = np.cross(u, f)
        k = unit(k) if np.linalg.norm(k) > 1e-4 else unit(G[up] @ ax["hinge"] - np.dot(G[up] @ ax["hinge"], u) * u)
        gu = frame_of(u, k) @ upper_rest.T
        # The forearm carries the hand's twist: the hand only swings off it.
        gf = want_std @ align(ax["forearm"], want_std.T @ f)
        flex, dev = (x - r for x, r in zip(bend(want_std, f, ax), REST_BEND[side]))
        twist = ax["sup"] * twist_of(gu.T @ gf, ax["forearm"])
        cost = over(flex, dev, twist) ** 2 * 100 + (flex / 10) ** 2 + (dev / 10) ** 2 + (swing / 30) ** 2
        if best is None or cost < best[0]:
            best = (cost, swing, gu, gf, flex, dev, twist)
    _, swing, gu, gf, flex, dev, twist = best
    G = G.copy()
    G[up], G[fore], G[hand] = gu, gf, want_std
    for j in (up, fore, hand):
        loc[j] = to_local(G, j)
    G, P = fk(loc, root)
    grip = P[hand] + G[hand] @ grip_offset(side)
    short = np.linalg.norm(hips + body(key["grip"]) - grip)
    return loc, {"grip": grip - hips, "blade": G[hand] @ REF[side][0], "short": short, "flex": flex, "dev": dev, "twist": twist,
                 "swing": swing, "past": over(flex, dev, twist), "elbow": unit(P[fore] - P[up])}


def solve(key, side):
    """A key's pose, its KEY line printed; refused past a person's wrist."""
    loc, r = reach(key, side)
    back_of = lambda v: [round(float(x), 2) for x in (-v[0], v[2], v[1])]  # noqa: E731
    past = r["past"]
    print(f"KEY {side.lower()} at {key['at']:.2f} s: grip {back_of(r['grip'])} m, blade {back_of(r['blade'])}, {r['short'] * 100:.1f} cm short; "
          f"wrist flexed {r['flex']:.0f}, deviated {r['dev']:.0f} (+ to the thumb), forearm twisted {r['twist']:.0f}, "
          f"elbow swung {r['swing']}{f'; ✗ {past:.0f} deg past a person' if past > 0 else ''}")
    if past > 0:
        turned = "blade and edge" if side == "Right" else "forearm's line and back"
        refused.append(f"{side.lower()} key at {key['at']:.2f} s: no arm turns the hand to its {turned} within a wrist (flexed {r['flex']:.0f}, "
                       f"deviated {r['dev']:.0f}, forearm twisted {r['twist']:.0f}; limits flexed {WRIST['flex']}, extended {WRIST['ext']}, "
                       f"radial {WRIST['radial']}, ulnar {WRIST['ulnar']} less {MARGIN}, twist {TWIST[0]} to {TWIST[1]})")
    return loc


def body_list(v):
    return [round(float(x), 4) for x in body(v)]


def reached(key, side):
    """A key's reach as numbers: where the fist and blade ended up (the body's frame), the wrist."""
    _, r = reach(key, side)
    return {"side": side, "at": key.get("at"), "grip": body_list(r["grip"]), "blade": body_list(r["blade"]), "elbow": body_list(r["elbow"]), "short": round(float(r["short"]), 4),
            **{k: round(float(r[k]), 2) for k in ("flex", "dev", "twist", "swing", "past")}}


def guard_hand(side):
    """The guard's own hand as a key would give it: its grip, blade and edge (the right) or back
    (the left); that key's reach; and the wrist on the guard's own bones, as reach measures it
    (its elbow is swing 0)."""
    up, fore, hand = (idx[f"{side}{b}"] for b in ("Arm", "ForeArm", "Hand"))
    ax = AXES[side]
    grip = P0[hand] + G0[hand] @ grip_offset(side) - hips
    blade, back = G0[hand] @ REF[side][0], G0[hand] @ REF[side][1]
    key = {"at": 0, "grip": body_list(grip), "blade": body_list(blade)}
    key.update({"edge": body_list(np.cross(blade, back))} if side == "Right" else {"back": body_list(back)})
    f = unit(P0[hand] - P0[fore])
    flex, dev = (x - r for x, r in zip(bend(G0[hand], f, ax), REST_BEND[side]))
    twist = ax["sup"] * twist_of(G0[up].T @ G0[hand] @ align(ax["forearm"], G0[hand].T @ f), ax["forearm"])
    bones = {"flex": flex, "dev": dev, "twist": twist, "swing": 0, "past": over(flex, dev, twist)}
    return {**key, "reach": reached(key, side), "bones": {**{k: round(float(v), 2) for k, v in bones.items()}, "elbow": body_list(unit(P0[fore] - P0[up]))}}


if REACH:
    print("REACH " + json.dumps({
        "bar": {**{k: v - MARGIN for k, v in WRIST.items()}, "twist": TWIST},
        "guard": {s: guard_hand(s) for s in ("Right", "Left")},
        "keys": [reached(k, k.get("side", "Right")) for k in spec.get("keys", [])],
    }))
    sys.exit(0)

last = round(float(spec["seconds"]) * FPS) - 1
aa = lambda m: R.from_matrix(m.reshape(-1, 3, 3)).as_rotvec().reshape(len(names), 3).tolist()  # noqa: E731
keys = [{
    "type": "fullbody",
    "frame_indices": [0, last],
    "local_joints_rot": [aa(guard), aa(guard)],
    "root_positions": [root.tolist(), root.tolist()],
}]
for side, field, kind in (("Right", "hand", "right-hand"), ("Left", "left", "left-hand")):
    poses = [(min(last - 1, max(1, round(k["at"] * FPS))), solve(k, side)) for k in spec.get(field, [])]
    if poses:
        keys.append({
            "type": kind,
            "frame_indices": [f for f, _ in poses],
            "local_joints_rot": [aa(p) for _, p in poses],
            "root_positions": [root.tolist() for _ in poses],
        })
if refused:
    for r in refused:
        print(f"✗ {r}: change the key's edge, blade or grip in the clip's spec (clip_spec.ts), not the limits")
    sys.exit(1)
json.dump(keys, open(out_path, "w"))
print(f"KEYS guard frame {frame} at 0 and {last}; hips {root[1]:.2f} m; {sum(len(k['frame_indices']) for k in keys[1:])} hand keys")
