"""Fits gear made with Tripo to a base body, as the game wears it: one look per item per base
body, which the game finds by name and puts on as the file says (character.rs).

Rigid pieces (a pack, a cap, a quiver) are placed against the body at a socket, saved in that
socket's own space, and name it in their glTF extras ({"socket": "socket.back"}), so the game
hangs them there with nothing to adjust. Clothing that bends (a coat) is fitted to the torso,
pushed clear of the skin, given the body's skin weights, and saved on the same skeleton, so
the game binds it to the body's joints. Each piece reports how much of it rests on the body;
less than its kind should means it floats.

    tools/blender/run.sh tools/characters/fit_gear.py tools/characters/out/base-male-game.glb small_pack large_pack quiver

Clothing, and anything worn close on the body such as a cap, is made on the body instead, by
shell_garment.py.

Reads art/gear/<item>/model.glb and its numbers, art/gear/<item>/fit.json (below); writes
tools/characters/out/gear/<body>/<item>.glb (look.GEAR_MADE), the item named as in the game's
catalogue in lower case with `_` for spaces. A piece's turn, size and fit are found by
looking at the model and then at the fitted result in the workbench; run it for every base
body.
"""

import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bmesh
import bpy
from mathutils import Matrix, Vector

import bands
import look
import patches
import skin_piece

GEAR = os.path.join(look.ART, "gear")
OUT = look.GEAR_MADE

# Each piece's numbers are data beside its model, art/gear/<item>/fit.json (docs/attachment.md,
# D546), so a new piece is files, never an edit here:
#   "turn"      about Z in degrees, so its front faces the body's front (-Y) and its body side
#               the body (Tripo turns each model its own way);
#   "fit"       how it is fitted: "back" (a pack's bag), "belt" (hung from the belt), "head",
#               "torso";
#   "size"      its height as a share of the body's;
#   "role"      the role its material plays, which a tint in the game names to recolour it;
#   "harness"   a pack's: the harness its bag rides on (D189), one of art/gear/harnesses.json,
#               each made from a model of the base person wearing one, whose straps it takes
#               its look from. Many bags, few harnesses: a new pack is a bag model naming one;
#   "hangs_at"  a belt piece's: where it hangs from, as a share of its height from its foot (a
#               quiver's mouth, the top of its leather tube, below the arrows standing in it),
#               read off its concept as an artist sets an attach point; the mesh alone cannot
#               tell the tube from the fanned fletching;
#   "swing"     a loose piece's (crates/terra_walker/src/swing.rs): its stiffness, drag and
#               gravity; its tail, where it rests, is measured from the piece itself;
#   "note"      why, for people.
HARNESSES = os.path.join(GEAR, "harnesses.json")
HARNESS_TEXTURE = 2048
HARNESS_LOOK_M = 0.04  # how far out and in from a strap the bake looks for the dressed one


def data(name):
    """A piece's numbers, art/gear/<item>/fit.json."""
    with open(os.path.join(GEAR, name, "fit.json")) as f:
        return json.load(f)
# A loose piece hangs nearly plumb from where it hangs, its foot a little back, resting on
# the seat.
HANG_TILT_DEG = 8

body_path, names = look.args()[0], look.args()[1:]
body_name = look.body_name(body_path)

look.reset()
bpy.ops.import_scene.gltf(filepath=look.path(body_path))
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
height_of_body = height
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
    bpy.ops.import_scene.gltf(filepath=os.path.join(GEAR, model, "model.glb"))
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


def pull_in(ob, direction, gap):
    """Moves `ob` along `direction` (a unit vector, world space) until its nearest part, along
    that line, is `gap` from the skin: a piece set against the body by contact, not by one
    measured point of the skin, which may stand out (a fold, a flap) where the piece is not."""
    inv = body.matrix_world.inverted()
    d_local = (inv.to_3x3() @ direction).normalized()
    best = None
    for p in points(ob)[::3]:
        hit, at, _, _ = body.ray_cast(inv @ p, d_local, distance=1.0)
        if hit:
            travel = ((body.matrix_world @ at) - p).length
            best = travel if best is None else min(best, travel)
    if best is not None:
        move(ob, Matrix.Translation(direction * (best - gap)))
    return best


# Heights off the skin, by slot (docs/characters.md): chest pieces stay within CHEST_M
# (shell_garment.py's BULK_M); what is worn over them, a pack's straps, reaches past it.
CHEST_M = 0.018
OVER_CHEST_M = 0.022


def fit_back(ob, height, harness):
    """A pack as it is worn: its bag riding the upper back, its straps made on the body.

    The bag's top is level with the top of the shoulders, which this body measures for itself,
    its back panel just off the skin. Tripo models shoulder straps as stiff loops standing out
    of the panel; they are cut away, and straps are made as bands of the body's own surface
    (bands.py) from the bag's top corners over the shoulders, down the chest, under the arms
    and back under the bag, so they bend as the shoulders do and cannot clip. Their ends are
    set on the body, not on this bag, where any bag covers them, so one harness serves every
    bag (D189); their look is baked from `harness`, a model of the base person wearing one.
    The bag rides the upper spine whole."""
    scale_to(ob, height)
    at = sockets["socket.back"].matrix_world.translation
    shoulders = max(p.z for p in region({"clavicle_l", "clavicle_r"}))
    lo, hi = box(points(ob))
    middle = sorted(p.y for p in points(ob) if abs(p.x - (lo.x + hi.x) / 2) < (hi.x - lo.x) * 0.3)
    panel = middle[len(middle) * 3 // 10]
    move(ob, Matrix.Translation(Vector((-(lo.x + hi.x) / 2, at.y + 0.02 - panel, shoulders - hi.z))))
    # The loops: whatever stands in front of the panel, toward the body.
    front = at.y + 0.015
    bm = bmesh.new()
    bm.from_mesh(ob.data)
    loops = [f for f in bm.faces if any((ob.matrix_world @ v.co).y < front for v in f.verts)]
    bmesh.ops.delete(bm, geom=loops, context="FACES")
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
    bm.to_mesh(ob.data)
    bm.free()
    # Then against the back by contact: forward (the body faces -Y) until it rests just off
    # the skin, whatever this bag's own panel is like.
    pull_in(ob, Vector((0.0, -1.0, 0.0)), 0.008)
    push_out(ob, 0.01)
    lo, hi = box(points(ob))
    rider = ob.vertex_groups.new(name="spine_03")
    rider.add([v.index for v in ob.data.vertices], 1.0, "REPLACE")
    # The straps, from this body's own joints and the bag's corners.
    chest = joint("spine_03").z
    leather = bpy.data.materials.new("Leather")
    straps = []
    for side in (1, -1):
        reach = abs(joint(f"upperarm_{'l' if side > 0 else 'r'}").x)
        x = side * reach
        route = [
            ((0.4 * x, 1.0, shoulders - 0.06), (0, -1, 0)),      # under the bag's top, in from behind
            ((0.7 * x, 0.0, shoulders + 0.5), (0, 0, -1)),       # down onto the top of the shoulder
            ((0.6 * x, -1.0, chest + 0.02), (0, 1, 0)),          # in onto the chest
            ((0.85 * x, -1.0, chest - 0.1), (0, 1, 0)),
            ((3.0 * x, 0.0, chest - 0.13), (-side, 0, 0)),       # in onto the side, under the arm
            ((0.6 * x, 1.0, chest - 0.18), (0, -1, 0)),          # under the bag's lower part, in from behind
        ]
        # Solid from just off the skin to above the thickest chest piece (OVER_CHEST_M).
        straps.append(bands.band(body, route, width=0.045, offset=OVER_CHEST_M, inner=0.006,
                                 name=f"strap.{side}", material=leather))
    # The harness's look, from the dressed model's straps.
    bpy.ops.object.select_all(action="DESELECT")
    for o in straps:
        o.select_set(True)
    bpy.context.view_layer.objects.active = straps[0]
    bpy.ops.object.join()
    dressed = skin_piece.line_up(look.path(harness), bands.tree_of(body), height_of_body, 0.03)
    # Its own role, so a tint can recolour it apart from the bag and its icon can leave it out
    # (it is made on the body, and hangs loose without one).
    skin_piece.bake(straps[0], dressed, f"{ob.name}_harness", "Harness", HARNESS_TEXTURE, HARNESS_LOOK_M)
    bpy.data.objects.remove(dressed)
    bpy.ops.object.select_all(action="DESELECT")
    for o in (ob, straps[0]):
        o.select_set(True)
    bpy.context.view_layer.objects.active = ob
    bpy.ops.object.join()
    ob.parent = arm
    ob.matrix_parent_inverse = arm.matrix_world.inverted()
    ob.modifiers.new("Armature", "ARMATURE").object = arm
    return None


def skin_weights(ob, near, rest):
    """Binds a piece to the skeleton: vertices within `near` of the skin take the body's
    weights there, fading to the bone `rest` alone further out."""
    from mathutils.bvhtree import BVHTree
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
    skin = BVHTree.FromObject(body, bpy.context.evaluated_depsgraph_get())
    rider = ob.vertex_groups[rest]
    for v in ob.data.vertices:
        p = body.matrix_world.inverted() @ (ob.matrix_world @ v.co)
        hit = skin.find_nearest(p)
        d = (p - hit[0]).length if hit[0] is not None else near * 3
        keep = max(0.0, min(1.0, 1.0 - (d - near) / near))
        for g in v.groups:
            g.weight *= keep
        rider.add([v.index], 1.0 - keep, "ADD")
    select(ob)
    bpy.ops.object.mode_set(mode="WEIGHT_PAINT")
    bpy.ops.object.vertex_group_limit_total(group_select_mode="ALL", limit=4)
    bpy.ops.object.vertex_group_normalize_all(group_select_mode="ALL", lock_active=False)
    bpy.ops.object.mode_set(mode="OBJECT")
    ob.parent = arm
    ob.matrix_parent_inverse = arm.matrix_world.inverted()
    ob.modifiers.new("Armature", "ARMATURE").object = arm


def fit_head(ob):
    """Over the top of the head, a little wider than the crown. A rigid cap cuts into the head
    above the ears, where the head is wider than the crown (gear_check.py); sizing it to its
    rim instead made that worse. A cap should be made on the head as clothing is
    (shell_garment.py), which is not done yet."""
    head = region({"Head"})
    top = max(p.z for p in head)
    crown = [p for p in head if p.z > top - 0.09]
    lo, hi = box(crown)
    g_lo, g_hi = box(points(ob))
    s = (hi.x - lo.x) * 1.12 / (g_hi.x - g_lo.x)
    move(ob, Matrix.Scale(s, 4))
    g_lo, g_hi = box(points(ob))
    centre = (lo + hi) / 2
    move(ob, Matrix.Translation(Vector((centre.x - (g_lo.x + g_hi.x) / 2, centre.y - (g_lo.y + g_hi.y) / 2, top + 0.012 - g_hi.z))))
    push_out(ob, 0.012)
    return "socket.head"


def fit_belt(ob, height, hangs_at):
    """Hanging from the belt behind the right hip, clear of the hand as the arm swings: its
    mouth (`hangs_at` of its height up from its foot) at the belt against the back of the hip,
    nearly plumb with its foot a little back, as a loose hip quiver hangs from its strap."""
    scale_to(ob, height)
    move(ob, Matrix.Rotation(math.radians(HANG_TILT_DEG), 4, "X"))
    at = sockets["socket.belt.R"].matrix_world.translation
    across = at.x * 0.55
    waist = [p for p in region({"pelvis", "spine_01", "thigh_r"})
             if abs(p.z - at.z) < 0.06 and abs(p.x - across) < 0.05]
    back = max(p.y for p in waist)
    lo, hi = box(points(ob))
    move(ob, Matrix.Translation(Vector((across - (lo.x + hi.x) / 2, back + 0.005 - lo.y, at.z - hangs_at * (hi.z - lo.z) - lo.z))))
    # Then against the hip by contact: forward (the body faces -Y) until it rests 1 cm off.
    pull_in(ob, Vector((0.0, -1.0, 0.0)), 0.01)
    push_out(ob, 0.01)
    return "socket.belt.R"


def fit_torso(ob):
    """Sized to the torso from the hips to the shoulders, pushed clear of the skin, given the
    body's weights and bound to its skeleton."""
    hips = joint("pelvis")
    torso = region({"pelvis", "spine_01", "spine_02", "spine_03", "clavicle_l", "clavicle_r", "neck_01"})
    bottom = hips.z - 0.14
    shoulders = max(p.z for p in region({"clavicle_l", "clavicle_r"}))
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
# A pack's bag stands further off the back than a garment does off the skin.
REACH_BACK_M = 0.3


def covers(ob, reach=REACH_M):
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
        hit[patch] = hit.get(patch, 0) + (tree.ray_cast(start + normal * 0.002, normal, reach)[0] is not None)
    covered = sorted(p for p in total if hit[p] / total[p] >= COVERED)
    print("COVERAGE", ob.name, f"{len(covered)} of {len(total)} patches")
    return covered


def contact(ob):
    """The share of a piece's vertices within 3 cm of the skin, how much of it rests on the
    body rather than floating off it, and how near its nearest part comes."""
    near, nearest = 0, float("inf")
    for p in points(ob):
        ok, hit, _, _ = body.closest_point_on_mesh(body.matrix_world.inverted() @ p)
        if ok:
            d = (body.matrix_world @ hit - p).length
            near += d < 0.03
            nearest = min(nearest, d)
    return near / max(len(ob.data.vertices), 1), nearest


# The least share of a piece that should rest on the body, by how it is fitted; less means
# it floats, as a pack placed by the tips of its straps did. A piece hung from the belt is
# long and hangs at an angle, touching in one place: it need only come within RESTS_M.
TOUCHING = {"back": 0.08, "head": 0.3, "belt": 0.0, "torso": 0.4}
RESTS_M = 0.02

OUT = os.path.join(OUT, body_name)
os.makedirs(OUT, exist_ok=True)
for name in names:
    d = data(name)
    model, turn, how, size, role = d.get("model", name), d["turn"], d["fit"], d["size"], d["role"]
    size = size * height if size else None
    ob = import_gear(model, turn)
    ob.name = name
    for slot in ob.material_slots:
        if slot.material:
            slot.material.name = role
    if how == "back":
        with open(HARNESSES) as f:
            socket = fit_back(ob, size, json.load(f)[d["harness"]])
    elif how == "head":
        socket = fit_head(ob)
    elif how == "belt":
        socket = fit_belt(ob, size, d["hangs_at"])
    else:
        socket = fit_torso(ob)
    # A pack hides the skin under its bag and straps as a garment does: that skin cannot be
    # seen, and a heavier back pressing into the bag would otherwise count as coming through.
    if how == "back":
        ob["icon_leaves_out"] = ["Harness"]
        ob["icon_turn"] = 180  # seen from behind its wearer, as a pack is
    hides = covers(ob, REACH_BACK_M if how == "back" else REACH_M) if how in ("torso", "head", "back") else []
    if hides:
        ob["covers"] = hides
    touching, nearest = contact(ob)
    rests = touching >= TOUCHING[how] and nearest <= RESTS_M
    print("FIT", name, f"touching {touching:.0%}, nearest {nearest * 100:.1f} cm",
          "ok" if rests else f"FLOATING: under {TOUCHING[how]:.0%} touching, or nothing within {RESTS_M * 100:.0f} cm")
    path = os.path.join(OUT, f"{name}.glb")
    if socket:
        # Where a loose piece hangs from, before it moves: the middle of it, at the height it
        # hangs at (a quiver's mouth).
        if "swing" in d:
            lo, hi = box(points(ob))
            pivot = Vector(((lo.x + hi.x) / 2, (lo.y + hi.y) / 2, lo.z + d["hangs_at"] * (hi.z - lo.z)))
        # In the socket's own space: the game puts it on the socket as it is.
        move(ob, sockets[socket].matrix_world.inverted())
        ob["socket"] = socket
        if "swing" in d:
            # Its origin where it hangs from, which it swings about (swing.rs), and its tail at
            # its middle from there, in its own space and glTF's axes.
            at = sockets[socket].matrix_world.inverted() @ pivot
            ob.data.transform(Matrix.Translation(-at))
            ob.matrix_world = Matrix.Translation(at)
            # Its tail: the middle of what hangs below where it hangs from (the vertices' own
            # middle sits up in the arrows, which have the most of them).
            below = [v.co for v in ob.data.vertices if v.co.z < 0]
            blo, bhi = box(below)
            c = (blo + bhi) / 2
            ob["swing"] = dict(d["swing"], tail=[round(c.x, 4), round(c.z, 4), round(-c.y, 4)])
        select(ob)
        bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_animations=False,
                                  export_extras=True)
    else:
        select(ob, arm)
        bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_animations=False,
                                  export_influence_nb=4, export_extras=True)
    print("FITTED", name, "on", socket or "the skeleton", len(ob.data.vertices), "vertices", "covers", hides)
    bpy.data.objects.remove(ob)
