"""The barrow halls' dressed kit (D527, docs/dungeons.md#themes-d527): the vaults, arches, piers
and burial niches the dungeon stamp lays over its cubes, as Skyrim's modular kits lay pieces
over their grid. Cubes cut to 45 degrees could not make a vault (research/2026-10-25-dungeon-
vaults.md), so each is a piece of its own, built here from curves and profiles, its masonry in
the textures rather than the triangles (art-style.md rule 1):

- `BarrelVault`: a corridor cell's barrel vault, a 3 m span a cell (4 m) long, rubble webs with an
  ashlar impost along each wall and a transverse rib of ashlar at its middle. Origin the middle
  of the springing line, the vault's axis along glTF Z.
- `GroinVault`: a room bay's groin vault, 4 m square, rising 2 m: two barrels crossing, ashlar
  ribs on its groins and half a transverse rib on each edge, so two bays side by side share one
  whole rib and a bay against a wall has its wall rib. Origin the bay's middle at the
  springing; the stamp scales it to its bay (a corridor's crossing at 0.75).
- `DoorArch`: a doorway's head on a wall's face, a round ring of ashlar over a tympanum, its
  intrados springing 3 m up (the doorway's height) over a 2.8 m span. Origin the foot of the
  wall face under the arch's middle, its front +Z, the wall's face at Z 0.
- `Pier`: a square ashlar pier, a plinth, its shaft and a capital at the springing, 2.95 m;
  half sunk in a wall, the same piece is a pilaster. Origin the middle of its base.
- `Loculus`: a burial niche's dressed front, a sill, jambs and a lintel round an opening 2 m by
  1 m, on the face of a loculi wall. Origin the foot of the opening's middle on the wall's face.
- `Shroud`, `Bones`: what lies in a loculus, a body bound in linen; a skull and long bones.
- `Sarcophagus`: a stone tomb chest 2.2 m by 1.1 m on a plinth, a moulded lid and a recumbent
  effigy on it, over the 2 m by 1 m by 1 m of cubes the crypt's `sarcophagus` module lays (its
  cubes still what is walked into). Origin the middle of its base, its length along X. Its
  ashlar darker than the arches', toward the Barrowstone walls round it.

    tools/blender/run.sh tools/props/barrow.py [Name ...]

Writes them (or only those named) to assets/models/props/barrow/. The rubble is `castle_wall_varriation` (the crypt's
walls, Barrowstone, tinted as its pack tints them), the ashlar `large_sandstone_blocks`, bone and
linen `white_stucco` tinted (assets/materials, README there), at their real sizes and cut to
1024 pixels. Nothing is random from run to run.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Vector

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
MATERIALS = os.path.join(ROOT, "assets", "materials")
OUT = os.path.join(ROOT, "assets", "models", "props", "barrow")
TEXTURE_PX = 1024
# The materials' widths in metres; the ashlar's blocks lie between V 0.09 and 0.87 of its
# image (a cornice above them, a plinth below), so a piece's courses map inside that band.
RUBBLE_M, ASHLAR_M, STUCCO_M = 2.0, 3.0, 2.0
BAND = (0.09, 0.87)
# Barrowstone's pack tint (assets/data/packs/barrowstone.ron), so the webs meet the walls.
RUBBLE = "castle_wall_varriation"
RUBBLE_TINT = (0.70, 0.70, 0.72)
ASHLAR_TINT = (0.80, 0.80, 0.82)
SPRING = 2.95


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def material(name, folder, tint=None, colour_only=None):
    """A material of an assets/materials folder, colour tinted by `tint` (glTF's base colour
    factor, a multiply the exporter reads), or a flat colour when `folder` is None."""
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nodes, links = mat.node_tree.nodes, mat.node_tree.links
    bsdf = nodes["Principled BSDF"]
    if folder is None:
        bsdf.inputs["Base Color"].default_value = (*colour_only, 1.0)
        bsdf.inputs["Roughness"].default_value = 0.95
        return mat

    def image(file, colour):
        img = bpy.data.images.load(os.path.join(MATERIALS, folder, file))
        if not colour:
            img.colorspace_settings.name = "Non-Color"
        w, h = img.size
        if max(w, h) > TEXTURE_PX:
            img.scale(TEXTURE_PX, TEXTURE_PX * h // w)
        node = nodes.new("ShaderNodeTexImage")
        node.image = img
        return node

    colour = image("color.jpg", True)
    if tint:
        mix = nodes.new("ShaderNodeMix")
        mix.data_type = "RGBA"
        mix.blend_type = "MULTIPLY"
        mix.inputs["Factor"].default_value = 1.0
        mix.inputs[7].default_value = (*tint, 1.0)
        links.new(colour.outputs["Color"], mix.inputs[6])
        links.new(mix.outputs[2], bsdf.inputs["Base Color"])
    else:
        links.new(colour.outputs["Color"], bsdf.inputs["Base Color"])
    normal = image("normal.jpg", False)
    bump = nodes.new("ShaderNodeNormalMap")
    links.new(normal.outputs["Color"], bump.inputs["Color"])
    links.new(bump.outputs["Normal"], bsdf.inputs["Normal"])
    arm = image("arm.jpg", False)
    split = nodes.new("ShaderNodeSeparateColor")
    links.new(arm.outputs["Color"], split.inputs["Color"])
    links.new(split.outputs["Green"], bsdf.inputs["Roughness"])
    links.new(split.outputs["Blue"], bsdf.inputs["Metallic"])
    return mat


class Mesh:
    """A bmesh with its UV layer, faces put in by grids whose sides are chosen by a point
    they face toward or away from."""

    def __init__(self):
        self.bm = bmesh.new()
        self.uv = self.bm.loops.layers.uv.new("UVMap")

    def grid(self, rows, mat, toward=None, away=None):
        """Quads between rows of (point, (u, v)), each facing toward the point `toward(p)`
        gives for its middle (or away from `away(p)`)."""
        verts = [[(self.bm.verts.new(p), uv) for p, uv in row] for row in rows]
        for i in range(len(rows) - 1):
            for j in range(len(rows[i]) - 1):
                quad = [verts[i][j], verts[i + 1][j], verts[i + 1][j + 1], verts[i][j + 1]]
                self.face(quad, mat, toward, away)

    def face(self, corners, mat, toward=None, away=None):
        # A quad with two corners in one place (at a vault's springing) is a triangle.
        corners = [c for k, c in enumerate(corners) if (c[0].co - corners[k - 1][0].co).length > 1e-6]
        if len(corners) < 3:
            return
        pts = [v.co for v, _ in corners]
        mid = sum(pts, Vector()) / len(pts)
        normal = (pts[1] - pts[0]).cross(pts[-1] - pts[0])
        want = (toward(mid) - mid) if toward else (mid - away(mid))
        if normal.dot(want) < 0:
            corners = corners[::-1]
        f = self.bm.faces.new([v for v, _ in corners])
        f.material_index = mat
        for loop, (_, uv) in zip(f.loops, corners):
            loop[self.uv].uv = uv

    def box(self, lo, hi, mat, metres, chamfer=0.0, band=None):
        """A box, its edges chamfered, each face's UV projected along its axis at `metres`,
        V squeezed into `band` when given (the ashlar's courses)."""
        lo, hi = Vector(lo), Vector(hi)
        c = chamfer
        # A chamfered box as the hull of three boxes, each pulled in on two axes.
        pts = []
        for ax in range(3):
            for sx in (0, 1):
                for sy in (0, 1):
                    for sz in (0, 1):
                        p = Vector([(hi[k] if s else lo[k]) for k, s in enumerate((sx, sy, sz))])
                        for k in range(3):
                            if k != ax:
                                p[k] += c if p[k] == lo[k] else -c
                        pts.append(p)
        bm = bmesh.new()
        vs = [bm.verts.new(p) for p in pts]
        bmesh.ops.remove_doubles(bm, verts=vs, dist=1e-5)
        bmesh.ops.convex_hull(bm, input=bm.verts[:])
        bmesh.ops.dissolve_limit(bm, angle_limit=0.01, verts=bm.verts[:], edges=bm.edges[:])
        centre = (lo + hi) / 2
        for f in bm.faces:
            ax = max(range(3), key=lambda k: abs(f.normal[k]))
            a, b = {0: (1, 2), 1: (0, 2), 2: (0, 1)}[ax]
            # Courses squeezed into the band when the face is taller than it.
            k = min(1.0, (band[1] - band[0]) * metres / max(hi[b] - lo[b], 1e-6)) if band and b == 2 else 1.0
            corners = []
            for v in f.verts:
                w = (band[0] + (v.co[b] - lo[b]) / metres * k) if band and b == 2 else v.co[b] / metres
                corners.append((self.bm.verts.new(v.co.copy()), (v.co[a] / metres, w)))
            self.face(corners, mat, away=lambda _: centre)
        bm.free()


def arc(r, steps, a0=0.0, a1=math.pi):
    """Points (x, z) round a half circle from +X over the top to -X."""
    return [(r * math.cos(a0 + (a1 - a0) * k / steps), r * math.sin(a0 + (a1 - a0) * k / steps)) for k in range(steps + 1)]


def rib(m, r_in, depth, width, at, axis, mat, half=0, steps=16, below=None):
    """An arch ring of ashlar: intrados radius `r_in`, `depth` deep, `width` along `axis`
    ('y' or 'x') at `at`; `half` +1 or -1 keeps only the half on that side of `at`. Its
    joints are the texture's head joints, U along the arch, the ring depth one course.
    `below`, how far it stands under the web it is sunk in: its sides are then chamfered at
    45 degrees a centimetre past that, so no side square to the axis shows. A square side
    faces along the corridor, square to the torches on the walls by it, and took no light:
    a black crescent round the ring (rounds 3-6's stills; not SSAO, the same at --quality
    low, nor a face turned away, Blender's face orientation clean from the floor, round 7).
    A chamfered rib, as Romanesque and Gothic ribs are cut, turns its sides to the floor."""
    c = below + 0.01 if below is not None else min(0.04, depth / 4)
    lo, hi = (-width / 2, width / 2) if half == 0 else ((0.0, width / 2) if half > 0 else (-width / 2, 0.0))
    # The section, (radial out from the intrados, along), chamfered on its visible edges.
    if half == 0:
        section = [(depth, lo), (c, lo), (0.0, lo + c), (0.0, hi - c), (c, hi), (depth, hi)]
    elif half > 0:
        section = [(0.0, lo), (0.0, hi - c), (c, hi), (depth, hi)]
    else:
        section = [(depth, lo), (c, lo), (0.0, lo + c), (0.0, hi)]

    def point(x, z, rr, w):
        return Vector((x * rr, at + w, z * rr)) if axis == "y" else Vector((at + w, x * rr, z * rr))

    rows = []
    for k, (x, z) in enumerate(arc(1.0, steps)):
        row, length = [], 0.0
        for j, (d, w) in enumerate(section):
            if j:
                length += math.hypot(d - section[j - 1][0], w - section[j - 1][1])
            uv = (r_in * math.pi * k / steps / ASHLAR_M, BAND[0] + length / ASHLAR_M)
            row.append((point(x, z, r_in + d, w), uv))
        rows.append(row)

    def inside(p):
        # The section's middle at this point of the arch: faces look out of it.
        x, z = (p.x, p.z) if axis == "y" else (p.y, p.z)
        n = math.hypot(x, z) or 1.0
        return point(x / n, z / n, r_in + depth * 0.5, (lo + hi) / 2)

    m.grid(rows, mat, away=inside)


def barrel_vault():
    """The corridor's barrel: 3 m span, a cell long."""
    r, length, ceiling = 1.5, 4.0, 4.5 - SPRING
    m = Mesh()
    rows = []
    steps = 24
    for y in (-length / 2, 0.0, length / 2):
        row = []
        for k, (x, z) in enumerate(arc(r, steps)):
            row.append((Vector((x, y, z)), (r * math.pi * k / steps / RUBBLE_M, y / RUBBLE_M)))
        rows.append(row)
    m.grid(rows, 0, toward=lambda p: Vector((0.0, p.y, 0.0)))
    # The ends, from the intrados up to the ceiling, facing out along the axis.
    for y in (-length / 2, length / 2):
        top = [(Vector((x, y, ceiling)), (x / RUBBLE_M, ceiling / RUBBLE_M)) for x, _ in arc(r, steps)]
        low = [(Vector((x, y, z)), (x / RUBBLE_M, z / RUBBLE_M)) for x, z in arc(r, steps)]
        m.grid([low, top], 0, away=lambda p, y=y: Vector((p.x, 0.0, p.z)))
    # Imposts along both walls, an ashlar band under the springing, moulded.
    for side in (-1, 1):
        prof = [(0.0, -0.32), (0.10, -0.26), (0.14, -0.12), (0.14, -0.02), (0.0, 0.02)]
        rows = []
        for y in (-length / 2, length / 2):
            row, length_v = [], 0.0
            for j, (d, z) in enumerate(prof):
                if j:
                    length_v += math.hypot(d - prof[j - 1][0], z - prof[j - 1][1])
                row.append((Vector((side * (r - d), y, z)), (y / ASHLAR_M, BAND[0] + length_v / ASHLAR_M)))
            rows.append(row)
        m.grid(rows, 1, away=lambda p, s=side: Vector((s * (r + 0.3), p.y, p.z)))
    rib(m, r - 0.12, 0.17, 0.5, 0.0, "y", 1, below=0.12)
    return m, [("Rubble", RUBBLE, RUBBLE_TINT), ("Ashlar", "large_sandstone_blocks", ASHLAR_TINT)]


def groin_vault():
    """A room bay: two barrels of 4 m span crossing, ribs on the groins and the edges."""
    r, ceiling, steps, cuts = 2.0, 5.0 - SPRING, 24, 6
    m = Mesh()
    # Four sectors between the diagonals; in the east and west ones the barrel running along X.
    for axis in ("x", "y"):
        for sign in (-1, 1):
            rows = []
            for t in range(cuts + 1):
                row = []
                for k, (w, z) in enumerate(arc(r, steps)):
                    a = abs(w) + (r - abs(w)) * t / cuts
                    p = Vector((sign * a, w, z)) if axis == "x" else Vector((w, sign * a, z))
                    row.append((p, (r * math.pi * k / steps / RUBBLE_M, a / RUBBLE_M)))
                rows.append(row)
            m.grid(rows, 0, toward=lambda p: Vector((0.0, 0.0, -0.5)))
            # The bay's edge, from the arch up to the ceiling, facing out.
            low = [(Vector((sign * r, w, z)) if axis == "x" else Vector((w, sign * r, z)), (w / RUBBLE_M, z / RUBBLE_M)) for w, z in arc(r, steps)]
            top = [(Vector((sign * r, w, ceiling)) if axis == "x" else Vector((w, sign * r, ceiling)), (w / RUBBLE_M, ceiling / RUBBLE_M)) for w, _ in arc(r, steps)]
            m.grid([low, top], 0, toward=lambda p: p * 2.0 - Vector((0, 0, p.z)))
            # Half a transverse rib on the edge, inside the bay.
            at = sign * r
            rib(m, r - 0.14, 0.2, 0.5, at, "y" if axis == "y" else "x", 1, half=-sign, steps=16, below=0.14)
    # Diagonal ribs on the groins: each over its diagonal, at a horizontal distance h from the
    # middle and z = sqrt(r² - h²/2), half an ellipse of semi-axes r√2 and r.
    ea, eb, n = r * math.sqrt(2.0), r, 24
    prof = [(-0.01, -0.13), (0.10, -0.13), (0.15, -0.06), (0.15, 0.06), (0.10, 0.13), (-0.01, 0.13)]

    def on_curve(h, diag):
        z = math.sqrt(max(0.0, eb * eb * (1.0 - h * h / (ea * ea))))
        inward = -(diag * (h / ea**2) + Vector((0, 0, z / eb**2)))
        return diag * h + Vector((0, 0, z)), inward.normalized()

    for d in ((1, 1), (1, -1)):
        diag = Vector((d[0], d[1], 0.0)).normalized()
        side = Vector((-diag.y, diag.x, 0.0))
        rows = []
        for k in range(n + 1):
            a = math.pi * k / n
            centre, inward = on_curve(ea * math.cos(a), diag)
            row, length = [], 0.0
            for j, (dn, w) in enumerate(prof):
                if j:
                    length += math.hypot(dn - prof[j - 1][0], w - prof[j - 1][1])
                row.append((centre + inward * dn + side * w, (2.45 * a / ASHLAR_M, BAND[0] + length / ASHLAR_M)))
            rows.append(row)

        def inside(p, diag=diag):
            h = max(-ea, min(ea, p.dot(diag)))
            centre, inward = on_curve(h, diag)
            return centre + inward * 0.07

        m.grid(rows, 1, away=inside)
    return m, [("Rubble", RUBBLE, RUBBLE_TINT), ("Ashlar", "large_sandstone_blocks", ASHLAR_TINT)]


def door_arch():
    """A ring over a doorway on a wall face: intrados 1.4 m round from 3 m up, 0.32 deep,
    standing 0.1 m proud, a tympanum filling it down to the doorway's head."""
    r_in, depth, proud, steps = 1.4, 0.32, 0.1, 20
    m = Mesh()
    rows = []
    prof = [(0.0, 0.0), (0.0, -proud + 0.03), (0.03, -proud), (depth - 0.03, -proud), (depth, -proud + 0.03), (depth, 0.0)]
    for k, (x, z) in enumerate(arc(1.0, steps)):
        row, length = [], 0.0
        for j, (d, y) in enumerate(prof):
            if j:
                length += math.hypot(d - prof[j - 1][0], y - prof[j - 1][1])
            rr = r_in + d
            row.append((Vector((x * rr, y, 3.0 + z * rr)), (r_in * math.pi * k / steps / ASHLAR_M, BAND[0] + length / ASHLAR_M)))
        rows.append(row)
    m.grid(rows, 1, away=lambda p: Vector((0.0, -proud / 2, 3.0)) + Vector((p.x, 0.0, p.z - 3.0)).normalized() * (r_in + depth / 2))
    # Imposts the ring stands on, a block each side down to the doorway's head.
    for s in (-1, 1):
        m.box((min(s * (r_in - 0.05), s * (r_in + depth + 0.05)), -proud - 0.04, 2.78), (max(s * (r_in - 0.05), s * (r_in + depth + 0.05)), 0.0, 3.0), 1, ASHLAR_M, 0.02)
    # The tympanum: the ring's inside down to the head, a flat stone a little proud of the wall.
    low = [(Vector((x * r_in, -0.03, 3.0)), (x * r_in / ASHLAR_M + 0.3, 0.5)) for x, _ in arc(1.0, steps)]
    high = [(Vector((x * r_in, -0.03, 3.0 + z * r_in)), (x * r_in / ASHLAR_M + 0.3, 0.5 + z * r_in / ASHLAR_M)) for x, z in arc(1.0, steps)]
    m.grid([low, high], 1, toward=lambda p: Vector((p.x, -2.0, p.z)))
    m.box((-r_in - 0.02, -0.05, 2.86), (r_in + 0.02, 0.0, 3.04), 1, ASHLAR_M, 0.02)
    return m, [("Rubble", RUBBLE, RUBBLE_TINT), ("Ashlar", "large_sandstone_blocks", ASHLAR_TINT)]


def pier():
    """Plinth, shaft and capital, 1.04 m square over its 1 m of cubes."""
    m = Mesh()
    m.box((-0.64, -0.64, 0.0), (0.64, 0.64, 0.28), 0, ASHLAR_M, 0.05)
    m.box((-0.58, -0.58, 0.28), (0.58, 0.58, 0.40), 0, ASHLAR_M, 0.04)
    m.box((-0.52, -0.52, 0.40), (0.52, 0.52, SPRING - 0.36), 0, ASHLAR_M, 0.03, band=BAND)
    m.box((-0.58, -0.58, SPRING - 0.36), (0.58, 0.58, SPRING - 0.22), 0, ASHLAR_M, 0.03)
    m.box((-0.66, -0.66, SPRING - 0.22), (0.66, 0.66, SPRING), 0, ASHLAR_M, 0.04)
    return m, [("Ashlar", "large_sandstone_blocks", ASHLAR_TINT)]


def loculus():
    """A niche's front: a sill, two jambs and a lintel round a 2 m by 1 m opening."""
    m = Mesh()
    m.box((-1.12, -0.10, -0.12), (1.12, 0.0, 0.02), 0, ASHLAR_M, 0.02)
    for s in (-1, 1):
        m.box((min(s * 1.0, s * 1.16), -0.08, 0.0), (max(s * 1.0, s * 1.16), 0.0, 1.0), 0, ASHLAR_M, 0.02)
    m.box((-1.18, -0.10, 1.0), (1.18, 0.0, 1.2), 0, ASHLAR_M, 0.025)
    return m, [("Ashlar", "large_sandstone_blocks", ASHLAR_TINT)]


def lump(m, centre, radii, mat, segs=10, rings=6, bulge=None):
    """An ellipsoid, its rings each pushed in or out by `bulge(t)` (t from one end to the
    other along X)."""
    bm = bmesh.new()
    bmesh.ops.create_uvsphere(bm, u_segments=segs, v_segments=rings, radius=1.0)
    for v in bm.verts:
        t = (v.co.x + 1) / 2
        k = bulge(t) if bulge else 1.0
        v.co = Vector((v.co.x * radii[0], v.co.y * radii[1] * k, v.co.z * radii[2] * k)) + Vector(centre)
    for f in bm.faces:
        corners = [(m.bm.verts.new(v.co.copy()), (v.co.x / STUCCO_M, (v.co.y + v.co.z) / STUCCO_M)) for v in f.verts]
        m.face(corners, mat, away=lambda _: Vector(centre))
    bm.free()


def shroud():
    """A body bound in linen, 1.7 m, lying on its back: head, shoulders, the narrowing to the
    feet, cords round it every 0.3 m."""
    m = Mesh()

    def body(t):
        head = 0.75 + 0.25 * math.cos(min(1.0, abs(t - 0.92) / 0.08) * math.pi / 2)
        waist = 1.0 - 0.45 * max(0.0, 0.7 - t)
        neck = 1.0 - 0.35 * math.exp(-((t - 0.83) / 0.03) ** 2)
        return min(waist, head if t > 0.85 else 1.0) * neck

    lump(m, (0.0, 0.0, 0.14), (0.85, 0.22, 0.14), 0, segs=20, rings=10, bulge=body)
    for k in range(5):
        x = -0.6 + 0.3 * k
        t = (x / 0.85 + 1) / 2
        lump(m, (x, 0.0, 0.14), (0.025, 0.22 * body(t) + 0.012, 0.14 * body(t) + 0.012), 1, segs=12, rings=6)
    return m, [("Linen", "white_stucco", (0.52, 0.47, 0.38)), ("Cord", "white_stucco", (0.30, 0.25, 0.18))]


def bones():
    """A skull at one end, long bones laid along the niche, a few ribs: a body long gone."""
    m = Mesh()
    lump(m, (0.62, 0.02, 0.10), (0.10, 0.075, 0.09), 0, segs=12, rings=8)
    lump(m, (0.69, 0.02, 0.06), (0.05, 0.055, 0.045), 0, segs=10, rings=6)
    for s in (-1, 1):
        lump(m, (0.665, 0.02 + s * 0.03, 0.10), (0.012, 0.018, 0.018), 1, segs=8, rings=4)
    for k, (x, y, ln, r) in enumerate(((-0.05, -0.08, 0.45, 0.022), (-0.1, 0.07, 0.43, 0.022), (-0.55, -0.05, 0.4, 0.02), (-0.5, 0.09, 0.38, 0.019), (0.25, 0.12, 0.3, 0.015))):
        lump(m, (x, y, r), (ln / 2, r, r), 0, segs=8, rings=4)
        for e in (-1, 1):
            lump(m, (x + e * ln / 2, y, r * 1.3), (r * 1.6, r * 1.7, r * 1.4), 0, segs=8, rings=4)
    for k in range(5):
        lump(m, (0.22 + 0.06 * k - 0.1, -0.02 - 0.01 * k, 0.03), (0.012, 0.13, 0.012), 0, segs=6, rings=4)
    lump(m, (0.15, -0.02, 0.05), (0.09, 0.12, 0.04), 0, segs=8, rings=4)
    return m, [("Bone", "white_stucco", (0.72, 0.65, 0.52)), ("Socket", None, (0.03, 0.025, 0.02))]


def sarcophagus():
    """A plinth, a chest with a base and a cornice moulding, a lid stepped up to a slab, and on
    it an effigy lying with its hands folded: 2.2 by 1.1 m, 1.25 m to the effigy's chest."""
    m = Mesh()
    m.box((-1.12, -0.57, 0.0), (1.12, 0.57, 0.12), 0, ASHLAR_M, 0.03)
    m.box((-1.06, -0.51, 0.12), (1.06, 0.51, 0.20), 0, ASHLAR_M, 0.03)
    m.box((-1.02, -0.47, 0.20), (1.02, 0.47, 0.82), 0, ASHLAR_M, 0.02, band=BAND)
    m.box((-1.07, -0.52, 0.82), (1.07, 0.52, 0.90), 0, ASHLAR_M, 0.03)
    m.box((-1.10, -0.55, 0.90), (1.10, 0.55, 1.00), 0, ASHLAR_M, 0.035)
    m.box((-1.00, -0.44, 1.00), (1.00, 0.44, 1.06), 0, ASHLAR_M, 0.03)

    def body(t):
        head = 0.75 + 0.25 * math.cos(min(1.0, abs(t - 0.92) / 0.08) * math.pi / 2)
        waist = 1.0 - 0.4 * max(0.0, 0.7 - t)
        neck = 1.0 - 0.3 * math.exp(-((t - 0.83) / 0.03) ** 2)
        return min(waist, head if t > 0.85 else 1.0) * neck

    lump(m, (0.02, 0.0, 1.14), (0.86, 0.24, 0.10), 0, segs=20, rings=10, bulge=body)
    # A cushion under the head, the hands folded on the breast, the feet on a block.
    m.box((0.66, -0.22, 1.06), (0.94, 0.22, 1.14), 0, ASHLAR_M, 0.03)
    lump(m, (0.40, 0.0, 1.25), (0.09, 0.12, 0.05), 0, segs=10, rings=6)
    m.box((-0.98, -0.18, 1.06), (-0.84, 0.18, 1.24), 0, ASHLAR_M, 0.03)
    return m, [("Ashlar", "large_sandstone_blocks", (0.66, 0.65, 0.66))]


def export(name, built):
    m, mats = built
    bmesh.ops.remove_doubles(m.bm, verts=m.bm.verts[:], dist=1e-5)
    bmesh.ops.triangulate(m.bm, faces=m.bm.faces[:])
    me = bpy.data.meshes.new(name)
    m.bm.to_mesh(me)
    m.bm.free()
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    for mname, folder, tint in mats:
        me.materials.append(material(mname, folder, tint) if folder else material(mname, None, colour_only=tint))
    bpy.context.view_layer.objects.active = ob
    ob.select_set(True)
    bpy.ops.object.shade_smooth_by_angle(angle=math.radians(40.0), keep_sharp_edges=True)
    lo = Vector([min(v.co[i] for v in me.vertices) for i in range(3)])
    hi = Vector([max(v.co[i] for v in me.vertices) for i in range(3)])
    tris = len(me.polygons)
    size = hi - lo
    print(f"BARROW {name}: {tris} triangles, {size.x:.2f} x {size.z:.2f} x {size.y:.2f} m (x, up, z), "
          f"up {lo.z:.2f} to {hi.z:.2f}, {len(mats)} materials")
    os.makedirs(OUT, exist_ok=True)
    bpy.ops.export_scene.gltf(
        filepath=os.path.join(OUT, f"{name}.glb"),
        export_format="GLB",
        use_selection=True,
        export_tangents=True,
        export_image_format="JPEG",
        export_jpeg_quality=85,
    )


ONLY = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
for name, make in (("BarrelVault", barrel_vault), ("GroinVault", groin_vault), ("DoorArch", door_arch), ("Pier", pier), ("Loculus", loculus), ("Shroud", shroud), ("Bones", bones), ("Sarcophagus", sarcophagus)):
    if ONLY and name not in ONLY:
        continue
    reset()
    export(name, make())
