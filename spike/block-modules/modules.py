"""Block modules spike (README.md): the same shapes of cubes drawn two ways.

    tools/blender/run.sh spike/block-modules/modules.py OUT TEXTURES SECTION...

OUT is where BlockModulesA.glb and BlockModulesB.glb go; TEXTURES holds Poly Haven's
stone_wall color.jpg, normal.jpg and height.png; each SECTION is a stone kit section,
one metre square and half a metre thick, its face to -Y once imported.

Blender works Z up; a cube is (x, y, z) with x along, y across, z up, half a metre each.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

args = sys.argv[sys.argv.index("--") + 1 :]
OUT, TEXTURES, SOURCES = args[0], os.path.abspath(args[1]), args[2:]

CELL = 0.5
EIGHTH = 0.25
HALF = EIGHTH / 2
# Feathering: how far from a seam the surface is drawn in, and how deep the joint is.
FEATHER = 0.035
GROOVE = 0.012
INSET = 0.01


def shapes():
    """Each test shape: its name and cubes."""
    wall = {(x, 0, z) for x in range(6) for z in range(4)}
    ell = wall | {(0, y, z) for y in range(1, 4) for z in range(4)}
    tee = {(x, 0, z) for x in range(8) for z in range(4)} | {
        (3, y, z) for y in range(1, 4) for z in range(4)
    }
    cross = {(x, 0, z) for x in range(8) for z in range(6)} - {
        (3, 0, 1), (3, 0, 2), (3, 0, 3), (2, 0, 2), (4, 0, 2), (6, 0, 4),
    }
    heights = [4, 4, 3, 3, 2, 1]
    part = {(x, 0, z) for x, h in enumerate(heights) for z in range(h)}
    return [("wall", wall), ("L", ell), ("T", tee), ("cross", cross), ("part-built", part)]


def offsets(shapes):
    """Where each shape stands along X, two metres apart."""
    at, out = 0.0, []
    for _, cubes in shapes:
        xs = [c[0] for c in cubes]
        out.append(at - min(xs) * CELL)
        at += (max(xs) - min(xs) + 1) * CELL + 2.0
    return out


def clear():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def load_section(path):
    """One stone section as one mesh, scaled to exactly a metre square and half a metre
    thick, its corner at (-0.5, -0.25, 0)."""
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=path)
    new = [o for o in bpy.data.objects if o not in before]
    meshes = [o for o in new if o.type == "MESH"]
    for o in meshes:
        world = o.matrix_world.copy()
        o.parent = None
        o.matrix_world = world
    bpy.ops.object.select_all(action="DESELECT")
    for o in meshes:
        o.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    if len(meshes) > 1:
        bpy.ops.object.join()
    ob = bpy.context.view_layer.objects.active
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    for o in new:
        if o.name in bpy.data.objects and o is not ob:
            bpy.data.objects.remove(o)
    co = [v.co.copy() for v in ob.data.vertices]
    low = Vector((min(c.x for c in co), min(c.y for c in co), min(c.z for c in co)))
    high = Vector((max(c.x for c in co), max(c.y for c in co), max(c.z for c in co)))
    size, corner = Vector((1.0, 0.5, 1.0)), Vector((-0.5, -0.25, 0.0))
    for v in ob.data.vertices:
        rel = v.co - low
        v.co = corner + Vector(rel[i] / (high[i] - low[i]) * size[i] for i in range(3))
    return ob


def cut(source, low, high):
    """The part of a mesh inside a box, its cuts filled, moved so the box's middle is the
    origin."""
    bm = bmesh.new()
    bm.from_mesh(source.data)
    for axis in range(3):
        for sign, value in ((-1, low[axis]), (1, high[axis])):
            normal = Vector((0, 0, 0))
            normal[axis] = sign
            point = Vector((0, 0, 0))
            point[axis] = value
            result = bmesh.ops.bisect_plane(
                bm,
                geom=bm.verts[:] + bm.edges[:] + bm.faces[:],
                dist=1e-5,
                plane_co=point,
                plane_no=normal,
                clear_outer=True,
            )
            edges = [e for e in result["geom_cut"] if isinstance(e, bmesh.types.BMEdge)]
            if edges:
                bmesh.ops.holes_fill(bm, edges=edges, sides=0)
    middle = (Vector(low) + Vector(high)) / 2
    bmesh.ops.translate(bm, verts=bm.verts, vec=-middle)
    return bm


def eighths(sections):
    """Every eighth-cube piece of every section, by (version, along, across, up)."""
    pieces = {}
    for version, section in enumerate(sections):
        for sx in range(4):
            for sy in range(2):
                for sz in range(4):
                    low = (-0.5 + sx * EIGHTH, -0.25 + sy * EIGHTH, sz * EIGHTH)
                    high = (low[0] + EIGHTH, low[1] + EIGHTH, low[2] + EIGHTH)
                    bm = cut(section, low, high)
                    me = bpy.data.meshes.new(f"eighth {version} {sx}{sy}{sz}")
                    bm.to_mesh(me)
                    bm.free()
                    for material in section.data.materials:
                        me.materials.append(material)
                    pieces[(version, sx, sy, sz)] = me
    return pieces


# A turn about the up axis, as world (axis, sign) from a section's (axis, sign).
TURNS = {
    "x": {0: lambda s: (0, s), 1: lambda s: (1, s)},
    # The section's length along world Y, its face to world +X.
    "y": {0: lambda s: (1, s), 1: lambda s: (0, -s)},
}
MATRICES = {"x": Matrix.Identity(4), "y": Matrix.Rotation(math.pi / 2, 4, "Z")}


def to_section(along, axis, sign):
    """A world direction in the section's own axes."""
    if axis == 2:
        return (2, sign)
    for source_axis in (0, 1):
        for source_sign in (-1, 1):
            if TURNS[along][source_axis](source_sign) == (axis, sign):
                return (source_axis, source_sign)
    raise ValueError


def choose(cubes):
    """For every eighth of every cube: which piece, turned which way."""
    chosen = {}
    for c in cubes:
        if (c[0] + 1, c[1], c[2]) in cubes or (c[0] - 1, c[1], c[2]) in cubes:
            along = "x"
        elif (c[0], c[1] + 1, c[2]) in cubes or (c[0], c[1] - 1, c[2]) in cubes:
            along = "y"
        else:
            along = "x"
        for ox in range(2):
            for oy in range(2):
                for oz in range(2):
                    o = (ox, oy, oz)
                    w = tuple(2 * c[i] + o[i] for i in range(3))
                    open_world = set()
                    for axis in range(3):
                        sign = -1 if o[axis] == 0 else 1
                        n = list(c)
                        n[axis] += sign
                        if tuple(n) not in cubes:
                            open_world.add((axis, sign))
                    open_src = {to_section(along, a, s) for a, s in open_world}
                    a = w[0] if along == "x" else w[1]
                    if (0, -1) in open_src:
                        sx = 0
                    elif (0, 1) in open_src:
                        sx = 3
                    else:
                        sx = a % 4
                    if (1, -1) in open_src:
                        sy = 0
                    elif (1, 1) in open_src:
                        sy = 1
                    else:
                        sy = o[1] if along == "x" else 1 - o[0]
                    if (2, -1) in open_src:
                        sz = 0
                    elif (2, 1) in open_src:
                        sz = 3
                    else:
                        sz = w[2] % 4
                    row = c[1] if along == "x" else c[0]
                    version = hash3(a // 4, w[2] // 4, row) % len(SOURCES)
                    chosen[w] = (version, along, sx, sy, sz, open_src)
    return chosen


def hash3(a, b, c):
    x = (a * 73856093) ^ (b * 19349663) ^ (c * 83492791)
    x ^= x >> 13
    return abs(x * 0x5BD1E995) & 0x7FFFFFFF


def conformed(piece, choice, chosen, w):
    """A copy of a piece with its surface drawn into a joint along every side where its
    neighbour did not come from the place beside it in the same section."""
    version, along, sx, sy, sz, open_src = choice
    me = piece.copy()
    seams = []
    for axis in range(3):
        for sign in (-1, 1):
            n = list(w)
            n[axis] += sign
            other = chosen.get(tuple(n))
            if other is None:
                continue
            s_axis, s_sign = to_section(along, axis, sign)
            expect = [sx, sy, sz]
            expect[s_axis] += s_sign
            if other[0] != version or other[1] != along or list(other[2:5]) != expect:
                seams.append((s_axis, s_sign))
    if not seams:
        return me
    for v in me.vertices:
        co = v.co.copy()
        for e_axis, e_sign in open_src:
            surface = e_sign * (HALF - INSET)
            if e_sign * co[e_axis] < e_sign * surface - 0.06:
                continue
            for k, k_sign in seams:
                if k == e_axis:
                    continue
                distance = HALF - k_sign * co[k]
                weight = max(0.0, 1.0 - distance / FEATHER)
                target = surface - e_sign * GROOVE
                co[e_axis] += (target - co[e_axis]) * weight
        v.co = co
    return me


def build_a(cubes, pieces, name, x):
    chosen = choose(cubes)
    objects = []
    for w, choice in chosen.items():
        version, along, sx, sy, sz, _ = choice
        me = conformed(pieces[(version, sx, sy, sz)], choice, chosen, w)
        ob = bpy.data.objects.new(f"{name} {w}", me)
        bpy.context.scene.collection.objects.link(ob)
        middle = Vector(w[i] * EIGHTH + HALF for i in range(3)) + Vector((x, 0, 0))
        ob.matrix_world = Matrix.Translation(middle) @ MATRICES[along]
        objects.append(ob)
    return join(objects, f"A {name}")


def join(objects, name):
    bpy.ops.object.select_all(action="DESELECT")
    for o in objects:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    bpy.ops.object.join()
    ob = bpy.context.view_layer.objects.active
    ob.name = name
    bm = bmesh.new()
    bm.from_mesh(ob.data)
    bmesh.ops.triangulate(bm, faces=bm.faces)
    bm.to_mesh(ob.data)
    bm.free()
    return ob


def stone_material():
    mat = bpy.data.materials.new("Stone B")
    mat.use_nodes = True
    nodes, links = mat.node_tree.nodes, mat.node_tree.links
    bsdf = nodes["Principled BSDF"]
    colour = nodes.new("ShaderNodeTexImage")
    colour.image = bpy.data.images.load(os.path.join(TEXTURES, "color.jpg"))
    links.new(colour.outputs["Color"], bsdf.inputs["Base Color"])
    normal = nodes.new("ShaderNodeTexImage")
    normal.image = bpy.data.images.load(os.path.join(TEXTURES, "normal.jpg"))
    normal.image.colorspace_settings.name = "Non-Color"
    bump = nodes.new("ShaderNodeNormalMap")
    links.new(normal.outputs["Color"], bump.inputs["Color"])
    links.new(bump.outputs["Normal"], bsdf.inputs["Normal"])
    bsdf.inputs["Roughness"].default_value = 0.85
    return mat


def build_b(cubes, name, x, triangles, material, height):
    bm = bmesh.new()
    corners = [(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0), (0, 0, 1), (1, 0, 1), (1, 1, 1), (0, 1, 1)]
    faces = {
        (1, 0, 0): (1, 2, 6, 5), (-1, 0, 0): (0, 4, 7, 3),
        (0, 1, 0): (2, 3, 7, 6), (0, -1, 0): (0, 1, 5, 4),
        (0, 0, 1): (4, 5, 6, 7), (0, 0, -1): (0, 3, 2, 1),
    }
    for c in cubes:
        for d, quad in faces.items():
            if (c[0] + d[0], c[1] + d[1], c[2] + d[2]) in cubes:
                continue
            verts = [
                bm.verts.new(Vector(((c[i] + corners[q][i]) * CELL for i in range(3))))
                for q in quad
            ]
            bm.faces.new(verts)
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-4)
    me = bpy.data.meshes.new(f"B {name}")
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(f"B {name}", me)
    bpy.context.scene.collection.objects.link(ob)
    bpy.context.view_layer.objects.active = ob
    bevel = ob.modifiers.new("Bevel", "BEVEL")
    bevel.width, bevel.segments, bevel.limit_method = 0.04, 3, "ANGLE"
    remesh = ob.modifiers.new("Remesh", "REMESH")
    remesh.mode, remesh.voxel_size = "VOXEL", 0.015
    for m in list(ob.modifiers):
        bpy.ops.object.select_all(action="DESELECT")
        ob.select_set(True)
        bpy.ops.object.modifier_apply(modifier=m.name)
    # Texture from each face's plane, two metres a copy, as the material is.
    me = ob.data
    uv = me.uv_layers.new(name="UVMap")
    for poly in me.polygons:
        n = poly.normal
        axis = max(range(3), key=lambda i: abs(n[i]))
        a, b = [(1, 2), (0, 2), (0, 1)][axis]
        for li in poly.loop_indices:
            p = me.vertices[me.loops[li].vertex_index].co
            uv.data[li].uv = (p[a] / 2.0, p[b] / 2.0)
    texture = bpy.data.textures.new("Stone height", "IMAGE")
    texture.image = height
    displace = ob.modifiers.new("Displace", "DISPLACE")
    displace.texture, displace.texture_coords, displace.uv_layer = texture, "UV", "UVMap"
    displace.strength, displace.mid_level = 0.05, 0.5
    decimate = ob.modifiers.new("Decimate", "DECIMATE")
    faces_now = sum(len(p.vertices) - 2 for p in me.polygons)
    decimate.ratio = min(1.0, triangles / max(faces_now, 1))
    for m in list(ob.modifiers):
        bpy.ops.object.select_all(action="DESELECT")
        ob.select_set(True)
        bpy.ops.object.modifier_apply(modifier=m.name)
    me.materials.append(material)
    for p in me.polygons:
        p.use_smooth = True
    ob.location.x += x
    return ob


def export(objects, path):
    bpy.ops.object.select_all(action="DESELECT")
    for o in objects:
        o.select_set(True)
    bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True)
    print(f"wrote {path}")


def triangles(ob):
    return sum(len(p.vertices) - 2 for p in ob.data.polygons)


clear()
sections = [load_section(path) for path in SOURCES]
pieces = eighths(sections)
for s in sections:
    bpy.data.objects.remove(s)
os.makedirs(OUT, exist_ok=True)
test = shapes()
a_objects, counts = [], []
for (name, cubes), x in zip(test, offsets(test)):
    ob = build_a(cubes, pieces, name, x)
    a_objects.append(ob)
    counts.append(triangles(ob))
    print(f"A {name}: {len(cubes)} cubes, {counts[-1]} triangles")
export(a_objects, os.path.join(OUT, "BlockModulesA.glb"))
for o in a_objects:
    bpy.data.objects.remove(o)
material = stone_material()
height = bpy.data.images.load(os.path.join(TEXTURES, "height.png"))
height.colorspace_settings.name = "Non-Color"
b_objects = []
for (name, cubes), x, count in zip(test, offsets(test), counts):
    ob = build_b(cubes, name, x, count, material, height)
    b_objects.append(ob)
    print(f"B {name}: {triangles(ob)} triangles")
export(b_objects, os.path.join(OUT, "BlockModulesB.glb"))
