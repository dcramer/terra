"""Bakes a reference model into a construction material's texture set (D290).

    tools/blender/run.sh tools/kits/bake.py <model.glb> <out folder> <metres> <side>

A reference model is a hand-made patch of a material, a Tripo model of a stretch of wall,
a run of shingles or a log, cleaned up in Blender (D251): its look, baked from above onto a
square `metres` across, `side` texels, is the texture set every piece of the material
wears, the industry's high-to-low bake used for a material rather than one asset. Writes
`color.jpg`, `normal.jpg` (tangent space, OpenGL's way, as Bevy reads it), `arm.jpg`
(occlusion, roughness, no metal) and `height.jpg` (white highest) into the folder.
Run by `tools/kits/pack.ts` for a `model:` source.

    tools/blender/run.sh tools/kits/bake.py <model.glb> <out folder> <metres> <side> round <diameter> <length>

A member's reference, a modelled log, is baked round instead: laid along its longest side
at the size the kit lays it (`diameter` thick, `length` long, as its pieces file says), baked from outside onto a cylinder through
its middle, its texture once round the member by `metres` along, the stretch beyond that
cross-faded into its start so it tiles along; the member's taper and bow taken out of its
height, which is the bark's relief alone. The member's generated geometry wears it.
"""

import bpy
import os
import sys

argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else sys.argv[-4:]
model, out, metres, side = argv[0], argv[1], float(argv[2]), int(argv[3])
round_bake = len(argv) > 6 and argv[4] == "round"

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
def bake_round(high, diameter, laid):
    """The member's look baked round onto a cylinder, tiling along (see the module's doc)."""
    import bmesh
    import math
    import numpy as np
    from mathutils import Matrix

    # Its axis, the way its points spread most (a model may lie at a slant in its box),
    # along Z through the origin, at the kit's size: its mean radius half the diameter.
    from mathutils import Vector

    points = np.array([v.co[:] for v in high.data.vertices], dtype=np.float64)
    middle = points.mean(axis=0)
    spread = np.linalg.eigh(np.cov((points - middle).T))[1][:, -1]
    rotation = Vector(spread.tolist()).rotation_difference(Vector((0.0, 0.0, 1.0))).to_matrix().to_4x4()
    high.matrix_world = rotation @ Matrix.Translation(-Vector(middle.tolist())) @ high.matrix_world
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    points = np.array([v.co[:] for v in high.data.vertices], dtype=np.float64)
    lo, hi = points[:, 2].min(), points[:, 2].max()
    inside = np.abs(points[:, 2] - (lo + hi) / 2) < (hi - lo) * 0.35
    # The axis through each end's middle, so the bark is round it.
    radius = np.hypot(points[inside, 0], points[inside, 1]).mean()
    across = 2 * radius
    along = laid / (hi - lo)
    high.scale = (diameter / across, diameter / across, along)
    high.location = (0.0, 0.0, -(hi + lo) / 2 * along)
    bpy.ops.object.transform_apply(location=True, scale=True)
    length = laid
    # A stretch beyond a copy to cross-fade into its start, clear of the sawn ends.
    overlap = min(0.25 * metres, max(0.0, length * 0.9 - metres))
    span = metres + overlap
    radii = [math.hypot(v.co.x, v.co.y) for v in high.data.vertices if abs(v.co.z) < span / 2]
    inner, outer = min(radii), max(radii)

    # Its distance from the axis as emission: how high its bark stands.
    for slot in high.material_slots:
        material = slot.material
        if material is None or not material.use_nodes:
            continue
        nodes, links = material.node_tree.nodes, material.node_tree.links
        geometry = nodes.new("ShaderNodeNewGeometry")
        flat = nodes.new("ShaderNodeVectorMath")
        flat.operation = "MULTIPLY"
        flat.inputs[1].default_value = (1.0, 1.0, 0.0)
        size = nodes.new("ShaderNodeVectorMath")
        size.operation = "LENGTH"
        ramp = nodes.new("ShaderNodeMapRange")
        ramp.inputs["From Min"].default_value = inner
        ramp.inputs["From Max"].default_value = outer
        links.new(geometry.outputs["Position"], flat.inputs[0])
        links.new(flat.outputs["Vector"], size.inputs[0])
        links.new(size.outputs["Value"], ramp.inputs["Value"])
        bsdf = next((n for n in nodes if n.type == "BSDF_PRINCIPLED"), None)
        if bsdf is not None:
            links.new(ramp.outputs["Result"], bsdf.inputs["Emission Color"])
            bsdf.inputs["Emission Strength"].default_value = 1.0

    # The cylinder, inside the bark, U once round from underneath, V along.
    columns = 96
    mesh = bpy.data.meshes.new("round")
    bm = bmesh.new()
    uv = bm.loops.layers.uv.new()
    ring = []
    for end in (0, 1):
        z = -span / 2 + span * end
        ring.append([
            bm.verts.new((
                inner * 0.98 * math.cos(-math.pi / 2 + 2 * math.pi * j / columns),
                inner * 0.98 * math.sin(-math.pi / 2 + 2 * math.pi * j / columns),
                z,
            ))
            for j in range(columns)
        ])
    for j in range(columns):
        k = (j + 1) % columns
        face = bm.faces.new((ring[0][j], ring[0][k], ring[1][k], ring[1][j]))
        u0, u1 = j / columns, (j + 1) / columns
        for loop, place in zip(face.loops, ((u0, 0.0), (u1, 0.0), (u1, 1.0), (u0, 1.0))):
            loop[uv].uv = place
        face.smooth = True
    bm.normal_update()
    bm.to_mesh(mesh)
    bm.free()
    low = bpy.data.objects.new("round", mesh)
    bpy.context.collection.objects.link(low)
    for poly in low.data.polygons:
        poly.use_smooth = True
    return low, span, overlap, outer - inner * 0.98


def tiled(pixels, size, span, overlap, high_pass):
    """A bake `span` along cut to its first `span - overlap`, the rest cross-faded into its
    start so its two ends meet; each row's mean taken out of the channels `high_pass` names
    (a member's taper and bow), then resampled `size` square, each row read between the two
    rows of the bake either side of it (the nearest row alone doubled every fifth one, a
    stair in the grain, research/texture-detail-2026-10-15.md)."""
    import numpy as np

    rows = np.asarray(pixels, dtype=np.float32).reshape(size, size, 4)
    keep = (span - overlap) / span
    fade = overlap / span
    total = rows.shape[0]

    def row_at(v):
        src = min(max(v * total - 0.5, 0.0), total - 1.0)
        low = int(src)
        high = min(low + 1, total - 1)
        t = src - low
        return rows[low] * (1 - t) + rows[high] * t

    out_rows = []
    for y in range(size):
        v = (y + 0.5) / size * keep
        row = row_at(v)
        if v < fade and fade > 0:
            t = v / fade
            row = row_at(v + keep) * (1 - t) + row * t
        out_rows.append(row)
    out = np.stack(out_rows)
    for channel, keep_mean in high_pass:
        mean = out[:, :, channel].mean(axis=1, keepdims=True)
        out[:, :, channel] = out[:, :, channel] - mean + keep_mean
    return np.clip(out, 0.0, 1.0)



def bake_flat(high):
    """The patch's look baked from above onto a square `metres` across (see the module's doc)."""
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
    return low, depth


if round_bake:
    low, span, overlap, depth = bake_round(high, float(argv[5]), float(argv[6]))
else:
    low, depth = bake_flat(high)
    span, overlap = metres, 0.0
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


# Round, how far its maps are turned so the colour's darkest side, the model's underside,
# lies underneath, where the member's texture starts and its seam is out of sight.
turn = None


def delit(pixels, keep=0.2):
    """The light a model's texture was painted in taken out of a round bake, as photogrammetry
    de-lights a scan: each column round the member brought to the colour's mean brightness,
    the columns' brightness smoothed round it first, so only the broad shading round the
    member goes and its bark's and wood's own light and dark stay; `keep` of the shading
    left. A Tripo log's texture is darkest underneath by six times (its light from above),
    which drew a black band under every log of a wall over the game's own shading."""
    import numpy as np

    light = pixels[:, :, :3].mean(axis=(0, 2))
    width = max(3, pixels.shape[1] // 12)
    kernel = np.ones(width) / width
    round_ = np.concatenate([light[-width:], light, light[:width]])
    broad = np.convolve(round_, kernel, "same")[width:-width]
    scale = np.power(light.mean() / np.maximum(broad, 1e-3), 1.0 - keep)
    pixels[:, :, :3] = np.clip(pixels[:, :, :3] * scale[None, :, None], 0.0, 1.0)
    return pixels


def save(image, file, high_pass=(), stretch=False, light=False):
    global turn
    if round_bake:
        import numpy as np

        pixels = tiled(list(image.pixels), side, span, overlap, high_pass)
        if turn is None:
            light_round = pixels[:, :, :3].mean(axis=(0, 2))
            width = side // 4
            window = np.convolve(np.concatenate([light_round, light_round[:width]]), np.ones(width), "valid")[:side]
            turn = int(np.argmin(window)) + width // 2
        pixels = np.roll(pixels, -turn, axis=1)
        if light:
            pixels = delit(pixels)
        if stretch:
            low_, high_ = pixels[:, :, 0].min(), pixels[:, :, 0].max()
            pixels[:, :, :3] = (pixels[:, :, :3] - low_) / max(high_ - low_, 1e-6)
        pixels[:, :, 3] = 1.0
        image.pixels = pixels.ravel().tolist()
    image.filepath_raw = os.path.join(out, file)
    image.file_format = "JPEG"
    scene.render.image_settings.quality = 92
    image.save()


os.makedirs(out, exist_ok=True)
colour = bake("DIFFUSE", "color", "sRGB", use_pass_direct=False, use_pass_indirect=False, use_pass_color=True)
save(colour, "color.jpg", light=True)
save(bake("NORMAL", "normal", normal_space="TANGENT"), "normal.jpg", [(0, 0.5), (1, 0.5)])
roughness = bake("ROUGHNESS", "roughness")
occlusion = bake("AO", "occlusion")
save(bake("EMIT", "height"), "height.jpg", [(0, 0.5), (1, 0.5), (2, 0.5)], stretch=True)
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
what = f"round, {metres} m along" if round_bake else f"{metres} m a side"
print(f"bake: {model} into {out}, {what}, {side} texels, its relief {depth:.3f} m")
