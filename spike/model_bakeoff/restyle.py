"""Renders finished models in two shadings, to see whether a look is a shader or a model.

A, facets: every triangle lit as one flat plane (what the game does now).
D, graphic: smooth surfaces with hard edges kept, light in a few crisp bands, and the
shadow band tinted cool, so a form reads as a couple of big flat areas of colour.

    run.sh restyle.py Wolf=out/refined-Wolf-Wolf.glb Villager=out/refined-Villager-Villager.glb
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import bpy
from mathutils import Matrix, Vector

import look


def load(path, label):
    before = set(bpy.context.scene.objects)
    bpy.ops.import_scene.gltf(filepath=os.path.join(look.HERE, path))
    new = [o for o in bpy.context.scene.objects if o not in before]
    ob = next(o for o in new if o.type == "MESH")
    ob.name = label
    ob.parent = None
    ob.data.transform(ob.matrix_world)
    ob.matrix_world = Matrix.Identity(4)
    for o in new:
        if o is not ob:
            bpy.data.objects.remove(o)
    return ob


def shade(ob, style):
    for o in bpy.context.scene.objects:
        o.select_set(o is ob)
    bpy.context.view_layer.objects.active = ob
    if style == "graphic":
        bpy.ops.object.shade_smooth_by_angle(angle=math.radians(50))
    else:
        bpy.ops.object.shade_flat()
    for slot in ob.material_slots:
        palette = slot.material.name.split("-")[0].split(".")[0]
        slot.material = look.graphic(palette) if style == "graphic" else look.material(palette)


look.reset()
look.stage()
bpy.data.objects["Sun"].data.energy = 3.0
# Light from the side, so shadow shapes show as they do in the concepts.
look.SUN_YAW, look.SUN_PITCH = 95, 30
pairs = [a.split("=", 1) for a in look.args()]
rows = []
for label, path in pairs:
    a = load(path, f"{label}A")
    d = load(path, f"{label}D")
    shade(a, "facets")
    shade(d, "graphic")
    look.ground_to_origin(a)
    look.ground_to_origin(d)
    tiles = []
    for ob, views, tag in ((a, [(30, 12)], "a"), (d, [(30, 12), (120, 12)], "d")):
        for other in bpy.context.scene.objects:
            if other.type == "MESH" and other.name != "Ground":
                other.hide_render = other is not ob
        bpy.context.scene.view_settings.view_transform = "Standard" if tag == "d" else "AgX"
        tile = os.path.join(look.OUT, f"restyle.{label}.{tag}.png")
        look.render_views(ob, tile, views=views)
        tiles.append(tile)
    row = os.path.join(look.OUT, f"restyle.{label}.png")
    look.stitch(tiles, row)
    for t in tiles:
        os.remove(t)
    rows.append(row)
    for ob in (a, d):
        ob.hide_render = True
look.stack(rows, os.path.join(look.OUT, "restyle.png"))
for r in rows:
    os.remove(r)
print("SHEET", os.path.join(look.OUT, "restyle.png"))
