"""The base body's arms and legs stretched to a person's proportions, the whole body then scaled
back to its height (D593; why and the measures in research/2026-11-01-why-the-torso-looks-wrong.md).
Every generated body comes with short arms (a 1.68 m reach on a 1.80 m man, below ANSUR II's 1st
percentile) and the crotch low, and every design sheet draws them so.

The stretch is a warp of space, not of one mesh: worked out once on a rigged body's skin and
joints (`plan`, in convert_rig.py), it moves any point (`apply`), so the skin, the skeleton's
joints, a design of the same person (morph.py) and a dressed model of them (skin_piece.line_up)
all move alike and still fit one another, as a cage deformer moves a body and its clothes. Pure
numpy, in the body's T-pose frame (Blender's axes, Z up, facing -Y), floor at its lowest point:

  1. Legs: each from its hip joint along its own line to the ankle, LEGS longer; the stretch
     eases in from nothing at the hip joint to its full rate at the crotch's depth, as the
     pelvis's weight gives way to the thigh's, so the groin and buttocks stay whole; past the
     ankle the foot moves rigidly. The two legs' stretches blend across the middle (SIDE).
  2. Back on the floor, and the whole body scaled about the floor to its height again.
  3. Arms: in the T-pose they lie along X above the shoulders' band, and from the shoulder joint
     out to the wrist are stretched along X, the hands moved rigidly, to REACH fingertip to
     fingertip at the final height.

The amounts are D593's, on the developer's word on a test copy ("He looks fine").
"""

import numpy as np

import theirs

REACH = 1.86 / 1.80  # fingertip to fingertip per height: ANSUR II's men's mean
LEGS = 1.10  # hip joint to ankle, before the whole body is scaled back to its height
BAND = 1.25 / 1.80  # the T-pose's arms: what lies above this share of the height
SIDE = 0.03  # m either side of the middle over which the two legs' stretches blend


def _smooth(u):
    u = np.clip(u, 0.0, 1.0)
    return u * u * (3 - 2 * u)


def plan(points, edges, joints, height):
    """The stretch for a body in its T-pose: `points` its skin (n x 3) and `edges` (k x 2 vertex
    indices), `joints` name -> (x, y, z) of our skeleton's joints, `height` what it is to stand
    when done. A dict of plain numbers (JSON). Never on Quaternius's parts (D599)."""
    theirs.refuse("stretch.py")
    P = np.asarray(points, dtype=float)
    J = {k: np.asarray(v, dtype=float) for k, v in joints.items()}
    floor = float(P[:, 2].min())
    tall = float(P[:, 2].max()) - floor
    cx = float(J["upperarm_l"][0] + J["upperarm_r"][0]) / 2
    cy = float(J["thigh_l"][1] + J["thigh_r"][1]) / 2
    crotch = measure_crotch(P, cx, edges)
    legs = {}
    for s in "lr":
        hip, ankle = J[f"thigh_{s}"], J[f"foot_{s}"]
        d = ankle - hip
        L = float(np.linalg.norm(d))
        a = d / L
        tc = float(np.clip((hip[2] - (floor + crotch)) / max(-a[2], 1e-6), 0.02, L / 2))
        legs[s] = {"hip": hip.tolist(), "dir": a.tolist(), "length": L, "ease": tc}
    sign_l = 1.0 if J["thigh_l"][0] > cx else -1.0
    shoulder = (abs(J["upperarm_l"][0] - cx) + abs(J["upperarm_r"][0] - cx)) / 2
    wrist = (abs(J["hand_l"][0] - cx) + abs(J["hand_r"][0] - cx)) / 2
    middle = [float(P[:, a].max() + P[:, a].min()) / 2 for a in (0, 1)]
    r = {"floor": floor, "height_was": tall, "height": float(height), "centre": [cx, cy], "middle": middle, "left": sign_l,
         "legs": legs, "stretch": LEGS, "band": floor + BAND * tall, "shoulder": shoulder, "wrist": wrist,
         "lift": 0.0, "scale": 1.0, "arm": 1.0}
    after = _legs(r, P)
    r["lift"] = floor - float(after[:, 2].min())
    r["scale"] = float(height) / (float(after[:, 2].max()) + r["lift"] - floor)
    span = float(P[:, 0].max() - P[:, 0].min())
    r["arm"] = 1.0 + (REACH * float(height) / r["scale"] - span) / (2 * (wrist - shoulder))
    return r


def measure_crotch(P, cx, edges):
    """The crotch's height above the floor: the lowest skin on the body's middle plane (x = cx)
    between a quarter and two thirds of its height, where the mesh's edges (k x 2 vertex
    indices) cross that plane. Vertices near the plane would do as well but for the inner
    thighs, which come within a centimetre of it under the crotch."""
    P = np.asarray(P, dtype=float)
    E = np.asarray(edges, dtype=int)
    floor = P[:, 2].min()
    tall = P[:, 2].max() - floor
    a, b = P[E[:, 0]], P[E[:, 1]]
    da, db = a[:, 0] - cx, b[:, 0] - cx
    cross = (da * db < 0) | (da == 0)
    t = np.where(cross, da / np.where(da - db == 0, 1, da - db), 0)[:, None]
    z = (a + (b - a) * t)[cross, 2]
    z = z[(z > floor + 0.25 * tall) & (z < floor + 0.65 * tall)]
    return float(z.min() - floor)


def _legs(r, P):
    """The legs' stretch alone: each point moved along each leg's line by how far down it lies,
    the two legs blended across the middle."""
    cx = r["centre"][0]
    u = ((P[:, 0] - cx) * r["left"] / SIDE + 1) / 2
    share = {"l": _smooth(u), "r": 1.0 - _smooth(u)}
    out = P.copy()
    for s, leg in r["legs"].items():
        hip, a, L, tc = np.array(leg["hip"]), np.array(leg["dir"]), leg["length"], leg["ease"]
        t = np.clip((P - hip) @ a, 0.0, L)
        v = t / tc
        eased = np.where(t < tc, tc * (v**3 - v**4 / 2), t - tc / 2)
        rate = (r["stretch"] - 1.0) * L / (L - tc / 2)
        out += (share[s] * rate * eased)[:, None] * a
    return out


def apply(r, points):
    """Where the stretch `r` (from `plan`) takes each point (n x 3, or one point)."""
    theirs.refuse("stretch.py")
    P = np.asarray(points, dtype=float)
    one = P.ndim == 1
    P = np.atleast_2d(P)
    high = P[:, 2] > r["band"]
    out = _legs(r, P)
    out[:, 2] += r["lift"]
    cx, cy = r["centre"]
    x = out[:, 0] - cx
    ax = np.abs(x)
    xs, xw, K = r["shoulder"], r["wrist"], r["arm"]
    grown = np.where(ax < xw, xs + (ax - xs) * K, ax + (xw - xs) * (K - 1))
    out[:, 0] = np.where(high & (ax > xs), cx + np.sign(x) * grown, out[:, 0])
    out[:, 0] = cx + (out[:, 0] - cx) * r["scale"]
    out[:, 1] = cy + (out[:, 1] - cy) * r["scale"]
    out[:, 2] = r["floor"] + (out[:, 2] - r["floor"]) * r["scale"]
    return out[0] if one else out


def line_up(r, points):
    """An unrigged model of the same person in the T-pose (a design, a dressed model), already
    turned to face -Y and scaled to the body's height: its feet put on the floor, moved to stand
    where the body stood before its stretch (by its bounds' middle, which the T-pose's
    fingertips set across), then stretched as the body was. Returns the points."""
    P = np.asarray(points, dtype=float).copy()
    P[:, 2] += r["floor"] - P[:, 2].min()
    for axis in (0, 1):
        P[:, axis] += r["middle"][axis] - (P[:, axis].max() + P[:, axis].min()) / 2
    return apply(r, P)
