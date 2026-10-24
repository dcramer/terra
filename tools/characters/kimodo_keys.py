"""The keys a combat clip's Kimodo takes are held to (D450): the set's guard pose as a full-body
keyframe at the clip's first and last frames, and the blade's path as right-hand (or, for a
shield move, left-hand) keys at the times the spec gives, each a pose solved by two-bone IK on
Kimodo's skeleton from the guard. Run in Kimodo's own Python (Kimodo speaks nothing else), by
tools/characters/clip.ts:

    $KIMODO_HOME/venv/bin/python -I tools/characters/kimodo_keys.py GUARD.npz KEYS.json OUT.json

GUARD.npz is the guard take as a Kimodo NPZ (kimodo_convert of its BVH). KEYS.json:
{"frame": the guard take's frame that is the pose, "seconds": the clip's length, "hand": [key],
"left": [key]}. A key: {"at": seconds, "grip": [right, ahead, up], "blade": [right, ahead, up],
"back": [right, ahead, up], "yaw": degrees (+ to the left)}, metres and directions in the
body's frame from the hips at the guard pose. The grip is the middle of the fist (the hand's
socket); for the right hand the blade is the sword's line out of the fist and the back the
back of the hand; for the left the blade is the forearm's line toward the fist and the back
the shield's face. `back` defaults to the guard's own hand, turned as little as can be;
`yaw` turns the chest about the spine before the arm reaches.

The guard pose is turned to face +Z and set on the origin, as Kimodo's canonical start is.
Prints a KEY line a key: where its fist and blade ended up, and how far short of the grip it
fell (a grip beyond the arm's reach is met at full stretch).
"""

import json
import math
import sys

import numpy as np
import torch
from scipy.spatial.transform import Rotation as R

from kimodo.skeleton import SOMASkeleton77

FPS = 30
guard_path, keys_path, out_path = sys.argv[1:4]
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


def solve(key, side):
    """The guard pose with one arm reaching for a key."""
    loc = guard.copy()
    if key.get("yaw"):
        G, _ = fk(loc, root)
        spin = R.from_euler("y", math.radians(key["yaw"])).as_matrix()
        j = idx["Spine1"]
        loc[j] = G[parents[j]].T @ spin @ G[j]
    G, P = fk(loc, root)
    up, fore, hand = idx[f"{side}Arm"], idx[f"{side}ForeArm"], idx[f"{side}Hand"]
    # The hand's turn from the T-pose: the blade (+Z at the T-pose, the right fist's sword
    # pointing ahead), the back of the hand (+Y, up).
    have = G[hand]
    blade = unit(body(key["blade"])) if "blade" in key else have @ T_AHEAD
    back = body(key["back"]) if "back" in key else have @ T_UP
    back = unit(back - np.dot(back, blade) * blade)
    # The turn taking the T-pose's (ahead, up) onto (blade, back).
    rest = np.stack([T_AHEAD, T_UP, np.cross(T_AHEAD, T_UP)], axis=1)
    want_std = np.stack([blade, back, np.cross(blade, back)], axis=1) @ rest.T
    target = hips + body(key["grip"]) - want_std @ grip_offset(side)
    # Two-bone IK, the elbow kept in the plane it bends in at the guard.
    S, E, W = P[up], P[fore], P[hand]
    a, b = np.linalg.norm(E - S), np.linalg.norm(W - E)
    to = target - S
    dist = min(max(np.linalg.norm(to), abs(a - b) + 1e-3), a + b - 1e-3)
    dirn = unit(to)
    pole = (E - S) - np.dot(E - S, dirn) * dirn
    if np.linalg.norm(pole) < 1e-6:
        pole = np.array([0, -1.0, 0])
    pole = unit(pole)
    cos_a = (a * a + dist * dist - b * b) / (2 * a * dist)
    elbow = S + a * (cos_a * dirn + math.sqrt(max(0.0, 1 - cos_a * cos_a)) * pole)
    wrist = S + dirn * dist
    d1 = align(E - S, elbow - S)
    d2 = align(d1 @ (W - E), wrist - elbow)
    G = G.copy()
    G[up] = d1 @ G[up]
    G[fore] = d2 @ d1 @ G[fore]
    G[hand] = want_std
    for j in (up, fore, hand):
        loc[j] = to_local(G, j)
    G, P = fk(loc, root)
    got_std = G[hand]
    grip = P[hand] + got_std @ grip_offset(side)
    short = np.linalg.norm(hips + body(key["grip"]) - grip)
    back_of = lambda v: [round(float(x), 2) for x in (-v[0], v[2], v[1])]  # noqa: E731
    print(f"KEY {side.lower()} at {key['at']:.2f} s: grip {back_of(grip - hips)} m, blade {back_of(got_std @ T_AHEAD)}, "
          f"{short * 100:.1f} cm short")
    return loc


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
json.dump(keys, open(out_path, "w"))
print(f"KEYS guard frame {frame} at 0 and {last}; hips {root[1]:.2f} m; {sum(len(k['frame_indices']) for k in keys[1:])} hand keys")
