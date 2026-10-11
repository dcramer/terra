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


def graphic(name):
    """A palette colour lit in three crisp bands, its shadow band tinted cool."""
    mat = bpy.data.materials.new(f"{name}-graphic")
    mat.use_nodes = True
    nt = mat.node_tree
    nt.nodes.clear()
    base = look.linear(look.PALETTE[name])
    diffuse = nt.nodes.new("ShaderNodeBsdfDiffuse")
    to_rgb = nt.nodes.new("ShaderNodeShaderToRGB")
    bw = nt.nodes.new("ShaderNodeRGBToBW")
    # Light runs from about 0.3 (sky only) to 3 (full sun); scale it into the ramp's 0..1.
    scale = nt.nodes.new("ShaderNodeMath")
    scale.operation = "MULTIPLY"
    scale.inputs[1].default_value = 0.3
    ramp = nt.nodes.new("ShaderNodeValToRGB")
    ramp.color_ramp.interpolation = "CONSTANT"
    stops = ramp.color_ramp.elements
    shadow = tuple(c * t for c, t in zip(base, (0.42, 0.42, 0.62)))
    mid = tuple(c * t for c, t in zip(base, (0.78, 0.76, 0.86)))
    lit = tuple(min(1.0, c * t) for c, t in zip(base, (1.08, 1.02, 0.92)))
    stops[0].position, stops[0].color = 0.0, shadow + (1,)
    stops[1].position, stops[1].color = 0.28, mid + (1,)
    stops.new(0.62).color = lit + (1,)
    emit = nt.nodes.new("ShaderNodeEmission")
    out = nt.nodes.new("ShaderNodeOutputMaterial")
    nt.links.new(diffuse.outputs[0], to_rgb.inputs[0])
    nt.links.new(to_rgb.outputs[0], bw.inputs[0])
    nt.links.new(bw.outputs[0], scale.inputs[0])
    nt.links.new(scale.outputs[0], ramp.inputs[0])
    nt.links.new(ramp.outputs[0], emit.inputs[0])
    nt.links.new(emit.outputs[0], out.inputs[0])
    return mat


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
        slot.material = graphic(palette) if style == "graphic" else look.material(palette)


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
