"""Fits gear made with Tripo to a base body, as the game wears it: one look per item per base
body, which the game finds by name and puts on as the file says (character.rs).

Rigid pieces (a pack, a cap, a quiver) are placed against the body at a socket, saved in that
socket's own space, and name it in their glTF extras ({"socket": "socket.back"}), so the game
hangs them there with nothing to adjust. Clothing that bends (a coat) is fitted to the torso,
pushed clear of the skin, given the body's skin weights, and saved on the same skeleton, so
the game binds it to the body's joints. Each piece reports how much of it rests on the body;
less than its kind should means it floats.

    spike/model_bakeoff/run.sh ../character_gear/fit_gear.py ../../crates/terra_sandbox/assets/models/character/base-male.glb small_pack large_pack leather_cap quiver

Clothing that bends is made on the body instead, by shell_garment.py.

Reads out/tripo/gear-<model>.glb in the bake-off folder; writes
crates/terra_sandbox/assets/models/gear/<body>/<item>.glb, the item named as in the game's
catalogue in lower case with `_` for spaces. Each piece's turn, size and fit are in PIECES,
found by looking at the models and then at the fitted result in the workbench; run it for
every base body.
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "model_bakeoff"))
sys.path.insert(0, HERE)

import bpy
from mathutils import Matrix, Vector

import look
import patches

GEAR = os.path.join(HERE, "..", "model_bakeoff", "out", "tripo")
OUT = os.path.join(HERE, "..", "..", "crates", "terra_sandbox", "assets", "models", "gear")

# item: (Tripo model, turn about Z in degrees so its front faces the body's front (-Y) and
# its body side faces the body, how it is fitted, size as a share of the body's height, and
# the role its material plays, which a tint in the game names to recolour it).
PIECES = {
    "small_pack": ("pack", 90, "back", 0.23, "Pack"),
    "large_pack": ("pack", 90, "back", 0.31, "Pack"),
    "leather_cap": ("cap", -90, "head", None, "Leather"),
    "quiver": ("quiver", 90, "belt", 0.4, "Leather"),
}

body_path, names = look.args()[0], look.args()[1:]
body_name = os.path.splitext(os.path.basename(body_path))[0]

look.reset()
bpy.ops.import_scene.gltf(filepath=os.path.join(look.HERE, body_path))
arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
body = max((o for o in bpy.context.scene.objects if o.type == "MESH"), key=lambda o: len(o.data.vertices))
# Each vertex's patch of skin (patches.py), which a piece may hide.
patch_of = patches.of(body) or {}
arm.animation_data_clear()
for pb in arm.pose.bones:
    pb.matrix_basis.identity()
bpy.context.view_layer.update()
sockets = {o.name: o for o in bpy.context.scene.objects if o.name.startswith("socket.")}
height = max((body.matrix_world @ v.co).z for v in body.data.vertices)
groups = {g.index: g.name for g in body.vertex_groups}


def region(bones):
    """Rest positions of the body's vertices whose strongest bone is one of `bones`."""
    pts = []
    for v in body.data.vertices:
        if v.groups:
            g = max(v.groups, key=lambda g: g.weight)
            if groups.get(g.group) in bones:
                pts.append(body.matrix_world @ v.co)
    return pts


def joint(name):
    return arm.matrix_world @ arm.data.bones[name].head_local


def points(ob):
    return [ob.matrix_world @ v.co for v in ob.data.vertices]


def box(pts):
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    return lo, hi


def select(*objs):
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]


def apply(ob, mod):
    select(ob)
    bpy.ops.object.modifier_apply(modifier=mod.name)


def import_gear(model, turn):
    before = set(bpy.context.scene.objects)
    bpy.ops.import_scene.gltf(filepath=os.path.join(GEAR, f"gear-{model}.glb"))
    new = [o for o in bpy.context.scene.objects if o not in before]
    ob = max((o for o in new if o.type == "MESH"), key=lambda o: len(o.data.vertices))
    mw = ob.matrix_world.copy()
    ob.parent = None
    ob.data.transform(Matrix.Rotation(math.radians(turn), 4, "Z") @ mw)
    ob.matrix_world = Matrix.Identity(4)
    for o in new:
        if o is not ob:
            bpy.data.objects.remove(o)
    return ob


def move(ob, m):
    ob.data.transform(m)
    ob.data.update()


def scale_to(ob, height):
    lo, hi = box(points(ob))
    move(ob, Matrix.Scale(height / (hi.z - lo.z), 4))


def push_out(ob, offset):
    mod = ob.modifiers.new("PushOut", "SHRINKWRAP")
    mod.target = body
    mod.wrap_method = "NEAREST_SURFACEPOINT"
    mod.wrap_mode = "OUTSIDE"
    mod.offset = offset
    apply(ob, mod)


def fit_back(ob, height):
    """On the upper back, the pack's own back panel just off the skin. Tripo models its
    shoulder straps as stiff loops standing out from that panel, so the panel is found from
    where most of the pack is, and the straps are laid onto the back and shoulders."""
    scale_to(ob, height)
    at = sockets["socket.back"].matrix_world.translation
    lo, hi = box(points(ob))
    middle = sorted(p.y for p in points(ob) if abs(p.x - (lo.x + hi.x) / 2) < (hi.x - lo.x) * 0.3)
    panel = middle[len(middle) * 3 // 10]
    move(ob, Matrix.Translation(Vector((-(lo.x + hi.x) / 2, at.y + 0.02 - panel, at.z - 0.06 - (lo.z + hi.z) / 2))))
    straps = ob.vertex_groups.new(name="Straps")
    straps.add([v.index for v in ob.data.vertices if (ob.matrix_world @ v.co).y < at.y + 0.015], 1.0, "REPLACE")
    lay = ob.modifiers.new("Straps", "SHRINKWRAP")
    lay.target = body
    lay.wrap_method = "NEAREST_SURFACEPOINT"
    lay.wrap_mode = "ABOVE_SURFACE"
    lay.offset = 0.012
    lay.vertex_group = "Straps"
    apply(ob, lay)
    ob.vertex_groups.remove(ob.vertex_groups["Straps"])
    push_out(ob, 0.01)
    return "socket.back"


def fit_head(ob):
    """Over the top of the head, a little wider than the skull."""
    head = region({"DEF-head"})
    top = max(p.z for p in head)
    crown = [p for p in head if p.z > top - 0.09]
    lo, hi = box(crown)
    g_lo, g_hi = box(points(ob))
    s = (hi.x - lo.x) * 1.12 / (g_hi.x - g_lo.x)
    move(ob, Matrix.Scale(s, 4))
    g_lo, g_hi = box(points(ob))
    centre = (lo + hi) / 2
    move(ob, Matrix.Translation(Vector((centre.x - (g_lo.x + g_hi.x) / 2, centre.y - (g_lo.y + g_hi.y) / 2, top + 0.012 - g_hi.z))))
    push_out(ob, 0.006)
    return "socket.head"


def fit_belt(ob, height):
    """Hanging at the right hip, its top tipped back a little, its inner side on the skin."""
    scale_to(ob, height)
    move(ob, Matrix.Rotation(math.radians(-15), 4, "X"))
    at = sockets["socket.belt.R"].matrix_world.translation
    lo, hi = box(points(ob))
    move(ob, Matrix.Translation(Vector((at.x - 0.01 - hi.x, at.y + 0.05 - (lo.y + hi.y) / 2, at.z - 0.42 - lo.z))))
    push_out(ob, 0.01)
    return "socket.belt.R"


def fit_torso(ob):
    """Sized to the torso from the hips to the shoulders, pushed clear of the skin, given the
    body's weights and bound to its skeleton."""
    hips = joint("DEF-hips")
    torso = region({"DEF-hips", "DEF-spine.001", "DEF-spine.002", "DEF-spine.003", "DEF-shoulder.L", "DEF-shoulder.R", "DEF-neck"})
    bottom = hips.z - 0.14
    shoulders = max(p.z for p in region({"DEF-shoulder.L", "DEF-shoulder.R"}))
    lo, hi = box([p for p in torso if bottom <= p.z <= shoulders])
    lo.z, hi.z = bottom, shoulders + 0.02
    g_lo, g_hi = box(points(ob))
    size, gsize = hi - lo, g_hi - g_lo
    pad = (1.08, 1.15, 1.0)
    scale = [size[i] * pad[i] / gsize[i] for i in range(3)]
    move(ob, Matrix.Translation((lo + hi) / 2) @ Matrix.Diagonal((*scale, 1)) @ Matrix.Translation(-(g_lo + g_hi) / 2))
    push_out(ob, 0.014)
    for g in body.vertex_groups:
        ob.vertex_groups.new(name=g.name)
    dt = ob.modifiers.new("Weights", "DATA_TRANSFER")
    dt.object = body
    dt.use_vert_data = True
    dt.data_types_verts = {"VGROUP_WEIGHTS"}
    dt.vert_mapping = "POLYINTERP_NEAREST"
    dt.layers_vgroup_select_src = "ALL"
    dt.layers_vgroup_select_dst = "NAME"
    apply(ob, dt)
    select(ob)
    bpy.ops.object.mode_set(mode="WEIGHT_PAINT")
    bpy.ops.object.vertex_group_limit_total(group_select_mode="ALL", limit=4)
    bpy.ops.object.vertex_group_normalize_all(group_select_mode="ALL", lock_active=False)
    bpy.ops.object.mode_set(mode="OBJECT")
    ob.parent = arm
    ob.matrix_parent_inverse = arm.matrix_world.inverted()
    ob.modifiers.new("Armature", "ARMATURE").object = arm
    return None


# A patch is covered when this share of its skin, looking straight out, meets the piece
# within REACH_M; covered patches are not drawn while the piece is worn.
COVERED = 0.9
REACH_M = 0.12


def covers(ob):
    """The patches of skin this piece hides from view."""
    from mathutils.bvhtree import BVHTree
    depsgraph = bpy.context.evaluated_depsgraph_get()
    tree = BVHTree.FromObject(ob, depsgraph)
    to_piece = ob.matrix_world.inverted() @ body.matrix_world
    turn = to_piece.to_3x3()
    hit, total = {}, {}
    for v in body.data.vertices:
        patch = patch_of.get(v.index)
        if patch is None:
            continue
        start = to_piece @ v.co
        normal = (turn @ v.normal).normalized()
        total[patch] = total.get(patch, 0) + 1
        hit[patch] = hit.get(patch, 0) + (tree.ray_cast(start + normal * 0.002, normal, REACH_M)[0] is not None)
    covered = sorted(p for p in total if hit[p] / total[p] >= COVERED)
    print("COVERAGE", ob.name, f"{len(covered)} of {len(total)} patches")
    return covered


def contact(ob):
    """The share of a piece's vertices within 3 cm of the skin: how much of it rests on the
    body rather than floating off it."""
    near = 0
    for p in points(ob):
        ok, hit, _, _ = body.closest_point_on_mesh(body.matrix_world.inverted() @ p)
        near += ok and (body.matrix_world @ hit - p).length < 0.03
    return near / max(len(ob.data.vertices), 1)


# The least share of a piece that should rest on the body, by how it is fitted; less means
# it floats, as a pack placed by the tips of its straps did.
TOUCHING = {"back": 0.08, "head": 0.3, "belt": 0.05, "torso": 0.4}

OUT = os.path.join(OUT, body_name)
os.makedirs(OUT, exist_ok=True)
for name in names:
    model, turn, how, size, role = PIECES[name]
    size = size * height if size else None
    ob = import_gear(model, turn)
    ob.name = name
    for slot in ob.material_slots:
        if slot.material:
            slot.material.name = role
    if how == "back":
        socket = fit_back(ob, size)
    elif how == "head":
        socket = fit_head(ob)
    elif how == "belt":
        socket = fit_belt(ob, size)
    else:
        socket = fit_torso(ob)
    hides = covers(ob) if how in ("torso", "head") else []
    if hides:
        ob["covers"] = hides
    touching = contact(ob)
    print("FIT", name, f"touching {touching:.0%}", "ok" if touching >= TOUCHING[how] else f"FLOATING, under {TOUCHING[how]:.0%}")
    path = os.path.join(OUT, f"{name}.glb")
    if socket:
        # In the socket's own space: the game puts it on the socket as it is.
        move(ob, sockets[socket].matrix_world.inverted())
        ob["socket"] = socket
        select(ob)
        bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_animations=False,
                                  export_extras=True)
    else:
        select(ob, arm)
        bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_animations=False,
                                  export_influence_nb=4, export_extras=True)
    print("FITTED", name, "on", socket or "the skeleton", len(ob.data.vertices), "vertices", "covers", hides)
    bpy.data.objects.remove(ob)
