"""Cuts a model's meshes to a triangle budget and writes them as a glTF binary, for a tool to
splice the new mesh into the original file (tools/iso-art's `decimate`): the skeleton, its clips
and the textures stay the original's, so nothing a glTF round trip through Blender could change
(bone frames, clip names, the root node) is taken from this file but the mesh.

  tools/blender/run.sh tools/blender/decimate.py IN.glb OUT.glb BUDGET

Collapse decimation (pieces.py's `cut`), which keeps UVs and interpolates vertex groups, so the
weights follow the cut. On a skinned mesh the Decimate goes first in the stack, ahead of the
Armature, so it cuts the rest pose; the Armature stays for the exporter's skin. BUDGET is the
whole file's: each mesh gets its share by its triangles. Prints the triangles before and after."""

import os
import sys

import bpy

from pieces import only, triangles


def cut_first(ob, budget):
    """As pieces.py's `cut`, but under the budget, never 5% over it, and the Decimate applied
    first in the stack, so a skinned mesh is cut in its rest pose."""
    for _ in range(8):
        now = triangles(ob)
        if now <= budget:
            return
        mod = ob.modifiers.new("cut", "DECIMATE")
        mod.decimate_type = "COLLAPSE"
        mod.ratio = budget * 0.99 / now
        mod.use_collapse_triangulate = True
        only(ob)
        bpy.ops.object.modifier_move_to_index(modifier=mod.name, index=0)
        bpy.ops.object.modifier_apply(modifier=mod.name)
    raise SystemExit(f"decimate: {ob.name} is still {triangles(ob)} triangles after 8 cuts, over {budget}")


def main(argv):
    src, out, budget = os.path.abspath(argv[0]), os.path.abspath(argv[1]), int(argv[2])
    for ob in list(bpy.data.objects):  # the factory scene's cube, camera and light
        bpy.data.objects.remove(ob)
    bpy.ops.import_scene.gltf(filepath=src)
    meshes = [ob for ob in bpy.context.scene.objects if ob.type == "MESH"]
    before = sum(triangles(ob) for ob in meshes)
    for ob in meshes:
        cut_first(ob, int(budget * triangles(ob) / before))
    after = sum(triangles(ob) for ob in meshes)
    bpy.ops.export_scene.gltf(filepath=out, export_format="GLB", export_animations=False, export_apply=False, export_yup=True)
    print(f"decimate: {src} {before} to {after} triangles (budget {budget}), {len(meshes)} meshes, to {out}")


if __name__ == "__main__":
    main(sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else sys.argv[1:])
