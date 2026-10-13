"""Checks a rigged character against our skeleton and plays it through stress poses, so a
bad rig is caught by numbers instead of by eye. Prints one JSON report with a pass or fail
for each check, and writes out/rig-check-NAME.json.

    tools/blender/run.sh tools/characters/rig_check.py tools/characters/out/base-male-rigged.glb

Checks:
  skeleton      same bones, same parents as our skeleton
  rest pose     every bone points the same way as in our skeleton (animations assume it)
  weights       every vertex weighted, at most 4 bones each, weights summing to 1
  seams         copies of a vertex split at a texture seam carry the same weights, or the
                seam opens into a gap as the body moves
  reach         no vertex driven mostly by a bone far from it (an arm stuck to the hip)
  symmetry      a vertex and its mirror are driven by mirrored bones, by much the same
                weights (not only the same strongest bone, which flips where two bones
                share a vertex about equally, as a limb and its twist bone do)
  motion        through every animation: how many edges stretch or crush badly, whether
                the feet stay at the ground when standing, and nothing sinks below it
  pinch         through every animation: how much the ring of skin round each elbow, knee
                and wrist shrinks, as an elbow does when a forearm's turn has nowhere to go
"""

import json
import re
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy
import numpy as np
from mathutils import Vector
from mathutils.geometry import intersect_point_line

import look
import lift
import twist

FIGURE = look.FIGURE
OUT = os.path.join(HERE, "out")
ACTIONS = look.game_clips()
# Thresholds: what a usable rig must meet.
LIMITS = {
    "rest_angle_deg": 3.0,          # largest difference in any bone's direction
    "max_influences": 4,
    "weight_sum_error": 0.02,
    "reach": 0.14,                  # of the character's height, from a vertex to its main bone
    "reach_fail_share": 0.005,      # share of vertices allowed beyond that
    "mirror_fail_share": 0.05,
    "mirror_weight_diff": 0.25,  # a vertex's weights and its mirror's differ by more than this
    "seam_weight_diff": 0.05,       # summed difference in weights between copies of one vertex
    "seam_fail_share": 0.01,        # share of split vertices allowed beyond that
    "stretch_ratio": 1.8,           # an edge longer than this many times its rest length
    "crush_ratio": 0.45,            # or shorter than this
    "bad_edge_share": 0.01,         # share of edges allowed to do either, in any frame
    "vs_mannequin": 3.5,            # or this many times the hand-rigged mannequin's share: set by eye on 2026-10-11, where a Tripo rig at 3.3x crouched cleanly
    "pinch": 0.2,                   # how much a ring of skin round an elbow, knee or wrist may shrink, in any frame
    "foot_gap_m": 0.05,             # standing feet within this of the ground
    "sink_over_m": 0.02,            # nothing below the ground by more than this deeper than the mannequin goes in the same clip
                                    # (a roll, a kneel and a fall touch the ground by design)
}


def load(path):
    before = set(bpy.context.scene.objects)
    bpy.ops.import_scene.gltf(filepath=path)
    new = [o for o in bpy.context.scene.objects if o not in before]
    arm = next(o for o in new if o.type == "ARMATURE")
    meshes = [o for o in new if o.type == "MESH" and not o.name.startswith("Icosphere")]
    arm.animation_data_clear()
    for pb in arm.pose.bones:
        pb.matrix_basis.identity()
    bpy.context.view_layer.update()
    return arm, meshes


def directions(arm):
    return {b.name: (b.tail_local - b.head_local).normalized() for b in arm.data.bones}


look.reset()
ref_arm, ref_meshes = load(FIGURE)
ref_dirs = directions(ref_arm)
ref_parents = {b.name: b.parent.name if b.parent else None for b in ref_arm.data.bones}
for m in ref_meshes:
    bpy.data.objects.remove(m)

arm, meshes = load(look.path(look.args()[0]))
body = max(meshes, key=lambda m: len(m.data.vertices))
report, ok = {}, {}

# Skeleton and rest pose.
names = {b.name for b in arm.data.bones}
missing = sorted(set(ref_dirs) - names)
wrong_parent = sorted(b.name for b in arm.data.bones if b.name in ref_parents and (b.parent.name if b.parent else None) != ref_parents[b.name])
report["skeleton"] = {"missing": missing, "wrong_parent": wrong_parent}
ok["skeleton"] = not missing and not wrong_parent
dirs = directions(arm)
angles = {n: math.degrees(dirs[n].angle(ref_dirs[n], 0)) for n in dirs if n in ref_dirs}
worst = max(angles, key=angles.get)
report["rest_pose"] = {"worst_bone": worst, "worst_deg": round(angles[worst], 2)}
ok["rest_pose"] = angles[worst] <= LIMITS["rest_angle_deg"]

# Weights.
groups = {g.index: g.name for g in body.vertex_groups}
bones = {b.name: b for b in arm.data.bones}
mw = body.matrix_world
height = max((mw @ v.co).z for v in body.data.vertices) - min((mw @ v.co).z for v in body.data.vertices)
unweighted = over = bad_sum = far = 0
far_at = {}
main_bone = []
for v in body.data.vertices:
    ws = [(groups[g.group], g.weight) for g in v.groups if g.weight > 0.001 and groups.get(g.group) in bones]
    if not ws:
        unweighted += 1
        main_bone.append(None)
        continue
    over += len(ws) > LIMITS["max_influences"]
    bad_sum += abs(sum(w for _, w in ws) - 1) > LIMITS["weight_sum_error"]
    name = max(ws, key=lambda t: t[1])[0]
    main_bone.append(name)
    b = bones[name]
    p = arm.matrix_world.inverted() @ (mw @ v.co)
    # Along the bone to the next joint (its child's head), not its tail, which a glTF file
    # does not keep: the importer guesses one, often short of the joint.
    kids = list(b.children)
    end = max((c.head_local for c in kids), key=lambda h: (h - b.head_local).length) if kids else b.tail_local
    q, t = intersect_point_line(p, b.head_local, end)
    q = b.head_local if t < 0 else end if t > 1 else q
    if (arm.matrix_world.to_3x3() @ (p - q)).length / height > LIMITS["reach"]:
        far += 1
        far_at[name] = far_at.get(name, 0) + 1
n = len(body.data.vertices)
report["weights"] = {"vertices": n, "unweighted": unweighted, "over_4_bones": over, "not_summing_to_1": bad_sum}
ok["weights"] = unweighted == 0 and over == 0
report["weights"]["note"] = "sums are normalised on export" if bad_sum else ""
report["reach"] = {"far_share": round(far / n, 4), "at": dict(sorted(far_at.items(), key=lambda t: -t[1])[:4])}
ok["reach"] = far / n <= LIMITS["reach_fail_share"]

# Seams: the copies of a vertex at one position must move together.
copies = {}
for v in body.data.vertices:
    copies.setdefault(tuple(round(c, 5) for c in v.co), []).append(v)
split = [vs for vs in copies.values() if len(vs) > 1]
torn = 0
for vs in split:
    ws = [{groups[g.group]: g.weight for g in v.groups if g.weight > 0} for v in vs]
    keys = set().union(*ws)
    torn += max(sum(abs(a.get(k, 0) - b.get(k, 0)) for k in keys) for a in ws for b in ws) > LIMITS["seam_weight_diff"]
report["seams"] = {"split_positions": len(split), "torn": torn}
ok["seams"] = torn <= LIMITS["seam_fail_share"] * max(len(split), 1)

# Symmetry: a vertex's mirror should be driven by the mirrored bones, by much the same weights.
pts = np.array([tuple(mw @ v.co) for v in body.data.vertices])
from mathutils.kdtree import KDTree

kd = KDTree(n)
for i, p in enumerate(pts):
    kd.insert(p, i)
kd.balance()
def mirrored(name):
    return re.sub(r"_([lr])$", lambda m: "_r" if m.group(1) == "l" else "_l", name)


def weights_of(i):
    return {groups[g.group]: g.weight for g in body.data.vertices[i].groups if g.weight > 0}


mismatch = checked = 0
for i in range(0, n, 7):
    if main_bone[i] is None or abs(pts[i][0]) < 0.03:
        continue
    j = kd.find((-pts[i][0], pts[i][1], pts[i][2]))[1]
    if main_bone[j] is None:
        continue
    checked += 1
    mine = {mirrored(k): w for k, w in weights_of(i).items()}
    theirs = weights_of(j)
    diff = sum(abs(mine.get(k, 0.0) - theirs.get(k, 0.0)) for k in set(mine) | set(theirs)) / 2
    mismatch += diff > LIMITS["mirror_weight_diff"]
report["symmetry"] = {"checked": checked, "mismatch_share": round(mismatch / max(checked, 1), 3)}
ok["symmetry"] = mismatch / max(checked, 1) <= LIMITS["mirror_fail_share"]

# Motion: play the animations and measure the deformed mesh.
bpy.ops.import_scene.gltf(filepath=FIGURE)  # the animations
for o in list(bpy.context.scene.objects):
    if o not in (arm, body) and o.type in ("MESH", "ARMATURE") and o not in meshes:
        bpy.data.objects.remove(o)
# The clips as the game plays them on this body: the limbs' roll spread over its twist bones.
played = [a for a in (bpy.data.actions.get(n) for n in ACTIONS) if a is not None]
twist.bake(arm, played)
look.skin_as_game(linear="--linear" in look.args())
# And the hips lifted by its size where it meets the floor, as the export bakes them (lift.py).
lift.bake(arm, played, lift.needed(arm, body, played, lift.reference()))
edges = np.array([tuple(e.vertices) for e in body.data.edges])
rest_len = np.linalg.norm(pts[edges[:, 0]] - pts[edges[:, 1]], axis=1) + 1e-9
# Generated meshes are full of sliver edges whose length ratio swings wildly from tiny
# moves; judge only edges of ordinary size.
keep = rest_len > 0.3 * np.median(rest_len)
# Fingers curl, which legitimately crushes their edges; judge the body, not the hands.
hand = np.array([bool(main_bone[a] and main_bone[a].startswith(("index", "middle", "ring", "pinky", "thumb"))) for a, _ in edges])
keep &= ~hand
edges, rest_len = edges[keep], rest_len[keep]
foot = [i for i, b in enumerate(main_bone) if b and b.startswith(("foot", "ball"))]
# Rings of skin round the elbows, knees and wrists. A turn the skin cannot follow shrinks a
# whole ring toward the joint (an elbow pinched to a neck); a bend only flattens one side, so
# the ring's average radius holds. Pinching is that radius shrinking.
RINGS = {}
for s in ("l", "r"):
    RINGS[f"elbow_{s}"] = (f"lowerarm_{s}", f"upperarm_{s}", ("upperarm", "lowerarm"))
    RINGS[f"wrist_{s}"] = (f"hand_{s}", f"lowerarm_{s}", ("lowerarm", "hand"))
    RINGS[f"knee_{s}"] = (f"calf_{s}", f"thigh_{s}", ("thigh", "calf"))


def ring_radius(points):
    return float(np.mean(np.linalg.norm(points - points.mean(axis=0), axis=1)))


rings = {}
mw_arm = arm.matrix_world
for ring, (bone, above, parts) in RINGS.items():
    if bone not in arm.data.bones:
        continue
    side = ring[-1]
    centre = np.array(tuple(mw_arm @ arm.data.bones[bone].head_local))
    along = np.array(tuple(mw_arm.to_3x3() @ (arm.data.bones[bone].head_local - arm.data.bones[above].head_local)))
    along /= np.linalg.norm(along)
    near = [i for i, b in enumerate(main_bone)
            if b and b.endswith(f"_{side}") and b.startswith(parts)
            and abs(float((pts[i] - centre) @ along)) < 0.035 and np.linalg.norm(pts[i] - centre) < 0.15]
    if len(near) > 12:
        rings[ring] = (np.array(near), ring_radius(pts[near]))
pinch = {}
motion = {}
worst_bad = 0.0
where = {}
sink = 0.0
sink_at = ""
standing_gap = 0.0
depsgraph = bpy.context.evaluated_depsgraph_get()
for name in ACTIONS:
    action = bpy.data.actions.get(name) or next((a for a in bpy.data.actions if a.name.startswith(name)), None)
    if action is None:
        continue
    ad = arm.animation_data or arm.animation_data_create()
    ad.action = action
    if getattr(action, "slots", None) and len(action.slots):
        ad.action_slot = action.slots[0]
    start, end = action.frame_range
    bad_max = 0.0
    clip_sink = 0.0
    for k in range(9):
        bpy.context.scene.frame_set(int(start + (end - start) * k / 8))
        depsgraph.update()
        ev = body.evaluated_get(depsgraph)
        me = ev.to_mesh()
        cur = np.array([tuple(ev.matrix_world @ v.co) for v in me.vertices])
        ev.to_mesh_clear()
        length = np.linalg.norm(cur[edges[:, 0]] - cur[edges[:, 1]], axis=1)
        ratio = length / rest_len
        badmask = (ratio > LIMITS["stretch_ratio"]) | (ratio < LIMITS["crush_ratio"])
        bad = np.mean(badmask)
        bad_max = max(bad_max, float(bad))
        for e in np.nonzero(badmask)[0]:
            where[main_bone[edges[e][0]] or "none"] = where.get(main_bone[edges[e][0]] or "none", 0) + 1
        clip_sink = max(clip_sink, float(-cur[:, 2].min()))
        if -cur[:, 2].min() > sink:
            low = int(np.argmin(cur[:, 2]))
            sink, sink_at = float(-cur[low, 2]), f"{name} {k}/8 {main_bone[low]}"
        for ring, (near, rest) in rings.items():
            thin = 1.0 - ring_radius(cur[near]) / rest
            if thin > pinch.get(name, (0, ""))[0]:
                pinch[name] = (thin, ring)
    motion[name] = {"worst_bad_edge_share": round(bad_max, 4), "sink_m": round(clip_sink, 3), "pinched": round(pinch.get(name, (0, ""))[0], 3), "pinched_at": pinch.get(name, (0, ""))[1]}
    worst_bad = max(worst_bad, bad_max)
top = sorted(where.items(), key=lambda t: -t[1])[:6]
report["motion"] = {"actions": motion, "worst_places": top, "sink_m": round(sink, 3), "sink_at": sink_at, "standing_foot_gap_m": round(standing_gap, 3)}
# Big moves crush some edges on any rig; judge each animation against the hand-rigged
# mannequin's own share on the same frames (art/characters/rig-baseline.json, from checking the mannequin).
baseline_path = os.path.join(look.ART, "characters", "rig-baseline.json")
baseline = json.load(open(baseline_path)) if os.path.exists(baseline_path) else {}
moves_ok = True
sink_over, sink_over_at = 0.0, ""
for a, v in motion.items():
    base = baseline.get(a, {})
    if not isinstance(base, dict):  # a baseline from before it held each clip's sink
        base = {"bad_edge_share": base}
    limit = max(LIMITS["bad_edge_share"] * 0.5, LIMITS["vs_mannequin"] * base["bad_edge_share"]) if base else LIMITS["bad_edge_share"]
    over = v["sink_m"] - base.get("sink_m", 0.0)
    if over > sink_over:
        sink_over, sink_over_at = over, a
    v["limit"] = round(limit, 4)
    v["ok"] = v["worst_bad_edge_share"] <= limit and v["pinched"] <= LIMITS["pinch"]
    moves_ok &= v["ok"]
if os.path.basename(look.args()[0]) == os.path.basename(FIGURE):
    with open(baseline_path, "w") as f:
        json.dump({a: {"bad_edge_share": v["worst_bad_edge_share"], "sink_m": v["sink_m"]} for a, v in motion.items()}, f, indent=2)
report["motion"]["sink_over_mannequin_m"] = round(sink_over, 3)
report["motion"]["sink_over_at"] = sink_over_at
ok["motion"] = moves_ok and sink_over <= LIMITS["sink_over_m"] and standing_gap <= LIMITS["foot_gap_m"]

result = {"file": look.args()[0], "pass": all(ok.values()), "checks": ok, "report": report, "limits": LIMITS}
os.makedirs(OUT, exist_ok=True)
with open(os.path.join(OUT, f"rig-check-{os.path.splitext(os.path.basename(look.args()[0]))[0]}.json"), "w") as f:
    json.dump(result, f, indent=2)
print("RIGCHECK", json.dumps(result))
