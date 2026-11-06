"""Planted faults the character checks must catch, in Blender's Python (a check is a gate only
once a test plants a known fault and shows the check catches it):

    tools/blender/run.sh tools/characters/test_checks.py

From the mannequin, a file in target/check/planted/, checked as the build checks it, in a
Blender of its own, by the line it prints (D388):
  - rig_check.py on the mannequin with every twentieth vertex's weights taken off: fails
    `weights`, naming it (the glTF exporter binds those vertices to a neutral_bone of its own).
  - gear_check.py on the mannequin wearing a vest (the torso's skin 2 cm out, skinned as the
    body is): passes (1.7% through at worst, Hammer_Loop; the control). The same vest bound
    to the root bone alone, so the body moves through it: fails (29.8%, PickUp_Table).
A whole skin 3 mm out is no fault: it clips no more than the vest (1.9% at worst, 0.5% without
the hands, where inflated fingers cross). A vest sunk 2 cm into the chest is invisible by
design: gear_check watches only skin with the look outward of it at rest.
About four minutes. Prints TESTCHECKS pass, or each failure; exits 1 on a failure.
"""

import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bmesh
import bpy

import look

OUT = os.path.join(look.ROOT, "target", "check", "planted")
TORSO = {"pelvis", "spine_01", "spine_02", "spine_03"}
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


def unweighted():
    arm, body = mannequin()
    some = list(range(0, len(body.data.vertices), 20))
    for g in body.vertex_groups:
        g.remove(some)
    path = os.path.join(OUT, "rig-unweighted.glb")
    export(path)
    return path


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


def main():
    failures = []
    rig = run("rig_check.py", unweighted())
    if not any(re.match(r"^✗ RIGCHECK rig-unweighted\.glb fails .*\bweights\b", l) for l in rig):
        failures.append("rig_check passed weights on a mannequin with every twentieth vertex unweighted: " + " | ".join(rig[:3]))
    vest = run("gear_check.py", look.FIGURE, shell(0.02, "gear-vest", TORSO))
    if not any(re.match(r"^✓ GEARCHECK gear-vest on ", l) for l in vest):
        failures.append("gear_check failed the control, a vest of the torso's skin 2 cm out skinned as the body is: " + " | ".join(vest[:3]))
    rigid = run("gear_check.py", look.FIGURE, shell(0.02, "gear-rigid-vest", TORSO, rigid=True))
    if not any(re.match(r"^✗ GEARCHECK gear-rigid-vest on ", l) for l in rigid):
        failures.append("gear_check passed a vest bound to the root alone, which the body moves through: " + " | ".join(rigid[:3]))
    for l in vest[:1] + rigid[:1]:
        print(l)
    print("TESTCHECKS pass" if not failures else "TESTCHECKS fail\n" + "\n".join(failures))
    if failures:
        sys.exit(1)


if __name__ == "__main__":
    main()
