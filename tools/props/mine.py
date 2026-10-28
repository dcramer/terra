"""The hewn mine's timber kit (D527, D559, docs/dungeons.md#the-hewn-mines-plan-d559-its-rooms-
d539-open-with-the-crypts): what the dungeon stamp stands in a mine's drifts, as a hard-rock
mine of the 1800s timbered them, built from the barrow kit's helpers (barrow.py) in Blender:

- `TimberSet`: a drift set, two round posts battered in from 2.6 m apart at the foot, a cap
  log across the drift's 3 m on them at 2.6 m, and lagging over them: boards laid along the
  drift on the caps, 2 m long so a set every 2 m roofs it whole, and boards behind the posts'
  heads against the rock, some missing so the rock shows. The drift 2.5 m clear between the
  posts and 2.6 m under the cap. Origin the middle of the drift's floor under the cap, the cap
  along X, the drift along glTF Z.

    tools/blender/run.sh tools/props/mine.py [Name ...]

Writes them (or only those named) to assets/models/props/mine/. The posts and cap are
`log_wood` (pine with patches of its bark), the boards `weathered_planks` (assets/materials,
README there), at their real sizes and cut to 1024 pixels. Nothing is random from run to run.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from mathutils import Vector

from barrow import ROOT, Mesh, build

OUT = os.path.join(ROOT, "assets", "models", "props", "mine")
# The textures' widths in metres (assets/materials/README.md).
LOG_M, PLANK_M = 1.0, 2.0
# The drift between its walls' faces, the set's posts and cap (D527: drifts 2.5-3 m).
HALF = 1.5
POST_R, CAP_R = 0.11, 0.13
FOOT_X, HEAD_X = 1.30, 1.24
CAP_Z = 2.6
# A set every 2 m: its lagging reaches 1 m each way, and a little past, so no seam shows sky.
REACH = 1.04
# The walls' lagging poles, round, butting the next set's a finger short of the half-way.
POLE_R, SIDE_REACH = 0.065, 0.99
# Old pine, weathered grey-brown and darkened by the mine's damp and lamp smoke.
WOOD = ("Timber", "log_wood", (0.62, 0.54, 0.44))
BOARD = ("Lagging", "weathered_planks", (0.55, 0.48, 0.40))


def jitter(k, salt):
    """A number in [0, 1) from an index, the same every run."""
    return (math.sin(k * 12.9898 + salt * 78.233) * 43758.5453) % 1.0


def log(m, a, b, r, mat, segs=9, turn=0.0):
    """A round timber from `a` to `b`, `r` across its middle, its ends capped: U round it, V along
    it, at the log texture's width; a little out of round, as a hewn pole is."""
    a, b = Vector(a), Vector(b)
    axis = (b - a).normalized()
    side = axis.cross(Vector((0.0, 0.0, 1.0)))
    if side.length < 1e-3:
        side = Vector((1.0, 0.0, 0.0))
    side.normalize()
    up = side.cross(axis)
    length = (b - a).length
    rings = max(2, int(length / 0.4) + 1)
    rows = []
    for i in range(rings):
        s = i / (rings - 1)
        c = a + (b - a) * s
        row = []
        for k in range(segs + 1):
            t = 2 * math.pi * k / segs + turn
            rr = r * (1.0 + 0.06 * math.sin(3 * t + i))
            row.append((c + (side * math.cos(t) + up * math.sin(t)) * rr, (t * r / LOG_M, s * length / LOG_M)))
        rows.append(row)

    def on_axis(p):
        return a + axis * (p - a).dot(axis)

    m.grid(rows, mat, away=on_axis)
    mid = (a + b) / 2
    for row in (rows[0], rows[-1]):
        ring = row[:-1]
        m.face([(m.bm.verts.new(p.copy()), ((p - mid).dot(side) / LOG_M, (p - mid).dot(up) / LOG_M)) for p, _ in ring], mat, away=lambda _: mid)


def board(m, lo, hi, mat, k):
    """A lagging board, its length along Y, its texture's planks taken at a place of its own."""
    lo, hi = Vector(lo), Vector(hi)
    centre = (lo + hi) / 2
    du, dv = jitter(k, 1), jitter(k, 2)
    for axis in range(3):
        for end in (lo, hi):
            a, b = [i for i in range(3) if i != axis]
            corners = []
            for sa, sb in ((0, 0), (1, 0), (1, 1), (0, 1)):
                p = Vector(lo)
                p[axis] = end[axis]
                p[a] = hi[a] if sa else lo[a]
                p[b] = hi[b] if sb else lo[b]
                # Along the board is the plank's length (V), across it its width (U).
                along, across = p.y, (p.x if axis != 0 else p.z)
                corners.append((m.bm.verts.new(p), (across / PLANK_M + du, along / PLANK_M + dv)))
            m.face(corners, mat, away=lambda _: centre)


def timber_set():
    m = Mesh()
    # The posts, battered in: their feet out, their heads in under the cap.
    for sx in (-1, 1):
        log(m, (sx * FOOT_X, 0.0, -0.02), (sx * HEAD_X, 0.0, CAP_Z + 0.04), POST_R, 0, turn=0.3 * sx)
    # The cap across the drift, its ends in hitches cut in the rock.
    log(m, (-HALF + 0.02, 0.0, CAP_Z + CAP_R), (HALF - 0.02, 0.0, CAP_Z + CAP_R), CAP_R, 0, segs=10)
    # The roof's lagging on the caps, along the drift, a finger's gap between boards.
    top = CAP_Z + 2 * CAP_R - 0.01
    x, k = -HALF + 0.04, 0
    while x < HALF - 0.12:
        w = 0.18 + 0.06 * jitter(k, 3)
        w = min(w, HALF - 0.04 - x)
        sag = 0.02 * jitter(k, 4)
        reach = REACH + 0.06 * (jitter(k, 5) - 0.5)
        board(m, (x, -reach, top - sag), (x + w, reach, top - sag + 0.04), 1, k)
        x += w + 0.015 + 0.02 * jitter(k, 6)
        k += 1
    # The walls' lagging: poles laid along the drift behind the posts, between them and the
    # rock, from a knee up to the cap, each bearing on the posts' backs as they batter in, so
    # the rock's bulges take them in places and it shows through the gaps of the few missing
    # (round 1's judges: boards on edge in front of the rock read as stuck flat on the wall).
    # They butt a finger short of the next set's, which lays the same poles at the same
    # heights, so no two overlap and flicker.
    for sx in (-1, 1):
        z = 0.35
        while z < CAP_Z - 0.06:
            r = POLE_R + 0.015 * (jitter(k, 7) - 0.5)
            if jitter(k, 8) > 0.18:
                x = sx * (post_x(z + r) + POST_R + r - 0.01)
                log(m, (x, -SIDE_REACH, z + r), (x, SIDE_REACH, z + r), r, 0, segs=7, turn=jitter(k, 9))
            z += 2 * r + 0.012
            k += 1
    return m, [WOOD, BOARD]


def post_x(z):
    """How far out from the drift's middle a post's axis is at a height: battered in from its
    foot to its head."""
    return FOOT_X + (HEAD_X - FOOT_X) * z / CAP_Z


if __name__ == "__main__":
    build((("TimberSet", timber_set),), OUT)
