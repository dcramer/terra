"""Shared Blender helpers for the model bake-off: our palette, our light, renders and checks.

Imported by the model scripts, which run inside Blender (see run.sh). Blender is Z-up in
metres; the glTF exporter turns that into the game's Y-up.
"""

import math
import os
import sys

import bmesh
import bpy
import numpy as np
from mathutils import Vector

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "out")
FIGURE = os.path.join(HERE, "..", "..", "crates", "terra_sandbox", "assets", "models", "character", "figure.gltf")

# The palette, in sRGB, from the sandbox's own recolouring (play/flora.rs and props.rs).
# A model's materials must be named from this list; its colour comes from here, not the file.
PALETTE = {
    "Rock": (0.66, 0.58, 0.54),
    "RockDark": (0.48, 0.42, 0.40),
    "Moss": (0.50, 0.58, 0.26),
    "Ore": (0.52, 0.33, 0.27),
    "Bark": (0.43, 0.29, 0.22),
    "Wood": (0.56, 0.39, 0.25),
    "WoodLight": (0.74, 0.56, 0.37),
    "WoodDark": (0.37, 0.26, 0.19),
    "Leaves": (0.42, 0.58, 0.25),
    "LeavesDark": (0.33, 0.50, 0.22),
    "LeavesLight": (0.55, 0.66, 0.28),
    "Pine": (0.20, 0.40, 0.30),
    "Grass": (0.47, 0.60, 0.22),
    "Hide": (0.62, 0.48, 0.36),
    "Fur": (0.55, 0.52, 0.50),
    "FurDark": (0.30, 0.28, 0.29),
    "FurLight": (0.86, 0.82, 0.76),
    "Cloth": (0.82, 0.74, 0.62),
    "ClothRed": (0.62, 0.20, 0.17),
    "ClothBlue": (0.20, 0.45, 0.56),
    "Leather": (0.45, 0.30, 0.20),
    "Skin": (0.80, 0.58, 0.45),
    "Hair": (0.24, 0.17, 0.13),
    "Metal": (0.62, 0.63, 0.66),
    "MetalDark": (0.30, 0.31, 0.36),
    "Rope": (0.78, 0.66, 0.46),
    "Bone": (0.93, 0.88, 0.78),
    "Black": (0.14, 0.14, 0.17),
    "Accent": (0.90, 0.46, 0.17),
}

# The game's light (experiments/look.rs): a warm sun, shadows lit by a cool sky.
SUN_COLOUR = (1.0, 0.9, 0.76)
SKY_COLOUR = (0.62, 0.68, 1.0)
GROUND = (0.78, 0.66, 0.50)


def linear(c):
    return tuple(x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4 for x in c)


def args():
    """Arguments after Blender's `--`."""
    return sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def material(name):
    """The palette material called `name`, made once."""
    if name not in PALETTE:
        raise KeyError(f"{name} is not in the palette")
    mat = bpy.data.materials.get(name)
    if mat is None:
        mat = bpy.data.materials.new(name)
        colour = linear(PALETTE[name]) + (1.0,)
        mat.diffuse_color = colour
        mat.use_nodes = True
        bsdf = mat.node_tree.nodes["Principled BSDF"]
        bsdf.inputs["Base Color"].default_value = colour
        bsdf.inputs["Roughness"].default_value = 0.9
    return mat


def mesh_object(name, verts, faces, mats=None, face_mats=None):
    """An object from raw vertices and faces; `face_mats` indexes into `mats` per face."""
    me = bpy.data.meshes.new(name)
    me.from_pydata([tuple(v) for v in verts], [], [tuple(f) for f in faces])
    me.validate()
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    for m in mats or []:
        me.materials.append(material(m))
    if face_mats is not None:
        for poly, i in zip(me.polygons, face_mats):
            poly.material_index = i
    flat(ob)
    return ob


def flat(ob):
    for poly in ob.data.polygons:
        poly.use_smooth = False


def assign(ob, name):
    ob.data.materials.clear()
    ob.data.materials.append(material(name))
    flat(ob)
    return ob


def join(objects, name):
    """Joins objects into one, keeping each one's material."""
    objects = [o for o in objects if o is not None]
    ctx = bpy.context
    for o in ctx.scene.objects:
        o.select_set(False)
    for o in objects:
        o.select_set(True)
    ctx.view_layer.objects.active = objects[0]
    bpy.ops.object.join()
    ob = ctx.view_layer.objects.active
    ob.name = name
    bpy.ops.object.transform_apply(location=False, rotation=True, scale=True)
    flat(ob)
    return ob


def ground_to_origin(ob):
    """Bakes the object's transform into its mesh, then puts its lowest point at z = 0,
    centred in x and y."""
    from mathutils import Matrix

    ob.data.transform(ob.matrix_world)
    ob.matrix_world = Matrix.Identity(4)
    pts = [v.co for v in ob.data.vertices]
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    ob.data.transform(Matrix.Translation(-Vector(((lo.x + hi.x) / 2, (lo.y + hi.y) / 2, lo.z))))
    ob.data.update()


# Checks ---------------------------------------------------------------------------------


def checks(ob, budget):
    """What a script can say about a model: size, triangles, loose parts, materials."""
    bm = bmesh.new()
    bm.from_mesh(ob.data)
    tris = sum(len(f.verts) - 2 for f in bm.faces)
    non_manifold = sum(1 for e in bm.edges if not e.is_manifold)
    # Loose parts: islands of connected faces.
    seen, islands = set(), 0
    for f in bm.faces:
        if f.index in seen:
            continue
        islands += 1
        stack = [f]
        while stack:
            g = stack.pop()
            if g.index in seen:
                continue
            seen.add(g.index)
            for e in g.edges:
                stack.extend(h for h in e.link_faces if h.index not in seen)
    bm.free()
    dims = ob.dimensions
    names = [m.name for m in ob.data.materials]
    off_palette = [n for n in names if n not in PALETTE]
    return {
        "triangles": tris,
        "budget": budget,
        "within_budget": tris <= budget,
        "size_m": [round(d, 2) for d in dims],
        "islands": islands,
        "non_manifold_edges": non_manifold,
        "materials": names,
        "off_palette": off_palette,
    }


# Rendering ------------------------------------------------------------------------------


def stage():
    """The game's light, a ground plane and the world colour."""
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_EEVEE"
    scene.eevee.taa_render_samples = 32
    scene.eevee.use_shadows = True
    scene.eevee.use_raytracing = True
    scene.eevee.use_fast_gi = True
    scene.view_settings.view_transform = "AgX"
    scene.view_settings.look = "AgX - Medium High Contrast"
    scene.render.film_transparent = False

    world = bpy.data.worlds.new("Sky")
    scene.world = world
    world.use_nodes = True
    bg = world.node_tree.nodes["Background"]
    bg.inputs["Color"].default_value = linear(SKY_COLOUR) + (1.0,)
    bg.inputs["Strength"].default_value = 0.55

    sun_data = bpy.data.lights.new("Sun", "SUN")
    sun_data.color = linear(SUN_COLOUR)
    sun_data.energy = 4.0
    sun_data.angle = math.radians(2.0)
    sun = bpy.data.objects.new("Sun", sun_data)
    scene.collection.objects.link(sun)
    sun.rotation_euler = (math.radians(52), 0, math.radians(-35))

    bpy.ops.mesh.primitive_plane_add(size=200, location=(0, 0, 0))
    plane = bpy.context.object
    plane.name = "Ground"
    mat = bpy.data.materials.new("GroundStage")
    mat.use_nodes = True
    mat.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = linear(GROUND) + (1.0,)
    mat.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 1.0
    plane.data.materials.append(mat)


def figure():
    """The sandbox's 1.8 m mannequin standing idle, to put beside a model for scale."""
    before = set(bpy.context.scene.objects)
    bpy.ops.import_scene.gltf(filepath=FIGURE)
    new = [o for o in bpy.context.scene.objects if o not in before]
    idle = next((a for a in bpy.data.actions if a.name.startswith("Idle_Loop")), None)
    for o in new:
        if o.type == "ARMATURE" and idle:
            ad = o.animation_data or o.animation_data_create()
            for track in list(ad.nla_tracks):
                ad.nla_tracks.remove(track)
            ad.action = idle
            if getattr(idle, "slots", None) and len(idle.slots):
                ad.action_slot = idle.slots[0]
        if o.type == "MESH":
            for slot in o.material_slots:
                slot.material = material("Cloth")
    bpy.context.scene.frame_set(8)
    root = [o for o in new if o.parent is None]
    return new, root


def bounds(objects):
    pts = []
    for ob in objects:
        if ob.type != "MESH":
            continue
        pts += [ob.matrix_world @ Vector(c) for c in ob.bound_box]
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    return lo, hi


def render_views(model, path, people=(), beside=None, size=400, views=None):
    """Renders a model from several angles into one strip. The figure stands beside it in
    the first view only, so it gives the scale without hiding the model in the others."""
    scene = bpy.context.scene
    scene.render.resolution_x = size
    scene.render.resolution_y = size
    cam = scene.camera
    if cam is None:
        cam = bpy.data.objects.new("Cam", bpy.data.cameras.new("Cam"))
        cam.data.lens = 50
        scene.collection.objects.link(cam)
        scene.camera = cam
    lo, hi = bounds([model])
    views = views or [(30, 12), (120, 12), (210, 12), (300, 45)]
    tiles = []
    for i, (yaw, pitch) in enumerate(views):
        show_figure = i == 0 and people
        for o in people:
            o.hide_render = not show_figure
        a, b = lo.copy(), hi.copy()
        if show_figure:
            a = Vector((min(a.x, beside.x - 0.4), min(a.y, beside.y - 0.3), 0))
            b = Vector((max(b.x, beside.x + 0.4), max(b.y, beside.y + 0.3), max(b.z, 1.85)))
        centre = (a + b) / 2
        radius = max((b - a).length / 2, 0.3)
        dist = radius / math.sin(cam.data.angle / 2) * 0.9
        y, p = math.radians(yaw), math.radians(pitch)
        offset = Vector((math.sin(y) * math.cos(p), -math.cos(y) * math.cos(p), math.sin(p))) * dist
        cam.location = centre + offset
        cam.rotation_euler = (centre - cam.location).to_track_quat("-Z", "Y").to_euler()
        tile = f"{path}.view{i}.png"
        scene.render.filepath = tile
        bpy.ops.render.render(write_still=True)
        tiles.append(tile)
    stitch(tiles, path)
    for t in tiles:
        os.remove(t)


def stitch(tiles, path):
    images = []
    for t in tiles:
        img = bpy.data.images.load(t)
        w, h = img.size
        images.append(np.array(img.pixels[:], dtype=np.float32).reshape(h, w, 4))
        bpy.data.images.remove(img)
    row = np.concatenate(images, axis=1)
    h, w = row.shape[:2]
    out = bpy.data.images.new("sheet", width=w, height=h, alpha=True)
    out.pixels = row.ravel()
    out.filepath_raw = path
    out.file_format = "PNG"
    out.save()
    bpy.data.images.remove(out)


def export(ob, path):
    for o in bpy.context.scene.objects:
        o.select_set(False)
    ob.select_set(True)
    bpy.context.view_layer.objects.active = ob
    bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_apply=True)


def finish(models, name, budget, with_figure=True):
    """Checks and exports each model, then renders a sheet: a row of views per model,
    each with the figure beside it for scale."""
    import json

    os.makedirs(OUT, exist_ok=True)
    report = {}
    for ob in models:
        ground_to_origin(ob)
        report[ob.name] = checks(ob, budget)
        export(ob, os.path.join(OUT, f"{name}-{ob.name}.glb"))
    stage()
    people, roots = figure() if with_figure else ([], [])
    rows = []
    for ob in models:
        for other in models:
            other.hide_render = other is not ob
        # Beside the model, a little behind it as seen in the first view.
        beside = Vector((ob.dimensions.x / 2 + 0.5, ob.dimensions.y / 2 * 0.5 + 0.2, 0))
        for r in roots:
            r.location = beside
        row = os.path.join(OUT, f"{name}.{ob.name}.png")
        render_views(ob, row, people, beside)
        rows.append(row)
    stack(rows, os.path.join(OUT, f"{name}.png"))
    for r in rows:
        os.remove(r)
    with open(os.path.join(OUT, f"{name}.json"), "w") as f:
        json.dump(report, f, indent=2)
    print("REPORT", json.dumps(report))


def stack(rows, path):
    images = []
    for t in rows:
        img = bpy.data.images.load(t)
        w, h = img.size
        images.append(np.array(img.pixels[:], dtype=np.float32).reshape(h, w, 4))
        bpy.data.images.remove(img)
    # Blender's pixel rows run bottom to top, so the first model goes last.
    sheet = np.concatenate(images[::-1], axis=0)
    h, w = sheet.shape[:2]
    out = bpy.data.images.new("sheet", width=w, height=h, alpha=True)
    out.pixels = sheet.ravel()
    out.filepath_raw = path
    out.file_format = "PNG"
    out.save()
    bpy.data.images.remove(out)
