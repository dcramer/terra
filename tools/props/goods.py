"""Composes the goods a market stall shows from scanned models, by the recipes in
art/goods.json (tools/props/goods.ts fetches the scans and runs this).

    tools/blender/run.sh tools/props/goods.py RECIPES.json SOURCES.json OUT_DIR [FILE...]

A recipe names its `sources`, each a scanned model (`from`, a Poly Haven id) cut to a
triangle `budget` with its own colour, normal and roughness maps kept at `texture` pixels
(D304: 1K for a small prop, 512 for a small piece of one), or a piece of cloth made here
(`make`: a `bolt` rolled on its board or a `fold` of cloth laid flat) in a CC0 fabric
(`cloth`), dyed by `tint` and `saturation`. Then its `pieces`, each a source stood at `at`
(glTF's metres: x across, y up, z to the front), turned `turn` degrees about up, after
`rotate` (degrees about Blender's X, Y and Z) lays it the way it rests; and its `heaps`, a
`count` of sources dropped into a piece (`into`) or onto an `area`, onto a false floor at
`floor` metres, by Blender's rigid body simulation, so fruit settles as fruit does, with
only its top layer modelled, as games fill a crate. Anything that rolls out is left out.
Nothing is random from run to run: each heap has its `seed`.

Each prop is one mesh, a part for each material, smooth across curves and split at creases
(docs/art-style.md), its origin the middle of its base, its front +Z (the model contract,
D303), written to OUT_DIR's assets/models/props/<file>.glb with its maps as JPEG and its
tangents. It prints GOODS and writes OUT_DIR/<file>.json with its triangles and size, the
`dims` its catalogue entry states.
"""

import json
import math
import os
import random
import sys

import bmesh
import bpy
import numpy as np
from mathutils import Euler, Matrix, Vector

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
PROPS = os.path.join(ROOT, "assets", "models", "props")
JPEG_QUALITY = 88
# How long the heaps are let fall and settle, in frames at 24 a second.
SETTLE_FRAMES = 140


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def triangles(ob):
    return sum(len(p.vertices) - 2 for p in ob.data.polygons)


def select_only(*obs):
    bpy.ops.object.select_all(action="DESELECT")
    for ob in obs:
        ob.select_set(True)
    bpy.context.view_layer.objects.active = obs[0]


def import_scan(path):
    """A scan's mesh objects, their places applied, parented to nothing."""
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=path)
    new = [o for o in bpy.data.objects if o not in before]
    meshes = [o for o in new if o.type == "MESH"]
    for o in new:
        if o.type != "MESH":
            bpy.data.objects.remove(o)
    select_only(*meshes)
    bpy.ops.object.parent_clear(type="CLEAR_KEEP_TRANSFORM")
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    return meshes


def cut(ob, budget):
    """Welded, cut to its budget by collapse decimation, which keeps its texture places."""
    select_only(ob)
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.mesh.remove_doubles(threshold=0.0001)
    bpy.ops.object.mode_set(mode="OBJECT")
    before = triangles(ob)
    for _ in range(6):
        now = triangles(ob)
        if now <= budget * 1.05:
            break
        modifier = ob.modifiers.new("cut", "DECIMATE")
        modifier.decimate_type = "COLLAPSE"
        modifier.ratio = budget / now
        modifier.use_collapse_triangulate = True
        bpy.ops.object.modifier_apply(modifier=modifier.name)
    print(f"CUT {ob.name}: {before} to {triangles(ob)} triangles, budget {budget}")


def smooth(ob):
    select_only(ob)
    bpy.ops.object.shade_smooth_by_angle(angle=math.radians(50.0), keep_sharp_edges=True)


def images_of(ob):
    found = set()
    for slot in ob.material_slots:
        if slot.material and slot.material.node_tree:
            for node in slot.material.node_tree.nodes:
                if node.type == "TEX_IMAGE" and node.image:
                    found.add(node.image)
    return found


def shrink(ob, px):
    for image in images_of(ob):
        w, h = image.size
        if max(w, h) > px:
            image.scale(w * px // max(w, h), h * px // max(w, h))


def rest(ob, rotate):
    """Turned to rest as it lies (Blender's degrees about X, Y, Z), its foot at the origin
    and its middle over it."""
    me = ob.data
    if rotate:
        me.transform(Euler([math.radians(a) for a in rotate], "XYZ").to_matrix().to_4x4())
    pts = np.array([v.co for v in me.vertices])
    lo, hi = pts.min(axis=0), pts.max(axis=0)
    me.transform(Matrix.Translation(Vector(((lo[0] + hi[0]) / -2, (lo[1] + hi[1]) / -2, -lo[2]))))
    me.update()


def dye(img, tint, saturation, mottle=0.0):
    """A colour map dyed: its saturation scaled, then multiplied by `tint` (sRGB); and with
    `mottle`, darkened here and there in broad soft patches that tile, as cloth fades and
    takes up dye unevenly, so a plain weave reads as cloth and not as paint."""
    w, h = img.size
    px = np.empty(w * h * 4, dtype=np.float32)
    img.pixels.foreach_get(px)
    px = px.reshape(h, w, 4)
    grey = px[:, :, :3].mean(axis=2, keepdims=True)
    px[:, :, :3] = (grey + (px[:, :, :3] - grey) * saturation) * np.array(tint)
    if mottle:
        u = np.linspace(0, 2 * math.pi, w, endpoint=False)[None, :]
        v = np.linspace(0, 2 * math.pi, h, endpoint=False)[:, None]
        patches = (np.sin(2 * u + 3 * v + 1.0) + np.sin(5 * u - 2 * v + 2.0) + np.sin(3 * u + 7 * v + 4.0)) / 6 + 0.5
        px[:, :, :3] *= (1.0 - mottle * patches)[:, :, None]
    px[:, :, :3] = np.clip(px[:, :, :3], 0.0, 1.0)
    img.pixels.foreach_set(px.ravel())
    img.update()


def colour_map(ob):
    for slot in ob.material_slots:
        tree = slot.material and slot.material.node_tree
        if tree:
            for node in tree.nodes:
                if node.type == "BSDF_PRINCIPLED":
                    for link in node.inputs["Base Color"].links:
                        if link.from_node.type == "TEX_IMAGE":
                            return link.from_node.image
    return None


def fabric(name, folder, tint, saturation, metres):
    """A material of a fetched CC0 fabric: its colour map dyed (sRGB, multiplied, after its
    saturation is scaled), its normal and roughness maps as they are, at 512 pixels."""
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nodes, links = mat.node_tree.nodes, mat.node_tree.links
    bsdf = nodes["Principled BSDF"]

    def image(file, colour):
        img = bpy.data.images.load(os.path.join(folder, file))
        img.name = f"{name}_{file.split('.')[0]}"
        if not colour:
            img.colorspace_settings.name = "Non-Color"
        img.scale(512, 512)
        node = nodes.new("ShaderNodeTexImage")
        node.image = img
        return node, img

    colour, img = image("color.jpg", True)
    dye(img, tint, saturation, mottle=0.12)
    links.new(colour.outputs["Color"], bsdf.inputs["Base Color"])
    normal, _ = image("normal.jpg", False)
    bump = nodes.new("ShaderNodeNormalMap")
    links.new(normal.outputs["Color"], bump.inputs["Color"])
    links.new(bump.outputs["Normal"], bsdf.inputs["Normal"])
    rough, _ = image("rough.jpg", False)
    links.new(rough.outputs["Color"], bsdf.inputs["Roughness"])
    bsdf.inputs["Metallic"].default_value = 0.0
    mat["metres"] = metres
    return mat


def cloth(name, spec, sources):
    """A piece of cloth made here: a `bolt`, cloth wound into a roll `size` long (along X)
    and across, its ends showing the cloth wound round, and its loose end, `flap` metres of
    it, lying out to the front; or a `fold`, cloth folded into a pad `size` long, deep and
    thick, with its folds' edges round. Its fabric is wrapped round it at its real size."""
    folder = sources[spec["cloth"]]["path"]
    metres = sources[spec["cloth"]].get("metres", 0.3)
    mat = fabric(name, folder, spec.get("tint", [1, 1, 1]), spec.get("saturation", 1.0), metres)
    length, wide, thick = spec["size"]
    roll = spec["make"] == "bolt"
    bm = bmesh.new()
    uv = bm.loops.layers.uv.new("UVMap")
    sides = 16 if roll else 20
    section = []
    for i in range(sides):
        a = 2 * math.pi * (i + 0.5) / sides
        c, s = math.cos(a), math.sin(a)
        if roll:
            # Round, a little soft and uneven as wound cloth is, and flattened where it
            # rests; the outer turn's edge a step in it.
            r = 1.0 + 0.025 * math.sin(3 * a + len(name)) + (0.03 if i < sides // 3 else 0.0)
            y, z = c * r * wide / 2, max(s * r * thick / 2, -thick * 0.42) + thick * 0.42
        else:
            # A superellipse: flat on its faces, round at its folded edges.
            k = 0.6
            y = math.copysign(abs(c) ** k, c) * wide / 2
            z = math.copysign(abs(s) ** k, s) * thick / 2 + thick / 2
        section.append((y, z))
    perimeter = [0.0]
    for i in range(sides):
        a, b = section[i], section[(i + 1) % sides]
        perimeter.append(perimeter[-1] + math.dist(a, b))
    # Along its length in a few steps: a fold's top soft and a little domed between its
    # ends, as folded cloth lies.
    steps = 1 if roll else 6
    lines = []
    for n in range(steps + 1):
        x = -length / 2 + length * n / steps
        dome = 0.0 if roll else 0.25 * (1.0 - (2.0 * x / length) ** 2)
        lines.append([bm.verts.new((x, y, z * (1.0 + dome * max(0.0, (z - thick * 0.5) / thick)))) for y, z in section])
    for n in range(steps):
        for i in range(sides):
            j = (i + 1) % sides
            a, b = lines[n], lines[n + 1]
            v0, v1 = length * n / steps, length * (n + 1) / steps
            face = bm.faces.new([a[i], a[j], b[j], b[i]])
            for loop, (u, v) in zip(face.loops, [(perimeter[i], v0), (perimeter[i + 1], v0), (perimeter[i + 1], v1), (perimeter[i], v1)]):
                loop[uv].uv = (u / metres, v / metres)
    ends = [lines[0], lines[-1]]
    centre_z = thick * 0.42 if roll else thick / 2
    for e, ring in enumerate(ends):
        x = ring[0].co.x
        if roll:
            # The end in steps, each turn of cloth a little sunk inside the one round it, so
            # it shows wound layer on layer; its fabric mapped round in rings.
            inward = -math.copysign(1.0, x)
            rings = [ring]
            for k, share in enumerate((0.78, 0.55, 0.3)):
                depth = 0.004 * (k + 1)
                rings.append([
                    bm.verts.new((x + inward * depth, v.co.y * share, centre_z + (v.co.z - centre_z) * share))
                    for v in ring
                ])
            middle = bm.verts.new((x + inward * 0.018, 0.0, centre_z))

            def mapped(face):
                for loop in face.loops:
                    r = math.hypot(loop.vert.co.y, loop.vert.co.z - centre_z)
                    a = math.atan2(loop.vert.co.z - centre_z, loop.vert.co.y)
                    loop[uv].uv = (r / metres * 4.0, a / math.pi * 0.08)

            for outer, inner in zip(rings, rings[1:]):
                for i in range(sides):
                    j = (i + 1) % sides
                    quad = [outer[i], outer[j], inner[j], inner[i]]
                    mapped(bm.faces.new(quad if e else quad[::-1]))
            for i in range(sides):
                j = (i + 1) % sides
                tri = [middle, rings[-1][i], rings[-1][j]] if e else [middle, rings[-1][j], rings[-1][i]]
                mapped(bm.faces.new(tri))
        else:
            face = bm.faces.new(ring if e else ring[::-1])
            for loop in face.loops:
                loop[uv].uv = (loop.vert.co.y / metres, loop.vert.co.z / metres)
    flap = spec.get("flap", 0.0)
    if roll and flap:
        # The loose end, unwound off the roll's front and lying flat before it.
        y0 = -wide * 0.3
        lift = 0.003
        quad = [
            bm.verts.new((-length * 0.48, y0, lift)),
            bm.verts.new((length * 0.48, y0, lift)),
            bm.verts.new((length * 0.47, y0 - flap, lift)),
            bm.verts.new((-length * 0.47, y0 - flap, lift)),
        ]
        face = bm.faces.new(quad[::-1])
        for loop in face.loops:
            loop[uv].uv = (loop.vert.co.x / metres, loop.vert.co.y / metres)
    bmesh.ops.triangulate(bm, faces=bm.faces[:])
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    me.materials.append(mat)
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    smooth(ob)
    return ob


def baked(high, budget, px, voxel):
    """A low model of a scan too open to cut, such as a woven basket, whose weave collapse
    decimation tears apart: its shape remeshed as one closed shell `voxel` metres fine, cut
    to its budget and unwrapped afresh, and the scan's colour, the relief the shell lost
    (tangent-space normals) and its roughness baked into it, as games bake a low model from
    its high (as tools/kits/pieces.py bakes the kits' pieces)."""
    low = high.copy()
    low.data = high.data.copy()
    bpy.context.scene.collection.objects.link(low)
    select_only(low)
    remesh = low.modifiers.new("remesh", "REMESH")
    remesh.mode = "VOXEL"
    remesh.voxel_size = voxel
    bpy.ops.object.modifier_apply(modifier=remesh.name)
    cut(low, budget)
    smooth(low)
    while low.data.uv_layers:
        low.data.uv_layers.remove(low.data.uv_layers[0])
    low.data.uv_layers.new(name="UVMap")
    low.data.materials.clear()
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.uv.smart_project(angle_limit=math.radians(66), island_margin=0.003)
    bpy.ops.uv.pack_islands(rotate=True, margin=0.003)
    bpy.ops.object.mode_set(mode="OBJECT")
    mat = bpy.data.materials.new(high.name)
    mat.use_nodes = True
    nodes, links = mat.node_tree.nodes, mat.node_tree.links
    bsdf = nodes["Principled BSDF"]
    low.data.materials.append(mat)
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.device = "CPU"
    scene.cycles.samples = 4
    bake = scene.render.bake
    bake.margin = 4
    bake.use_selected_to_active = True
    bake.cage_extrusion = voxel * 1.5
    bake.max_ray_distance = voxel * 4
    made = {}
    for kind, kwargs in (
        ("color", {"type": "DIFFUSE", "pass_filter": {"COLOR"}}),
        ("normal", {"type": "NORMAL"}),
        ("rough", {"type": "ROUGHNESS"}),
    ):
        image = bpy.data.images.new(f"{high.name}_{kind}", px, px)
        if kind != "color":
            image.colorspace_settings.name = "Non-Color"
        node = nodes.new("ShaderNodeTexImage")
        node.image = image
        nodes.active = node
        bpy.ops.object.select_all(action="DESELECT")
        high.select_set(True)
        low.select_set(True)
        bpy.context.view_layer.objects.active = low
        result = bpy.ops.object.bake(**kwargs)
        if result != {"FINISHED"}:
            raise SystemExit(f"baking {high.name}'s {kind}: {result}")
        image.pack()
        made[kind] = node
    links.new(made["color"].outputs["Color"], bsdf.inputs["Base Color"])
    bump = nodes.new("ShaderNodeNormalMap")
    links.new(made["normal"].outputs["Color"], bump.inputs["Color"])
    links.new(bump.outputs["Normal"], bsdf.inputs["Normal"])
    links.new(made["rough"].outputs["Color"], bsdf.inputs["Roughness"])
    bsdf.inputs["Metallic"].default_value = 0.0
    bpy.data.objects.remove(high)
    low.name = mat.name
    return low


def templates(recipe, sources):
    """Each source made once: cut, smoothed, its textures shrunk, at rest at the origin. A
    source with `split` gives one template a loose object, as a set of pears has."""
    made = {}
    for key, spec in recipe["sources"].items():
        if "make" in spec:
            made[key] = [cloth(key, spec, sources)]
            continue
        meshes = import_scan(sources[spec["from"]]["path"])
        keep = [o for o in meshes if not any(s in o.name for s in spec.get("skip", []))]
        for o in meshes:
            if o not in keep:
                bpy.data.objects.remove(o)
        if spec.get("split"):
            groups = [[o] for o in sorted(keep, key=lambda o: o.name)]
        else:
            groups = [keep]
        made[key] = []
        for group in groups:
            select_only(*group)
            if len(group) > 1:
                bpy.ops.object.join()
            ob = bpy.context.view_layer.objects.active
            ob.name = key
            if "bake" in spec:
                ob = baked(ob, spec.get("budget", 500), spec.get("texture", 512), spec["bake"])
            else:
                cut(ob, spec.get("budget", 500))
                shrink(ob, spec.get("texture", 512))
            if "tint" in spec or "saturation" in spec:
                dye(colour_map(ob), spec.get("tint", [1, 1, 1]), spec.get("saturation", 1.0))
            smooth(ob)
            rest(ob, spec.get("rotate"))
            made[key].append(ob)
    for obs in made.values():
        for ob in obs:
            ob.hide_set(True)
            ob.hide_render = True
    return made


def copy_of(template):
    ob = template.copy()
    ob.data = template.data.copy()
    bpy.context.scene.collection.objects.link(ob)
    ob.hide_set(False)
    ob.hide_render = False
    return ob


def blender_place(at):
    """glTF's x, y (up), z (front) as Blender's x, y, z: x, -z, y."""
    x, y, z = at
    return Vector((x, -z, y))


def stand(template, at, turn, rotate=None):
    ob = copy_of(template)
    if rotate:
        ob.data.transform(Euler([math.radians(a) for a in rotate], "XYZ").to_matrix().to_4x4())
        pts = np.array([v.co for v in ob.data.vertices])
        ob.data.transform(Matrix.Translation(Vector((0, 0, -pts[:, 2].min()))))
    ob.matrix_world = Matrix.Translation(blender_place(at)) @ Matrix.Rotation(math.radians(turn), 4, "Z")
    return ob


def bounds(ob):
    pts = [ob.matrix_world @ v.co for v in ob.data.vertices]
    lo = Vector([min(p[i] for p in pts) for i in range(3)])
    hi = Vector([max(p[i] for p in pts) for i in range(3)])
    return lo, hi


def rigid(ob, kind, shape, **settings):
    select_only(ob)
    result = bpy.ops.rigidbody.object_add(type=kind)
    if result != {"FINISHED"}:
        raise SystemExit(f"rigid body on {ob.name}: {result}")
    body = ob.rigid_body
    body.collision_shape = shape
    body.collision_margin = 0.001
    for name, value in settings.items():
        setattr(body, name, value)


def box(name, lo, hi):
    me = bpy.data.meshes.new(name)
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    ob.location = (lo + hi) / 2
    ob.scale = hi - lo
    select_only(ob)
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    return ob


def heap(spec, pieces, made):
    """Drops `count` of the sources onto a false floor in a piece or an area, lets them
    settle, and keeps those that came to rest inside."""
    scene = bpy.context.scene
    if scene.rigidbody_world is None:
        bpy.ops.rigidbody.world_add()
    world = scene.rigidbody_world
    world.substeps_per_frame = 20
    world.solver_iterations = 30
    scene.frame_start, scene.frame_end = 1, SETTLE_FRAMES
    world.point_cache.frame_start, world.point_cache.frame_end = 1, SETTLE_FRAMES
    helpers = []
    if "into" in spec:
        container = pieces[spec["into"]]
        lo, hi = bounds(container)
        rigid(container, "PASSIVE", "MESH")
        inset = spec.get("inset", 0.02)
    else:
        w, d = spec["area"]
        base = spec.get("base", 0.0)
        lo = Vector((-w / 2, -d / 2, base)) + blender_place(spec.get("at", [0, 0, 0])) * Vector((1, 1, 0))
        hi = Vector((w / 2, d / 2, base + spec.get("walls", 0.15))) + blender_place(spec.get("at", [0, 0, 0])) * Vector((1, 1, 0))
        inset = 0.0
    floor = lo.z + spec.get("floor", 0.0)
    # The false floor and walls round the edge, up to the rim, which nothing passes through.
    helpers.append(box("floor", Vector((lo.x, lo.y, floor - 0.05)), Vector((hi.x, hi.y, floor))))
    wall = 0.05
    top = hi.z if "into" in spec else hi.z
    helpers += [
        box("wall", Vector((lo.x - wall, lo.y, floor)), Vector((lo.x + inset, hi.y, top))),
        box("wall", Vector((hi.x - inset, lo.y, floor)), Vector((hi.x + wall, hi.y, top))),
        box("wall", Vector((lo.x, lo.y - wall, floor)), Vector((hi.x, lo.y + inset, top))),
        box("wall", Vector((lo.x, hi.y - inset, floor)), Vector((hi.x, hi.y + wall, top))),
    ]
    for helper in helpers:
        rigid(helper, "PASSIVE", "BOX", friction=0.9)
    rng = random.Random(spec.get("seed", 1))
    kinds = [t for key in spec["of"] for t in made[key]]
    dropped = []
    margin = spec.get("margin", 0.06)
    for i in range(spec["count"]):
        ob = copy_of(rng.choice(kinds))
        ob.matrix_world = Matrix.Translation(
            Vector((
                rng.uniform(lo.x + margin, hi.x - margin),
                rng.uniform(lo.y + margin, hi.y - margin),
                floor + 0.06 + i * spec.get("spacing", 0.03),
            ))
        ) @ Euler((rng.uniform(0, 6.28), rng.uniform(0, 6.28), rng.uniform(0, 6.28))).to_matrix().to_4x4()
        if spec.get("upright"):
            ob.matrix_world = Matrix.Translation(ob.matrix_world.translation) @ Matrix.Rotation(rng.uniform(0, 6.28), 4, "Z")
        rigid(ob, "ACTIVE", spec.get("shape", "CONVEX_HULL"), mass=0.2, friction=0.8, restitution=0.0,
              linear_damping=0.6, angular_damping=0.8)
        dropped.append(ob)
    for frame in range(1, SETTLE_FRAMES + 1):
        scene.frame_set(frame)
    select_only(*dropped)
    bpy.ops.object.visual_transform_apply()
    for ob in dropped + helpers + ([pieces[spec["into"]]] if "into" in spec else []):
        select_only(ob)
        bpy.ops.rigidbody.object_remove()
    kept = []
    for ob in dropped:
        c = ob.matrix_world.translation
        if lo.x <= c.x <= hi.x and lo.y <= c.y <= hi.y and c.z >= floor - 0.01:
            kept.append(ob)
        else:
            bpy.data.objects.remove(ob)
    for helper in helpers:
        bpy.data.objects.remove(helper)
    scene.frame_set(1)
    return kept


def make(file, recipe, sources, out):
    reset()
    made = templates(recipe, sources)
    pieces = [stand(made[p["of"]][p.get("variant", 0)], p.get("at", [0, 0, 0]), p.get("turn", 0.0), p.get("rotate")) for p in recipe.get("pieces", [])]
    heaped = []
    for spec in recipe.get("heaps", []):
        heaped += heap(spec, pieces, made)
    for obs in made.values():
        for ob in obs:
            bpy.data.objects.remove(ob)
    everything = pieces + heaped
    select_only(*everything)
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    if len(everything) > 1:
        bpy.ops.object.join()
    ob = bpy.context.view_layer.objects.active
    ob.name = file
    # Its origin the middle of its base (the model contract).
    pts = np.array([v.co for v in ob.data.vertices])
    lo, hi = pts.min(axis=0), pts.max(axis=0)
    ob.data.transform(Matrix.Translation(Vector(((lo[0] + hi[0]) / -2, (lo[1] + hi[1]) / -2, -lo[2]))))
    ob.data.update()
    bpy.context.view_layer.update()
    size = ob.dimensions
    report = {
        "triangles": triangles(ob),
        "dims": [round(size.x, 3), round(size.z, 3), round(size.y, 3)],
        "materials": [m.name for m in ob.data.materials],
        "textures": sorted(f"{i.name}: {i.size[0]}x{i.size[1]}" for i in images_of(ob)),
        "pieces": len(everything),
    }
    select_only(ob)
    bpy.ops.export_scene.gltf(
        filepath=os.path.join(PROPS, f"{file}.glb"),
        export_format="GLB",
        use_selection=True,
        export_tangents=True,
        export_image_format="JPEG",
        export_jpeg_quality=JPEG_QUALITY,
    )
    with open(os.path.join(out, f"{file}.json"), "w") as f:
        json.dump(report, f, indent=1)
    print("GOODS", file, json.dumps(report))


def main():
    args = sys.argv[sys.argv.index("--") + 1 :]
    recipes_file, sources_file, out, *wanted = args
    with open(recipes_file) as f:
        recipes = json.load(f)
    with open(sources_file) as f:
        sources = json.load(f)
    for file in wanted or list(recipes):
        make(file, recipes[file], sources, out)


main()
