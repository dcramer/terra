"""Planted faults the character checks must catch, in Blender's Python (a check is a gate only
once a test plants a known fault and shows the check catches it):

    tools/blender/run.sh tools/characters/test_checks.py

From the mannequin, a file in target/check/planted/, checked as the build checks it, in a
Blender of its own, by the line it prints (D388):
  - rig_check.py on the mannequin with every twentieth vertex's weights taken off: fails
    `weights`, naming it (the glTF exporter binds those vertices to a neutral_bone of its own).
gear_check.py has no planted fault yet. A whole skin 3 mm out, skinned as the body is, is no
fault: it clips no more than a vest of the torso's skin 2 cm out (1.9% of its skin at worst
against the vest's 1.7%; 0.5% without the hands, where inflated fingers cross). The whole skin
6 cm out is no control: inflating along the normals merges the fingers. `shell` makes both,
and the vest (`only=TORSO`), for the fault still to plant.
About two minutes. Prints TESTCHECKS pass, or each failure; exits 1 on a failure.
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


def shell(out_m, name, only=None):
    """The mannequin's skin `out_m` out along its normals, skinned as it is: a look. With
    `only`, just the skin driven mostly by those bones (a vest over the torso)."""
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
    print("TESTCHECKS pass" if not failures else "TESTCHECKS fail\n" + "\n".join(failures))
    if failures:
        sys.exit(1)


if __name__ == "__main__":
    main()
