"""Makes the look of each thing that is held: one file per item, models/held/<item>.glb, that
the game puts in a hand socket as it is (character.rs).

Each look is one mesh with the place it is held at the origin and its handle along +Y toward
its working end, as a hand socket expects, and says in its glTF extras how far it tips when
carried rather than used: {"carry_tilt": -45} tips its head forward and down in a hanging
hand, positive raises it. Body-independent: a hand socket is the same on every body.

Its materials are named by role (Wood, Metal, Trim, String), so a colour given for a role
reaches every part of it: here, a stone or iron head; in the game, a tint (character.rs).

    tools/blender/run.sh tools/characters/fit_held.py hammer stone_axe iron_axe knife spear bow stone_pickaxe iron_pickaxe

Reads the free tool models in assets/models/tools (THIRD_PARTY.md), or
builds a pickaxe from a few faceted pieces, since no free one exists; writes
assets/models/held/<item>.glb, the item named as in the game's catalogue
in lower case with `_` for spaces. Each item's numbers are in TOOLS.
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))

import bpy
from mathutils import Matrix, Vector

import look

ASSETS = os.path.join(HERE, "..", "..", "assets", "models")
OUT = os.path.join(ASSETS, "held")
STONE = (0.55, 0.53, 0.5)
IRON = (0.38, 0.42, 0.48)

# item: (source model under models/tools, or "pickaxe" to build one; length in metres; how
# far from its end it is held; the colour of its working end, if its material shows there;
# carry tilt in degrees).
TOOLS = {
    "hammer": ("Hammer_Small.glb", 0.42, 0.06, None, -45),
    "stone_axe": ("Axe.glb", 0.68, 0.08, STONE, -45),
    "iron_axe": ("Axe.glb", 0.68, 0.08, IRON, -45),
    "knife": ("Knife.glb", 0.26, 0.05, None, -30),
    "spear": ("Spear.glb", 1.8, 0.75, None, 60),
    "bow": ("Bow_Wooden.glb", 1.2, 0.6, None, -30),
    "stone_pickaxe": ("pickaxe", 0.72, 0.08, STONE, -45),
    "iron_pickaxe": ("pickaxe", 0.72, 0.08, IRON, -45),
}


# The free models' material names, by the role each part plays.
ROLES = {"DarkWood": "Wood", "LightWood": "Wood", "Steel": "Metal", "LightSteel": "Metal", "Grey": "Metal",
         "LightGrey": "Metal", "Red": "Metal", "Yellow": "Trim", "White": "String"}


def material(name, colour):
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = look.linear(colour) + (1.0,)
    return mat


def pickaxe(head):
    """A handle, and a head that tapers to two points."""
    wood = material("Wood", (0.48, 0.32, 0.2))
    metal = material("Metal", head)
    parts = []
    bpy.ops.mesh.primitive_cylinder_add(vertices=6, radius=0.025, depth=1.0, location=(0, 0, 0.5))
    parts.append((bpy.context.object, wood))
    bpy.ops.mesh.primitive_cube_add(size=1, location=(0, 0, 0.95))
    bpy.context.object.scale = (0.12, 0.07, 0.08)
    parts.append((bpy.context.object, metal))
    for side in (1, -1):
        bpy.ops.mesh.primitive_cone_add(vertices=4, radius1=0.04, depth=0.32, location=(0.2 * side, 0, 0.93))
        bpy.context.object.rotation_euler = (0, math.radians(90 + 10) * side, 0)
        parts.append((bpy.context.object, metal))
    for ob, mat in parts:
        ob.data.materials.append(mat)
    return [ob for ob, _ in parts]


def one(objects, name):
    """The objects as one mesh, with their transforms applied."""
    bpy.ops.object.select_all(action="DESELECT")
    for ob in objects:
        ob.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    if len(objects) > 1:
        bpy.ops.object.join()
    ob = bpy.context.view_layer.objects.active
    ob.name = name
    return ob


os.makedirs(OUT, exist_ok=True)
for item in look.args():
    source, length, held_from_end, head, carry_tilt = TOOLS[item]
    look.reset()
    if source == "pickaxe":
        objects = pickaxe(head)
    else:
        bpy.ops.import_scene.gltf(filepath=os.path.join(ASSETS, "tools", source))
        objects = [o for o in bpy.context.scene.objects if o.type == "MESH"]
        for o in objects:
            mw = o.matrix_world.copy()
            o.parent = None
            o.matrix_world = mw
        for o in list(bpy.context.scene.objects):
            if o.type != "MESH":
                bpy.data.objects.remove(o)
        # Each part keeps its own material, named by its role (Blender numbers the second
        # Metal as Metal.001; the game reads the role before the dot). The working end
        # takes its colour, where it has one.
        renamed = {}
        for o in objects:
            for slot in o.material_slots:
                if slot.material is None:
                    continue
                source = slot.material.name.split(".")[0]
                if source not in renamed:
                    renamed[source] = slot.material.copy()
                    renamed[source].name = ROLES.get(source, source)
                    if head and ROLES.get(source) == "Metal":
                        renamed[source].node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = look.linear(head) + (1.0,)
                slot.material = renamed[source]
    ob = one(objects, item)
    # Its length along Blender's Z, which glTF's +Y becomes: scaled to `length`, the foot of
    # its handle at the origin, then down by where it is held.
    pts = [ob.matrix_world @ v.co for v in ob.data.vertices]
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    s = length / (hi.z - lo.z)
    foot = Vector(((lo.x + hi.x) / 2, (lo.y + hi.y) / 2, lo.z))
    ob.data.transform(Matrix.Translation((0, 0, -held_from_end)) @ Matrix.Scale(s, 4) @ Matrix.Translation(-foot))
    for slot in ob.material_slots:
        if slot.material and slot.material.use_nodes:
            bsdf = slot.material.node_tree.nodes["Principled BSDF"]
            bsdf.inputs["Roughness"].default_value = 0.7
            bsdf.inputs["Metallic"].default_value = 0.0
    ob["carry_tilt"] = carry_tilt
    bpy.ops.object.select_all(action="DESELECT")
    ob.select_set(True)
    path = os.path.join(OUT, f"{item}.glb")
    bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_extras=True)
    print("HELD", item, f"{length} m, held {held_from_end} m from its end, carried at {carry_tilt} degrees")
