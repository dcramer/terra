"""Bakes a reference model into a construction material's texture set (D290).

    tools/blender/run.sh tools/kits/bake.py <model.glb> <out folder> <metres> <side>

A reference model is a hand-made patch of a material, a Tripo model of a stretch of wall,
a run of shingles or a log, cleaned up in Blender (D251): its look, baked from above onto a
square `metres` across, `side` texels, is the texture set every piece of the material
wears, the industry's high-to-low bake used for a material rather than one asset. Writes
`color.jpg`, `normal.jpg` (tangent space, OpenGL's way, as Bevy reads it), `arm.jpg`
(occlusion, roughness, no metal) and `height.jpg` (white highest) into the folder.
Run by `tools/kits/pack.ts` for a `model:` source.
"""

import bpy
import os
import sys

argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else sys.argv[-4:]
model, out, metres, side = argv[0], argv[1], float(argv[2]), int(argv[3])

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=model)
meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
if not meshes:
    sys.exit(f"{model}: no mesh")
# One object, its middle over the origin, its top at z = 0.
bpy.ops.object.select_all(action="DESELECT")
for o in meshes:
    o.select_set(True)
bpy.context.view_layer.objects.active = meshes[0]
if len(meshes) > 1:
    bpy.ops.object.join()
high = bpy.context.view_layer.objects.active
bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
xs = [v.co.x for v in high.data.vertices]
ys = [v.co.y for v in high.data.vertices]
zs = [v.co.z for v in high.data.vertices]
span = max(max(xs) - min(xs), max(ys) - min(ys))
# Scaled so its square covers the material's metres.
scale = metres / span
high.scale = (scale, scale, scale)
high.location = (-(max(xs) + min(xs)) / 2 * scale, -(max(ys) + min(ys)) / 2 * scale, -max(zs) * scale)
bpy.ops.object.transform_apply(location=True, scale=True)
depth = (max(zs) - min(zs)) * scale

# Its height as emission, for a bake of how high each point is: white at its top.
for slot in high.material_slots:
    material = slot.material
    if material is None or not material.use_nodes:
        continue
    nodes, links = material.node_tree.nodes, material.node_tree.links
    geometry = nodes.new("ShaderNodeNewGeometry")
    split = nodes.new("ShaderNodeSeparateXYZ")
    ramp = nodes.new("ShaderNodeMapRange")
    ramp.inputs["From Min"].default_value = -depth
    ramp.inputs["From Max"].default_value = 0.0
    links.new(geometry.outputs["Position"], split.inputs[0])
    links.new(split.outputs["Z"], ramp.inputs["Value"])
    bsdf = next((n for n in nodes if n.type == "BSDF_PRINCIPLED"), None)
    if bsdf is not None:
        links.new(ramp.outputs["Result"], bsdf.inputs["Emission Color"])
        bsdf.inputs["Emission Strength"].default_value = 1.0

# The square it is baked onto, a little over it.
bpy.ops.mesh.primitive_plane_add(size=metres, location=(0, 0, 0.001))
low = bpy.context.active_object
bpy.ops.object.mode_set(mode="EDIT")
bpy.ops.uv.unwrap()
bpy.ops.object.mode_set(mode="OBJECT")
bake_material = bpy.data.materials.new("bake")
bake_material.use_nodes = True
low.data.materials.append(bake_material)
target = bake_material.node_tree.nodes.new("ShaderNodeTexImage")
bake_material.node_tree.nodes.active = target

scene = bpy.context.scene
scene.render.engine = "CYCLES"
scene.cycles.samples = 16
scene.render.bake.use_selected_to_active = True
scene.render.bake.cage_extrusion = depth + 0.01
scene.render.bake.max_ray_distance = depth + 0.02
bpy.ops.object.select_all(action="DESELECT")
high.select_set(True)
low.select_set(True)
bpy.context.view_layer.objects.active = low


def bake(kind, name, colour_space="Non-Color", **settings):
    image = bpy.data.images.new(name, side, side)
    image.colorspace_settings.name = colour_space
    target.image = image
    for key, value in settings.items():
        setattr(scene.render.bake, key, value)
    bpy.ops.object.bake(type=kind)
    return image


def save(image, file):
    image.filepath_raw = os.path.join(out, file)
    image.file_format = "JPEG"
    scene.render.image_settings.quality = 92
    image.save()


os.makedirs(out, exist_ok=True)
colour = bake("DIFFUSE", "color", "sRGB", use_pass_direct=False, use_pass_indirect=False, use_pass_color=True)
save(colour, "color.jpg")
save(bake("NORMAL", "normal", normal_space="TANGENT"), "normal.jpg")
roughness = bake("ROUGHNESS", "roughness")
occlusion = bake("AO", "occlusion")
save(bake("EMIT", "height"), "height.jpg")
# Occlusion in red, roughness in green, no metal.
arm = bpy.data.images.new("arm", side, side)
arm.colorspace_settings.name = "Non-Color"
ao, rough = list(occlusion.pixels), list(roughness.pixels)
pixels = [0.0] * (side * side * 4)
for i in range(side * side):
    pixels[i * 4] = ao[i * 4]
    pixels[i * 4 + 1] = rough[i * 4]
    pixels[i * 4 + 3] = 1.0
arm.pixels = pixels
save(arm, "arm.jpg")
print(f"bake: {model} into {out}, {metres} m a side, {side} texels, its relief {depth:.3f} m")
