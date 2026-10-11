"""A wolf, grown from a stick skeleton: body, legs and tail skinned as one, faceted."""

import os
import sys

sys.path[:0] = [os.path.dirname(os.path.abspath(__file__)), os.path.dirname(os.path.dirname(os.path.abspath(__file__)))]

import look
from parts import box, paint, prism, skinned

look.reset()
J = {}
pts, radii, edges = [], [], []


def joint(name, at, r, parent=None):
    J[name] = len(pts)
    pts.append(at)
    radii.append(r if isinstance(r, tuple) else (r, r))
    if parent is not None:
        edges.append((J[parent], J[name]))


# Spine, x forward, z up, metres.
joint("hip", (0.0, 0, 0.62), (0.16, 0.15))
joint("waist", (0.28, 0, 0.64), (0.15, 0.14), "hip")
joint("chest", (0.55, 0, 0.64), (0.25, 0.19), "waist")
joint("neck", (0.76, 0, 0.8), (0.18, 0.17), "chest")
joint("head", (0.93, 0, 0.86), (0.13, 0.125), "neck")
joint("snout", (1.11, 0, 0.8), (0.075, 0.065), "head")
joint("nose", (1.23, 0, 0.78), (0.04, 0.035), "snout")
# Legs: front from the chest, back from the hip, with the dog's backward hock.
for side, y in (("l", 0.11), ("r", -0.11)):
    joint(f"shoulder{side}", (0.58, y, 0.56), 0.1, "chest")
    joint(f"elbow{side}", (0.6, y * 1.1, 0.33), 0.07, f"shoulder{side}")
    joint(f"wrist{side}", (0.6, y * 1.1, 0.1), 0.05, f"elbow{side}")
    joint(f"paw{side}", (0.68, y * 1.1, 0.035), 0.05, f"wrist{side}")
    joint(f"thigh{side}", (0.04, y, 0.52), 0.13, "hip")
    joint(f"knee{side}", (0.12, y * 1.15, 0.34), 0.08, f"thigh{side}")
    joint(f"hock{side}", (-0.05, y * 1.15, 0.17), 0.05, f"knee{side}")
    joint(f"hpaw{side}", (-0.02, y * 1.15, 0.035), 0.05, f"hock{side}")
    joint(f"htoe{side}", (0.07, y * 1.15, 0.03), 0.045, f"hpaw{side}")
# A full tail, carried low.
joint("tail0", (-0.12, 0, 0.66), 0.05, "hip")
joint("tail1", (-0.33, 0, 0.54), 0.11, "tail0")
joint("tail2", (-0.53, 0, 0.36), 0.08, "tail1")
joint("tail3", (-0.62, 0, 0.26), 0.025, "tail2")

body = skinned("WolfBody", pts, edges, radii, "Fur", smooth=1, decimate=0.45)


def coat(c, n):
    if c.x > 1.19:
        return "Black"
    if n.z < -0.35 and c.z < 0.78 or (c.x > 1.0 and n.z < -0.2):
        return "FurLight"
    if c.z < 0.12:
        return "FurDark"
    if n.z > 0.55 and c.z > 0.66 or c.x < -0.5:
        return "FurDark"
    return None


paint(body, coat)
parts = [body]
for y in (0.06, -0.06):
    parts.append(prism("Ear", 0.05, 0.11, "FurDark", sides=4, at=(0.89, y * 1.2, 0.95), rot=(y * 200, -12, 45), top=0.004))
    parts.append(box("Eye", (0.022, 0.015, 0.018), "Black", at=(1.02, y * 1.35, 0.9)))
wolf = look.join(parts, "Wolf")
look.finish([wolf], "code-wolf", budget=3000)
