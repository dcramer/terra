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
- `Loculus`: a burial niche's front, a heavy lintel over an opening 2 m by 1 m and a plain
  lining in its depth, in the walls' rubble a step darker and warmer, on the face of a loculi wall. Origin the foot of the opening's middle on the wall's face.
- `Shroud`: what lies in a loculus, a body bound in linen (its bones are the Tripo props
  `Skull` and `BoneHeap`, art/props/barrow_skull and barrow_bone_heap).
- `Sarcophagus`: a stone tomb chest 2.2 m by 1.1 m on a plinth, a moulded lid and a recumbent
  effigy on it, over the 2 m by 1 m by 1 m of cubes the crypt's `sarcophagus` module lays (its
  cubes still what is walked into). Origin the middle of its base, its length along X. Its
  ashlar darker than the arches', toward the Barrowstone walls round it.

- `Candles`, `Altar`, `Urns`, `Coffin`: a room's dressing by its type (docs/dungeons.md, room
  types, D539): grave candles on their wax, an offering altar with its runner and candles,
  three lidded cinerary urns, a coffin with its lid pushed aside. Their flames glow.
- `Debris`: what lies strewn over a floor, stones fallen from the walls and their grit.
- `UrnFallen`, `Planks`, `StandFallen`: bigger things fallen on a floor that tell what
  happened: an urn knocked over and its ashes spilt, a coffin lid's broken boards, a candle
  stand toppled.
    tools/blender/run.sh tools/props/barrow.py [Name ...]

Writes them (or only those named) to assets/models/props/barrow/. The rubble is `castle_wall_varriation` (the crypt's
walls, Barrowstone, tinted as its pack tints them), the ashlar `large_sandstone_blocks`, bone and
linen `white_stucco` tinted (assets/materials, README there), at their real sizes and cut to
1024 pixels. Nothing is random from run to run.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import bmesh
import bpy
from mathutils import Matrix, Vector

from aged import grime

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
# The ashlar warm and a step darker than the rubble's face, so arches and piers read
# as the same stone dressed rather than a pale kit set in a dark wall (round 10's reviewers: cold
# blue-grey piers against the warm walls).
ASHLAR_TINT = (0.62, 0.56, 0.48)
SPRING = 2.95
# A loculus's lintel a step darker and warmer than the walls' rubble, its recess darker still,
# so the niche reads deep and the lintel as one heavy stone of the wall (barrow round 17).
LINTEL_TINT = (0.60, 0.55, 0.50)
RECESS_TINT = (0.42, 0.36, 0.31)


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def material(name, folder, tint=None, colour_only=None):
    """A material of an assets/materials folder, colour tinted by `tint` (glTF's base colour
    factor, a multiply the exporter reads), or a flat colour when `folder` is None."""
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nodes, links = mat.node_tree.nodes, mat.node_tree.links
    bsdf = nodes["Principled BSDF"]
    if folder == "glow":
        # A candle's flame: its own light, under the fire cards' 8 (refine.py), since a flame a
        # centimetre across at 8 clipped to a white speck in the crypt's adapted exposure
        # (barrow dress2's stills); at 2 its red and green stay apart and it reads orange.
        bsdf.inputs["Base Color"].default_value = (*tint, 1.0)
        bsdf.inputs["Emission Color"].default_value = (*tint, 1.0)
        bsdf.inputs["Emission Strength"].default_value = 2.0
        return mat
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
        # Aged: dirt baked into its vertices' colour (`grime`), as old bone lying in a niche.
        self.aged = False

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
    """A niche's front: a heavy lintel over the 2 m by 1 m opening and a plain lining in its
    half metre of depth, both in the walls' own rubble a step darker and warmer than they are
    (the walls get dirt in the game, this piece none). A sill and jambs round the opening read
    as thin picture frames pasted on the wall (barrow round 17); the catacombs' loculi are cut
    plain into the rock, framed by nothing."""
    m = Mesh()
    # The lintel, 0.35 m high and 0.15 m proud, bearing 0.2 m into the wall each side.
    m.box((-1.2, -0.15, 1.0), (1.2, 0.0, 1.35), 0, RUBBLE_M, 0.04)
    # The lining just inside the cubes' cut (0.5 m deep), facing into the opening.
    w, d, h, e = 1.0 - 0.004, 0.5 - 0.004, 1.0 - 0.004, 0.004
    inside = lambda _: Vector((0.0, -1.0, 0.5))
    m.grid([[(Vector((-w, d, e)), (-w / RUBBLE_M, e / RUBBLE_M)), (Vector((w, d, e)), (w / RUBBLE_M, e / RUBBLE_M))],
            [(Vector((-w, d, h)), (-w / RUBBLE_M, h / RUBBLE_M)), (Vector((w, d, h)), (w / RUBBLE_M, h / RUBBLE_M))]],
           1, toward=inside)
    for s in (-1, 1):
        m.grid([[(Vector((s * w, 0.0, e)), (0.0, e / RUBBLE_M)), (Vector((s * w, d, e)), (d / RUBBLE_M, e / RUBBLE_M))],
                [(Vector((s * w, 0.0, h)), (0.0, h / RUBBLE_M)), (Vector((s * w, d, h)), (d / RUBBLE_M, h / RUBBLE_M))]],
               1, toward=inside)
    for z in (e, h):
        m.grid([[(Vector((-w, 0.0, z)), (-w / RUBBLE_M, 0.0)), (Vector((w, 0.0, z)), (w / RUBBLE_M, 0.0))],
                [(Vector((-w, d, z)), (-w / RUBBLE_M, d / RUBBLE_M)), (Vector((w, d, z)), (w / RUBBLE_M, d / RUBBLE_M))]],
               1, toward=lambda p: Vector((p.x, p.y, 0.5)))
    return m, [("Lintel", RUBBLE, LINTEL_TINT), ("Recess", RUBBLE, RECESS_TINT)]


def lump(m, centre, radii, mat, segs=10, rings=6, bulge=None, turn=0.0):
    """An ellipsoid, its rings each pushed in or out by `bulge(t)` (t from one end to the
    other along X), turned `turn` radians about the up axis."""
    bm = bmesh.new()
    bmesh.ops.create_uvsphere(bm, u_segments=segs, v_segments=rings, radius=1.0)
    c, s = math.cos(turn), math.sin(turn)
    for v in bm.verts:
        t = (v.co.x + 1) / 2
        k = bulge(t) if bulge else 1.0
        x, y = v.co.x * radii[0], v.co.y * radii[1] * k
        v.co = Vector((x * c - y * s, x * s + y * c, v.co.z * radii[2] * k)) + Vector(centre)
    for f in bm.faces:
        corners = [(m.bm.verts.new(v.co.copy()), (v.co.x / STUCCO_M, (v.co.y + v.co.z) / STUCCO_M)) for v in f.verts]
        m.face(corners, mat, away=lambda _: Vector(centre))
    bm.free()


def shroud():
    """A body bound in linen, 1.7 m, lying on its back: head, shoulders, the narrowing to the
    feet, cords round it every 0.3 m. Grimed, so it lies in the dust."""
    m = Mesh()
    m.aged = True

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
    # Linen centuries in the dust, a dun grey-brown: at 0.52 it lit to a white bolster in the
    # torchlight, paler than the floor (dress10's stills).
    return m, [("Linen", "white_stucco", (0.20, 0.17, 0.13)), ("Cord", "white_stucco", (0.12, 0.09, 0.06))]


def debris():
    """Stones fallen from the walls and their grit, as lie on any old crypt's floor: two the
    size of a fist and a half, four of an egg, a scatter of chips, faceted (few faces, the
    shading's sharp edges) in the walls' rubble, over 0.7 by 0.5 m. Grimed (aged.py), so they
    sit in the dust rather than on it."""
    m = Mesh()
    m.aged = True
    for x, y, rx, ry, rz, a in ((-0.18, 0.05, 0.09, 0.07, 0.05, 0.3), (0.15, -0.08, 0.07, 0.06, 0.045, 1.9),
                                (0.02, 0.16, 0.045, 0.035, 0.03, 0.8), (-0.28, -0.14, 0.04, 0.03, 0.025, 2.4),
                                (0.30, 0.10, 0.035, 0.03, 0.025, 1.1), (-0.02, -0.18, 0.04, 0.028, 0.022, -0.6)):
        lump(m, (x, y, rz * 0.7), (rx, ry, rz), 0, segs=6, rings=4, turn=a)
    for k in range(9):
        x, y = -0.3 + 0.075 * k, 0.2 * math.sin(k * 2.3)
        lump(m, (x, y, 0.008), (0.016, 0.012, 0.01), 0, segs=5, rings=3, turn=k * 0.7)
    return m, [("Rubble", RUBBLE, (0.55, 0.53, 0.52))]


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
    return m, [("Ashlar", "large_sandstone_blocks", (0.56, 0.51, 0.44))]


def lathe(m, centre, profile, mat, metres, segs=12):
    """A surface turned about the vertical through `centre`, its `profile` (radius, height)
    from the foot up; U round it and V up it at `metres` a texture's width. Faces look away
    from the axis at the profile's middle height, so its caps face up and down."""
    c = Vector(centre)
    mid = c + Vector((0.0, 0.0, sum(z for _, z in profile) / len(profile)))
    rows = []
    for k in range(segs + 1):
        a = 2 * math.pi * k / segs
        rows.append([(c + Vector((r * math.cos(a), r * math.sin(a), z)), (a * 0.2 / metres, z / metres)) for r, z in profile])
    m.grid(rows, mat, away=lambda _: mid)


def prism(m, outline, z0, z1, side, top, metres, bottom=None):
    """An upright prism over a plan `outline` (x, y), counter-clockwise: its sides in `side`,
    U along the outline, its top in `top`."""
    mid = Vector((sum(x for x, _ in outline) / len(outline), sum(y for _, y in outline) / len(outline), (z0 + z1) / 2))
    run = 0.0
    for k, (x, y) in enumerate(outline):
        nx, ny = outline[(k + 1) % len(outline)]
        ln = math.hypot(nx - x, ny - y)
        u0, u1 = run / metres, (run + ln) / metres
        run += ln
        m.face([(m.bm.verts.new((x, y, z0)), (u0, z0 / metres)), (m.bm.verts.new((nx, ny, z0)), (u1, z0 / metres)),
                (m.bm.verts.new((nx, ny, z1)), (u1, z1 / metres)), (m.bm.verts.new((x, y, z1)), (u0, z1 / metres))], side, away=lambda _: mid)
    for z, mat in ((z1, top), (z0, side if bottom is None else bottom)):
        m.face([(m.bm.verts.new((x, y, z)), (x / metres, y / metres)) for x, y in outline], mat, away=lambda _: mid)


# Tallow candles, the crypt's lights left by mourners: wax, the wick, the flame (emissive).
# Old tallow, yellowed and grimed: at (0.62, 0.54, 0.40) each mound read as a white plate.
WAX = ("Wax", "white_stucco", (0.45, 0.37, 0.25))
WICK = ("Wick", None, (0.03, 0.025, 0.02))
FLAME = ("Flame", "glow", (1.0, 0.42, 0.10))
# The grave candles' mound: old tallow under dust, about 0.3 in sRGB over the stucco, half the
# candles' own wax, before its grime.
POOL = ("Pool", "white_stucco", (0.25, 0.19, 0.12))


def candle(m, x, y, z, r, h, mats=(0, 1, 2)):
    """A candle burnt down to `h`, its top hollowed by the flame, a drip or two down its side,
    its wick and its flame. Its materials: wax, wick, flame."""
    wax, wick, flame = mats
    lathe(m, (x, y, z), [(0.0, 0.0), (r, 0.0), (r, h * 0.92), (r * 0.82, h), (r * 0.3, h - 0.008), (0.0, h - 0.008)], wax, STUCCO_M, 10)
    for k in range(2):
        a = 1.3 + 2.4 * k + x * 7.0
        lump(m, (x + r * math.cos(a), y + r * math.sin(a), z + h * (0.55 + 0.2 * k)), (0.009, 0.009, h * 0.16), wax, segs=6, rings=4)
    m.box((x - 0.003, y - 0.003, z + h - 0.01), (x + 0.003, y + 0.003, z + h + 0.012), wick, 1.0)
    lump(m, (x, y, z + h + 0.032), (0.009, 0.009, 0.024), flame, segs=6, rings=4)


def candles():
    """Grave candles stood on a mound of their own wax on the floor, seven of them from a stub
    to a hand and a half, as mourners leave them in Elden Ring's catacombs and Skyrim's halls:
    0.5 by 0.35 m, 0.36 m to the tallest flame. The mound is years of tallow trodden and
    dusted over, darker than the candles (`POOL`) and grimed as the bones are: flat, pale and
    30 cm under the lamp's light it read as a white plate on the floor (barrow dress6, all
    reviewers). Broken at its edge into a few runs and drops, not one disc."""
    m = Mesh()
    m.aged = True
    lump(m, (0.0, 0.0, 0.0), (0.21, 0.15, 0.022), 3, segs=14, rings=6)
    for x, y, rx, ry in ((0.20, -0.07, 0.07, 0.045), (-0.19, 0.09, 0.06, 0.04), (0.05, 0.16, 0.05, 0.03), (-0.26, -0.03, 0.035, 0.025)):
        lump(m, (x, y, 0.0), (rx, ry, 0.012), 3, segs=8, rings=4)
    for x, y, r, h in ((-0.12, 0.05, 0.04, 0.30), (0.02, 0.08, 0.035, 0.22), (0.14, 0.02, 0.03, 0.16), (-0.03, -0.07, 0.03, 0.11),
                       (0.10, -0.09, 0.025, 0.07), (-0.18, -0.06, 0.028, 0.13), (0.20, 0.10, 0.022, 0.05)):
        candle(m, x, y, 0.015, r, h)
    return m, [WAX, WICK, FLAME, POOL]


def altar():
    """An offering altar against a wall: a stone table 1.4 by 0.6 m and 0.95 m high on a plinth,
    a linen runner over its top and down its front, candles at its ends and a bowl between
    them. Origin the middle of its base; its front -Y (glTF +Z), its back on the wall."""
    m = Mesh()
    m.box((-0.72, -0.31, 0.0), (0.72, 0.30, 0.10), 0, ASHLAR_M, 0.03)
    m.box((-0.64, -0.25, 0.10), (0.64, 0.27, 0.85), 0, ASHLAR_M, 0.02, band=BAND)
    m.box((-0.74, -0.31, 0.85), (0.74, 0.30, 0.95), 0, ASHLAR_M, 0.03)
    m.box((-0.28, -0.312, 0.95), (0.28, 0.24, 0.962), 1, STUCCO_M)
    m.box((-0.28, -0.322, 0.50), (0.28, -0.31, 0.962), 1, STUCCO_M)
    for x, y, r, h in ((-0.58, 0.05, 0.04, 0.26), (-0.50, -0.10, 0.03, 0.15), (0.56, 0.08, 0.04, 0.22), (0.62, -0.08, 0.03, 0.11)):
        candle(m, x, y, 0.95, r, h, (2, 3, 4))
    lathe(m, (0.0, 0.05, 0.962), [(0.0, 0.0), (0.07, 0.0), (0.12, 0.05), (0.11, 0.055), (0.0, 0.03)], 5, STUCCO_M, 14)
    # The runner a dark wine: in pale linen it read as a sheet of paper pinned to the altar, and
    # as an oxblood of 0.30/0.13/0.09 it lit salmon-pink in the torchlight (dress12's rooms).
    return m, [("Ashlar", "large_sandstone_blocks", (0.56, 0.51, 0.44)), ("Linen", "white_stucco", (0.16, 0.05, 0.06)),
               WAX, WICK, FLAME, ("Bowl", "white_stucco", (0.36, 0.24, 0.16))]


def urns():
    """Three cinerary urns of fired clay, lidded, 0.5, 0.4 and 0.3 m tall, the smallest's lid
    off and lying at its foot: the ashes of the dead set against a wall. 0.9 by 0.4 m."""
    m = Mesh()
    m.aged = True
    for x, y, s, lid in ((-0.22, 0.02, 1.0, True), (0.14, -0.04, 0.8, True), (0.36, 0.10, 0.6, False)):
        prof = [(0.0, 0.0), (0.09 * s, 0.0), (0.10 * s, 0.03 * s), (0.17 * s, 0.2 * s), (0.18 * s, 0.28 * s), (0.13 * s, 0.39 * s),
                (0.085 * s, 0.43 * s), (0.10 * s, 0.46 * s), (0.0, 0.46 * s)]
        lathe(m, (x, y, 0.0), prof, 0, STUCCO_M, 14)
        top = (x, y, 0.46 * s) if lid else (x + 0.14, y - 0.12, 0.0)
        lathe(m, top, [(0.0, 0.0), (0.11 * s, 0.0), (0.10 * s, 0.025 * s), (0.04 * s, 0.05 * s), (0.03 * s, 0.075 * s), (0.0, 0.08 * s)], 0, STUCCO_M, 12)
        if not lid:
            lathe(m, (x, y, 0.461 * s), [(0.0, 0.0), (0.085 * s, 0.0), (0.0, 0.001)], 1, STUCCO_M, 12)
    # A dark fired clay: at 0.50/0.33/0.22 it lit salmon-pink in the torchlight (dress10's crops).
    return m, [("Clay", "white_stucco", (0.30, 0.17, 0.10)), ("Ash", None, (0.05, 0.045, 0.04))]


def coffin():
    """A wooden coffin, shouldered, 1.95 by 0.62 m and 0.42 m high, its lid pushed half off by
    a robber: the dark of its inside shows. Length along X, the head at +X."""
    m = Mesh()
    plan = [(-0.98, -0.20), (0.42, -0.31), (0.98, -0.23), (0.98, 0.23), (0.42, 0.31), (-0.98, 0.20)]
    # Hollow, its boards 3 cm thick: the rim, the inner walls and the dark floor show where
    # the lid is off, so it reads as an open coffin rather than a box painted dark on top.
    top, floor, t = 0.36, 0.06, 0.03
    inner = [(x - math.copysign(t, x), y - math.copysign(t, y)) for x, y in plan]
    centre = Vector((0.0, 0.0, (top + floor) / 2))
    up = lambda p: p + Vector((0.0, 0.0, 1.0))
    run = 0.0
    for k, (x, y) in enumerate(plan):
        nx, ny = plan[(k + 1) % len(plan)]
        u0, u1 = run / 2.0, (run + math.hypot(nx - x, ny - y)) / 2.0
        run += math.hypot(nx - x, ny - y)
        m.face([(m.bm.verts.new((x, y, 0.0)), (u0, 0.0)), (m.bm.verts.new((nx, ny, 0.0)), (u1, 0.0)),
                (m.bm.verts.new((nx, ny, top)), (u1, top / 2.0)), (m.bm.verts.new((x, y, top)), (u0, top / 2.0))], 0,
               away=lambda _: centre)
        ix, iy = inner[k]
        jx, jy = inner[(k + 1) % len(inner)]
        m.face([(m.bm.verts.new((x, y, top)), (x / 2.0, y / 2.0)), (m.bm.verts.new((nx, ny, top)), (nx / 2.0, ny / 2.0)),
                (m.bm.verts.new((jx, jy, top)), (jx / 2.0, jy / 2.0)), (m.bm.verts.new((ix, iy, top)), (ix / 2.0, iy / 2.0))], 0,
               toward=up)
        m.face([(m.bm.verts.new((ix, iy, floor)), (u0, floor / 2.0)), (m.bm.verts.new((jx, jy, floor)), (u1, floor / 2.0)),
                (m.bm.verts.new((jx, jy, top)), (u1, top / 2.0)), (m.bm.verts.new((ix, iy, top)), (u0, top / 2.0))], 0,
               toward=lambda _: centre)
    m.face([(m.bm.verts.new((x, y, floor)), (x / 2.0, y / 2.0)) for x, y in inner], 1, toward=up)
    m.face([(m.bm.verts.new((x, y, 0.0)), (x / 2.0, y / 2.0)) for x, y in plan], 0, toward=lambda p: p - Vector((0.0, 0.0, 1.0)))
    lid = [(x * 1.02, y * 1.04) for x, y in plan]
    # Slid toward the foot and turned, so a third of the opening shows at the head: from eye
    # height 5 m off, a lid 22 cm aside still covered it and the coffin read as a box.
    a, dx, dy = math.radians(12.0), -0.42, -0.12
    lid = [(x * math.cos(a) - y * math.sin(a) + dx, x * math.sin(a) + y * math.cos(a) + dy) for x, y in lid]
    prism(m, lid, 0.362, 0.42, 0, 0, 2.0)
    return m, [("Wood", "weathered_planks", (0.46, 0.38, 0.30)), ("Inside", None, (0.02, 0.018, 0.015))]


# The floor's fallen things (`UrnFallen`, `Planks`, `StandFallen`): fewer, bigger pieces that
# tell what happened here, in place of a sprinkle of chips (the developer: "make more sense, be
# rich, use more things"; dress9's reviewers: "program-sprinkled pebbles"). Each is built
# standing, as its upright kin are, then laid down by `laid`.
def laid(m, since, tilt=0.0, turn=0.0, at=(0.0, 0.0, 0.0)):
    """The vertices added to `m` since it had `since`, tipped `tilt` radians about Y (its up
    toward +X), turned `turn` about the up axis and moved to `at`."""
    rot = Matrix.Rotation(turn, 3, "Z") @ Matrix.Rotation(tilt, 3, "Y")
    m.bm.verts.ensure_lookup_table()
    for v in m.bm.verts[since:]:
        v.co = rot @ v.co + Vector(at)


def urn_fallen():
    """A cinerary urn knocked over, 0.39 m long on its belly, its mouth dipped to the floor
    toward +X, its ashes spilt from it in a grey fan and its lid rolled off beyond: a grave
    robbed or kicked over, told in one piece. 0.8 by 0.45 m. Its clay the `Urns`'."""
    m = Mesh()
    m.aged = True
    s = 0.85
    since = len(m.bm.verts)
    lathe(m, (0.0, 0.0, 0.0), [(0.0, 0.0), (0.09 * s, 0.0), (0.10 * s, 0.03 * s), (0.17 * s, 0.2 * s), (0.18 * s, 0.28 * s),
                               (0.13 * s, 0.39 * s), (0.085 * s, 0.43 * s), (0.10 * s, 0.46 * s), (0.0, 0.46 * s)], 0, STUCCO_M, 14)
    lathe(m, (0.0, 0.0, 0.461 * s), [(0.0, 0.0), (0.085 * s, 0.0), (0.0, 0.001)], 1, STUCCO_M, 12)
    laid(m, since, tilt=math.radians(97.0), at=(-0.30, 0.0, 0.15))
    for x, y, rx, ry, rz in ((0.17, 0.0, 0.10, 0.09, 0.02), (0.29, 0.05, 0.09, 0.07, 0.013), (0.27, -0.08, 0.06, 0.05, 0.01),
                             (0.40, 0.01, 0.05, 0.04, 0.008)):
        lump(m, (x, y, 0.0), (rx, ry, rz), 2, segs=10, rings=4)
    lathe(m, (0.38, 0.17, 0.0), [(0.0, 0.0), (0.11 * s, 0.0), (0.10 * s, 0.025 * s), (0.04 * s, 0.05 * s), (0.03 * s, 0.075 * s),
                                 (0.0, 0.08 * s)], 0, STUCCO_M, 12)
    # The ashes a dark charcoal grey: at 0.24 they lit to white blobs in the torchlight (dress10's
    # reviewers), paler than the floor they lie on.
    return m, [("Clay", "white_stucco", (0.30, 0.17, 0.10)), ("Inside", None, (0.05, 0.045, 0.04)),
               ("Ashes", "white_stucco", (0.065, 0.062, 0.058))]


def planks():
    """A coffin's lid broken up and heaped: three boards, 1.1, 0.8 and 0.65 m by 0.26 m and
    4.5 cm thick, each snapped at one end in a jagged break, two lying side by side and the
    third thrown across the long one, one end on the floor. Grimed, so they lie in the dust. Their
    wood a step darker than the `Coffin`'s, longer on the floor. Two thin boards lying apart
    were a sliver from a room's width off (dress10's reviewers); heaped, they stand 0.1 m."""
    m = Mesh()
    m.aged = True
    t = 0.045
    long_ = [(-0.58, -0.13), (0.42, -0.13), (0.48, -0.07), (0.44, -0.02), (0.55, 0.03), (0.49, 0.08), (0.52, 0.13), (-0.58, 0.13)]
    prism(m, long_, 0.0, t, 0, 0, 2.0)
    since = len(m.bm.verts)
    mid = [(-0.40, -0.13), (0.30, -0.13), (0.36, -0.05), (0.31, 0.01), (0.40, 0.13), (-0.40, 0.13)]
    prism(m, mid, 0.0, t, 0, 0, 2.0)
    laid(m, since, turn=0.18, at=(-0.02, 0.35, 0.0))
    since = len(m.bm.verts)
    short = [(-0.32, -0.12), (0.24, -0.12), (0.30, -0.06), (0.26, -0.01), (0.33, 0.06), (0.28, 0.12), (-0.32, 0.12)]
    prism(m, short, 0.0, t, 0, 0, 2.0)
    # Thrown across the long board, levered on its edge (y -0.13): its sawn end up over the
    # board, its broken end down on the floor at -Y.
    laid(m, since, tilt=0.12, turn=-1.35, at=(0.15, -0.20, 0.0375))
    return m, [("Wood", "weathered_planks", (0.40, 0.33, 0.26))]


def stand_fallen():
    """A wrought-iron pricket candle stand, 0.73 m, toppled: a thin shaft with a knop, three
    splayed legs (one up in the air, two on the floor) and a drip pan with its spike, lying on
    the pan's rim and two legs' feet, its candle rolled off and out beside it. 0.75 by 0.4 m,
    its foot at -X. The wooden stand of 1.08 m, a broad foot disc and a pan on a thick shaft,
    read as "an oversized plunger or ladle" (dress10's reviewers)."""
    m = Mesh()
    m.aged = True
    since = len(m.bm.verts)
    for turn in (60.0, 180.0, 300.0):
        leg = len(m.bm.verts)
        m.box((0.0, -0.009, -0.007), (0.18, 0.009, 0.007), 0, 1.0)
        laid(m, leg, tilt=math.radians(38.0), turn=math.radians(turn), at=(0.0, 0.0, 0.11))
    lathe(m, (0.0, 0.0, 0.09), [(0.0, 0.0), (0.022, 0.0), (0.022, 0.04), (0.0, 0.04)], 0, 1.0, 10)
    lathe(m, (0.0, 0.0, 0.13), [(0.0, 0.0), (0.011, 0.0), (0.011, 0.25), (0.019, 0.265), (0.019, 0.285), (0.011, 0.30),
                                (0.011, 0.52), (0.0, 0.52)], 0, 1.0, 8)
    lathe(m, (0.0, 0.0, 0.65), [(0.0, 0.0), (0.012, 0.0), (0.075, 0.012), (0.078, 0.03), (0.070, 0.03), (0.0, 0.018)], 0, 1.0, 14)
    lathe(m, (0.0, 0.0, 0.668), [(0.0, 0.0), (0.006, 0.0), (0.0, 0.06)], 0, 1.0, 6)
    laid(m, since, tilt=math.radians(90.0), at=(-0.36, 0.0, 0.078))
    since = len(m.bm.verts)
    lathe(m, (0.0, 0.0, 0.0), [(0.0, 0.0), (0.022, 0.0), (0.022, 0.14), (0.017, 0.152), (0.0, 0.152)], 1, STUCCO_M, 10)
    laid(m, since, tilt=math.radians(90.0), turn=0.6, at=(0.30, 0.20, 0.022))
    return m, [("Iron", None, (0.085, 0.065, 0.05)), WAX]


def export(name, built, out=None):
    out = out or OUT
    m, mats = built
    bmesh.ops.remove_doubles(m.bm, verts=m.bm.verts[:], dist=1e-5)
    bmesh.ops.triangulate(m.bm, faces=m.bm.faces[:])
    me = bpy.data.meshes.new(name)
    m.bm.to_mesh(me)
    if m.aged:
        grime(m.bm, me)
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
    os.makedirs(out, exist_ok=True)
    bpy.ops.export_scene.gltf(
        filepath=os.path.join(out, f"{name}.glb"),
        export_format="GLB",
        use_selection=True,
        export_tangents=True,
        # The aged pieces' grime, their active colour attribute, as COLOR_0.
        export_vertex_color="ACTIVE" if m.aged else "NONE",
        export_image_format="JPEG",
        export_jpeg_quality=85,
    )


def build(pieces, out=None):
    """Exports each (name, make) of `pieces`, or only those named after `--`, to `out`."""
    only = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    for name, make in pieces:
        if only and name not in only:
            continue
        reset()
        export(name, make(), out)


# Run, not imported: the mine's kit (mine.py) builds from these helpers.
if __name__ == "__main__":
    build((("BarrelVault", barrel_vault), ("GroinVault", groin_vault), ("DoorArch", door_arch), ("Pier", pier), ("Loculus", loculus), ("Shroud", shroud), ("Sarcophagus", sarcophagus), ("Candles", candles), ("Altar", altar), ("Urns", urns), ("Coffin", coffin), ("Debris", debris), ("UrnFallen", urn_fallen), ("Planks", planks), ("StandFallen", stand_fallen)))
