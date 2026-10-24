"""The creature check (D437): six measures a rigged, animated creature must pass, from its
exported model alone, so any rig is judged the same, ours or bought. Calibrated on Quaternius's
hand-rigged Deer.glb, which passes; a measure the deer fails is the measure's fault.

    tools/blender/run.sh tools/creatures/creature_check.py OUT.json MODEL.glb[@SCALE] ...

SCALE is the size the game draws the model at (assets/data/creatures.ron; the deer 0.36).
Prints a line a model and its failures, writes every number to OUT.json and a table to OUT.md.
`--table OUT.json PART.json ...` makes one table of several runs' models, so models can be
checked side by side, one run each (the deer among them sets the skin-lag bar).

| Measure | Bar |
|---|---|
| Trunk stretch: p95 of the belly's and torso's edges, stretched or shrunk | 25% in gaits, 45% in other clips |
| Belly weight on leg bones (four-legged) | 25% |
| Skin lag: a joint whose bone and parent share skin (a vertex 20-80% each), turned about the bone (its turn's rotation vector along the bone; bending reads 0; linear blending pinches evenly shared skin to cos(t/2)) | 30 deg, or 1.25 x the deer's worst |
| Lowest skin, standing clips | -1 cm |
| Lowest skin, gaits and one-shot moves (a hoof digs in as it lands) | -max(1 cm, 4% of height) |
| Lowest trunk skin, lying clips (Death: head, ears and legs may dip, the body may not) | -max(1 cm, 4% of height) |
| Loop gap: the wrap's jump (loops) | min(0.5 cm, 1% of body size) |
| Foot slide, generated models only (`terra_rig` on the armature, set by animal.py): the skin touching the ground slipping from the clip's pace, mid-stance / the whole stance with touch-down and lift-off | 1 cm / 3 cm |

A bought model's foot slide is reported, not failed: the deer, the reference for deformation
only, skates (its Walk carries the hind hooves at 1.2 m/s and the fore at 0.8; its idles slide
a hoof back 9-15 cm after a step). Beside them it reports, without failing a model: kinks
(a bone tail's velocity changing by over half its top speed in one frame); and the velocity
jump at a loop's seam.
Each clip starts from the rest pose: a bone it leaves unkeyed stands at rest, as in glTF.
The skin is read at 60 frames a second, the game's sampling: the scene is set to 60 before
import, else the importer lays the keys on Blender's 24.
"""

import json
import math
import os
import sys

import bpy
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import measures as ms  # noqa: E402

GAIT = ("walk", "trot", "run", "gallop", "hop", "canter", "bound")
ONE_SHOT = ("death", "attack", "jump", "hit", "sit", "strike")
LYING = ("death", "die", "lie", "sleep")
AFLOAT = ("fly", "glide", "hover", "swim", "float", "dabble")
TRUNK_SKIP = ("Neck", "Head", "Tail", "Ear", "Jaw", "IK", "Pole", "Root")
BAR = {"trunk_gait": 0.25, "trunk": 0.45, "limb_share": 0.25, "lag_deg": 30.0, "low": -0.01, "low_moving": 0.04,
       "gap": 0.005, "gap_body": 0.01, "slide_mid": 0.01, "slide": 0.03}


def clip_name(a):
    return a.split("|")[-1]


def is_(name, words):
    n = clip_name(name).lower()
    return any(w in n for w in words)


def key_frames(act):
    """The frames an action has keys on, every channel's together, sorted."""
    curves = [fc for layer in act.layers for strip in layer.strips for bag in strip.channelbags for fc in bag.fcurves]
    return sorted({round(k.co.x, 4) for fc in curves for k in fc.keyframe_points})


def sided(n):
    return n.endswith((".L", ".R", "_L", "_R"))


def check(src, scale):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    scene.render.fps, scene.render.fps_base = 60, 1.0
    bpy.ops.import_scene.gltf(filepath=src)
    rig = next(o for o in scene.objects if o.type == "ARMATURE")
    mesh = max((o for o in scene.objects if o.type == "MESH"), key=lambda o: len(o.data.vertices))
    bones = rig.data.bones
    S = np.diag([scale, scale, scale])
    RW = rig.matrix_world

    def at(v):
        return np.array(RW @ v) * scale

    n = len(mesh.data.vertices)
    rest = np.empty(n * 3)
    mesh.data.vertices.foreach_get("co", rest)
    mw = np.array(mesh.matrix_world)
    rest = (rest.reshape(n, 3) @ mw[:3, :3].T + mw[:3, 3]) @ S
    lo, hi = rest.min(axis=0), rest.max(axis=0)
    height, length = hi[2] - lo[2], hi[1] - lo[1]
    body = float((hi - lo).max())
    names = {g.index: g.name for g in mesh.vertex_groups}
    W = {}
    for v in mesh.data.vertices:
        for g in v.groups:
            if g.weight > 0:
                W.setdefault(names[g.group], {})[v.index] = g.weight
    deform = set(W) & set(b.name for b in bones)
    total = np.zeros(n)
    for ws in W.values():
        for i, w in ws.items():
            total[i] += w
    bird = any(b.name.startswith("Humerus") for b in bones)
    trunk = [b.name for b in bones if b.name in deform and not sided(b.name) and not b.name.startswith(TRUNK_SKIP)]
    spine_z = float(np.mean([at(bones[b].head_local)[2] for b in trunk])) if trunk else hi[2] * 0.6
    legs = [b.name for b in bones if b.name in deform and sided(b.name) and at(bones[b.name].tail_local)[2] < spine_z - 0.05 * height
            and not b.name.startswith(("Humerus", "Forearm", "Hand", "Ear", "Wing"))]
    roots = [b for b in legs if not (bones[b].parent and bones[b].parent.name in legs)]

    # The trunk's skin: the belly box under the trunk between the legs (four-legged), and every
    # vertex whose biggest weight is on a trunk bone.
    dom = np.full(n, "", dtype=object)
    best = np.zeros(n)
    for b, ws in W.items():
        for i, w in ws.items():
            if w > best[i]:
                best[i], dom[i] = w, b
    torso = np.isin(dom, trunk)
    belly = np.zeros(n, bool)
    limb_share = None
    if not bird and len(roots) >= 4:
        ys = sorted(at(bones[b].head_local)[1] for b in roots)
        mid = (ys[0] + ys[-1]) / 2
        fy = np.mean([y for y in ys if y < mid])
        hy = np.mean([y for y in ys if y >= mid])
        a, b2 = min(fy, hy), max(fy, hy)
        legx = min(abs(at(bones[b].head_local)[0]) for b in roots)
        sel = (rest[:, 1] > a + 0.15 * (b2 - a)) & (rest[:, 1] < b2 - 0.15 * (b2 - a)) & (np.abs(rest[:, 0]) < 0.5 * legx)
        if sel.any():
            low = rest[sel, 2].min()
            belly = sel & (rest[:, 2] < low + 0.35 * (spine_z - low))
            # The torso of a four-legged body is its barrel between the legs' roots, all round:
            # the neck's base and the rump bend by design and are judged by their rings.
            torso = (rest[:, 1] > a + 0.15 * (b2 - a)) & (rest[:, 1] < b2 - 0.15 * (b2 - a)) & torso
        tot = {}
        for bname, ws in W.items():
            tot[bname] = sum(w for i, w in ws.items() if belly[i])
        s = sum(tot.values()) or 1.0
        limb_share = sum(v for k, v in tot.items() if k in legs) / s
    skin = belly | torso
    edges = np.array([e.vertices[:] for e in mesh.data.edges])
    el0 = np.linalg.norm(rest[edges[:, 0]] - rest[edges[:, 1]], axis=1)
    te = (el0 > 1e-7) & skin[edges[:, 0]] & skin[edges[:, 1]]

    # Joints whose bone and parent share skin round the bone's head: their turn's twist pinches it.
    rings = {}
    for b in bones:
        if not b.parent or b.name not in deform or b.length < 1e-5:
            continue
        hd = at(b.head_local)
        ax = np.array((RW.to_3x3() @ (b.tail_local - b.head_local)).normalized())
        rel = rest - hd
        along = rel @ ax
        r = np.linalg.norm(rel - np.outer(along, ax), axis=1)
        moved = list(set(W.get(b.name, {})) | set(W.get(b.parent.name, {})))
        m = np.zeros(n, bool)
        m[moved] = True
        cand = m & (np.abs(along) < 0.12 * b.length * scale * RW.to_scale()[0])
        if cand.sum() < 6:
            continue
        cand &= r < np.median(r[cand]) * 2.0
        idx = np.nonzero(cand)[0]
        own = np.array([W.get(b.name, {}).get(i, 0.0) for i in idx])
        par = np.array([W.get(b.parent.name, {}).get(i, 0.0) for i in idx])
        # The skin the two share, mostly theirs: a vertex led by a third bone twists with it.
        keep = (own + par > 0.5 * total[idx]) & (own > 0) & (par > 0)
        share = own[keep] / (own[keep] + par[keep])
        if ((share > 0.2) & (share < 0.8)).any():
            rings[b.name] = share

    feet = ms.foot_clusters(rest, height, length, 2 if bird else 4)
    eps = max(0.003, 0.01 * height)
    dg = bpy.context.evaluated_depsgraph_get()
    pbs = rig.pose.bones

    def skin_at(frame):
        scene.frame_set(int(frame), subframe=frame - int(frame))
        ev = mesh.evaluated_get(dg)
        me = ev.to_mesh()
        q = np.empty(len(me.vertices) * 3)
        me.vertices.foreach_get("co", q)
        emw = np.array(ev.matrix_world)
        ev.to_mesh_clear()
        return (q.reshape(-1, 3) @ emw[:3, :3].T + emw[:3, 3]) @ S

    rig.animation_data_create()
    out = {"model": src, "scale": scale, "body_m": round(body, 3), "bird": bird, "trunk_bones": trunk, "leg_bones": legs,
           "generated": bool(rig.get("terra_rig")),
           "belly_verts": int(belly.sum()), "torso_verts": int(torso.sum()), "feet": len(feet),
           "limb_share": None if limb_share is None else round(limb_share, 3),
           "rest_lowest_cm": round(float(lo[2]) * 100, 2), "height_m": round(float(height), 3), "clips": {}}
    for act in bpy.data.actions:
        # A bone a clip leaves unkeyed stands at rest, as glTF has it, not where the last clip
        # left it: Blender keeps a pose channel no action writes, which put the deer's hind
        # hooves 19-37 cm up through its idles after Gallop_Jump.
        for pb in pbs:
            pb.location, pb.rotation_quaternion, pb.scale = (0, 0, 0), (1, 0, 0, 0), (1, 1, 1)
            pb.rotation_euler = (0, 0, 0)
        rig.animation_data.action = act
        if act.slots:
            rig.animation_data.action_slot = act.slots[0]
        f0, f1 = (int(round(x)) for x in act.frame_range)
        name = act.name
        loop = not is_(name, ONE_SHOT)
        trunk_w, lag, lowest, lowest_trunk, hardest = 0.0, 0.0, 1e9, 1e9, 0.0
        first = last = prev = None
        soles = [[] for _ in feet]
        tails = []
        for k, f in enumerate(range(f0, f1 + 1)):
            p = skin_at(f)
            first = p if first is None else first
            if prev is not None:
                hardest = max(hardest, ms.turn(prev, last, p))
            prev, last = last, p
            lowest = min(lowest, float(p[:, 2].min()))
            if skin.any():
                lowest_trunk = min(lowest_trunk, float(p[skin, 2].min()))
            if te.any():
                el = np.linalg.norm(p[edges[te, 0]] - p[edges[te, 1]], axis=1)
                trunk_w = max(trunk_w, ms.stretch(el0[te], el))
            for i, c in enumerate(feet):
                soles[i].append(p[c])
            tails.append([at(pb.tail) for pb in pbs if pb.name in deform])
            if k % 2:
                continue
            for bn in rings:
                b = bones[bn]
                # Turns only: a bone grown or shrunk (a bird's folding wing) scales its matrix,
                # which read as an 80-degree twist.
                own = np.array((pbs[bn].matrix @ b.matrix_local.inverted()).to_3x3().normalized())
                par = np.array((pbs[b.parent.name].matrix @ b.parent.matrix_local.inverted()).to_3x3().normalized())
                lag = max(lag, float(ms.twist_about(own.T @ par, np.array(b.tail_local - b.head_local))))
        tails = np.array(tails)
        c = {"frames": f1 - f0, "loop": loop, "gait": is_(name, GAIT) and not is_(name, ONE_SHOT), "lying": is_(name, LYING),
             "trunk_p95": round(trunk_w, 3), "skin_lag_deg": round(lag, 1), "lowest_cm": round(lowest * 100, 2),
             "lowest_trunk_cm": round(lowest_trunk * 100, 2) if skin.any() else None}
        if loop:
            keys = key_frames(act)
            step = np.diff(keys)
            if len(keys) >= 4 and step.min() > 1.01 and np.ptp(step) < 1e-3:
                # Keys further apart than a frame (the deer's are 1/24 s): a loop a key short of
                # closing wraps by a key's step, so it is judged on the skin at its keys.
                at_keys = [skin_at(k) for k in keys]
                hardest = max(ms.turn(*at_keys[i - 1:i + 2]) for i in range(1, len(at_keys) - 1))
                first, last, prev = at_keys[0], at_keys[-1], at_keys[-2]
            c["gap_cm"] = round(ms.loop_gap(first, last, prev, hardest) * 100, 3)
            if len(tails) > 3:
                seam = np.linalg.norm((tails[1] - tails[0]) - (tails[-1] - tails[-2]), axis=1).max()
                c["seam_dv_cm"] = round(float(seam) * 100, 2)
        if len(tails) > 3:
            kn, worst = ms.kinks(tails[:-1] if loop else tails)
            c["kinks"], c["kink_worst"] = kn, round(worst, 2)
        if feet and not is_(name, AFLOAT) and not is_(name, ONE_SHOT):
            mid, whole, pace, steps = ms.foot_slide([np.array(t) for t in soles], eps, loop)
            c["slide_mid_cm"], c["slide_cm"] = round(mid * 100, 2), round(whole * 100, 2)
            c["pace_mps"], c["steps"] = round(pace, 3), steps
        out["clips"][clip_name(name)] = c
        print("CLIP", os.path.basename(src), clip_name(name), json.dumps(c), flush=True)
    return out


def low_bar(m, c):
    """The lowest a clip's skin may go, in cm, and which lowest it is judged on."""
    moving = -max(-BAR["low"], BAR["low_moving"] * m.get("height_m", 0)) * 100
    if c.get("lying"):
        return moving, "lowest_trunk_cm"
    if c["gait"] or not c["loop"]:
        return moving, "lowest_cm"
    return BAR["low"] * 100, "lowest_cm"


def fails(m, lag_bar):
    out = []
    if m["limb_share"] is not None and m["limb_share"] > BAR["limb_share"]:
        out.append(f"belly {m['limb_share'] * 100:.0f}% on legs")
    gap_bar = min(BAR["gap"], BAR["gap_body"] * m["body_m"]) * 100
    for name, c in m["clips"].items():
        bar = BAR["trunk_gait"] if c["gait"] else BAR["trunk"]
        if c["trunk_p95"] > bar:
            out.append(f"{name} stretch {c['trunk_p95'] * 100:.0f}%")
        if c["skin_lag_deg"] > lag_bar:
            out.append(f"{name} lag {c['skin_lag_deg']:.0f} deg")
        low, key = low_bar(m, c)
        if c[key] is not None and c[key] < low:
            out.append(f"{name} {'trunk ' if key == 'lowest_trunk_cm' else ''}low {c[key]:.1f} cm")
        if c.get("gap_cm", 0) > gap_bar:
            out.append(f"{name} gap {c['gap_cm']:.2f} cm")
        slid = c.get("slide_mid_cm", 0) > BAR["slide_mid"] * 100 or c.get("slide_cm", 0) > BAR["slide"] * 100
        if m.get("generated") and slid:
            out.append(f"{name} slide {c['slide_mid_cm']:.1f}/{c['slide_cm']:.1f} cm")
    return out


def row(m, f):
    cl = m["clips"].values()
    w = lambda k: max((c.get(k, 0) for c in cl), default=0)
    gait = max((c["trunk_p95"] for c in cl if c["gait"]), default=0)
    other = max((c["trunk_p95"] for c in cl if not c["gait"]), default=0)
    share = "n/a" if m["limb_share"] is None else f"{m['limb_share'] * 100:.0f}%"
    return (f"| {os.path.basename(m['model'])} | {share} | {gait * 100:.0f}% / {other * 100:.0f}% | {w('skin_lag_deg'):.0f} |"
            f" {w('slide_mid_cm'):.1f} / {w('slide_cm'):.1f}{'' if m.get('generated') else ' (info)'} | {min(c['lowest_cm'] for c in cl):.1f} | {w('gap_cm'):.2f} | {sum(c.get('kinks', 0) for c in cl)} |"
            f" {'PASS' if not f else 'FAIL: ' + '; '.join(f[:4]) + (' ...' if len(f) > 4 else '')} |")


def main():
    argv = sys.argv[sys.argv.index("--") + 1:]
    if argv[0] == "--table":
        out_path, res = argv[1], []
        for part in argv[2:]:
            res += json.load(open(part))["models"]
    else:
        out_path, res = argv[0], []
        for a in argv[1:]:
            path, _, s = a.partition("@")
            res.append(check(os.path.abspath(path), float(s) if s else 1.0))
    deer = [m for m in res if os.path.basename(m["model"]) == "Deer.glb"]
    deer_lag = max((c["skin_lag_deg"] for m in deer for c in m["clips"].values()), default=0.0)
    lag_bar = max(BAR["lag_deg"], 1.25 * deer_lag)
    lines = ["| Model | Belly on legs | Trunk p95 gait / other | Skin lag deg | Slide cm mid / whole | Lowest cm | Loop gap cm | Kinks | Verdict |",
             "|---|---|---|---|---|---|---|---|---|"]
    passed = 0
    for m in res:
        f = fails(m, lag_bar)
        m["fails"] = f
        passed += not f
        lines.append(row(m, f))
        print("MODEL", os.path.basename(m["model"]), "PASS" if not f else "FAIL " + "; ".join(f), flush=True)
    summary = f"Passed {passed} of {len(res)} (skin-lag bar {lag_bar:.1f} deg)"
    lines.append("")
    lines.append(summary)
    json.dump({"bars": BAR, "lag_bar": lag_bar, "models": res}, open(out_path, "w"), indent=1)
    open(os.path.splitext(out_path)[0] + ".md", "w").write("\n".join(lines) + "\n")
    print(summary)


if __name__ == "__main__":
    main()
