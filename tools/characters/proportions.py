"""The proportion check (D593): a body's reach, fingertips and crotch against a real man's, as
shares of its own height, from ANSUR II's 1st to 99th percentile (art/characters/proportions.json;
the measures as research/2026-11-01-why-the-torso-looks-wrong.md takes them). Generated bodies
come with short arms and the crotch low; the stretch (stretch.py, in convert_rig.py) fixes them,
and this holds every body to it.

    tools/blender/run.sh tools/characters/proportions.py BODY.glb [BODY.glb ...]

Measured in the T-pose at rest: the reach fingertip to fingertip; the fingertips and wrist with
the arm hung straight down from the shoulder joint (its joint-to-wrist length, then the hand's,
off the shoulder joint's height), so no clip's pose is in it; the crotch, the lowest skin on the
middle line. A PROPORTIONS line a body, JSON; and for a body outside a checked band a line
`PROPORTIONS fail FILE: ...` saying which, by how much and what to do. Exits 0 either way: the
build's verdict reads the fail lines (build_verdict.ts).
"""

import json
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import stretch  # noqa: E402

BANDS = os.path.join(HERE, "..", "..", "art", "characters", "proportions.json")


def bands():
    with open(BANDS) as f:
        return json.load(f)


def measure(points, edges, joints):
    """Metres: height, reach, fingertip, wrist, crotch, from the T-pose skin (its points and
    edges) and joints."""
    P = np.asarray(points, dtype=float)
    J = {k: np.asarray(v, dtype=float) for k, v in joints.items()}
    floor = float(P[:, 2].min())
    cx = float(J["upperarm_l"][0] + J["upperarm_r"][0]) / 2
    tips, wrists = [], []
    for s in "lr":
        shoulder, wrist = J[f"upperarm_{s}"], J[f"hand_{s}"]
        side = 1.0 if shoulder[0] > cx else -1.0
        tip = float((side * (P[:, 0] - cx)).max())
        arm = float(np.linalg.norm(wrist - shoulder))
        hand = tip - side * (wrist[0] - cx)
        wrists.append(shoulder[2] - arm - floor)
        tips.append(shoulder[2] - arm - hand - floor)
    return {
        "height": float(P[:, 2].max()) - floor,
        "reach": float(P[:, 0].max() - P[:, 0].min()),
        "fingertip": float(np.mean(tips)),
        "wrist": float(np.mean(wrists)),
        "crotch": stretch.measure_crotch(P, cx, edges),
    }


def judge(m, data=None):
    """Each measure with its band at this height, and the faults of the checked ones."""
    data = data or bands()
    H = m["height"]
    out, faults = {}, []
    for name, b in data["measures"].items():
        lo, mean, hi = b["p1"] * H, b["mean"] * H, b["p99"] * H
        out[name] = {"m": round(m[name], 3), "real": round(mean, 3), "band": [round(lo, 3), round(hi, 3)]}
        if name in data["checked"] and not lo <= m[name] <= hi:
            edge, word = (lo, "below the 1st") if m[name] < lo else (hi, "above the 99th")
            faults.append(f"{name} {m[name]:.2f} m, {word} percentile's {edge:.2f} m (a real man's {mean:.2f} m)")
    return out, faults


ADVICE = ("Every body of a person is stretched to a person's proportions as it is converted (stretch.py in convert_rig.py, D593); "
          "a body this far off was not, or was made another way: rebuild it with tools/characters/build_bodies.sh. "
          "Don't widen a band in art/characters/proportions.json to pass it.")
# Quaternius's bodies stay as made (D599): their man's fingertips sit 2.2 cm above the 99th
# percentile and the developers keep his arms so, so a body of theirs out of band is reported
# to them with the evidence, not failed or stretched.
THEIRS = os.path.join(HERE, "..", "..", "art", "characters", "quaternius")
AS_MADE = ("Their parts stay as made (D599): reported to the developers with these numbers, never stretched; "
           "no band in art/characters/proportions.json is widened for it.")


def theirs(*names):
    """Whether any of `names` (an object's or mesh's, Blender's ".001" ignored) is the name of a
    node or mesh in one of Quaternius's files under art/characters/quaternius/."""
    known = set()
    for folder, _, files in os.walk(THEIRS):
        for f in files:
            if f.endswith(".gltf"):
                with open(os.path.join(folder, f)) as fh:
                    g = json.load(fh)
                known |= {n.get("name") for n in g.get("nodes", []) if "mesh" in n} | {m.get("name") for m in g.get("meshes", [])}
    return any(n.split(".0")[0] in known or n in known for n in names)


def main():
    import bpy

    import look

    for given in look.args():
        look.reset()
        bpy.ops.import_scene.gltf(filepath=look.path(given))
        arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
        if arm.animation_data:
            arm.animation_data.action = None
        for pb in arm.pose.bones:
            pb.matrix_basis.identity()
        bpy.context.view_layer.update()
        body = max((o for o in bpy.context.scene.objects if o.type == "MESH"), key=lambda o: len(o.data.vertices))
        body.shape_key_clear()
        n = len(body.data.vertices)
        co = np.zeros(n * 3)
        body.data.vertices.foreach_get("co", co)
        m4 = np.array(body.matrix_world)
        P = co.reshape(-1, 3) @ m4[:3, :3].T + m4[:3, 3]
        E = np.zeros(len(body.data.edges) * 2, dtype=np.int32)
        body.data.edges.foreach_get("vertices", E)
        J = {b.name: tuple(arm.matrix_world @ b.head_local) for b in arm.data.bones}
        result, faults = judge(measure(P, E.reshape(-1, 2), J))
        name = os.path.basename(given)
        print("PROPORTIONS", json.dumps({"file": name, "pass": not faults, "measures": result}), flush=True)
        if faults and theirs(body.name, body.data.name):
            print(f"PROPORTIONS as made {name}: {'; '.join(faults)}. {AS_MADE}", flush=True)
        elif faults:
            print(f"PROPORTIONS fail {name}: {'; '.join(faults)}. {ADVICE}", flush=True)


if __name__ == "__main__":
    main()
