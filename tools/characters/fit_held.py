"""Makes the look of each thing that is held: one file per item, models/held/<item>.glb, that
the game puts in a hand socket as it is (character.rs), or, strapped (STRAPS: a shield), on the
forearm's bone where its extras say.

Each look is one mesh with the place it is held at the origin and its handle along +Y toward
its working end, as a hand socket expects, and says in its glTF extras how far it tips when
carried rather than used: {"carry_tilt": -45} tips its head forward and down in a hanging
hand, positive raises it. Body-independent: a hand socket is the same on every body.

Its materials are named by role (Wood, Metal, Trim, String), so a colour given for a role
reaches every part of it: here, a stone or iron head; in the game, a tint (character.rs).

    tools/blender/run.sh tools/characters/fit_held.py hammer stone_axe iron_axe knife spear bow stone_pickaxe iron_pickaxe torch basket

A thing carried hanging by a handle (a market basket, in HUNG) is made from its prop instead,
and a textured model of ours (a sword, a shield, in TEXTURED) keeps its own materials and maps:

    tools/blender/run.sh tools/characters/fit_held.py arming_sword round_shield heater_shield

Reads the tool models in assets/models/tools: free ones (THIRD_PARTY.md), and ours where no
free one exists, a pickaxe and a torch, made by tools/props standing on their handles; writes
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
    "stone_pickaxe": ("Pickaxe.glb", 0.9, 0.08, STONE, -45),
    "iron_pickaxe": ("Pickaxe.glb", 0.9, 0.08, IRON, -45),
    # Carried upright, its burning head up; the flame is the game's (torchlight.rs).
    "torch": ("Torch.glb", 0.55, 0.1, None, 35),
}

# Things carried hanging from the hand by a handle over their top, as a basket is, from the
# props in assets/models/props (tools/props): the prop, and how far out from the leg it hangs.
# The hand's socket in a hanging arm has +Z up the arm, +Y forward and +X out from the body,
# so the thing hangs down -Z from its handle's top, its long way forward along +Y.
HUNG = {
    "basket": ("MarketBasket.glb", 0.05),
}

# Things held whose source is a textured model of ours in assets/models/tools (tools/props,
# "into": "tools"), kept in its own materials and textures: the source; its grip, the point
# held at the middle of the fist; a point its handle runs toward, at its working end (the
# look's +Y); the direction that becomes the look's +X, out of the back of a right hand and
# the palm of a left; carry tilt in degrees. Points and directions in metres in the source's
# glTF axes (X, Y up, Z its front), as the workbench and props.ts's sheets show it.
TEXTURED = {
    # Its flats out of the back of the hand and the palm, so its edges cut as the fist swings.
    "arming_sword": ("ArmingSword.glb", (0.002, 0.12, 0.0075), (-0.03, 0.95, -0.019), (0, 0, 1), -40),
    # A centre grip, held in the left hand across its planks just behind the board, the
    # knuckles up into the boss: its face away from the wrist (-Z), the forearm square to the
    # board behind it, so with the fist forward it covers the forearm and body. Its planks run
    # along +X.
    "round_shield": ("RoundShield.glb", (0, 0.405, -0.025), (0.4, 0.405, -0.025), (0, 1, 0), 0),
    # Strapped by enarmes (D504): held at its hand strap, on its back right of the middle seen
    # from behind, where the fist closes; the forearm runs from the arm strap on the left to it
    # (+Y, across the board, 15 degrees down toward the hand as enarmes are often set), its
    # point toward +X. Its back view, with the straps, is art/props/heater_shield/back.png.
    "heater_shield": ("HeaterShield.glb", (-0.12, 0.40, -0.035), (-0.41, 0.322, -0.035), (0, -1, 0), 0),
}

# Things strapped to the forearm rather than hung on the hand socket, said in their extras as
# {"strap": {"bone", "at", "face", "grip"}} for the game (character.rs) and moves.ts: the bone
# they ride on; where their origin (the grip) sits in its frame, metres; and the directions in
# that frame their front (-Z) and grip (+Y) turn to. The round shield rides on lowerarm_l,
# which follows the forearm but not the hand's roll (on lowerarm_twist_01_l, which takes half
# of it, the block's face turns 27 degrees off ahead): its face out of the little-finger side,
# ahead when the forearm is raised across the body (5 degrees off at the block's hold); its
# grip bar along the forearm, its centre over the knuckles, 6.5 cm out from the bone's line,
# clear of the skin. Measured by tools/characters/moves.ts: clear of the head and body in the
# guard, block and strafes on every template. The heater (D504) rides the same way, its hand
# strap 42 cm along the bone and 4 cm out, its point down in the guard: square across the board
# or nearer the elbow, its top corner went 21 cm into the spine in the heavy and the left strafe.
STRAPS = {
    "round_shield": {"bone": "lowerarm_l", "at": [-0.02, 0.34, -0.065], "face": [0, 0, -1], "grip": [0, 1, 0]},
    "heater_shield": {"bone": "lowerarm_l", "at": [0.0, 0.42, -0.04], "face": [0, 0, -1], "grip": [0, 1, 0]},
}


# The free models' material names, by the role each part plays.
ROLES = {"DarkWood": "Wood", "LightWood": "Wood", "Steel": "Metal", "LightSteel": "Metal", "Grey": "Metal",
         "LightGrey": "Metal", "Red": "Metal", "Yellow": "Trim", "White": "String"}


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


def hung(item):
    """A thing hung from the hand by its handle: its handle's top at the origin, its long way
    along glTF's +Y and its top up glTF's +Z (Blender's -Y), its own materials kept."""
    source, out = HUNG[item]
    look.reset()
    bpy.ops.import_scene.gltf(filepath=os.path.join(ASSETS, "props", source))
    objects = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    for o in list(bpy.context.scene.objects):
        if o.type != "MESH":
            bpy.data.objects.remove(o)
    ob = one(objects, item)
    pts = [ob.matrix_world @ v.co for v in ob.data.vertices]
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    # The handle's top, the middle of what is within 2 cm of the highest point; gripped a
    # finger's width below it.
    top = [p for p in pts if p.z > hi.z - 0.02]
    grip = sum(top, Vector()) / len(top) - Vector((0, 0, 0.015))
    # Blender's model: up +Z, its long way along X (glTF's), its front -Y. Its long way turned
    # to Blender's Y, then the whole tipped so up is Blender's -Y (glTF's +Z) and Y Blender's Z.
    long_y = Matrix.Rotation(math.radians(90), 4, "Z") if hi.x - lo.x > hi.y - lo.y else Matrix()
    tip = Matrix.Rotation(math.radians(90), 4, "X")
    ob.data.transform(Matrix.Translation((out, 0, 0)) @ tip @ long_y @ Matrix.Translation(-grip))
    ob["carry_tilt"] = 0
    bpy.ops.object.select_all(action="DESELECT")
    ob.select_set(True)
    path = os.path.join(OUT, f"{item}.glb")
    bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_extras=True,
                              export_image_format="JPEG", export_jpeg_quality=85)
    print("HELD", item, f"hung from its handle, {out} m out from the leg")


def textured(item):
    """A textured model held as TEXTURED says: its grip at the origin, its handle toward its
    working end along +Y, its own materials and maps kept."""
    source, grip, toward, x, carry_tilt = TEXTURED[item]
    look.reset()
    bpy.ops.import_scene.gltf(filepath=os.path.join(ASSETS, "tools", source))
    objects = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    for o in list(bpy.context.scene.objects):
        if o.type != "MESH":
            bpy.data.objects.remove(o)
    ob = one(objects, item)
    # glTF's (x, y, z) is Blender's (x, -z, y); the look's +X, +Y and +Z go to Blender's X, Z
    # and -Y, so they come out as glTF's X, Y and Z again.
    blender = lambda v: Vector((v[0], -v[2], v[1]))
    grip = blender(grip)
    along = (blender(toward) - grip).normalized()
    out = blender(x)
    out = (out - out.dot(along) * along).normalized()
    frame = Matrix((out, along, out.cross(along))).transposed()
    held = Matrix(((1, 0, 0), (0, 0, 1), (0, -1, 0))).transposed()
    ob.data.transform((held @ frame.transposed()).to_4x4() @ Matrix.Translation(-grip))
    ob["carry_tilt"] = carry_tilt
    if item in STRAPS:
        ob["strap"] = STRAPS[item]
    bpy.ops.object.select_all(action="DESELECT")
    ob.select_set(True)
    path = os.path.join(OUT, f"{item}.glb")
    bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_extras=True,
                              export_tangents=True, export_image_format="JPEG", export_jpeg_quality=85)
    print("HELD", item, f"from {source}, its textures kept, carried at {carry_tilt} degrees")


os.makedirs(OUT, exist_ok=True)
for item in look.args():
    if item in HUNG:
        hung(item)
        continue
    if item in TEXTURED:
        textured(item)
        continue
    source, length, held_from_end, head, carry_tilt = TOOLS[item]
    look.reset()
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
                if head and ROLES.get(source, source) == "Metal":
                    renamed[source].node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = look.linear(head) + (1.0,)
            slot.material = renamed[source]
    # A textured source (ours, made by tools/props: the torch) keeps its maps; a flat one takes
    # the plain roughness below.
    maps = any(n.type == "TEX_IMAGE" for m in renamed.values() if m.use_nodes for n in m.node_tree.nodes)
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
        if slot.material and slot.material.use_nodes and not maps:
            bsdf = slot.material.node_tree.nodes["Principled BSDF"]
            bsdf.inputs["Roughness"].default_value = 0.7
            bsdf.inputs["Metallic"].default_value = 0.0
    ob["carry_tilt"] = carry_tilt
    bpy.ops.object.select_all(action="DESELECT")
    ob.select_set(True)
    path = os.path.join(OUT, f"{item}.glb")
    kept = {"export_tangents": True, "export_image_format": "JPEG", "export_jpeg_quality": 85} if maps else {}
    bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_extras=True, **kept)
    print("HELD", item, f"{length} m, held {held_from_end} m from its end, carried at {carry_tilt} degrees",
          "its textures kept" if maps else "")
