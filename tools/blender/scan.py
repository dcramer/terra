"""Makes a scanned model, such as a Poly Haven rock or fern, fit to place by the hundred: each
piece cut to a triangle budget (Blender's collapse decimation, which keeps its texture places,
so its scanned colour, normal and roughness maps still fit), its normals smooth, its foot at
the origin and its middle over it, and written as a glTF binary of its own with its textures
in it.

    tools/blender/run.sh tools/blender/scan.py MODEL.gltf OUT_PREFIX BUDGET [--whole] [--loose] [--crease DEG]

Each separate object in the file is its own piece, as a set of rocks is, written to
OUT_PREFIX_1.glb, OUT_PREFIX_2.glb and so on; --whole keeps them as one, written to
OUT_PREFIX.glb. --loose takes each loose part as a piece instead, largest first, for a set
scanned as one object; crumbs are dropped. --crease shades smooth across curves and splits
where faces meet at more than so many degrees (docs/art-style.md), rather than smooth
everywhere. Prints SCAN with each piece's triangles before and after.
"""

import math
import os
import sys

import bpy

args = sys.argv[sys.argv.index("--") + 1 :]
whole = "--whole" in args
loose = "--loose" in args
crease = None
if "--crease" in args:
    at = args.index("--crease")
    crease = float(args[at + 1])
    del args[at : at + 2]
model, prefix, budget = [a for a in args if a not in ("--whole", "--loose")][:3]
budget = int(budget)

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=os.path.abspath(model))
meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
if not meshes:
    raise SystemExit("no meshes in " + model)

# Apply each object's place, so a piece's own frame is the world's.
for ob in meshes:
    ob.select_set(True)
bpy.context.view_layer.objects.active = meshes[0]
bpy.ops.object.parent_clear(type="CLEAR_KEEP_TRANSFORM")
bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
bpy.ops.object.select_all(action="DESELECT")

if whole and len(meshes) > 1:
    for ob in meshes:
        ob.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    bpy.ops.object.join()
    meshes = [bpy.context.view_layer.objects.active]
    bpy.ops.object.select_all(action="DESELECT")

if loose:
    # One object of every part, welded so a part split along its seams stays one.
    for ob in meshes:
        ob.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    if len(meshes) > 1:
        bpy.ops.object.join()
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.mesh.remove_doubles(threshold=0.0005)
    bpy.ops.mesh.separate(type="LOOSE")
    bpy.ops.object.mode_set(mode="OBJECT")
    parts = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    # Crumbs and chips are not pieces.
    for ob in [o for o in parts if len(o.data.vertices) <= 50]:
        bpy.data.objects.remove(ob)
    meshes = sorted(
        (o for o in bpy.context.scene.objects if o.type == "MESH"),
        key=lambda o: o.dimensions.length,
        reverse=True,
    )
    bpy.ops.object.select_all(action="DESELECT")


def triangles(ob):
    return sum(len(p.vertices) - 2 for p in ob.data.polygons)


for number, ob in enumerate(meshes, start=1):
    before = triangles(ob)
    # Scans are often split along their texture seams; welded, they can be cut as one.
    bpy.context.view_layer.objects.active = ob
    bpy.ops.object.select_all(action="DESELECT")
    ob.select_set(True)
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.mesh.remove_doubles(threshold=0.0005)
    bpy.ops.object.mode_set(mode="OBJECT")
    # One pass can stop short on a scan with many seams and loose pieces; a few more get it
    # to its budget.
    for _ in range(6):
        now = triangles(ob)
        if now <= budget * 1.05:
            break
        cut = ob.modifiers.new("cut", "DECIMATE")
        cut.decimate_type = "COLLAPSE"
        cut.ratio = budget / now
        cut.use_collapse_triangulate = True
        bpy.context.view_layer.objects.active = ob
        bpy.ops.object.modifier_apply(modifier=cut.name)
    if crease is None:
        for poly in ob.data.polygons:
            poly.use_smooth = True
    else:
        bpy.ops.object.select_all(action="DESELECT")
        ob.select_set(True)
        bpy.context.view_layer.objects.active = ob
        bpy.ops.object.shade_smooth_by_angle(angle=math.radians(crease), keep_sharp_edges=True)
    # Foot at the origin, the middle over it: the lowest point of the piece on the ground.
    low = min(v.co.z for v in ob.data.vertices)
    xs = [v.co.x for v in ob.data.vertices]
    ys = [v.co.y for v in ob.data.vertices]
    middle = ((min(xs) + max(xs)) / 2, (min(ys) + max(ys)) / 2)
    for v in ob.data.vertices:
        v.co.x -= middle[0]
        v.co.y -= middle[1]
        v.co.z -= low
    ob.location = (0, 0, 0)

    bpy.ops.object.select_all(action="DESELECT")
    ob.select_set(True)
    out = f"{prefix}.glb" if whole else f"{prefix}_{number}.glb"
    os.makedirs(os.path.dirname(os.path.abspath(out)), exist_ok=True)
    bpy.ops.export_scene.gltf(
        filepath=os.path.abspath(out),
        export_format="GLB",
        use_selection=True,
        export_yup=True,
        export_apply=True,
    )
    size = ob.dimensions
    print(f"SCAN {out}: {before} triangles, now {triangles(ob)}; {size.x:.2f} by {size.y:.2f} m, {size.z:.2f} m tall")
