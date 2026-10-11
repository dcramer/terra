"""A composed scene from the bake-off's models, to judge a look in a whole shot rather than
one model at a time: rolling ground, a grove, rocks, a fence, a camp with the workbench and
the villager, the wolf at the forest's edge, low sun and haze.

    run.sh scene.py graphic starter      # shading: graphic (D) or facets (A)
    run.sh scene.py graphic ashen        # the same models in another realm's colours

A realm is only a palette: the same palette roles given other colours.
"""

import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import bmesh
import bpy
from mathutils import Matrix, Vector

import look

shading, realm = (look.args() + ["graphic", "starter"])[:2]

# Each realm's colours for the palette roles the models use, plus its ground and sky.
REALMS = {
    "starter": {
        "colours": {},
        "ground": (0.55, 0.62, 0.30), "horizon": (0.98, 0.80, 0.58), "zenith": (0.45, 0.58, 0.82),
        "sun": (1.0, 0.82, 0.60), "haze": (0.96, 0.80, 0.62),
    },
    "ashen": {
        "colours": {
            "Leaves": (0.30, 0.27, 0.26), "LeavesDark": (0.20, 0.18, 0.18), "LeavesLight": (0.42, 0.38, 0.34),
            "Bark": (0.16, 0.14, 0.14), "Wood": (0.32, 0.28, 0.26), "WoodLight": (0.45, 0.40, 0.36),
            "WoodDark": (0.22, 0.19, 0.18), "RockPale": (0.62, 0.60, 0.58), "Rock": (0.48, 0.46, 0.45),
            "Moss": (0.80, 0.42, 0.20), "FurGrey": (0.40, 0.38, 0.38), "FurLight": (0.62, 0.60, 0.58),
        },
        "ground": (0.56, 0.54, 0.52), "horizon": (0.86, 0.76, 0.66), "zenith": (0.50, 0.50, 0.54),
        "sun": (1.0, 0.70, 0.50), "haze": (0.80, 0.72, 0.66),
    },
    "crystal": {
        "colours": {
            "Leaves": (0.30, 0.70, 0.72), "LeavesDark": (0.22, 0.45, 0.62), "LeavesLight": (0.62, 0.86, 0.80),
            "Bark": (0.36, 0.22, 0.48), "Wood": (0.50, 0.36, 0.52), "WoodLight": (0.70, 0.56, 0.70),
            "WoodDark": (0.30, 0.22, 0.38), "RockPale": (0.86, 0.80, 0.90), "Rock": (0.66, 0.58, 0.76),
            "Moss": (0.40, 0.90, 0.80), "FurGrey": (0.56, 0.52, 0.66), "Fur": (0.42, 0.38, 0.54),
        },
        "ground": (0.84, 0.74, 0.66), "horizon": (0.98, 0.72, 0.70), "zenith": (0.34, 0.30, 0.56),
        "sun": (1.0, 0.86, 0.80), "haze": (0.92, 0.72, 0.76),
    },
}
REALMS["cold"] = {
    "colours": {
        "Leaves": (0.24, 0.40, 0.38), "LeavesDark": (0.16, 0.28, 0.30), "LeavesLight": (0.86, 0.90, 0.94),
        "Bark": (0.30, 0.24, 0.22), "RockPale": (0.82, 0.86, 0.90), "Rock": (0.62, 0.66, 0.72),
        "Moss": (0.94, 0.96, 0.98), "FurGrey": (0.82, 0.84, 0.86), "Fur": (0.70, 0.72, 0.76),
    },
    "ground": (0.90, 0.93, 0.97), "horizon": (0.96, 0.86, 0.80), "zenith": (0.48, 0.62, 0.82),
    "sun": (1.0, 0.88, 0.78), "haze": (0.84, 0.88, 0.94), "shade": (0.50, 0.58, 0.80),
}
REALMS["gothic"] = {
    "colours": {
        "Leaves": (0.20, 0.16, 0.26), "LeavesDark": (0.12, 0.10, 0.18), "LeavesLight": (0.32, 0.26, 0.40),
        "Bark": (0.12, 0.10, 0.12), "Wood": (0.26, 0.20, 0.22), "WoodLight": (0.38, 0.30, 0.30),
        "WoodDark": (0.16, 0.12, 0.14), "RockPale": (0.46, 0.46, 0.54), "Rock": (0.34, 0.34, 0.42),
        "Moss": (0.40, 0.56, 0.48), "FurGrey": (0.30, 0.30, 0.36), "FurLight": (0.50, 0.50, 0.58),
    },
    "ground": (0.26, 0.26, 0.34), "horizon": (0.36, 0.30, 0.46), "zenith": (0.06, 0.06, 0.14),
    "sun": (0.70, 0.80, 1.0), "haze": (0.30, 0.28, 0.42), "shade": (0.30, 0.30, 0.50),
    "energy": 2.2, "exposure": 1.8,
}
R = REALMS[realm]


def height(x, y):
    """Real-world scale (D110): rolling hills tens of metres high, flattening out where the
    camp is, and a mountain range about a kilometre away."""
    d = math.hypot(x, y)
    flat = min(1.0, max(0.0, (d - 25.0) / 90.0)) ** 2
    hills = (16 * math.sin(x * 0.0045 + 0.7) * math.cos(y * 0.0038)
             + 6 * math.sin((x + y) * 0.013) + 1.5 * math.sin(x * 0.05) * math.cos(y * 0.045))
    far = min(1.0, max(0.0, (y - 950) / 450))
    far = far * far * (3 - 2 * far)
    ridge = (0.55 + 0.30 * math.sin(x * 0.0042 + 1.3) + 0.15 * math.cos(x * 0.0105 + 0.4))
    mountains = far * 300 * ridge
    return hills * flat + 0.4 * math.sin(x * 0.11) * math.cos(y * 0.09) + mountains


def ground():
    bm = bmesh.new()
    n, size = 360, 2400.0
    verts = {}
    for i in range(n + 1):
        for j in range(n + 1):
            x = -size / 2 + size * i / n
            y = -200 + (size * 0.75) * j / n
            verts[i, j] = bm.verts.new((x, y, height(x, y)))
    for i in range(n):
        for j in range(n):
            bm.faces.new((verts[i, j], verts[i + 1, j], verts[i + 1, j + 1], verts[i, j + 1]))
    me = bpy.data.meshes.new("Terrain")
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new("Terrain", me)
    bpy.context.scene.collection.objects.link(ob)
    look.PALETTE["Terrain"] = R["ground"]
    return ob


def load(path):
    before = set(bpy.context.scene.objects)
    bpy.ops.import_scene.gltf(filepath=os.path.join(look.HERE, path))
    new = [o for o in bpy.context.scene.objects if o not in before]
    ob = next(o for o in new if o.type == "MESH")
    ob.parent = None
    ob.data.transform(ob.matrix_world)
    ob.matrix_world = Matrix.Identity(4)
    for o in new:
        if o is not ob:
            bpy.data.objects.remove(o)
    return ob


def dress(ob):
    """Our shading and this realm's colours on every material slot."""
    for o in bpy.context.scene.objects:
        o.select_set(o is ob)
    bpy.context.view_layer.objects.active = ob
    if shading == "graphic":
        bpy.ops.object.shade_smooth_by_angle(angle=math.radians(50))
    else:
        bpy.ops.object.shade_flat()
    for slot in ob.material_slots:
        role = slot.material.name.split("-")[0].split(".")[0] if slot.material else "Terrain"
        colour = R["colours"].get(role, look.PALETTE[role])
        mat = bpy.data.materials.new(f"{role}-{shading}")
        mat.use_nodes = True
        nt = mat.node_tree
        bsdf = nt.nodes["Principled BSDF"]
        bsdf.inputs["Base Color"].default_value = look.linear(colour) + (1,)
        bsdf.inputs["Roughness"].default_value = 1.0 if shading == "graphic" else 0.9
        if shading == "graphic":
            bsdf.inputs["Specular IOR Level"].default_value = 0.0
        else:
            # The same distance fade as the game's fog, mixed in as emission.
            cam = nt.nodes.new("ShaderNodeCameraData")
            far = nt.nodes.new("ShaderNodeMapRange")
            far.inputs["From Min"].default_value, far.inputs["From Max"].default_value = 40.0, 1400.0
            far.inputs["To Max"].default_value = 0.85
            fog = nt.nodes.new("ShaderNodeEmission")
            fog.inputs[0].default_value = look.linear(R["haze"]) + (1,)
            mix = nt.nodes.new("ShaderNodeMixShader")
            nt.links.new(cam.outputs["View Z Depth"], far.inputs["Value"])
            nt.links.new(far.outputs["Result"], mix.inputs[0])
            nt.links.new(bsdf.outputs[0], mix.inputs[1])
            nt.links.new(fog.outputs[0], mix.inputs[2])
            nt.links.new(mix.outputs[0], nt.nodes["Material Output"].inputs[0])
        slot.material = mat


def graphic_post(scene):
    """Direction D after the render: the light on each pixel cut into three crisp bands,
    the shadow band tinted cool, times the surface's flat colour; then haze by distance,
    and the sky behind. A game shader would do the same sums per pixel."""
    vl = scene.view_layers[0]
    vl.use_pass_diffuse_direct = True
    vl.use_pass_diffuse_color = True
    vl.use_pass_mist = True
    vl.use_pass_environment = True
    scene.render.film_transparent = True
    scene.world.mist_settings.start = 40.0
    scene.world.mist_settings.depth = 1400.0
    scene.world.mist_settings.falloff = "LINEAR"
    g = bpy.data.node_groups.new("Graphic", "CompositorNodeTree")
    g.interface.new_socket("Image", in_out="OUTPUT", socket_type="NodeSocketColor")
    scene.compositing_node_group = g
    n = g.nodes
    rl = n.new("CompositorNodeRLayers")
    bw = n.new("CompositorNodeRGBToBW")
    scale = n.new("ShaderNodeMath")
    scale.operation = "MULTIPLY"
    scale.inputs[1].default_value = R.get("exposure", 1.0)
    ramp = n.new("ShaderNodeValToRGB")
    ramp.color_ramp.interpolation = "CONSTANT"
    st = ramp.color_ramp.elements
    st[0].position, st[0].color = 0.0, (0.42, 0.42, 0.62, 1)
    st[0].color = R.get("shade", (0.42, 0.42, 0.62)) + (1,)
    st[1].position, st[1].color = 0.33, (0.78, 0.76, 0.86, 1)
    st.new(0.62).color = (1.08, 1.02, 0.92, 1)
    times = n.new("ShaderNodeMix")
    times.data_type, times.blend_type = "RGBA", "MULTIPLY"
    times.inputs["Factor"].default_value = 1.0
    hazed = n.new("ShaderNodeMix")
    hazed.data_type = "RGBA"
    hazed.inputs["B"].default_value = look.linear(R["haze"]) + (1,)
    fade = n.new("ShaderNodeMath")
    fade.operation = "MULTIPLY"
    fade.inputs[1].default_value = 0.85
    sky = n.new("ShaderNodeMix")
    sky.data_type = "RGBA"
    out = n.new("NodeGroupOutput")
    L = g.links.new
    L(rl.outputs["Diffuse Direct"], bw.inputs[0])
    L(bw.outputs[0], scale.inputs[0])
    L(scale.outputs[0], ramp.inputs["Fac"])
    L(rl.outputs["Diffuse Color"], times.inputs["A"])
    L(ramp.outputs["Color"], times.inputs["B"])
    L(rl.outputs["Mist"], fade.inputs[0])
    L(fade.outputs[0], hazed.inputs["Factor"])
    L(times.outputs["Result"], hazed.inputs["A"])
    L(rl.outputs["Alpha"], sky.inputs["Factor"])
    L(rl.outputs["Environment"], sky.inputs["A"])
    L(hazed.outputs["Result"], sky.inputs["B"])
    L(sky.outputs["Result"], out.inputs[0])
    if os.environ.get("DEBUG_PASS"):
        L(rl.outputs["Diffuse Direct"], out.inputs[0])


def place(src, at, turn=0.0, scale=1.0):
    ob = src.copy()
    bpy.context.scene.collection.objects.link(ob)
    ob.location = (at[0], at[1], height(at[0], at[1]) - 0.05 * scale)
    ob.rotation_euler = (0, 0, turn)
    ob.scale = (scale, scale, scale)
    return ob


look.reset()
scene = bpy.context.scene
scene.render.engine = "BLENDER_EEVEE"
scene.eevee.taa_render_samples = 48
scene.eevee.use_shadows = True
scene.render.resolution_x, scene.render.resolution_y = 1600, 900
scene.view_settings.view_transform = "Standard" if shading == "graphic" else "AgX"

terrain = ground()
terrain.data.materials.append(look.material("Rock"))
terrain.material_slots[0].material.name = "Terrain"
models = {
    "tree": load("out/refined-Tree-Tree.glb"),
    "rock": load("out/refined-Rock-Rock300.glb"),
    "fence": load("out/refined-Fence-Fence.glb"),
    "bench": load("out/refined-Workbench-Workbench.glb"),
    "villager": load("out/refined-Villager-Villager.glb"),
    "wolf": load("out/refined-Wolf-Wolf3000.glb"),
}
for ob in [terrain, *models.values()]:
    dress(ob)
for ob in models.values():
    ob.hide_render = True
    ob.hide_viewport = True

rng = random.Random(7)
# Trees at real size, 14 to 22 m (the model is 5.5 m), in groves over the hills.
groves = [(rng.uniform(-600, 600), rng.uniform(40, 900)) for _ in range(26)] + [(-30, 45), (40, 60), (-70, 30)]
for gx, gy in groves:
    for _ in range(rng.randint(6, 16)):
        x, y = gx + rng.gauss(0, 22), gy + rng.gauss(0, 22)
        if math.hypot(x, y) < 22:
            continue
        place(models["tree"], (x, y), rng.uniform(0, math.tau), rng.uniform(2.5, 4.0)).hide_render = False
# Rocks from knee-high stones to boulders bigger than a house (the model is 1.4 m).
for _ in range(90):
    x, y = rng.uniform(-400, 400), rng.uniform(-10, 700)
    big = rng.random() < 0.18
    place(models["rock"], (x, y), rng.uniform(0, math.tau), rng.uniform(2.5, 6.0) if big else rng.uniform(0.3, 1.4)).hide_render = False
# The camp, all at true size: fence posts 1.3 m, a workbench, the villager at 1.78 m, a wolf.
for k in range(5):
    place(models["fence"], (-12 + k * 4.4, 9.0), math.radians(90) + 0.04 * k, 1.0).hide_render = False
place(models["bench"], (2.8, 3.5), math.radians(200)).hide_render = False
place(models["villager"], (0.0, 0.0), math.radians(170)).hide_render = False
place(models["wolf"], (-7.0, 12.0), math.radians(135), 1.0).hide_render = False

# Low warm sun from behind the camera's left shoulder, raking across the hills.
sun = bpy.data.objects.new("Sun", bpy.data.lights.new("Sun", "SUN"))
scene.collection.objects.link(sun)
sun.data.color = look.linear(R["sun"])
sun.data.energy = R.get("energy", 4.5 if shading == "graphic" else 5.0)
sun.data.angle = math.radians(1.5)
sun.rotation_euler = Vector((0.75, 0.55, -0.36)).to_track_quat("-Z", "Y").to_euler()

# Sky: horizon to zenith. Haze is in each material, fading with distance.
world = bpy.data.worlds.new("Sky")
scene.world = world
world.use_nodes = True
nt = world.node_tree
coords = nt.nodes.new("ShaderNodeTexCoord")
sep = nt.nodes.new("ShaderNodeSeparateXYZ")
ramp = nt.nodes.new("ShaderNodeValToRGB")
ramp.color_ramp.elements[0].color = look.linear(R["horizon"]) + (1,)
ramp.color_ramp.elements[1].position = 0.45
ramp.color_ramp.elements[1].color = look.linear(R["zenith"]) + (1,)
bg = nt.nodes["Background"]
bg.inputs["Strength"].default_value = 1.0 if shading == "graphic" else 0.8
nt.links.new(coords.outputs["Generated"], sep.inputs[0])
nt.links.new(sep.outputs["Z"], ramp.inputs[0])
nt.links.new(ramp.outputs[0], bg.inputs["Color"])
# The camera sees the full sky; the scene is lit by a dimmer copy of it, so shadows fall
# into the dark band instead of being filled in by the sky.
dim = nt.nodes.new("ShaderNodeBackground")
dim.inputs["Strength"].default_value = 0.35 if shading == "graphic" else 0.8
nt.links.new(ramp.outputs[0], dim.inputs["Color"])
path = nt.nodes.new("ShaderNodeLightPath")
pick = nt.nodes.new("ShaderNodeMixShader")
nt.links.new(path.outputs["Is Camera Ray"], pick.inputs[0])
nt.links.new(dim.outputs[0], pick.inputs[1])
nt.links.new(bg.outputs[0], pick.inputs[2])
nt.links.new(pick.outputs[0], nt.nodes["World Output"].inputs["Surface"])

cam = bpy.data.objects.new("Cam", bpy.data.cameras.new("Cam"))
cam.data.lens = 22
cam.data.clip_end = 5000
scene.collection.objects.link(cam)
scene.camera = cam
cam.location = (2.2, -5.0, height(2.2, -5.0) + 2.3)
target = Vector((-6.0, 40.0, height(-6.0, 40.0) + 4.0))
cam.rotation_euler = (target - cam.location).to_track_quat("-Z", "Y").to_euler()

if shading == "graphic":
    graphic_post(scene)
os.makedirs(look.OUT, exist_ok=True)
scene.render.filepath = os.path.join(look.OUT, f"scene-{shading}-{realm}.png")
bpy.ops.render.render(write_still=True)
print("SCENE", scene.render.filepath)
