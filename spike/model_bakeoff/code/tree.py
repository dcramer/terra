"""A broadleaf tree as a generator: a leaning trunk that forks, crowned with faceted clumps."""

import math
import os
import random
import sys

sys.path[:0] = [os.path.dirname(os.path.abspath(__file__)), os.path.dirname(os.path.dirname(os.path.abspath(__file__)))]

from mathutils import Vector

import look
from parts import blob, skinned


def tree(name, seed, height=5.5):
    rng = random.Random(seed)
    # The trunk and branches are one skeleton, skinned so the forks join smoothly.
    pts = [Vector((0, 0, 0)), Vector((0, 0, height * 0.18))]
    radii = [(0.26, 0.26), (0.19, 0.19)]
    edges = [(0, 1)]
    lean = Vector((rng.uniform(-0.3, 0.3), rng.uniform(-0.3, 0.3), 1)).normalized()
    pts.append(pts[1] + lean * height * 0.25)
    radii.append((0.15, 0.15))
    edges.append((1, 2))
    tips = []
    for b in range(3):
        a = b / 3 * math.tau + rng.uniform(-0.5, 0.5)
        d = Vector((math.cos(a) * 0.55, math.sin(a) * 0.55, 1)).normalized()
        mid = pts[2] + d * height * 0.22
        tip = mid + Vector((d.x * 0.6, d.y * 0.6, 0.9)).normalized() * height * 0.2
        pts += [mid, tip]
        radii += [(0.09, 0.09), (0.05, 0.05)]
        edges += [(2, len(pts) - 2), (len(pts) - 2, len(pts) - 1)]
        tips.append(tip)
    trunk = skinned(f"{name}Trunk", pts, edges, radii, "Bark")
    # Roots: the base flares a little.
    crown = []
    for i, tip in enumerate(tips + [pts[2] + lean * height * 0.45]):
        for k in range(3):
            off = Vector((rng.uniform(-0.6, 0.6), rng.uniform(-0.6, 0.6), rng.uniform(-0.1, 0.6)))
            r = rng.uniform(0.75, 1.1) * height / 5.5
            mat = rng.choice(["Leaves", "Leaves", "LeavesDark", "LeavesLight"])
            crown.append(blob(f"{name}Clump{i}{k}", r, mat, at=tip + off, squash=(1, 1, 0.75), seed=seed * 31 + i * 7 + k))
    return look.join([trunk] + crown, name)


look.reset()
trees = [tree("Oak", 4), tree("OakYoung", 9, height=3.6)]
look.finish(trees, "code-tree", budget=2500)
