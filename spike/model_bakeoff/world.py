"""Four real scenes for the mood board, built from the pipeline's models and the code kit,
at real size (D110), in either look:

    run.sh world.py medieval-town painted
    run.sh world.py techno-street graphic

painted: direction C made procedural. Colour stays a palette role per part, so realms can
recolour anything; the painted look is shared material detail (brush-stroke variation,
painted top light and rims) tinted by that colour, with soft warm light, bounce, occlusion,
haze and bloom. graphic: direction D, light cut into three crisp bands.
"""

import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import bpy
from mathutils import Matrix, Vector

import kit
import look

scene_name, mode = (look.args() + ["medieval-town", "painted"])[:2]
look.PALETTE.update(kit.ROLES)
rng = random.Random(11)


# Materials ------------------------------------------------------------------------------


def painted(role, colour):
    """A role's colour with painted detail: low-frequency colour drift, stretched
    brush-stroke variation, tops lit a little warmer, silhouettes caught by a soft rim."""
    mat = bpy.data.materials.new(f"{role}-painted")
    mat.use_nodes = True
    nt = mat.node_tree
    L = nt.links.new
    bsdf = nt.nodes["Principled BSDF"]
    bsdf.inputs["Roughness"].default_value = 0.8
    bsdf.inputs["Specular IOR Level"].default_value = 0.25
    geo = nt.nodes.new("ShaderNodeNewGeometry")
    base = nt.nodes.new("ShaderNodeRGB")
    base.outputs[0].default_value = look.linear(colour) + (1,)
    # Broad drift, so no surface is one dead flat value.
    drift = nt.nodes.new("ShaderNodeTexNoise")
    drift.inputs["Scale"].default_value = 0.35
    drift.inputs["Detail"].default_value = 1.0
    # Brush strokes: noise squashed along one axis, so it reads as strokes, not grain.
    stretch = nt.nodes.new("ShaderNodeVectorMath")
    stretch.operation = "MULTIPLY"
    stretch.inputs[1].default_value = (2.4, 2.4, 0.7)
    strokes = nt.nodes.new("ShaderNodeTexNoise")
    strokes.inputs["Scale"].default_value = 1.6
    strokes.inputs["Detail"].default_value = 3.0
    strokes.inputs["Roughness"].default_value = 0.6
    L(geo.outputs["Position"], stretch.inputs[0])
    L(stretch.outputs[0], strokes.inputs["Vector"])
    L(geo.outputs["Position"], drift.inputs["Vector"])
    vary = nt.nodes.new("ShaderNodeMath")
    vary.operation = "MULTIPLY_ADD"
    vary.inputs[1].default_value = 0.5
    L(drift.outputs["Fac"], vary.inputs[0])
    L(strokes.outputs["Fac"], vary.inputs[2])
    shade = nt.nodes.new("ShaderNodeMapRange")
    shade.inputs["From Min"].default_value, shade.inputs["From Max"].default_value = 0.45, 1.05
    shade.inputs["To Min"].default_value, shade.inputs["To Max"].default_value = 0.8, 1.15
    L(vary.outputs[0], shade.inputs["Value"])
    # Painted top light: faces turned up are a touch lighter and warmer.
    sep = nt.nodes.new("ShaderNodeSeparateXYZ")
    L(geo.outputs["Normal"], sep.inputs[0])
    top = nt.nodes.new("ShaderNodeMapRange")
    top.inputs["From Min"].default_value, top.inputs["From Max"].default_value = -0.2, 1.0
    top.inputs["To Min"].default_value, top.inputs["To Max"].default_value = 0.88, 1.08
    L(sep.outputs["Z"], top.inputs["Value"])
    lift = nt.nodes.new("ShaderNodeMath")
    lift.operation = "MULTIPLY"
    L(shade.outputs["Result"], lift.inputs[0])
    L(top.outputs["Result"], lift.inputs[1])
    tint = nt.nodes.new("ShaderNodeMix")
    tint.data_type, tint.blend_type = "RGBA", "MULTIPLY"
    tint.inputs["Factor"].default_value = 1.0
    L(base.outputs[0], tint.inputs["A"])
    L(lift.outputs[0], tint.inputs["B"])
    # A soft rim of painted light where a surface turns away.
    rim = nt.nodes.new("ShaderNodeLayerWeight")
    rim.inputs["Blend"].default_value = 0.25
    rimmed = nt.nodes.new("ShaderNodeMix")
    rimmed.data_type, rimmed.blend_type = "RGBA", "SCREEN"
    rimmed.inputs["B"].default_value = (0.35, 0.3, 0.25, 1)
    L(tint.outputs["Result"], rimmed.inputs["A"])
    rim_amount = nt.nodes.new("ShaderNodeMath")
    rim_amount.operation = "MULTIPLY"
    rim_amount.inputs[1].default_value = 0.35
    L(rim.outputs["Facing"], rim_amount.inputs[0])
    L(rim_amount.outputs[0], rimmed.inputs["Factor"])
    L(rimmed.outputs["Result"], bsdf.inputs["Base Color"])
    if role == "Cobble":
        # Cobbles: dark seams from a cell pattern on the ground.
        cells = nt.nodes.new("ShaderNodeTexVoronoi")
        cells.feature = "DISTANCE_TO_EDGE"
        cells.inputs["Scale"].default_value = 3.4
        L(geo.outputs["Position"], cells.inputs["Vector"])
        seam = nt.nodes.new("ShaderNodeMapRange")
        seam.inputs["From Min"].default_value, seam.inputs["From Max"].default_value = 0.0, 0.06
        seam.inputs["To Min"].default_value, seam.inputs["To Max"].default_value = 0.45, 1.0
        L(cells.outputs["Distance"], seam.inputs["Value"])
        dark = nt.nodes.new("ShaderNodeMix")
        dark.data_type, dark.blend_type = "RGBA", "MULTIPLY"
        dark.inputs["Factor"].default_value = 1.0
        L(rimmed.outputs["Result"], dark.inputs["A"])
        L(seam.outputs["Result"], dark.inputs["B"])
        L(dark.outputs["Result"], bsdf.inputs["Base Color"])
    if role in ("Water", "Glass"):
        bsdf.inputs["Roughness"].default_value = 0.08
        bsdf.inputs["Specular IOR Level"].default_value = 0.9
    if role in kit.GLOW:
        bsdf.inputs["Emission Color"].default_value = look.linear(colour) + (1,)
        bsdf.inputs["Emission Strength"].default_value = kit.GLOW[role]
    return mat


def flat_lit(role, colour):
    """For the graphic look: a plain colour; the banding happens after the render."""
    mat = bpy.data.materials.new(f"{role}-flat")
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = look.linear(colour) + (1,)
    bsdf.inputs["Roughness"].default_value = 1.0
    bsdf.inputs["Specular IOR Level"].default_value = 0.0
    if role in kit.GLOW:
        bsdf.inputs["Emission Color"].default_value = look.linear(colour) + (1,)
        bsdf.inputs["Emission Strength"].default_value = kit.GLOW[role]
    return mat


MATERIALS = {}


def material(role, colours):
    colour = colours.get(role, look.PALETTE.get(role, (0.6, 0.6, 0.6)))
    key = (role, colour)
    if key not in MATERIALS:
        MATERIALS[key] = (painted if mode == "painted" else flat_lit)(role, colour)
    return MATERIALS[key]


def role_of(mat):
    return mat.name.split("-")[0].split(".")[0] if mat else "Stone"


def dress(ob, colours, smooth=True):
    for o in bpy.context.scene.objects:
        o.select_set(o is ob)
    bpy.context.view_layer.objects.active = ob
    if smooth:
        bpy.ops.object.shade_smooth_by_angle(angle=math.radians(40))
    else:
        bpy.ops.object.shade_flat()
    for slot in ob.material_slots:
        slot.material = material(role_of(slot.material), colours)


# Models -----------------------------------------------------------------------------------


MODELS = {}


def model(name):
    """A finished model from the pipeline, loaded once and hidden; place() copies it."""
    if name in MODELS:
        return MODELS[name]
    path = os.path.join(look.OUT, f"refined-{name}-{name}.glb")
    if not os.path.exists(path):
        MODELS[name] = None
        return None
    before = set(bpy.context.scene.objects)
    bpy.ops.import_scene.gltf(filepath=path)
    new = [o for o in bpy.context.scene.objects if o not in before]
    ob = next(o for o in new if o.type == "MESH")
    ob.parent = None
    ob.data.transform(ob.matrix_world)
    ob.matrix_world = Matrix.Identity(4)
    for o in new:
        if o is not ob:
            bpy.data.objects.remove(o)
    ob.hide_render = True
    MODELS[name] = ob
    return ob


def place(name, at, turn=0.0, scale=1.0, colours=None, ground=None):
    """A copy of a model, standing on the ground, optionally in its own colours."""
    src = model(name)
    if src is None:
        return None
    ob = src.copy()
    bpy.context.scene.collection.objects.link(ob)
    z = ground(at[0], at[1]) if ground else 0.0
    ob.location = (at[0], at[1], z + (at[2] if len(at) > 2 else 0.0))
    ob.rotation_euler = (0, 0, turn)
    ob.scale = (scale, scale, scale)
    ob.hide_render = False
    if colours:
        for slot in ob.material_slots:
            slot.link = "OBJECT"
            slot.material = material(role_of(src.material_slots[slot.slot_index].material), colours)
    return ob


# The stage --------------------------------------------------------------------------------


def terrain(name, size, n, height, role, colours, centre=(0, 0)):
    import bmesh

    bm = bmesh.new()
    verts = {}
    for i in range(n + 1):
        for j in range(n + 1):
            x = centre[0] - size / 2 + size * i / n
            y = centre[1] - size / 2 + size * j / n
            verts[i, j] = bm.verts.new((x, y, height(x, y)))
    for i in range(n):
        for j in range(n):
            bm.faces.new((verts[i, j], verts[i + 1, j], verts[i + 1, j + 1], verts[i, j + 1]))
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    me.materials.append(material(role, colours))
    for p in me.polygons:
        p.use_smooth = True
    return ob


def sky_and_light(sun_dir, sun_colour, sun_energy, horizon, zenith, ambient):
    scene = bpy.context.scene
    sun = bpy.data.objects.new("Sun", bpy.data.lights.new("Sun", "SUN"))
    scene.collection.objects.link(sun)
    sun.data.color = look.linear(sun_colour)
    sun.data.energy = sun_energy
    sun.data.angle = math.radians(2.5 if mode == "painted" else 1.0)
    sun.rotation_euler = Vector(sun_dir).to_track_quat("-Z", "Y").to_euler()
    world = bpy.data.worlds.new("Sky")
    scene.world = world
    world.use_nodes = True
    nt = world.node_tree
    coords = nt.nodes.new("ShaderNodeTexCoord")
    sep = nt.nodes.new("ShaderNodeSeparateXYZ")
    ramp = nt.nodes.new("ShaderNodeValToRGB")
    ramp.color_ramp.elements[0].color = look.linear(horizon) + (1,)
    ramp.color_ramp.elements[1].position = 0.5
    ramp.color_ramp.elements[1].color = look.linear(zenith) + (1,)
    nt.links.new(coords.outputs["Generated"], sep.inputs[0])
    nt.links.new(sep.outputs["Z"], ramp.inputs[0])
    seen = nt.nodes["Background"]
    seen.inputs["Strength"].default_value = 1.0
    nt.links.new(ramp.outputs[0], seen.inputs["Color"])
    lit = nt.nodes.new("ShaderNodeBackground")
    lit.inputs["Strength"].default_value = ambient
    nt.links.new(ramp.outputs[0], lit.inputs["Color"])
    path = nt.nodes.new("ShaderNodeLightPath")
    pick = nt.nodes.new("ShaderNodeMixShader")
    nt.links.new(path.outputs["Is Camera Ray"], pick.inputs[0])
    nt.links.new(lit.outputs[0], pick.inputs[1])
    nt.links.new(seen.outputs[0], pick.inputs[2])
    nt.links.new(pick.outputs[0], nt.nodes["World Output"].inputs["Surface"])


def point_light(at, colour, power, radius=0.15):
    lamp = bpy.data.objects.new("Lamp", bpy.data.lights.new("Lamp", "POINT"))
    bpy.context.scene.collection.objects.link(lamp)
    lamp.location = at
    lamp.data.color = look.linear(colour)
    lamp.data.energy = power
    lamp.data.shadow_soft_size = radius


def camera(at, target, lens=24.0):
    scene = bpy.context.scene
    cam = bpy.data.objects.new("Cam", bpy.data.cameras.new("Cam"))
    cam.data.lens = lens
    cam.data.clip_end = 6000
    scene.collection.objects.link(cam)
    scene.camera = cam
    cam.location = at
    cam.rotation_euler = (Vector(target) - Vector(at)).to_track_quat("-Z", "Y").to_euler()


def post(haze, mist_start, mist_depth, haze_amount, bands=None):
    """After the render: haze by distance, the sky behind, bloom; in the graphic look, the
    light cut into crisp bands first."""
    scene = bpy.context.scene
    vl = scene.view_layers[0]
    vl.use_pass_mist = True
    vl.use_pass_environment = True
    vl.use_pass_emit = True
    vl.use_pass_diffuse_direct = True
    vl.use_pass_diffuse_color = True
    scene.render.film_transparent = True
    ms = scene.world.mist_settings
    ms.start, ms.depth, ms.falloff = mist_start, mist_depth, "QUADRATIC"
    g = bpy.data.node_groups.new("Post", "CompositorNodeTree")
    g.interface.new_socket("Image", in_out="OUTPUT", socket_type="NodeSocketColor")
    scene.compositing_node_group = g
    n, L = g.nodes, g.links.new
    rl = n.new("CompositorNodeRLayers")
    surface = rl.outputs["Image"]
    if mode == "graphic":
        bw = n.new("CompositorNodeRGBToBW")
        ramp = n.new("ShaderNodeValToRGB")
        ramp.color_ramp.interpolation = "CONSTANT"
        st = ramp.color_ramp.elements
        st[0].color = (bands or (0.42, 0.42, 0.62)) + (1,)
        st[1].position, st[1].color = 0.33, (0.78, 0.76, 0.86, 1)
        st.new(0.62).color = (1.08, 1.02, 0.92, 1)
        times = n.new("ShaderNodeMix")
        times.data_type, times.blend_type = "RGBA", "MULTIPLY"
        times.inputs["Factor"].default_value = 1.0
        glow = n.new("ShaderNodeMix")
        glow.data_type, glow.blend_type = "RGBA", "ADD"
        glow.inputs["Factor"].default_value = 1.0
        L(rl.outputs["Diffuse Direct"], bw.inputs[0])
        L(bw.outputs[0], ramp.inputs["Fac"])
        L(rl.outputs["Diffuse Color"], times.inputs["A"])
        L(ramp.outputs["Color"], times.inputs["B"])
        L(times.outputs["Result"], glow.inputs["A"])
        L(rl.outputs["Emission"], glow.inputs["B"])
        surface = glow.outputs["Result"]
    fade = n.new("ShaderNodeMath")
    fade.operation = "MULTIPLY"
    fade.inputs[1].default_value = haze_amount
    hazed = n.new("ShaderNodeMix")
    hazed.data_type = "RGBA"
    hazed.inputs["B"].default_value = look.linear(haze) + (1,)
    L(rl.outputs["Mist"], fade.inputs[0])
    L(fade.outputs[0], hazed.inputs["Factor"])
    L(surface, hazed.inputs["A"])
    sky = n.new("ShaderNodeMix")
    sky.data_type = "RGBA"
    L(rl.outputs["Alpha"], sky.inputs["Factor"])
    L(rl.outputs["Environment"], sky.inputs["A"])
    L(hazed.outputs["Result"], sky.inputs["B"])
    result = sky.outputs["Result"]
    try:
        bloom = n.new("CompositorNodeGlare")
        if "Type" in bloom.inputs:
            bloom.inputs["Type"].default_value = "Bloom"
        else:
            bloom.glare_type = "BLOOM"
        for key, value in (("Threshold", 0.9), ("Strength", 0.35 if mode == "painted" else 0.2), ("Size", 0.6)):
            if key in bloom.inputs:
                bloom.inputs[key].default_value = value
        L(result, bloom.inputs["Image"])
        result = bloom.outputs["Image"]
    except Exception as e:
        print("no bloom:", e)
    out = n.new("NodeGroupOutput")
    L(result, out.inputs[0])


def render(name):
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_EEVEE"
    scene.eevee.taa_render_samples = 64
    scene.eevee.use_shadows = True
    scene.eevee.use_raytracing = mode == "painted"
    scene.eevee.use_fast_gi = mode == "painted"
    scene.render.resolution_x, scene.render.resolution_y = 1920, 1080
    if mode == "painted":
        scene.view_settings.view_transform = "AgX"
        scene.view_settings.look = "AgX - Medium High Contrast"
    else:
        scene.view_settings.view_transform = "Standard"
    os.makedirs(look.OUT, exist_ok=True)
    scene.render.filepath = os.path.join(look.OUT, f"world-{name}-{mode}.png")
    bpy.ops.render.render(write_still=True)
    print("WORLD", scene.render.filepath)


# Scenes -----------------------------------------------------------------------------------


def medieval_town():
    """A market street in warm afternoon light: jettied houses either side, a square with a
    well and a stall, lanterns, townsfolk in their own colours, a cart's barrels."""
    colours = {}
    flat = lambda x, y: 0.02 * math.sin(x * 0.7) * math.cos(y * 0.6)
    dress(terrain("Street", 220, 110, flat, "Cobble", colours), colours)
    for side in (-1, 1):
        y = -6.0
        while y < 130:
            w = rng.uniform(5.5, 8.5)
            d = rng.uniform(7.0, 9.0)
            medieval_house(f"House{side}{y:.0f}", (side * (4.2 + d / 2), y + w / 2, 0), math.radians(-90 * side),
                           w, d, rng.randint(1, 3), rng)
            y += w + (rng.uniform(0.8, 1.6) if rng.random() < 0.3 else 0.05)
    place("Stall", (-2.4, 22.0), math.radians(90), 1.0, ground=flat)
    place("Well", (1.6, 36.0), 0.4, 1.0, ground=flat)
    for i, (x, y) in enumerate(((-3.3, 14), (-3.5, 15.1), (3.2, 27), (3.5, 28.1), (3.6, 26.4))):
        place("Barrel", (x, y), rng.uniform(0, 6), 1.0, ground=flat)
    for y in (8, 30, 52):
        for side in (-1, 1):
            place("Lantern", (side * 3.7, y), 0.0, 1.0, ground=flat)
    place("Workbench", (3.5, 18.0), math.radians(-90), 1.0, ground=flat)
    tunics = [{"Red": (0.25, 0.42, 0.62)}, {"Red": (0.42, 0.52, 0.28)}, {"Red": (0.72, 0.58, 0.30)}, {}, {"Red": (0.50, 0.30, 0.50)}]
    for i, (x, y, t) in enumerate(((0.4, 0.0, 170), (-1.4, 18.0, 120), (-2.0, 20.5, 300), (2.2, 31.0, 200), (0.6, 45.0, 180))):
        place("Villager", (x, y), math.radians(t), rng.uniform(0.95, 1.05), colours=tunics[i % len(tunics)], ground=flat)
    place("Wolf", (2.4, 9.0), math.radians(200), 0.8, colours={"FurGrey": (0.55, 0.40, 0.28), "Fur": (0.45, 0.32, 0.22)}, ground=flat)
    sky_and_light((0.55, 0.65, -0.5), (1.0, 0.86, 0.66), 4.5, (0.98, 0.84, 0.66), (0.45, 0.62, 0.86), 0.45)
    post((0.95, 0.84, 0.70), 20.0, 260.0, 0.6)
    camera((1.2, -8.5, 1.75), (-0.4, 30.0, 4.8), lens=22)


def techno_street():
    """A canyon of modular blocks at dusk: neon, lit windows, cables overhead, vending
    machines, a hover bike, a drone, the explorer, a fabricator in a shopfront."""
    colours = {}
    flat = lambda x, y: 0.0
    dress(terrain("Street", 260, 60, flat, "ConcreteDark", colours), colours)
    for side in (-1, 1):
        y = -10.0
        while y < 160:
            w = rng.uniform(10, 16)
            tech_block(f"Block{side}{y:.0f}", (side * (8.0 + 6.0), y + w / 2, 0), math.radians(-90 * side),
                       w, 12.0, rng.randint(5, 12), rng)
            y += w + rng.uniform(0.0, 2.0)
    for y in range(4, 120, 9):
        cable(f"Cable{y}", (-8.2, y, rng.uniform(9, 16)), (8.2, y + rng.uniform(-3, 3), rng.uniform(9, 16)), sag=1.4)
    for x, y, t in ((-7.0, 12, 90), (-7.0, 13.4, 90), (7.0, 30, -90)):
        place("Vending", (x, y), math.radians(t), 1.0, ground=flat)
    for y in range(6, 110, 16):
        for side in (-1, 1):
            place("Streetlamp", (side * 6.8, y + (4 if side > 0 else 0)), math.radians(-90 * side), 1.0, ground=flat)
            point_light((side * 6.0, y + (4 if side > 0 else 0), 5.5), (0.5, 0.9, 1.0), 220)
    place("Hoverbike", (3.0, 9.0, 0.6), math.radians(-20), 1.0, ground=flat)
    place("Drone", (-1.5, 15.0, 4.0), math.radians(160), 0.7, ground=flat)
    place("Explorer", (0.3, 2.0), math.radians(175), 1.0, ground=flat)
    place("Fabricator", (-6.5, 22.0), math.radians(90), 1.0, ground=flat)
    sky_and_light((0.3, 0.8, -0.25), (1.0, 0.55, 0.45), 1.2, (0.92, 0.48, 0.42), (0.10, 0.10, 0.22), 0.25)
    post((0.30, 0.22, 0.34), 25.0, 300.0, 0.7, bands=(0.32, 0.30, 0.55))
    camera((1.6, -8.0, 1.75), (-0.5, 40.0, 7.0), lens=20)


def fantasy_wilds():
    """Real-scale wilds in daylight: giant mushrooms, crystal clusters, a standing-stone
    arch, towering trees, a river, a traveller and a wolf for scale, mountains in haze."""
    colours = {"Leaves": (0.36, 0.62, 0.30), "LeavesLight": (0.62, 0.78, 0.36)}

    def ground(x, y):
        d = math.hypot(x, y)
        flat = min(1.0, max(0.0, (d - 20) / 80.0)) ** 2
        hills = 14 * math.sin(x * 0.006 + 0.4) * math.cos(y * 0.005) + 5 * math.sin((x + y) * 0.017)
        far = min(1.0, max(0.0, (y - 700) / 500))
        mountains = far * far * (3 - 2 * far) * 420 * (0.6 + 0.4 * math.sin(x * 0.004 + 2.0))
        river = -2.5 * math.exp(-((x - 0.15 * y + 30) / 7.0) ** 2)
        return hills * flat + river + mountains + 0.3 * math.sin(x * 0.2) * math.cos(y * 0.17)

    look.PALETTE["Meadow"] = (0.52, 0.66, 0.30)
    dress(terrain("Land", 2600, 380, ground, "Meadow", colours, centre=(0, 900)), colours)
    water = terrain("River", 2600, 4, lambda x, y: -1.2, "Water", colours, centre=(0, 900))
    for _ in range(90):
        x, y = rng.uniform(-500, 500), rng.uniform(30, 900)
        place("Tree", (x, y), rng.uniform(0, 6.3), rng.uniform(3.0, 4.5), colours=colours, ground=ground)
    for x, y, s in ((-14, 28, 4.5), (-22, 40, 6.0), (18, 55, 5.0), (-8, 70, 3.5), (30, 34, 3.0)):
        place("Mushroom", (x, y), rng.uniform(0, 6.3), s, ground=ground)
    for x, y, s in ((9, 20, 2.0), (12, 23, 1.3), (-30, 60, 3.0), (40, 90, 4.0)):
        place("Crystals", (x, y), rng.uniform(0, 6.3), s, ground=ground)
    place("Runearch", (-2.0, 44.0), math.radians(10), 2.6, ground=ground)
    place("Flora", (5.0, 14.0), 0.3, 2.2, ground=ground)
    place("Flora", (-6.0, 18.0), 1.4, 1.8, ground=ground)
    for _ in range(25):
        place("Rock", (rng.uniform(-120, 120), rng.uniform(5, 300)), rng.uniform(0, 6.3), rng.uniform(0.5, 4.0), ground=ground)
    place("Villager", (0.8, 2.0), math.radians(170), 1.0, colours={"Red": (0.26, 0.40, 0.58)}, ground=ground)
    place("Wolf", (3.0, 4.5), math.radians(150), 1.0, ground=ground)
    sky_and_light((0.45, 0.6, -0.65), (1.0, 0.95, 0.85), 4.0, (0.92, 0.90, 0.86), (0.38, 0.60, 0.92), 0.5)
    post((0.80, 0.86, 0.94), 40.0, 1600.0, 0.85)
    camera((3.5, -9.0, ground(3.5, -9.0) + 2.2), (-3.0, 60.0, ground(-3.0, 60.0) + 9.0), lens=20)


def gothic_graveyard():
    """A hill by moonlight: a ruined chapel, crooked gravestones inside iron railings, dead
    trees, lanterns glowing warm in the mist."""
    colours = {"Leaves": (0.18, 0.16, 0.22), "Bark": (0.14, 0.12, 0.14)}

    def ground(x, y):
        hill = 9.0 * math.exp(-((x / 70) ** 2 + ((y - 40) / 80) ** 2))
        return hill + 0.25 * math.sin(x * 0.3) * math.cos(y * 0.27) + 6 * math.sin(x * 0.01) * math.cos(y * 0.008)

    look.PALETTE["Grave"] = (0.30, 0.34, 0.30)
    dress(terrain("Hill", 900, 220, ground, "Grave", colours, centre=(0, 200)), colours)
    chapel_ruin("Chapel", (6.0, 52.0, ground(6.0, 52.0) - 0.3), math.radians(8), rng)
    for i in range(46):
        x, y = rng.uniform(-22, 18), rng.uniform(8, 40)
        if abs(x - 6) < 7 and y > 38:
            continue
        place("Gravestone", (x, y), math.radians(rng.uniform(-25, 25)) + math.pi, rng.uniform(0.9, 1.2), ground=ground)
    for (a, b) in (((-24, 6), (20, 6)), ((-24, 6), (-24, 42)), ((20, 6), (20, 42))):
        kit.iron_fence(f"Rail{a}{b}", (a[0], a[1], ground(*a)), (b[0], b[1], ground(*b)))
    for x, y, s in ((-30, 20, 1.3), (26, 30, 1.5), (-12, 60, 1.8), (34, 70, 1.6), (-40, 50, 2.0)):
        place("Deadtree", (x, y), rng.uniform(0, 6.3), s, ground=ground)
    for x, y in ((-3.0, 7.0), (4.0, 7.0), (-1.0, 30.0)):
        place("Lantern", (x, y), 0.0, 1.0, ground=ground)
        point_light((x, y, ground(x, y) + 2.6), (1.0, 0.7, 0.4), 140, 0.2)
    place("Villager", (0.6, 3.0), math.radians(175), 1.0, colours={"Red": (0.20, 0.18, 0.24), "Cloth": (0.30, 0.28, 0.32)}, ground=ground)
    place("Wolf", (-9.0, 22.0), math.radians(120), 1.1, colours={"FurGrey": (0.24, 0.24, 0.28), "Fur": (0.18, 0.18, 0.22), "FurLight": (0.40, 0.40, 0.46)}, ground=ground)
    sky_and_light((-0.4, 0.7, -0.45), (0.62, 0.74, 1.0), 1.4, (0.30, 0.30, 0.46), (0.04, 0.05, 0.12), 0.3)
    post((0.26, 0.28, 0.40), 12.0, 220.0, 0.75, bands=(0.28, 0.30, 0.52))
    camera((2.0, -6.0, ground(2.0, -6.0) + 1.9), (3.0, 40.0, ground(3.0, 40.0) + 4.5), lens=22)


medieval_house, tech_block, chapel_ruin, cable = kit.medieval_house, kit.tech_block, kit.chapel_ruin, kit.cable
look.reset()
look.PALETTE.update(kit.ROLES)
{"medieval-town": medieval_town, "techno-street": techno_street,
 "fantasy-wilds": fantasy_wilds, "gothic-graveyard": gothic_graveyard}[scene_name]()
# Kit buildings and terrain were made with bare role materials; dress everything not yet dressed.
for ob in list(bpy.context.scene.objects):
    if ob.type == "MESH" and not ob.hide_render and ob.material_slots and ob.material_slots[0].material \
            and not ob.material_slots[0].material.name.endswith(("-painted", "-flat")):
        dress(ob, {}, smooth=False)
render(scene_name)
