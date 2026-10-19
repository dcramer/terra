# Simulates one fire set in Mantaflow and renders its raw passes in Cycles (docs/fire.md,
# "The baker"; D415), run by bake.ts through tools/blender/run.sh:
#
#   tools/blender/run.sh tools/fire/bake/bake.py <recipe.json> <out dir>
#
# The recipe (tools/fire/bake/sets/<set>.json) says the domain, the fuel sources, the sim's
# settings, how many frames and the camera. Every frame from the warm-up on is rendered three
# times, each an emission-and-absorption volume with no scattering, so each pixel is an exact
# line integral through the sim's grids (f the flame, d the smoke density, v the velocity):
#
#   heat:   absorbed by kf*f + ks*d, emits (f*f, f, d)        -> heat = R/G, f's own mean
#   motion: absorbed the same, emits w*(0.5+s*vx, 0.5+s*vz, 1) with w = f + d
#   depth:  no absorption, emits (kf*f, ks*d*(1 - min(1, m*f)), 0) -> cover and soot depths
#
# It writes raw.bin (every frame's three passes, RGB half floats, frame by frame) and meta.json;
# bake.ts packs them. Our own script (D121): nothing of Blender's or anyone's is copied in.

import json
import os
import sys
import time

import bpy
import numpy as np

recipe_path, out = sys.argv[sys.argv.index("--") + 1:][:2]
with open(recipe_path) as f:
    recipe = json.load(f)
os.makedirs(out, exist_ok=True)
os.chdir(out)  # Mantaflow writes its noise tile (waveletNoiseTile.bin) where it runs
dom_r = recipe["domain"]
frames = recipe["frames"]
first = recipe.get("warmup", 0) + 1
last = first + frames + recipe.get("crossfade", 0) - 1
passes_r = recipe.get("passes", {})
KF = passes_r.get("cover", 12.0)
KS = passes_r.get("soot", 3.0)
MASK = passes_r.get("flame_mask", 4.0)
VMAX = passes_r.get("velocity_max", 12.0)

scene = bpy.context.scene
bpy.context.preferences.edit.keyframe_new_interpolation_type = "LINEAR"
for o in list(bpy.data.objects):
    bpy.data.objects.remove(o)
scene.frame_start, scene.frame_end = 1, last
scene.render.fps = recipe.get("fps", 24)

# The domain: a box of gas, its settings as the recipe gives them.
bpy.ops.mesh.primitive_cube_add(size=1, location=dom_r["center"])
dom = bpy.context.object
dom.scale = dom_r["size"]
bpy.ops.object.transform_apply(scale=True)
ds = dom.modifiers.new("Fluid", "FLUID")
ds.fluid_type = "DOMAIN"
ds = ds.domain_settings
ds.domain_type = "GAS"
ds.cache_type = "ALL"
ds.cache_directory = os.path.join(out, "cache")
ds.cache_frame_start, ds.cache_frame_end = 1, last
ds.resolution_max = dom_r.get("resolution", 64)
ds.use_noise = dom_r.get("noise", 0) > 1
if ds.use_noise:
    ds.noise_scale = dom_r["noise"]
    ds.noise_strength = dom_r.get("noise_strength", 1.0)
for key in ("alpha", "beta", "vorticity", "burning_rate", "flame_smoke", "flame_vorticity",
            "flame_ignition", "flame_max_temp", "time_scale", "cfl_condition"):
    if key in dom_r:
        setattr(ds, key, dom_r[key])
if "dissolve" in dom_r:
    ds.use_dissolve_smoke = True
    ds.dissolve_speed = dom_r["dissolve"]
    ds.use_dissolve_smoke_log = dom_r.get("dissolve_log", True)
ds.use_adaptive_domain = False
# Open on every side but the floor, so smoke leaves the frame rather than filling the box.
for side in ("top", "front", "back", "left", "right"):
    setattr(ds, "use_collision_border_" + side, False)
ds.use_collision_border_bottom = dom_r.get("floor", True)

# The fuel: meshes that emit fire, smoke or both, broken up by a moving cloud texture so the
# inflow flickers as wood does, and switched off at `until` for a one-shot.
SHAPES = {
    "sphere": lambda: bpy.ops.mesh.primitive_uv_sphere_add(radius=1, segments=24, ring_count=12),
    "cylinder": lambda: bpy.ops.mesh.primitive_cylinder_add(radius=1, depth=2, vertices=24),
    "cube": lambda: bpy.ops.mesh.primitive_cube_add(size=2),
    "plane": lambda: bpy.ops.mesh.primitive_plane_add(size=2),
}
for i, s in enumerate(recipe["sources"]):
    SHAPES[s["shape"]]()
    src = bpy.context.object
    src.location = s["location"]
    src.rotation_euler = s.get("rotation", (0, 0, 0))
    src.scale = s.get("scale", (1, 1, 1))
    src.hide_render = True
    fl = src.modifiers.new("Fluid", "FLUID")
    fl.fluid_type = "FLOW"
    fs = fl.flow_settings
    fs.flow_type = s.get("type", "BOTH")
    fs.flow_behavior = s.get("behavior", "INFLOW")
    fs.flow_source = "MESH"
    fs.surface_distance = s.get("surface", 1.0)
    fs.use_plane_init = s["shape"] == "plane"
    fs.fuel_amount = s.get("fuel", 1.0)
    fs.temperature = s.get("temperature", 1.0)
    fs.density = s.get("density", 1.0)
    fs.subframes = s.get("subframes", 0)
    if "velocity" in s or "normal" in s:
        fs.use_initial_velocity = True
        fs.velocity_coord = s.get("velocity", (0, 0, 0))
        fs.velocity_normal = s.get("normal", 0.0)
        fs.velocity_random = s.get("random", 0.0)
    if "texture" in s:
        t = s["texture"]
        tex = bpy.data.textures.new("fuel%d" % i, "CLOUDS")
        tex.noise_scale = t.get("scale", 0.25)
        tex.noise_depth = t.get("depth", 2)
        tex.contrast = t.get("contrast", 1.5)
        fs.use_texture = True
        fs.noise_texture = tex
        fs.texture_map_type = "AUTO"
        fs.texture_size = t.get("size", 1.0)
        speed = t.get("speed", 0.02)
        fs.texture_offset = 0.0
        fs.keyframe_insert("texture_offset", frame=1)
        fs.texture_offset = speed * last
        fs.keyframe_insert("texture_offset", frame=last)
    if "until" in s:
        fs.use_inflow = True
        fs.keyframe_insert("use_inflow", frame=s["until"])
        fs.use_inflow = False
        fs.keyframe_insert("use_inflow", frame=s["until"] + 1)

t0 = time.time()
with bpy.context.temp_override(object=dom, active_object=dom, selected_objects=[dom], scene=scene):
    bpy.ops.fluid.bake_all()
sim_s = time.time() - t0

# The passes: one node tree, its volume output relinked for each.
mat = bpy.data.materials.new("passes")
dom.data.materials.append(mat)
nt = mat.node_tree
nt.nodes.clear()
N, L = nt.nodes, nt.links
output = N.new("ShaderNodeOutputMaterial")


def attr(name, socket="Fac"):
    a = N.new("ShaderNodeAttribute")
    a.attribute_type = "GEOMETRY"
    a.attribute_name = name
    return a.outputs[socket]


def math(op, a, b=None, clamp=False):
    m = N.new("ShaderNodeMath")
    m.operation = op
    m.use_clamp = clamp
    for k, x in enumerate((a, b)):
        if x is None:
            continue
        if isinstance(x, (int, float)):
            m.inputs[k].default_value = x
        else:
            L.new(x, m.inputs[k])
    return m.outputs[0]


def rgb(r, g, b):
    c = N.new("ShaderNodeCombineColor")
    for k, x in enumerate((r, g, b)):
        if isinstance(x, (int, float)):
            c.inputs[k].default_value = x
        else:
            L.new(x, c.inputs[k])
    return c.outputs[0]


def volume(colour, absorb):
    e = N.new("ShaderNodeEmission")
    L.new(colour, e.inputs["Color"])
    e.inputs["Strength"].default_value = 1.0
    if absorb is None:
        return e.outputs[0]
    a = N.new("ShaderNodeVolumeAbsorption")
    a.inputs["Color"].default_value = (0, 0, 0, 1)
    L.new(absorb, a.inputs["Density"])
    add = N.new("ShaderNodeAddShader")
    L.new(a.outputs[0], add.inputs[0])
    L.new(e.outputs[0], add.inputs[1])
    return add.outputs[0]


f = math("MAXIMUM", attr(recipe.get("heat_attr", "flame")), 0.0)
d = math("MAXIMUM", attr("density"), 0.0)
CELL = max(dom_r["size"]) / ds.resolution_max
sep = N.new("ShaderNodeSeparateXYZ")
L.new(attr("velocity", "Vector"), sep.inputs[0])
sigma = math("ADD", math("MULTIPLY", f, KF), math("MULTIPLY", d, KS))
w = math("ADD", f, d)


def enc(v):
    """A velocity component as 0 to 1 (0.5 is still), times the weight w. Mantaflow's velocity
    is in cells a second, so a cell's size makes it metres a second."""
    return math("MULTIPLY", math("ADD", math("MULTIPLY", v, 0.5 * CELL / VMAX), 0.5, clamp=True), w)



PASSES = {
    "heat": volume(rgb(math("MULTIPLY", f, f), f, d), sigma),
    "motion": volume(rgb(enc(sep.outputs["X"]), enc(sep.outputs["Z"]), w), sigma),
    "depth": volume(rgb(math("MULTIPLY", f, KF),
                        math("MULTIPLY", math("MULTIPLY", d, KS),
                             math("SUBTRACT", 1.0, math("MULTIPLY", f, MASK), clamp=True)), 0.0), None),
}

# The camera: orthographic, square, looking along +Y, so the picture's x is the world's x and
# its y the world's z (the motion pass's vx and vz).
cam_r = recipe["camera"]
cam = bpy.data.objects.new("camera", bpy.data.cameras.new("camera"))
scene.collection.objects.link(cam)
cam.data.type = "ORTHO"
cam.data.ortho_scale = cam_r["extent"]
cam.data.clip_end = 100
cam.location = (cam_r["center"][0], -20, cam_r["center"][1])
cam.rotation_euler = (1.5707963, 0, 0)
scene.camera = cam
world = bpy.data.worlds.new("black")
world.color = (0, 0, 0)
scene.world = world

size = recipe["size"]
r = scene.render
r.engine = "CYCLES"
r.resolution_x = r.resolution_y = size
r.resolution_percentage = 100
r.film_transparent = True
r.image_settings.file_format = "OPEN_EXR"
r.image_settings.color_depth = "32"
c = scene.cycles
c.device = "CPU"
c.samples = recipe.get("samples", 16)
c.use_adaptive_sampling = False
c.use_denoising = False
c.max_bounces = c.volume_bounces = c.diffuse_bounces = c.glossy_bounces = 0
c.transparent_max_bounces = 8
c.volume_biased = True  # ray marched, so an emission integral is smooth, not sampled
c.volume_step_rate = recipe.get("step_rate", 0.5)
c.sample_clamp_direct = c.sample_clamp_indirect = 0.0
c.filter_width = 1.5

exr = os.path.join(out, "pass.exr")
t0 = time.time()
with open(os.path.join(out, "raw.bin"), "wb") as raw:
    for frame in range(first, last + 1):
        scene.frame_set(frame)
        for name, shader in PASSES.items():
            L.new(shader, output.inputs["Volume"])
            r.filepath = exr
            bpy.ops.render.render(write_still=True)
            img = bpy.data.images.load(exr)
            px = np.empty(size * size * 4, np.float32)
            img.pixels.foreach_get(px)
            bpy.data.images.remove(img)
            # Blender's rows run bottom to top; the packer's run top to bottom.
            px = px.reshape(size, size, 4)[::-1, :, :3]
            raw.write(np.ascontiguousarray(px).astype(np.float16).tobytes())
        if frame % 10 == 0:
            print("frame %d of %d" % (frame, last), flush=True)
os.remove(exr)
render_s = time.time() - t0
meta = {"set": recipe.get("name"), "size": size, "frames": last - first + 1, "first": first,
        "passes": list(PASSES), "fps": scene.render.fps, "velocity_max": VMAX,
        "extent": cam_r["extent"], "sim_s": round(sim_s, 1), "render_s": round(render_s, 1)}
with open(os.path.join(out, "meta.json"), "w") as m:
    json.dump(meta, m, indent=1)
print("baked %s: %d frames, sim %.0f s, render %.0f s" % (meta["set"], meta["frames"], sim_s, render_s))
