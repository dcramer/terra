"""The role pieces spike's stone pieces, authored in Blender (spike/role-pieces/README.md).

Run headless: tools/blender/run.sh spike/role-pieces/blender/pieces.py <out folder>

Writes, into the folder:

- profiles.json: the 2D sections swept along role edges (candidate 3): coping, plinth,
  sill, head and fascia, each drawn here as a polyline with its corners rounded by
  Blender's vertex bevel, in (out, up) metres from the edge, overhanging an eighth of a
  cube (D192 as amended) and, for reference, flush.
- pieces.glb: the corner stones fitted to those profiles (candidate 3): quoins for outside
  corners, long and short, and jamb stones for openings, each with bevelled arrises and a
  pitched, pillowed face, in three variants.
- tiles.glb: candidate 4's modules: a cube face of coursed stone for every combination of
  what its four edges are (a coping or a sill over it, a plinth or a lintel under it, an
  outside or inside corner either side), modelled to the same profiles, so any two meet.

Everything is in glTF's frame once exported: Y up. A piece's frame is given where it is
made. Sizes in metres; a cube is half a metre.
"""

import bmesh
import bpy
import json
import math
import os
import random
import sys
import zlib

from mathutils import Vector, noise

CUBE = 0.5
OVER = CUBE / 8  # how far trims may stand out (D192)
COURSE = 0.25
JOINT = 0.006  # half the gap between two stones

args = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
OUT = args[0] if args else os.path.join(os.path.dirname(__file__), "..", "out")
os.makedirs(OUT, exist_ok=True)


def clear():
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete()
    for mesh in list(bpy.data.meshes):
        bpy.data.meshes.remove(mesh)


# Profiles -------------------------------------------------------------------------------


def profile(points, rounded, radius=0.008):
    """A polyline in (out, up), its corners at the indices in `rounded` rounded by
    Blender's vertex bevel; returned as points in order."""
    bm = bmesh.new()
    verts = [bm.verts.new((o, u, 0.0)) for o, u in points]
    for a, b in zip(verts, verts[1:]):
        bm.edges.new((a, b))
    bm.verts.ensure_lookup_table()
    bmesh.ops.bevel(
        bm,
        geom=[verts[i] for i in rounded],
        offset=radius,
        segments=3,
        affect="VERTICES",
        profile=0.5,
    )
    # Walk the chain from the first point's end.
    ends = [v for v in bm.verts if len(v.link_edges) == 1]
    start = min(ends, key=lambda v: (v.co - Vector((points[0][0], points[0][1], 0))).length)
    out, prev, here = [], None, start
    while here is not None:
        out.append([round(here.co.x, 5), round(here.co.y, 5)])
        nxt = None
        for e in here.link_edges:
            other = e.other_vert(here)
            if other is not prev:
                nxt = other
        prev, here = here, nxt
    bm.free()
    return out


def profiles():
    o = OVER
    found = {}
    for name, over in [("over", o), ("flush", 0.0)]:
        s = over - o  # flush: everything an eighth back, inside the cube
        # A coping over a wall's top: a crowned top weathered to shed water, a rounded
        # nose standing out, a drip groove under it, then back to the wall face.
        coping = [
            (-0.25, -0.02),
            (-0.25, 0.04),
            (-0.12, 0.05),
            (o + s - 0.012, 0.035),
            (o + s, 0.02),
            (o + s, -0.07),
            (o + s - 0.008, -0.085),
            (0.03 + s, -0.085),
            (0.025 + s, -0.095),
            (0.012 + s, -0.095),
            (0.006 + s, -0.1),
            (-0.03, -0.1),
        ]
        # A plinth along a wall's foot: a plain face standing out, its top weathered back
        # to the wall in a chamfer.
        plinth = [
            (-0.03, 0.27),
            (s, 0.27),
            (s + 0.01, 0.262),
            (o + s, 0.21),
            (o + s, -0.06),
            (-0.03, -0.06),
        ]
        # A window's sill: sloped to throw the water off, a nose and a drip.
        sill = [
            (-0.25, -0.04),
            (-0.25, 0.012),
            (-0.04, 0.01),
            (o + s - 0.01, -0.012),
            (o + s, -0.022),
            (o + s, -0.072),
            (0.028 + s, -0.08),
            (0.022 + s, -0.09),
            (s, -0.09),
            (-0.03, -0.09),
        ]
        # An opening's head, its lintel: a deep face, chamfered at its top, its soffit
        # across the opening.
        head = [
            (-0.25, 0.0),
            (o * 0.5 + s * 0.5, 0.0),
            (o * 0.5 + s * 0.5, 0.235),
            (s, 0.26),
            (-0.03, 0.26),
        ]
        # A fascia along an overhang's edge: a band with a rounded bottom arris.
        fascia = [
            (-0.25, 0.0),
            (o + s, 0.0),
            (o + s, 0.13),
            (o + s - 0.015, 0.15),
            (s, 0.15),
            (-0.03, 0.15),
        ]
        found[name] = {
            "coping": profile(coping, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]),
            "plinth": profile(plinth, [1, 2, 3, 4]),
            "sill": profile(sill, [1, 2, 3, 4, 5, 6, 7, 8]),
            "head": profile(head, [1, 2, 3]),
            "fascia": profile(fascia, [1, 2, 3, 4]),
        }
    with open(os.path.join(OUT, "profiles.json"), "w") as f:
        json.dump(found, f, indent=1)


# Stones ---------------------------------------------------------------------------------


def stone(name, size, seed, closed=(True, True), pillow=0.004, pitch=0.003, bevel=0.012,
          full=None, ends=False):
    """A dressed stone from (0, 0, 0) to `size` in Blender's frame (x along its face, z up,
    -y its face's outward): its face divided finely, pillowed towards its middle and
    pitched by the chisel, its arrises bevelled; `closed` says whether its two ends along
    x are its own (rounded) or cut flat where it runs on into the next module's.
    `full` is (from, to) along x of the whole stone it is part of, for its pillow; `ends`
    pitches its closed ends too, where they show, as a quoin's do. Each stone is given a
    shade of its own in its corners' colour, as stones from one bed differ."""
    rng = random.Random(seed)
    sx, sy, sz = size
    nx = max(2, int(round(sx / 0.04)))
    nz = max(2, int(round(sz / 0.04)))
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    bmesh.ops.scale(bm, vec=(sx, sy, sz), verts=bm.verts)
    bmesh.ops.translate(bm, vec=(sx / 2, -sy / 2, sz / 2), verts=bm.verts)
    face_y = -sy
    # Divided first, while its faces are quads, so the grid fills: its face, and its ends
    # where they show.
    shown = [f for f in bm.faces if f.normal.y < -0.99]
    for i, own in enumerate(closed):
        if own and ends:
            x = 0.0 if i == 0 else sx
            shown += [f for f in bm.faces if abs(f.normal.x) > 0.99 and abs(f.calc_center_median().x - x) < 1e-4]
    cuts = max(nx, nz) // 2
    bmesh.ops.subdivide_edges(bm, edges=list({e for f in shown for e in f.edges}), cuts=cuts,
                              use_grid_fill=True)
    # Then its arrises bevelled: round the face, and up its closed ends; not where it is cut
    # to run on, nor at its back.
    def on_plane(e, axis, value):
        return all(abs(v.co[axis] - value) < 1e-5 for v in e.verts)
    edges = []
    for e in bm.edges:
        if len(e.link_faces) != 2:
            continue
        a, b = e.link_faces
        if a.normal.dot(b.normal) > 0.5:
            continue
        if on_plane(e, 1, 0.0):
            continue
        if (not closed[0] and on_plane(e, 0, 0.0)) or (not closed[1] and on_plane(e, 0, sx)):
            continue
        front = on_plane(e, 1, face_y)
        at_end = (closed[0] and on_plane(e, 0, 0.0)) or (closed[1] and on_plane(e, 0, sx))
        if front or at_end:
            edges.append(e)
    bmesh.ops.bevel(bm, geom=edges, offset=bevel, segments=2, affect="EDGES", profile=0.6,
                    clamp_overlap=True)
    lo, hi = full if full else (0.0, sx)
    for v in bm.verts:
        if v.co.y > face_y + 1e-4:
            continue
        # Within the bevel of the face: no pillow there, so it stays on the arris.
        dx = min(v.co.x - lo, hi - v.co.x)
        dz = min(v.co.z, sz - v.co.z)
        inner = min(dx, dz)
        if inner < bevel * 0.9:
            continue
        p = math.sin(min(1.0, dx / (0.5 * (hi - lo))) * math.pi / 2) * math.sin(min(1.0, dz / (0.5 * sz)) * math.pi / 2)
        n = noise.noise(Vector((v.co.x * 9.0 + seed * 3.1, v.co.z * 9.0, seed * 1.7)))
        n += 0.5 * noise.noise(Vector((v.co.x * 23.0, v.co.z * 23.0 + seed, 2.0)))
        # Fade the chisel's pitch out near a cut end, so the next module's half meets it.
        cut = 1.0
        if not closed[0]:
            cut = min(cut, v.co.x / 0.03)
        if not closed[1]:
            cut = min(cut, (sx - v.co.x) / 0.03)
        cut = max(0.0, min(1.0, cut))
        v.co.y -= pillow * p + pitch * n * cut * min(1.0, inner / 0.02)
    # Its closed ends, where they show: divided and pitched as its face is.
    for i, own in enumerate(closed):
        if not (own and ends):
            continue
        x = 0.0 if i == 0 else sx
        inward = 1.0 if i == 0 else -1.0
        for v in bm.verts:
            if abs(v.co.x - x) > 1e-4:
                continue
            dy = min(v.co.y + sy, -v.co.y)
            dz = min(v.co.z, sz - v.co.z)
            inner = min(dy, dz)
            if inner < bevel * 0.9:
                continue
            p = math.sin(min(1.0, dy / (0.5 * sy)) * math.pi / 2) * math.sin(min(1.0, dz / (0.5 * sz)) * math.pi / 2)
            n = noise.noise(Vector((v.co.y * 9.0 + seed * 1.3, v.co.z * 9.0, seed * 2.9 + i)))
            n += 0.5 * noise.noise(Vector((v.co.y * 23.0, v.co.z * 23.0 + seed, 5.0 + i)))
            v.co.x += inward * (pillow * p + pitch * n * min(1.0, inner / 0.02))
    # A little out of true, as set by hand.
    tilt = rng.uniform(-0.004, 0.004)
    for v in bm.verts:
        if v.co.y < face_y + 0.02:
            v.co.y += tilt * (v.co.z - sz / 2)
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    shade = rng.uniform(0.93, 1.02)
    warm = rng.uniform(-0.015, 0.015)
    colours = mesh.color_attributes.new(name="Col", type="BYTE_COLOR", domain="CORNER")
    for c in colours.data:
        c.color = (shade * (1 + warm), shade, shade * (1 - warm), 1.0)
    ob = bpy.data.objects.new(name, mesh)
    bpy.context.collection.objects.link(ob)
    for poly in mesh.polygons:
        poly.use_smooth = True
    return ob


def place(ob, at=(0, 0, 0), turn=0.0, mirror=False):
    if mirror:
        ob.scale.x = -1.0
    ob.rotation_euler.z = turn
    ob.location = at
    return ob


def join(name, parts):
    bpy.ops.object.select_all(action="DESELECT")
    for p in parts:
        p.select_set(True)
    bpy.context.view_layer.objects.active = parts[0]
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    if len(parts) > 1:
        bpy.ops.object.join()
    ob = bpy.context.view_layer.objects.active
    ob.name = name
    ob.data.name = name
    # Hard at creases over about 50 degrees, smooth across curves (docs/art-style.md).
    bpy.ops.object.shade_smooth_by_angle(angle=math.radians(50))
    # Weighted by face area, as hard-surface game art is: the big faces keep their own
    # normals and the bevels take the turn, so a stone's face is not shaded as if bowed.
    weighted = ob.modifiers.new("Weighted", "WEIGHTED_NORMAL")
    weighted.mode = "FACE_AREA"
    weighted.weight = 100
    weighted.keep_sharp = True
    return ob


def export(path):
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True,
                              export_apply=True, export_materials="NONE",
                              export_vertex_color="ACTIVE")


def quoins():
    """Quoins and jamb stones, each with its outside corner at the origin in glTF's frame,
    standing `proud` out of both faces: its faces' outward normals glTF's +X and +Z, the
    wall in -X and -Z, from y = 0 up a course. Long ones show `long` along +Z's face and
    `short` along +X's; turned and mirrored where placed, so they alternate."""
    clear()
    made = []
    for proud, tag in [(0.02, "over"), (0.0, "flush")]:
        for kind, long, short in [("quoin", 0.4, 0.2), ("jamb_long", 0.3, 0.25), ("jamb_short", 0.18, 0.25)]:
            for v in range(3):
                h = COURSE - 2 * JOINT
                # In Blender: x along the long face, -y out of it, z up. glTF's +Z face is
                # Blender's -Y face; glTF's +X face is Blender's +X end.
                ob = stone(f"{kind}_{tag}_{v}", (long + proud - JOINT, short + proud - JOINT, h),
                           seed=zlib.crc32(f"{kind}{v}{tag}".encode()) % 1000, closed=(True, True), pillow=0.007,
                           pitch=0.006, bevel=0.014, ends=True)
                # Its outer end at x = proud, its face at y = -proud (glTF z = +proud).
                ob.location = (proud - (long + proud - JOINT), (short + proud - JOINT) - proud, JOINT)
                made.append(join(ob.name, [ob]))
    export(os.path.join(OUT, "pieces.glb"))


# Modules (candidate 4) ------------------------------------------------------------------


def band_stone(name, x0, x1, z0, z1, depth, lip, ends, seed):
    """A band's stone along a tile from x0 to x1 and z0 to z1 (Blender's frame), its face
    `lip` out of the wall's face, reaching `depth` into the wall; each end 'closed' (its
    own, rounded), 'cut' (running on into the next tile), 'arris' (mitred out round an
    outside corner) or 'inside' (mitred back from an inside one)."""
    ob = stone(name, (x1 - x0, depth + lip, z1 - z0), seed,
               closed=(ends[0] == "closed", ends[1] == "closed"), pillow=0.003, pitch=0.003,
               bevel=0.01, ends=True)
    # The stone spans local y from -(depth + lip), its face, to 0; placed with its face
    # `lip` out of the wall's, at Blender y = -lip.
    ob.location = (x0, depth, z0)
    bm = bmesh.new()
    bm.from_mesh(ob.data)
    length = x1 - x0
    for i, end in enumerate(ends):
        if end not in ("arris", "inside"):
            continue
        edge = 0.0 if i == 0 else length
        sign = -1.0 if i == 0 else 1.0
        for v in bm.verts:
            out = -v.co.y - depth  # how far out of the wall's face
            limit = edge + sign * out if end == "arris" else edge - sign * out
            if abs(v.co.x - edge) < 1e-5 or (v.co.x - limit) * sign > 0:
                v.co.x = limit
    bm.to_mesh(ob.data)
    bm.free()
    return ob


def band(name, z0, z1, depth, lip, left, right, seed):
    """A band across a tile in two stones, their joint half a cube from the cubes' lines."""
    end = lambda e: "cut" if e == "plain" else e
    return [
        band_stone(name + "a", 0.0, 0.25 - JOINT, z0, z1, depth, lip, (end(left), "closed"), seed),
        band_stone(name + "b", 0.25 + JOINT, CUBE, z0, z1, depth, lip, ("closed", end(right)), seed + 1),
    ]


DEPTH = 0.1  # how deep a course's stones are


def tile(top, bottom, left, right, parity, variant):
    """A cube face of coursed dressed stone: in glTF's frame from (0, 0, 0) to (0.5, 0.5),
    x along it as seen from outside, y up, its face at z = 0 and +z out of the wall.
    Built in Blender's frame: x along, z up, -y out of the wall."""
    parts = []
    seed = zlib.crc32(f"{top}{bottom}{left}{right}{parity}{variant}".encode()) % 100000
    y0, y1 = 0.0, CUBE
    if bottom == "foot":
        parts += band("plinth", -0.06, 0.25 - JOINT, 0.08, OVER, left, right, seed + 1)
        y0 = 0.25
    elif bottom == "head":
        parts += band("head", 0.0, 0.25 - JOINT, 0.25, OVER * 0.5, left, right, seed + 2)
        y0 = 0.25
    elif bottom == "fascia":
        parts += band("fascia", 0.0, 0.15 - JOINT, 0.25, OVER, left, right, seed + 3)
        y0 = 0.15
    if top == "coping":
        y1 = CUBE - 0.1
        parts += band("coping", y1 + JOINT, CUBE + 0.045, 0.25, OVER, left, right, seed + 4)
    elif top == "sill":
        y1 = CUBE - 0.09
        parts += band("sill", y1 + JOINT, CUBE + 0.012, 0.25, OVER, left, right, seed + 5)
    # The courses between: two to a cube in a half bond. A course whose joints fall on the
    # tile's edges has a whole stone; the other has two halves, cut flat where each runs on
    # into the next tile. At an outside corner the course's stone that owns the corner runs
    # to it and shows its end on the other face; the other face's stone stops short of it.
    for k, (a, b) in enumerate([(0.0, COURSE), (COURSE, CUBE)]):
        a, b = max(a, y0), min(b, y1)
        if b - a < 0.04:
            continue
        whole = (k + parity) % 2 == 0
        if whole:
            spans = [(0.0, CUBE, "closed", "closed")]
            if variant == 1:
                spans = [(0.0, 0.3, "closed", "closed"), (0.3, CUBE, "closed", "closed")]
        else:
            spans = [(0.0, 0.25, "cut", "closed"), (0.25, CUBE, "closed", "cut")]
        for i, (s, e, l, r) in enumerate(spans):
            if s == 0.0 and left != "plain":
                l = "closed"
                if left == "arris" and not whole:
                    s = DEPTH  # the other face's stone owns the corner
            if e == CUBE and right != "plain":
                r = "closed"
                if right == "arris" and not whole:
                    e = CUBE - DEPTH
            full = (s - 0.25, e) if l == "cut" else (s, e + 0.25) if r == "cut" else None
            x0 = s + (JOINT if l == "closed" else 0.0)
            x1 = e - (JOINT if r == "closed" else 0.0)
            ob = stone(f"s{k}{i}", (x1 - x0, DEPTH, b - a - 2 * JOINT), seed + 10 * k + i,
                       closed=(l == "closed", r == "closed"), pitch=0.005,
                       full=(full[0] - x0, full[1] - x0) if full else None,
                       ends=(left == "arris" and s < 0.11) or (right == "arris" and e > CUBE - 0.11))
            ob.location = (x0, DEPTH, a + JOINT)
            parts.append(ob)
    return join(f"tile_{top}_{bottom}_{left}_{right}_{parity}_{variant}", parts)


def tiles():
    clear()
    made = 0
    for top in ("none", "coping", "sill"):
        for bottom in ("none", "foot", "head", "fascia"):
            for left in ("plain", "arris", "inside"):
                for right in ("plain", "arris", "inside"):
                    banded = top != "none" or bottom != "none"
                    if not banded and "inside" in (left, right):
                        continue
                    for parity in (0, 1):
                        plain = not banded and left == right == "plain"
                        for variant in ((0, 1) if plain else (0,)):
                            tile(top, bottom, left, right, parity, variant)
                            made += 1
    print("tiles", made)
    export(os.path.join(OUT, "tiles.glb"))


profiles()
quoins()
tiles()
print("done")
