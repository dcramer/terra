"""Re-exports a Tripo model as a plain glTF the game can read, its textures kept: optionally
cut to a budget of triangles, normals smooth across curves and split at creases."""
import math
import sys

import bpy
import numpy as np
from mathutils import Matrix, Vector

args = sys.argv[sys.argv.index("--") + 1:]
src, out = args[0], args[1]
budget = int(args[2]) if len(args) > 2 else 0

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=src)
meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
for o in meshes:
    bpy.context.view_layer.objects.active = o
    o.select_set(True)
    tris = sum(len(p.vertices) - 2 for p in o.data.polygons)
    if budget and tris > budget:
        cut = o.modifiers.new("Reduce", "DECIMATE")
        cut.ratio = budget / tris
        bpy.ops.object.modifier_apply(modifier=cut.name)
    # Laid along its length: most spread along X, least up, middle on the origin.
    me = o.data
    me.transform(o.matrix_world)
    o.matrix_world = Matrix.Identity(4)
    pts = np.array([v.co for v in me.vertices])
    centre = pts.mean(axis=0)
    _, _, axes = np.linalg.svd(pts - centre, full_matrices=False)
    length, _, thin = (Vector(axis) for axis in axes)
    me.transform(Matrix((length, thin.cross(length), thin)).to_4x4() @ Matrix.Translation(-Vector(centre)))
    pts = np.array([v.co for v in me.vertices])
    me.transform(Matrix.Translation(-Vector((pts.min(axis=0) + pts.max(axis=0)) / 2)))
    bpy.ops.object.shade_smooth_by_angle(angle=math.radians(50), keep_sharp_edges=True)
    print("TRIANGLES", o.name, sum(len(p.vertices) - 2 for p in o.data.polygons))
bpy.ops.export_scene.gltf(filepath=out, export_format="GLB")
