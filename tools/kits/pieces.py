"""A kit's dressed pieces made in Blender from their source (D215; tools/kits/README.md).

    tools/blender/run.sh tools/kits/pieces.py art/kits/stone/pieces.json assets/data/kits/pieces/stone.ron

Reads the source, a JSON file under art/kits/<kit>/: its modules (D258), lengths of mesh
swept from a section along +X and shaped as they go, each at a near and a far budget with
a cap for where its line stops; its profiles, each a polyline in
(out, up) metres from the edge it is swept along, the corners named rounded by Blender's
vertex bevel; and its corner stones, each a block at an edge's corner, standing out of both
faces, its two faces divided, pillowed and pitched as a mason dresses them, its arrises
bevelled, shaded smooth across curves and hard at creases with face-weighted normals.

Writes the game's file, RON as `terra_build::dress::Pieces` reads it: the profiles as points,
the corner stones as indexed triangles with their normals. Game space: Y up, a stone's
corner at the origin, its faces looking +X and +Z, the wall behind it in -X and -Z, from
y = 0 up a course. Nothing else is written; the file is committed with the kit, as the
props' models are.
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

args = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
if len(args) != 2:
    raise SystemExit("usage: tools/blender/run.sh tools/kits/pieces.py <source.json> <out.ron>")
SOURCE, OUT = args


def clear():
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob)
    for mesh in list(bpy.data.meshes):
        bpy.data.meshes.remove(mesh)


def profile(spec):
    """A polyline's corners named in `round` rounded by Blender's vertex bevel; its points
    in order from the first."""
    points = spec["points"]
    bm = bmesh.new()
    verts = [bm.verts.new((o, u, 0.0)) for o, u in points]
    for a, b in zip(verts, verts[1:]):
        bm.edges.new((a, b))
    bm.verts.ensure_lookup_table()
    bmesh.ops.bevel(
        bm,
        geom=[verts[i] for i in spec.get("round", [])],
        offset=spec.get("radius", 0.01),
        segments=2,
        affect="VERTICES",
        profile=0.5,
        clamp_overlap=True,
    )
    ends = [v for v in bm.verts if len(v.link_edges) == 1]
    first = Vector((points[0][0], points[0][1], 0.0))
    start = min(ends, key=lambda v: (v.co - first).length)
    out, prev, here = [], None, start
    while here is not None:
        out.append((round(here.co.x, 4), round(here.co.y, 4)))
        nxt = None
        for e in here.link_edges:
            other = e.other_vert(here)
            if other is not prev:
                nxt = other
        prev, here = here, nxt
    bm.free()
    return out


def stone(name, spec, along_a, along_b, seed):
    """A corner stone: in game space (x, y up, z), from x = -along_a to proud along its +Z
    face, z = -along_b to proud along its +X face, a course high less its joints. Built in
    Blender's frame, Z up: game (x, y, z) is Blender (x, -z, y)."""
    rng = random.Random(seed)
    proud = spec["proud"]
    joint = spec["joint"]
    height = spec["height"] - 2 * joint
    bevel = spec["bevel"]
    cell = spec["cell"]
    # The box in Blender's frame: x from -a to proud, y (game -z) from -proud to b.
    x0, x1 = -along_a + joint, proud
    y0, y1 = -proud, along_b - joint
    z0, z1 = joint, joint + height
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    bmesh.ops.scale(bm, vec=(x1 - x0, y1 - y0, z1 - z0), verts=bm.verts)
    bmesh.ops.translate(bm, vec=((x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2), verts=bm.verts)
    # Its back, in the wall, is never seen.
    back = [f for f in bm.faces if f.normal.x < -0.99 or f.normal.y > 0.99]
    bmesh.ops.delete(bm, geom=back, context="FACES")
    # Its two faces divided, so they can be pillowed and pitched.
    faces = [f for f in bm.faces if f.normal.x > 0.99 or f.normal.y < -0.99]
    longest = max(x1 - x0, y1 - y0, z1 - z0)
    cuts = max(1, int(round(longest / cell)) - 1)
    bmesh.ops.subdivide_edges(
        bm, edges=list({e for f in faces for e in f.edges}), cuts=cuts, use_grid_fill=True
    )
    # Its arrises round its faces bevelled; not where it is cut open at the back.
    edges = []
    for e in bm.edges:
        if len(e.link_faces) != 2:
            continue
        a, b = e.link_faces
        if a.normal.dot(b.normal) > 0.5:
            continue
        edges.append(e)
    bmesh.ops.bevel(
        bm, geom=edges, offset=bevel, segments=2, affect="EDGES", profile=0.6, clamp_overlap=True
    )
    # Pillowed towards each face's middle and pitched by the chisel, never on the bevels.
    for v in bm.verts:
        on_x = v.co.x > proud - 1e-4
        on_y = v.co.y < -proud + 1e-4
        if not (on_x or on_y):
            continue
        if on_x:
            dy, size_a = min(v.co.y - y0, y1 - v.co.y), y1 - y0
        else:
            dy, size_a = min(v.co.x - x0, x1 - v.co.x), x1 - x0
        dz = min(v.co.z - z0, z1 - v.co.z)
        inner = min(dy, dz)
        if inner < bevel * 0.9:
            continue
        pillow = math.sin(min(1.0, dy / (0.5 * size_a)) * math.pi / 2) * math.sin(
            min(1.0, dz / (0.5 * height)) * math.pi / 2
        )
        at = Vector((v.co.x * 11.0 + seed * 0.37, v.co.y * 11.0, v.co.z * 11.0 + seed * 0.71))
        n = noise.noise(at) + 0.5 * noise.noise(at * 2.3 + Vector((3.0, 1.0, 2.0)))
        push = spec["pillow"] * pillow + spec["pitch"] * n * min(1.0, inner / 0.02)
        if on_x:
            v.co.x += push
        else:
            v.co.y -= push
    # A little out of true, as set by hand.
    tilt = rng.uniform(-0.006, 0.006)
    for v in bm.verts:
        if v.co.x > proud - 0.03:
            v.co.x += tilt * (v.co.z - (z0 + z1) / 2)
    bmesh.ops.triangulate(bm, faces=bm.faces)
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    ob = bpy.data.objects.new(name, mesh)
    bpy.context.collection.objects.link(ob)
    bpy.context.view_layer.objects.active = ob
    ob.select_set(True)
    # Hard at creases over about 50 degrees, smooth across curves (docs/art-style.md); the
    # big faces keep their own normals and the bevels take the turn, as hard-surface game
    # art is weighted.
    bpy.ops.object.shade_smooth_by_angle(angle=math.radians(50))
    weighted = ob.modifiers.new("Weighted", "WEIGHTED_NORMAL")
    weighted.mode = "FACE_AREA"
    weighted.weight = 100
    weighted.keep_sharp = True
    ob.select_set(False)
    return ob


def triangles(ob):
    """An object's triangles as shared corners in game space: positions, normals, indices."""
    depsgraph = bpy.context.evaluated_depsgraph_get()
    evaluated = ob.evaluated_get(depsgraph)
    mesh = evaluated.to_mesh()
    mesh.calc_loop_triangles()
    normals = mesh.corner_normals
    index, positions, out_normals, indices = {}, [], [], []
    for tri in mesh.loop_triangles:
        for loop in tri.loops:
            p = mesh.vertices[mesh.loops[loop].vertex_index].co
            n = normals[loop].vector
            game_p = (round(p.x, 4), round(p.z, 4), round(-p.y, 4))
            game_n = (round(n.x, 3), round(n.z, 3), round(-n.y, 3))
            key = (game_p, game_n)
            if key not in index:
                index[key] = len(positions)
                positions.append(game_p)
                out_normals.append(game_n)
            indices.append(index[key])
    evaluated.to_mesh_clear()
    return positions, out_normals, indices


def ron_floats(values):
    return "(" + ", ".join(f"{v:g}" for v in values) + ")"


def ron_mesh(positions, normals, indices):
    tris = [indices[i : i + 3] for i in range(0, len(indices), 3)]
    return (
        "(\n                positions: ["
        + ", ".join(ron_floats(p) for p in positions)
        + "],\n                normals: ["
        + ", ".join(ron_floats(n) for n in normals)
        + "],\n                triangles: ["
        + ", ".join("(%d, %d, %d)" % tuple(t) for t in tris)
        + "],\n            )"
    )


# Modules (D258): lengths of mesh along +X, modelled here from their numbers, each a set of
# rings of a section swept along it and shaped as it goes: an adze's scallops, a bundle
# pinched where it is tied, a ridge's pointed edge. Built in game space (x along, y up,
# z across) and turned into Blender's frame, Z up: game (x, y, z) is Blender (x, -z, y).


def fade(x, length, reach=0.05):
    """1 inside a module, falling to 0 at its two ends, so copies laid end to end meet in
    one section and read as one member."""
    return min(1.0, x / reach, (length - x) / reach) if reach > 0 else 1.0


def lumps(x, y, z, seed, size=0.12):
    """Smooth noise from -1 to 1, about `size` metres across."""
    at = Vector((x / size + seed * 0.31, y / size, z / size + seed * 0.17))
    return noise.noise(at)


def rounded_rect(y0, y1, z0, z1, r, per_corner=3):
    """A rectangle's corners rounded by r, as points (y, z) round it."""
    r = max(0.001, min(r, (y1 - y0) / 2.01, (z1 - z0) / 2.01))
    centres = [(y1 - r, z1 - r, 0.0), (y1 - r, z0 + r, 90.0), (y0 + r, z0 + r, 180.0), (y0 + r, z1 - r, 270.0)]
    out = []
    for cy, cz, start in centres:
        for k in range(per_corner):
            a = math.radians(start + 90.0 * k / (per_corner - 1))
            out.append((cy + r * math.cos(a), cz + r * math.sin(a)))
    return out


def ring_mesh(name, rings, closed=True, caps=(False, False), smooth=40.0):
    """A mesh of rings, each (x, [(y, z), ...]) with as many points, joined in order; a
    closed section joins its last point to its first; `caps` fills its first and last ring."""
    bm = bmesh.new()
    vs = [[bm.verts.new((x, -z, y)) for (y, z) in points] for x, points in rings]
    n = len(rings[0][1])
    for a, b in zip(vs, vs[1:]):
        for i in range(n if closed else n - 1):
            j = (i + 1) % n
            bm.faces.new((a[i], a[j], b[j], b[i]))
    if caps[0]:
        bm.faces.new(vs[0])
    if caps[1]:
        bm.faces.new(vs[-1])
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    # Outward, from the middle of its section where it is: an open tube's faces, which
    # Blender cannot tell inside from out, turned round if they look in.
    bm.normal_update()
    middles = {round(x, 5): Vector((x, -sum(p[1] for p in pts) / len(pts), sum(p[0] for p in pts) / len(pts))) for x, pts in rings}
    keys = sorted(middles)
    out = 0.0
    for f in bm.faces:
        c = f.calc_center_median()
        k = min(keys, key=lambda x: abs(x - c.x))
        middle = middles[k].copy()
        middle.x = c.x
        out += f.normal.dot(c - middle) * f.calc_area()
    if out < 0:
        bmesh.ops.reverse_faces(bm, faces=bm.faces)
    return finish(name, bm, smooth, recalc=False)


def finish(name, bm, smooth, recalc=True):
    if recalc:
        bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bmesh.ops.triangulate(bm, faces=bm.faces)
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    ob = bpy.data.objects.new(name, mesh)
    bpy.context.collection.objects.link(ob)
    bpy.context.view_layer.objects.active = ob
    ob.select_set(True)
    bpy.ops.object.shade_smooth_by_angle(angle=math.radians(smooth))
    ob.select_set(False)
    return ob


def join(name, obs):
    """Objects made one."""
    for ob in bpy.context.view_layer.objects:
        ob.select_set(False)
    for ob in obs:
        ob.select_set(True)
    bpy.context.view_layer.objects.active = obs[0]
    bpy.ops.object.join()
    obs[0].name = name
    obs[0].select_set(False)
    return obs[0]


def xs(length, step):
    count = max(1, int(round(length / step)))
    return [length * i / count for i in range(count + 1)]


def section(spec, x, seed, length, end=False):
    """A module's section at a point along it, as points (y, z) round it, and whether it is
    closed. `end`, the plain section its copies meet in."""
    kind = spec["kind"]
    f = 0.0 if end else fade(x, length, spec.get("fade", 0.05))
    if kind == "timber":
        # Squared timber: its corners rounded where the axe met the tree's round and wandering
        # along it; its faces scalloped by the adze in shallow dishes.
        y0, y1 = spec["y"]
        z0, z1 = spec["z"]
        r = spec["round"] * (1.0 + 0.5 * f * lumps(x, 0.0, 1.0, seed, 0.3))
        points = rounded_rect(y0, y1, z0, z1, r, spec.get("per_corner", 3))
        adze, period = spec.get("adze", 0.0), spec.get("period", 0.2)
        out = []
        cy, cz = (y0 + y1) / 2, (z0 + z1) / 2
        for i, (y, z) in enumerate(points):
            dy, dz = y - cy, z - cz
            # In along the face it is on, by the scallop there, each face its own phase.
            face = 0 if abs(dy) / (y1 - y0) > abs(dz) / (z1 - z0) else 1
            phase = (seed * 0.37 + face * 0.5 + (1 if (dy if face == 0 else dz) > 0 else 0) * 0.25) % 1.0
            scallop = 0.5 - 0.5 * math.cos(2 * math.pi * (x / period + phase))
            dig = adze * f * (scallop + 0.4 * lumps(x, y, z, seed, 0.15))
            if face == 0:
                y -= math.copysign(dig, dy)
            else:
                z -= math.copysign(dig, dz)
            out.append((y, z))
        return out, True
    if kind == "pole":
        # A round pole in its bark, a little lumpy.
        r, cy, cz = spec["radius"], spec.get("y", 0.0), spec.get("z", 0.0)
        n = spec.get("sides", 10)
        out = []
        for i in range(n):
            a = 2 * math.pi * i / n
            k = r * (1.0 + spec.get("lumps", 0.06) * f * lumps(x, math.cos(a), math.sin(a), seed, 0.25))
            out.append((cy + k * math.cos(a), cz + k * math.sin(a)))
        return out, True
    if kind == "log":
        # A log in its bark (D262): round, a little uneven and wandering, lumpy where its
        # bark is thick, scribed flatter top and bottom where it bears on its neighbours.
        r = spec["radius"]
        n = spec.get("sides", 18)
        flat = spec.get("flat", 0.0) * r
        out = []
        for i in range(n):
            a = 2 * math.pi * i / n
            wave = spec.get("uneven", 0.03) * f * (
                math.sin(2 * a + seed * 0.7 + 1.3 * x) + 0.6 * math.sin(3 * a + seed * 1.3 - 0.7 * x)
            )
            k = r * (1.0 + wave + spec.get("lumps", 0.04) * f * lumps(x, math.cos(a), math.sin(a), seed, 0.25))
            y, z = k * math.cos(a), k * math.sin(a)
            if flat > 0 and abs(y) > r - flat:
                y = math.copysign(r - flat + (abs(y) - (r - flat)) * 0.35, y)
            out.append((y, z))
        return out, True
    if kind == "mound":
        # A ridge or a fillet laid over what it sits on: crowned across, its foot cut down
        # square at its edges, which may be cut in points along it; straw lumpy.
        base, h, w = spec["base"], spec["height"], spec["half"]
        p = spec.get("power", 2.2)
        point = spec.get("points", 0.0)
        period = spec.get("period", 0.25)
        tri = 1.0 - abs(2.0 * ((x / period) % 1.0) - 1.0)
        n = spec.get("across", 13)
        out = []
        sides = []
        for side in (-1, 1):
            ww = w + point * (tri - 0.5) * (0.0 if end else 1.0)
            sides.append(ww)
        top = []
        for i in range(n):
            t = -1.0 + 2.0 * i / (n - 1)
            ww = sides[0] if t < 0 else sides[1]
            z = t * ww
            crown = (1.0 - abs(t) ** p) ** (1.0 / p)
            y = base + h * crown + spec.get("lumpy", 0.0) * f * lumps(x, 0.0, z, seed, 0.18)
            top.append((y, z))
        drop = spec.get("drop", 0.03)
        out = [(base - drop, -sides[0])] + top + [(base - drop, sides[1])]
        return out, True
    if kind == "bundle":
        # A bundle bound round: an oval about its middle, pinched where it is tied.
        cy, cz = spec["y"], spec["z"]
        ry, rz = spec["radii"]
        period = spec.get("period", 0.5)
        pinch = spec.get("pinch", 0.1) * (0.5 + 0.5 * math.cos(2 * math.pi * x / period))
        n = spec.get("sides", 12)
        out = []
        for i in range(n):
            a = 2 * math.pi * i / n
            k = 1.0 - pinch + spec.get("lumpy", 0.0) * f * lumps(x, math.cos(a), math.sin(a), seed, 0.15)
            out.append((cy + ry * k * math.cos(a), cz + rz * k * math.sin(a)))
        return out, True
    raise SystemExit(f"a module of kind {kind} is not known")


def parts_of(spec):
    """A module's parts, each a section swept along it with the module's numbers."""
    base = {k: v for k, v in spec.items() if k != "parts"}
    return [{**base, **part} for part in spec.get("parts", [spec])]


def module_mesh(name, spec, seed):
    """One copy of a module, its parts joined."""
    obs = [part_mesh(f"{name}_{k}", part, seed) for k, part in enumerate(parts_of(spec))]
    return join(name, obs) if len(obs) > 1 else obs[0]


def part_mesh(name, spec, seed):
    """One part of a module's copy, from its section swept along it; a timber module open
    at its ends so copies run on as one member, others closed."""
    length = spec["length"]
    rings = [(x, section(spec, x, seed, length)[0]) for x in xs(length, spec.get("step", 0.08))]
    closed = section(spec, 0.0, seed, length)[1]
    # A gap pressed in where one tile, stone or board ends against the next.
    joint = spec.get("joint", 0.0)
    if joint > 0:
        rings = [(rings[0][0], shrink(rings[0][1], joint))] + rings + [(rings[-1][0], shrink(rings[-1][1], joint))]
        rings[1], rings[-2] = (rings[1][0] + joint, rings[1][1]), (rings[-2][0] - joint, rings[-2][1])
        caps = (True, True)
    else:
        caps = (not spec.get("open", False), not spec.get("open", False))
    parts = [ring_mesh(name, rings, closed, caps, spec.get("smooth", 40.0))]
    # Branch stubs sawn off close on a log's two faces, a few a copy, clear of its ends.
    if spec["kind"] == "log" and spec.get("stubs"):
        rng = random.Random(seed * 7919 + 13)
        lo, hi = spec["stubs"]
        r = spec["radius"]
        for k in range(rng.randint(lo, hi)):
            x = rng.uniform(0.15, max(0.16, length - 0.15))
            a = rng.choice([1, -1]) * math.pi / 2 + rng.uniform(-0.6, 0.6)
            out, up = rng.uniform(*spec["stub_length"]), rng.uniform(*spec["stub_radius"])
            # Each ring round the stub's own axis, which points out of the log at angle a; the
            # stub's section lies across it and along the log.
            stub = []
            for reach, size in ((r * 0.85, up), (r + out * 0.6, up * 0.95), (r + out, up * 0.85)):
                cy, cz = math.cos(a) * reach, math.sin(a) * reach
                pts = []
                for i in range(8):
                    b = 2 * math.pi * i / 8
                    # Round the stub: across the log (perpendicular to a) and along it.
                    pts.append((cy - math.sin(a) * size * math.cos(b), cz + math.cos(a) * size * math.cos(b), size * math.sin(b)))
                stub.append(pts)
            parts.append(stub_mesh(f"{name}_stub{k}", x, stub))
    # Rods laid along it, as a ridge's liggers are pegged over its straw.
    for k, (ry, rz) in enumerate(spec.get("rods", [])):
        rr = spec.get("rod", 0.012)
        rod = []
        for x in xs(length, 0.1):
            ring = []
            sides = spec.get("rod_sides", 5)
            for i in range(sides):
                a = 2 * math.pi * i / sides
                ring.append((ry + rr * math.cos(a), rz + rr * math.sin(a)))
            rod.append((x, ring))
        parts.append(ring_mesh(f"{name}_rod{k}", rod, True, (False, False), 60.0))
    return join(name, parts) if len(parts) > 1 else parts[0]


def stub_mesh(name, x, rings):
    """A branch stub: rings of (y, z, along) points round its axis, joined in order, its
    outer ring filled where it was sawn."""
    bm = bmesh.new()
    vs = [[bm.verts.new((x + d, -z, y)) for (y, z, d) in ring] for ring in rings]
    n = len(rings[0])
    for a, b in zip(vs, vs[1:]):
        for i in range(n):
            j = (i + 1) % n
            bm.faces.new((a[i], a[j], b[j], b[i]))
    bm.faces.new(vs[-1])
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    return finish(name, bm, 40.0, recalc=False)


def shrink(points, by):
    """A section drawn in towards its middle."""
    cy = sum(p[0] for p in points) / len(points)
    cz = sum(p[1] for p in points) / len(points)
    return [(cy + (y - cy) * (1 - by * 4), cz + (z - cz) * (1 - by * 4)) for y, z in points]


def cap_mesh(name, spec, seed):
    """What closes a module's line where it stops, its parts' caps joined."""
    if not spec.get("cap"):
        return None
    obs = [part_cap(f"{name}_{k}", part, seed) for k, part in enumerate(parts_of(spec))]
    return join(name, obs) if len(obs) > 1 else obs[0]


def part_cap(name, spec, seed):
    """A part's cap, past its +X end: its plain section filled flat, or drawn in to a
    rounded end over `reach` metres."""
    cap = spec["cap"]
    points, _ = section(spec, 0.0, seed, spec["length"], end=True)
    if cap == "flat":
        return ring_mesh(name, [(0.0, points), (0.004, points)], True, (False, True), 30.0)
    reach = spec.get("reach", 0.2)
    cy = spec.get("pivot", [None, None])
    py = cy[0] if cy[0] is not None else min(p[0] for p in points)
    pz = cy[1] if cy[1] is not None else 0.0
    rings = []
    steps = 5
    for i in range(steps + 1):
        t = i / steps
        k = math.sqrt(max(0.0, 1.0 - t * t)) if i < steps else 0.02
        rings.append((reach * t, [(py + (y - py) * k, pz + (z - pz) * k) for y, z in points]))
    return ring_mesh(name, rings, True, (False, True), 60.0)


def far_copy(ob, ratio):
    """A copy at a far budget, by Blender's decimate."""
    far = ob.copy()
    far.data = ob.data.copy()
    bpy.context.collection.objects.link(far)
    mod = far.modifiers.new("Far", "DECIMATE")
    mod.ratio = ratio
    return far


def main():
    with open(SOURCE, encoding="utf-8") as f:
        source = json.load(f)
    clear()
    lines = ["// Made by tools/kits/pieces.py from " + SOURCE.replace("\\", "/") + "; do not edit.", "("]
    lines.append("    profiles: {")
    for name, spec in source.get("profiles", {}).items():
        points = profile(spec)
        lines.append(f'        "{name}": [' + ", ".join(ron_floats(p) for p in points) + "],")
        print(f"profile {name}: {len(points)} points")
    lines.append("    },")
    lines.append("    corners: {")
    for name, spec in source.get("corners", {}).items():
        lines.append(f'        "{name}": (')
        for which in ("long", "short"):
            meshes = []
            for variant, (a, b) in enumerate(spec.get(which, [])):
                seed = zlib.crc32(f"{name}{which}{variant}".encode()) % 1000
                ob = stone(f"{name}_{which}_{variant}", spec, a, b, seed)
                positions, normals, indices = triangles(ob)
                print(f"corner {name} {which} {variant}: {len(indices) // 3} triangles")
                meshes.append(ron_mesh(positions, normals, indices))
            lines.append(f"            {which}: [" + ", ".join(meshes) + "],")
        lines.append("        ),")
    lines.append("    },")
    lines.append("    modules: {")
    for name, spec in source.get("modules", {}).items():
        near, far = [], []
        for variant in range(spec.get("variants", 1)):
            seed = zlib.crc32(f"{name}{variant}".encode()) % 1000
            ob = module_mesh(f"{name}_{variant}", spec, seed)
            made = triangles(ob)
            near.append(ron_mesh(*made))
            print(f"module {name} {variant}: {len(made[2]) // 3} triangles")
            far.append(ron_mesh(*triangles(far_copy(ob, spec.get("far", 0.35)))))
        cap = cap_mesh(f"{name}_cap", spec, 7)
        lines.append(f'        "{name}": (')
        lines.append(f"            length: {spec['length']:g},")
        lines.append(f"            blend: {spec.get('blend', 0.0):g},")
        lines.append("            near: [" + ", ".join(near) + "],")
        lines.append("            far: [" + ", ".join(far) + "],")
        lines.append("            cap: " + (f"Some({ron_mesh(*triangles(cap))})" if cap else "None") + ",")
        lines.append("        ),")
    lines.append("    },")
    lines.append(")")
    os.makedirs(os.path.dirname(os.path.abspath(OUT)), exist_ok=True)
    with open(OUT, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    print("wrote", OUT)


main()
