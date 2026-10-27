"""Makes the look of each thing that is held: one file per item, models/held/<item>.glb, that
the game puts in a hand socket as it is (character.rs), or, strapped (a shield, its "strap"), on the
forearm's bone where its extras say.

Each look is one mesh with the place it is held at the origin and its handle along +Y toward
its working end, as a hand socket expects, and says in its glTF extras how far it tips when
carried rather than used: {"carry_tilt": -45} tips its head forward and down in a hanging
hand, positive raises it. Body-independent: a hand socket is the same on every body.

Its materials are named by role (Wood, Metal, Trim, String), so a colour given for a role
reaches every part of it: here, a stone or iron head; in the game, a tint (character.rs).

    tools/blender/run.sh tools/characters/fit_held.py hammer stone_axe arming_sword basket ...

Each item's numbers are data, art/held/<item>.json (docs/attachment.md, D546), so adding an
item is adding a file, never editing this script. Its "fit" says how the look is made:

- "length": a model standing on the end of its handle, along its height (the free tool models
  in assets/models/tools, THIRD_PARTY.md, and ours made by tools/props): scaled to "length"
  metres, held "grip_from_end" metres up from its foot, its working end coloured "head_colour"
  where its material shows there.
- "points": a textured model of ours (tools/props, "into": "tools"), kept in its own materials
  and maps: "grip", the point held at the middle of the fist; "toward", a point its handle runs
  toward at its working end (the look's +Y); "out", the direction that becomes the look's +X,
  out of the back of a right hand and the palm of a left. Points and directions in metres in
  the source's glTF axes (X, Y up, Z its front), as the workbench and props.ts's sheets show.
- "hung": a thing carried hanging from the hand by a handle over its top, as a basket is, from
  its prop: "out", how far out from the leg it hangs. The hand's socket in a hanging arm has +Z
  up the arm, +Y forward and +X out from the body, so it hangs down -Z from its handle's top,
  its long way forward along +Y; its extras say {"hold": "hung"}.

Every item gives "source" (under assets/models) and "carry_tilt" (degrees), and may give
"strap": strapped to the forearm rather than hung on the hand socket, said in its extras as
{"strap": {"bone", "at", "face", "grip"}} for the game (terra_walker::strap) and moves.ts: the
bone it rides on; where its origin (the grip) sits in that bone's frame, metres; and the
directions in that frame its front (-Z) and grip (+Y) turn to. "note" says why, for people.

Writes assets/models/held/<item>.glb, the item named as in the game's catalogue in lower case
with `_` for spaces.
"""

import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))

import bpy
from mathutils import Matrix, Vector

import look

ASSETS = os.path.join(HERE, "..", "..", "assets", "models")
OUT = os.path.join(ASSETS, "held")
DATA = os.path.join(look.ART, "held")


def data(item):
    """The item's numbers, art/held/<item>.json."""
    with open(os.path.join(DATA, f"{item}.json")) as f:
        return json.load(f)


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
    d = data(item)
    source, out = d["source"], d["out"]
    look.reset()
    bpy.ops.import_scene.gltf(filepath=os.path.join(ASSETS, source))
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
    ob["carry_tilt"] = d["carry_tilt"]
    # Its kind, so a reader of the file alone tells it from a look held by its handle (D546).
    ob["hold"] = "hung"
    bpy.ops.object.select_all(action="DESELECT")
    ob.select_set(True)
    path = os.path.join(OUT, f"{item}.glb")
    bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_extras=True,
                              export_image_format="JPEG", export_jpeg_quality=85)
    print("HELD", item, f"hung from its handle, {out} m out from the leg")


def textured(item):
    """A textured model held by its points: its grip at the origin, its handle toward its
    working end along +Y, its own materials and maps kept."""
    d = data(item)
    source, grip, toward, x, carry_tilt = d["source"], d["grip"], d["toward"], d["out"], d["carry_tilt"]
    look.reset()
    bpy.ops.import_scene.gltf(filepath=os.path.join(ASSETS, source))
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
    if "strap" in d:
        ob["strap"] = d["strap"]
    bpy.ops.object.select_all(action="DESELECT")
    ob.select_set(True)
    path = os.path.join(OUT, f"{item}.glb")
    bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_extras=True,
                              export_tangents=True, export_image_format="JPEG", export_jpeg_quality=85)
    print("HELD", item, f"from {source}, its textures kept, carried at {carry_tilt} degrees")


os.makedirs(OUT, exist_ok=True)
for item in look.args():
    fit = data(item)["fit"]
    if fit == "hung":
        hung(item)
        continue
    if fit == "points":
        textured(item)
        continue
    d = data(item)
    source, length, held_from_end, carry_tilt = d["source"], d["length"], d["grip_from_end"], d["carry_tilt"]
    head = d.get("head_colour")
    look.reset()
    bpy.ops.import_scene.gltf(filepath=os.path.join(ASSETS, source))
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
