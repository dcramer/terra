"""Dyes a refined prop's one strongly coloured cloth another colour, as a copy in its own file:
a market stall's red-and-cream awning in woad blue, weld yellow or green, so a market's stalls
are not one stall over and over (tools/props, `dyes` in prop.json).

    tools/blender/run.sh tools/props/dye.py SOURCE.glb OUT.glb R,G,B [WEATHER]

The cloth is what in the colour map is far more saturated than the rest (the wood, the
canvas's cream): those texels take the dye's hue and saturation, keeping their own light and
dark, so the weave and the folds stay. R,G,B is the dye in sRGB at the cloth's mean lightness.
WEATHER, if given, greys and darkens the rest by that much (0.75: a quarter darker, a fifth
toward grey), for wood the generator painted fresh that a market has had out in the weather
for years. Everything else of the file, its mesh, normals, roughness and sockets, is kept.
"""

import colorsys
import sys

import bpy
import numpy as np

given = sys.argv[sys.argv.index("--") + 1 :]
source, out, dye = given[:3]
weather = float(given[3]) if len(given) > 3 else 1.0
dye_rgb = [float(v) for v in dye.split(",")]
dye_h, dye_l, dye_s = colorsys.rgb_to_hls(*dye_rgb)

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=source)

# Saturation above this is the cloth's dye; the wood and the cream are far below it.
CLOTH_S = 0.35


def to_hls(rgb):
    """sRGB to hue, lightness and saturation, per pixel."""
    mx, mn = rgb.max(axis=1), rgb.min(axis=1)
    light = (mx + mn) / 2
    delta = mx - mn
    sat = np.where(delta < 1e-6, 0.0, delta / np.maximum(1e-6, 1 - np.abs(2 * light - 1)))
    return light, sat


def from_hls(h, light, s):
    """Hue (one value), lightness and saturation per pixel, to sRGB."""
    c = (1 - np.abs(2 * light - 1)) * s
    hp = h * 6
    x = c * (1 - abs(hp % 2 - 1))
    r1, g1, b1 = [
        (c, x, 0), (x, c, 0), (0, c, x), (0, x, c), (x, 0, c), (c, 0, x)
    ][int(hp) % 6]
    m = light - c / 2
    return np.stack([r1 + m, g1 + m, b1 + m], axis=1)


dyed = 0
for material in bpy.data.materials:
    if not material.use_nodes:
        continue
    bsdf = material.node_tree.nodes.get("Principled BSDF")
    if bsdf is None or not bsdf.inputs["Base Color"].is_linked:
        continue
    image = bsdf.inputs["Base Color"].links[0].from_node.image
    px = np.array(image.pixels[:], dtype=np.float32).reshape(-1, 4)
    # Blender gives an sRGB texture's pixels as stored, in sRGB.
    light, sat = to_hls(px[:, :3])
    cloth = sat > CLOTH_S
    if not cloth.any():
        continue
    # The cloth's light and dark kept about the dye's own lightness.
    mean = light[cloth].mean()
    new_light = np.clip(dye_l + (light[cloth] - mean) * 0.9, 0.0, 1.0)
    new_sat = np.clip(sat[cloth] / sat[cloth].mean() * dye_s, 0.0, 1.0)
    px[cloth, :3] = from_hls(dye_h, new_light, new_sat)
    if weather < 1.0:
        rest = px[~cloth, :3]
        grey = rest.mean(axis=1, keepdims=True)
        px[~cloth, :3] = (rest + (grey - rest) * (1.0 - weather) * 0.8) * weather
    image.pixels[:] = px.ravel()
    image.update()
    dyed += int(cloth.sum())

bpy.ops.export_scene.gltf(filepath=out, export_format="GLB", export_tangents=True, export_image_format="JPEG",
                          export_jpeg_quality=90, export_extras=True)
print("DYED", out, f"{dyed} texels to {dye}")
