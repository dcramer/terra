"""Fits gear made with Tripo to a character, as the game wears it.

Rigid pieces (a pack, a cap, a quiver) are placed against the body at a socket and saved in
that socket's own space, so the game hangs them there with nothing to adjust. Clothing that
bends (a coat) is fitted to the torso, pushed clear of the skin, given the body's skin
weights, and saved on the same skeleton, so the game binds it to the body's joints.

    spike/model_bakeoff/run.sh ../character_gear/fit_gear.py ../../crates/terra_sandbox/assets/models/character/base-male.glb pack-small pack-large cap coat quiver

Reads out/tripo/gear-<model>.glb in the bake-off folder; writes
crates/terra_sandbox/assets/models/gear/<name>.glb. Each piece's turn, size and place are in
PIECES, found by looking at the models and then at the fitted result in the workbench.
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "model_bakeoff"))

import bpy
from mathutils import Matrix, Vector

import look

GEAR = os.path.join(HERE, "..", "model_bakeoff", "out", "tripo")
OUT = os.path.join(HERE, "..", "..", "crates", "terra_sandbox", "assets", "models", "gear")

# name: (Tripo model, turn about Z in degrees so its front faces the body's front (-Y) and
# its body side faces the body, how it is fitted, size in metres).
PIECES = {
    "pack-small": ("pack", 90, "back", 0.42),
    "pack-large": ("pack", 90, "back", 0.55),
    "cap": ("cap", -90, "head", None),
    "coat": ("coat", -90, "torso", None),
    "quiver": ("quiver", 90, "belt", 0.72),
}

body_path, names = look.args()[0], look.args()[1:]

look.reset()
bpy.ops.import_scene.gltf(filepath=os.path.join(look.HERE, body_path))
arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
body = max((o for o in bpy.context.scene.objects if o.type == "MESH"), key=lambda o: len(o.data.vertices))
arm.animation_data_clear()
for pb in arm.pose.bones:
    pb.matrix_basis.identity()
bpy.context.view_layer.update()
sockets = {o.name: o for o in bpy.context.scene.objects if o.name.startswith("socket.")}
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
    """On the upper back, its body side just off the skin."""
    scale_to(ob, height)
    at = sockets["socket.back"].matrix_world.translation
    lo, hi = box(points(ob))
    move(ob, Matrix.Translation(Vector((-(lo.x + hi.x) / 2, at.y + 0.01 - lo.y, at.z - 0.06 - (lo.z + hi.z) / 2))))
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


os.makedirs(OUT, exist_ok=True)
for name in names:
    model, turn, how, size = PIECES[name]
    ob = import_gear(model, turn)
    ob.name = name
    if how == "back":
        socket = fit_back(ob, size)
    elif how == "head":
        socket = fit_head(ob)
    elif how == "belt":
        socket = fit_belt(ob, size)
    else:
        socket = fit_torso(ob)
    path = os.path.join(OUT, f"{name}.glb")
    if socket:
        # In the socket's own space: the game puts it on the socket as it is.
        move(ob, sockets[socket].matrix_world.inverted())
        select(ob)
        bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_animations=False)
    else:
        select(ob, arm)
        bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_animations=False,
                                  export_influence_nb=4)
    print("FITTED", name, "on", socket or "the skeleton", len(ob.data.vertices), "vertices")
    bpy.data.objects.remove(ob)
