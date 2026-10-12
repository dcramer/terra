"""Shared Blender helpers for the character pipeline: paths, our palette, our light, sheets.

Imported by the pipeline's scripts, which run inside Blender through tools/blender/run.sh, from
the repository root. Paths given to a script are relative to that root. Blender is Z-up in
metres; the glTF exporter turns that into the game's Y-up.
"""

import math
import os
import sys

import bpy
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
# What the pipeline writes and does not keep.
OUT = os.path.join(HERE, "out")
# Worn looks as made on a base body, before its morphs are added (shell_garment.py, fit_gear.py):
# out/gear/<body>/<item>.glb. The game loads assets/models/gear/<item>.glb, made from these by
# build_bodies.sh, one per item for every body.
GEAR_MADE = os.path.join(OUT, "gear")
# The source art it reads (art/README.md), and the assets it writes for the game.
ART = os.path.join(ROOT, "art")
ASSETS = os.path.join(ROOT, "assets")
FIGURE = os.path.join(ASSETS, "models", "character", "figure.gltf")

# The palette, in sRGB, from the sandbox's own recolouring (play/flora.rs and props.rs).
# A model's materials must be named from this list; its colour comes from here, not the file.
PALETTE = {
    "Rock": (0.66, 0.58, 0.54),
    "RockDark": (0.48, 0.42, 0.40),
    "RockPale": (0.80, 0.78, 0.72),
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
    "FurGrey": (0.68, 0.67, 0.66),
    "Cloth": (0.82, 0.74, 0.62),
    "ClothRed": (0.62, 0.20, 0.17),
    "Red": (0.84, 0.27, 0.20),
    "ClothBlue": (0.20, 0.45, 0.56),
    "Leather": (0.45, 0.30, 0.20),
    "LeatherLight": (0.62, 0.42, 0.26),
    "Brass": (0.80, 0.62, 0.30),
    "Skin": (0.80, 0.58, 0.45),
    "Hair": (0.24, 0.17, 0.13),
    "Metal": (0.62, 0.63, 0.66),
    "MetalDark": (0.30, 0.31, 0.36),
    "Rope": (0.78, 0.66, 0.46),
    "Bone": (0.93, 0.88, 0.78),
    "Black": (0.14, 0.14, 0.17),
    "Accent": (0.90, 0.46, 0.17),
    "Panel": (0.86, 0.85, 0.82),
    "PanelDark": (0.36, 0.38, 0.44),
    "Glow": (0.35, 0.85, 0.90),
    "Visor": (0.20, 0.30, 0.42),
    "Alien": (0.45, 0.30, 0.62),
    "AlienLight": (0.72, 0.56, 0.86),
    "AlienStem": (0.25, 0.36, 0.50),
    "Olive": (0.55, 0.58, 0.42),
    "Peach": (0.95, 0.70, 0.55),
    "Clay": (0.72, 0.53, 0.33),
    "ClothDark": (0.33, 0.31, 0.30),
    # Glows in the game: coals, a furnace's mouth (props.rs).
    "Fire": (1.00, 0.72, 0.35),
}

# The game's light (experiments/look.rs): a warm sun, shadows lit by a cool sky.
SUN_COLOUR = (1.0, 0.9, 0.76)
SKY_COLOUR = (0.62, 0.68, 1.0)
GROUND = (0.70, 0.66, 0.56)


def linear(c):
    return tuple(x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4 for x in c)


def args():
    """Arguments after Blender's `--`."""
    return sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []


def path(given):
    """A path given to a script, relative to the repository root it is run from."""
    return os.path.abspath(given)


def body_name(path):
    """A body's name from its file: base-male.glb and out/base-male-game.glb are base-male."""
    stem = os.path.splitext(os.path.basename(path))[0]
    return stem[: -len("-game")] if stem.endswith("-game") else stem


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
    bg.inputs["Strength"].default_value = 0.4

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
