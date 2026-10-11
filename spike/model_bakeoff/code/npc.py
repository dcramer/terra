"""A villager, grown from a stick skeleton in an A pose, dressed by painting faces."""

import os
import sys

sys.path[:0] = [os.path.dirname(os.path.abspath(__file__)), os.path.dirname(os.path.dirname(os.path.abspath(__file__)))]

import look
from parts import blob, box, paint, skinned

look.reset()
J = {}
pts, radii, edges = [], [], []


def joint(name, at, r, parent=None):
    J[name] = len(pts)
    pts.append(at)
    radii.append(r if isinstance(r, tuple) else (r, r))
    if parent is not None:
        edges.append((J[parent], J[name]))


# x forward, y to the figure's left, z up; about 1.75 m tall.
joint("pelvis", (0, 0, 0.96), (0.13, 0.17))
joint("belly", (0, 0, 1.16), (0.12, 0.155), "pelvis")
joint("chest", (0, 0, 1.36), (0.135, 0.2), "belly")
joint("neck", (0, 0, 1.5), 0.055, "chest")
for side, y in (("l", 1), ("r", -1)):
    joint(f"hip{side}", (0, y * 0.095, 0.9), 0.085, "pelvis")
    joint(f"knee{side}", (0.02, y * 0.1, 0.5), 0.06, f"hip{side}")
    joint(f"ankle{side}", (-0.01, y * 0.1, 0.09), 0.05, f"knee{side}")
    joint(f"toe{side}", (0.14, y * 0.1, 0.04), 0.045, f"ankle{side}")
    joint(f"shoulder{side}", (0, y * 0.22, 1.42), 0.07, "chest")
    joint(f"elbow{side}", (-0.02, y * 0.27, 1.16), 0.045, f"shoulder{side}")
    joint(f"wrist{side}", (0.02, y * 0.31, 0.92), 0.035, f"elbow{side}")
    joint(f"hand{side}", (0.03, y * 0.32, 0.82), 0.04, f"wrist{side}")

body = skinned("Body", pts, edges, radii, "ClothRed", smooth=1, decimate=0.5)


def clothes(c, n):
    if c.z < 0.3:
        return "Leather"  # boots
    if c.z < 0.9:
        return "Cloth"  # trousers
    if 0.9 <= c.z < 0.98 and abs(c.y) < 0.2:
        return "Leather"  # belt
    if abs(c.y) > 0.24 and c.z < 1.12:
        return "Skin"  # forearms and hands
    if c.z > 1.47:
        return "Skin"  # neck
    return None  # tunic


paint(body, clothes)
head = blob("Head", 0.1, "Skin", at=(0.01, 0, 1.63), squash=(1.0, 0.85, 1.15), seed=3, detail=2, rough=0.03)
hair = blob("Hair", 0.105, "Hair", at=(-0.025, 0, 1.68), squash=(1.0, 0.92, 0.9), seed=5, detail=1, rough=0.06)
nose = box("Nose", (0.03, 0.025, 0.04), "Skin", at=(0.1, 0, 1.61), rot=(0, 15, 0))
pouch = box("Pouch", (0.1, 0.06, 0.1), "Leather", at=(0.06, -0.15, 0.9), bevel=0.01)
npc = look.join([body, head, hair, nose, pouch], "Villager")
look.finish([npc], "code-npc", budget=3000, with_figure=True)
