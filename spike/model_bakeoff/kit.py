"""Buildings and street furniture written as code, from parts, at real size (D108, D110).

Every piece is made of named palette roles, so a realm can recolour it. Pieces are joined
into one object per building; its front faces -Y before it is turned and placed.
"""

import math
import random

import bmesh
import bpy
from mathutils import Euler, Matrix, Vector

# Roles the kit uses, with default colours (sRGB); look.PALETTE gains them on import.
ROLES = {
    "Stone": (0.62, 0.60, 0.56), "StoneDark": (0.44, 0.42, 0.42), "Plaster": (0.90, 0.85, 0.74),
    "PlasterOchre": (0.88, 0.72, 0.46), "PlasterRose": (0.88, 0.70, 0.64),
    "Timber": (0.30, 0.21, 0.15), "RoofTile": (0.64, 0.32, 0.22), "Slate": (0.30, 0.32, 0.38),
    "Cobble": (0.56, 0.52, 0.48), "Concrete": (0.60, 0.60, 0.62), "ConcreteDark": (0.32, 0.33, 0.37),
    "Glass": (0.18, 0.26, 0.36), "Iron": (0.15, 0.15, 0.18), "Lamp": (1.0, 0.78, 0.45),
    "Neon": (1.0, 0.25, 0.60), "NeonCyan": (0.20, 0.90, 1.0), "Water": (0.28, 0.50, 0.62),
}
# Roles that give off light, and how strongly.
GLOW = {"Lamp": 6.0, "Neon": 9.0, "NeonCyan": 9.0, "Glow": 6.0}


class Builder:
    """Collects boxes, prisms and columns, each with a role, then joins them."""

    def __init__(self, name):
        self.name = name
        self.parts = []

    def _add(self, bm, role, at, rot):
        me = bpy.data.meshes.new(f"{self.name}-{role}")
        bm.to_mesh(me)
        bm.free()
        me.materials.append(bpy.data.materials.get(role) or bpy.data.materials.new(role))
        ob = bpy.data.objects.new(me.name, me)
        bpy.context.scene.collection.objects.link(ob)
        ob.matrix_world = Matrix.LocRotScale(Vector(at), Euler([math.radians(r) for r in rot]), Vector((1, 1, 1)))
        self.parts.append(ob)
        return ob

    def box(self, role, size, at, rot=(0, 0, 0)):
        """A box `size` across, its base centred on `at`."""
        bm = bmesh.new()
        bmesh.ops.create_cube(bm, size=1.0)
        for v in bm.verts:
            v.co = Vector((v.co.x * size[0], v.co.y * size[1], (v.co.z + 0.5) * size[2]))
        return self._add(bm, role, at, rot)

    def column(self, role, radius, height, at, rot=(0, 0, 0), sides=8, top=None):
        bm = bmesh.new()
        top = radius if top is None else top
        rings = []
        for z, r in ((0, radius), (height, top)):
            rings.append([bm.verts.new((math.cos(i / sides * math.tau) * r, math.sin(i / sides * math.tau) * r, z)) for i in range(sides)])
        for i in range(sides):
            k = (i + 1) % sides
            bm.faces.new((rings[0][i], rings[0][k], rings[1][k], rings[1][i]))
        bm.faces.new(rings[1])
        bm.faces.new(rings[0][::-1])
        return self._add(bm, role, at, rot)

    def gable(self, role, width, depth, rise, at, overhang=0.5, along_y=False):
        """A pitched roof: a triangular prism, ridge along X (or Y), eaves overhanging."""
        if along_y:
            ob = self.gable(role, depth, width, rise, (0, 0, 0), overhang)
            ob.matrix_world = Matrix.LocRotScale(Vector(at), Euler((0, 0, math.pi / 2)), Vector((1, 1, 1)))
            return ob
        w, d = width / 2 + overhang, depth / 2 + overhang
        bm = bmesh.new()
        t = 0.25
        a = [bm.verts.new(p) for p in ((-w, -d, 0), (w, -d, 0), (w, 0, rise), (-w, 0, rise))]
        b = [bm.verts.new(p) for p in ((-w, d, 0), (w, d, 0), (w, 0, rise), (-w, 0, rise))]
        a2 = [bm.verts.new((v.co.x, v.co.y, v.co.z + t)) for v in a]
        b2 = [bm.verts.new((v.co.x, v.co.y, v.co.z + t)) for v in b]
        bm.faces.new((a2[0], a2[1], a2[2], a2[3]))
        bm.faces.new((b2[3], b2[2], b2[1], b2[0]))
        bm.faces.new((a[3], a[2], a[1], a[0]))
        bm.faces.new((b[0], b[1], b[2], b[3]))
        for q in ((a[0], a[1], a2[1], a2[0]), (b[1], b[0], b2[0], b2[1]), (a[1], a[2], a2[2], a2[1]),
                  (a[3], a[0], a2[0], a2[3]), (b[2], b[1], b2[1], b2[2]), (b[0], b[3], b2[3], b2[0])):
            bm.faces.new(q)
        bmesh.ops.remove_doubles(bm, verts=bm.verts[:], dist=1e-4)
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces[:])
        return self._add(bm, role, at, (0, 0, 0))

    def gable_end(self, role, width, rise, at, thickness=0.3, facing_y=False):
        """The triangle of wall under a gable's end."""
        bm = bmesh.new()
        w = width / 2
        f = [bm.verts.new(p) for p in ((-w, -thickness / 2, 0), (w, -thickness / 2, 0), (0, -thickness / 2, rise))]
        g = [bm.verts.new((v.co.x, thickness / 2, v.co.z)) for v in f]
        bm.faces.new(f)
        bm.faces.new(g[::-1])
        for i in range(3):
            k = (i + 1) % 3
            bm.faces.new((f[i], g[i], g[k], f[k]))
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces[:])
        return self._add(bm, role, at, (0, 0, 0 if facing_y else 90))

    def finish(self, at, turn=0.0):
        ctx = bpy.context
        for o in ctx.scene.objects:
            o.select_set(False)
        for o in self.parts:
            o.select_set(True)
        ctx.view_layer.objects.active = self.parts[0]
        bpy.ops.object.join()
        ob = ctx.view_layer.objects.active
        ob.name = self.name
        ob.data.transform(ob.matrix_world)
        ob.matrix_world = Matrix.LocRotScale(Vector(at), Euler((0, 0, turn)), Vector((1, 1, 1)))
        return ob


def medieval_house(name, at, turn, width, depth, floors, rng):
    """A timber-framed town house, its front to -Y: a stone ground floor with a door and a
    shop window, plaster upper floors jettied out over the street and framed in dark beams,
    shuttered windows, a steep roof of tile or slate (sometimes its gable to the street),
    and a chimney."""
    b = Builder(name)
    plaster = rng.choice(["Plaster", "Plaster", "PlasterOchre", "PlasterRose"])
    roof = rng.choice(["RoofTile", "RoofTile", "Slate"])
    gable_front = rng.random() < 0.45
    ground = 3.0
    front = -depth / 2
    b.box("Stone", (width, depth, ground), (0, 0, 0))
    door = rng.uniform(-width / 4, width / 4)
    b.box("Timber", (1.2, 0.3, 2.3), (door, front - 0.05, 0))
    b.box("StoneDark", (1.6, 0.25, 0.35), (door, front - 0.05, 2.3))
    shop = door + (2.0 if door < 0 else -2.0)
    b.box("Glass", (1.5, 0.2, 1.2), (shop, front - 0.03, 0.9))
    b.box("Timber", (1.9, 0.6, 0.12), (shop, front - 0.35, 2.2))
    z, f_front = ground, front
    bays = max(2, int(width / 1.8))
    for f in range(floors):
        h = 2.7
        f_front -= 0.35
        d = depth / 2 - f_front
        b.box(plaster, (width, d, h), (0, (depth / 2 + f_front) / 2, z))
        y = f_front - 0.06
        for zz in (z, z + h - 0.2):
            b.box("Timber", (width + 0.1, 0.2, 0.22), (0, y, zz))
        for i in range(bays + 1):
            x = -width / 2 + width * i / bays
            b.box("Timber", (0.22, 0.2, h), (x, y, z))
        for i in range(bays):
            x0 = -width / 2 + width * (i + 0.5) / bays
            corner = i in (0, bays - 1) and bays > 2
            if corner:
                ang = math.degrees(math.atan2(h, width / bays)) * (1 if i == 0 else -1)
                b.box("Timber", (math.hypot(width / bays, h) * 0.9, 0.18, 0.2), (x0, y, z + h / 2 - 0.1), (0, ang, 0))
            else:
                b.box("Glass", (0.8, 0.22, 1.1), (x0, y + 0.02, z + 0.75))
                b.box("Timber", (1.0, 0.3, 0.1), (x0, y - 0.05, z + 0.7))
                for sgn in (-1, 1):
                    b.box("Timber", (0.4, 0.12, 1.15), (x0 + sgn * 0.62, y - 0.04, z + 0.72), (0, 0, sgn * 20))
        z += h
    total_d = depth / 2 - f_front
    mid_y = (depth / 2 + f_front) / 2
    if gable_front:
        rise = total_d * 0.0 + width * 0.85
        b.gable_end(plaster, width, rise, (0, f_front + 0.15, z), facing_y=True)
        b.box("Timber", (0.2, 0.2, rise * 0.9), (0, f_front - 0.02, z))
        b.gable(roof, width, total_d, rise, (0, mid_y, z - 0.1), overhang=0.5, along_y=True)
    else:
        rise = total_d * 0.8
        for sgn in (-1, 1):
            b.gable_end(plaster, total_d, rise, (sgn * (width / 2 - 0.15), mid_y, z))
        b.gable(roof, width, total_d, rise, (0, mid_y, z - 0.1), overhang=0.5)
    b.box("Stone", (0.8, 0.8, rise + 1.4), (width / 3 * rng.choice((-1, 1)), mid_y + total_d / 4, z))
    return b.finish(at, turn)


def tech_block(name, at, turn, width, depth, floors, rng):
    """A modular concrete block: floor bands, glass strips with lit windows, balconies,
    air-conditioning units, pipes, neon signs and roof clutter."""
    b = Builder(name)
    fh = 3.2
    h = floors * fh
    b.box("Concrete", (width, depth, h), (0, 0, 0))
    front = -depth / 2
    for f in range(floors):
        z = f * fh
        b.box("ConcreteDark", (width + 0.3, depth + 0.3, 0.35), (0, 0, z + fh - 0.35))
        if f == 0:
            b.box("Lamp", (width * 0.7, 0.1, 2.3), (0, front - 0.05, 0.2))
            b.box("ConcreteDark", (width * 0.8, 1.6, 0.15), (0, front - 0.8, 2.7))
            b.box(rng.choice(["Neon", "NeonCyan"]), (width * 0.8, 0.12, 0.12), (0, front - 1.6, 2.7))
            continue
        b.box("Glass", (width * 0.86, 0.1, 1.5), (0, front - 0.05, z + 0.9))
        for i in range(int(width / 1.6)):
            if rng.random() < 0.35:
                x = -width * 0.43 + 1.6 * i + 0.7
                b.box(rng.choice(["Lamp", "Lamp", "NeonCyan"]), (1.2, 0.12, 1.3), (x, front - 0.07, z + 1.0))
        if rng.random() < 0.35:
            x = rng.uniform(-width / 3, width / 3)
            b.box("ConcreteDark", (2.6, 1.2, 0.2), (x, front - 0.6, z))
            b.box("Iron", (2.6, 0.06, 1.0), (x, front - 1.2, z + 0.2))
        if rng.random() < 0.5:
            b.box("Concrete", (0.9, 0.6, 0.6), (rng.uniform(-width / 2.4, width / 2.4), front - 0.3, z + 2.3))
    for k in range(2):
        x = (-1) ** k * (width / 2 - 0.4)
        b.column("Iron", 0.12, h, (x, front - 0.25, 0))
    for _ in range(rng.randint(1, 3)):
        x = rng.uniform(-width / 2 + 0.6, width / 2 - 0.6)
        z = rng.uniform(4, h - 4)
        b.box(rng.choice(["Neon", "NeonCyan"]), (0.7, 0.25, rng.uniform(2.5, 5.0)), (x, front - 0.5, z))
    b.column("ConcreteDark", 1.2, 2.2, (width / 4, 0, h))
    b.column("Iron", 0.06, 6.0, (-width / 4, depth / 4, h), sides=6)
    b.box("ConcreteDark", (2.2, 1.6, 1.4), (-width / 5, -depth / 5, h))
    return b.finish(at, turn)


def chapel_ruin(name, at, turn, rng):
    """A ruined stone chapel: piers and broken walls, pointed-arch openings, a gable front
    with a tall pointed door, a stub of tower and a few charred rafters."""
    b = Builder(name)
    length, width, height = 18.0, 9.0, 8.0
    for side in (-1, 1):
        x = side * width / 2
        for i in range(7):
            y = -length / 2 + i * length / 6
            top = height if rng.random() > 0.3 else rng.uniform(3.5, height - 1)
            b.box("Stone", (1.0, 1.3, top), (x, y, 0))
            if i < 6:
                ym = y + length / 12
                b.box("Stone", (0.8, length / 6 - 1.3, 1.2), (x, ym, 0))
                if top > 6.5 and rng.random() > 0.3:
                    span = length / 6 - 1.3
                    for s in (-1, 1):
                        b.box("Stone", (0.7, span * 0.62, 0.6), (x, ym + s * span * 0.24, 6.0),
                              (math.degrees(math.atan2(1.3, span / 2)) * -s, 0, 0))
    fy = -length / 2
    for s in (-1, 1):
        b.box("Stone", (width / 2 - 1.0, 1.1, height), (s * (width / 4 + 0.5), fy, 0))
        b.box("Stone", (1.6, 1.1, 2.4), (s * 0.9, fy, height - 0.1), (0, s * -35, 0))
    b.gable_end("Stone", width + 1, 4.5, (0, fy, height), thickness=1.1, facing_y=True)
    b.box("Timber", (1.6, 0.3, 4.2), (0, fy - 0.2, 0))
    ty = length / 2 + 2
    b.box("StoneDark", (4.0, 4.0, 13.0), (0, ty, 0))
    b.box("Stone", (4.4, 4.4, 0.5), (0, ty, 13.0))
    for i in range(5):
        y = -length / 2 + 2 + i * 3.2
        if rng.random() < 0.6:
            b.box("Timber", (width + 0.6, 0.3, 0.3), (0, y, height - 0.2), (0, rng.uniform(-14, 14), 0))
    return b.finish(at, turn)


def iron_fence(name, start, end, height=1.6):
    """Wrought-iron railings with spear tips between two points."""
    b = Builder(name)
    a, z = Vector(start), Vector(end)
    run = (z - a).length
    n = int(run / 0.18)
    for i in range(n + 1):
        p = Vector((0, -run / 2 + run * i / n, 0))
        b.column("Iron", 0.025, height, p, sides=4, top=0.025)
        b.column("Iron", 0.05, 0.14, p + Vector((0, 0, height)), sides=4, top=0.0)
    for zz in (0.25, height - 0.25):
        b.box("Iron", (0.06, run, 0.06), (0, 0, zz))
    mid = (a + z) / 2
    turn = math.atan2(z.y - a.y, z.x - a.x) - math.pi / 2
    return b.finish(mid, turn)


def cable(name, a, b, sag=1.2, radius=0.03):
    """A cable hanging between two points."""
    bld = Builder(name)
    a, b = Vector(a), Vector(b)
    pts = [a.lerp(b, t) - Vector((0, 0, sag * 4 * t * (1 - t))) for t in (i / 8 for i in range(9))]
    for p, q in zip(pts, pts[1:]):
        d = q - p
        rot = d.to_track_quat("Z", "Y").to_euler()
        bld.column("Iron", radius, d.length, (0, 0, 0), sides=4)
        bld.parts[-1].matrix_world = Matrix.LocRotScale(p, rot, Vector((1, 1, 1)))
    return bld.finish((0, 0, 0))
