"""Planted faults the character checks must catch, in Blender's Python (a check is a gate only
once a test plants a known fault and shows the check catches it, D632):

    tools/blender/run.sh tools/characters/test_checks.py

From the mannequin, a file in target/check/planted/, checked as the build checks it, in a
Blender of its own, by the line it prints (D388), seven at a time, each as its file is written:
  - rig_check.py on the mannequin re-exported as it is (the control): passes all seven checks.
    On a copy with one fault each, it fails the check named:
      weights    every twentieth vertex's weights taken off (the glTF exporter binds those
                 vertices to a neutral_bone of its own)
      skeleton   hand_l parented to upperarm_l
      rest_pose  spine_02 turned 10 degrees off spine_03, on a rig with twist bones (judged
                 only there); the same rig, every bone pointing at its child, is its control
      reach      the left forearm's skin driven by the right foot alone
      seams      one copy of each vertex split at a texture seam given a third of pelvis
      symmetry   the left side's skin given as much of spine_01 again
      motion     every tenth vertex bound to the root alone, so its edges stretch as he moves;
                 the skin 6 cm up off its skeleton, so he stands off the ground (by its
                 standing feet's gap, past its 5 cm); the soles 5 cm through the ground and
                 bound to the root, which the hips' lift does not raise (by its sink, past
                 the mannequin's by 2 cm)
    A fault fails other checks too (an unweighted vertex tears its seam); each must fail its own.
  - gear_check.py on the mannequin wearing a vest (the torso's skin 2 cm out, skinned as the
    body is): passes (1.7% through at worst, Hammer_Loop; the control). The same vest bound
    to the root bone alone, so the body moves through it: fails (29.8%, PickUp_Table).
A whole skin 3 mm out is no fault: it clips no more than the vest (1.9% at worst, 0.5% without
the hands, where inflated fingers cross). A vest sunk 2 cm into the chest is invisible by
design: gear_check watches only skin with the look outward of it at rest.
About three minutes (2 min 46 s on 2026-11-06, the machine's load at 50). Prints TESTCHECKS pass, or each failure; exits 1 on a failure.
"""

import json
import math
import os
import re
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bmesh
import bpy
from mathutils import Matrix

import lift
import look
import retarget
import twist

OUT = os.path.join(look.ROOT, "target", "check", "planted")
TORSO = {"pelvis", "spine_01", "spine_02", "spine_03"}
WORKERS = 7  # Blenders at once: the 13 checks in two rounds
os.makedirs(OUT, exist_ok=True)


def mannequin():
    look.reset()
    bpy.ops.import_scene.gltf(filepath=look.FIGURE)
    arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
    body = max((o for o in bpy.context.scene.objects if o.type == "MESH"), key=lambda o: len(o.data.vertices))
    for pb in arm.pose.bones:
        pb.matrix_basis.identity()
    if arm.animation_data:
        arm.animation_data.action = None
    return arm, body


def export(path, objects=None):
    if objects:
        bpy.ops.object.select_all(action="DESELECT")
        for o in objects:
            o.select_set(True)
    bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=bool(objects), export_animations=not objects)


def planted(name, change=None):
    """The mannequin with `change(arm, body)` made to it, as a file of its own."""
    arm, body = mannequin()
    if change:
        change(arm, body)
    path = os.path.join(OUT, f"{name}.glb")
    export(path)
    return path


def edit(arm):
    bpy.ops.object.select_all(action="DESELECT")
    arm.select_set(True)
    bpy.context.view_layer.objects.active = arm
    bpy.ops.object.mode_set(mode="EDIT")
    return arm.data.edit_bones


def unweighted(arm, body):
    some = list(range(0, len(body.data.vertices), 20))
    for g in body.vertex_groups:
        g.remove(some)


def reparented(arm, body):
    eb = edit(arm)
    eb["hand_l"].use_connect = False
    eb["hand_l"].parent = eb["upperarm_l"]
    bpy.ops.object.mode_set(mode="OBJECT")


def twisted(turn_deg):
    """Our converted rig's shape (convert_rig.py): every bone pointing at its child, twist bones
    halfway along each segment; then spine_02 turned `turn_deg` off its child."""
    def change(arm, body):
        children = retarget.main_children(arm)
        eb = edit(arm)
        retarget.orient(eb, children)
        heads = {n: (eb[seg].head + eb[children[seg]].head) / 2 for n, seg in twist.names() if seg in children}
        bpy.ops.object.mode_set(mode="OBJECT")
        twist.add(arm, heads)
        eb = edit(arm)
        b = eb["spine_02"]
        b.tail = b.head + Matrix.Rotation(math.radians(turn_deg), 3, "X") @ (b.tail - b.head)
        bpy.ops.object.mode_set(mode="OBJECT")
    return change


def main_bone(body, v):
    names = {g.index: g.name for g in body.vertex_groups}
    return names[max(v.groups, key=lambda g: g.weight).group] if v.groups else None


def only(body, verts, bone):
    """Each of `verts` driven by `bone` alone."""
    for g in body.vertex_groups:
        g.remove(verts)
    (body.vertex_groups.get(bone) or body.vertex_groups.new(name=bone)).add(verts, 1.0, "REPLACE")


def far(arm, body):
    only(body, [v.index for v in body.data.vertices if main_bone(body, v) == "lowerarm_l"], "foot_r")


def torn(arm, body):
    copies = {}
    for v in body.data.vertices:
        copies.setdefault(tuple(round(c, 5) for c in v.co), []).append(v.index)
    g = body.vertex_groups.get("pelvis") or body.vertex_groups.new(name="pelvis")
    g.add([vs[0] for vs in copies.values() if len(vs) > 1], 0.33, "ADD")


def lopsided(arm, body):
    mw = body.matrix_world
    g = body.vertex_groups.get("spine_01") or body.vertex_groups.new(name="spine_01")
    g.add([v.index for v in body.data.vertices if (mw @ v.co).x > 0.03], 1.0, "ADD")


def stretched(arm, body):
    only(body, list(range(0, len(body.data.vertices), 10)), "root")


def raised(arm, body):
    """The skin 6 cm up off its skeleton, skinned as it is: he stands that far off the ground
    (the lift never lowers a body, lift.py)."""
    for v in body.data.vertices:
        v.co.z += 0.06


def sunk(arm, body):
    """The soles (the feet's skin within 3 cm of the ground) 5 cm through it and bound to the
    root, which the hips' lift does not raise: an auto-rig that weights the floor-side skin to
    the root bone."""
    mw = body.matrix_world
    soles = [v.index for v in body.data.vertices if main_bone(body, v) in ("foot_l", "foot_r", "ball_l", "ball_r") and (mw @ v.co).z < 0.03]
    for i in soles:
        body.data.vertices[i].co.z -= 0.05
    only(body, soles, "root")


def shell(out_m, name, only=None, rigid=False):
    """The mannequin's skin `out_m` out along its normals, skinned as it is: a look. With
    `only`, just the skin driven mostly by those bones (a vest over the torso); `rigid`, bound
    to the root bone alone."""
    arm, body = mannequin()
    piece = body.copy()
    piece.data = body.data.copy()
    piece.name = name
    bpy.context.scene.collection.objects.link(piece)
    for v in piece.data.vertices:
        v.co += v.normal * out_m
    if only:
        names = {g.index: g.name for g in piece.vertex_groups}
        bm = bmesh.new()
        bm.from_mesh(piece.data)
        deform = bm.verts.layers.deform.active
        bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v[deform] or names[max(v[deform].items(), key=lambda t: t[1])[0]] not in only],
                         context="VERTS")
        bm.to_mesh(piece.data)
        bm.free()
    if rigid:
        # Every vertex bound to the root alone, so the look stays where it is at rest while the
        # body moves inside it: a garment whose weights were lost on the way out.
        for g in list(piece.vertex_groups):
            piece.vertex_groups.remove(g)
        piece.vertex_groups.new(name="root").add(list(range(len(piece.data.vertices))), 1.0, "REPLACE")
    path = os.path.join(OUT, f"{name}.glb")
    export(path, [arm, piece])
    return path


def run(script, *args):
    """A check run as tools/blender/run.sh runs it: its own lines, Blender's left out."""
    p = subprocess.run([bpy.app.binary_path, "--background", "--factory-startup", "--python", os.path.join(HERE, script), "--", *args],
                       capture_output=True, text=True, stdin=subprocess.DEVNULL)
    return [l for l in (p.stdout + p.stderr).splitlines() if re.match(r"^(✓|✗|  |GEARCHECKS|Traceback|Error)", l)]


# A rig_check case: the file's name, its change, the check it must fail (None: pass all, or
# with `passes` the one it must pass).
RIG = [
    ("rig-control", None, None),
    ("rig-unweighted", unweighted, "weights"),
    ("rig-reparented", reparented, "skeleton"),
    ("rig-pointed", twisted(0), None),
    ("rig-turned", twisted(10), "rest_pose"),
    ("rig-far", far, "reach"),
    ("rig-torn", torn, "seams"),
    ("rig-lopsided", lopsided, "symmetry"),
    ("rig-stretched", stretched, "motion"),
    ("rig-raised", raised, "motion:standing_foot_gap_m"),
    ("rig-sunk", sunk, "motion:sink_over_mannequin_m"),
]
# The part of motion a fault must fail by, its number in rig_check's report, and the limit it is held to.
PART_LIMIT = {"standing_foot_gap_m": "foot_gap_m", "sink_over_mannequin_m": "sink_over_m"}


def judged(name, want, lines):
    """What is wrong with rig_check's `lines` for the planted file `name`, or None."""
    first = lines[0] if lines else ""
    failed = re.match(rf"^✗ RIGCHECK {name}\.glb fails ([a-z_, ]+);", first)
    want, _, part = (want or "").partition(":")
    want = want or None
    if part and failed and want in failed.group(1).split(", "):
        # Failing motion is not enough: it must be by this part, past its limit.
        with open(os.path.join(HERE, "out", f"rig-check-{name}.json")) as f:
            result = json.load(f)
        got, limit = result["report"]["motion"].get(part), result["limits"][PART_LIMIT[part]]
        return None if got is not None and got > limit else f"rig_check failed motion on {name} but not by {part} ({got}, limit {limit}): {first}"
    if want is None and name == "rig-pointed":
        # Its motion is not this rig's to judge: only that the rest pose passes once judged.
        return None if not failed or "rest_pose" not in failed.group(1).split(", ") else f"rig_check failed rest_pose on every bone pointing at its child: {first}"
    if want is None:
        return None if first.startswith(f"✓ RIGCHECK {name}.glb") else f"rig_check failed the mannequin re-exported as it is: {' | '.join(lines[:3])}"
    if not failed or want not in failed.group(1).split(", "):
        return f"rig_check passed {want} on {name}: {' | '.join(lines[:3])}"
    return None


def main():
    lift.reference()  # measured once here, so the checks at once do not each measure it
    # Each check starts as its file is written, while the next is planted (bpy stays on this thread).
    with ThreadPoolExecutor(WORKERS) as pool:
        files = [(name, want, pool.submit(run, "rig_check.py", planted(name, change))) for name, change, want in RIG]
        vest = pool.submit(run, "gear_check.py", look.FIGURE, shell(0.02, "gear-vest", TORSO))
        rigid = pool.submit(run, "gear_check.py", look.FIGURE, shell(0.02, "gear-rigid-vest", TORSO, rigid=True))
        rigs = [f.result() for _, _, f in files]
        vest_lines, rigid_lines = vest.result(), rigid.result()
    failures = [f for (name, want, _), lines in zip(files, rigs) if (f := judged(name, want, lines))]
    if not any(re.match(r"^✓ GEARCHECK gear-vest on ", l) for l in vest_lines):
        failures.append("gear_check failed the control, a vest of the torso's skin 2 cm out skinned as the body is: " + " | ".join(vest_lines[:3]))
    if not any(re.match(r"^✗ GEARCHECK gear-rigid-vest on ", l) for l in rigid_lines):
        failures.append("gear_check passed a vest bound to the root alone, which the body moves through: " + " | ".join(rigid_lines[:3]))
    for lines in rigs + [vest_lines, rigid_lines]:
        print(lines[0][:200] if lines else "(no line)")
    print("TESTCHECKS pass" if not failures else "TESTCHECKS fail\n" + "\n".join(failures))
    if failures:
        sys.exit(1)


if __name__ == "__main__":
    main()
