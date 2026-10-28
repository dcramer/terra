"""Makes the look of each thing that is held: one file per item, models/held/<item>.glb, that
the game puts in a hand socket as it is (character.rs), or, strapped (a shield, its "strap"), on the
forearm's bone where its extras say.

Each look is one mesh with the place it is held at the origin and its handle along +Y toward
its working end, as a hand socket expects. Carried rather than used, the game tips it by a rule
(terra_walker::character::carry_rule); a "carry_tilt" in its data, written to its glTF extras,
wins over the rule: -45 tips its head forward and down in a hanging hand, positive raises it.
None needs one today. Body-independent: a hand socket is the same on every body.

Its materials are named by role (Wood, Metal, Trim, String), so a colour given for a role
reaches every part of it: here, a stone or iron head; in the game, a tint (character.rs).

    tools/blender/run.sh tools/characters/fit_held.py hammer stone_axe arming_sword basket ...

Each item's numbers are data, art/held/<item>.json (docs/attachment.md, D546), so adding an
item is adding a file, never editing this script. Its "fit" says how the look is made:

- "length": a model standing on the end of its handle, along its height (the free tool models
  in assets/models/tools, THIRD_PARTY.md, and ours made by tools/props): scaled to "length"
  metres, held "grip_from_end" metres up from its foot, centred there on its handle's section,
  turned "turn" degrees about +Y (so a one-sided head faces -Z, a blade's edges lie in Y-Z), its
  working end coloured "head_colour" where its material shows there.
- "points": a textured model of ours (tools/props, "into": "tools"), kept in its own materials
  and maps: "grip", the point held at the middle of the fist; "toward", a point its handle runs
  toward at its working end (the look's +Y); "out", the direction that becomes the look's +X,
  out of the back of a right hand and the palm of a left. Points and directions in metres in
  the source's glTF axes (X, Y up, Z its front), as the workbench and props.ts's sheets show.
- "hung": a thing carried hanging from the hand by a handle over its top, as a basket is, from
  its prop: "out", how far out from the leg it hangs. The hand's socket in a hanging arm has +Z
  up the arm, +Y forward and +X out from the body, so it hangs down -Z from its handle's top,
  its long way forward along +Y; its extras say {"hold": "hung"}.

Every item gives "source" (under assets/models), and may give "carry_tilt" (degrees) and
"strap": strapped to the forearm rather than hung on the hand socket, said in its extras as
{"strap": {"bone", "fist", "face", "grip"}} for the game (terra_walker::strap) and moves.ts: the
bone it rides on; the hand socket whose fist closes on its origin (its hand strap), wherever that
body's fist is; and the directions its front (-Z) and grip (+Y) turn to in the true forearm's
frame (the bone's, turned to run from the elbow to that body's wrist). "note" says why, for people.

Held in two hands, it gives where the other hand goes, in the look's own space, metres, and its
extras say the same (docs/attachment.md, "Second grip"): "second", how far along +Y the off hand
closes on its handle (a spear's, a greatsword's; negative behind the grip); "nock", [x, y, z], a
bow's string's middle at rest, which the drawing hand takes.

Writes assets/models/held/<item>.glb, the item named as in the game's catalogue in lower case
with `_` for spaces.
"""

import hashlib
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


def source_sha(source):
    """The first 12 hex digits of the SHA-256 of its source's bytes, its extras' source_sha
    (D552), which attach_contract.ts compares with the source as committed."""
    with open(os.path.join(ASSETS, source), "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()[:12]


def carried(ob, d):
    """Its data's "carry_hold" (a carry pose, the clip whose first pose the arm holding it takes
    while it is carried, D560) or "carry_tilt", into its extras, only where given: the game
    carries every other look by its rule (terra_walker::character::carry_rule). Says how, for
    the HELD line."""
    if "carry_hold" in d:
        ob["carry_hold"] = d["carry_hold"]
        return f"in its carry pose {d['carry_hold']}"
    if "carry_tilt" not in d:
        return "by the rule"
    ob["carry_tilt"] = d["carry_tilt"]
    return f"at {d['carry_tilt']} degrees, its data's"


def second_hand(ob, d):
    """Where its other hand goes, if it is held in two, into its extras as its data says."""
    for key in ("second", "nock"):
        if key in d:
            ob[key] = d[key]


def handle_middle(ob):
    """The middle of where the plane z = 0 cuts the mesh's edges, across X and Y: the handle's
    section at the grip, as attach_contract.ts's handleAt measures it."""
    vs = ob.data.vertices
    cuts = []
    for e in ob.data.edges:
        a, b = vs[e.vertices[0]].co, vs[e.vertices[1]].co
        if (a.z < 0) == (b.z < 0):
            continue
        k = a.z / (a.z - b.z)
        cuts.append(a + (b - a) * k)
    if not cuts:
        raise SystemExit(f"{ob.name}: nothing crosses its grip; check its grip_from_end")
    xs, ys = [c.x for c in cuts], [c.y for c in cuts]
    return Vector(((min(xs) + max(xs)) / 2, (min(ys) + max(ys)) / 2, 0))


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
    carried(ob, d)
    ob["source_sha"] = source_sha(d["source"])
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
    source, grip, toward, x = d["source"], d["grip"], d["toward"], d["out"]
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
    tilt = carried(ob, d)
    ob["source_sha"] = source_sha(d["source"])
    if "strap" in d:
        ob["strap"] = d["strap"]
    second_hand(ob, d)
    bpy.ops.object.select_all(action="DESELECT")
    ob.select_set(True)
    path = os.path.join(OUT, f"{item}.glb")
    bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_extras=True,
                              export_tangents=True, export_image_format="JPEG", export_jpeg_quality=85)
    print("HELD", item, f"from {source}, its textures kept, carried {tilt}")


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
    source, length, held_from_end = d["source"], d["length"], d["grip_from_end"]
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
    # The grip on the handle's middle where the fist closes (the plane z = 0 here, glTF's y = 0),
    # not the bounds' middle, which a head's overhang moves off the handle (docs/attachment.md,
    # "The grip across the handle"); then turned "turn" degrees about the handle (Blender's +Z
    # is glTF's +Y, and a turn about it is the same turn there), so a one-sided head faces -Z.
    ob.data.transform(Matrix.Rotation(math.radians(d.get("turn", 0)), 4, "Z") @ Matrix.Translation(-handle_middle(ob)))
    for slot in ob.material_slots:
        if slot.material and slot.material.use_nodes and not maps:
            bsdf = slot.material.node_tree.nodes["Principled BSDF"]
            bsdf.inputs["Roughness"].default_value = 0.7
            bsdf.inputs["Metallic"].default_value = 0.0
    tilt = carried(ob, d)
    ob["source_sha"] = source_sha(d["source"])
    second_hand(ob, d)
    bpy.ops.object.select_all(action="DESELECT")
    ob.select_set(True)
    path = os.path.join(OUT, f"{item}.glb")
    kept = {"export_tangents": True, "export_image_format": "JPEG", "export_jpeg_quality": 85} if maps else {}
    bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_extras=True, **kept)
    print("HELD", item, f"{length} m, held {held_from_end} m from its end, carried {tilt}",
          "its textures kept" if maps else "")
