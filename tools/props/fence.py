"""Fence modules for a town's yards, built from the building kits' own materials rather than
generated (D346): a wattle panel, hazel woven between stakes, and a split-rail length, two
rails through hewn posts. Each is a module of the model contract (D303, docs/art-style.md):
it runs along glTF's +X from its `socket.start` to its `socket.end`, its origin the middle of
its base, so the game lays a yard's edge as whole modules end to end, each stretched the
same to meet the edge's ends, its posts at the ends shared with its neighbours'.

    tools/blender/run.sh tools/props/fence.py

Writes assets/models/props/WattleFence.glb and RailFence.glb. The wood is `log_wood`, the
wattle `wattle` (assets/materials, README there), at their real sizes, their colour and
normal maps cut to 1024 pixels, as a prop under 1.5 m wears them (D304). Nothing is random
from run to run: the small bends are fixed by each piece's place.
"""

import math
import os

import bmesh
import bpy
from mathutils import Matrix, Vector

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
MATERIALS = os.path.join(ROOT, "assets", "materials")
OUT = os.path.join(ROOT, "assets", "models", "props")
TEXTURE_PX = 1024

# log_wood is 1 m round by 2 m along; wattle 0.9 m across.
WOOD_ROUND_M, WOOD_ALONG_M = 1.0, 2.0
WATTLE_M = 0.9


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def material(name, folder):
    """A material of an assets/materials folder: colour, normal, and roughness and metal
    from its ARM map's green and blue, as glTF packs them."""
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nodes, links = mat.node_tree.nodes, mat.node_tree.links
    bsdf = nodes["Principled BSDF"]

    def image(file, colour):
        path = os.path.join(MATERIALS, folder, file)
        img = bpy.data.images.load(path)
        if not colour:
            img.colorspace_settings.name = "Non-Color"
        w, h = img.size
        if max(w, h) > TEXTURE_PX:
            img.scale(TEXTURE_PX, TEXTURE_PX * h // w)
        node = nodes.new("ShaderNodeTexImage")
        node.image = img
        return node

    colour = image("color.jpg", True)
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


def wobble(*key):
    """A fixed small number in -1..1 from a piece's place, so every run makes the same file."""
    x = 0
    for k in key:
        x = (x * 1_000_003 + int(k * 1000) + 7919) & 0xFFFFFFFF
        x ^= x >> 13
        x = (x * 0x5BD1E995) & 0xFFFFFFFF
        x ^= x >> 15
    return (x / 0xFFFFFFFF) * 2.0 - 1.0


def round_piece(bm, uv, start, end, radius, sides, material_index, flat=0.0):
    """A round length of wood from `start` to `end`, its bark wrapped round it at the wood
    material's real size, capped at both ends; `flat` presses one side in, as a rail split
    from a log is."""
    axis = (end - start).normalized()
    length = (end - start).length
    side = axis.orthogonal().normalized()
    other = axis.cross(side)
    rings = []
    for e, at in enumerate((start, end)):
        ring = []
        for s in range(sides + 1):
            a = 2 * math.pi * s / sides
            r = radius * (1.0 + 0.08 * wobble(at.x, at.y, at.z, s % sides))
            offset = side * math.cos(a) * r + other * math.sin(a) * r
            if flat and math.sin(a) < -0.3:
                offset = side * math.cos(a) * r + other * -0.3 * r
            ring.append((bm.verts.new(at + offset), a * radius / WOOD_ROUND_M, e * length / WOOD_ALONG_M))
        rings.append(ring)
    for s in range(sides):
        a0, a1 = rings[0][s], rings[0][s + 1]
        b0, b1 = rings[1][s], rings[1][s + 1]
        verts = [a0[0], a1[0], b1[0], b0[0]]
        if a1[0] is a0[0] or len({id(v) for v in verts}) < 4:
            continue
        face = bm.faces.new(verts)
        face.material_index = material_index
        for loop, (_, u, v) in zip(face.loops, (a0, a1, b1, b0)):
            loop[uv].uv = (u, v)
    # Caps: the cut ends.
    for e, ring in enumerate(rings):
        verts = [v for v, _, _ in ring[:-1]]
        if e == 0:
            verts = verts[::-1]
        face = bm.faces.new(verts)
        face.material_index = material_index
        for loop in face.loops:
            d = loop.vert.co - (start if e == 0 else end)
            loop[uv].uv = (0.5 + d.dot(side) / WOOD_ROUND_M, 0.5 + d.dot(other) / WOOD_ROUND_M)


def finish(bm, name, mats, length, dims):
    """The mesh as an object, smooth across curves and split at creases, exported as the
    game's file with its start and end sockets."""
    # Triangles, which the tangents the normal map needs are worked out on.
    bmesh.ops.triangulate(bm, faces=bm.faces[:])
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    for mat in mats:
        me.materials.append(mat)
    bpy.context.view_layer.objects.active = ob
    ob.select_set(True)
    bpy.ops.object.shade_smooth_by_angle(angle=math.radians(50.0), keep_sharp_edges=True)
    empties = []
    for socket, x in (("start", -length / 2), ("end", length / 2)):
        empty = bpy.data.objects.new(f"socket.{socket}", None)
        empty.location = (x, 0.0, 0.0)
        bpy.context.scene.collection.objects.link(empty)
        empty.select_set(True)
        empties.append(empty)
    lo = Vector([min(v.co[i] for v in me.vertices) for i in range(3)])
    hi = Vector([max(v.co[i] for v in me.vertices) for i in range(3)])
    tris = sum(len(p.vertices) - 2 for p in me.polygons)
    size = hi - lo
    print(f"FENCE {name}: {tris} triangles, {size.x:.3f} x {size.z:.3f} x {size.y:.3f} m "
          f"(dims {dims}), from {lo.x:.3f} to {hi.x:.3f}")
    bpy.ops.export_scene.gltf(
        filepath=os.path.join(OUT, f"{name}.glb"),
        export_format="GLB",
        use_selection=True,
        export_tangents=True,
        export_image_format="JPEG",
        export_jpeg_quality=85,
    )


def wattle():
    """Hazel rods woven between six stakes 0.4 m apart, the weave bending in and out round
    them, the stakes standing a little above it; 2 m between its end stakes."""
    length, height = 2.0, 1.0
    bm = bmesh.new()
    uv = bm.loops.layers.uv.new()
    stakes = 6
    for i in range(stakes):
        x = -length / 2 + length * i / (stakes - 1)
        top = height + 0.06 + 0.04 * wobble(x, 1)
        lean = Vector((0.015 * wobble(x, 2), 0.015 * wobble(x, 3), 0))
        round_piece(bm, uv, Vector((x, 0, 0)), Vector((x, 0, top)) + lean, 0.032, 7, 0)
    # The weave: a band between the end stakes, bending to either side of each stake in turn.
    steps = 40
    low, high, thick = 0.1, height - 0.04, 0.025
    grid = []
    for i in range(steps + 1):
        t = i / steps
        x = -length / 2 + length * t
        bend = 0.035 * math.sin(t * (stakes - 1) * math.pi)
        column = []
        for j, z in enumerate((low, high)):
            sag = 0.02 * wobble(i, j) if 0 < i < steps else 0.0
            for k, y in enumerate((-thick, thick)):
                column.append(bm.verts.new((x, y + bend, z + sag)))
        grid.append((column, x))
    faces = []
    for i in range(steps):
        (a, xa), (b, xb) = grid[i], grid[i + 1]
        # Front (-y), back (+y), top and bottom, each a quad between two columns.
        for (p, q), face_uv in (
            ((0, 2), "front"),
            ((3, 1), "back"),
            ((2, 3), "top"),
            ((1, 0), "bottom"),
        ):
            verts = [a[p], b[p], b[q], a[q]]
            face = bm.faces.new(verts)
            face.material_index = 1
            for loop in face.loops:
                co = loop.vert.co
                loop[uv].uv = (co.x / WATTLE_M, (co.z if face_uv in ("front", "back") else co.y * 4) / WATTLE_M)
            faces.append(face)
    for (column, x), flip in ((grid[0], True), (grid[-1], False)):
        verts = [column[0], column[1], column[3], column[2]]
        face = bm.faces.new(verts[::-1] if flip else verts)
        face.material_index = 1
        for loop in face.loops:
            loop[uv].uv = (loop.vert.co.y / WATTLE_M, loop.vert.co.z / WATTLE_M)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces[:])
    wood = material("Wood", "log_wood")
    weave = material("Wattle", "wattle")
    finish(bm, "WattleFence", [wood, weave], length, "2.06 x 1.1 x 0.14")


def rails():
    """Two split rails through hewn posts at its ends, 2.5 m apart, each rail sagging a little
    and running on past the post into the next."""
    length, height = 2.5, 1.15
    bm = bmesh.new()
    uv = bm.loops.layers.uv.new()
    for x in (-length / 2, length / 2):
        round_piece(bm, uv, Vector((x, 0, 0)), Vector((x, 0, height)), 0.075, 7, 0)
    for k, z in enumerate((0.45, 0.92)):
        reach = 0.12
        y = 0.0
        start = Vector((-length / 2 - reach, y, z + 0.02 * wobble(k, 5)))
        end = Vector((length / 2 + reach, y, z + 0.02 * wobble(k, 6)))
        # A sag in its middle: two lengths meeting a little low.
        middle = (start + end) / 2 - Vector((0, 0.01 * wobble(k, 7), 0.03))
        round_piece(bm, uv, start, middle, 0.055, 6, 0, flat=1.0)
        round_piece(bm, uv, middle, end, 0.055, 6, 0, flat=1.0)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces[:])
    wood = material("Wood", "log_wood")
    finish(bm, "RailFence", [wood], length, "2.74 x 1.15 x 0.16")


reset()
wattle()
reset()
rails()
