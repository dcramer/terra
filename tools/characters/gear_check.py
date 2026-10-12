"""Checks worn looks on a body through our animations, so clipping is caught by numbers rather
than by eye (docs/characters.md, D128).

For each look it watches the skin near it at rest, in patches it does not hide (hidden patches
are not drawn, so cannot show). Through each clip it counts that skin which shows through the
look: a ray from the skin inward meets the look within DEPTH, and the skin is not inside the look
(a ray outward does not leave it, within COVERED, through a face turned the ray's way). That is skin poking out through clothing, or a rigid look sunk into the
body where the skin shows round it, and it holds for any mesh, single-sided or closed. It also watches the skin the look hides (its patches are not
drawn while it is worn) and lies close to at rest: wherever the look moves away from that skin
(a hem lifting off a thigh), the body shows a hole, counted per clip under "holes". Skin that
has passed into a closed piece (a heavy back into a pack's bag) is inside it, covered by it,
and not counted: until 2026-10-12 it was, which failed packs on skin no one could see. Prints one GEARCHECK line of JSON per look; a look passes when,
in every frame, no more than `through_share` of the skin it watches is through it.

    tools/blender/run.sh tools/characters/gear_check.py tools/characters/out/base-male-game.glb leather_coat leather_cap small_pack quiver
"""

import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy
import numpy as np
from mathutils.bvhtree import BVHTree

import look
import patches

GEAR = look.GEAR_MADE
CLIPS = ("Idle_Loop", "Walk_Loop", "Jog_Fwd_Loop", "Sword_Attack", "Crouch_Idle_Loop", "Roll")
LIMITS = {
    "near_m": 0.06,          # skin this close to a look at rest is watched
    "depth_m": 0.03,         # the look's surface this far beneath the skin means it is through
    "through_share": 0.02,   # share of the watched skin allowed through, in any frame
    "covered_m": 0.3,        # the look's surface this far outward hides skin that is through it
    "hidden_near_m": 0.03,   # skin the look hides, with the look this close at rest, is watched for holes
    "hole_m": 0.05,          # ... and is a hole in a frame where the look is further from it than this
    "hole_share": 0.01,      # share of the skin a look hides allowed uncovered (a hole), in any frame
}

body_path, items = look.args()[0], [a for a in look.args()[1:] if a != "--explain"]
# With --explain, say where the first few points of skin found through a piece at rest are.
explain = "--explain" in look.args()
body_name = look.body_name(body_path)

look.reset()
bpy.ops.import_scene.gltf(filepath=look.path(body_path))
arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
body = max((o for o in bpy.context.scene.objects if o.type == "MESH"), key=lambda o: len(o.data.vertices))
sockets = {o.name: o for o in bpy.context.scene.objects if o.name.startswith("socket.")}
patch_of = patches.of(body) or {}
actions = {a.name.split(".")[0]: a for a in bpy.data.actions}
for pb in arm.pose.bones:
    pb.matrix_basis.identity()
ad = arm.animation_data or arm.animation_data_create()
for track in list(ad.nla_tracks):
    ad.nla_tracks.remove(track)
# The importer leaves an animation active, which would pose the body for the first look.
ad.action = None
bpy.context.view_layer.update()


def evaluated(ob):
    """Its vertices and their normals, as posed now."""
    depsgraph = bpy.context.evaluated_depsgraph_get()
    ev = ob.evaluated_get(depsgraph)
    me = ev.to_mesh()
    turn = ev.matrix_world.to_3x3()
    pts = [ev.matrix_world @ v.co for v in me.vertices]
    nrm = [(turn @ v.normal).normalized() for v in me.vertices]
    ev.to_mesh_clear()
    return pts, nrm


def tree(ob):
    """The look as posed now, in world space, to cast rays at."""
    depsgraph = bpy.context.evaluated_depsgraph_get()
    ev = ob.evaluated_get(depsgraph)
    me = ev.to_mesh()
    verts = [ev.matrix_world @ v.co for v in me.vertices]
    faces = [tuple(p.vertices) for p in me.polygons]
    ev.to_mesh_clear()
    return BVHTree.FromPolygons(verts, faces)


body_faces = [tuple(p.vertices) for p in body.data.polygons]


def through(pts, nrm, bvh, which):
    """Which of `which` show through the look: the look's surface is just beneath them, inside
    the body (an inward ray meets it before the ray leaves the body again: through an ear, it
    would meet a cap above the ear, which is not skin through the cap), and nothing of the look
    covers them from outside. Skin that has passed into a closed piece, such as a back pressing
    into a pack's bag, is inside it and hidden by it, so it is not counted."""
    flesh = BVHTree.FromPolygons(pts, body_faces)
    out = []
    for i in which:
        start = pts[i] - nrm[i] * 0.001
        below = bvh.ray_cast(start, -nrm[i], LIMITS["depth_m"])
        above = bvh.ray_cast(pts[i] + nrm[i] * 0.001, nrm[i], LIMITS["covered_m"])
        leaves = flesh.ray_cast(start - nrm[i] * 0.001, -nrm[i], LIMITS["depth_m"])
        # Inside the piece: the ray outward leaves it through a face turned the ray's way.
        # Another part of the piece met face on (a coat's front, seen from a bent thigh) does
        # not hide the skin.
        inside = above[0] is not None and above[1].dot(nrm[i]) > 0
        out.append(below[0] is not None
                   and not inside
                   and (leaves[0] is None or below[3] < leaves[3]))
    return np.array(out)


def holes(pts, bvh, which):
    """Which of `which`, skin a look hides (not drawn while it is worn) and lies close to at
    rest, the look has moved away from: nothing of it within hole_m, as where a hem lifts off a
    thigh. There the body shows a hole."""
    return np.array([bvh.find_nearest(pts[i], LIMITS["hole_m"])[0] is None for i in which])


def wear(item):
    before = set(bpy.context.scene.objects)
    # An item is named as in the catalogue, or given as a path to a look (a .glb).
    given = item if item.endswith(".glb") else os.path.join(GEAR, body_name, f"{item}.glb")
    bpy.ops.import_scene.gltf(filepath=os.path.join(look.ROOT, given))
    new = [o for o in bpy.context.scene.objects if o not in before]
    piece = max((o for o in new if o.type == "MESH"), key=lambda o: len(o.data.vertices))
    covers = set(piece.get("covers", []))
    socket = piece.get("socket")
    mw = piece.matrix_world.copy()
    piece.parent = None
    piece.modifiers.clear()
    if socket:
        # In the socket's space: hang it there.
        piece.matrix_world = mw
        local = piece.matrix_world.copy()
        piece.parent = sockets[socket]
        piece.matrix_parent_inverse.identity()
        piece.matrix_basis = local
    else:
        piece.matrix_world = mw
        piece.parent = arm
        piece.matrix_parent_inverse = arm.matrix_world.inverted()
        piece.modifiers.new("Armature", "ARMATURE").object = arm
    for o in new:
        if o is not piece:
            bpy.data.objects.remove(o)
    # The body and the piece skinned alike, as the game skins them.
    look.skin_as_game()
    bpy.context.view_layer.update()
    return piece, covers, socket


groups = {g.index: g.name for g in body.vertex_groups}
bone_of = [groups.get(max(v.groups, key=lambda g: g.weight).group, "") if v.groups else ""
           for v in body.data.vertices]
results = []
for item in items:
    piece, covers, socket = wear(item)
    pts, nrm = evaluated(body)
    rest_pts = list(pts)
    bvh = tree(piece)
    watch = [i for i in range(len(pts)) if patch_of.get(i) not in covers
             and bvh.find_nearest(pts[i], LIMITS["near_m"])[0] is not None]
    # The skin it hides, every few vertices: a hole wherever the look stops covering it.
    hidden = [i for i in range(0, len(pts), 3) if patch_of.get(i) in covers
              and bvh.find_nearest(pts[i], LIMITS["hidden_near_m"])[0] is not None]
    at_rest = float(np.mean(through(pts, nrm, bvh, watch))) if watch else 0.0
    if explain:
        lo = [min(c) for c in zip(*[piece.matrix_world @ v.co for v in piece.data.vertices])]
        hi = [max(c) for c in zip(*[piece.matrix_world @ v.co for v in piece.data.vertices])]
        print("PLACED", item, "on", socket, "at", tuple(round(c, 3) for c in (sockets[socket].matrix_world.translation if socket else (0, 0, 0))),
              "spans", tuple(round(c, 2) for c in lo), "to", tuple(round(c, 2) for c in hi),
              "body top", round(max(p.z for p in pts), 3))
    if explain and watch:
        hits = through(pts, nrm, bvh, watch)
        for i in np.array(watch)[hits][:12]:
            below = bvh.ray_cast(pts[i] - nrm[i] * 0.001, -nrm[i], LIMITS["depth_m"])
            print("THROUGH", item, bone_of[i], "at", tuple(round(c, 3) for c in pts[i]), "normal", tuple(round(c, 2) for c in nrm[i]),
                  "piece beneath by", round(below[3], 4), "its face points", tuple(round(c, 2) for c in below[1]))
    report = {"item": item, "body": body_name, "watched": len(watch), "at_rest": round(at_rest, 3), "clips": {},
              "hidden": len(hidden), "holes": {}}
    ok = at_rest <= LIMITS["through_share"]
    for name in CLIPS:
        action = actions.get(name)
        if action is None:
            continue
        ad.action = action
        if getattr(action, "slots", None) and len(action.slots):
            ad.action_slot = action.slots[0]
        start, end = action.frame_range
        worst = 0.0
        worst_at = None
        worst_hole = 0.0
        places = {}
        for k in range(9):
            bpy.context.scene.frame_set(int(start + (end - start) * k / 8))
            pts, nrm = evaluated(body)
            bvh = tree(piece)
            if hidden:
                open_ = holes(pts, bvh, hidden)
                worst_hole = max(worst_hole, float(np.mean(open_)))
                for i in np.array(hidden)[open_]:
                    places["hole " + bone_of[i]] = places.get("hole " + bone_of[i], 0) + 1
            if watch:
                hits = through(pts, nrm, bvh, watch)
                if float(np.mean(hits)) > worst:
                    worst = float(np.mean(hits))
                    worst_at = (k, [(i, bvh.ray_cast(pts[i] - nrm[i] * 0.001, -nrm[i], LIMITS["depth_m"])[3])
                                    for i in np.array(watch)[hits]])
                for i in np.array(watch)[hits]:
                    places[bone_of[i]] = places.get(bone_of[i], 0) + 1
        report["clips"][name] = round(worst, 3)
        report["holes"][name] = round(worst_hole, 3)
        ok &= worst_hole <= LIMITS["hole_share"]
        # With --explain, where the skin shows through at the clip's worst moment: each point
        # by its place on the body at rest, and how far beneath it the piece is.
        if explain and worst_at and worst > LIMITS["through_share"] / 2:
            k, found = worst_at
            print("WORST", item, name, f"at {k}/8 through the clip", f"{worst:.1%}")
            for i, depth in found[:: max(1, len(found) // 10)][:10]:
                print("  THROUGH", bone_of[i], "at rest", tuple(round(c, 3) for c in rest_pts[i]),
                      "piece beneath by", round(depth, 4))
        for bone, n in places.items():
            report.setdefault("where", {})[bone] = report.get("where", {}).get(bone, 0) + n
        ok &= worst <= LIMITS["through_share"]
    ad.action = None
    for pb in arm.pose.bones:
        pb.matrix_basis.identity()
    report["where"] = dict(sorted(report.get("where", {}).items(), key=lambda t: -t[1])[:6])
    report["pass"] = ok
    print("GEARCHECK", json.dumps(report))
    results.append(report)
    bpy.data.objects.remove(piece)
print("GEARCHECKS", sum(r["pass"] for r in results), "of", len(results), "pass")
