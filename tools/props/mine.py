"""The hewn mine's timber kit (D527, D559, docs/dungeons.md#the-hewn-mines-plan-d559-its-rooms-
d539-open-with-the-crypts): what the dungeon stamp stands in a mine's drifts, as a hard-rock
mine of the 1800s timbered them, built from the barrow kit's helpers (barrow.py) in Blender:

- `TimberSet`: a drift set, two round posts battered in from 2.6 m apart at the foot, a cap
  log across the drift's 3 m on them at 2.6 m, and lagging over them: boards laid along the
  drift on the caps, 2 m long so a set every 2 m roofs it whole, and boards behind the posts'
  heads against the rock, some missing so the rock shows. The drift 2.5 m clear between the
  posts and 2.6 m under the cap. Origin the middle of the drift's floor under the cap, the cap
  along X, the drift along glTF Z.
- `MineDoor`: a leaf of a mine's doorway, ledged and braced boards with iron straps, 1.5 m
  wide (half the doorway) and 3 m high, so it hangs unstretched; origin the hinge's foot,
  reaching +X.

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


# The mine's door leaf, one of a doorway's two: half of its 3 m and its full 3 m high, so the
# game hangs it unstretched (the theme's `door_leaf`).
LEAF_W, LEAF_H = 1.5, 3.0
# weathered_planks' planks stand along V, their seams at these U (assets/materials/README.md):
# a board takes one plank, a little in from its seam, so no seam crosses a board.
SEAM_U0, SEAM_DU = 0.062, 0.1125
# Seven boards with a finger's gap, a board's half thickness, the ledges' and braces' face.
BOARDS, GAP, HALF_T, FACE_T = 7, 0.006, 0.018, 0.045
LEDGES_Z, LEDGE_H = (0.32, 1.50, 2.68), 0.20
LEDGE_X = (0.03, 1.47)
# The braces rise from the hinge's foot to the latch's head, between the ledges.
BRACE_X, BRACE_W = (0.17, 1.33), 0.17
STRAP_REACH, STRAP_H, STRAP_T = 0.75, 0.05, 0.006
# The boards' wood tinted as the drifts' lagging; the straps' iron, dark and matte.
DOOR_WOOD = ("Planks", "weathered_planks", (0.60, 0.53, 0.45))
IRON = ("Iron", None, (0.08, 0.075, 0.07))


def slab(m, outline, y0, y1, mat, grain, seam, v0=0.0):
    """A piece of wood `outline` (four (x, z) corners round it) in the leaf's plane, from y0
    to y1 thick: its faces' grain along `grain` (a unit (x, z)) on one plank of the planks
    texture, the plank at seam `seam`, so its width falls inside that plank."""
    gx, gz = grain
    qx, qz = -gz, gx

    def along(x, z):
        return x * gx + z * gz

    def across(x, z):
        return x * qx + z * qz

    q0 = min(across(x, z) for x, z in outline)
    u0 = SEAM_U0 + SEAM_DU * seam + 0.004 - q0 / PLANK_M
    pts = [Vector((x, 0.0, z)) for x, z in outline]
    centre = sum(pts, Vector()) / 4 + Vector((0.0, (y0 + y1) / 2, 0.0))

    def uv(p, edge=None):
        a, b = across(p.x, p.z), along(p.x, p.z)
        if edge is not None:
            # A side: its long way the grain or across it, its short way the thickness.
            ex, ez = edge
            if abs(ex * qx + ez * qz) > abs(ex * gx + ez * gz):
                b = p.y - y0 + along(*outline[0])
            else:
                a = p.y - y0 + q0
        return (u0 + a / PLANK_M, v0 + b / PLANK_M)

    for y in (y0, y1):
        corners = [Vector((x, y, z)) for x, z in outline]
        m.face([(m.bm.verts.new(p), uv(p)) for p in corners], mat, away=lambda _: centre)
    for i in range(4):
        (ax, az), (bx, bz) = outline[i], outline[(i + 1) % 4]
        ln = math.hypot(bx - ax, bz - az)
        edge = ((bx - ax) / ln, (bz - az) / ln)
        corners = [Vector((ax, y0, az)), Vector((bx, y0, bz)), Vector((bx, y1, bz)), Vector((ax, y1, az))]
        m.face([(m.bm.verts.new(p), uv(p, edge)) for p in corners], mat, away=lambda _: centre)


def rect(x0, x1, z0, z1):
    return [(x0, z0), (x1, z0), (x1, z1), (x0, z1)]


def mine_door():
    """A ledged and braced door of seven boards, the leaf of a mine's doorway: hinged at its
    origin (the hinge's foot), reaching +X its 1.5 m, 3 m up, its boards' faces 18 mm each way
    of the plane the game hangs it in. Ledges and braces on both faces, as the doorway's second
    leaf is this one turned half round; iron straps over the top and bottom ledges from the
    hinge side, as a strap hinge's. As the timber of a drift's sets: real sizes, one plank of
    the texture a board, nothing random from run to run."""
    m = Mesh()
    w = (LEAF_W - (BOARDS - 1) * GAP) / BOARDS
    for k in range(BOARDS):
        x = k * (w + GAP)
        slab(m, rect(x, x + w, 0.0, LEAF_H), -HALF_T, HALF_T, 0, (0.0, 1.0), (3 * k) % 8, jitter(k, 11))
    for face in (1, -1):
        y0, y1 = sorted((face * HALF_T, face * FACE_T))
        for i, zc in enumerate(LEDGES_Z):
            z0, z1 = zc - LEDGE_H / 2, zc + LEDGE_H / 2
            slab(m, rect(LEDGE_X[0], LEDGE_X[1], z0, z1), y0, y1, 0, (1.0, 0.0), (2 * i + (face > 0)) % 8, jitter(i, 12 + face))
        for i in range(2):
            z0 = LEDGES_Z[i] + LEDGE_H / 2
            z1 = LEDGES_Z[i + 1] - LEDGE_H / 2
            dx, dz = BRACE_X[1] - BRACE_X[0], z1 - z0
            ln = math.hypot(dx, dz)
            # Cut level at the ledges: its width across the grain, BRACE_W, is wider level.
            half = BRACE_W / 2 * ln / dz
            outline = [(BRACE_X[0] - half, z0), (BRACE_X[0] + half, z0), (BRACE_X[1] + half, z1), (BRACE_X[1] - half, z1)]
            slab(m, outline, y0, y1, 0, (dx / ln, dz / ln), (5 + 3 * i + (face > 0)) % 8, jitter(i, 14 + face))
        # The straps over the top and bottom ledges.
        s0, s1 = sorted((face * FACE_T, face * (FACE_T + STRAP_T)))
        for zc in (LEDGES_Z[0], LEDGES_Z[2]):
            m.box((0.0, s0, zc - STRAP_H / 2), (STRAP_REACH, s1, zc + STRAP_H / 2), 1, 1.0)
    return m, [DOOR_WOOD, IRON]


if __name__ == "__main__":
    build((("TimberSet", timber_set), ("MineDoor", mine_door)), OUT)
