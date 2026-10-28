"""Turned parts made by rule and textured from a model (D565 step 4): a round part along the line
(a trim's tube, a haft) is built here as a closed lathe of rings, exact where it joins and its own
UVs laid round it, and a generated model's look is baked onto it from a copy of that model moved
to the same place: the high-to-low bake every game pipeline uses for a mesh it cannot keep (Blender
manual, Render > Cycles > Baking, Selected to Active). Tripo's tubes are not closed, and a
decimated open tube cannot be cut, so the trim is turned instead and only its look is kept.

Blender's frame here: the line is Blender +Z (glTF +Y).
"""

import contextlib
import math
import os

import bmesh
import bpy
import numpy as np
from mathutils import Vector
from mathutils.bvhtree import BVHTree


@contextlib.contextmanager
def no_metal(ob):
    """ob's materials with their metal at 0 (its links cut, then put back)."""
    kept = []
    for mat in {s.material for s in ob.material_slots if s.material and s.material.node_tree}:
        tree = mat.node_tree
        for bsdf in (n for n in tree.nodes if n.type == "BSDF_PRINCIPLED"):
            socket = bsdf.inputs["Metallic"]
            links = [(l.from_socket, socket) for l in socket.links]
            kept.append((tree, socket, socket.default_value, links))
            for l in list(socket.links):
                tree.links.remove(l)
            socket.default_value = 0.0
    try:
        yield
    finally:
        for tree, socket, value, links in kept:
            socket.default_value = value
            for a, b in links:
                tree.links.new(a, b)


def _tree(ob):
    return BVHTree.FromObject(ob, bpy.context.evaluated_depsgraph_get())


def heights(ob):
    zs = [v.co.z for v in ob.data.vertices]
    return min(zs), max(zs)


def inner_radius(ob, centre, rays=32):
    """A tube's own hole: the median first wall of rays out from the line, a fifth to four fifths up."""
    tree, (lo, hi) = _tree(ob), heights(ob)
    far = 2 * max(math.hypot(v.co.x - centre[0], v.co.y - centre[1]) for v in ob.data.vertices)
    walls = []
    for k in range(1, 8):
        z = lo + (hi - lo) * (0.2 + 0.6 * k / 8)
        for i in range(rays):
            a = 2 * math.pi * i / rays
            hit = tree.ray_cast(Vector((centre[0], centre[1], z)), Vector((math.cos(a), math.sin(a), 0.0)), far)[0]
            if hit is not None:
                walls.append(math.hypot(hit.x - centre[0], hit.y - centre[1]))
    if len(walls) < rays:
        raise SystemExit("prep: rays out from the trim's line meet no wall: it is not upright along glTF +Y, or it is no tube; set turn_deg in its prep.json, or look at its model")
    return sorted(walls)[len(walls) // 2]


def outer_profile(ob, centre, rings, rays=32):
    """The model's outer radius at `rings` heights, ends included: the mean of rays in from outside
    toward the line, each ring's (a bead or a wound turn stays, a fleck averages away)."""
    tree, (lo, hi) = _tree(ob), heights(ob)
    far = 2 * max(math.hypot(v.co.x - centre[0], v.co.y - centre[1]) for v in ob.data.vertices)
    out = []
    for k in range(rings):
        z = lo + (hi - lo) * (0.01 + 0.98 * k / (rings - 1))
        rs = []
        for i in range(rays):
            d = Vector((math.cos(2 * math.pi * i / rays), math.sin(2 * math.pi * i / rays), 0.0))
            hit = tree.ray_cast(Vector((centre[0], centre[1], z)) + d * far, -d, far)[0]
            if hit is not None:
                rs.append(math.hypot(hit.x - centre[0], hit.y - centre[1]))
        out.append((lo + (hi - lo) * k / (rings - 1), sum(rs) / len(rs) if rs else 0.0))
    return out


def remap(ob, centre, radius):
    """Every vertex moved out or in from the line to `radius(r)`, round it as it was."""
    for v in ob.data.vertices:
        r = math.hypot(v.co.x - centre[0], v.co.y - centre[1])
        if r > 1e-9:
            s = radius(r) / r
            v.co.x = centre[0] + (v.co.x - centre[0]) * s
            v.co.y = centre[1] + (v.co.y - centre[1]) * s


def lathe(name, profile, bore, sides):
    """A closed tube on the line: its outer wall the profile's rings [(z, r)], its bore exact at
    `bore` across (its flats touching the circle), flat ends between. UVs: the outer wall round u
    and up v over the lower four fifths of the map, the bore and the ends in strips above."""
    bm = bmesh.new()
    uv = bm.loops.layers.uv.new("UVMap")
    rb = bore / 2 / math.cos(math.pi / sides)
    z0, z1 = profile[0][0], profile[-1][0]
    rings = [(z, r, 0.02 + 0.76 * (z - z0) / (z1 - z0)) for z, r in profile]
    # Walls in order round the section: up the outside, in over the top, down the bore, out under.
    section = rings + [(z1, rb, 0.84), (z0, rb, 0.9)]
    loops = []
    for z, r, _ in section:
        loops.append([bm.verts.new((r * math.cos(2 * math.pi * i / sides), r * math.sin(2 * math.pi * i / sides), z)) for i in range(sides)])
    vs = [s[2] for s in section] + [0.98]
    for k in range(len(section)):
        a, b = loops[k], loops[(k + 1) % len(section)]
        va, vb = vs[k], vs[k + 1] if k + 1 < len(section) else 0.98
        if k == len(rings) - 1:
            va, vb = 0.8, 0.82  # the top end
        if k == len(section) - 1:
            va, vb = 0.92, 0.94  # the bottom end
        for i in range(sides):
            j = (i + 1) % sides
            f = bm.faces.new((a[i], a[j], b[j], b[i]))
            for loop, (u, v) in zip(f.loops, ((i / sides, va), ((i + 1) / sides, va), ((i + 1) / sides, vb), (i / sides, vb))):
                loop[uv].uv = (u, v)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bmesh.ops.triangulate(bm, faces=bm.faces)
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    ob = bpy.data.objects.new(name, mesh)
    bpy.context.scene.collection.objects.link(ob)
    return ob


def rod(name, length, diameter, sides):
    """A closed straight rod up the line from 0 to `length`, as terra_bake's turned handle lays it
    out (crates/terra_bake/src/item.rs): `sides` faces round, u = i / sides round from +X toward
    +Y (glTF +X toward +Z), v = 1 at the butt and 0 at the top (glTF's v runs down the picture);
    its ends take the picture's edge rows."""
    bm = bmesh.new()
    uv = bm.loops.layers.uv.new("UVMap")
    r = diameter / 2
    # glTF +Z is Blender -Y: the ring turns from +X toward glTF +Z as item.rs's does.
    ring = [[bm.verts.new((r * math.cos(2 * math.pi * i / sides), -r * math.sin(2 * math.pi * i / sides), z)) for i in range(sides)] for z in (0.0, length)]
    for i in range(sides):
        j = (i + 1) % sides
        f = bm.faces.new((ring[0][i], ring[0][j], ring[1][j], ring[1][i]))
        for loop, (u, v) in zip(f.loops, ((i / sides, 1.0), ((i + 1) / sides, 1.0), ((i + 1) / sides, 0.0), (i / sides, 0.0))):
            loop[uv].uv = (u, 1.0 - v)  # Blender's v runs up the picture
    for k, v in ((0, 1.0), (1, 0.0)):
        f = bm.faces.new(ring[k])
        for loop in f.loops:
            loop[uv].uv = (0.5, 1.0 - v)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bmesh.ops.triangulate(bm, faces=bm.faces)
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    ob = bpy.data.objects.new(name, mesh)
    bpy.context.scene.collection.objects.link(ob)
    return ob


def concept_colour(path):
    """The median colour (sRGB, 0 to 1) of the part on its concept cut-out: every pixel further
    than 0.1 from the sheet's colour, the median of the cut-out's border."""
    image = bpy.data.images.load(os.path.abspath(path))
    w, h = image.size
    px = np.array(image.pixels[:], dtype=np.float32).reshape(h, w, 4)[:, :, :3]
    bpy.data.images.remove(image)
    border = np.concatenate([px[0], px[-1], px[:, 0], px[:, -1]])
    sheet = np.median(border, axis=0)
    part = px.reshape(-1, 3)[np.abs(px.reshape(-1, 3) - sheet).max(axis=1) > 0.1]
    return np.median(part, axis=0)


def _linear(c):
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def _srgb(c):
    return np.where(c <= 0.0031308, c * 12.92, 1.055 * np.power(np.maximum(c, 0.0), 1 / 2.4) - 0.055)


def grade(image, want):
    """Scales a baked colour map, per channel in linear light, so its median over the baked pixels
    is `want`, the concept's (concept_colour). Tripo paints its shading's shadows into the colour
    and drifts darker than the concept (the binding 47 of 255 against the sheet's leather, the
    hickory 87): the concept sheet is the approved look (art-style.md), so the map keeps its own
    pattern and takes the concept's value and hue."""
    w, h = image.size
    px = np.array(image.pixels[:], dtype=np.float32).reshape(-1, 4)
    baked = px[:, 3] > 0.5
    if not baked.any():
        baked[:] = True
    have = np.median(px[baked, :3], axis=0)
    k = _linear(np.asarray(want)) / np.maximum(_linear(have), 1e-4)
    px[:, :3] = np.clip(_srgb(_linear(px[:, :3]) * k), 0.0, 1.0)
    px[:, 3] = 1.0
    image.pixels[:] = px.ravel()
    print(f"prep: colour graded to the concept: median {np.round(have * 255).astype(int).tolist()} -> {np.round(np.asarray(want) * 255).astype(int).tolist()} (sRGB)")


def bake(src, dst, role, px, metallic, reach, concept=None):
    """Bakes src's colour, normal and roughness onto dst's UVs (Cycles, selected to active, rays
    `reach` out and in), into one material named for its role: colour and normal at px, roughness
    at half, metallic a constant (a part is one material, types.json's roles). With `concept`, the
    part's cut-out, the colour is graded to its value (grade)."""
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.device = "CPU"
    scene.cycles.samples = 4
    mat = bpy.data.materials.new(role)
    dst.data.materials.clear()
    dst.data.materials.append(mat)
    nodes, links = mat.node_tree.nodes, mat.node_tree.links
    bsdf = next(n for n in nodes if n.type == "BSDF_PRINCIPLED")
    bsdf.inputs["Metallic"].default_value = metallic
    maps = {}
    w, h = px if isinstance(px, tuple) else (px, px)
    for key, size, colour in (("colour", (w, h), True), ("normal", (w, h), False), ("rough", (w // 2, h // 2), False)):
        image = bpy.data.images.new(f"{dst.name}_{key}", *size, alpha=True)
        image.generated_color = (0.0, 0.0, 0.0, 0.0)  # baked pixels come out opaque (grade's mask)
        image.colorspace_settings.name = "sRGB" if colour else "Non-Color"
        node = nodes.new("ShaderNodeTexImage")
        node.image = image
        maps[key] = node
    bpy.ops.object.select_all(action="DESELECT")
    src.select_set(True)
    dst.select_set(True)
    bpy.context.view_layer.objects.active = dst
    bake = scene.render.bake
    bake.use_selected_to_active = True
    bake.cage_extrusion = reach
    bake.max_ray_distance = 2 * reach
    bake.margin = 4
    for key, kind, extra in (("colour", "DIFFUSE", {"pass_filter": {"COLOR"}}), ("normal", "NORMAL", {"normal_space": "TANGENT"}), ("rough", "ROUGHNESS", {})):
        nodes.active = maps[key]
        # Cycles' diffuse colour is the base colour times (1 - metallic): a metal source bakes
        # black (the iron collar's colour came out at 13 of 255). Its metal is off while the
        # colour is baked, so the map is its base colour; dst's metal is the constant above.
        with (no_metal(src) if key == "colour" else contextlib.nullcontext()):
            print(f"prep: bake {key}:", bpy.ops.object.bake(type=kind, **extra))
    if concept:
        grade(maps["colour"].image, concept_colour(concept))
    nodes.active = maps["colour"]  # what a Workbench look draws in its colours
    links.new(maps["colour"].outputs["Color"], bsdf.inputs["Base Color"])
    nmap = nodes.new("ShaderNodeNormalMap")
    links.new(maps["normal"].outputs["Color"], nmap.inputs["Color"])
    links.new(nmap.outputs["Normal"], bsdf.inputs["Normal"])
    split = nodes.new("ShaderNodeSeparateColor")
    links.new(maps["rough"].outputs["Color"], split.inputs["Color"])
    links.new(split.outputs["Green"], bsdf.inputs["Roughness"])
    for node in maps.values():
        node.image.file_format = "PNG"
        node.image.filepath_raw = f"{node.image.name}.png"
        node.image.pack()
